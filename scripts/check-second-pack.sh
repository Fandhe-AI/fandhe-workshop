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
#   ar12-reading（TASK-756。SECOND-22）/ ar8-reading（TASK-755。SECOND-21・SECOND-14）/ vocab / no-board-words / test / steps / row-owner / play-sync / row-asserts /
#   alt-differs / baseline（TASK-745）/ no-input-command / no-input-text / wrong-scenes・reject-keeps-rows（TASK-746 1/2）/
#   error-inputs-sync・text-checks・zero-counts（TASK-746 2/2）/ ir-match・format-b・learning-goals（TASK-747）/
#   ep-used・label-field・ep-public・ep-record・board-ep-overlap・board-rejections・no-rounds・input-kinds（TASK-753。SECOND-2・SECOND-11）
# ir-match の基準は tests/ir.json（compile の出力そのもの）。教材を変えたら fandhe-pack compile <pack> -o tests/ir.json で書き直す。
# CI の教材テスト（TASK-102）は FANDHE_PACK_BIN を渡してこのスクリプトを呼べば足りる（cargo 不要）。
# ar12-reading は AR-12 の読み（コマンドを持たない形で満たす。D-93・2026-11-02）の記録と設定値の一致を見る。
# 読みの是非は人の決定であり、確かめるのは docs/requirements.md の行・workshop.yaml・IR の設定値が一致することだけである。
# ar8-reading は AR-8 の読み（属性の変化を含む読み。D-93・2026-11-02）の記録と、5 行の属性の形のまま（本文を出さない側。SECOND-14 は検討中）
# であることのデータ側の固定を見る。SECOND-14 を人が「本文を出す形」に決めたら、この項目の IR 側の条件と
# tools/browser/choice-journey/ の docBodyRows を同時に逆にして、教材を書き直す（docs/requirements.md の 7h）。
# E-PROFILE-NO-INPUT を出すのは compile だけで validate は出さない（crates/pack/src/validate.rs の check_module_expansion が
# 捨てる）。そのため変種の確認は compile で行う。
#
# 使い方: scripts/check-second-pack.sh [--pack <dir>] [--board-pack <dir>] [--vocab-doc <path>]
#   --pack  教材パックの根。省略時は template-workshop/examples/review-request。回帰テストが変異を入れた写しを渡すための口
#   --board-pack  比べる掲示板教材の根。省略時は testdata/module-reuse-rounds/test-data/pack（公開の写し。教材 repo の掲示板教材ができたら差し替える）
#   --vocab-doc   公開の拡張点の一覧の文書。省略時は docs/vocabulary/vocabulary.md（回帰テストが変異を入れた写しを渡す口）
# 環境変数: FANDHE_PACK_BIN（無ければ scripts/build-pack-bin.sh で作る）
# 終了コード: 0 = 全項目合格 / 1 = 不合格 / 2 = 誤用・前提の欠け（fail-closed）
# 回帰テスト: scripts/test-check-second-pack.sh
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

usage() {
  echo "usage: $(basename "$0") [--pack <dir>] [--board-pack <dir>] [--vocab-doc <path>]" >&2
  exit 2
}

PACK="${ROOT_DIR}/template-workshop/examples/review-request"
BOARD="${ROOT_DIR}/testdata/module-reuse-rounds/test-data/pack"
VOCAB_DOC="${ROOT_DIR}/docs/vocabulary/vocabulary.md"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --pack)
      [ "$#" -ge 2 ] || usage
      PACK="$2"
      shift 2
      ;;
    --board-pack)
      [ "$#" -ge 2 ] || usage
      BOARD="$2"
      shift 2
      ;;
    --vocab-doc)
      [ "$#" -ge 2 ] || usage
      VOCAB_DOC="$2"
      shift 2
      ;;
    *) usage ;;
  esac
done

# 値は fandhe-pack の引数・cp の対象になる。相対の親（..）の要素と先頭の - は誤用として断る。
for p in "${PACK}" "${BOARD}" "${VOCAB_DOC}"; do
  case "${p}" in
    -* | */../* | ../* | */.. | ..)
      echo "error: unsafe path" >&2
      exit 2
      ;;
  esac
done
if [ ! -f "${PACK}/workshop.yaml" ] || [ ! -f "${BOARD}/workshop.yaml" ]; then
  echo "error: workshop.yaml not found in pack dir" >&2
  exit 2
fi
if [ ! -f "${VOCAB_DOC}" ]; then
  echo "error: vocabulary document not found" >&2
  exit 2
fi

require_cmd jq || exit 2
require_cmd grep || exit 2
require_cmd mktemp || exit 2
require_cmd rm || exit 2
require_cmd cmp || exit 2
require_cmd cp || exit 2
require_cmd wc || exit 2
require_cmd sed || exit 2
require_cmd awk || exit 2
require_cmd sort || exit 2
require_cmd comm || exit 2
require_cmd cut || exit 2
require_cmd tr || exit 2

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
# 教材ディレクトリへ cd した後でも見つかるよう、相対パスの BIN は絶対パスへ解決する。
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

# ar8-reading: AR-8 の読み（D-93）の記録と、5 行の属性の形のままであること（TASK-755）。理由は固定の英語の文だけを出す。
# 記録が読めない・節 7h が無いときも fail-closed で FAIL にする。
ar8_ok=1
if [ ! -f "${REQ}" ]; then
  ar8_ok=0
else
  ar8_rows="$(grep -c '^| AR-8 |' "${REQ}" || true)"
  if [ "${ar8_rows}" != "1" ]; then
    ar8_ok=0
  else
    ar8_row="$(grep '^| AR-8 |' "${REQ}")"
    for needle in '属性の変化' 'D-93（2026-11-02）' 'SECOND-21' 'AR-8 満たした'; do
      printf '%s' "${ar8_row}" | grep -qF -- "${needle}" || ar8_ok=0
    done
    if printf '%s' "${ar8_row}" | grep -qF -- '部分的' || printf '%s' "${ar8_row}" | grep -qF -- '満たしていない'; then
      ar8_ok=0
    fi
  fi
  if [ "$(grep -c '^## 7h\. TASK-755 の記録' "${REQ}" || true)" != "1" ]; then
    ar8_ok=0
  fi
fi
if ! printf '%s' "${IR}" | jq -e '.pack.vocab == "1.3" and ((.views["v-doc"].body // "hidden") == "hidden")' >/dev/null 2>&1; then
  ar8_ok=0
fi
if [ "${ar8_ok}" -eq 1 ]; then
  pass ar8-reading
else
  fail ar8-reading "AR-8 reading record or the attribute-form (no body shown) settings do not agree"
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
  && printf '%s\n' "${OUT}" | grep -qE '^模範解答: ([1-9][0-9]*)/\1 完走' \
  && printf '%s\n' "${OUT}" | grep -qE '^誤入力: ([0-9]+)/\1 が期待どおり'; then
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
  in_err=0
  while IFS= read -r line; do
    case "${line}" in
      "  - id: "*) if [ "${in_err}" -eq 0 ]; then cur="${line#  - id: }"; fi ;;
      "error_inputs:"*)
        cur=""
        in_err=1
        ;;
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

# 20a error-inputs-sync（TASK-746 2/2）: tests/playthrough.yaml の誤入力の区画を固定の配置で厳密に読み、
# 文面の確認（tests/text-checks.json）の error_input の集合と一致することを確かめる。
# 配置: `  - id:` → `    start: s-1` → 任意の `    pre:` と `      - { ... }` の列 → `    input: { ... }`。
# コメント・空行・別の形は読めない形として不合格にする（fail-closed）。baseline がこの結果を使うので baseline の前に置く。
EI_TSV="${WORK}/ei.tsv"
ERRS="${WORK}/errs.json"
: >"${EI_TSV}"
echo '[]' >"${ERRS}"
EI_OK=0
if [ -f "${PACK}/tests/playthrough.yaml" ]; then
  ei_started=0
  ei_bad=0
  ei_stage=0 # 0 = 次は id か終わり、1 = id の後、2 = start の後、4 = pre: の後、5 = pre の項目の後、3 = input の後
  ei_count=0
  while IFS= read -r line || [ -n "${line}" ]; do
    if [ "${ei_started}" -eq 0 ]; then
      if [ "${line}" = "error_inputs:" ]; then
        ei_started=1
      fi
      continue
    fi
    ei_id="$(printf '%s\n' "${line}" | sed -n -E 's/^  - id: ([A-Za-z0-9_-]+)$/\1/p')"
    ei_pre="$(printf '%s\n' "${line}" | sed -n -E 's/^      - \{ (choice|ui): "?([^",]*)"?, expect: ([a-z_:-]+) \}$/\1:\2|\3/p')"
    ei_inp="$(printf '%s\n' "${line}" | sed -n -E 's/^    input: \{ (choice|ui): "?([^",]*)"?, expect: "([a-z_:-]+)" \}$/\1:\2|\3/p')"
    ei_can="$(printf '%s\n' "${line}" | sed -n -E 's/^    input: \{ cancel: true, expect: "([a-z_:-]+)" \}$/cancel|\1/p')"
    if [ -n "${ei_id}" ] && { [ "${ei_stage}" -eq 0 ] || [ "${ei_stage}" -eq 3 ]; }; then
      printf '%s\tid\t-\t-\n' "${ei_id}" >>"${EI_TSV}"
      ei_count=$((ei_count + 1))
      ei_stage=1
    elif [ "${line}" = "    start: s-1" ] && [ "${ei_stage}" -eq 1 ]; then
      ei_stage=2
    elif [ "${line}" = "    pre:" ] && [ "${ei_stage}" -eq 2 ]; then
      ei_stage=4
    elif [ -n "${ei_pre}" ] && { [ "${ei_stage}" -eq 4 ] || [ "${ei_stage}" -eq 5 ]; }; then
      printf '%s\tpre\t%s\t%s\n' "-" "${ei_pre%%|*}" "${ei_pre##*|}" >>"${EI_TSV}"
      ei_stage=5
    elif [ -n "${ei_inp}" ] && { [ "${ei_stage}" -eq 2 ] || [ "${ei_stage}" -eq 5 ]; }; then
      printf '%s\tinput\t%s\t%s\n' "-" "${ei_inp%%|*}" "${ei_inp##*|}" >>"${EI_TSV}"
      ei_stage=3
    elif [ -n "${ei_can}" ] && { [ "${ei_stage}" -eq 2 ] || [ "${ei_stage}" -eq 5 ]; }; then
      printf '%s\tinput\tcancel\t%s\n' "-" "${ei_can##*|}" >>"${EI_TSV}"
      ei_stage=3
    else
      ei_bad=1
    fi
  done <"${PACK}/tests/playthrough.yaml"
  if [ "${ei_started}" -eq 1 ] && [ "${ei_bad}" -eq 0 ] && [ "${ei_count}" -ge 1 ] && [ "${ei_stage}" -eq 3 ]; then
    EI_OK=1
  fi
fi
if [ "${EI_OK}" -eq 1 ]; then
  jq -R -s -c 'reduce (split("\n")[] | select(length > 0) | split("\t")) as $r ([];
      if $r[1] == "id" then . + [{id: $r[0], pre: [], input: null, expect: null}]
      elif $r[1] == "pre" then .[-1].pre += [{op: $r[2], expect: $r[3]}]
      else (.[-1].input = $r[2]) | (.[-1].expect = $r[3]) end)' <"${EI_TSV}" >"${ERRS}" 2>/dev/null || EI_OK=0
fi
TC="${PACK}/tests/text-checks.json"
TC_OK=0
if [ -f "${TC}" ] && jq -e '.format == 1 and (.checks | type == "array") and (.excluded | type == "array")' "${TC}" >/dev/null 2>&1; then
  TC_OK=1
fi
# shellcheck disable=SC2016 # $tc は jq の変数
if [ "${EI_OK}" -eq 1 ] && [ "${TC_OK}" -eq 1 ] \
  && jq -e --slurpfile tc "${TC}" '(length >= 1) and ((map(.id) | unique | length) == length) and (map(.input != null) | all) and (([.[].id] | sort) == ([$tc[0].checks[].error_input] | sort))' "${ERRS}" >/dev/null 2>&1; then
  pass error-inputs-sync
else
  fail error-inputs-sync "playthrough.yaml error_inputs are unreadable, or do not match tests/text-checks.json (ids)"
fi

# 20b text-checks（TASK-746 2/2。SECOND-12）: 文面の確認。各 check について次をすべて満たすときだけ合格にする。
#   (a) test の出力に PASS <id> → rejected:<分類> の行がある（分類は test が観測した値）
#   (b) 静的に解いた（scripts/second-pack-judge.jq）最後の入力が rejected で、文面の ID と分類が check・test と一致し、pre の入力の結果も期待と一致する
#   (c) 文面の ID の分類が (a) と一致する
#   (d) message・next_action が IR の値と一致し、空でない
#   (e) 網羅: 場面 1〜8 のうち除外（場面 5 だけ）を除く 7 場面に check がある。IR の確認の誤答の選択肢すべてに場面 1 の check がある
# 限界: 照合するのは IR に載った文面と静的に解いた文面の ID で、実行時にエンジンが返した文面ではない。画面の文面は TASK-750 が確かめる。
JUDGE="${DIR}/second-pack-judge.jq"
JR="${WORK}/judge-results.json"
echo '{}' >"${JR}"
tc_pass=0
if [ "${EI_OK}" -eq 1 ] && [ "${TC_OK}" -eq 1 ] && [ "$(printf '%s' "${IR}" | jq -r '(.nodes | length)')" != "0" ]; then
  tc_pass=1
  for eid in $(jq -r '.[].id' "${ERRS}" | grep -E '^[A-Za-z0-9_-]+$'); do
    eops="$(jq -c --arg id "${eid}" '[.[] | select(.id == $id)][0] | [(.pre[] | .op), .input]' "${ERRS}")"
    eres="$(printf '%s' "${IR}" | jq -c --argjson ops "${eops}" -f "${JUDGE}" 2>/dev/null)" || eres='null'
    jq --arg id "${eid}" --argjson r "${eres}" '.[$id] = $r' "${JR}" >"${JR}.new" && mv "${JR}.new" "${JR}"
  done
  TMAP="$(printf '%s\n' "${TEST_OUT}" | sed -n -E 's/^PASS ([A-Za-z0-9_-]+) → rejected:([a-z_]+)$/\1\t\2/p' | jq -R -s -c '[split("\n")[] | select(length > 0) | split("\t") | {key: .[0], value: .[1]}] | from_entries')"
  # shellcheck disable=SC2016 # jq の変数をシェルに展開させない
  TC_FILTER='
    def trim: gsub("^\\s+|\\s+$"; "");
    . as $ir
    | ($tc[0]) as $t
    | (($t.checks | length) >= 1)
      and (($t.checks | map(.error_input) | unique | length) == ($t.checks | length))
      and all($t.checks[]; . as $c
        | ([$errs[0][] | select(.id == $c.error_input)][0]) as $e
        | $e != null
        and ($tmap[$c.error_input] != null)
        and ($ir.rejections[$c.rejection] != null)
        and ($tmap[$c.error_input] == $ir.rejections[$c.rejection].category)
        and (($jr[0][$c.error_input] | type) == "array")
        and (($jr[0][$c.error_input] | length) == (($e.pre | length) + 1))
        and ($jr[0][$c.error_input] as $r
          | ($r[-1].kind == "rejected") and ($r[-1].rejection == $c.rejection)
          and ($r[-1].category == ($e.expect | ltrimstr("rejected:")))
          and ([range(0; ($e.pre | length)) as $i | ($e.pre[$i].expect) as $x | $r[$i]
                | if ($x | startswith("rejected:")) then (.kind == "rejected" and .category == ($x | ltrimstr("rejected:"))) else (.kind == $x) end] | all))
        and ($c.message == $ir.rejections[$c.rejection].message)
        and ($c.next_action == $ir.rejections[$c.rejection].next_action)
        and (($c.message | type) == "string") and (($c.message | trim | length) >= 1)
        and (($c.next_action | type) == "string") and (($c.next_action | trim | length) >= 1)
        and (($c.scene | type) == "number") and ($c.scene >= 0) and ($c.scene <= 8) and ($c.scene != 5))
      and (($t.excluded | map(.scene)) == [5])
      and (([$t.checks[] | select(.scene >= 1) | .scene] | unique) == [1, 2, 3, 4, 6, 7, 8])
      and ([$ir.nodes[].step.confirm.options[]? | select(.correct == false) | .id] as $w
            | (($w | length) >= 1) and all($w[]; . as $oid
            | any($t.checks[]; .scene == 1 and (. as $c | ([$errs[0][] | select(.id == $c.error_input)][0].input) == ("choice:" + $oid)))))
  '
  printf '%s' "${IR}" | jq -e --slurpfile tc "${TC}" --slurpfile errs "${ERRS}" --slurpfile jr "${JR}" --argjson tmap "${TMAP}" "${TC_FILTER}" >/dev/null 2>&1 || tc_pass=0
fi
if [ "${tc_pass}" -eq 1 ]; then
  pass text-checks
else
  fail text-checks "rejection texts do not match tests/text-checks.json (ids, categories, messages, next actions or scene coverage)"
fi

# 20c zero-counts（TASK-746 2/2）: test の R-22・R-21 の行の 0 であるべき数が 0 であること。行が無い（古い実行ファイル）ときも不合格にする。
R22_RE='^R-22 .*確認の操作で達成になった ([0-9]+) 件、確認を省くと明示した Step の達成 ([0-9]+) 件、確認なしで達成になった ([0-9]+) 件$'
R21_RE='^R-21 返した文面 ([0-9]+) 件のうち「次の行動」が空 ([0-9]+) 件$'
R22_CONFIRMED="$(printf '%s\n' "${TEST_OUT}" | sed -n -E "s/${R22_RE}/\\1/p")"
R22_SKIP="$(printf '%s\n' "${TEST_OUT}" | sed -n -E "s/${R22_RE}/\\2/p")"
R22_NOCONF="$(printf '%s\n' "${TEST_OUT}" | sed -n -E "s/${R22_RE}/\\3/p")"
R21_TEXTS="$(printf '%s\n' "${TEST_OUT}" | sed -n -E "s/${R21_RE}/\\1/p")"
R21_EMPTY="$(printf '%s\n' "${TEST_OUT}" | sed -n -E "s/${R21_RE}/\\2/p")"
if [ "${R22_NOCONF}" = "0" ] && [ "${R21_EMPTY}" = "0" ] && [ -n "${R22_CONFIRMED}" ] && [ -n "${R22_SKIP}" ] && [ -n "${R21_TEXTS}" ]; then
  pass zero-counts
else
  fail zero-counts "test output lacks the R-22 or R-21 line, or its must-be-zero count is not 0"
fi
TC_COUNT=0
TC_SCENES=0
TC_EXCLUDED=0
if [ "${TC_OK}" -eq 1 ]; then
  TC_COUNT="$(jq '.checks | length' "${TC}" 2>/dev/null || echo 0)"
  TC_SCENES="$(jq '[.checks[] | select(.scene >= 1) | .scene] | unique | length' "${TC}" 2>/dev/null || echo 0)"
  TC_EXCLUDED="$(jq '.excluded | length' "${TC}" 2>/dev/null || echo 0)"
fi

# ir-match（TASK-747。SECOND-4）: compile の出力が tests/ir.json とバイト単位で一致する。
# 4 通り（-o・標準出力のリダイレクト・-o の 2 回目・別のディレクトリへ写して相対パス）で書き出して cmp -s で比べる。
# 項目 compile の IR 変数はコマンド置換で末尾の改行が落ちるため、バイト比較には使わず、必ずファイルへ書き出す。
# 基準が無い・通常のファイルでない場合は不合格（fail-closed）。差分の中身は出さない。
IR_REF="${PACK}/tests/ir.json"
ir_ok=1
IR_BYTES=0
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
  if cp -R "${PACK}" "${WORK}/elsewhere/review-request" \
    && (cd "${WORK}/elsewhere" && "${BIN}" compile review-request -o "${WORK}/ir-copy.json" >/dev/null 2>&1) \
    && cmp -s "${IR_REF}" "${WORK}/ir-copy.json"; then
    :
  else
    ir_ok=0
  fi
fi
if [ "${ir_ok}" -eq 1 ]; then
  pass ir-match
  IR_BYTES="$(wc -c <"${IR_REF}" | tr -d ' ')"
else
  fail ir-match "compile output is not byte-identical to tests/ir.json (-o, stdout, repeat, copied dir)"
fi

# format-b（TASK-747。SECOND-4）: 形式 B の構成（workshop.yaml・章 1 つの chapter.yaml・本文の Markdown 8 つ・playthrough・練習データ）。
FB_OK=1
FB_BODIES=0
printf '%s' "${VALIDATE_OUT}" | grep -q '形式 B(YAML)' || FB_OK=0
[ -f "${PACK}/workshop.yaml" ] || FB_OK=0
[ -f "${PACK}/tests/playthrough.yaml" ] || FB_OK=0
fb_chapters=0
fb_dir=""
for f in "${PACK}"/chapters/*/chapter.yaml; do
  if [ -f "${f}" ]; then
    fb_chapters=$((fb_chapters + 1))
    fb_dir="$(dirname "${f}")"
  fi
done
if [ "${fb_chapters}" -ne 1 ]; then
  FB_OK=0
else
  FB_LIST="$(sed -n -E 's/^    body: (.*)$/\1/p' "${fb_dir}/chapter.yaml")"
  while IFS= read -r b; do
    [ -n "${b}" ] || continue
    if printf '%s' "${b}" | grep -qE '^[A-Za-z0-9_-]+\.md$' && [ -f "${fb_dir}/${b}" ]; then
      FB_BODIES=$((FB_BODIES + 1))
    else
      FB_OK=0
    fi
  done <<EOF2
${FB_LIST}
EOF2
  [ "${FB_BODIES}" -eq 8 ] || FB_OK=0
fi
FB_PATHS="$(printf '%s' "${IR}" | jq -r '.fixtures[]?.path' 2>/dev/null || true)"
while IFS= read -r p; do
  [ -n "${p}" ] || continue
  if printf '%s' "${p}" | grep -qE '^[A-Za-z0-9_/-]+\.[a-z]+$' && ! printf '%s' "${p}" | grep -qF '..' && [ -f "${PACK}/${p}" ]; then
    :
  else
    FB_OK=0
  fi
done <<EOF2
${FB_PATHS}
EOF2
if [ "${FB_OK}" -eq 1 ]; then
  pass format-b
else
  fail format-b "pack is not the format B layout (workshop.yaml, one chapter.yaml, 8 body files, playthrough, fixtures)"
fi

# learning-goals（TASK-747。SECOND-4）: Step の lo の和集合が requirements.md の学習目標の表の ID 集合と一致し、2 つで、全 Step が 1 つ以上を持つ。
LG_COUNT=0
LG_OK=1
if [ -f "${REQ}" ]; then
  LG_DOC="$(sed -n -E 's/^\| (LO-[0-9]+) \|.*/\1/p' "${REQ}" | sort -u | jq -R . | jq -sc .)"
else
  LG_DOC='[]'
  LG_OK=0
fi
if printf '%s' "${IR}" | jq -e --argjson doc "${LG_DOC}" '([.nodes[].step.lo[]?] | unique) as $u | ($u == $doc) and ($u | length == 2) and ([.nodes[] | (.step.lo | length) >= 1] | all)' >/dev/null 2>&1; then
  LG_COUNT="$(printf '%s' "${LG_DOC}" | jq 'length')"
else
  LG_OK=0
fi
if [ "${LG_OK}" -eq 1 ]; then
  pass learning-goals
else
  fail learning-goals "learning goals in steps and requirements.md differ, are not 2, or a step has none"
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
    printf 'learning_goals=%s\nformat_b.bodies=%s\nir.bytes=%s\n' "${LG_COUNT}" "${FB_BODIES}" "${IR_BYTES}"
    printf 'ir.steps=%s\n' "$(printf '%s' "${IR}" | jq '.nodes | length')"
    printf 'text_checks=%s\ntext_checks.scenes=%s\ntext_checks.excluded=%s\n' "${TC_COUNT}" "${TC_SCENES}" "${TC_EXCLUDED}"
    printf 'r22.confirmed=%s\nr22.skip=%s\nr22.no_confirm=%s\nr21.texts=%s\nr21.empty=%s\n' "${R22_CONFIRMED}" "${R22_SKIP}" "${R22_NOCONF}" "${R21_TEXTS}" "${R21_EMPTY}"
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

# ---- TASK-753（SECOND-2・SECOND-11）: 使った拡張点の照合と、掲示板教材との比較 ----
# 比べる相手は掲示板教材の公開の写し（既定は testdata/module-reuse-rounds/test-data/pack。--board-pack で差し替え）。
# private の spec の掲示板教材はここから読まない（public repo に文面を写さないため）。写しが覆わない部分の限界は
# docs/requirements.md の 7g と docs/testing.md の 5.453 に記録する。FAIL の理由は固定の文と件数だけで、文面は出さない。

# ep_set <vocab の出力>: EP の ID を番号順・重複なしで 1 行 1 件に出す（step の行の 4 列目と「# EP-n を使った」の行）。
ep_set() {
  {
    printf '%s\n' "$1" | grep -v '^#' | cut -d, -f4 | grep -oE 'EP-[0-9]+' || true
    printf '%s\n' "$1" | grep -E '^# EP-[0-9]+ を使った' | grep -oE 'EP-[0-9]+' || true
  } | sort -t- -k2,2n -u
}
# kind_set <IR の JSON>: 基本の語彙（入力・確認・ビューの種類）を接頭辞つきで出す。
kind_set() {
  printf '%s' "$1" | jq -r '[.nodes[]?.step | ((.input.kind? // empty) | "input:" + .), ((.confirm.kind? // empty) | "confirm:" + .)] + [.views[]?.kind | "view:" + .] | .[]' | LC_ALL=C sort -u
}

# 項目 vocab の結果（VOCAB）を使う前に、vocab 自体が 0 で終わったかを取り直す
run_pack vocab "${PACK}"
EP_VOCAB_RC="${RC}"
EP_USED="$(ep_set "${OUT}")"
EP_USED_LINE="$(printf '%s\n' "${EP_USED}" | tr '\n' ' ' | sed 's/ $//')"

# ep-used: 使った拡張点がちょうど EP-6・EP-7・EP-10・EP-11・EP-12（EP-9 などが混じれば不合格）。vocab が 0 で終わらないときも不合格
if [ "${EP_VOCAB_RC}" -eq 0 ] && [ "${EP_USED_LINE}" = "EP-6 EP-7 EP-10 EP-11 EP-12" ]; then
  pass ep-used
else
  fail ep-used "used extension points are not exactly EP-6, EP-7, EP-10, EP-11 and EP-12 (vocab exit ${EP_VOCAB_RC})"
fi

# label-field: 確認の選択肢が 1 件以上あり、すべてが label を持ち、語彙の版が 1.3。公開の文書に label の行がちょうど 1 行ある
# shellcheck disable=SC2016 # バッククォートは表の文字そのもの
LABEL_DOC_ROWS="$(grep -cF '| `field` | `confirm.options[].label` | `v1.3` | — |' "${VOCAB_DOC}" || true)"
if printf '%s' "${IR}" | jq -e '([.nodes[].step.confirm.options[]?] | length) >= 1 and ([.nodes[].step.confirm.options[]? | has("label")] | all) and (.pack.vocab == "1.3")' >/dev/null 2>&1 \
  && [ "${LABEL_DOC_ROWS}" = "1" ]; then
  pass label-field
else
  fail label-field "confirm.options[].label is not on every option, pack.vocab is not 1.3, or the public list lacks the label row"
fi

# ep-public: 使った拡張点がすべて公開の一覧（vocabulary.md の 2 節の表）にある。一覧が読めない（0 件）ときも不合格
EP_PUBLIC="$(awk '/^## 2\./{f=1;next} /^## /{f=0} f' "${VOCAB_DOC}" | grep -oE '^\| EP-[0-9]+ ' | grep -oE 'EP-[0-9]+' | LC_ALL=C sort -u || true)"
ep_public_ok=1
if [ -z "${EP_PUBLIC}" ] || [ -z "${EP_USED}" ]; then
  ep_public_ok=0
else
  for e in ${EP_USED}; do
    printf '%s\n' "${EP_PUBLIC}" | grep -qxF -- "${e}" || ep_public_ok=0
  done
fi
if [ "${ep_public_ok}" -eq 1 ]; then
  pass ep-public
else
  fail ep-public "a used extension point is not in the public list, or the list could not be read"
fi

# ep-record: 作者の記録（requirements.md の 7g の表）の EP の集合が vocab の集合と一致し、label の行がちょうど 1 行ある
ep_record_ok=1
if [ ! -f "${REQ}" ]; then
  ep_record_ok=0
else
  REC_SET="$({ grep -oE '^\| EP-[0-9]+ ' "${REQ}" || true; } | grep -oE 'EP-[0-9]+' | sort -t- -k2,2n -u | tr '\n' ' ' | sed 's/ $//')"
  [ "${REC_SET}" = "${EP_USED_LINE}" ] || ep_record_ok=0
  # shellcheck disable=SC2016 # バッククォートは表の文字そのもの
  [ "$(grep -cF '| `confirm.options[].label` |' "${REQ}" || true)" = "1" ] || ep_record_ok=0
fi
if [ "${ep_record_ok}" -eq 1 ]; then
  pass ep-record
else
  fail ep-record "the author record of used extension points does not match the vocab output"
fi

# board-ep-overlap / board-rejections: 掲示板教材の写しと比べる（重なる拡張点は EP-6・EP-7 だけ。拒否の文面の一致は 0 件）
run_pack vocab "${BOARD}"
BOARD_VOCAB="${OUT}"
BOARD_VOCAB_RC="${RC}"
run_pack compile "${BOARD}"
BOARD_IR="${OUT}"
BOARD_IR_RC="${RC}"
if ! printf '%s' "${BOARD_IR}" | jq -e . >/dev/null 2>&1; then
  BOARD_IR_RC=1
  BOARD_IR='{"nodes":[],"views":{}}'
fi

overlap_ok=1
if [ "${BOARD_VOCAB_RC}" -ne 0 ] || [ "${BOARD_IR_RC}" -ne 0 ]; then
  overlap_ok=0
fi
BOARD_EP="$(ep_set "${BOARD_VOCAB}")"
EP_COMMON="$(comm -12 <(printf '%s\n' "${EP_USED}" | LC_ALL=C sort) <(printf '%s\n' "${BOARD_EP}" | LC_ALL=C sort) | sort -t- -k2,2n | tr '\n' ' ' | sed 's/ $//')"
[ "${EP_COMMON}" = "EP-6 EP-7" ] || overlap_ok=0
OWN_KINDS="$(kind_set "${IR}")"
BOARD_KINDS="$(kind_set "${BOARD_IR}")"
COMMON_KINDS="$(comm -12 <(printf '%s\n' "${OWN_KINDS}") <(printf '%s\n' "${BOARD_KINDS}") | grep -c . || true)"
BOARD_ONLY_KINDS="$(comm -13 <(printf '%s\n' "${OWN_KINDS}") <(printf '%s\n' "${BOARD_KINDS}") | grep -c . || true)"
if [ "${COMMON_KINDS}" -lt 1 ] || [ "${BOARD_ONLY_KINDS}" -lt 1 ]; then
  overlap_ok=0
fi
if [ "${overlap_ok}" -eq 1 ]; then
  pass board-ep-overlap
else
  fail board-ep-overlap "shared extension points are not exactly EP-6 and EP-7, the basic vocabulary does not overlap partially, or the board pack did not compile (vocab exit ${BOARD_VOCAB_RC}, compile exit ${BOARD_IR_RC})"
fi

# shellcheck disable=SC2016 # jq の式
REJ_FILTER='[.rejections[]? | .message, .next_action | select(type == "string" and length > 0)] | unique | .[] | tojson'
OWN_REJ="$(printf '%s' "${IR}" | jq -r "${REJ_FILTER}" 2>/dev/null | LC_ALL=C sort -u || true)"
BOARD_REJ="$(printf '%s' "${BOARD_IR}" | jq -r "${REJ_FILTER}" 2>/dev/null | LC_ALL=C sort -u || true)"
REJ_COMMON="$(comm -12 <(printf '%s\n' "${OWN_REJ}") <(printf '%s\n' "${BOARD_REJ}") | grep -c . || true)"
if [ -n "${OWN_REJ}" ] && [ -n "${BOARD_REJ}" ] && [ "${REJ_COMMON}" = "0" ]; then
  pass board-rejections
else
  fail board-rejections "rejection texts shared with the board pack: ${REJ_COMMON} (or one side has no rejection text)"
fi

# no-rounds（SECOND-11）: 周が 1 つ（instances は main の 1 件・scope は main だけ）で、モジュールの置き場を持たない
if printf '%s' "${IR}" | jq -e '(.instances | length == 1) and (.instances[0].id == "main") and ([.nodes[].scope] | unique == ["main"])' >/dev/null 2>&1 \
  && [ ! -e "${PACK}/modules" ]; then
  pass no-rounds
else
  fail no-rounds "the pack has more than one round, a scope other than main, or a modules directory"
fi

# input-kinds（SECOND-11）: 入力の種類は choice・ui_action だけ（command・text など、シェル風の入力を持たない）
# shellcheck disable=SC2016 # jq の変数
if printf '%s' "${IR}" | jq -e '([.nodes[].step.input.kind] | unique) as $k | ($k | length) >= 1 and (($k - ["choice","ui_action"]) | length == 0)' >/dev/null 2>&1; then
  pass input-kinds
else
  fail input-kinds "an input kind other than choice and ui_action is used"
fi

if [ "${FAILS}" -ne 0 ]; then
  echo "NG: ${FAILS} item(s) failed" >&2
  exit 1
fi
echo "OK: all items passed"
