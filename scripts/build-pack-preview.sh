#!/usr/bin/env bash
# エンジンの配信物を同梱した fandhe-pack（`preview` が URL を出せるビルド）を作り、実行ファイルのパスを標準出力へ
# 1 行だけ書く（`make build-pack-preview`・scripts/build-pack-release.sh・scripts/test-browser.sh から呼ばれる。TASK-429 2/3）。
#
# 目的: 作成者の端末に Rust も Node も無くても `fandhe-pack preview` が動くよう、web の配信物（6 ファイル）を
# cargo の feature `embed-web` で実行ファイルへ同梱する。同梱の元は dist/web-embed/ に決め打ち（crates/pack/src/preview.rs の
# include_bytes! と対応する）。手順の実体はここ 1 か所に置く。
#
# 処理の順（どこで失敗しても実行ファイルを出さない。fail-closed）
#   1. 同梱の元を作る: --web-dir（IR_FILE つきの配信する設定の出力）を写す。無ければ仮の IR を IR_FILE に渡して
#      tools/build-web.sh を流す。仮の IR は fandhe-ir の meta を出させるためだけに使い、中身は同梱しない
#   2. dist/web-embed/ir/ を消し、index.html に fandhe-ir の meta がちょうど 1 個・試験用の口が無いことを確かめる
#   3. tools/check-delivery.sh と scripts/check-no-egress.sh dist で試験用の口・通信の手段が無いことを確かめる
#   4. cargo build --features embed-web
#   5. 実行ファイルを一時の置き場へ写してそのパスを出す（feature なしのビルドと出力先が同じなので、取り違えを防ぐ）
#
# 使い方: scripts/build-pack-preview.sh [--release] [--target <triple>] [--web-dir <dir>] [--out <file>]
#   --out  実行ファイルの写し先（既定: mktemp -d の下。呼び出し側が消す）
# 終了コード: 0 = 成功 / 1 = 検査・ビルドの失敗 / 2 = 誤用・道具が無い
# 回帰テスト: scripts/test-build-pack-preview.sh
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

usage() {
  echo "usage: $(basename "$0") [--release] [--target <triple>] [--web-dir <dir>] [--out <file>]" >&2
  exit 2
}

RELEASE=0
TARGET=""
WEB_DIR=""
OUT=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --release)
      RELEASE=1
      shift
      ;;
    --target)
      [ "$#" -ge 2 ] || usage
      TARGET="$2"
      shift 2
      ;;
    --web-dir)
      [ "$#" -ge 2 ] || usage
      WEB_DIR="$2"
      shift 2
      ;;
    --out)
      [ "$#" -ge 2 ] || usage
      OUT="$2"
      shift 2
      ;;
    *) usage ;;
  esac
done
# 値はそのまま cargo の引数になるため、許可リストで照合する
case "${TARGET}" in
  "" | aarch64-apple-darwin | x86_64-unknown-linux-gnu) ;;
  *)
    echo "error: unsupported --target: ${TARGET}" >&2
    exit 2
    ;;
esac
if [ -n "${WEB_DIR}" ] && [ ! -d "${WEB_DIR}" ]; then
  echo "error: --web-dir is not a directory: ${WEB_DIR}" >&2
  exit 2
fi
if [ -n "${WEB_DIR}" ]; then
  case "${WEB_DIR}" in /*) ;; *) WEB_DIR="$(pwd)/${WEB_DIR}" ;; esac
fi
if [ -n "${OUT}" ]; then
  case "${OUT}" in /*) ;; *) OUT="$(pwd)/${OUT}" ;; esac
fi
for c in cargo jq mktemp node; do
  require_cmd "${c}" || exit 2
done

cd "${ROOT_DIR}"
EMBED="${ROOT_DIR}/dist/web-embed"
TMP_DIR="$(mktemp -d)"
KEEP_TMP=0
cleanup() {
  [ "${KEEP_TMP}" -eq 1 ] || rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

# 1. 同梱の元
rm -rf "${EMBED}"
mkdir -p "${EMBED}"
if [ -n "${WEB_DIR}" ]; then
  cp -R "${WEB_DIR}/." "${EMBED}/"
else
  printf '{}\n' >"${TMP_DIR}/placeholder-ir.json"
  PROFILE=debug
  [ "${RELEASE}" -eq 1 ] && PROFILE=release
  log "building the web files (IR_FILE is a placeholder; it is not embedded)"
  (TEST_HOOKS="" IR_FILE="${TMP_DIR}/placeholder-ir.json" OUT_DIR="${EMBED}" "${ROOT_DIR}/tools/build-web.sh" "${PROFILE}") >&2
fi

# 2. 同梱の形の検査
rm -rf "${EMBED}/ir"
for f in index.html app.js app.css icon.svg wasm/fandhe_workshop_web.js wasm/fandhe_workshop_web_bg.wasm; do
  [ -f "${EMBED}/${f}" ] || {
    echo "error: missing ${f} in dist/web-embed" >&2
    exit 1
  }
done
metas="$(grep -c 'name="fandhe-ir"' "${EMBED}/index.html" || true)"
if [ "${metas}" != "1" ]; then
  echo "error: index.html must have exactly one fandhe-ir meta (found ${metas})" >&2
  exit 1
fi
if grep -q 'fandhe-test-hooks\|test-hooks\.js' "${EMBED}/index.html" || [ -e "${EMBED}/test-hooks.js" ]; then
  echo "error: the web files must be the delivery configuration (no test hooks)" >&2
  exit 1
fi

# 3. 試験用の口・通信の手段が無いこと
"${ROOT_DIR}/tools/check-delivery.sh" "${EMBED}" >&2
"${ROOT_DIR}/scripts/check-no-egress.sh" dist "${EMBED}" >&2

# 4. ビルド
ARGS=(build --locked -p fandhe-workshop-pack --bin fandhe-pack --features embed-web --message-format=json-render-diagnostics)
[ "${RELEASE}" -eq 1 ] && ARGS+=(--release)
[ -n "${TARGET}" ] && ARGS+=(--target "${TARGET}")
log "building fandhe-pack with embed-web"
messages="$(cargo "${ARGS[@]}")"
bin="$(printf '%s\n' "${messages}" | jq -r 'select(.reason == "compiler-artifact" and .target.name == "fandhe-pack" and .executable != null) | .executable')"
if [ -z "${bin}" ] || [ "$(printf '%s\n' "${bin}" | wc -l | tr -d ' ')" != "1" ] || [ ! -x "${bin}" ]; then
  echo "error: could not resolve a single fandhe-pack executable from cargo output" >&2
  exit 1
fi

# 5. 写して出す
if [ -z "${OUT}" ]; then
  OUT="${TMP_DIR}/fandhe-pack"
  KEEP_TMP=1
fi
cp "${bin}" "${OUT}"
chmod 0755 "${OUT}"
printf '%s\n' "${OUT}"
