#!/usr/bin/env bash
# tools/measure/lib.sh の結果の組み立て（measure_result_json）と基点の収集（measure_pack_json）を、ビルドとブラウザなしで確かめる
# （make test-measure-result から呼ばれる。CI の check ジョブが流す）。
#
# 存在理由: macOS 同梱の jq 1.7 では、オブジェクトの値の位置の `{...} + $extra` がコンパイルエラーになり、4 種の測定が止まった（#1822）。
# 組み立ては遅い試験（make test-measure）でしか流れず CI で一度も実行されていなかったため、速い試験として分ける。
# 使っている jq が 1.7 の文法かどうかは (c) の負の対照で判定し、1.8 以上では「1.7 の文法の検査ではない」と標準エラーに出す。
# 生成物は mktemp -d の一時ディレクトリにだけ書き、終了時に消す。
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
require_cmd git
require_cmd mktemp
require_cmd rm

echo "jq: $(jq --version)" >&2

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-measure-result.XXXXXX")"
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
  if "$@" >/dev/null; then pass "${desc}"; else fail "${desc}"; fi
}

# 作り物の入力。browser.mjs の出力と同じ形（measurements・compression・conditions・browser・playwright）。
BROWSER="${TMP}/browser.json"
jq -n '{
  measurements: [{name: "first_paint_mounted_median", unit: "us", value: 1234}],
  compression: {command: "gzip -9 -c", implementation: "gzip 1.14"},
  conditions: {runs: 1},
  browser: "Chromium 0",
  playwright: "0.0.0"
}' >"${BROWSER}"
COMMIT='{"sha":"0123456789abcdef0123456789abcdef01234567","dirty":false}'
FEATURES='[]'
EXTRA_FULL='{"pack":{"id":"p","version":"1","nodes":3,"sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"},"device":{"os":"x","cpu":"y","logical_cores":4,"memory_bytes":1,"apple_silicon_macos":false}}'
export MEASURE_WASM_OPT_APPLIED=false
export MEASURE_WASM_OPT_DETAIL="SKIP_WASM_OPT=1"
export MEASURE_CONFIG=delivery

# --- (a) 組み立て ---
R1="${TMP}/r1.json"
if measure_result_json "${COMMIT}" "${FEATURES}" '{}' "${BROWSER}" >"${R1}"; then
  pass "assemble without extra (first-paint / redraw path)"
  check "result without extra passes measure_validate" measure_validate "${R1}"
  check "result without extra has no pack" jq -e '.baseline | has("pack") | not' "${R1}"
else
  fail "assemble without extra (first-paint / redraw path)"
fi
R2="${TMP}/r2.json"
if measure_result_json "${COMMIT}" "${FEATURES}" "${EXTRA_FULL}" "${BROWSER}" >"${R2}"; then
  pass "assemble with pack and device (log-redraw / input-paint path)"
  check "result with extra passes measure_validate" measure_validate "${R2}"
  check "result with extra has pack.id and device.os" jq -e '.baseline.pack.id == "p" and .baseline.device.os == "x"' "${R2}"
  check "result with extra keeps commit, wasm_opt, features, compression" \
    jq -e '.baseline | has("commit") and has("wasm_opt") and has("features") and has("compression")' "${R2}"
else
  fail "assemble with pack and device (log-redraw / input-paint path)"
fi

# --- (b) 基点の収集（決まった値になるもの） ---
PACK="${ROOT_DIR}/testdata/engine-event-consistency/test-data/min-3step.json"
PACK_OUT="${TMP}/pack.json"
if measure_pack_json "${PACK}" >"${PACK_OUT}"; then
  # shellcheck disable=SC2016 # jq の式であり、シェルで展開しない
  check "measure_pack_json returns id, version, nodes and a 64-hex sha256" \
    jq -e '(.id | type == "string") and (.version | type == "string") and (.nodes | type == "number") and (.sha256 | test("^[0-9a-f]{64}$"))' "${PACK_OUT}"
else
  fail "measure_pack_json succeeds on the minimal pack"
fi

# --- (d) native の様式（measure-native/v1。history-size.sh） ---
LINE="${TMP}/line.json"
jq -n '{
  pack: {id: "p", version: "1", nodes: 3, ir_sha256: ("a" * 64), content_hash: ("b" * 64), source: "s"},
  ops_sequence: {id: "o", ops: 2, ops_sha256: ("c" * 64), goto_substitutions: 1},
  size: {total_bytes: 100, ops: 2, ops_bytes: 10, snapshot_bytes: 80, log_entries: 5, changed_entries: 2, changed_bytes: 20, changed_ratio_ppm: 200000, checkpoints: 1, checkpoint_bytes: 7}
}' >"${LINE}"
RN="${TMP}/rn.json"
if measure_native_result_json "${COMMIT}" "${LINE}" >"${RN}"; then
  pass "assemble a native result"
  check "native result passes measure_validate_native" measure_validate_native "${RN}"
  check "native result has 10 measurements" jq -e '.measurements | length == 10' "${RN}"
  jq 'del(.baseline.commit.sha)' "${RN}" >"${TMP}/rn-nosha.json"
  if measure_validate_native "${TMP}/rn-nosha.json"; then fail "native result without commit.sha is rejected"; else pass "native result without commit.sha is rejected"; fi
  jq '.measurements[0].value = 0.5' "${RN}" >"${TMP}/rn-frac.json"
  if measure_validate_native "${TMP}/rn-frac.json"; then fail "native result with a non-integer value is rejected"; else pass "native result with a non-integer value is rejected"; fi
  jq -c . "${RN}" >"${TMP}/rn-two.json"
  jq -c 'del(.baseline.commit.sha)' "${RN}" >>"${TMP}/rn-two.json"
  if measure_validate_native "${TMP}/rn-two.json"; then fail "two concatenated native results are rejected"; else pass "two concatenated native results are rejected"; fi
else
  fail "assemble a native result"
fi

# --- (c) 負の対照: 使っている jq が 1.7 の文法かどうか ---
if jq -n '{a: {} + {}}' >/dev/null 2>&1; then
  echo "note: this jq accepts a binary operator in an object value (jq 1.8 or later); (a) is not a check of the jq 1.7 grammar here" >&2
else
  pass "this jq has the jq 1.7 grammar (a bare binary operator in an object value is rejected), so (a) is meaningful"
fi

if [ "${FAILURES}" -ne 0 ]; then
  echo "${FAILURES} check(s) failed" >&2
  exit 1
fi
echo "all checks passed" >&2
