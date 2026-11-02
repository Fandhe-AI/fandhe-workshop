#!/usr/bin/env bash
# 第 2 教材（レビューの頼み方）の骨格の設定値と診断の試験（`make check-second-pack`。CI 未組み込み。TASK-743）。
#
# 目的: 第 2 教材が「エンジンの Rust を変えずに足した教材」であること（W-20）を、設定値で固定する。
# 選択肢だけで進み（SECOND-7）、左に文書のビューだけを出し（SECOND-8）、コマンドを持たず（SECOND-9）、
# 読む外観で（SECOND-10）、章 1 つ・開始点 1 つ・掲示板の語彙なし（SECOND-11）であることを fandhe-pack の出力で確かめる。
# Rust・Cargo.toml・Cargo.lock には触れない（エンジンの差分 0 の数え方に入るため。試験は scripts/ の側に置く）。
#
# 項目（1 項目 1 行。PASS <名前> / FAIL <名前>: <理由>。教材の本文・IR の中身は出さない）
#   validate / compile / profile / commands / appearance / left_pane / starts / chapters / views / doc-rows（TASK-744）/
#   ar12-reading（TASK-756。SECOND-22）/ vocab / no-board-words / test / steps / row-owner / play-sync / row-asserts /
#   alt-differs / baseline（TASK-745）/ no-input-command / no-input-text / wrong-scenes・reject-keeps-rows（TASK-746）
# ar12-reading は AR-12 の読み（コマンドを持たない形で満たす。D-93・2026-11-02）の記録と設定値の一致を見る。
# 読みの是非は人の決定であり、確かめるのは docs/requirements.md の行・workshop.yaml・IR の設定値が一致することだけである。
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
VALIDATE_OUT="${OUT}"
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

# ar12-reading: AR-12 の読み（D-93）の記録・ソースの設定値・IR の設定値の 3 つがそろうこと（SECOND-22）。
# 理由は固定の英語の文だけを出し、記録の本文は出さない。記録が読めないときも fail-closed で FAIL にする。
REQ="${PACK}/docs/requirements.md"
ar12_ok=1
if [ ! -f "${REQ}" ]; then
  ar12_ok=0
else
  ar12_rows="$(grep -c '^| AR-12 |' "${REQ}" || true)"
  if [ "${ar12_rows}" != "1" ]; then
    ar12_ok=0
  else
    ar12_row="$(grep '^| AR-12 |' "${REQ}")"
    # shellcheck disable=SC2016 # 記録の行のバッククォートは字句どおりの固定文字列（展開しない）
    for needle in 'コマンドを持たない形' '`command_prefix: ""`' '`commands: {}`' 'D-93（2026-11-02）' 'SECOND-22' 'AR-12 満たした（解釈つき）'; do
      printf '%s' "${ar12_row}" | grep -qF -- "${needle}" || ar12_ok=0
    done
    if printf '%s' "${ar12_row}" | grep -qF -- '満たしていない'; then
      ar12_ok=0
    fi
  fi
fi
if [ "$(grep -cE '^[[:space:]]+command_prefix: ""([[:space:]]|$)' "${PACK}/workshop.yaml" || true)" != "1" ] \
  || [ "$(grep -cE '^commands: \{\}([[:space:]]|$)' "${PACK}/workshop.yaml" || true)" != "1" ]; then
  ar12_ok=0
fi
if ! printf '%s' "${IR}" | jq -e '.pack.profile.command_prefix == "" and .commands == {}' >/dev/null 2>&1; then
  ar12_ok=0
fi
if [ "${ar12_ok}" -eq 1 ]; then
  pass ar12-reading
else
  fail ar12-reading "AR-12 reading record, workshop.yaml and IR settings do not agree (no-command form)"
fi

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
TEST_OUT="${OUT}"
if [ "${RC}" -eq 0 ] && ! printf '%s\n' "${OUT}" | grep -q '^FAIL ' \
  && printf '%s\n' "${OUT}" | grep -qE '^模範解答: ([1-9][0-9]*)/\1 完走'; then
  pass test
else
  fail test "test did not pass all cases with at least 1 model answer completed (exit ${RC})"
fi

# 15〜20 選んで整える Step 群と模範解答（TASK-745）。
# fandhe-pack の test は asserts を判定しない（AUTHOR-4。定義があると終了コード 1）ため、行の変化の期待は
# tests/row-asserts.json に置き、コンパイル済みの IR の効果を通しの入力の列に沿って畳み込んで照合する（scripts/second-pack-fold.jq）。
# 限界: 状態変化の語（set_attr・put）の適用をここで写している。test・画面は開始点の effects を適用しないので、
# 実行時の世界状態とは一致しない（docs/testing.md 5.442）。
RA="${PACK}/tests/row-asserts.json"
FOLD="${DIR}/second-pack-fold.jq"
BASELINE="${PACK}/tests/baseline.txt"
RA_OK=0
if [ -f "${RA}" ] && jq -e '.format == 1 and (.playthroughs | type == "object")' "${RA}" >/dev/null 2>&1; then
  RA_OK=1
fi

# 15 steps
# shellcheck disable=SC2016 # $i は jq の変数
jq_is steps "${IR}" '([.nodes[].uid] == ["main/st-1","main/st-2","main/st-3","main/st-4","main/st-5","main/st-6","main/st-7","main/st-8"]) and ([.nodes[] | select(.step.final == true) | .uid] == ["main/st-8"]) and ([.nodes[0:7][] | ((.step.next | length) == 1) and (.step.next[0].when == null)] | all) and ([range(0; 7) as $i | .nodes[$i].step.next[0].to == .nodes[$i + 1].uid] | all)' "steps are not st-1..st-8 in order with only st-8 final and one next each"

# 16 row-owner: 各 Step の効果が、その Step の担当の行と review/status だけを set_attr で変える（put は開始点だけ）
# shellcheck disable=SC2016 # $own・$n・$paths は jq の変数
jq_is row-owner "${IR}" '{"main/st-1":["review/status"],"main/st-2":["review/to","review/status"],"main/st-3":["review/scope","review/status"],"main/st-4":["review/focus","review/status"],"main/st-5":["review/due","review/status"],"main/st-6":["review/due","review/status"],"main/st-7":["review/status"],"main/st-8":["review/status"]} as $own | [.nodes[] | . as $n | ([($n.step.rules[]?.outcome.effects[]?), ($n.step.on_achieved[]?)] | map(if (keys == ["set_attr"]) then .set_attr.path else "<unsupported>" end)) as $paths | (($paths - ($own[$n.uid] // [])) | length) == 0] | all' "a step changes a row outside its own row and review/status, or uses an effect other than set_attr"

# 17 play-sync: tests/playthrough.yaml の入力の列（1 入力 1 行の flow 形式）が row-asserts.json の op・expect と一致する
SYNC_ACTUAL="${WORK}/sync-actual.tsv"
: >"${SYNC_ACTUAL}"
if [ -f "${PACK}/tests/playthrough.yaml" ]; then
  cur=""
  while IFS= read -r line; do
    case "${line}" in
      "  - id: "*) cur="${line#  - id: }" ;;
      "error_inputs:"*) cur="" ;;
      *)
        if [ -n "${cur}" ]; then
          parsed="$(printf '%s\n' "${line}" | sed -n -E 's/^ +- \{ (choice|ui): "?([^",]*)"?, expect: ([a-z_:-]+) \}$/\1:\2|\3/p')"
          if [ -n "${parsed}" ]; then
            printf '%s\t%s\n' "${cur}" "${parsed}" >>"${SYNC_ACTUAL}"
          fi
        fi
        ;;
    esac
  done <"${PACK}/tests/playthrough.yaml"
fi
if [ "${RA_OK}" -eq 1 ] \
  && jq -R -s -e --slurpfile ra "${RA}" '(split("\n") | map(select(length > 0) | split("\t")) | group_by(.[0]) | map({key: .[0][0], value: map(.[1])}) | from_entries) == ($ra[0].playthroughs | map_values([.inputs[] | .op + "|" + .expect]))' <"${SYNC_ACTUAL}" >/dev/null 2>&1; then
  pass play-sync
else
  fail play-sync "playthrough.yaml inputs do not match tests/row-asserts.json (or row-asserts.json is missing or malformed)"
fi

# 18 row-asserts: IR の効果の畳み込みが、入力ごとの変わる行・終わりの 5 行・最終 Step の到達と一致する
FOLD_RESULTS="${WORK}/fold-results.json"
echo '{}' >"${FOLD_RESULTS}"
ra_pass=1
ra_checked=0
if [ "${RA_OK}" -eq 1 ] && [ "$(printf '%s' "${IR}" | jq -r '(.nodes | length)')" != "0" ]; then
  for id in $(jq -r '.playthroughs | keys[]' "${RA}" | grep -E '^[A-Za-z0-9_-]+$'); do
    ops="$(jq -c --arg id "${id}" '[.playthroughs[$id].inputs[].op]' "${RA}")"
    res="$(printf '%s' "${IR}" | jq -c --argjson ops "${ops}" -f "${FOLD}" 2>/dev/null)" || res='{"ok":false}'
    printf '%s' "${res}" | jq -e --arg id "${id}" --slurpfile ra "${RA}" '.ok == true and .node == "main/st-8" and ([.steps[].changes] == [$ra[0].playthroughs[$id].inputs[].changes]) and (.final == $ra[0].playthroughs[$id].final)' >/dev/null 2>&1 || ra_pass=0
    ra_checked=$((ra_checked + $(jq --arg id "${id}" '.playthroughs[$id].inputs | length' "${RA}")))
    jq --arg id "${id}" --argjson r "${res}" '.[$id] = $r' "${FOLD_RESULTS}" >"${FOLD_RESULTS}.new" && mv "${FOLD_RESULTS}.new" "${FOLD_RESULTS}"
  done
else
  ra_pass=0
fi
if [ "${ra_pass}" -eq 1 ] && [ "${ra_checked}" -gt 0 ]; then
  pass row-asserts
else
  fail row-asserts "folded IR effects do not match tests/row-asserts.json (changed rows per input, final rows, or final step)"
fi

# 19 alt-differs: 別の選び方（pt-alt）では、宛先と期限の行が別の文面になり、どちらも初期値のままではない
ALT_DIFFERS=0
if [ "${RA_OK}" -eq 1 ]; then
  ALT_DIFFERS="$(jq -r --slurpfile ra "${RA}" '[$ra[0].differs[]? as $d | select(((.["pt-main"].final[$d[0]][$d[1]]) != null) and ((.["pt-alt"].final[$d[0]][$d[1]]) != null) and ((.["pt-main"].final[$d[0]][$d[1]]) != (.["pt-alt"].final[$d[0]][$d[1]])) and ((.["pt-main"].final[$d[0]][$d[1]]) != "（まだ決めていません）") and ((.["pt-alt"].final[$d[0]][$d[1]]) != "（まだ決めていません）")) | 1] | length' "${FOLD_RESULTS}" 2>/dev/null || echo 0)"
fi
if [ "${RA_OK}" -eq 1 ] && [ "${ALT_DIFFERS}" -ge 2 ] && [ "${ALT_DIFFERS}" = "$(jq '.differs | length' "${RA}")" ] \
  && jq -e '.differs | (map(.[0] + "." + .[1]) | (index("review/to.detail") != null) and (index("review/due.detail") != null))' "${RA}" >/dev/null 2>&1; then
  pass alt-differs
else
  fail alt-differs "pt-main and pt-alt do not end with different review/to and review/due texts"
fi

# 20 baseline: 件数が tests/baseline.txt（時点の値）と一致する（キーの欠け・余り・不一致は不合格）
if [ -f "${BASELINE}" ]; then
  BASE_ACTUAL="${WORK}/baseline-actual.txt"
  {
    printf 'validate.chapters=%s\n' "$(printf '%s' "${VALIDATE_OUT}" | sed -n -E 's/.*章 ([0-9]+)、.*/\1/p')"
    printf 'validate.steps=%s\n' "$(printf '%s' "${VALIDATE_OUT}" | sed -n -E 's/.*Step の定義 ([0-9]+).*/\1/p')"
    printf 'playthroughs=%s\n' "$(printf '%s\n' "${TEST_OUT}" | sed -n -E 's/^模範解答: ([0-9]+\/[0-9]+) 完走.*/\1/p')"
    printf '%s\n' "${TEST_OUT}" | sed -n -E 's/^PASS ([A-Za-z0-9_-]+)（入力 ([0-9]+) 件）$/inputs.\1=\2/p'
    printf 'error_inputs=%s\n' "$(printf '%s\n' "${TEST_OUT}" | sed -n -E 's/^誤入力: ([0-9]+\/[0-9]+) .*/\1/p')"
    printf 'total_inputs=%s\n' "$(printf '%s\n' "${TEST_OUT}" | sed -n -E 's/^入力の総数: ([0-9]+)$/\1/p')"
    jq -r 'to_entries[] | "row_changes.\(.key)=\([.value.steps[]? | select(.changes != {})] | length)"' "${FOLD_RESULTS}"
    printf 'row_asserts=%s\n' "${ra_checked}"
    printf 'alt_differs=%s\n' "${ALT_DIFFERS}"
  } | sort >"${BASE_ACTUAL}"
  grep -v -e '^#' -e '^$' "${BASELINE}" | sort >"${WORK}/baseline-expected.txt" || true
  if cmp -s "${BASE_ACTUAL}" "${WORK}/baseline-expected.txt"; then
    pass baseline
  else
    fail baseline "counts differ from tests/baseline.txt (missing, extra or changed keys)"
  fi
else
  fail baseline "tests/baseline.txt is missing"
fi

# 21 wrong-scenes（TASK-746 1/2。SECOND-12）: 誤答への応答の 8 場面の規則と文面が IR に揃っている。
# 各場面の (Step, 規則, 文面 ID, 分類) の固定の表で照合する。文面は message・next_action の前後の空白を除いた長さが 1 以上であること。
# 場面 5 の規則 r-tomorrow-large は r-tomorrow より前にある（規則は上から読むため。畳み込みは when のある規則を除くので、並びはここでしか見ない）。
# shellcheck disable=SC2016 # jq の変数（$ir など）をシェルに展開させない
WS_FILTER='
  . as $ir
  | def st($id): [$ir.nodes[] | select(.step.id == $id) | .step][0];
  def rule($s; $r): [st($s).rules[] | select(.id == $r)][0];
  def rjok($id; $cat): ($ir.rejections[$id] // null) as $j | $j != null and $j.category == $cat
    and (($j.message | gsub("^\\s+|\\s+$"; "") | length) >= 1) and (($j.next_action | gsub("^\\s+|\\s+$"; "") | length) >= 1);
  def rej($s; $r; $rj; $cat): (rule($s; $r)) as $x | $x != null and $x.outcome.kind == "reject" and $x.outcome.rejection == $rj and rjok($rj; $cat);
  def wrongs($s): [st($s).confirm.options[] | select(.correct == false)] as $w | ($w | length) >= 1 and all($w[]; (.on_wrong // "") as $o | $o != "" and rjok($o; "not_in_this_step"));
  (["st-1","st-3","st-4","st-5","st-7","st-8"] | all(.[]; (st(.).confirm.kind == "answer") and wrongs(.)))
  and rej("st-2"; "r-all"; "rj-to-all"; "not_in_this_step")
  and ((st("st-2").confirm.on_miss // "") != "") and ((st("st-6").confirm.on_miss // "") != "")
  and ((st("st-5").input.targets | length) == 4)
  and rej("st-5"; "r-ui-to"; "rj-row-filled"; "not_in_this_step")
  and rej("st-5"; "r-ui-scope"; "rj-row-filled"; "not_in_this_step")
  and rej("st-5"; "r-ui-focus"; "rj-row-filled"; "not_in_this_step")
  and rej("st-6"; "r-tomorrow-large"; "rj-due-short"; "not_in_this_step")
  and (rule("st-6"; "r-tomorrow-large").when == {"chose": {"step": "st-3", "option": "opt-diff-tests"}})
  and ([st("st-6").rules[].id] | index("r-tomorrow-large")) < ([st("st-6").rules[].id] | index("r-tomorrow"))
  and rej("st-6"; "r-asap"; "rj-due-vague"; "bad_arg_shape")
  and rej("st-7"; "r-skip"; "rj-skip"; "not_in_this_step")
  and rej("st-8"; "r-revise"; "rj-revise"; "wrong_order")
'
jq_is wrong-scenes "${IR}" "${WS_FILTER}" "IR does not hold the 8 wrong-answer scenes (rules, rejection texts or categories)"

# 22 reject-keeps-rows: 拒否の規則の outcome は kind と rejection だけを持つ（effects を持たない＝拒否のあと左の行は変わらない）。拒否の規則が 1 件以上ある
# shellcheck disable=SC2016 # jq の変数をシェルに展開させない
RK_FILTER='[.nodes[].step.rules[]? | select(.outcome.kind == "reject")] as $r | ($r | length) >= 1 and all($r[]; (.outcome | keys) == ["kind", "rejection"])'
jq_is reject-keeps-rows "${IR}" "${RK_FILTER}" "a reject rule carries effects, or there is no reject rule"

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
