#!/usr/bin/env bash
# 第 2 教材（レビューの頼み方）の骨格の設定値と診断の試験（`make check-second-pack`。CI 未組み込み。TASK-743）。
#
# 目的: 第 2 教材が「エンジンの Rust を変えずに足した教材」であること（W-20）を、設定値で固定する。
# 選択肢だけで進み（SECOND-7）、左に文書のビューだけを出し（SECOND-8）、コマンドを持たず（SECOND-9）、
# 読む外観で（SECOND-10）、章 1 つ・開始点 1 つ・掲示板の語彙なし（SECOND-11）であることを fandhe-pack の出力で確かめる。
# Rust・Cargo.toml・Cargo.lock には触れない（エンジンの差分 0 の数え方に入るため。試験は scripts/ の側に置く）。
#
# 項目（1 項目 1 行。PASS <名前> / FAIL <名前>: <理由>。教材の本文・IR の中身は出さない）
#   validate / compile / profile / commands / appearance / left_pane / starts / chapters / views / doc-rows（TASK-744）/ vocab /
#   no-board-words / test / no-input-command / no-input-text
# E-PROFILE-NO-INPUT を出すのは compile だけで validate は出さない（crates/pack/src/validate.rs の check_module_expansion が
# 捨てる）。そのため変種の確認は compile で行う。
#
# 使い方: scripts/check-second-pack.sh [--pack <dir>]
#   --pack  教材パックの根。省略時は template-workshop/examples/review-request。回帰テストが変異を入れた写しを渡すための口
# 環境変数: FANDHE_PACK_BIN（無ければ scripts/build-pack-bin.sh で作る）
# 終了コード: 0 = 全項目合格 / 1 = 不合格 / 2 = 誤用・前提の欠け（fail-closed）
# 回帰テスト: scripts/test-check-second-pack.sh
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

usage() {
  echo "usage: $(basename "$0") [--pack <dir>]" >&2
  exit 2
}

PACK="${ROOT_DIR}/template-workshop/examples/review-request"
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

# 値は fandhe-pack の引数・cp の対象になる。相対の親（..）の要素と先頭の - は誤用として断る。
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

require_cmd jq || exit 2
require_cmd grep || exit 2
require_cmd mktemp || exit 2
require_cmd rm || exit 2

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

# run_pack <command> <dir> [args...]: 標準出力を OUT、標準エラー出力を ERR、終了コードを RC に入れる（失敗しても止めない）。
OUT=""
ERR=""
RC=0
run_pack() {
  local errf="${WORK}/stderr.txt"
  RC=0
  OUT="$("${BIN}" "$@" 2>"${errf}")" || RC=$?
  ERR="$(cat "${errf}")"
}

# jq_is <name> <json> <filter> <reason>: filter が true を返したときだけ合格。
jq_is() {
  local name="$1" json="$2" filter="$3" reason="$4"
  if printf '%s' "${json}" | jq -e "${filter}" >/dev/null 2>&1; then
    pass "${name}"
  else
    fail "${name}" "${reason}"
  fi
}

# 1 validate
run_pack validate "${PACK}"
if [ "${RC}" -eq 0 ] && [ -z "${ERR}" ] && [ "$(printf '%s\n' "${OUT}" | wc -l | tr -d ' ')" = "1" ] \
  && printf '%s' "${OUT}" | grep -q '^OK: 診断 0 件（' && printf '%s' "${OUT}" | grep -q '章 1、'; then
  pass validate
else
  fail validate "validate did not end with 0 diagnostics and 1 chapter (exit ${RC})"
fi

# 2 compile（IR を以降の項目の入力にする）
run_pack compile "${PACK}"
IR="${OUT}"
if [ "${RC}" -eq 0 ] && [ -z "${ERR}" ] && printf '%s' "${IR}" | jq -e . >/dev/null 2>&1; then
  pass compile
else
  fail compile "compile did not exit 0 with empty stderr and a JSON IR (exit ${RC})"
  IR='{}'
fi

# 3〜9 IR の設定値
jq_is profile "${IR}" '.pack.profile == {"command_prefix":"","input_field":"hidden"}' "pack.profile is not command_prefix \"\" and input_field hidden"
jq_is commands "${IR}" '.commands == {}' "commands is not empty"
jq_is appearance "${IR}" '.pack.appearance.theme == "reading"' "pack.appearance.theme is not reading"
jq_is left_pane "${IR}" '.pack.left_pane == {"world_list":"hidden","view_tabs":"hidden"}' "pack.left_pane is not world_list hidden and view_tabs hidden"
jq_is starts "${IR}" '(.starts | type == "array") and (.starts | length == 1)' "starts is not exactly 1 entry"
jq_is chapters "${IR}" '([.nodes[] | select(.scope == "main") | .unit] | unique | length) == 1' "chapter count is not 1"
jq_is views "${IR}" '((.views | keys) == ["v-doc"]) and ([.views[].kind] | unique == ["document"]) and ((.nodes | length) >= 1) and ([.nodes[] | .step.views == ["v-doc"]] | all)' "views are not only v-doc (document) with every step on [v-doc]"

# 練習データと文書のビューの 5 行（SECOND-5。TASK-744）。開始点の effects が review/* の 5 つの put だけで、
# すべて練習データ fx-draft（document）を指し、IR 全体に別の経路の put が無いことを固定する。
# 架空の印の欠けは項目 validate（E-FICTIONAL-MARK）が検出するので、別の項目は作らない。
jq_is doc-rows "${IR}" '([.starts[0].effects[]? | .put.path] == ["review/status","review/to","review/scope","review/focus","review/due"]) and ([.starts[0].effects[]? | (has("put") and .put.fixture == "fx-draft")] | all) and (([.. | objects | select(has("put")) | .put.path] | unique) == ["review/due","review/focus","review/scope","review/status","review/to"]) and ((.fixtures | keys) == ["fx-draft"]) and (.fixtures["fx-draft"].kind == "document") and ((.fixture_docs | keys) == ["fx-draft"])' "document rows are not exactly the 5 review/* puts on fx-draft (document)"

# 章のディレクトリ数も確かめる（IR とファイルの両面）
chapter_dirs=0
for d in "${PACK}"/chapters/*/; do
  [ -d "${d}" ] && chapter_dirs=$((chapter_dirs + 1))
done
if [ "${chapter_dirs}" -ne 1 ]; then
  fail chapters "chapters/ does not hold exactly 1 directory"
fi

# 10 vocab
run_pack vocab "${PACK}"
VOCAB="${OUT}"
if [ "${RC}" -eq 0 ] && printf '%s' "${VOCAB}" | grep -q '^# EP-10 .*: pack.profile' \
  && printf '%s' "${VOCAB}" | grep -q '^# EP-11 .*: pack.appearance' \
  && printf '%s' "${VOCAB}" | grep -q '^# EP-12 .*: pack.left_pane'; then
  pass vocab
else
  fail vocab "vocab did not exit 0 with EP-10, EP-11 and EP-12 lines (exit ${RC})"
fi

# 11 掲示板の語彙・シェル・工程の語が無い（SECOND-11）。
# 固定の一覧で、付録 A の検査道具（tools/check-generic-words.mjs。crates/ 向け）は流用しない。
# 英語は字句の一致（英数字の連なりが語になる。_ と - で区切られた語も当たる）と bbs の部分一致。
# 日本語は部分一致。`工程` 単体は一般の語として誤検出しやすいため入れない。
EN_WORDS='(board|forum|thread|post|reply)'
JA_WORDS='掲示板|スレッド|投稿|返信|書き込み|シェル|ターミナル|6 工程|6工程'
hits=0
SCAN="${WORK}/scan.txt"
: >"${SCAN}"
for f in "${PACK}/workshop.yaml" "${PACK}"/chapters "${PACK}"/tests "${PACK}"/fixtures; do
  [ -e "${f}" ] || continue
  find "${f}" -type f -print0 | while IFS= read -r -d '' g; do cat "${g}"; echo; done >>"${SCAN}"
done
printf '%s\n' "${VOCAB}" >>"${SCAN}"
if grep -qiE "bbs|(^|[^A-Za-z0-9])${EN_WORDS}([^A-Za-z0-9]|$)|${JA_WORDS}" "${SCAN}"; then
  hits=1
fi
if [ "${hits}" -eq 0 ]; then
  pass no-board-words
else
  fail no-board-words "forbidden board/shell/process words found in pack text or vocab output"
fi

# 12 test
run_pack test "${PACK}"
if [ "${RC}" -eq 0 ] && ! printf '%s\n' "${OUT}" | grep -q '^FAIL ' \
  && printf '%s\n' "${OUT}" | grep -qE '^模範解答: ([1-9][0-9]*)/\1 完走'; then
  pass test
else
  fail test "test did not pass all cases with at least 1 model answer completed (exit ${RC})"
fi

# 13・14 入力欄のない教材に command / text の Step を足した変種は compile が E-PROFILE-NO-INPUT で止める（SECOND-7）。
# chapter.yaml は steps: が最後の欄であることを前提に、末尾へ Step を追記する。
# 追記が成り立たないとき（steps: が無い・最後でない）は fail-closed で失敗にする。
variant_check() {
  local name="$1" step_yaml="$2" vdir="${WORK}/variant-${1}"
  local chapter
  rm -rf "${vdir}"
  cp -R "${PACK}" "${vdir}"
  chapter="$(find "${vdir}/chapters" -name chapter.yaml | head -n 1)"
  if [ -z "${chapter}" ] || ! grep -q '^steps:' "${chapter}" \
    || [ "$(grep '^[A-Za-z_]' "${chapter}" | tail -n 1 | cut -d: -f1)" != "steps" ]; then
    fail "${name}" "cannot build variant (last field of chapter.yaml is not steps:)"
    return
  fi
  printf '%s\n' "${step_yaml}" >>"${chapter}"
  run_pack compile "${vdir}"
  if [ "${RC}" -eq 1 ] && printf '%s\n%s\n' "${OUT}" "${ERR}" | grep -q 'step:st-variant: E-PROFILE-NO-INPUT:'; then
    pass "${name}"
  else
    fail "${name}" "compile did not stop with E-PROFILE-NO-INPUT (exit ${RC})"
  fi
}

variant_check no-input-command '  - id: st-variant
    title: 変種
    do: 変種
    views: [v-doc]
    input:
      kind: command
      name: noop
    rules: []'
variant_check no-input-text '  - id: st-variant
    title: 変種
    do: 変種
    views: [v-doc]
    input: { kind: text, min: 1, max: 10 }
    rules: []'

if [ "${FAILS}" -ne 0 ]; then
  echo "NG: ${FAILS} item(s) failed" >&2
  exit 1
fi
echo "OK: all items passed"
