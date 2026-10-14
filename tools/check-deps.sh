#!/usr/bin/env bash
# web crate のソースが DOM を読む API を呼ばないことの静的検査（BUILD-6・F-9。`make check-dom-reads` と CI の rust-lint から呼ばれる）。
#
# 目的: 描画の結果（DOM の文字列・値・属性）を読んで状態や判定に流す経路を、web crate に作らせない。
# 入力欄の識別は属性の文字列ではなく、要素の同一性（draft_field() で取った要素との JS の ===）で行う。
# そのため get_attribute は 0 件とし、get_attribute("data-testid") による識別を持ち込ませない。
#
# 注意: これは DOM 読み取りの検査である。crate 間の依存の向き（core が web-sys 等に届かないこと）は
# scripts/check-crate-deps.sh（`make check-deps`）が持つ。同じ不変条件を二重に定義しない。
#
# 例外は 1 種類だけ（下の EXC_*）。glue.rs 直下の `field.value()` で、ファイル・パターン・件数の 3 つで限定する。
# 件数は増えても減っても失敗する。固定値は現在のコードの実数であり、仕様上の最終値 1 ではない:
#   textarea の現在値と状態の下書きを比べる sync_draft_to_dom を足す TASK-356（#964）が、同じ変更で 1 に上げる。
#   上げるときは例外の理由の説明も実装に合わせて更新する。
#
# 照合の単位: 呼び出し 1 件ずつ（同じ行に複数あれば複数と数える）。空白・改行を挟んだ `.value ( )` も検出する。
# getter だけを対象にし、`set_text_content` などの setter は名前の境界で区別して対象外にする。
#
# コメント: `//`（`///`・`//!` を含む）と `/* */`（入れ子を数える）を空白として扱ってから照合する。そのため
# `el.value(/* x */)` や `el./* r */value()` も検出し、コメント内の一致は数えない。文字列・生文字列・文字リテラルの
# 中の `//`・`/*` はコメントにしない。文字列リテラル内の一致は検出側に倒れる（誤検出はあり得るが、見逃しにはならない）。
#
# 使い方: tools/check-deps.sh [--root <dir>]（--root は回帰テスト用。既定はこのファイルの親の親）
# 終了コード: 0 = 違反なし / 1 = 違反あり / 2 = 誤用、または検査を完了できない（対象が無い・awk の失敗）。
#   2 でも合格にしない（fail-closed）。回帰テストは scripts/test-check-dom-reads.sh。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../scripts/lib.sh
source "${DIR}/../scripts/lib.sh"

ROOT="${DIR}/.."
if [ "$#" -eq 2 ] && [ "$1" = "--root" ]; then
  ROOT="$2"
elif [ "$#" -ne 0 ]; then
  echo "usage: $(basename "$0") [--root <dir>]" >&2
  exit 2
fi

require_cmd awk || exit 2
require_cmd find || exit 2
require_cmd mktemp || exit 2
require_cmd rm || exit 2
require_cmd wc || exit 2
require_cmd tr || exit 2
require_cmd sed || exit 2

# 例外: crates/web/src 直下の glue.rs にある `field.value()`（F-9）。サブディレクトリの同名ファイルは例外にしない。
# 理由: 上流の Runtime の束縛は HtmlInputElement の value だけを書き、textarea の現在値を同期しない。
# そのため textarea の現在値を状態の下書きと比べ、食い違うときだけ書き戻す（毎回書くとカーソルが動く）。
# 読んだ値は状態に入れない。
EXC_FILE="glue.rs"
EXC_PATTERN='field\.value\(\)'
EXC_N=0

WEB_SRC="${ROOT}/crates/web/src"
if [ ! -d "${WEB_SRC}" ]; then
  echo "error: web crate source directory not found: ${WEB_SRC}" >&2
  exit 2
fi
GLUE="${WEB_SRC}/${EXC_FILE}"
if [ ! -f "${GLUE}" ]; then
  echo "error: exception file not found: ${GLUE}" >&2
  exit 2
fi

# find の出力を一時ファイルへ受け、終了状態を検査してから読む。プロセス置換の中の失敗は errexit に伝わらず、
# 一部しか列挙できないまま検査が成功する（fail-open）ため。
LIST_FILE="$(mktemp)" || exit 2
trap 'rm -f "${LIST_FILE}"' EXIT
if ! find "${WEB_SRC}" -type f -name '*.rs' -print0 >"${LIST_FILE}"; then
  echo "error: find failed under ${WEB_SRC}" >&2
  exit 2
fi
FILES=()
while IFS= read -r -d '' f; do
  FILES+=("${f}")
done <"${LIST_FILE}"
if [ "${#FILES[@]}" -eq 0 ]; then
  echo "error: no .rs files under ${WEB_SRC}" >&2
  exit 2
fi

# scan <pattern> [<mode> <exclude-file> <prefix-pattern>]: コメント行を除いた各ファイルを 1 本の文字列にして、
# パターンの一致を 1 件ずつ `パス:行番号:一致の周辺` で出す（呼び出し単位。空白・改行をまたぐ呼び出しも検出する）。
# mode が `skip` のとき、exclude-file と同じパスで、一致の直前が prefix-pattern に合う一致を出さない（例外の除外）。
# mode が `only` のときは逆に、その一致だけを出す（例外の件数）。mode が空なら全件を出す。
# パターンは環境変数で渡し、awk の -v によるエスケープ解釈を避ける。awk の失敗は errexit で検査全体を止める。
scan() {
  SCAN_PAT="$1" SCAN_MODE="${2:-}" SCAN_XFILE="${3:-}" SCAN_XPAT="${4:-}" awk '
    # Rust のコメントを空白に置き換える（改行は残して行番号を保つ）。文字列・生文字列・文字リテラルの中の `//` や `/*` は
    # コメントとして扱わない（扱うと行の残りが隠れて見逃す）。ブロックコメントは入れ子を数える。
    function strip(s,   out, i, n, c, d, j, k, h, hs, depth, p) {
      n = length(s); out = ""; i = 1
      while (i <= n) {
        c = substr(s, i, 1); d = substr(s, i + 1, 1)
        if (c == "/" && d == "/") {
          j = index(substr(s, i), "\n")
          out = out " "
          if (j == 0) i = n + 1; else i = i + j - 1
          continue
        }
        if (c == "/" && d == "*") {
          depth = 1; i += 2; out = out " "
          while (i <= n && depth > 0) {
            p = substr(s, i, 2)
            if (p == "/*") { depth++; i += 2 }
            else if (p == "*/") { depth--; i += 2 }
            else { if (substr(s, i, 1) == "\n") out = out "\n"; i++ }
          }
          continue
        }
        if (c == "r") {
          p = (i > 1 ? substr(s, i - 1, 1) : "")
          if (p !~ /[A-Za-z0-9_]/ || p == "b") {
            k = i + 1
            while (substr(s, k, 1) == "#") k++
            if (substr(s, k, 1) == "\"") {
              h = k - i - 1; hs = ""
              for (j = 0; j < h; j++) hs = hs "#"
              j = k + 1
              while (j <= n) {
                if (substr(s, j, 1) == "\"" && substr(s, j + 1, h) == hs) { j += 1 + h; break }
                j++
              }
              out = out substr(s, i, j - i); i = j
              continue
            }
          }
        }
        if (c == "\"") {
          j = i + 1
          while (j <= n) {
            p = substr(s, j, 1)
            if (p == "\\") { j += 2; continue }
            if (p == "\"") { j++; break }
            j++
          }
          out = out substr(s, i, j - i); i = j
          continue
        }
        if (c == "\047") {
          if (d == "\\") {
            j = i + 3
            while (j <= n && substr(s, j, 1) != "\047") j++
            j++
            out = out substr(s, i, j - i); i = j
            continue
          }
          if (substr(s, i + 2, 1) == "\047") { out = out substr(s, i, 3); i += 3; continue }
        }
        out = out c; i++
      }
      return out
    }
    function flush(   t, rest, off, n, pre, ln, snip, excepted, emit) {
      if (name == "") return
      t = strip(text); rest = t; off = 0
      while (match(rest, pat)) {
        n = off + RSTART
        pre = substr(t, (n > 200 ? n - 200 : 1), (n > 200 ? 200 : n - 1))
        excepted = (mode != "" && name == xf && pre ~ xp)
        emit = (mode == "" || (mode == "skip" && !excepted) || (mode == "only" && excepted))
        if (emit) {
          ln = split(substr(t, 1, n - 1), parts, "\n")
          if (substr(t, n, 1) == "\n") ln++
          snip = substr(t, 1, n - 1)
          sub(/^.*\n/, "", snip)
          snip = snip substr(t, n, RLENGTH + 20)
          gsub(/\n/, " ", snip)
          printf "%s:%d:%s\n", name, ln, snip
        }
        off = n + RLENGTH - 1
        rest = substr(t, off + 1)
      }
      text = ""
    }
    BEGIN { pat = ENVIRON["SCAN_PAT"]; mode = ENVIRON["SCAN_MODE"]; xf = ENVIRON["SCAN_XFILE"]; xp = ENVIRON["SCAN_XPAT"] }
    FNR == 1 { flush(); name = FILENAME }
    { text = text $0 "\n" }
    END { flush() }
  ' "${FILES[@]}"
}

# 呼び出しの検出パターン。直前・直後が識別子の文字でないことを要求し、set_inner_html などの setter を拾わない。
WS='[[:space:]]*'
VALUE_CALL="\\.${WS}value${WS}\\(${WS}\\)"
# 関連関数形式（`web_sys::HtmlInputElement::value(&el)` など）。レシーバが引数なので例外の対象にしない。
ASSOC_VALUE_CALL="::${WS}value${WS}\\("
# 例外の呼び出しの直前（レシーバが `field` であること）
EXC_PREFIX='(^|[^A-Za-z0-9_])field[[:space:]]*$'

violations=0

echo "DOM reads in crates/web/src ($(basename "$0"))"

# 1. 例外の件数（増えても減っても失敗）
exc_lines="$(scan "${VALUE_CALL}" only "${GLUE}" "${EXC_PREFIX}")"
exc_hits=0
if [ -n "${exc_lines}" ]; then
  exc_hits="$(printf '%s\n' "${exc_lines}" | wc -l | tr -d '[:space:]')"
fi
if [ "${exc_hits}" = "${EXC_N}" ]; then
  echo "  ok   exception ${EXC_FILE}: ${EXC_PATTERN} = ${exc_hits} (allowed ${EXC_N})"
else
  echo "  FAIL exception ${EXC_FILE}: ${EXC_PATTERN} = ${exc_hits} (allowed ${EXC_N})"
  violations=$((violations + 1))
fi

# 2. 例外以外の .value()
other="$(scan "${VALUE_CALL}" skip "${GLUE}" "${EXC_PREFIX}")"
if [ -z "${other}" ]; then
  echo "  ok   .value() outside the exception = 0"
else
  echo "  FAIL .value() outside the exception:"
  printf '%s\n' "${other}" | sed 's/^/    /'
  violations=$((violations + 1))
fi

# 2b. 関連関数形式の value 読み取り（例外なし）
assoc="$(scan "${ASSOC_VALUE_CALL}")"
if [ -z "${assoc}" ]; then
  echo "  ok   Type::value(&el) form = 0"
else
  echo "  FAIL Type::value(&el) form:"
  printf '%s\n' "${assoc}" | sed 's/^/    /'
  violations=$((violations + 1))
fi

# 3. その他の DOM を読む API（例外なし）
# 名前の境界で照合する（`set_text_content`・`set_inner_html` などの setter は別の名前なので対象外）。
B='(^|[^A-Za-z0-9_])'
E='($|[^A-Za-z0-9_])'
for pat in 'text_content' 'inner_html' 'inner_text' 'outer_html' 'get_attribute' 'get_element_by_id' \
  'get_elements_by[A-Za-z0-9_]*' 'first_child' 'children()' 'children(assoc)' 'dataset' 'closest(' 'get_computed_style' \
  'get_bounding_client_rect'; do
  case "${pat}" in
    'children()') rx="${B}children${WS}\\(${WS}\\)" ;;
    'children(assoc)') rx="::${WS}children${WS}\\(" ;;
    'closest(') rx="${B}closest${WS}\\(" ;;
    *) rx="${B}${pat}${E}" ;;
  esac
  hits="$(scan "${rx}")"
  if [ -z "${hits}" ]; then
    echo "  ok   ${pat} = 0"
  else
    echo "  FAIL ${pat}:"
    printf '%s\n' "${hits}" | sed 's/^/    /'
    violations=$((violations + 1))
  fi
done

if [ "${violations}" -eq 0 ]; then
  echo "RESULT: ok"
else
  echo "RESULT: violations found (${violations})"
  exit 1
fi
