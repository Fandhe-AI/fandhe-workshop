#!/usr/bin/env bash
# scripts/check-wasm-size.sh 自体の回帰テスト（make test-check-wasm-size・CI の browser-test から呼ばれる）。
#
# ビルドはしない。--result に用意した JSON を渡し、判定（終了コードと出力）だけを確かめる。速く、ネットワークも使わない。
# 出力は TAP 風。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd jq
require_cmd mktemp
require_cmd rm

CHECK="${DIR}/check-wasm-size.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0
SHA="0123456789abcdef0123456789abcdef01234567"

ok() {
  echo "ok - $1"
  passed=$((passed + 1))
}
not_ok() {
  echo "not ok - $1"
  failed=$((failed + 1))
}

# make_json <value> <config> <features-json> <profile> <dirty> [applied] [detail]: 様式 v1 の結果を標準出力へ出す。
make_json() {
  jq -n --argjson v "$1" --arg config "$2" --argjson feats "$3" --arg profile "$4" --argjson dirty "$5" --argjson applied "${6:-false}" --arg detail "${7:-skipped}" --arg sha "${SHA}" '{
    schema: "fandhe-workshop/measure/v1",
    measured_at: "2026-10-20T00:00:00Z",
    baseline: {
      commit: {sha: $sha, dirty: $dirty},
      wasm_opt: {applied: $applied, detail: $detail},
      features: {config: $config, cargo_features: $feats, profile: $profile},
      compression: {command: "gzip -9 -c", implementation: "gzip"}
    },
    toolchain: {rustc: "x", wasm_bindgen: "y"},
    measurements: [
      {name: "wasm_raw", file: "a.wasm", unit: "bytes", value: 2000000},
      {name: "wasm_gzip", file: "a.wasm", unit: "bytes", value: $v}
    ]
  }'
}

# run_case <name> <expected-rc> <file> [expected stderr substring] [extra args...]
run_case() {
  local name="$1" want="$2" file="$3" needle="${4:-}" rc=0 err="${TMP_DIR}/err" out="${TMP_DIR}/out"
  shift 4 || shift "$#"
  "${CHECK}" --result "${file}" "$@" >"${out}" 2>"${err}" || rc=$?
  if [ "${rc}" != "${want}" ]; then
    not_ok "${name} (expected exit ${want}, got ${rc})"
    return
  fi
  if [ -n "${needle}" ] && ! grep -qF -- "${needle}" "${err}"; then
    not_ok "${name} (stderr lacks: ${needle})"
    return
  fi
  ok "${name} (exit ${rc})"
}

F="${TMP_DIR}/case.json"

make_json 1048576 delivery '[]' release false >"${F}"
run_case "exactly the limit passes" 0 "${F}" "limit 1048576"
make_json 1048577 delivery '[]' release false >"${F}"
run_case "one byte over fails" 1 "${F}" "exceeds the limit by 1 bytes"

make_json 353359 delivery '[]' release false >"${F}"
run_case "typical value passes" 0 "${F}" "353359"
if "${CHECK}" --result "${F}" 2>/dev/null | jq -e --arg sha "${SHA}" \
  '.gate.limit_bytes == 1048576 and .gate.verdict == "pass" and .baseline.commit.sha == $sha and (.baseline.wasm_opt | has("applied"))' >/dev/null; then
  ok "stdout JSON keeps gate and baseline"
else
  not_ok "stdout JSON keeps gate and baseline"
fi
make_json 1048577 delivery '[]' release false >"${F}"
rc=0
"${CHECK}" --result "${F}" >"${TMP_DIR}/out" 2>/dev/null || rc=$?
if jq -e '.gate.verdict == "fail"' "${TMP_DIR}/out" >/dev/null; then
  ok "stdout JSON verdict is fail when over"
else
  not_ok "stdout JSON verdict is fail when over"
fi

make_json 353359 delivery '[]' release true >"${F}"
run_case "dirty commit still passes and is logged" 0 "${F}" "dirty: true"

make_json 353359 test '[]' release false >"${F}"
run_case "test config is refused" 2 "${F}"
make_json 353359 delivery '["test-hooks"]' release false >"${F}"
run_case "test-hooks feature is refused" 2 "${F}"
make_json 353359 delivery '[]' debug false >"${F}"
run_case "non-release profile is refused" 2 "${F}"

make_json 353359 delivery '[]' release false | jq 'del(.measurements[] | select(.name == "wasm_gzip"))' >"${F}"
run_case "missing wasm_gzip is refused" 2 "${F}"
make_json 353359 delivery '[]' release false | jq '.measurements += [.measurements[1]]' >"${F}"
run_case "duplicate wasm_gzip is refused" 2 "${F}"
make_json 353359 delivery '[]' release false | jq '.measurements |= map(if .name == "wasm_gzip" then .unit = "kb" else . end)' >"${F}"
run_case "non-bytes unit is refused" 2 "${F}"

make_json 1e20 delivery '[]' release false >"${F}"
run_case "1e20 value is refused" 2 "${F}"
make_json 1.5 delivery '[]' release false >"${F}"
run_case "non-integer value is refused" 2 "${F}"
make_json -1 delivery '[]' release false >"${F}"
run_case "negative value is refused" 2 "${F}"
make_json '"1"' delivery '[]' release false >"${F}"
run_case "string value is refused" 2 "${F}"

make_json 353359 delivery '[]' release false | jq 'del(.baseline.commit)' >"${F}"
run_case "missing commit baseline is refused" 2 "${F}"
make_json 353359 delivery '[]' release false | jq 'del(.baseline.wasm_opt)' >"${F}"
run_case "missing wasm_opt baseline is refused" 2 "${F}"

{
  make_json 353359 delivery '[]' release false | jq 'del(.baseline.commit)'
  make_json 353359 delivery '[]' release false
} >"${F}"
run_case "concatenated JSON values are refused" 2 "${F}"

printf '{broken' >"${F}"
run_case "broken JSON is refused" 2 "${F}"
: >"${F}"
run_case "empty file is refused" 2 "${F}"
run_case "missing file is refused" 2 "${TMP_DIR}/nope.json"

rc=0
"${CHECK}" --bogus >/dev/null 2>&1 || rc=$?
if [ "${rc}" = "2" ]; then ok "unknown argument is refused"; else not_ok "unknown argument is refused (got ${rc})"; fi
rc=0
"${CHECK}" --result >/dev/null 2>&1 || rc=$?
if [ "${rc}" = "2" ]; then ok "--result without a value is refused"; else not_ok "--result without a value is refused (got ${rc})"; fi

# DLV-4: --require-wasm-opt
make_json 353359 delivery '[]' release false true "wasm-opt version 116" >"${F}"
run_case "require-wasm-opt with applied passes" 0 "${F}" "wasm-opt: applied" --require-wasm-opt
make_json 353359 delivery '[]' release false false skipped >"${F}"
run_case "require-wasm-opt without applied is refused" 2 "${F}" "without wasm-opt" --require-wasm-opt
make_json 353359 delivery '[]' release false | jq 'del(.baseline.wasm_opt)' >"${F}"
run_case "require-wasm-opt with missing wasm_opt is refused" 2 "${F}" "" --require-wasm-opt
make_json 353359 delivery '[]' release false | jq '.baseline.wasm_opt.applied = "true"' >"${F}"
run_case "require-wasm-opt with string applied is refused" 2 "${F}" "" --require-wasm-opt
make_json 353359 delivery '[]' release false true "wasm-opt version 116" >"${F}"
run_case "applied without the flag passes" 0 "${F}" "wasm-opt: applied"
make_json 1048577 delivery '[]' release false true "wasm-opt version 116" >"${F}"
run_case "require-wasm-opt still fails when over" 1 "${F}" "exceeds the limit by 1 bytes" --require-wasm-opt

echo "pass ${passed} / fail ${failed}"
[ "${failed}" -eq 0 ]
