#!/usr/bin/env bash
# tools/measure/all.sh（初回表示・再描画を含む一括測定）と lib.sh の束の検証の受入基準を確かめる（make test-measure から呼ばれる）。
#
# 確かめること: input-paint（TASK-223 2/2）・view-redraw（TASK-485 1/3）・product-redraw（TASK-485 2/3）の単体の結果、束（fandhe-workshop/measure-run/v1）の検証、誤用の拒否、基点を集められないとき・ブラウザの測定が失敗したときの
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

# --- (g) log-slope（ログ 100〜4,000 項目の再描画の傾き。TASK-232 1/3。束には入れない単体の測定） ---
check "log-slope.sh: unknown argument exits 2" status_is 2 "${ROOT_DIR}/tools/measure/log-slope.sh" --unknown
check_fails "log-slope: missing pack/device baseline is refused" measure_browser_result log-slope "$(measure_commit_json)" "${TMP}" '{}'
check_fails "log-slope: missing device baseline is refused" measure_browser_result log-slope "$(measure_commit_json)" "${TMP}" '{"pack":{"id":"x","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}'
LOGS="${TMP}/log-slope.json"
if SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/measure/log-slope.sh" >"${LOGS}" 2>"${LOGS}.err"; then
  check "log-slope: passes the v1 validation" measure_validate "${LOGS}"
  check "log-slope: pack baseline is min-3step with a sha256" jq -e '.baseline.pack.id == "min-3step" and (.baseline.pack.sha256 | test("^[0-9a-f]{64}$"))' "${LOGS}"
  check "log-slope: device baseline has typed fields" jq -e '.baseline.device | (.os | type == "string") and (.cpu | type == "string") and (.logical_cores | type == "number") and (.memory_bytes | type == "number") and (.apple_silicon_macos | type == "boolean")' "${LOGS}"
  check "log-slope: median, p95 and max for each of 12 steps" jq -e '(.measurements | length) == 36 and ([.measurements[].name] == [.conditions.log_steps[] | tostring | ("0000" + .)[-4:] | ("log_slope_" + . + "_median", "log_slope_" + . + "_p95", "log_slope_" + . + "_max")])' "${LOGS}"
  check "log-slope: conditions record the steps, folding and the scope" jq -e '(.conditions.log_steps | length) == 12 and .conditions.trials * .conditions.samples_per_trial == 200 and .conditions.fold_target == 1000 and (.conditions.rows_observed | map(.rows_max <= 1000) | all) and (.conditions.scope_note | contains("not representative"))' "${LOGS}"
  check "log-slope: uses the test-hooks feature" jq -e '.baseline.features.cargo_features == ["test-hooks"]' "${LOGS}"
else
  fail "log-slope: log-slope.sh failed (see ${LOGS}.err)"
  cat "${LOGS}.err" >&2
fi

# --- (i) log-rounds（世代と Step をまたぐ 2 周分の通しの再描画。TASK-232 3/3。束には入れない単体の測定） ---
check "log-rounds.sh: unknown argument exits 2" status_is 2 "${ROOT_DIR}/tools/measure/log-rounds.sh" --unknown
check_fails "log-rounds: missing pack/device baseline is refused" measure_browser_result log-rounds "$(measure_commit_json)" "${TMP}" '{}'
check_fails "log-rounds: missing device baseline is refused" measure_browser_result log-rounds "$(measure_commit_json)" "${TMP}" '{"pack":{"id":"x","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}'
LOGG="${TMP}/log-rounds.json"
if SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/measure/log-rounds.sh" >"${LOGG}" 2>"${LOGG}.err"; then
  check "log-rounds: passes the v1 validation" measure_validate "${LOGG}"
  check "log-rounds: pack baseline is min-3step with a sha256" jq -e '.baseline.pack.id == "min-3step" and (.baseline.pack.sha256 | test("^[0-9a-f]{64}$"))' "${LOGG}"
  check "log-rounds: device baseline has typed fields" jq -e '.baseline.device | (.os | type == "string") and (.cpu | type == "string") and (.logical_cores | type == "number") and (.memory_bytes | type == "number") and (.apple_silicon_macos | type == "boolean")' "${LOGG}"
  # shellcheck disable=SC2016 # $g・$r は jq の変数であり、シェルで展開しない
  check "log-rounds: median, p95 and max for each of 2 group counts x 3 stages" jq -e '(.measurements | length) == 18 and ([.measurements[].name] == [.conditions.group_counts[] as $g | .conditions.row_stages[] as $r | ("g" + ("0" + ($g | tostring))[-2:] + "_" + ("000" + ($r | tostring))[-4:]) | ("log_rounds_" + . + "_median", "log_rounds_" + . + "_p95", "log_rounds_" + . + "_max")])' "${LOGG}"
  check "log-rounds: conditions record the groups, folding and the scope" jq -e '.conditions.group_counts == [6, 48] and .conditions.trials * .conditions.samples_per_trial == 200 and .conditions.fold_target == 1000 and (.conditions.rows_observed | length == 6 and (map(.rows_max <= 1000) | all)) and (.conditions.scope_note | contains("not representative"))' "${LOGG}"
  check "log-rounds: uses the test-hooks feature" jq -e '.baseline.features.cargo_features == ["test-hooks"]' "${LOGG}"
else
  fail "log-rounds: log-rounds.sh failed (see ${LOGG}.err)"
  cat "${LOGG}.err" >&2
fi

# --- (f) input-paint（ログ 120〜158 項目の入力から描画まで。TASK-223 2/2。束には入れない単体の測定） ---
check "input-paint.sh: unknown argument exits 2" status_is 2 "${ROOT_DIR}/tools/measure/input-paint.sh" --unknown
check_fails "input-paint: missing pack/device baseline is refused" measure_browser_result input-paint "$(measure_commit_json)" "${TMP}" '{}'
check_fails "input-paint: missing device baseline is refused" measure_browser_result input-paint "$(measure_commit_json)" "${TMP}" '{"pack":{"id":"x","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}'
INP="${TMP}/input-paint.json"
if SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/measure/input-paint.sh" >"${INP}" 2>"${INP}.err"; then
  check "input-paint: passes the v1 validation" measure_validate "${INP}"
  check "input-paint: pack baseline is min-3step with a sha256" jq -e '.baseline.pack.id == "min-3step" and (.baseline.pack.sha256 | test("^[0-9a-f]{64}$"))' "${INP}"
  check "input-paint: device baseline has typed fields" jq -e '.baseline.device | (.os | type == "string") and (.cpu | type == "string") and (.logical_cores | type == "number") and (.memory_bytes | type == "number") and (.apple_silicon_macos | type == "boolean")' "${INP}"
  check "input-paint: four input_paint measurements" jq -e '[.measurements[].name] == ["input_paint_median", "input_paint_p95", "input_paint_max", "input_paint_capture_p95"]' "${INP}"
  check "input-paint: conditions record the window and the scope" jq -e '.conditions.log_window == [120, 158] and .conditions.trials * .conditions.samples_per_trial == 200 and (.conditions.scope_note | contains("not representative"))' "${INP}"
  check "input-paint: uses the test-hooks feature" jq -e '.baseline.features.cargo_features == ["test-hooks"]' "${INP}"
else
  fail "input-paint: input-paint.sh failed (see ${INP}.err)"
  cat "${INP}.err" >&2
fi

# --- (g) view-redraw（左のビューの部品単位の再描画。TASK-485 1/3。束には入れない単体の測定） ---
check "view-redraw.sh: unknown argument exits 2" status_is 2 "${ROOT_DIR}/tools/measure/view-redraw.sh" --unknown
ZERO64="0000000000000000000000000000000000000000000000000000000000000000"
DEV_OK='{"os":"Linux","cpu":"x","logical_cores":1,"memory_bytes":1,"apple_silicon_macos":false}'
check_fails "view-redraw: missing data/device baseline is refused" measure_browser_result view-redraw "$(measure_commit_json)" "${TMP}" '{}'
check_fails "view-redraw: missing device baseline is refused" measure_browser_result view-redraw "$(measure_commit_json)" "${TMP}" "{\"data\":{\"id\":\"x\",\"manifest_sha256\":\"${ZERO64}\",\"files\":{\"a\":\"b\"}}}"
check_fails "view-redraw: a pack baseline alone is refused" measure_browser_result view-redraw "$(measure_commit_json)" "${TMP}" "{\"pack\":{\"id\":\"x\",\"sha256\":\"${ZERO64}\"},\"device\":${DEV_OK}}"
VRD="${TMP}/view-redraw.json"
if SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/measure/view-redraw.sh" >"${VRD}" 2>"${VRD}.err"; then
  check "view-redraw: passes the v1 validation" measure_validate "${VRD}"
  check "view-redraw: data baseline has the manifest and file hashes" jq -e '.baseline.data.id == "artifact-view-parts" and (.baseline.data.manifest_sha256 | test("^[0-9a-f]{64}$")) and (.baseline.data.files | length == 7) and (.baseline.data.files | all(.[]; test("^[0-9a-f]{64}$")))' "${VRD}"
  check "view-redraw: device baseline has typed fields" jq -e '.baseline.device | (.os | type == "string") and (.cpu | type == "string") and (.logical_cores | type == "number") and (.memory_bytes | type == "number") and (.apple_silicon_macos | type == "boolean")' "${VRD}"
  check "view-redraw: 72 measurements named by kind, variant and op" jq -e '(.measurements | length == 72) and (.measurements | all(.name | test("^view_redraw_[a-z_]+_[ab]_(toggle|switch)_(median|p95|max)$")))' "${VRD}"
  check "view-redraw: 200 values per case and the scope is the fixture screen" jq -e '.conditions.trials * .conditions.samples_per_trial == 200 and (.conditions.scope_note | contains("not the product view()"))' "${VRD}"
  check "view-redraw: judged kinds are recorded" jq -e '.conditions.judged == ["document", "diff", "task_tree"] and (.conditions.judgement.document.judged == true)' "${VRD}"
  check "view-redraw: uses the test-hooks feature" jq -e '.baseline.features.cargo_features == ["test-hooks"]' "${VRD}"
else
  fail "view-redraw: view-redraw.sh failed (see ${VRD}.err)"
  cat "${VRD}.err" >&2
fi

# --- (h) product-redraw（本体の画面の再描画。TASK-485 2/3。束には入れない単体の測定） ---
check "product-redraw.sh: unknown argument exits 2" status_is 2 "${ROOT_DIR}/tools/measure/product-redraw.sh" --unknown
check_fails "product-redraw: missing pack/device baseline is refused" measure_browser_result product-redraw "$(measure_commit_json)" "${TMP}" '{}'
check_fails "product-redraw: missing device baseline is refused" measure_browser_result product-redraw "$(measure_commit_json)" "${TMP}" "{\"pack\":{\"id\":\"x\",\"sha256\":\"${ZERO64}\"}}"
PRD="${TMP}/product-redraw.json"
if SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/measure/product-redraw.sh" >"${PRD}" 2>"${PRD}.err"; then
  check "product-redraw: passes the v1 validation" measure_validate "${PRD}"
  check "product-redraw: pack baseline is min-3step with a sha256" jq -e '.baseline.pack.id == "min-3step" and (.baseline.pack.sha256 | test("^[0-9a-f]{64}$"))' "${PRD}"
  check "product-redraw: 21 measurements named by kind, rows and op" jq -e '(.measurements | length == 21) and (.measurements | all(.name | test("^product_redraw_[a-z_]+_[0-9]+_switch_(median|p95|max)$")))' "${PRD}"
  check "product-redraw: 200 values per case and the scope is the product view()" jq -e '.conditions.trials * .conditions.samples_per_trial == 200 and (.conditions.scope_note | startswith("product view()"))' "${PRD}"
  check "product-redraw: sizes, judged kinds and the toggle gap are recorded" jq -e '.conditions.sizes.document.rows == 200 and .conditions.sizes.diff.rows == 500 and .conditions.sizes.task_tree.rows == 50 and .conditions.judged == ["task_tree", "document", "diff"] and (.conditions.ops.toggle | startswith("not_applicable")) and .conditions.judgement.diff.partial == true' "${PRD}"
  check "product-redraw: the sample diff size is recorded" jq -e '.conditions.sample_diff_rows.total > 0 and .conditions.sample_diff_rows.hunk >= 1' "${PRD}"
  check "product-redraw: uses the test-hooks feature" jq -e '.baseline.features.cargo_features == ["test-hooks"]' "${PRD}"
else
  fail "product-redraw: product-redraw.sh failed (see ${PRD}.err)"
  cat "${PRD}.err" >&2
fi

# --- (j) view-redraw-cpu4x（左のビューの CPU 4 倍の記録。TASK-485 3/3。記録だけで判定に使わない。束には入れない単体の測定） ---
check "view-redraw-cpu4x.sh: unknown argument exits 2" status_is 2 "${ROOT_DIR}/tools/measure/view-redraw-cpu4x.sh" --unknown
check_fails "view-redraw-cpu4x: missing data/device baseline is refused" measure_browser_result view-redraw-cpu4x "$(measure_commit_json)" "${TMP}" '{}'
check_fails "view-redraw-cpu4x: a pack baseline alone is refused" measure_browser_result view-redraw-cpu4x "$(measure_commit_json)" "${TMP}" "{\"pack\":{\"id\":\"x\",\"sha256\":\"${ZERO64}\"},\"device\":${DEV_OK}}"
VRC="${TMP}/view-redraw-cpu4x.json"
if SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/measure/view-redraw-cpu4x.sh" >"${VRC}" 2>"${VRC}.err"; then
  check "view-redraw-cpu4x: passes the v1 validation" measure_validate "${VRC}"
  check "view-redraw-cpu4x: 24 measurements named with the cpu4x prefix" jq -e '(.measurements | length == 24) and (.measurements | all(.name | test("^view_redraw_cpu4x_(document|diff|task_tree)_[ab]_(toggle|switch)_(median|p95|max)$")))' "${VRC}"
  check "view-redraw-cpu4x: record only, 4x throttle, and no judgement field" jq -e '.conditions.cpu_throttle == 4 and .conditions.record_only == true and (.conditions | has("judgement") | not) and (.conditions | has("judged") | not)' "${VRC}"
  check "view-redraw-cpu4x: the record has the 3 kinds and no verdict" jq -e '(.conditions.record_only_cpu4x | keys == ["diff", "document", "task_tree"]) and (.conditions.record_only_cpu4x | [.. | objects | select(has("within") or has("judged"))] | length == 0)' "${VRC}"
  check "view-redraw-cpu4x: the calibration ratio reached the minimum" jq -e '.conditions.cpu_calibration.ratio_min >= 2 and .conditions.cpu_calibration.checks == 10' "${VRC}"
  check "view-redraw-cpu4x: the sample diff size and 200 values are recorded" jq -e '.conditions.sample_diff_rows.total > 0 and .conditions.trials * .conditions.samples_per_trial == 200' "${VRC}"
  check "view-redraw-cpu4x: uses the test-hooks feature" jq -e '.baseline.features.cargo_features == ["test-hooks"]' "${VRC}"
else
  fail "view-redraw-cpu4x: view-redraw-cpu4x.sh failed (see ${VRC}.err)"
  cat "${VRC}.err" >&2
fi

# --- (k) product-redraw-cpu4x（本体の画面の CPU 4 倍の記録。TASK-485 3/3。記録だけで判定に使わない。束には入れない単体の測定） ---
check "product-redraw-cpu4x.sh: unknown argument exits 2" status_is 2 "${ROOT_DIR}/tools/measure/product-redraw-cpu4x.sh" --unknown
check_fails "product-redraw-cpu4x: missing pack/device baseline is refused" measure_browser_result product-redraw-cpu4x "$(measure_commit_json)" "${TMP}" '{}'
check_fails "product-redraw-cpu4x: a data baseline alone is refused" measure_browser_result product-redraw-cpu4x "$(measure_commit_json)" "${TMP}" "{\"data\":{\"id\":\"x\",\"manifest_sha256\":\"${ZERO64}\",\"files\":{\"a\":\"b\"}},\"device\":${DEV_OK}}"
PRC="${TMP}/product-redraw-cpu4x.json"
if SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/measure/product-redraw-cpu4x.sh" >"${PRC}" 2>"${PRC}.err"; then
  check "product-redraw-cpu4x: passes the v1 validation" measure_validate "${PRC}"
  check "product-redraw-cpu4x: 9 measurements named with the cpu4x prefix" jq -e '(.measurements | length == 9) and (.measurements | all(.name | test("^product_redraw_cpu4x_(task_tree_50|document_200|diff_500)_switch_(median|p95|max)$")))' "${PRC}"
  check "product-redraw-cpu4x: record only, 4x throttle, and no judgement field" jq -e '.conditions.cpu_throttle == 4 and .conditions.record_only == true and (.conditions | has("judgement") | not) and (.conditions | has("judged") | not)' "${PRC}"
  check "product-redraw-cpu4x: the record has the 3 kinds and no verdict" jq -e '(.conditions.record_only_cpu4x | keys == ["diff", "document", "task_tree"]) and (.conditions.record_only_cpu4x | [.. | objects | select(has("within") or has("judged") or has("partial"))] | length == 0)' "${PRC}"
  check "product-redraw-cpu4x: the calibration ratio reached the minimum" jq -e '.conditions.cpu_calibration.ratio_min >= 2 and .conditions.cpu_calibration.checks == 30' "${PRC}"
  check "product-redraw-cpu4x: the sample diff size is recorded" jq -e '.conditions.sample_diff_rows.total > 0 and .conditions.sample_diff_rows.hunk >= 1' "${PRC}"
  check "product-redraw-cpu4x: uses the test-hooks feature" jq -e '.baseline.features.cargo_features == ["test-hooks"]' "${PRC}"
else
  fail "product-redraw-cpu4x: product-redraw-cpu4x.sh failed (see ${PRC}.err)"
  cat "${PRC}.err" >&2
fi

# --- (l) first-paint-direct（IR つきの直接リンクで操作できるまで。TASK-108。束には入れない単体の測定。終了コードは 0 か 3） ---
check "first-paint-direct.sh: unknown argument exits 2" status_is 2 "${ROOT_DIR}/tools/measure/first-paint-direct.sh" --unknown
FPD="${TMP}/first-paint-direct.json"
FPD_RC=0
SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/measure/first-paint-direct.sh" >"${FPD}" 2>"${FPD}.err" || FPD_RC=$?
if [ "${FPD_RC}" -eq 0 ] || [ "${FPD_RC}" -eq 3 ]; then
  pass "first-paint-direct: exits 0 or 3 (got ${FPD_RC})"
  check "first-paint-direct: passes the v1 validation" measure_validate "${FPD}"
  FPD_VERDICT=0
  measure_first_paint_direct_verdict "${FPD}" || FPD_VERDICT=$?
  if [ "${FPD_VERDICT}" -eq "${FPD_RC}" ]; then pass "first-paint-direct: verdict agrees with the exit code"; else fail "first-paint-direct: verdict ${FPD_VERDICT} differs from exit code ${FPD_RC}"; fi
  check "first-paint-direct: 5 measurements and 10 values" jq -e '(.measurements | length == 5) and (.conditions.values_us | length == 10)' "${FPD}"
  check "first-paint-direct: the pack id is separated and the device has no cpu" jq -e '.baseline.pack.id == "measure-first-paint-direct" and (.baseline.device | has("cpu") | not)' "${FPD}"
  check "first-paint-direct: the IR was transferred" jq -e '[.measurements[] | select(.name == "first_paint_direct_ir_transfer")][0].value > 0' "${FPD}"
  check "first-paint-direct: delivery build without test hooks" jq -e '.baseline.features.config == "delivery" and .baseline.features.cargo_features == []' "${FPD}"
else
  fail "first-paint-direct: first-paint-direct.sh failed with ${FPD_RC} (see ${FPD}.err)"
  cat "${FPD}.err" >&2
fi

# --- (i) 統計関数と引数の試験 ---
check "node:test for browser.mjs passes" node --test "${ROOT_DIR}/tools/measure/browser.test.mjs"

if [ "${FAILURES}" -ne 0 ]; then
  echo "${FAILURES} check(s) failed" >&2
  exit 1
fi
echo "all checks passed" >&2
