#!/usr/bin/env bash
# scripts/build-pack-release.sh 自体の回帰テスト（`make test-build-pack-release` と ci.yml の check ジョブから呼ばれる。TASK-415 1/2）。
#
# ビルドはしない（速く保つ）。--bin に偽の実行ファイル（validate・test に 0 か 1 を返すシェルスクリプト）を渡し、
# file の偽物を PATH の先頭に置いて Mach-O arm64 などのふりをさせる。実ビルドの確認は release.yml の run と手元で行う。
# 確かめること: 梱包の中身と mode・sha256 の照合、アーキテクチャ違い・煙試験の失敗・Rust が見える PATH の失敗（1）、
# 誤用（2）、失敗のときに成果物が出ないこと（fail-closed）。
# 作業ツリーは変えない。ネットワークは使わない。
#
# 出力は TAP 風（scripts/test-check-fork-ci.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
for c in mktemp mkdir rm tar file jq cargo; do
  require_cmd "${c}" || exit 1
done

BUILD="${DIR}/build-pack-release.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0

expect() {
  if [ "$2" = "$3" ]; then
    echo "ok - $1"
    passed=$((passed + 1))
  else
    echo "not ok - $1 (expected '$2', got '$3')"
    failed=$((failed + 1))
  fi
}

# 偽の実行ファイル。自分と同じ場所に fail-<サブコマンド> があるときだけ 1 を返す
# （煙試験は env -i で環境変数を消すため、失敗の指定は環境変数では渡せない）
FAKE_BIN="${TMP_DIR}/fandhe-pack"
cat >"${FAKE_BIN}" <<'EOF'
#!/bin/sh
[ ! -e "$(dirname "$0")/fail-$1" ] || exit 1
case "$1" in
  preview)
    # 偽の配信。URL の行を出して待つ（煙試験が止める）。no-url があるときは URL を出さずに終わる
    [ ! -e "$(dirname "$0")/no-url" ] || exit 0
    echo "OK: fake"
    echo "URL: http://127.0.0.1:1/preview/"
    exec sleep 30
    ;;
  compile) echo '{}' ;;
esac
exit 0
EOF
chmod 0755 "${FAKE_BIN}"

# file の偽物。実際の file は PATH の後ろにあっても、-b の出力だけ FAKE_FILE_DESC に置き換える
SHIM="${TMP_DIR}/shim"
mkdir -p "${SHIM}"
cat >"${SHIM}/file" <<'EOF'
#!/bin/sh
echo "${FAKE_FILE_DESC}"
EOF
chmod +x "${SHIM}/file"

# curl の偽物。URL の末尾で本文と content-type を決める。SHIM の ir-differs があるときは IR だけ違う本文を返す
cat >"${SHIM}/curl" <<'EOF'
#!/bin/sh
out=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift ;;
    -w | --noproxy | --max-time) shift ;;
    -*) ;;
    *) url="$1" ;;
  esac
  shift
done
case "${url}" in
  *.wasm) body="w"; ct="application/wasm" ;;
  *ir/main.json)
    ct="application/json"
    if [ -e "$(dirname "$0")/ir-differs" ]; then body="{x}"; else body="{}"; fi
    ;;
  *) body="<html>"; ct="text/html; charset=utf-8" ;;
esac
printf '%s\n' "${body}" >"${out}"
printf '%s' "${ct}"
EOF
chmod +x "${SHIM}/curl"

MACHO="Mach-O 64-bit executable arm64"
OUT="${TMP_DIR}/out"

# run_build <args...>: 終了コードを RC、標準エラーを ERR に残す
run_build() {
  RC=0
  PATH="${SHIM}:${PATH}" FAKE_FILE_DESC="${FAKE_FILE_DESC:-${MACHO}}" "${BUILD}" "$@" >"${TMP_DIR}/stdout" 2>"${TMP_DIR}/stderr" || RC=$?
}

# 1. 合格
rm -rf "${OUT}"
run_build --out "${OUT}" --bin "${FAKE_BIN}"
expect "passes with an arm64 Mach-O fake" 0 "${RC}"
ARCHIVE="$(find "${OUT}" -name 'fandhe-pack-*-aarch64-apple-darwin.tar.gz' | head -n 1)"
if [ -n "${ARCHIVE}" ]; then
  members="$(tar -tzf "${ARCHIVE}" | sed 's|^[^/]*/||' | grep -v '^$' | sort | tr '\n' ' ')"
  expect "archive holds the executable, both licenses and the notice" "LICENSE-APACHE LICENSE-MIT README.txt fandhe-pack " "${members}"
  X="${TMP_DIR}/x"
  mkdir -p "${X}"
  tar -xzf "${ARCHIVE}" -C "${X}"
  rc=0
  cmp -s "${X}"/*/README.txt "${DIR}/../tools/pack-release/README.txt" || rc=$?
  expect "bundled README.txt equals the notice source" 0 "${rc}"
  for needle in "xattr -d com.apple.quarantine fandhe-pack" "shasum -a 256 -c"; do
    rc=0
    grep -qF -- "${needle}" "${X}"/*/README.txt || rc=$?
    expect "notice contains: ${needle}" 0 "${rc}"
  done
  mode="$(find "${X}" -name fandhe-pack -type f -perm 755 | wc -l | tr -d ' ')"
  expect "executable has mode 755" 1 "${mode}"
  rc=0
  (cd "${OUT}" && if command -v shasum >/dev/null 2>&1; then shasum -a 256 -c "$(basename "${ARCHIVE}").sha256"; else sha256sum -c "$(basename "${ARCHIVE}").sha256"; fi) >/dev/null 2>&1 || rc=$?
  expect "sha256 file verifies" 0 "${rc}"
  grep -q '^sha256: ' "${TMP_DIR}/stdout" && rc=0 || rc=1
  expect "stdout reports the sha256" 0 "${rc}"
else
  expect "archive was created" "yes" "no"
fi

# 2. 検査の失敗（1）。成果物が出ないことも確かめる
fail_case() {
  local name="$1"
  shift
  rm -rf "${OUT}"
  run_build "$@"
  expect "${name} exits 1" 1 "${RC}"
  n=0
  [ ! -d "${OUT}" ] || n="$(find "${OUT}" -type f | wc -l | tr -d ' ')"
  expect "${name} leaves no artifact" 0 "${n}"
}
FAKE_FILE_DESC="Mach-O 64-bit executable x86_64" fail_case "wrong architecture" --out "${OUT}" --bin "${FAKE_BIN}"
FAKE_FILE_DESC="ELF 64-bit LSB executable, ARM aarch64" fail_case "wrong format" --out "${OUT}" --bin "${FAKE_BIN}"
for sub in validate test preview; do
  : >"${TMP_DIR}/fail-${sub}"
  fail_case "failing ${sub} smoke test" --out "${OUT}" --bin "${FAKE_BIN}"
  rm -f "${TMP_DIR}/fail-${sub}"
done

# preview の煙試験の失敗（URL が出ない・IR が compile と違う）
: >"${TMP_DIR}/no-url"
fail_case "preview prints no URL" --out "${OUT}" --bin "${FAKE_BIN}"
rm -f "${TMP_DIR}/no-url"
: >"${SHIM}/ir-differs"
fail_case "preview IR differs from compile" --out "${OUT}" --bin "${FAKE_BIN}"
rm -f "${SHIM}/ir-differs"

# cargo が見える PATH で煙試験を流すと失敗する（Rust の無い環境、という前提を保つ）
RUSTY="${TMP_DIR}/rusty"
mkdir -p "${RUSTY}"
printf '#!/bin/sh\nexit 0\n' >"${RUSTY}/cargo"
chmod +x "${RUSTY}/cargo"
SMOKE_PATH="${RUSTY}:/usr/bin:/bin" fail_case "cargo visible on the smoke PATH" --out "${OUT}" --bin "${FAKE_BIN}"

# 3. 誤用（2）
rm -rf "${OUT}"
run_build --unknown x
expect "unknown option exits 2" 2 "${RC}"
run_build --target x86_64-pc-windows-msvc --out "${OUT}" --bin "${FAKE_BIN}"
expect "unsupported target exits 2" 2 "${RC}"
run_build --out "${OUT}" --bin "${TMP_DIR}/none"
expect "missing --bin file exits 2" 2 "${RC}"
run_build --out
expect "option without a value exits 2" 2 "${RC}"

# 案内文の元が無いときは成果物を出さない。元は ROOT_DIR 固定のため、写しのツリーで確かめる
NT="${TMP_DIR}/nt"
mkdir -p "${NT}/scripts" "${NT}/docs"
cp "${DIR}/build-pack-release.sh" "${DIR}/lib.sh" "${NT}/scripts/"
cp -R "${DIR}/../docs/examples" "${NT}/docs/examples"
cp "${DIR}/../LICENSE-MIT" "${DIR}/../LICENSE-APACHE" "${NT}/"
rm -rf "${OUT}"
RC=0
PATH="${SHIM}:${PATH}" FAKE_FILE_DESC="${MACHO}" "${NT}/scripts/build-pack-release.sh" --out "${OUT}" --bin "${FAKE_BIN}" >/dev/null 2>&1 || RC=$?
expect "missing notice source exits 2" 2 "${RC}"
n=0
[ ! -d "${OUT}" ] || n="$(find "${OUT}" -type f | wc -l | tr -d ' ')"
expect "missing notice source leaves no artifact" 0 "${n}"

echo "# ${passed} passed, ${failed} failed"
[ "${failed}" -eq 0 ]
