#!/usr/bin/env bash
# tools/measure/wasm-size.sh と tools/measure/lib.sh の受入基準を確かめる（make test-measure-wasm-size から呼ばれる）。
#
# 確かめること: 結果の様式の検証（基点の欠落を拒否し、失敗時は標準出力に何も出さない）、誤用の拒否、
# 基点を集められないときの fail-closed（ビルド前に止まる）、実ビルドの結果が基点 4 項目を持つこと。
# 実ビルドを含むため遅い。make check・フック・CI には入れない（CI の関門は TASK-78）。
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

SCRIPT="${ROOT_DIR}/tools/measure/wasm-size.sh"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-measure.XXXXXX")"
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
  # jq -e の "true" などの標準出力はログを汚すだけなので捨てる。
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

# --- (a) 様式の検証（ビルドなし） ---
GOOD="${TMP}/good.json"
cat >"${GOOD}" <<'EOF'
{
  "schema": "fandhe-workshop/measure/v1",
  "measured_at": "2026-10-14T00:00:00Z",
  "baseline": {
    "commit": {"sha": "0123456789abcdef0123456789abcdef01234567", "dirty": false},
    "wasm_opt": {"applied": false, "detail": "SKIP_WASM_OPT=1"},
    "features": {"config": "delivery", "cargo_features": [], "profile": "release"},
    "compression": {"command": "gzip -9 -c", "implementation": "gzip 1.14"}
  },
  "measurements": [{"name": "wasm_gzip", "file": "a.wasm", "unit": "bytes", "value": 10}]
}
EOF
check "validate accepts a well-formed result" measure_validate "${GOOD}"

# bad <説明> <jq フィルタ>: GOOD を壊した JSON を作り、検証が拒否することと emit が標準出力へ何も出さないことを確かめる。
bad() {
  local desc="$1" filter="$2" f out
  f="${TMP}/bad.json"
  jq "${filter}" "${GOOD}" >"${f}"
  check_fails "validate rejects: ${desc}" measure_validate "${f}"
  out="$(measure_emit "${f}" 2>/dev/null || true)"
  if [ -z "${out}" ]; then pass "emit prints nothing: ${desc}"; else fail "emit printed output: ${desc}"; fi
  check_fails "emit fails: ${desc}" measure_emit "${f}"
}
bad "sha is not 40 hex digits" '.baseline.commit.sha = "abc"'
bad "commit is missing" 'del(.baseline.commit)'
bad "dirty is missing" 'del(.baseline.commit.dirty)'
bad "wasm_opt.applied is missing" 'del(.baseline.wasm_opt.applied)'
bad "wasm_opt.detail is empty" '.baseline.wasm_opt.detail = ""'
bad "features is missing" 'del(.baseline.features)'
bad "features.config is unknown" '.baseline.features.config = "other"'
bad "compression.implementation is empty" '.baseline.compression.implementation = ""'
bad "measurements is empty" '.measurements = []'
bad "value is 0" '.measurements[0].value = 0'
bad "schema differs" '.schema = "x"'

# 複数の JSON 値の入力（基点の欠けた値の後ろに正常な値を連結）は、emit が何も出さず失敗する。
MULTI="${TMP}/multi.json"
{ jq 'del(.baseline.commit)' "${GOOD}"; jq . "${GOOD}"; } >"${MULTI}"
check_fails "validate rejects: multiple JSON values (bad then good)" measure_validate "${MULTI}"
MULTI_OUT="$(measure_emit "${MULTI}" 2>/dev/null || true)"
if [ -z "${MULTI_OUT}" ]; then pass "emit prints nothing: multiple JSON values"; else fail "emit printed output: multiple JSON values"; fi
{ jq . "${GOOD}"; jq . "${GOOD}"; } >"${MULTI}"
check_fails "validate rejects: two valid JSON values" measure_validate "${MULTI}"

# --- (b) 誤用 ---
check "unknown argument exits 2" status_is 2 "${SCRIPT}" --unknown
check "TEST_HOOKS=0 exits 2" status_is 2 env TEST_HOOKS=0 "${SCRIPT}"

# --- (c) 基点の欠落（git が失敗する。ビルド前に止まる） ---
mkdir -p "${TMP}/stub"
printf '#!/bin/sh\nexit 1\n' >"${TMP}/stub/git"
chmod 755 "${TMP}/stub/git"
STUB_OUT="$(PATH="${TMP}/stub:${PATH}" "${SCRIPT}" 2>/dev/null || true)"
check "git failure: stdout is empty" test -z "${STUB_OUT}"
check "git failure: exit code is 1" status_is 1 env PATH="${TMP}/stub:${PATH}" "${SCRIPT}"

# --- 実ビルド（(d)(e)(f)） ---
HEAD_SHA="$(git -C "${ROOT_DIR}" rev-parse HEAD)"

# run_measure <出力ファイル> <環境変数...>: 実ビルドの測定を実行し、標準出力を指定ファイルへ受ける。
run_measure() {
  local out="$1"
  shift
  env "$@" "${SCRIPT}" >"${out}" 2>"${out}.err"
}

R1="${TMP}/r1.json"
if run_measure "${R1}" SKIP_WASM_OPT=1; then
  check "skip: result passes validation" measure_validate "${R1}"
  check "skip: wasm_opt.applied is false" jq -e '.baseline.wasm_opt.applied == false' "${R1}"
  check "skip: detail mentions SKIP_WASM_OPT=1" jq -e '.baseline.wasm_opt.detail | contains("SKIP_WASM_OPT=1")' "${R1}"
  # shellcheck disable=SC2016 # $s は jq の変数であり、シェルで展開しない
  check "skip: commit.sha matches HEAD" jq -e --arg s "${HEAD_SHA}" '.baseline.commit.sha == $s' "${R1}"
  check "skip: config is delivery with no features" jq -e '.baseline.features.config == "delivery" and .baseline.features.cargo_features == []' "${R1}"
  check "skip: gzip is smaller than raw" jq -e '([.measurements[] | select(.name == "wasm_gzip") | .value][0]) < ([.measurements[] | select(.name == "wasm_raw") | .value][0])' "${R1}"
else
  fail "skip: wasm-size.sh failed (see ${R1}.err)"
  cat "${R1}.err" >&2
fi

R2="${TMP}/r2.json"
if run_measure "${R2}" SKIP_WASM_OPT=; then
  if command -v wasm-opt >/dev/null 2>&1; then
    check "wasm-opt present: applied is true" jq -e '.baseline.wasm_opt.applied == true' "${R2}"
    check "wasm-opt present: detail has a version" jq -e '.baseline.wasm_opt.detail | contains("version")' "${R2}"
  else
    check "wasm-opt absent: applied is false" jq -e '.baseline.wasm_opt.applied == false' "${R2}"
    check "wasm-opt absent: detail says not found" jq -e '.baseline.wasm_opt.detail | contains("wasm-opt not found")' "${R2}"
  fi
else
  fail "default: wasm-size.sh failed (see ${R2}.err)"
  cat "${R2}.err" >&2
fi

R3="${TMP}/r3.json"
if run_measure "${R3}" SKIP_WASM_OPT=1 TEST_HOOKS=1; then
  check "test config: features" jq -e '.baseline.features.config == "test" and .baseline.features.cargo_features == ["test-hooks"]' "${R3}"
else
  fail "test config: wasm-size.sh failed (see ${R3}.err)"
  cat "${R3}.err" >&2
fi

if [ "${FAILURES}" -ne 0 ]; then
  echo "${FAILURES} check(s) failed" >&2
  exit 1
fi
echo "all checks passed" >&2
