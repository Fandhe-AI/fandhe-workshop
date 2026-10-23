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
# EVT-1・EVT-23 の各規則の検査の所在（TASK-206。この対応表以外に同じ規則を定義しない）:
#   - DOM の文字列の読み取りが 0 件で例外が 1 件: 本スクリプトと crates/web/tests/state_paths.rs
#   - 状態を変える経路は Session の操作だけ: crates/web/tests/no_progress.rs
#   - 右の描画が左の状態を読まない・左が右の文字列を読まない: crates/web/tests/pane_isolation.rs
#   - core の依存の閉包に表示基盤が無い: scripts/check-crate-deps.sh
# 入力イベントの欄（KeyboardEvent::key・InputEvent::data など）と保存先の get_item は表示の文字列ではないため対象外である。
#
# 例外は 1 種類だけ（下の EXC_*）。glue.rs 直下の `field.value()` で、ファイル・パターン・件数の 3 つで限定する。
# 件数は増えても減っても失敗する。固定値 1 は glue.rs の sync_draft にある textarea の現在値の読み取り（TASK-158）。
# 件数を変えるときは、例外の理由の説明も実装に合わせて更新する。
#
# 照合の単位: 呼び出し 1 件ずつ（同じ行に複数あれば複数と数える）。空白・改行を挟んだ `.value ( )` も検出する。
# getter だけを対象にし、`set_text_content` などの setter は名前の境界で区別して対象外にする。
# 呼び出し名の前の raw identifier（`el.r#value()`・`::r#children(`）は、通常の名前と同じ呼び出しとして検出する。
#
# 例外の計数: 実際のコードの呼び出しだけを数える。一致の全体（レシーバ `field` を含む）が文字列・生文字列・文字リテラルに
# かかるものは数えない（`let note = "field.value()";` で件数を水増しできない）。`field.r#value()` も保守的に数えない
# （例外外の違反として報告し、件数も満たさない）。
#
# 字句: 照合の前に Rust の字句を読み、コメントを空白にし、字句の中身を「コードでない」と記録する。扱う形は次のとおり。
#   - コメント: `//`（`///`・`//!` を含む）、`/* */`（`/** */`・`/*! */` を含み、入れ子を数える）。
#     `el.value(/* x */)` や `el./* r */value()` も検出し、コメント内の一致は数えない。
#   - 文字列: `"..."`・`b"..."`・`c"..."`（`\"`・`\\`・行末の `\` による継続を含むエスケープ）。
#   - 生文字列: `r"..."`・`r#"..."#`（`#` は任意の個数）・`br`・`cr` の各形。
#   - 文字・バイト: `'x'`・`'\''`・`'\u{1F600}'`・`b'x'`・`b'\''`・ASCII 以外の 1 文字。
#     ライフタイム・ラベル（`'a`・`'static`・`'outer:`）は、閉じ引用符が続くかどうかで文字リテラルと区別する。
#   - raw identifier `r#ident` はコードとして読む。識別子は英数字と `_` の連なりを 1 語として読み、語の末尾の `r`・`b`・`c` を
#     接頭辞と取り違えない。
# 字句の中の `//`・`/*`・`"` はコメントや文字列の終端にしない。文字列・生文字列・文字リテラルの中の一致は検出側に倒れる
# （誤検出はあり得るが、見逃しにはならない）。例外の対象外にもなるため、glue.rs の文字列に `field.value()` を書くと
# 違反として報告される。
#
# 字句を判定できない形（閉じていない文字列・生文字列・ブロックコメント・文字リテラル、未知の接頭辞 `foo"`、`r#` や `br#` の
# 後に引用符も識別子も続かない形、引用符の後ろが文字リテラルにもライフタイムにもならない形）が 1 つでもあれば、
# ファイル名と行番号を出して終了コード 2 で止める（fail-closed）。コンパイルできないコードを読み飛ばして合格にしない。
# 残る限界: ASCII 以外の識別子・ライフタイム（`'é`）は判定できず、2 になる。マクロ内のトークン列も Rust の字句として読む。
# 2015 / 2018 edition でだけ有効な `return"x"` のような接頭辞なしの連結も 2 になる（2021 以降では予約された接頭辞のため）。
#
# 使い方: tools/check-deps.sh [--root <dir>]（--root は回帰テスト用。既定はこのファイルの親の親）
# 終了コード: 0 = 違反なし / 1 = 違反あり / 2 = 誤用、または検査を完了できない（対象が無い・awk の失敗・字句を判定できない）。
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
EXC_N=1

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
    # Rust の字句を読み、コメントを空白に置き換える（改行は残して行番号を保つ）。MASK には、各文字が実際のコード
    # （c）か、文字列・生文字列・バイト列・C 文字列・文字リテラルの中（s）かを 1 文字ずつ入れる。
    # 字句の形が判定できないとき（閉じていない文字列・コメント、未知の接頭辞など）は fail で検査を止める（fail-closed）。
    function strip(s,   out, msk, i, n, c, d, j, k, e, h, depth, p, w, nx) {
      n = length(s); out = ""; msk = ""; i = 1
      while (i <= n) {
        c = substr(s, i, 1); d = substr(s, i + 1, 1)
        if (c == "/" && d == "/") {
          j = index(substr(s, i), "\n")
          out = out " "; msk = msk "c"
          if (j == 0) i = n + 1; else i = i + j - 1
          continue
        }
        if (c == "/" && d == "*") {
          depth = 1; j = i + 2; out = out " "; msk = msk "c"
          while (j <= n && depth > 0) {
            p = substr(s, j, 2)
            if (p == "/*") { depth++; j += 2 }
            else if (p == "*/") { depth--; j += 2 }
            else { if (substr(s, j, 1) == "\n") { out = out "\n"; msk = msk "c" }; j++ }
          }
          if (depth > 0) fail(s, i, "unterminated block comment")
          i = j
          continue
        }
        if (c ~ /[A-Za-z0-9_]/) {
          k = i
          while (k <= n && substr(s, k, 1) ~ /[A-Za-z0-9_]/) k++
          w = substr(s, i, k - i); nx = substr(s, k, 1)
          e = 0
          if ((w == "r" || w == "br" || w == "cr") && (nx == "\"" || nx == "#")) {
            h = 0; e = k
            while (substr(s, e, 1) == "#") { e++; h++ }
            if (substr(s, e, 1) == "\"") {
              e = rawlit(s, e, h)
              if (e == 0) fail(s, i, "unterminated raw string")
            } else if (w == "r" && h == 1 && substr(s, e, 1) ~ /[A-Za-z_]/) {
              # raw identifier（r#ident）はコードとして読む
              while (e <= n && substr(s, e, 1) ~ /[A-Za-z0-9_]/) e++
              out = out substr(s, i, e - i); msk = msk rep("c", e - i); i = e
              continue
            } else fail(s, i, "malformed raw string or raw identifier prefix")
          } else if (nx == "\"") {
            if (w != "b" && w != "c") fail(s, i, "unknown string prefix " w)
            e = strlit(s, k)
            if (e == 0) fail(s, i, "unterminated string")
          } else if (w == "b" && nx == Q) {
            e = charlit(s, k)
            if (e == 0) fail(s, i, "malformed byte literal")
          }
          if (e > 0) {
            out = out substr(s, i, e - i); msk = msk rep("s", e - i); i = e
          } else {
            out = out w; msk = msk rep("c", k - i); i = k
          }
          continue
        }
        if (c == "\"") {
          e = strlit(s, i)
          if (e == 0) fail(s, i, "unterminated string")
          out = out substr(s, i, e - i); msk = msk rep("s", e - i); i = e
          continue
        }
        if (c == Q) {
          e = charlit(s, i)
          if (e > 0) {
            out = out substr(s, i, e - i); msk = msk rep("s", e - i); i = e
            continue
          }
          # 文字リテラルでなければライフタイム・ラベル（`a・`static・`outer:）
          if (d !~ /[A-Za-z_]/) fail(s, i, "malformed character literal or lifetime")
          k = i + 1
          while (k <= n && substr(s, k, 1) ~ /[A-Za-z0-9_]/) k++
          out = out substr(s, i, k - i); msk = msk rep("c", k - i); i = k
          continue
        }
        out = out c; msk = msk "c"; i++
      }
      MASK = msk
      return out
    }
    # strlit(s, q): q の位置の引用符で始まる通常の文字列の、閉じ引用符の次の位置。閉じていなければ 0。
    function strlit(s, q,   j, n, p) {
      n = length(s); j = q + 1
      while (j <= n) {
        p = substr(s, j, 1)
        if (p == "\\") { j += 2; continue }
        if (p == "\"") return j + 1
        j++
      }
      return 0
    }
    # rawlit(s, q, h): q の位置の引用符で始まる生文字列（# が h 個）の、終端の次の位置。閉じていなければ 0。
    function rawlit(s, q, h,   j, n, hs, x) {
      n = length(s); hs = rep("#", h); j = q + 1
      while (j <= n) {
        if (substr(s, j, 1) == "\"" && substr(s, j + 1, h) == hs) return j + 1 + h
        j++
      }
      return 0
    }
    # charlit(s, q): q の位置の単引用符で始まる文字・バイトリテラルの、閉じ引用符の次の位置。リテラルでなければ 0。
    function charlit(s, q,   d, j, n, k, ch) {
      n = length(s); d = substr(s, q + 1, 1)
      if (d == "\\") {
        j = q + 3
        while (j <= n && substr(s, j, 1) != Q && substr(s, j, 1) != "\n") j++
        if (substr(s, j, 1) == Q) return j + 1
        return 0
      }
      if (d == "" || d == Q || d == "\n") return 0
      if (substr(s, q + 2, 1) == Q) return q + 3
      if (d !~ /[ -~]/) {
        # ASCII 以外の 1 文字（バイト単位の awk では最大 4 バイト）
        for (k = q + 2; k <= q + 5; k++) {
          ch = substr(s, k, 1)
          if (ch == Q) return k + 1
          if (ch == "" || ch == "\n" || ch ~ /[ -~]/) return 0
        }
      }
      return 0
    }
    # fail(s, pos, msg): 字句を判定できないとき、位置を示して終了コード 2 で止める（END の flush は failed で飛ばす）。
    function fail(s, pos, msg,   x) {
      x = substr(s, 1, pos - 1); gsub(/[^\n]/, "", x)
      printf "error: %s:%d: cannot lex Rust source (%s); refusing to pass\n", name, length(x) + 1, msg > "/dev/stderr"
      failed = 1
      exit 2
    }
    # rep(c, n): 文字 c を n 個並べた文字列。
    function rep(c, n,   r) {
      r = ""
      while (n-- > 0) r = r c
      return r
    }
    function flush(   t, m, rest, off, n, mlen, pre, ps, from, ln, snip, excepted, emit) {
      if (name == "") return
      t = strip(text); m = MASK; rest = t; off = 0
      while (match(rest, pat)) {
        n = off + RSTART; mlen = RLENGTH
        pre = substr(t, (n > 200 ? n - 200 : 1), (n > 200 ? 200 : n - 1))
        # 例外は、レシーバ `field` から呼び出しの末尾までがすべて実際のコード（文字列・文字リテラルの外）で、
        # 呼び出し名が raw identifier でないときだけ成り立つ。判定が曖昧なら例外にしない（違反または件数の不一致になる）。
        excepted = 0
        if (mode != "" && name == xf && pre ~ xp && substr(t, n, mlen) !~ /r#/) {
          ps = (n > 200 ? n - 200 : 1)
          match(pre, xp)
          from = ps + RSTART - 1
          if (index(substr(m, from, n + mlen - from), "s") == 0) excepted = 1
        }
        emit = (mode == "" || (mode == "skip" && !excepted) || (mode == "only" && excepted))
        if (emit) {
          ln = split(substr(t, 1, n - 1), parts, "\n")
          if (substr(t, n, 1) == "\n") ln++
          snip = substr(t, 1, n - 1)
          sub(/^.*\n/, "", snip)
          snip = snip substr(t, n, mlen + 20)
          gsub(/\n/, " ", snip)
          printf "%s:%d:%s\n", name, ln, snip
        }
        off = n + mlen - 1
        rest = substr(t, off + 1)
      }
      text = ""
    }
    BEGIN { Q = "\047"; failed = 0; pat = ENVIRON["SCAN_PAT"]; mode = ENVIRON["SCAN_MODE"]; xf = ENVIRON["SCAN_XFILE"]; xp = ENVIRON["SCAN_XPAT"] }
    FNR == 1 { flush(); name = FILENAME }
    { text = text $0 "\n" }
    END { if (!failed) flush() }
  ' "${FILES[@]}"
}

# 呼び出しの検出パターン。直前・直後が識別子の文字でないことを要求し、set_inner_html などの setter を拾わない。
# raw identifier の `r#` は識別子の文字ではないので、名前の境界の照合（B）がそのまま通す。value と children の
# `.`・`::` 直後の形だけ、`(r#)?` を明示する。
WS='[[:space:]]*'
VALUE_CALL="\\.${WS}(r#)?value${WS}\\(${WS}\\)"
# 関連関数形式（`web_sys::HtmlInputElement::value(&el)` など）。レシーバが引数なので例外の対象にしない。
ASSOC_VALUE_CALL="::${WS}(r#)?value${WS}\\("
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
    'children(assoc)') rx="::${WS}(r#)?children${WS}\\(" ;;
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
