#!/usr/bin/env bash
# scripts/check-mutation-defs.sh 自体の回帰テスト（`make test-check-mutation-defs` から呼ばれる。CI の check ジョブも流す）。
#
# 「古い定義（find の出現が 1 回でない・expect-fail の名前が実在しない）を置くと静的検査が失敗する」ことを確かめる。
# 作業ツリーは変えない。検査スクリプトと小さな crate を mktemp -d の疑似 repo へ組み立てて流す。
# 出力は TAP 風。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd rm
require_cmd mktemp
require_cmd cp
require_cmd grep

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0

# expect <name> <expected-rc> <actual-rc>
expect() {
  if [ "$2" = "$3" ]; then
    echo "ok - $1 (exit $3)"
    passed=$((passed + 1))
  else
    echo "not ok - $1 (expected exit $2, got $3)"
    failed=$((failed + 1))
  fi
}

# expect_stderr <name> <fixed-string>: 直前の run_check の標準エラーに文字列があること。
expect_stderr() {
  if grep -Fq -- "$2" "${TMP_DIR}/stderr.txt"; then
    echo "ok - $1"
    passed=$((passed + 1))
  else
    echo "not ok - $1 (stderr lacks: $2)"
    failed=$((failed + 1))
  fi
}

REPO="${TMP_DIR}/repo"
mkdir -p "${REPO}/scripts" "${REPO}/crates/c/src" "${REPO}/crates/c/tests/mutations"
cp "${DIR}/check-mutation-defs.sh" "${DIR}/lib-mutations.sh" "${DIR}/lib.sh" "${REPO}/scripts/"
printf '[package]\nname = "fake-pkg"\nversion = "0.0.0"\n' >"${REPO}/crates/c/Cargo.toml"
printf 'let v = 1;\nlet w = 1;\nlet w = 1;\nlet x = (a.*b);\n' >"${REPO}/crates/c/src/lib.rs"
printf 'fn alpha() {}\nmod inner {\n    fn beta() {}\n}\nfn 日本語の試験() {}\nfn gamma_x() {}\n' >"${REPO}/crates/c/tests/fake_test.rs"
MUT="${REPO}/crates/c/tests/mutations"

# def <name> <find> <expect-fail...>: 定義を 1 件書く。
def() {
  local name="$1" find="$2"
  shift 2
  {
    printf 'id: %s\npackage: fake-pkg\ntest: fake_test\ntarget: crates/c/src/lib.rs\n' "${name}"
    printf 'find: %s\nreplace: zzz\n' "${find}"
    for e in "$@"; do printf 'expect-fail: %s\n' "${e}"; done
  } >"${MUT}/${name}.mutation"
}

# run_check [args...]: 終了コードを RC に入れる。
run_check() {
  RC=0
  "${REPO}/scripts/check-mutation-defs.sh" "$@" >/dev/null 2>"${TMP_DIR}/stderr.txt" || RC=$?
}

reset() { rm -f "${MUT}"/*.mutation; }

reset
def ok1 'let v = 1;' alpha
run_check
expect "valid definition passes" 0 "${RC}"

reset
def zero 'let nothing = 0;' alpha
run_check
expect "find occurring 0 times fails" 1 "${RC}"
expect_stderr "message names the definition" "crates/c/tests/mutations/zero.mutation"

reset
def twice 'let w = 1;' alpha
run_check
expect "find occurring 2 times fails" 1 "${RC}"

reset
def noname 'let v = 1;' missing_fn
run_check
expect "missing expect-fail name fails" 1 "${RC}"

reset
def bad1 'let nothing = 0;' alpha
def bad2 'let v = 1;' missing_fn
run_check
expect "two stale definitions fail" 1 "${RC}"
expect_stderr "first stale definition listed" "bad1.mutation"
expect_stderr "second stale definition listed" "bad2.mutation"

reset
def modok 'let v = 1;' inner::beta
run_check
expect "mod::fn name found passes" 0 "${RC}"

reset
def modng 'let v = 1;' nomod::beta
run_check
expect "unknown mod prefix fails" 1 "${RC}"

reset
def jp 'let v = 1;' 日本語の試験
run_check
expect "non-ascii fn name found passes" 0 "${RC}"

reset
def regex 'let x = (a.*b);' alpha
run_check
expect "regex metacharacters in find are literal" 0 "${RC}"

reset
def regname 'let v = 1;' 'alpha.*'
run_check
expect "regex metacharacters in name are literal" 1 "${RC}"

reset
def ok1 'let v = 1;' alpha
sed 's/^package: fake-pkg$/package: other-pkg/' "${MUT}/ok1.mutation" >"${MUT}/ok1.tmp" && mv "${MUT}/ok1.tmp" "${MUT}/ok1.mutation"
run_check
expect "unknown package is a usage-level failure" 2 "${RC}"

reset
def ok1 'let v = 1;' alpha
sed 's/^test: fake_test$/test: no_such_test/' "${MUT}/ok1.mutation" >"${MUT}/ok1.tmp" && mv "${MUT}/ok1.tmp" "${MUT}/ok1.mutation"
run_check
expect "missing test source is a usage-level failure" 2 "${RC}"

reset
def ok1 'let v = 1;' alpha
sed '/^replace:/d' "${MUT}/ok1.mutation" >"${MUT}/ok1.tmp" && mv "${MUT}/ok1.tmp" "${MUT}/ok1.mutation"
run_check
expect "missing key is a usage-level failure" 2 "${RC}"

reset
def ok1 'let v = 1;' alpha
printf 'bogus: x\n' >>"${MUT}/ok1.mutation"
run_check
expect "unknown key is a usage-level failure" 2 "${RC}"

reset
run_check
expect "no definitions is a usage-level failure" 2 "${RC}"

reset
def ok1 'let v = 1;' alpha
run_check extra
expect "extra argument is rejected" 2 "${RC}"

# 本物の repo: 現在の定義が通り、作業ツリーを変えない。
before="$(git -C "${DIR}/.." status --porcelain 2>/dev/null || true)"
RC=0
"${DIR}/check-mutation-defs.sh" >/dev/null 2>"${TMP_DIR}/stderr.txt" || RC=$?
after="$(git -C "${DIR}/.." status --porcelain 2>/dev/null || true)"
expect "real repository definitions pass" 0 "${RC}"
if [ "${before}" = "${after}" ]; then
  echo "ok - working tree unchanged"
  passed=$((passed + 1))
else
  echo "not ok - working tree changed"
  failed=$((failed + 1))
fi

echo "1..$((passed + failed))"
echo "# passed: ${passed}, failed: ${failed}"
[ "${failed}" -eq 0 ]
