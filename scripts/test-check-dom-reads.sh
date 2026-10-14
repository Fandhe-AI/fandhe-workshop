#!/usr/bin/env bash
# tools/check-deps.sh（DOM 読み取りの検査）自体の回帰テスト（`make test-check-dom-reads` から呼ばれる）。
#
# 「例外の件数が増減すると失敗する」「例外以外の .value() を検出する」ことを、手作業ではなくここで確かめる。
# 作業ツリーは変えない。検査スクリプトの写しと小さな web crate を mktemp -d の一時ディレクトリへ組み立てる。
# 固定値 1 の挙動は、写しの EXC_N を書き換えて確かめる（書き換えが空振りしたら試験自体を失敗にする）。
#
# 出力は TAP 風（scripts/test-check-testdata-refs.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd grep
require_cmd mktemp
require_cmd cp
require_cmd mv

CHECK_SRC="${DIR}/../tools/check-deps.sh"
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

# fresh_tree <name> <exc_n>: 最小の repo 構成を作り、そのパスを出力する。
# exc_n が 1 のときは写しの EXC_N=0 を EXC_N=1 に置き換える（sed -i は GNU/BSD で引数が違うので使わない）。
fresh_tree() {
  local dest="${TMP_DIR}/$1" exc_n="$2"
  mkdir -p "${dest}/tools" "${dest}/scripts" "${dest}/crates/web/src"
  cp "${DIR}/lib.sh" "${dest}/scripts/lib.sh"
  if [ "${exc_n}" = "1" ]; then
    while IFS= read -r line || [ -n "${line}" ]; do
      if [ "${line}" = "EXC_N=0" ]; then
        printf '%s\n' "EXC_N=1"
      else
        printf '%s\n' "${line}"
      fi
    done <"${CHECK_SRC}" >"${dest}/tools/check-deps.sh.tmp"
    mv "${dest}/tools/check-deps.sh.tmp" "${dest}/tools/check-deps.sh"
    if ! grep -qx 'EXC_N=1' "${dest}/tools/check-deps.sh"; then
      echo "not ok - fixture setup: EXC_N was not rewritten" >&2
      exit 1
    fi
  else
    cp "${CHECK_SRC}" "${dest}/tools/check-deps.sh"
  fi
  chmod 755 "${dest}/tools/check-deps.sh"
  printf 'pub fn run() {}\n' >"${dest}/crates/web/src/glue.rs"
  printf 'pub mod glue;\n' >"${dest}/crates/web/src/lib.rs"
  echo "${dest}"
}

# run_check <tree> [args...]: 写しの検査を実行し、終了コードを RC、標準出力を OUT に入れる。
run_check() {
  local tree="$1"
  shift
  RC=0
  "${tree}/tools/check-deps.sh" "$@" >"${TMP_DIR}/stdout.txt" 2>"${TMP_DIR}/stderr.txt" || RC=$?
  OUT="$(cat "${TMP_DIR}/stdout.txt")"
}

# 1. 現状相当（DOM 読み取りなし、固定値 0）は通る
tree="$(fresh_tree clean0 0)"
run_check "${tree}"
expect "clean tree passes with exception count 0" 0 "${RC}"

# 2. 固定値 0 で field.value() が 1 件あると失敗する（増加）
tree="$(fresh_tree inc0 0)"
printf 'fn a() { let v = field.value(); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "exception count above 0 is rejected" 1 "${RC}"

# 3. 固定値 1 で 1 件なら通る
tree="$(fresh_tree exact1 1)"
printf 'fn a() { let v = field.value(); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "exception count 1 passes with fixed value 1" 0 "${RC}"

# 4. 固定値 1 で 0 件なら失敗する（減少）
tree="$(fresh_tree dec1 1)"
run_check "${tree}"
expect "exception count below fixed value is rejected" 1 "${RC}"

# 5. 固定値 1 で 2 件なら失敗する（増加）
tree="$(fresh_tree inc1 1)"
printf 'fn a() { let v = field.value(); }\nfn b() { let w = field.value(); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "exception count above fixed value is rejected" 1 "${RC}"

# 6. 例外に加えて別の変数の .value() があれば、その行を出して失敗する
tree="$(fresh_tree other 1)"
printf 'fn a() { let v = field.value(); }\nfn b() { let w = other.value(); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "other .value() next to the exception is rejected" 1 "${RC}"
case "${OUT}" in
  *"other.value()"*) expect "offending line is reported" 0 0 ;;
  *) expect "offending line is reported" 0 1 ;;
esac

# 7. 別のファイルの field.value() は例外にならない
tree="$(fresh_tree otherfile 1)"
printf 'fn a() { let v = field.value(); }\n' >"${tree}/crates/web/src/glue.rs"
printf 'fn h() { let v = field.value(); }\n' >"${tree}/crates/web/src/hello.rs"
run_check "${tree}"
expect "field.value() in another file is rejected" 1 "${RC}"

# 8. サブディレクトリの glue.rs は例外にならない
tree="$(fresh_tree subdir 1)"
printf 'fn a() { let v = field.value(); }\n' >"${tree}/crates/web/src/glue.rs"
mkdir -p "${tree}/crates/web/src/sub"
printf 'fn h() { let v = field.value(); }\n' >"${tree}/crates/web/src/sub/glue.rs"
run_check "${tree}"
expect "field.value() in a nested glue.rs is rejected" 1 "${RC}"

# 9. get_attribute は例外があっても失敗する
tree="$(fresh_tree attr 1)"
printf 'fn a() { let v = field.value(); let t = el.get_attribute("data-testid"); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "get_attribute is rejected" 1 "${RC}"

# 10. コメント行の一致は数えない
tree="$(fresh_tree comments 1)"
printf 'fn a() { let v = field.value(); }\n// x.value() and get_attribute\n    /// text_content field.value()\n//! inner_html\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "comment-only matches are ignored" 0 "${RC}"

# 11. 判定 3 の各 API を 1 件ずつ入れると、それぞれ失敗する
for api in 'text_content' 'inner_html' 'inner_text' 'outer_html' 'get_attribute' 'get_element_by_id' \
  'get_elements_by_tag_name' 'first_child' 'children()' 'dataset' 'closest("x")' 'get_computed_style' \
  'get_bounding_client_rect'; do
  tree="$(fresh_tree "api-${api%%[^a-z_]*}" 0)"
  printf 'fn a() { el.%s; }\n' "${api}" >"${tree}/crates/web/src/hello.rs"
  run_check "${tree}"
  expect "API ${api} is rejected" 1 "${RC}"
done

# 12. crates/web/src が無いときは成功にしない
tree="$(fresh_tree nosrc 0)"
rm -rf "${tree}/crates/web/src"
run_check "${tree}"
expect "missing web source fails closed" 2 "${RC}"

# 13. glue.rs が無いときは成功にしない
tree="$(fresh_tree noglue 0)"
rm -f "${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "missing glue.rs fails closed" 2 "${RC}"

# 14. 誤用
tree="$(fresh_tree misuse 0)"
run_check "${tree}" --bogus
expect "unknown argument is a usage error" 2 "${RC}"
run_check "${tree}" --root
expect "--root without a value is a usage error" 2 "${RC}"

# 14b. --root 指定の経路（別の場所のツリーを検査する）
tree="$(fresh_tree rootopt 0)"
printf 'fn a() { let v = field.value(); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}" --root "${TMP_DIR}/clean0"
expect "--root points the check at another tree" 0 "${RC}"
run_check "${tree}" --root "${tree}"
expect "--root with a violating tree fails" 1 "${RC}"

# 15. 例外の行がインデント付き・行末コメント付きでも例外として数える
tree="$(fresh_tree indent 1)"
printf 'fn a() {\n    let v = field.value(); // textarea の現在値\n}\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "indented exception with trailing comment is counted" 0 "${RC}"

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
