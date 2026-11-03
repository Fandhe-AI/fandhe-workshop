#!/usr/bin/env bash
# 3 本目の教材（template-workshop/examples/task-breakdown）の教材テストの検査（`make check-third-pack`。
# CI の rust-test が流す。TASK-797。D-98。BUILD-9）。
#
# 目的: 3 本目の教材も BUILD-9 の教材テストの対象に数える（D-98）。参照切れ 0・到達できない Step 0・行き止まり 0 は
# validate の診断 0 件が、模範解答での完走は test が覆う。どちらかが外れたら CI を失敗させる。手元の記録
# （docs/testing.md 5.458・5.459）だけでは教材を直したときの崩れに気づけないため、毎回流す。
# 件数（Step 数・誤入力の数）は固定しない。教材を直すたびに書き直しが要るのを避ける（第 2 教材の検査と同じ）。
# Rust・Cargo.* には触れない（エンジンの差分 0 の記録を崩さない）。
#
# 項目（1 項目 1 行。PASS <名前> / FAIL <名前>: <理由>。教材の本文・IR は出さない）
#   validate    終了コード 0・標準エラー空・出力 1 行で「OK: 診断 0 件（」かつ「章 1、」
#   test        終了コード 0・FAIL の行なし・模範解答 n/n（n>=1）完走・誤入力 m/m が期待どおり
#   zero-counts R-22「確認なしで達成」と R-21「次の行動が空」が 0（行が無ければ不合格）
#
# 使い方: scripts/check-third-pack.sh [--pack <dir>]
#   --pack  省略時は template-workshop/examples/task-breakdown（回帰テストが壊した写しを渡すための口）
# 環境変数: FANDHE_PACK_BIN（無ければ scripts/build-pack-bin.sh で作る）
# 終了コード: 0 = 全項目合格 / 1 = 不合格 / 2 = 誤用・前提の欠け（fail-closed）
# 回帰テスト: scripts/test-check-third-pack.sh
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

usage() {
  echo "usage: $(basename "$0") [--pack <dir>]" >&2
  exit 2
}

PACK="${ROOT_DIR}/template-workshop/examples/task-breakdown"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --pack)
      [ "$#" -ge 2 ] || usage
      PACK="$2"
      shift 2
      ;;
    *) usage ;;
  esac
done

# 値は fandhe-pack の引数になる。相対の親（..）の要素と先頭の - は誤用として断る。
case "${PACK}" in
  -* | */../* | ../* | */.. | ..)
    echo "error: unsafe pack path" >&2
    exit 2
    ;;
esac
if [ ! -f "${PACK}/workshop.yaml" ]; then
  echo "error: workshop.yaml not found in pack dir" >&2
  exit 2
fi

require_cmd grep || exit 2
require_cmd mktemp || exit 2
require_cmd rm || exit 2
require_cmd cat || exit 2
require_cmd wc || exit 2
require_cmd sed || exit 2

BIN="${FANDHE_PACK_BIN:-}"
if [ -z "${BIN}" ]; then
  require_cmd cargo || exit 2
  BIN="$("${DIR}/build-pack-bin.sh")" || {
    echo "error: could not build fandhe-pack" >&2
    exit 2
  }
fi
if [ ! -f "${BIN}" ] || [ ! -x "${BIN}" ]; then
  echo "error: fandhe-pack is not an executable file" >&2
  exit 2
fi
BIN="$(cd "$(dirname "${BIN}")" && pwd)/$(basename "${BIN}")"

WORK=""
cleanup() {
  if [ -n "${WORK}" ] && [ -d "${WORK}" ]; then
    rm -rf "${WORK}"
  fi
}
trap cleanup EXIT
WORK="$(mktemp -d)"

FAILS=0
pass() { echo "PASS $1"; }
fail() {
  echo "FAIL $1: $2"
  FAILS=$((FAILS + 1))
}

# run_pack <command> <dir>: 標準出力を OUT、標準エラー出力を ERR、終了コードを RC に入れる（失敗しても止めない）。
OUT=""
ERR=""
RC=0
run_pack() {
  local errf="${WORK}/stderr.txt"
  RC=0
  OUT="$("${BIN}" "$@" 2>"${errf}")" || RC=$?
  ERR="$(cat "${errf}")"
}

# 1 validate
run_pack validate "${PACK}"
if [ "${RC}" -eq 0 ] && [ -z "${ERR}" ] \
  && [ "$(printf '%s\n' "${OUT}" | wc -l | tr -d ' ')" = "1" ] \
  && printf '%s' "${OUT}" | grep -q '^OK: 診断 0 件（' && printf '%s' "${OUT}" | grep -q '章 1、'; then
  pass validate
else
  fail validate "validate did not end with 0 diagnostics and 1 chapter (exit ${RC})"
fi

# 2 test
run_pack test "${PACK}"
TEST_OUT="${OUT}"
PT_LINE="$(printf '%s\n' "${TEST_OUT}" | grep -E '^模範解答: [0-9]+/[0-9]+ 完走' || true)"
EI_LINE="$(printf '%s\n' "${TEST_OUT}" | grep -E '^誤入力: [0-9]+/[0-9]+ が期待どおり' || true)"
if [ "${RC}" -eq 0 ] && ! printf '%s\n' "${TEST_OUT}" | grep -q '^FAIL ' \
  && printf '%s' "${PT_LINE}" | grep -qE '^模範解答: ([1-9][0-9]*)/\1 完走' \
  && printf '%s' "${EI_LINE}" | grep -qE '^誤入力: ([0-9]+)/\1 が期待どおり'; then
  pass test
else
  fail test "test did not pass every case with at least 1 model answer (exit ${RC})"
fi

# 3 zero-counts（行が無い古い実行ファイルでは不合格にする。正規表現は check-second-pack.sh と同じ形）
R22_RE='^R-22 .*確認なしで達成になった ([0-9]+) 件$'
R21_RE='^R-21 返した文面 [0-9]+ 件のうち「次の行動」が空 ([0-9]+) 件$'
R22="$(printf '%s\n' "${TEST_OUT}" | sed -n -E 's/'"${R22_RE#^}"'/\1/p' || true)"
R21="$(printf '%s\n' "${TEST_OUT}" | sed -n -E 's/'"${R21_RE#^}"'/\1/p' || true)"
if [ "${R22}" = "0" ] && [ "${R21}" = "0" ]; then
  pass zero-counts
else
  fail zero-counts "R-22 or R-21 line is missing or not 0"
fi

if [ "${FAILS}" -ne 0 ]; then
  echo "NG: ${FAILS} item(s) failed"
  exit 1
fi
echo "OK: all items passed"
