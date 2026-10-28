#!/usr/bin/env bash
# tools/clean-room.sh の judge と引数の扱いの回帰テスト（make test-clean-room。docker は使わない。CI の check ジョブが流す）。
#
# 合成の run.log を judge に渡し、終了コード（0 合格 / 1 不合格 / 2 判定不能）が tools/clean-room.sh の冒頭コメントの契約どおりであることを確かめる。
# 実際のコンテナでの実行は make clean-room（遅く、docker と通信が要る）。
#
# 終了コード: 0 = 全件合格 / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"
require_cmd mktemp
require_cmd rm
TOOL="${ROOT_DIR}/tools/clean-room.sh"
M="### clean-room:"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-clean-room.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

passed=0
failed=0
expect() {
  local label="$1" want="$2" got="$3"
  if [ "${want}" = "${got}" ]; then
    echo "ok - ${label}"
    passed=$((passed + 1))
  else
    echo "not ok - ${label} (want ${want}, got ${got})"
    failed=$((failed + 1))
  fi
}

# section <name> <exit> <body>: 1 回の実行の区切りと本文を出す。
section() {
  echo "${M} begin $1"
  printf '%s\n' "$3"
  echo "${M} seconds $1 5"
  echo "${M} end $1 exit=$2"
}

OK_BODY='test a::b ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s'
IGN_BODY='test core::emit_histories_for_browser ... ignored
test web::emit_round_handoff_for_browser ... ignored, writes a file
test result: ok. 3 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.1s'

# mk <name> <pre-line> <default-exit> <default-body> <workspace-exit> <workspace-body>
mk() {
  local d="${TMP}/$1"
  mkdir -p "${d}"
  {
    echo "${M} $2"
    echo "${M} rustc rustc 1.98.1"
    section default "$3" "$4"
    section workspace "$5" "$6"
  } >"${d}/run.log"
}
# judge <name>: judge の終了コードを標準出力へ出す。
judge() {
  local code=0
  "${TOOL}" judge "${TMP}/$1" >/dev/null 2>&1 || code=$?
  echo "${code}"
}

mk good "precondition ok" 0 "${OK_BODY}" 0 "${IGN_BODY}"
expect "all pass with allowed ignored" 0 "$(judge good)"

mk drc "precondition ok" 0 "${OK_BODY}" 0 "${IGN_BODY}"
printf 'container_exit=0\n' >"${TMP}/drc/meta.txt"
expect "container_exit 0 passes" 0 "$(judge drc)"
printf 'container_exit=137\n' >"${TMP}/drc/meta.txt"
expect "container_exit nonzero with good log" 2 "$(judge drc)"
printf 'wall_seconds=1\n' >"${TMP}/drc/meta.txt"
expect "meta without container_exit" 2 "$(judge drc)"

mk failed "precondition ok" 0 "${OK_BODY}" 101 'test a ... FAILED
test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s'
expect "failed test" 1 "$(judge failed)"

mk compile "precondition ok" 101 "${OK_BODY}" 0 "${OK_BODY}"
expect "cargo exit nonzero with failed 0" 1 "$(judge compile)"

mk badignore "precondition ok" 0 "${OK_BODY}" 0 'test x::measure_foo ... ignored
test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.1s'
expect "ignored not in allow list" 1 "$(judge badignore)"

mk noresult "precondition ok" 0 "${OK_BODY}" 0 'Compiling x'
expect "no test result line" 2 "$(judge noresult)"

mkdir -p "${TMP}/cut"
{
  echo "${M} precondition ok"
  section default 0 "${OK_BODY}"
  echo "${M} begin workspace"
  echo 'test a ... ok'
} >"${TMP}/cut/run.log"
expect "log cut before end line" 2 "$(judge cut)"

mk impure "precondition fail secret-like-env-1" 0 "${OK_BODY}" 0 "${OK_BODY}"
expect "precondition failure" 1 "$(judge impure)"

mk nopre "x" 0 "${OK_BODY}" 0 "${OK_BODY}"
expect "no precondition line" 2 "$(judge nopre)"

mk mismatch "precondition ok" 0 "${OK_BODY}" 0 'test a::emit_histories_for_browser ... ignored
test result: ok. 3 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.1s'
expect "ignored total differs from lines" 2 "$(judge mismatch)"

mk unknownres "precondition ok" 0 "${OK_BODY}" 0 'test result: unknown'
expect "unparsable test result line" 2 "$(judge unknownres)"

mk nonnumres "precondition ok" 0 "${OK_BODY}" 0 'test result: ok. x passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s'
expect "non-numeric test result field" 2 "$(judge nonnumres)"

mk partialres "precondition ok" 0 "${OK_BODY}" 0 "${OK_BODY}
test result: ok. 3 passed; 0 measured; 0 filtered out; finished in 0.1s"
expect "test result line missing a field" 2 "$(judge partialres)"

code=0
"${TOOL}" judge >/dev/null 2>&1 || code=$?
expect "judge without dir" 2 "${code}"
code=0
"${TOOL}" judge "${TMP}/nonexistent" >/dev/null 2>&1 || code=$?
expect "judge with missing dir" 2 "${code}"
code=0
"${TOOL}" --bogus >/dev/null 2>&1 || code=$?
expect "unknown argument" 2 "${code}"
code=0
CONTAINER_RUNTIME=foo "${TOOL}" run >/dev/null 2>&1 || code=$?
expect "invalid CONTAINER_RUNTIME" 2 "${code}"

# docker が PATH に無い環境では、未検査のまま成功にしない。
BIN="${TMP}/bin"
mkdir -p "${BIN}"
for c in bash git awk grep sed head tr wc cat env dirname basename mkdir ls uname sha256sum shasum; do
  p="$(command -v "${c}" || true)"
  if [ -n "${p}" ]; then ln -s "${p}" "${BIN}/${c}"; fi
done
code=0
PATH="${BIN}" "${TOOL}" run >/dev/null 2>&1 || code=$?
expect "no container runtime" 2 "${code}"

echo "passed=${passed} failed=${failed}"
if [ "${failed}" -ne 0 ]; then exit 1; fi
