#!/usr/bin/env bash
# tools/build-web.sh の受入基準を実際にビルドして確かめる（make test-build-web から呼ばれる）。
#
# 確かめること: 配信する設定に試験用の口が入らないこと、IR_FILE を指定したときだけ ir/main.json が入ること、
# 試験の設定には口が入ること（検査が空振りしていないことの対照）、誤用が非 0 で止まること。
# 生成物は mktemp -d の一時ディレクトリにだけ書き、終了時に消す。
#
# 終了コード: 0 = すべて成功 / 1 = 失敗が 1 件以上 / 2 = 誤用。CI への組み込みは TASK-33。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd node
require_cmd jq

BUILD="${ROOT_DIR}/tools/build-web.sh"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-build-web.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

FAILURES=0
fail() {
  echo "FAIL: $*" >&2
  FAILURES=$((FAILURES + 1))
}
pass() {
  echo "ok: $*" >&2
}
# check <説明> <コマンド...>: コマンドが成功すれば ok、失敗すれば FAIL を数える。
check() {
  local desc="$1"
  shift
  if "$@"; then pass "${desc}"; else fail "${desc}"; fi
}
# check_fails <説明> <コマンド...>: コマンドが非 0 で終わることを確かめる。
check_fails() {
  local desc="$1"
  shift
  if "$@" >/dev/null 2>&1; then fail "${desc}"; else pass "${desc}"; fi
}

# wasm_exports <wasm>: 書き出し名を 1 行ずつ出す。
wasm_exports() {
  node -e '
    const fs = require("fs");
    const m = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
    for (const e of WebAssembly.Module.exports(m)) console.log(e.name);
  ' "$1"
}
has_export() { wasm_exports "$2" | grep -q "$1"; }
no_debug_export() { ! wasm_exports "$1" | grep -q '^debug_'; }

DELIVERY="${TMP}/delivery"
DELIVERY_IR="${TMP}/delivery-ir"
TESTCFG="${TMP}/test"
echo '{"ir":"sample"}' >"${TMP}/ir.json"

log "building delivery config"
OUT_DIR="${DELIVERY}" "${BUILD}" >/dev/null
log "building delivery config with IR_FILE"
OUT_DIR="${DELIVERY_IR}" IR_FILE="${TMP}/ir.json" "${BUILD}" >/dev/null
log "building test config"
OUT_DIR="${TESTCFG}" TEST_HOOKS=1 "${BUILD}" >/dev/null

WASM="wasm/fandhe_workshop_web_bg.wasm"

echo "-- (a) delivery config" >&2
check "mount_app is exported" has_export '^mount_app$' "${DELIVERY}/${WASM}"
check "no debug_* export in wasm" no_debug_export "${DELIVERY}/${WASM}"
check_fails "no debug_ in glue js" grep -q 'debug_' "${DELIVERY}/wasm/fandhe_workshop_web.js"
check "no test-hooks.js" test ! -e "${DELIVERY}/test-hooks.js"
check_fails "no fandhe-test-hooks meta" grep -q 'fandhe-test-hooks' "${DELIVERY}/index.html"
check_fails "no test-hooks marker" grep -q 'fandhe:test-hooks' "${DELIVERY}/index.html"
check "no ir/ without IR_FILE" test ! -e "${DELIVERY}/ir"
check "no .d.ts" test -z "$(find "${DELIVERY}" -name '*.d.ts')"

echo "-- (b) delivery config with IR_FILE" >&2
check "ir/main.json matches IR_FILE" cmp -s "${TMP}/ir.json" "${DELIVERY_IR}/ir/main.json"
check "no debug_* export with IR_FILE" no_debug_export "${DELIVERY_IR}/${WASM}"

echo "-- (c) test config" >&2
check "debug_state_json is exported" has_export '^debug_state_json$' "${TESTCFG}/${WASM}"
check "test-hooks.js exists" test -f "${TESTCFG}/test-hooks.js"
check "fandhe-test-hooks meta exists" grep -q 'name="fandhe-test-hooks"' "${TESTCFG}/index.html"

echo "-- (d) misuse" >&2
check_fails "TEST_HOOKS=0 is rejected" env TEST_HOOKS=0 OUT_DIR="${TMP}/m1" "${BUILD}"
check_fails "unknown argument is rejected" env OUT_DIR="${TMP}/m2" "${BUILD}" bogus
mkdir -p "${TMP}/nonempty"
echo keep >"${TMP}/nonempty/file"
check_fails "non-empty OUT_DIR outside dist/ is rejected" env OUT_DIR="${TMP}/nonempty" "${BUILD}"
check "non-empty OUT_DIR is left intact" test -f "${TMP}/nonempty/file"
check_fails "missing IR_FILE is rejected" env IR_FILE="${TMP}/missing.json" OUT_DIR="${TMP}/m3" "${BUILD}"

if [ "${FAILURES}" -ne 0 ]; then
  echo "error: ${FAILURES} check(s) failed" >&2
  exit 1
fi
log "all checks passed"
