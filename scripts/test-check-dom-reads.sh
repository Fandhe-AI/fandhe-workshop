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
# 写しの `EXC_N=<数字>` の行を `EXC_N=<exc_n>` に置き換える（本体の固定値に試験を依存させない。sed -i は GNU/BSD で引数が違うので使わない）。
fresh_tree() {
  local dest="${TMP_DIR}/$1" exc_n="$2"
  mkdir -p "${dest}/tools" "${dest}/scripts" "${dest}/crates/web/src"
  cp "${DIR}/lib.sh" "${dest}/scripts/lib.sh"
  while IFS= read -r line || [ -n "${line}" ]; do
    case "${line}" in
      EXC_N=[0-9]*) printf '%s\n' "EXC_N=${exc_n}" ;;
      *) printf '%s\n' "${line}" ;;
    esac
  done <"${CHECK_SRC}" >"${dest}/tools/check-deps.sh.tmp"
  mv "${dest}/tools/check-deps.sh.tmp" "${dest}/tools/check-deps.sh"
  if ! grep -qx "EXC_N=${exc_n}" "${dest}/tools/check-deps.sh"; then
    echo "not ok - fixture setup: EXC_N was not rewritten" >&2
    exit 1
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

# 25. raw identifier（r#）つきの呼び出しも通常の名前と同じに検出する（メソッド形式・関連関数形式）
for call in 'el.r#value()' 'el . r#value ( )' 'web_sys::HtmlInputElement::r#value(&el)' 'el.r#children()' \
  'web_sys::Element::r#children(&el)' 'el.r#closest("x")' 'el.r#text_content()' 'web_sys::Node::r#text_content(&el)' \
  'el.r#get_attribute("a")'; do
  tree="$(fresh_tree "raw-${#call}-${call%%[!a-z]*}" 0)"
  printf 'fn a() {\n  let v = %s;\n}\n' "${call}" >"${tree}/crates/web/src/hello.rs"
  run_check "${tree}"
  expect "raw identifier call is rejected: ${call}" 1 "${RC}"
done

# 25b. raw identifier の setter・別名は誤検出しない
tree="$(fresh_tree raw-setters 0)"
printf 'fn a() {\n  el.r#set_text_content(Some("x"));\n  el.r#set_inner_html("x");\n  el.r#set_value("x");\n  let r#valuex = 1;\n}\n' >"${tree}/crates/web/src/hello.rs"
run_check "${tree}"
expect "raw identifier setters are not rejected" 0 "${RC}"

# 25c. glue.rs の field.r#value() は例外として数えない（保守的に違反）
tree="$(fresh_tree raw-exc 1)"
printf 'fn a() { let v = field.value(); let w = field.r#value(); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "field.r#value() is not an exception" 1 "${RC}"
tree="$(fresh_tree raw-exc-only 1)"
printf 'fn a() { let w = field.r#value(); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "field.r#value() alone does not satisfy the exception count" 1 "${RC}"

# 26. 文字列の中の field.value() は例外の件数に数えない（本物の呼び出しが無ければ件数の不一致で失敗）
tree="$(fresh_tree str-only 1)"
printf 'fn a() { let note = "field.value()"; }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "field.value() only inside a string does not satisfy the count" 1 "${RC}"
case "${OUT}" in
  *"FAIL exception glue.rs"*"= 0 (allowed 1)"*) expect "string match is reported as count 0" 0 0 ;;
  *) expect "string match is reported as count 0" 0 1 ;;
esac

# 26b. 本物の呼び出し 1 件と文字列内の一致があっても、文字列は数えず水増しされない（件数 1 のまま、文字列は違反側）
tree="$(fresh_tree str-plus 1)"
printf 'fn a() { let v = field.value(); let note = "field.value()"; }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
case "${OUT}" in
  *"= 1 (allowed 1)"*) expect "real call is counted once next to a string match" 0 0 ;;
  *) expect "real call is counted once next to a string match" 0 1 ;;
esac
expect "string match next to the real call is reported" 1 "${RC}"

# 26c. 生文字列・エスケープした引用符を含む文字列・バイト文字列の中の一致も数えない
tree="$(fresh_tree str-raw 1)"
cat >"${tree}/crates/web/src/glue.rs" <<'EOF'
fn a() { let n = r"field.value()"; }
fn b() { let n = r#"x "quoted" field.value()"#; }
fn c() { let n = "say \"hi\" field.value()"; }
fn d() { let n = b"field.value()"; }
fn e() { let n = br#"field.value()"#; }
EOF
run_check "${tree}"
case "${OUT}" in
  *"= 0 (allowed 1)"*) expect "literal matches are not counted" 0 0 ;;
  *) expect "literal matches are not counted" 0 1 ;;
esac
expect "literal-only exception is rejected" 1 "${RC}"

# 26d. エスケープした引用符の直後の本物の呼び出しは数える（文字列の終わりを取り違えない）
tree="$(fresh_tree str-escape 1)"
cat >"${tree}/crates/web/src/glue.rs" <<'EOF'
fn a() { let n = "a \" b \\"; let v = field.value(); }
EOF
run_check "${tree}"
expect "real call after an escaped-quote string is counted" 0 "${RC}"

# 26e. 文字列の直後の .value()（レシーバが文字列）は例外にならない
tree="$(fresh_tree str-recv 1)"
printf 'fn a() { let v = field.value(); let w = "field".value(); }\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "receiver inside a string literal is not an exception" 1 "${RC}"

# 27. 字句の形ごとに、字句の中の `/*`・`//`・`"` が後続の本物の呼び出しを隠さない（Rust Reference の Tokens に沿う）
# 検出は呼び出し元が分かるよう、字句の中に .value() を置かず、受け手 zed の呼び出しで確かめる。
lex_n=0
# lex_detect <label> <source>: 一時ファイルに <source> を書き、zed.value() が違反として報告されることを確かめる。
lex_detect() {
  lex_n=$((lex_n + 1))
  tree="$(fresh_tree "lex-${lex_n}" 0)"
  printf '%s\n' "$2" >"${tree}/crates/web/src/hello.rs"
  run_check "${tree}"
  case "${OUT}" in
    *"zed.value()"*) expect "lexical form does not hide a later call: $1" 1 "${RC}" ;;
    *) expect "lexical form does not hide a later call: $1" 1 0 ;;
  esac
}
Y=' /* // '
lex_detect 'plain string' "fn a() { let s = \"${Y}\"; zed.value(); }"
lex_detect 'byte string' "fn a() { let s = b\"${Y}\"; zed.value(); }"
lex_detect 'C string' "fn a() { let s = c\"${Y}\"; zed.value(); }"
lex_detect 'escaped quote and backslash' "fn a() { let s = \"${Y} \\\" \\\\\"; zed.value(); }"
lex_detect 'escaped quote in byte string' "fn a() { let s = b\"${Y} \\\" \"; zed.value(); }"
lex_detect 'escaped quote in C string' "fn a() { let s = c\"${Y} \\\" \"; zed.value(); }"
lex_detect 'line continuation in string' "$(printf 'fn a() { let s = "x \\\n  /* "; zed.value(); }')"
lex_detect 'raw string' "fn a() { let s = r\"${Y}\"; zed.value(); }"
lex_detect 'raw string with one hash' "fn a() { let s = r#\"${Y} \" \"#; zed.value(); }"
lex_detect 'raw string with two hashes' "fn a() { let s = r##\"${Y} \"# \"##; zed.value(); }"
lex_detect 'raw byte string' "fn a() { let s = br\"${Y}\"; zed.value(); }"
lex_detect 'raw byte string with hash' "fn a() { let s = br#\"${Y} \" \"#; zed.value(); }"
lex_detect 'raw byte string with two hashes' "fn a() { let s = br##\"${Y} \"# \"##; zed.value(); }"
lex_detect 'raw C string' "fn a() { let s = cr\"${Y}\"; zed.value(); }"
lex_detect 'raw C string with hash' "fn a() { let s = cr#\"${Y} \" \"#; zed.value(); }"
lex_detect 'raw C string with two hashes' "fn a() { let s = cr##\"${Y} \"# \"##; zed.value(); }"
lex_detect 'raw C string (review example)' 'let s = cr#" " /* "#; zed.value();'
lex_detect 'char slash' "fn a() { let c = '/'; let d = '/'; zed.value(); }"
lex_detect 'char double quote' "fn a() { let c = '\"'; zed.value(); }"
lex_detect 'char single quote escape' "fn a() { let c = '\\''; zed.value(); }"
lex_detect 'char backslash escape' "fn a() { let c = '\\\\'; let s = \"/*\"; zed.value(); }"
lex_detect 'char unicode escape' "fn a() { let c = '\\u{1F600}'; let s = \"/*\"; zed.value(); }"
lex_detect 'char multibyte' "fn a() { let c = 'あ'; let s = \"/*\"; zed.value(); }"
lex_detect 'byte char' "fn a() { let c = b'/'; let s = \"/*\"; zed.value(); }"
lex_detect 'byte double quote' "fn a() { let c = b'\"'; zed.value(); }"
lex_detect 'byte single quote escape' "fn a() { let c = b'\\''; let s = \"/*\"; zed.value(); }"
lex_detect 'lifetime then string' "fn a<'a>(x: &'a str) { let s = \"/*\"; zed.value(); }"
lex_detect 'static lifetime then string' "fn a(x: &'static str) { let s = \"//\"; zed.value(); }"
lex_detect 'label then string' "fn a() { 'outer: loop { let s = \"/*\"; break 'outer; } zed.value(); }"
lex_detect 'char literal that looks like a lifetime' "fn a() { let c = 'a'; let s = \"/*\"; zed.value(); }"
lex_detect 'doc comment with a quote' "$(printf '/// say "hi\nfn a() { zed.value(); }')"
lex_detect 'inner doc comment with a block opener' "$(printf '//! /* open\nfn a() { zed.value(); }')"
lex_detect 'outer block doc comment with a quote' '/** " */ fn a() { zed.value(); }'
lex_detect 'inner block doc comment with a quote' "/*! ' */ fn a() { zed.value(); }"
lex_detect 'empty block comment' 'fn a() { /**/ zed.value(); }'
lex_detect 'nested block comment' 'fn a() { /* /* " */ " */ zed.value(); }'
lex_detect 'raw identifier then string' "fn a() { let r#type = \"/*\"; zed.value(); }"
lex_detect 'r b c as plain names' "fn a() { foo(r, b, c); let s = \"/*\"; zed.value(); }"
lex_detect 'names ending in r b c before a space' "fn a() { let abc = \"/*\"; let bar = \"//\"; let cr = 1; zed.value(); }"
lex_detect 'r as a name before a paren' "fn a() { let s = f(r) + \"/*\".len(); zed.value(); }"
lex_detect 'number suffix then string' "fn a() { let s = 1u8; let t = \"/*\"; zed.value(); }"

# 27b. 字句の中の field.value() は例外の件数を水増しせず（件数 1 のまま）、字句の外の本物の呼び出しは数える
X=' /* // field.value() '
lex_n=0
for lit in "\"${X}\"" "b\"${X}\"" "c\"${X}\"" "r\"${X}\"" "r#\"${X} \" \"#" "r##\"${X} \"# \"##" "br\"${X}\"" \
  "br#\"${X} \" \"#" "cr\"${X}\"" "cr#\"${X} \" \"#" "cr##\"${X} \"# \"##" "\"${X} \\\" \\\\\"" "c\"${X} \\\" \""; do
  lex_n=$((lex_n + 1))
  tree="$(fresh_tree "lexcnt-${lex_n}" 1)"
  printf 'fn a() { let n = %s; let v = field.value(); }\n' "${lit}" >"${tree}/crates/web/src/glue.rs"
  run_check "${tree}"
  case "${OUT}" in
    *"= 1 (allowed 1)"*) expect "literal does not inflate the exception count: ${lit}" 0 0 ;;
    *) expect "literal does not inflate the exception count: ${lit}" 0 1 ;;
  esac
done

# 27c. コメントだけの呼び出しは数えない（入れ子・doc コメント）
tree="$(fresh_tree lex-comments-only 0)"
cat >"${tree}/crates/web/src/hello.rs" <<'EOF'
/// doc el.value()
//! inner doc el.value()
/** outer block doc el.value() */
/*! inner block doc el.value() */
fn a() { /* /* el.value() */ el.value() */ let s = 'a'; }
EOF
run_check "${tree}"
expect "calls only inside comments are ignored" 0 "${RC}"

# 28. 字句を判定できない入力は、成功にも通常の違反にもせず検査の失敗（終了コード 2）にする（fail-closed）
lex_bad() {
  lex_n=$((lex_n + 1))
  tree="$(fresh_tree "lexbad-${lex_n}" 0)"
  printf '%s\n' "$2" >"${tree}/crates/web/src/hello.rs"
  run_check "${tree}"
  expect "undecidable source fails closed: $1" 2 "${RC}"
}
lex_bad 'unterminated string' 'fn a() { let s = "abc; }'
lex_bad 'unterminated byte string' 'fn a() { let s = b"abc; }'
lex_bad 'unterminated C string' 'fn a() { let s = c"abc; }'
lex_bad 'string ending with an escape' "fn a() { let s = \"abc\\"
lex_bad 'unterminated block comment' 'fn a() { /* open'
lex_bad 'unterminated nested block comment' 'fn a() { /* /* inner */ still open'
lex_bad 'unterminated doc block comment' '/** open'
lex_bad 'slash star slash is not closed' 'fn a() { /*/ }'
lex_bad 'unterminated raw string' 'fn a() { let s = r#"abc"; }'
lex_bad 'raw string closed with too few hashes' 'fn a() { let s = r##"abc"#; }'
lex_bad 'unterminated raw byte string' 'fn a() { let s = br#"abc"; }'
lex_bad 'unterminated raw C string' 'fn a() { let s = cr"abc; }'
lex_bad 'raw prefix without a quote' 'fn a() { let s = r#; }'
lex_bad 'raw byte prefix without a quote' 'fn a() { let s = br#x; }'
lex_bad 'raw identifier with two hashes' 'fn a() { let s = r##x; }'
lex_bad 'unknown prefix foo' 'fn a() { let s = foo"x"; }'
lex_bad 'unknown prefix bar' 'fn a() { let s = bar"x"; }'
lex_bad 'unknown prefix abc' 'fn a() { let s = abc"x"; }'
lex_bad 'unknown prefix rb' 'fn a() { let s = rb"x"; }'
lex_bad 'unterminated char' "fn a() { let c = '"
lex_bad 'unterminated escaped char' "fn a() { let c = '\\u{1F600; }"
lex_bad 'empty char' "fn a() { let c = ''; }"
lex_bad 'multi-character byte literal' "fn a() { let c = b'ab'; }"
lex_bad 'quote followed by a digit' "fn a() { let c = '1x; }"
lex_bad 'quote followed by a space' "fn a() { let c = ' x; }"
# 判定できないファイルが 1 つでもあれば、例外の件数が合っていても成功にしない
tree="$(fresh_tree lexbad-other-file 1)"
printf 'fn a() { let v = field.value(); }\n' >"${tree}/crates/web/src/glue.rs"
printf 'fn h() { let s = "open; }\n' >"${tree}/crates/web/src/hello.rs"
run_check "${tree}"
expect "undecidable file next to a satisfied exception fails closed" 2 "${RC}"
tree="$(fresh_tree lexbad-glue 1)"
printf 'fn a() { let v = field.value(); /* open\n' >"${tree}/crates/web/src/glue.rs"
run_check "${tree}"
expect "undecidable glue.rs fails closed" 2 "${RC}"
if grep -q 'cannot lex Rust source' "${TMP_DIR}/stderr.txt" && grep -q 'glue.rs:1' "${TMP_DIR}/stderr.txt"; then
  expect "undecidable source is reported with file and line" 0 0
else
  expect "undecidable source is reported with file and line" 0 1
fi

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
