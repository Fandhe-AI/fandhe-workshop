#!/usr/bin/env bash
# scripts/check-ignored-tests.sh 自体の回帰テスト（`make test-check-ignored-tests` から呼ばれる。CI の check ジョブも流す）。
#
# 「ALLOWED_IGNORED と crates/ の #[ignore] の現物がずれると検査が失敗する」ことを確かめる。
# 作業ツリーは変えない。検査スクリプトと見本の crate・clean-room.sh を mktemp -d の疑似 repo へ組み立てて流す。
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
mkdir -p "${REPO}/scripts" "${REPO}/tools" "${REPO}/crates/c/src" "${REPO}/crates/c/tests"
cp "${DIR}/check-ignored-tests.sh" "${DIR}/lib.sh" "${REPO}/scripts/"

# list <names...>: 見本の clean-room.sh を書く（コメント中の ALLOWED_IGNORED は数えられないことの確認を兼ねる）。
list() {
  {
    printf '#!/usr/bin/env bash\n# ALLOWED_IGNORED は下の 1 行。readonly ALLOWED_IGNORED=(not_a_real_one) とコメントに書いても数えない\n'
    printf 'readonly ALLOWED_IGNORED=(%s)\n' "$*"
  } >"${REPO}/tools/clean-room.sh"
}

# base_sources: 一致する基準の現物（= で理由を付けた形・素の形・src の中・属性の順序違いを含む）。
base_sources() {
  rm -f "${REPO}"/crates/c/tests/*.rs "${REPO}"/crates/c/src/*.rs
  {
    printf '//! #[ignore] をコメントで言及するだけの行は数えない。\n'
    printf '/// #[ignore] fn doc_only() {}\n'
    printf '// #[ignore]\n'
    printf '#[test]\n#[ignore = "理由"]\nfn alpha() {}\n\n'
    printf '#[ignore]\n#[test]\npub fn beta() {}\n'
  } >"${REPO}/crates/c/tests/a.rs"
  printf '#[ignore = "x"]\n\n// memo\n#[test]\nasync fn gamma() {}\n' >"${REPO}/crates/c/src/lib.rs"
}

# run_check <args...>: 疑似 repo の検査を流し、終了コードを RC に入れる。
run_check() {
  RC=0
  "${REPO}/scripts/check-ignored-tests.sh" "$@" >/dev/null 2>"${TMP_DIR}/stderr.txt" || RC=$?
}

base_sources
list alpha beta gamma
run_check
expect "match (assignment form, bare form, src, attribute order, comments ignored)" 0 "${RC}"

list alpha beta
run_check
expect "ignored test missing from list" 1 "${RC}"
expect_stderr "missing: name reported" "gamma"
expect_stderr "missing: path reported" "crates/c/src/lib.rs:1"

list alpha beta gamma stale
run_check
expect "list has name without #[ignore]" 1 "${RC}"
expect_stderr "stale: name reported" "stale"

list alpha stale
run_check
expect "missing and stale at once" 1 "${RC}"
expect_stderr "both: missing name" "beta"
expect_stderr "both: stale name" "stale"

list alpha beta gamma gamma
run_check
expect "duplicate name in list" 1 "${RC}"

# #[ignore] の後に fn が来ない。
base_sources
list alpha beta gamma
printf '#[ignore]\nstruct NotAFn;\n' >"${REPO}/crates/c/tests/b.rs"
run_check
expect "#[ignore] not followed by fn is undecidable" 2 "${RC}"
rm -f "${REPO}/crates/c/tests/b.rs"

# 同じ行に fn が続く形は取れる。
printf '#[ignore] fn delta() {}\n' >"${REPO}/crates/c/tests/b.rs"
list alpha beta gamma delta
run_check
expect "fn on the same line" 0 "${RC}"
rm -f "${REPO}/crates/c/tests/b.rs"

# 一覧の行の形の異常。
printf '#!/usr/bin/env bash\n# no declaration\n' >"${REPO}/tools/clean-room.sh"
run_check
expect "no ALLOWED_IGNORED line" 2 "${RC}"

printf 'readonly ALLOWED_IGNORED=(alpha)\nreadonly ALLOWED_IGNORED=(beta)\n' >"${REPO}/tools/clean-room.sh"
run_check
expect "two ALLOWED_IGNORED lines" 2 "${RC}"

# shellcheck disable=SC2016 # 展開されない文字列として一覧に書き込むのが目的
list 'alpha beta; gamma' 'x$(id)'
run_check
expect "non-identifier element" 2 "${RC}"

list alpha beta gamma
run_check unexpected-arg
expect "unknown argument" 2 "${RC}"

# 本物の repo: 現状で一致すること。
RC=0
"${DIR}/check-ignored-tests.sh" >/dev/null 2>"${TMP_DIR}/stderr.txt" || RC=$?
expect "real repository matches" 0 "${RC}"

echo "1..$((passed + failed))"
echo "# passed: ${passed}, failed: ${failed}"
[ "${failed}" -eq 0 ]
