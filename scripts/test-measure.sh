#!/usr/bin/env bash
# tools/measure/all.sh（初回表示・再描画を含む一括測定）と lib.sh の束の検証の受入基準を確かめる（make test-measure から呼ばれる）。
#
# 確かめること: 束（fandhe-workshop/measure-run/v1）の検証、誤用の拒否、基点を集められないとき・ブラウザの測定が失敗したときの
# fail-closed（標準出力が空）、実行の結果が基点つきで正しい構成であること、統計関数の試験（node:test）。
# 実ビルドと Chromium を使うため遅い。make check・フック・CI には入れない（CI の大きさの関門は scripts/check-wasm-size.sh）。
# Playwright や Chromium が無い場合はスキップせず失敗にする（未検査の成功を作らない）。
# 生成物は mktemp -d の一時ディレクトリにだけ書き、終了時に消す。単体の様式の試験は scripts/test-measure-wasm-size.sh。
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
require_cmd node
require_cmd mktemp
require_cmd rm

SCRIPT="${ROOT_DIR}/tools/measure/all.sh"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-measure-run.XXXXXX")"
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
check_fails() {
  local desc="$1"
  shift
  if "$@" >/dev/null 2>&1; then fail "${desc}"; else pass "${desc}"; fi
}
# status_is <期待する終了コード> <コマンド...>: 終了コードが一致すれば成功。
status_is() {
  local want="$1" got=0
  shift
  "$@" >/dev/null 2>&1 || got=$?
  [ "${got}" -eq "${want}" ]
}

# --- (a) 束の検証（ビルドなし） ---
# result_json <config> <測定名> <単位>: 様式 v1 の正常な結果を 1 件作る。
result_json() {
  jq -n --arg config "$1" --arg name "$2" --arg unit "$3" '{
    schema: "fandhe-workshop/measure/v1",
    measured_at: "2026-10-14T00:00:00Z",
    baseline: {
      commit: {sha: "0123456789abcdef0123456789abcdef01234567", dirty: false},
      wasm_opt: {applied: false, detail: "SKIP_WASM_OPT=1"},
      features: {config: $config, cargo_features: [], profile: "release"},
      compression: {command: "gzip -9 -c", implementation: "gzip 1.14"}
    },
    measurements: [{name: $name, file: "a", unit: $unit, value: 10}]
  }'
}
GOOD="${TMP}/good.json"
jq -n \
  --argjson a "$(result_json delivery wasm_gzip bytes)" \
  --argjson b "$(result_json delivery first_paint_mounted_median us)" \
  --argjson c "$(result_json test redraw_median us)" \
  '{schema: "fandhe-workshop/measure-run/v1", results: [$a, $b, $c]}' >"${GOOD}"
check "run validate accepts a well-formed bundle" measure_validate_run "${GOOD}"
check "run emit prints a well-formed bundle" measure_emit_run "${GOOD}"

# bad <説明> <jq フィルタ>: GOOD を壊した束を作り、検証が拒否することと emit が標準出力へ何も出さないことを確かめる。
bad() {
  local desc="$1" filter="$2" f out
  f="${TMP}/bad.json"
  jq "${filter}" "${GOOD}" >"${f}"
  check_fails "run validate rejects: ${desc}" measure_validate_run "${f}"
  out="$(measure_emit_run "${f}" 2>/dev/null || true)"
  if [ -z "${out}" ]; then pass "run emit prints nothing: ${desc}"; else fail "run emit printed output: ${desc}"; fi
  check_fails "run emit fails: ${desc}" measure_emit_run "${f}"
}
bad "2 results" '.results |= .[0:2]'
bad "4 results" '.results += [.results[0]]'
bad "1 result has no baseline compression" 'del(.results[1].baseline.compression)'
bad "1 result has no baseline commit" 'del(.results[2].baseline.commit)'
bad "commit sha differs" '.results[2].baseline.commit.sha = "fedcba9876543210fedcba9876543210fedcba98"'
bad "dirty differs" '.results[1].baseline.commit.dirty = true'
bad "same kind is duplicated" '.results[1].measurements[0].name = "wasm_gzip"'
bad "a kind is missing" '.results[2].measurements[0].name = "redraw_p95"'
bad "bundle schema differs" '.schema = "x"'
bad "results is not an array" '.results = {}'

# 複数の JSON 値の入力（不正な束の後ろに正常な束を連結）は、emit が何も出さず失敗する。
MULTI="${TMP}/multi.json"
{ jq 'del(.results[0].baseline.commit)' "${GOOD}"; jq . "${GOOD}"; } >"${MULTI}"
check_fails "run validate rejects: multiple JSON values (bad then good)" measure_validate_run "${MULTI}"
MULTI_OUT="$(measure_emit_run "${MULTI}" 2>/dev/null || true)"
if [ -z "${MULTI_OUT}" ]; then pass "run emit prints nothing: multiple JSON values"; else fail "run emit printed output: multiple JSON values"; fi
{ jq . "${GOOD}"; jq . "${GOOD}"; } >"${MULTI}"
check_fails "run validate rejects: two valid bundles" measure_validate_run "${MULTI}"

# --- (b) 誤用 ---
check "all.sh: unknown argument exits 2" status_is 2 "${SCRIPT}" --unknown
check "wasm-size.sh: TEST_HOOKS=0 exits 2" status_is 2 env TEST_HOOKS=0 "${ROOT_DIR}/tools/measure/wasm-size.sh"
check "first-paint.sh: unknown argument exits 2" status_is 2 "${ROOT_DIR}/tools/measure/first-paint.sh" --unknown
check "redraw.sh: unknown argument exits 2" status_is 2 "${ROOT_DIR}/tools/measure/redraw.sh" --unknown

# --- (c) fail-closed ---
mkdir -p "${TMP}/stub-git" "${TMP}/stub-node"
printf '#!/bin/sh\nexit 1\n' >"${TMP}/stub-git/git"
chmod 755 "${TMP}/stub-git/git"
STUB_OUT="$(PATH="${TMP}/stub-git:${PATH}" "${SCRIPT}" 2>/dev/null || true)"
check "git failure: stdout is empty" test -z "${STUB_OUT}"
check "git failure: exit code is 1" status_is 1 env PATH="${TMP}/stub-git:${PATH}" "${SCRIPT}"

# node が失敗する（ブラウザの測定が失敗する）と、all.sh の標準出力は空で終了コードは 1 になる。
printf '#!/bin/sh\nexit 1\n' >"${TMP}/stub-node/node"
chmod 755 "${TMP}/stub-node/node"
NODE_OUT="$(PATH="${TMP}/stub-node:${PATH}" SKIP_WASM_OPT=1 "${SCRIPT}" 2>/dev/null || true)"
check "browser failure: stdout is empty" test -z "${NODE_OUT}"
check "browser failure: exit code is 1" status_is 1 env PATH="${TMP}/stub-node:${PATH}" SKIP_WASM_OPT=1 "${SCRIPT}"

# --- (d) 実行（実ビルドと Chromium。Playwright が無ければ失敗にする） ---
HEAD_SHA="$(git -C "${ROOT_DIR}" rev-parse HEAD)"
RUN="${TMP}/run.json"
if SKIP_WASM_OPT=1 "${SCRIPT}" >"${RUN}" 2>"${RUN}.err"; then
  check "run: bundle passes validation" measure_validate_run "${RUN}"
  check "run: configs are delivery, delivery, test" jq -e '[.results[].baseline.features.config] == ["delivery", "delivery", "test"]' "${RUN}"
  check "run: redraw uses the test-hooks feature" jq -e '.results[2].baseline.features.cargo_features == ["test-hooks"]' "${RUN}"
  check "run: delivery results have no cargo features" jq -e '.results[0].baseline.features.cargo_features == [] and .results[1].baseline.features.cargo_features == []' "${RUN}"
  # shellcheck disable=SC2016 # $s は jq の変数であり、シェルで展開しない
  check "run: commit.sha matches HEAD in all results" jq -e --arg s "${HEAD_SHA}" '[.results[].baseline.commit.sha] | all(. == $s)' "${RUN}"
  check "run: wasm_opt is not applied in all results" jq -e '[.results[].baseline.wasm_opt.applied] | all(. == false)' "${RUN}"
  check "run: time values are positive integers" jq -e '[.results[1].measurements[], .results[2].measurements[] | select(.unit == "us") | .value] | length > 0 and all(type == "number" and . > 0 and . == floor)' "${RUN}"
  check "run: first-paint compression records gzip level 9" jq -e '.results[1].baseline.compression.command | contains("level 9")' "${RUN}"
  check "run: first-paint records the network conditions" jq -e '.results[1].conditions.network.down_mbps == 10 and .results[1].conditions.network.latency_ms == 40' "${RUN}"
  check "run: toolchain records the browser" jq -e '.results[1].toolchain.browser | startswith("chromium ")' "${RUN}"
else
  fail "run: all.sh failed (see ${RUN}.err)"
  cat "${RUN}.err" >&2
fi

# --- (e) log-redraw（ログ 100〜119 項目の再描画。TASK-223 1/2。束には入れない単体の測定） ---
check "log-redraw.sh: unknown argument exits 2" status_is 2 "${ROOT_DIR}/tools/measure/log-redraw.sh" --unknown
# 基点（教材・端末）が欠けた入力は拒否する（fail-closed）。ブラウザの測定の前に止まるため、標準出力は空になる。
check_fails "log-redraw: missing pack/device baseline is refused" measure_browser_result log-redraw "$(measure_commit_json)" "${TMP}" '{}'
check_fails "log-redraw: missing device baseline is refused" measure_browser_result log-redraw "$(measure_commit_json)" "${TMP}" '{"pack":{"id":"x","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}'
LOGR="${TMP}/log-redraw.json"
if SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/measure/log-redraw.sh" >"${LOGR}" 2>"${LOGR}.err"; then
  check "log-redraw: passes the v1 validation" measure_validate "${LOGR}"
  check "log-redraw: pack baseline is min-3step with a sha256" jq -e '.baseline.pack.id == "min-3step" and (.baseline.pack.sha256 | test("^[0-9a-f]{64}$"))' "${LOGR}"
  check "log-redraw: device baseline has typed fields" jq -e '.baseline.device | (.os | type == "string") and (.cpu | type == "string") and (.logical_cores | type == "number") and (.memory_bytes | type == "number") and (.apple_silicon_macos | type == "boolean")' "${LOGR}"
  check "log-redraw: three log_redraw measurements" jq -e '[.measurements[].name] == ["log_redraw_median", "log_redraw_p95", "log_redraw_max"]' "${LOGR}"
  check "log-redraw: conditions record the window and the scope" jq -e '.conditions.log_window == [100, 119] and .conditions.trials * .conditions.samples_per_trial == 200 and (.conditions.scope_note | contains("not representative"))' "${LOGR}"
  check "log-redraw: uses the test-hooks feature" jq -e '.baseline.features.cargo_features == ["test-hooks"]' "${LOGR}"
else
  fail "log-redraw: log-redraw.sh failed (see ${LOGR}.err)"
  cat "${LOGR}.err" >&2
fi

# --- (f) 統計関数と引数の試験 ---
check "node:test for browser.mjs passes" node --test "${ROOT_DIR}/tools/measure/browser.test.mjs"

if [ "${FAILURES}" -ne 0 ]; then
  echo "${FAILURES} check(s) failed" >&2
  exit 1
fi
echo "all checks passed" >&2
