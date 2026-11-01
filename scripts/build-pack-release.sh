#!/usr/bin/env bash
# fandhe-pack の配布用の実行ファイルを作る（`make build-pack-release` と release.yml から呼ばれる。TASK-415 1/2）。
#
# 目的: 教材の作成者に Rust の導入を求めない（TASK-414 の決定）。OS・CPU ごとの実行ファイルを 1 つ作り、
# アーキテクチャの検査・Rust の無い環境での煙試験（AUTHOR-14）・梱包・sha256 まで行う。
# 呼び出し元は release.yml の build ジョブと人の手元。署名・公証はしない（D-71 と同じ方針。案内文は TASK-415 2/2）。
#
# 処理の順（どの段階の失敗も成果物を出さない。fail-closed）
#   1. ビルド（--bin を渡したときは省く）
#   2. アーキテクチャの検査: file の出力が --target の期待と合うこと
#   3. 煙試験: PATH を絞り cargo・rustc が見えない環境で、例の教材に validate と test を流して 0 で終わること
#   4. 梱包: <out>/fandhe-pack-<version>-<target>.tar.gz と同名の .sha256
#
# 使い方: scripts/build-pack-release.sh [--target <triple>] [--out <dir>] [--bin <prebuilt>]
#   --target  aarch64-apple-darwin（既定。配布するのはこれだけ）か x86_64-unknown-linux-gnu
#             （Linux の手元でこのスクリプト自体を通すための確認用。配布しない）
#   --out     出力先（既定 dist/pack-release。dist/ は git 管理外）
#   --bin     ビルド済みの実行ファイル。ビルドを省き、検査と梱包だけを行う（回帰テスト用）
# 環境変数 SMOKE_PATH: 煙試験の PATH（既定 /usr/bin:/bin。回帰テストが cargo の見える PATH を渡して失敗を確かめる）
# 終了コード: 0 = 成功 / 1 = 検査の失敗 / 2 = 誤用・道具が無い
# 回帰テスト: scripts/test-build-pack-release.sh
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

usage() {
  echo "usage: $(basename "$0") [--target <triple>] [--out <dir>] [--bin <prebuilt>]" >&2
  exit 2
}

TARGET="aarch64-apple-darwin"
OUT_DIR="${ROOT_DIR}/dist/pack-release"
PREBUILT=""
while [ "$#" -gt 0 ]; do
  [ "$#" -ge 2 ] || usage
  case "$1" in
    --target) TARGET="$2" ;;
    --out) OUT_DIR="$2" ;;
    --bin) PREBUILT="$2" ;;
    *) usage ;;
  esac
  shift 2
done

# 許可リストで照合する。値はそのまま cargo の引数になるため、リスト外は受け付けない
case "${TARGET}" in
  aarch64-apple-darwin)
    WANT_FORMAT="Mach-O"
    WANT_ARCH="arm64"
    ;;
  x86_64-unknown-linux-gnu)
    WANT_FORMAT="ELF"
    WANT_ARCH="x86-64"
    ;;
  *)
    echo "error: unsupported --target: ${TARGET}" >&2
    exit 2
    ;;
esac
if [ -n "${PREBUILT}" ] && [ ! -f "${PREBUILT}" ]; then
  echo "error: --bin is not a file: ${PREBUILT}" >&2
  exit 2
fi
# 以降の起動は ROOT_DIR へ cd した子シェルで行うため、呼び出し時の cwd 基準の絶対パスに解決する。
# スラッシュ無しの指定も ./ を付けた経路にして、PATH 検索にならないようにする
if [ -n "${PREBUILT}" ]; then
  case "${PREBUILT}" in
    /*) ;;
    *) PREBUILT="$(pwd)/${PREBUILT}" ;;
  esac
fi
if [ -n "${OUT_DIR}" ]; then
  case "${OUT_DIR}" in
    /*) ;;
    *) OUT_DIR="$(pwd)/${OUT_DIR}" ;;
  esac
fi

# cargo はビルドの有無によらず版の取得（cargo metadata）に使う
for c in tar file mktemp jq cargo; do
  require_cmd "${c}" || exit 2
done
if command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256 "$1" | awk '{print $1}'; }
elif command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | awk '{print $1}'; }
else
  echo "error: neither shasum nor sha256sum is on PATH" >&2
  exit 2
fi

[ -d "${ROOT_DIR}/docs/examples/minimal-pack" ] || {
  echo "error: smoke-test pack not found: docs/examples/minimal-pack" >&2
  exit 2
}

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

META="$(cd "${ROOT_DIR}" && cargo metadata --no-deps --format-version 1 --locked)"

# 1. ビルド
if [ -z "${PREBUILT}" ]; then
  log "building fandhe-pack for ${TARGET}"
  (cd "${ROOT_DIR}" && cargo build --locked --release -p fandhe-workshop-pack --bin fandhe-pack --target "${TARGET}") >&2
  BIN="$(printf '%s' "${META}" | jq -r '.target_directory')/${TARGET}/release/fandhe-pack"
else
  BIN="${PREBUILT}"
fi
[ -f "${BIN}" ] || {
  echo "error: executable not found: ${BIN}" >&2
  exit 1
}

# 2. アーキテクチャの検査。runner のラベルが別の CPU に移っても、黙って違うものを配らないための関門
desc="$(file -b "${BIN}")"
case "${desc}" in
  *"${WANT_FORMAT}"*"${WANT_ARCH}"*) ;;
  *)
    echo "error: executable does not match ${TARGET} (expected ${WANT_FORMAT} ${WANT_ARCH}): ${desc}" >&2
    exit 1
    ;;
esac
log "file: ${desc}"
# 依存の記録だけ（判定には使わない）。macOS は libSystem だけになる想定
if command -v otool >/dev/null 2>&1; then
  otool -L "${BIN}" >&2 || true
fi

# 3. 煙試験。絶対パスで起動し、cargo・rustc が見えない環境であることを先に確かめる
SMOKE_PATH="${SMOKE_PATH:-/usr/bin:/bin}"
SMOKE_HOME="${TMP_DIR}/home"
mkdir -p "${SMOKE_HOME}"
for tool in cargo rustc; do
  if env -i PATH="${SMOKE_PATH}" HOME="${SMOKE_HOME}" /bin/sh -c "command -v ${tool}" >/dev/null 2>&1; then
    echo "error: ${tool} is visible on the smoke-test PATH; the test must run without Rust" >&2
    exit 1
  fi
done
for sub in validate test; do
  rc=0
  (cd "${ROOT_DIR}" && env -i PATH="${SMOKE_PATH}" HOME="${SMOKE_HOME}" "${BIN}" "${sub}" docs/examples/minimal-pack) >"${TMP_DIR}/smoke.out" 2>&1 || rc=$?
  if [ "${rc}" -ne 0 ]; then
    echo "error: smoke test failed: fandhe-pack ${sub} docs/examples/minimal-pack (exit ${rc})" >&2
    sed 's/^/  | /' "${TMP_DIR}/smoke.out" >&2
    exit 1
  fi
  log "smoke ok: ${sub}"
done

# 4. 梱包
VERSION="$(printf '%s' "${META}" | jq -r '.packages[] | select(.name == "fandhe-workshop-pack") | .version')"
if [ -z "${VERSION}" ] || [ "${VERSION}" = "null" ]; then
  echo "error: cannot read the pack crate version" >&2
  exit 1
fi
NAME="fandhe-pack-${VERSION}-${TARGET}"
STAGE="${TMP_DIR}/stage/${NAME}"
mkdir -p "${STAGE}" "${TMP_DIR}/pack"
cp "${BIN}" "${STAGE}/fandhe-pack"
chmod 0755 "${STAGE}/fandhe-pack"
cp "${ROOT_DIR}/LICENSE-MIT" "${ROOT_DIR}/LICENSE-APACHE" "${STAGE}/"
# アーカイブと .sha256 は一時領域で完成させてから出力先へ置く（途中の失敗で不完全な成果物・既存の成果物の切り詰めを出さない）。
# COPYFILE_DISABLE は macOS の tar が ._* のメタデータ項目を混ぜるのを防ぐ
WORK_ARCHIVE="${TMP_DIR}/pack/${NAME}.tar.gz"
COPYFILE_DISABLE=1 tar -czf "${WORK_ARCHIVE}" -C "${TMP_DIR}/stage" "${NAME}"
HASH="$(sha256 "${WORK_ARCHIVE}")"
printf '%s  %s\n' "${HASH}" "${NAME}.tar.gz" >"${WORK_ARCHIVE}.sha256"
mkdir -p "${OUT_DIR}"
ARCHIVE="${OUT_DIR}/${NAME}.tar.gz"
# 確定は「以前の組を退避 → 2 つを置く」の順で行う。途中（2 つ目の mv 等）で失敗したら、今回置いた分を消して
# 以前の組（無ければ何も無い状態）へ戻す。新アーカイブと古い .sha256 が食い違って残らないようにする
PREV_DIR="${TMP_DIR}/prev"
mkdir -p "${PREV_DIR}"
HAD_PREV=0
if [ -f "${ARCHIVE}" ] && [ -f "${ARCHIVE}.sha256" ]; then
  cp -p "${ARCHIVE}" "${PREV_DIR}/archive"
  cp -p "${ARCHIVE}.sha256" "${PREV_DIR}/sha256"
  HAD_PREV=1
fi
COMMITTED=0
rollback() {
  if [ "${COMMITTED}" -eq 0 ]; then
    rm -f "${ARCHIVE}" "${ARCHIVE}.sha256"
    if [ "${HAD_PREV}" -eq 1 ]; then
      cp -p "${PREV_DIR}/archive" "${ARCHIVE}" || true
      cp -p "${PREV_DIR}/sha256" "${ARCHIVE}.sha256" || true
    fi
  fi
  rm -f "${ARCHIVE}.sha256.tmp" "${ARCHIVE}.tmp"
  rm -rf "${TMP_DIR}"
}
trap rollback EXIT
cp "${WORK_ARCHIVE}" "${ARCHIVE}.tmp"
cp "${WORK_ARCHIVE}.sha256" "${ARCHIVE}.sha256.tmp"
mv "${ARCHIVE}.tmp" "${ARCHIVE}"
mv "${ARCHIVE}.sha256.tmp" "${ARCHIVE}.sha256"
COMMITTED=1

SIZE="$(wc -c <"${ARCHIVE}" | tr -d ' ')"
echo "archive: ${ARCHIVE}"
echo "bytes: ${SIZE}"
echo "sha256: ${HASH}"
