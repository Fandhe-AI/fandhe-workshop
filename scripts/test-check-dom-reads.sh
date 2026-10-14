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
require_cmd rm
require_cmd mkdir
require_cmd chmod
require_cmd cat
require_cmd id

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

# 11b. children の関連関数形式（レシーバを引数で渡す）も検出する
tree="$(fresh_tree "children-assoc" 0)"
printf 'fn a() { let _ = web_sys::Element::children(&el); }\n' >"${tree}/crates/web/src/hello.rs"
run_check "${tree}"
expect "Type::children(&el) form is rejected" 1 "${RC}"

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

# 13b. find が一部しか列挙できないとき（読めないサブディレクトリ）は成功にしない
# root は権限を無視するため、この確認は非 root のときだけ行う
if [ "$(id -u)" != "0" ]; then
  tree="$(fresh_tree findfail 1)"
  printf 'fn a() { let v = field.value(); }\n' >"${tree}/crates/web/src/glue.rs"
  mkdir -p "${tree}/crates/web/src/locked"
  chmod 000 "${tree}/crates/web/src/locked"
  run_check "${tree}"
  chmod 755 "${tree}/crates/web/src/locked"
  expect "find failure fails closed" 2 "${RC}"
fi

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

# 16. 同じ行に field.value() が 2 件あっても、呼び出し単位で数えて固定値 1 を超えたら失敗する
tree="$(fresh_tree sameline 1)"
printf 'fn a() { let v = (field.value(), field.value()); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "two exception calls on one line exceed the fixed value" 1 "${RC}"

# 17. 例外の呼び出しと同じ行にある別の .value() は例外にならない
tree="$(fresh_tree mixedline 1)"
printf 'fn a() { let v = (field.value(), other.value()); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "other .value() on the same line as the exception is rejected" 1 "${RC}"

# 18. 空白・改行を含む value 呼び出しも検出する
for call in 'el.value ()' 'el.value( )' 'el\n    .value(\n    )' 'el . value ( )'; do
  tree="$(fresh_tree "ws-${#call}" 0)"
  printf 'fn a() {\n  let v = %b;\n}\n' "${call}" >"${tree}/crates/web/src/hello.rs"
  run_check "${tree}"
  expect "value call with whitespace is rejected: ${call}" 1 "${RC}"
done

# 19. 空白を含む例外の呼び出しは、例外として数える
tree="$(fresh_tree wsexc 1)"
printf 'fn a() {\n  let v = field\n    .value (\n    );\n}\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "exception call split across lines is counted" 0 "${RC}"

# 20. setter は読み取りではないので失敗にしない
tree="$(fresh_tree setters 0)"
printf 'fn a() {\n  el.set_text_content(Some("x"));\n  el.set_inner_html("x");\n  el.set_value("x");\n  el.set_outer_html("x");\n  el.set_inner_text("x");\n}\n' >"${tree}/crates/web/src/hello.rs"
run_check "${tree}"
expect "setters are not rejected" 0 "${RC}"

# 21. 空白を挟んだ getter 呼び出しは失敗にする
tree="$(fresh_tree wsgetter 0)"
printf 'fn a() { el . text_content ( ); }\n' >"${tree}/crates/web/src/hello.rs"
run_check "${tree}"
expect "spaced text_content getter is rejected" 1 "${RC}"

# 22. コメントを挟んだ value 呼び出しも検出する
for call in 'el.value(/* current value */)' 'el./* read */value()' 'el.value(/* a\n b */)' 'el . /* r */ value ( // x\n )' 'el./* /* nested */ */value()'; do
  tree="$(fresh_tree "cm-${#call}" 0)"
  printf 'fn a() {\n  let v = %b;\n}\n' "${call}" >"${tree}/crates/web/src/hello.rs"
  run_check "${tree}"
  expect "value call split by a comment is rejected: ${call}" 1 "${RC}"
done

# 22b. 関連関数形式の value 読み取りも検出する（例外の field があっても許可しない）
for call in 'web_sys::HtmlTextAreaElement::value(&field)' 'web_sys::HtmlInputElement::value(&el)' 'HtmlInputElement :: value ( &el )' 'web_sys::HtmlInputElement::/* r */value(&el)'; do
  tree="$(fresh_tree "assoc-${#call}" 0)"
  printf 'fn a() {\n  let v = %s;\n}\n' "${call}" >"${tree}/crates/web/src/hello.rs"
  run_check "${tree}"
  expect "associated-function value call is rejected: ${call}" 1 "${RC}"
done
tree="$(fresh_tree assoc-glue 1)"
printf 'fn a() { let v = field.value(); let w = web_sys::HtmlTextAreaElement::value(&field); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "associated-function value call in glue.rs is not an exception" 1 "${RC}"

# 23. ブロックコメント内・行末コメント内の呼び出しは数えない
tree="$(fresh_tree incomment 0)"
printf 'fn a() {\n  /* el.value() */\n  let x = 1; // el.value()\n  /*\n el.text_content\n */\n}\n' >"${tree}/crates/web/src/hello.rs"
run_check "${tree}"
expect "matches inside comments are ignored" 0 "${RC}"

# 24. 文字列・生文字列・文字リテラルの中の // や /* はコメントではない（後続の呼び出しを隠さない）
tree="$(fresh_tree strslash 0)"
printf 'fn a() { let u = "http://x"; el.value(); }\n' >"${tree}/crates/web/src/hello.rs"
run_check "${tree}"
expect "// inside a string does not hide a later call" 1 "${RC}"
tree="$(fresh_tree rawslash 0)"
cat >"${tree}/crates/web/src/hello.rs" <<'EOF'
fn a() { let u = r#"/* "#; el.value(); let c = '"'; }
EOF
run_check "${tree}"
expect "raw string does not hide a later call" 1 "${RC}"
tree="$(fresh_tree charquote 0)"
cat >"${tree}/crates/web/src/hello.rs" <<'EOF'
fn a() { let c = '"'; el.value(); }
EOF
run_check "${tree}"
expect "quote char literal does not hide a later call" 1 "${RC}"

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
