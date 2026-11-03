#!/usr/bin/env bash
# 別の作者が公開の資料だけで書いた第 2 教材の写し（docs/second-pack-repro/pack）の再現検査（`make check-second-pack-repro`。
# CI 未組み込み。TASK-413 1/2。AUTHOR-12）。
#
# 目的: エンジンのソースを見ない作者が、公開の資料と fandhe-pack だけで第 2 教材を書き、validate・test を通せたことの
# 記録（docs/testing.md 5.449）を、写しに対して再実行できるようにする。製品の第 2 教材の検査
# （scripts/check-second-pack.sh）とは別の教材を見るため、ID・件数は共有しない。Rust・Cargo.* には触れない（W-20）。
# 分担: IR のバイト一致（ir-match。TASK-413 2/2）は作者が compile した tests/ir.json を基準にする。作者の写し
# （pack/** と tests/ir.json）は凍結し、一致が崩れたら compile で書き直さず所見として記録する（製品の検査とは逆）。
# 不足の記録は docs/known-gaps.md が持つ。
#
# 項目（1 項目 1 行。PASS <名前> / FAIL <名前>: <理由>）
#   validate    診断 0 件・章 1
#   test        終了コード 0・模範解答と誤入力が全件合格（a/a・b/b）
#   constraints compile の IR が brief の設定値（profile・appearance・left_pane・commands・文書のビューだけ・開始点 1・章 1）を満たす
#   baseline    <pack>/../baseline.txt の件数と実測が一致する
#   ir-match    compile の出力が tests/ir.json（作者の出力）とバイト単位で一致する（-o・標準出力・-o の 2 回目・別の場所へ写した相対パス）
#
# 使い方: scripts/check-second-pack-repro.sh [--pack <dir>]
#   --pack  省略時は docs/second-pack-repro/pack。baseline.txt は <dir> の 1 つ上に置く
# 環境変数: FANDHE_PACK_BIN（無ければ scripts/build-pack-bin.sh で作る）
# 終了コード: 0 = 全項目合格 / 1 = 不合格 / 2 = 誤用・前提の欠け（fail-closed）
# 回帰テスト: scripts/test-check-second-pack-repro.sh
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

usage() {
  echo "usage: $(basename "$0") [--pack <dir>]" >&2
  exit 2
}

PACK="${ROOT_DIR}/docs/second-pack-repro/pack"
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
BASELINE="${PACK}/../baseline.txt"
if [ ! -f "${BASELINE}" ]; then
  echo "error: baseline.txt not found next to pack dir" >&2
  exit 2
fi

require_cmd jq || exit 2
require_cmd grep || exit 2
require_cmd mktemp || exit 2
require_cmd rm || exit 2
require_cmd cat || exit 2
require_cmd cmp || exit 2
require_cmd cp || exit 2
require_cmd mkdir || exit 2

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

# baseline_get <key>: baseline.txt の key=value の value（無ければ空）。
baseline_get() {
  local line
  line="$(grep -E "^$1=" "${BASELINE}" | head -n 1 || true)"
  printf '%s' "${line#*=}"
}

# 1 validate
run_pack validate "${PACK}"
VALIDATE_OUT="${OUT}"
if [ "${RC}" -eq 0 ] && [ -z "${ERR}" ] && printf '%s' "${OUT}" | grep -q '^OK: 診断 0 件（' && printf '%s' "${OUT}" | grep -q '章 1、'; then
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

# 3 constraints
run_pack compile "${PACK}"
IR="${OUT}"
if [ "${RC}" -ne 0 ] || ! printf '%s' "${IR}" | jq -e . >/dev/null 2>&1; then
  IR='{}'
fi
if printf '%s' "${IR}" | jq -e '
  (.pack.profile == {"command_prefix":"","input_field":"hidden"})
  and (.commands == {})
  and (.pack.appearance.theme == "reading")
  and (.pack.left_pane == {"world_list":"hidden","view_tabs":"hidden"})
  and ((.starts | type == "array") and (.starts | length == 1))
  and (([.nodes[] | select(.scope == "main") | .unit] | unique | length) == 1)
  and ((.views | keys) == ["v-doc"])
  and ([.views[].kind] | unique == ["document"])
  and ((.nodes | length) >= 1)
  and ([.nodes[] | .step.views == ["v-doc"]] | all)' >/dev/null 2>&1; then
  pass constraints
else
  fail constraints "IR settings differ from the brief (profile, appearance, left_pane, commands, views, starts or chapters)"
fi

# 4 baseline（件数は test の出力と IR から作る。値の書式は baseline.txt を参照）
act_steps="$(printf '%s' "${IR}" | jq -r '.nodes | length' 2>/dev/null || echo x)"
act_pt="$(printf '%s' "${PT_LINE}" | sed -n -E 's/^模範解答: ([0-9]+\/[0-9]+) 完走.*/\1/p')"
act_ei="$(printf '%s' "${EI_LINE}" | sed -n -E 's/^誤入力: ([0-9]+\/[0-9]+) が期待どおり.*/\1/p')"
act_total="$(printf '%s\n' "${TEST_OUT}" | sed -n -E 's/^入力の総数: ([0-9]+).*/\1/p')"
act_chapters="$(printf '%s' "${VALIDATE_OUT}" | sed -n -E 's/.*章 ([0-9]+)、.*/\1/p')"
if [ "$(baseline_get validate.chapters)" = "${act_chapters}" ] \
  && [ "$(baseline_get validate.steps)" = "${act_steps}" ] \
  && [ "$(baseline_get playthroughs)" = "${act_pt}" ] \
  && [ "$(baseline_get error_inputs)" = "${act_ei}" ] \
  && [ "$(baseline_get total_inputs)" = "${act_total}" ]; then
  pass baseline
else
  fail baseline "counts differ from baseline.txt (chapters, steps, playthroughs, error_inputs or total_inputs)"
fi

# 5 ir-match（バイト比較。IR 変数はコマンド置換で末尾の改行が落ちるため使わず、必ずファイルへ書き出して cmp -s で比べる）
# 基準が無い・通常のファイルでない場合は不合格（fail-closed）。差分の中身は出さない。
IR_REF="${PACK}/tests/ir.json"
ir_ok=1
if [ ! -f "${IR_REF}" ] || [ -L "${IR_REF}" ]; then
  ir_ok=0
else
  mkdir -p "${WORK}/elsewhere"
  if "${BIN}" compile "${PACK}" -o "${WORK}/ir-o1.json" >"${WORK}/ir-o1.out" 2>"${WORK}/ir-o1.err" \
    && [ ! -s "${WORK}/ir-o1.out" ] && [ ! -s "${WORK}/ir-o1.err" ] && cmp -s "${IR_REF}" "${WORK}/ir-o1.json"; then
    :
  else
    ir_ok=0
  fi
  if "${BIN}" compile "${PACK}" >"${WORK}/ir-stdout.json" 2>/dev/null && cmp -s "${IR_REF}" "${WORK}/ir-stdout.json"; then
    :
  else
    ir_ok=0
  fi
  if "${BIN}" compile "${PACK}" -o "${WORK}/ir-o2.json" >/dev/null 2>&1 && cmp -s "${IR_REF}" "${WORK}/ir-o2.json"; then
    :
  else
    ir_ok=0
  fi
  if cp -R "${PACK}" "${WORK}/elsewhere/authored" \
    && (cd "${WORK}/elsewhere" && "${BIN}" compile authored -o "${WORK}/ir-copy.json" >/dev/null 2>&1) \
    && cmp -s "${IR_REF}" "${WORK}/ir-copy.json"; then
    :
  else
    ir_ok=0
  fi
fi
if [ "${ir_ok}" -eq 1 ]; then
  pass ir-match
else
  fail ir-match "compile output is not byte-identical to tests/ir.json (-o, stdout, repeat, copied dir)"
fi

if [ "${FAILS}" -ne 0 ]; then
  echo "NG: ${FAILS} item(s) failed"
  exit 1
fi
echo "OK: all items passed"
