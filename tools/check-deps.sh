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
# 検出の限界: 除くのは先頭が `//` の行（`///`・`//!` を含む）だけ。ブロックコメントや行末コメント内の一致は
# 検出側に倒れる（誤検出はあり得るが、見逃しにはならない）。
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

FILES=()
while IFS= read -r -d '' f; do
  FILES+=("${f}")
done < <(find "${WEB_SRC}" -type f -name '*.rs' -print0)
if [ "${#FILES[@]}" -eq 0 ]; then
  echo "error: no .rs files under ${WEB_SRC}" >&2
  exit 2
fi

# scan <pattern> [<exclude-file> <exclude-pattern>]: コメント行を除く一致を `パス:行番号:本文` で出す。
# exclude-file と同じパスの行のうち exclude-pattern に一致するものは出さない（例外の除外）。
# パターンは環境変数で渡し、awk の -v によるエスケープ解釈を避ける。awk の失敗は errexit で検査全体を止める。
scan() {
  SCAN_PAT="$1" SCAN_XFILE="${2:-}" SCAN_XPAT="${3:-}" awk '
    BEGIN { pat = ENVIRON["SCAN_PAT"]; xf = ENVIRON["SCAN_XFILE"]; xp = ENVIRON["SCAN_XPAT"] }
    /^[[:space:]]*\/\// { next }
    $0 ~ pat {
      if (xf != "" && FILENAME == xf && $0 ~ xp) next
      printf "%s:%d:%s\n", FILENAME, FNR, $0
    }
  ' "${FILES[@]}"
}

violations=0

echo "DOM reads in crates/web/src ($(basename "$0"))"

# 1. 例外の件数（増えても減っても失敗）
exc_lines="$(scan "${EXC_PATTERN}")"
exc_hits=0
if [ -n "${exc_lines}" ]; then
  exc_hits="$(printf '%s\n' "${exc_lines}" | awk -v f="${GLUE}:" 'index($0, f) == 1' | wc -l | tr -d '[:space:]')"
fi
if [ "${exc_hits}" = "${EXC_N}" ]; then
  echo "  ok   exception ${EXC_FILE}: ${EXC_PATTERN} = ${exc_hits} (allowed ${EXC_N})"
else
  echo "  FAIL exception ${EXC_FILE}: ${EXC_PATTERN} = ${exc_hits} (allowed ${EXC_N})"
  violations=$((violations + 1))
fi

# 2. 例外以外の .value()
other="$(scan '\.value\(\)' "${GLUE}" "${EXC_PATTERN}")"
if [ -z "${other}" ]; then
  echo "  ok   .value() outside the exception = 0"
else
  echo "  FAIL .value() outside the exception:"
  printf '%s\n' "${other}" | sed 's/^/    /'
  violations=$((violations + 1))
fi

# 3. その他の DOM を読む API（例外なし）
for pat in 'text_content' 'inner_html' 'inner_text' 'outer_html' 'get_attribute' 'get_element_by_id' \
  'get_elements_by' 'first_child' 'children\(\)' 'dataset' 'closest\(' 'get_computed_style' \
  'get_bounding_client_rect'; do
  hits="$(scan "${pat}")"
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
