#!/usr/bin/env bash
# tools/build-web.sh の受入基準を実際にビルドして確かめる（make test-build-web から呼ばれる）。
#
# 確かめること: 配信する設定に試験用の口が入らないこと、IR_FILE を指定したときだけ ir/main.json が入ること、
# 試験の設定には口が入ること（検査が空振りしていないことの対照）、誤用が非 0 で止まること。
# 生成物は mktemp -d の一時ディレクトリ（dist/ 配下のケースは dist/ に作る専用の一時ディレクトリ）にだけ書き、終了時に消す。
#
# (a) の配信物の検査は tools/check-delivery.sh（make check-delivery）と同種の項目を持つ。こちらは build-web 固有の
# 受入基準（keydown-probe・ir/ の有無・誤用）も確かめる。検査の一本化は TASK-82 の拡張時に行う。
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
# dist/ 配下の出力先を試すケース用。既存の dist/ のファイルに触れないよう専用ディレクトリを確保し、それだけを消す。
DIST_TMP=""
trap 'rm -rf "${TMP}"; if [ -n "${DIST_TMP}" ]; then rm -rf "${DIST_TMP}"; fi' EXIT

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

# check_status <説明> <期待する終了状態> <コマンド...>: 終了状態が期待と完全に一致することを確かめる。
# grep の 1（不一致）と 2（ファイル欠落等の検査失敗）を区別するために使う。
check_status() {
  local desc="$1" want="$2" rc=0
  shift 2
  "$@" >/dev/null 2>&1 || rc=$?
  if [ "${rc}" -eq "${want}" ]; then pass "${desc}"; else fail "${desc} (exit ${rc}, want ${want})"; fi
}

# wasm_exports <wasm>: 書き出し名を 1 行ずつ出す。
wasm_exports() {
  node -e '
    const fs = require("fs");
    const m = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
    for (const e of WebAssembly.Module.exports(m)) console.log(e.name);
  ' "$1"
}
# has_export <regex> <wasm>: 書き出しの取得に失敗したら失敗。取得できたうえで一致があれば成功。
has_export() {
  local exports rc=0
  exports="$(wasm_exports "$2")" || return 1
  printf '%s\n' "${exports}" | grep -q "$1" || rc=$?
  [ "${rc}" -eq 0 ]
}
# no_debug_export <wasm>: 書き出しの取得に成功し、かつ debug_* が無い（grep の終了状態 1）ときだけ成功。
# 取得失敗や grep の検査失敗（2 以上）を「該当なし」と読み替えない。
no_debug_export() {
  local exports rc=0
  exports="$(wasm_exports "$1")" || return 1
  [ -n "${exports}" ] || return 1
  printf '%s\n' "${exports}" | grep -q '^debug_' || rc=$?
  [ "${rc}" -eq 1 ]
}

DELIVERY="${TMP}/delivery"
DELIVERY_IR="${TMP}/delivery-ir"
TESTCFG="${TMP}/test"
echo '{"ir":"sample"}' >"${TMP}/ir.json"

log "building delivery config"
OUT_DIR="${DELIVERY}" "${BUILD}" >/dev/null
log "building delivery config with IR_FILE"
# 相対パスの IR_FILE が、ビルド中の cd に影響されず呼び出し元の cwd 基準で解決されることも確かめる。
(cd "${TMP}" && OUT_DIR="${DELIVERY_IR}" IR_FILE="ir.json" "${BUILD}" >/dev/null)
log "building test config"
OUT_DIR="${TESTCFG}" TEST_HOOKS=1 "${BUILD}" >/dev/null

WASM="wasm/fandhe_workshop_web_bg.wasm"

echo "-- (a) delivery config" >&2
check "mount_app is exported" has_export '^mount_app$' "${DELIVERY}/${WASM}"
check "no debug_* export in wasm" no_debug_export "${DELIVERY}/${WASM}"
check_status "no debug_ in glue js" 1 grep -q 'debug_' "${DELIVERY}/wasm/fandhe_workshop_web.js"
check "no test-hooks.js" test ! -e "${DELIVERY}/test-hooks.js"
check_status "no fandhe-test-hooks meta" 1 grep -q 'fandhe-test-hooks' "${DELIVERY}/index.html"
check_status "no test-hooks marker" 1 grep -q 'fandhe:test-hooks' "${DELIVERY}/index.html"
check_status "no keydown-probe in wasm" 1 grep -aq 'keydown-probe' "${DELIVERY}/${WASM}"
check_status "no keydown-probe in glue js" 1 grep -q 'keydown-probe' "${DELIVERY}/wasm/fandhe_workshop_web.js"
check "no ir/ without IR_FILE" test ! -e "${DELIVERY}/ir"
check "no .d.ts" test -z "$(find "${DELIVERY}" -name '*.d.ts')"

echo "-- (b) delivery config with IR_FILE" >&2
check "ir/main.json matches IR_FILE" cmp -s "${TMP}/ir.json" "${DELIVERY_IR}/ir/main.json"
check "mount_app is exported with IR_FILE" has_export '^mount_app$' "${DELIVERY_IR}/${WASM}"
check "no debug_* export with IR_FILE" no_debug_export "${DELIVERY_IR}/${WASM}"

echo "-- (c) test config" >&2
check "debug_state_json is exported" has_export '^debug_state_json$' "${TESTCFG}/${WASM}"
check "debug_rerender is exported" has_export '^debug_rerender$' "${TESTCFG}/${WASM}"
check "test-hooks.js exists" test -f "${TESTCFG}/test-hooks.js"
# 配信設定の「keydown-probe が無い」検査が空振りしていないことの対照（試験の設定には有る）。
check "keydown-probe is in the test config wasm" grep -aq 'keydown-probe' "${TESTCFG}/${WASM}"
check "fandhe-test-hooks meta exists" grep -q 'name="fandhe-test-hooks"' "${TESTCFG}/index.html"

echo "-- (d) misuse" >&2
check_fails "TEST_HOOKS=0 is rejected" env TEST_HOOKS=0 OUT_DIR="${TMP}/m1" "${BUILD}"
check_fails "unknown argument is rejected" env OUT_DIR="${TMP}/m2" "${BUILD}" bogus
mkdir -p "${TMP}/nonempty"
echo keep >"${TMP}/nonempty/file"
check_fails "non-empty OUT_DIR outside dist/ is rejected" env OUT_DIR="${TMP}/nonempty" "${BUILD}"
check "non-empty OUT_DIR is left intact" test -f "${TMP}/nonempty/file"
check_status "empty OUT_DIR is rejected" 2 env OUT_DIR= "${BUILD}"
mkdir -p "${TMP}/sub"
check_status "OUT_DIR ending in .. is rejected" 2 env OUT_DIR="${TMP}/sub/.." "${BUILD}"
check_status "OUT_DIR ending in . is rejected" 2 env OUT_DIR="${TMP}/sub/." "${BUILD}"
check "dir behind .. is left intact" test -d "${TMP}/sub"
check_status "missing IR_FILE is rejected" 2 env IR_FILE="${TMP}/missing.json" OUT_DIR="${TMP}/m3" "${BUILD}"
# 出力先の配下にある IR（シンボリックリンク経由を含む）は、削除で入力が消えるため削除前に拒否する。
mkdir -p "${TMP}/inside/ir" "${TMP}/elsewhere"
echo '{"ir":"keep"}' >"${TMP}/inside/ir/main.json"
ln -s "${TMP}/inside/ir/main.json" "${TMP}/elsewhere/link.json"
check_status "IR_FILE inside OUT_DIR is rejected" 2 env IR_FILE="${TMP}/inside/ir/main.json" OUT_DIR="${TMP}/inside" "${BUILD}"
check_status "symlinked IR_FILE inside OUT_DIR is rejected" 2 env IR_FILE="${TMP}/elsewhere/link.json" OUT_DIR="${TMP}/inside" "${BUILD}"
check "IR_FILE inside OUT_DIR is left intact" test -f "${TMP}/inside/ir/main.json"
# IR_FILE と OUT_DIR が同一のファイルでも、削除前に拒否して入力を残す（dist/ 配下の出力先でも同じ）。
echo '{"ir":"keep"}' >"${TMP}/same.json"
check_status "IR_FILE equal to OUT_DIR is rejected" 2 env IR_FILE="${TMP}/same.json" OUT_DIR="${TMP}/same.json" "${BUILD}"
check "IR_FILE equal to OUT_DIR is left intact" test -f "${TMP}/same.json"
mkdir -p "${ROOT_DIR}/dist"
DIST_TMP="$(mktemp -d "${ROOT_DIR}/dist/test-build-web.XXXXXX")"
SAME_DIST="${DIST_TMP}/same.json"
echo '{"ir":"keep"}' >"${SAME_DIST}"
check_status "IR_FILE equal to OUT_DIR under dist/ is rejected" 2 env IR_FILE="${SAME_DIST}" OUT_DIR="${SAME_DIST}" "${BUILD}"
check "IR_FILE equal to OUT_DIR under dist/ is left intact" test -f "${SAME_DIST}"

if [ "${FAILURES}" -ne 0 ]; then
  echo "error: ${FAILURES} check(s) failed" >&2
  exit 1
fi
log "all checks passed"
