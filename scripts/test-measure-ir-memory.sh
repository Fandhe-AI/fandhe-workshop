#!/usr/bin/env bash
# ir-memory の測定（tools/measure/ir-memory.*・lib.sh の measure_ir_memory_check）の様式と fail-closed を、ビルドとブラウザなしで確かめる
# （make test-measure-ir-memory から呼ばれる。TASK-163）。生成物は mktemp -d の一時ディレクトリにだけ書き、終了時に消す。
#
# 終了コード: 0 = すべて成功 / 1 = 失敗が 1 件以上 / 2 = 誤用。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
# shellcheck source=../tools/measure/lib.sh
source "${ROOT_DIR}/tools/measure/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd jq
require_cmd node
require_cmd git

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-ir-memory.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

FAILURES=0
fail() {
  echo "FAIL: $*" >&2
  FAILURES=$((FAILURES + 1))
}
pass() {
  echo "ok: $*" >&2
}
check() {
  local desc="$1"
  shift
  if "$@" >/dev/null 2>&1; then pass "${desc}"; else fail "${desc}"; fi
}
check_not() {
  local desc="$1"
  shift
  if "$@" >/dev/null 2>&1; then fail "${desc}"; else pass "${desc}"; fi
}

# (a) 純関数
check "node --test ir-memory.test.mjs" node --test "${ROOT_DIR}/tools/measure/ir-memory.test.mjs"

# 作り物の browser.mjs の出力。第 1 引数 = crash の件数（pack_title の場合）。
make_browser() {
  local crashes="$1" out="$2"
  jq -n --argjson c "${crashes}" '
    def case($n; $f; $cr): {name: $n, field: $f, chars: 0, ir_bytes: 100, ir_sha256: ("0" * 64), crashes: $cr, notices: 1, reason: "string_too_long", trial_peaks_kib: [1000, 1100, 1050]};
    {
      measurements: [{name: "ir_memory_base_renderer_rss_max", unit: "KiB", value: 1100}],
      compression: {command: "gzip -9", implementation: "node"},
      conditions: {trials: 3, crashes_total: $c, low_memory_device: "unverified",
        cases: [case("base"; "none"; 0), case("pack_title"; "pack.title"; $c), case("hint_text"; "hints.h-1-1.text"; 0)]},
      browser: "chromium 0", playwright: "0.0.0"
    }' >"${out}"
}

# (b) 組み立てと検証
make_browser 0 "${TMP}/ok.json"
COMMIT="$(measure_commit_json)"
DEVICE="$(measure_device_json)"
EXTRA="$(jq -n --argjson d "${DEVICE}" '{pack: {id: "p", version: "1", nodes: 1, sha256: ("a" * 64)}, device: $d}')"
export MEASURE_WASM_OPT_APPLIED=false MEASURE_WASM_OPT_DETAIL="test" MEASURE_CONFIG=delivery
measure_result_json "${COMMIT}" '[]' "${EXTRA}" "${TMP}/ok.json" >"${TMP}/result.json"
check "result validates (v1)" measure_validate "${TMP}/result.json"
check "result has device and browser" jq -e '.baseline.device.os and (.toolchain.browser | startswith("chromium"))' "${TMP}/result.json"
check_not "result has no pid/path/host fields" jq -e '[.. | strings | select(test("/tmp/|/home/|/Users/"))] | length > 0' "${TMP}/result.json"

# (c) fail-closed: 基点の欠け・IR ファイルなし（ビルドの前に断る）
OUT="$(measure_browser_result ir-memory "${COMMIT}" "${TMP}" '{}' "${ROOT_DIR}/Makefile" 2>/dev/null)" && RC=0 || RC=$?
if [ "${RC}" = "1" ] && [ -z "${OUT}" ]; then pass "missing device baseline is refused"; else fail "missing device baseline (rc=${RC})"; fi
OUT="$(measure_browser_result ir-memory "${COMMIT}" "${TMP}" "${EXTRA}" "" 2>/dev/null)" && RC=0 || RC=$?
if [ "${RC}" = "1" ] && [ -z "${OUT}" ]; then pass "missing IR file is refused"; else fail "missing IR file (rc=${RC})"; fi

# (d) 終了コードと crash の一致
make_browser 1 "${TMP}/crash.json"
check "ok output with rc 0" measure_ir_memory_check "${TMP}/ok.json" 0
check "crash output with rc 3" measure_ir_memory_check "${TMP}/crash.json" 3
check_not "crash 0 with rc 3 is refused" measure_ir_memory_check "${TMP}/ok.json" 3
check_not "crash 1 with rc 0 is refused" measure_ir_memory_check "${TMP}/crash.json" 0
check_not "rc 1 is refused" measure_ir_memory_check "${TMP}/ok.json" 1
jq '.conditions.cases |= .[0:2]' "${TMP}/ok.json" >"${TMP}/two.json"
check_not "two cases are refused" measure_ir_memory_check "${TMP}/two.json" 0
jq '.conditions.cases[1].reason = 5' "${TMP}/ok.json" >"${TMP}/reason.json"
check_not "numeric reason is refused" measure_ir_memory_check "${TMP}/reason.json" 0
jq '.conditions.crashes_total = 2' "${TMP}/crash.json" >"${TMP}/sum.json"
check_not "crashes_total mismatching the sum is refused" measure_ir_memory_check "${TMP}/sum.json" 3

# (e) 誤用
RC=0
"${ROOT_DIR}/tools/measure/ir-memory.sh" extra-arg >/dev/null 2>&1 || RC=$?
if [ "${RC}" = "2" ]; then pass "extra argument is a misuse (2)"; else fail "extra argument rc=${RC}"; fi

if [ "${FAILURES}" -ne 0 ]; then
  echo "error: ${FAILURES} check(s) failed" >&2
  exit 1
fi
log "all checks passed"
