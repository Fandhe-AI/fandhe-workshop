#!/usr/bin/env bash
# scripts/check-second-pack.sh の回帰テスト（`make test-check-second-pack`。本物の fandhe-pack を使うため cargo が要り、やや遅い。TASK-743）。
#
# 確かめること: 誤用の拒否（終了コード 2）・変えていない写しの合格・設定値や文面を 1 か所ずつ壊した写しが
# 対応する項目で不合格になること（負の対照。置き換えが写しの中でちょうど 1 回起きない場合はテスト自体を失敗にする）。
# 終了コード: 0 = 全件期待どおり / 1 = 期待と違う件がある
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

SCRIPT="${DIR}/check-second-pack.sh"
SRC="${ROOT_DIR}/template-workshop/examples/review-request"

require_cmd cargo
require_cmd mktemp
require_cmd rm
if [ -z "${FANDHE_PACK_BIN:-}" ]; then
  FANDHE_PACK_BIN="$("${DIR}/build-pack-bin.sh")"
  export FANDHE_PACK_BIN
fi

WORK="$(mktemp -d)"
cleanup() {
  if [ -n "${WORK}" ] && [ -d "${WORK}" ]; then
    rm -rf "${WORK}"
  fi
}
trap cleanup EXIT

FAILS=0
ok() { echo "ok: $1"; }
ng() {
  echo "not ok: $1" >&2
  FAILS=$((FAILS + 1))
}

# expect_rc <名前> <期待の終了コード> <引数...>: 出力は OUT に残す。
OUT=""
expect_rc() {
  local name="$1" want="$2" rc=0
  shift 2
  OUT="$("${SCRIPT}" "$@" 2>&1)" || rc=$?
  if [ "${rc}" -eq "${want}" ]; then ok "${name}"; else ng "${name} (exit ${rc}, expected ${want})"; fi
}

expect_rc "未知の引数は 2" 2 --bogus
expect_rc "--pack の値なしは 2" 2 --pack
expect_rc "親ディレクトリの要素を含む値は 2" 2 --pack "${SRC}/../review-request"
expect_rc "- で始まる値は 2" 2 --pack -x
expect_rc "workshop.yaml の無い値は 2" 2 --pack "${WORK}"
expect_rc "変えていない写しは 0" 0 --pack "${SRC}"
# FANDHE_PACK_BIN を相対パスで渡しても（cd の後で）見つかり、合格する。
REL_BIN_DIR="$(dirname "${FANDHE_PACK_BIN}")"
REL_BIN_NAME="$(basename "${FANDHE_PACK_BIN}")"
REL_RC=0
(cd "${REL_BIN_DIR}" && FANDHE_PACK_BIN="./${REL_BIN_NAME}" "${SCRIPT}" --pack "${SRC}" >/dev/null 2>&1) || REL_RC=$?
if [ "${REL_RC}" -eq 0 ]; then ok "相対パスの FANDHE_PACK_BIN でも 0"; else ng "相対パスの FANDHE_PACK_BIN (exit ${REL_RC}, expected 0)"; fi

# mutate <名前> <相対パス> <sed の式> <期待する FAIL の項目>: 写しを作り、置き換えがちょうど 1 行に効くことを確かめる。
n=0
mutate() {
  local name="$1" rel="$2" expr="$3" item="$4" dir before after
  n=$((n + 1))
  dir="${WORK}/m${n}"
  cp -R "${SRC}" "${dir}"
  before="$(cat "${dir}/${rel}")"
  sed "${expr}" "${dir}/${rel}" >"${dir}/tmp.out"
  mv "${dir}/tmp.out" "${dir}/${rel}"
  after="$(cat "${dir}/${rel}")"
  if [ "${before}" = "${after}" ]; then
    ng "${name} (substitution had no effect)"
    return
  fi
  expect_rc "${name}は 1" 1 --pack "${dir}"
  if printf '%s\n' "${OUT}" | grep -q "^FAIL ${item}:"; then ok "${name}は ${item} で不合格"; else ng "${name} (no FAIL ${item} line)"; fi
}

W="workshop.yaml"
# BSD sed と GNU sed の両方で通る改行の渡し方（置換文字列の中は バックスラッシュ + 実改行）
NL="$(printf '\nX')"; NL="${NL%X}"
BS="\\" # 1 文字のバックスラッシュ
SED_VIEWS="s/^  v-doc: { kind: document }/  v-doc: { kind: document }${BS}${NL}  v-x: { kind: document }/"
SED_STARTS="s/^steps:/  - { id: s-2, node: st-1 }${BS}${NL}steps:/"
mutate "外観を standard にした写し" "${W}" 's/theme: reading/theme: standard/' appearance
mutate "入力欄を出す写し" "${W}" 's/input_field: hidden/input_field: shown/' profile
mutate "command_prefix を消した写し" "${W}" '/command_prefix:/d' profile
mutate "ビューの切り替えを出す写し" "${W}" 's/view_tabs: hidden/view_tabs: shown/' left_pane
mutate "コマンドを 1 件持つ写し" "${W}" 's/^commands: {}/commands: { noop: { args: [] } }/' commands
mutate "接頭辞を ! にした写し（採らなかった読み）" "${W}" 's/command_prefix: ""/command_prefix: "!"/' profile
mutate "commands の行を消した写し" "${W}" '/^commands: {}/d' ar12-reading
R="docs/requirements.md"
mutate "AR-12 の記録を満たしていないにした写し" "${R}" '/^| AR-12 |/s/AR-12 満たした（解釈つき）/AR-12 満たしていない/' ar12-reading
mutate "AR-12 の決定の日付を変えた写し" "${R}" '/^| AR-12 |/s/D-93（2026-11-02）/D-93（2026-11-03）/' ar12-reading
mutate "AR-12 の行を消した写し" "${R}" '/^| AR-12 |/d' ar12-reading
mutate "AR-8 の記録を部分的にした写し" "${R}" '/^| AR-8 |/s/AR-8 満たした/AR-8 部分的/' ar8-reading
mutate "AR-8 の決定の日付を変えた写し" "${R}" '/^| AR-8 |/s/D-93（2026-11-02）/D-93（2026-11-03）/' ar8-reading
mutate "AR-8 の行を消した写し" "${R}" '/^| AR-8 |/d' ar8-reading
mutate "TASK-755 の記録の節を消した写し" "${R}" '/^## 7h\. TASK-755 の記録/d' ar8-reading
mutate "文書のビューに本文を出す写し（vocab 1.4 を足す）" "${W}" 's/vocab: "1.3"/vocab: "1.4"/;s/v-doc: { kind: document }/v-doc: { kind: document, body: shown }/' ar8-reading
mutate "ビューを 1 件足した写し" "${W}" "${SED_VIEWS}" views
mutate "文面に禁止語を足した写し" "chapters/ch-request/st-1.md" 's/^# 状況を確認する/# 状況を確認する（掲示板）/' no-board-words
mutate "開始点を 1 件足した写し" "chapters/ch-request/chapter.yaml" "${SED_STARTS}" starts
C="chapters/ch-request/chapter.yaml"
mutate "期限の行を消した写し" "${C}" '/put: { path: review\/due/d' doc-rows
mutate "見てほしい点の経路を改名した写し" "${C}" 's/put: { path: review\/focus/put: { path: review\/point/' doc-rows
mutate "練習データの宣言を足した写し" "${W}" "s/^  fx-draft: .*/&${BS}${NL}  fx-extra: { path: fixtures\/draft.md, kind: document }/" doc-rows
mutate "練習データの架空の印を外した写し" "fixtures/draft.md" 's/【架空】//' validate
mutate "期限の文面の効果を外した写し" "${C}" '/value: 明日の 17 時までです/d' row-asserts
mutate "宛先の 2 つの文面を同じにした写し" "${C}" 's/同じチームの 1 人に頼みます/この部分に詳しい 1 人に頼みます/' alt-differs
mutate "最終 Step の印を外した写し" "${C}" '/^    final: true$/d' steps
mutate "基準値を 1 つ変えた写し" "tests/baseline.txt" 's/^row_asserts=32/row_asserts=31/' baseline
mutate "期待の変わる行を書き換えた写し" "tests/row-asserts.json" 's/今回直した差分だけです/今回直した差分です/' row-asserts
mutate "別の選び方の入力を替えた写し" "tests/playthrough.yaml" 's/opt-friday/opt-tomorrow/' play-sync
mutate "場面 5 の条件を外した写し" "${C}" '/when: { chose: { step: st-3/d' wrong-scenes
mutate "場面 8 の分類を替えた写し" "${W}" 's/rj-revise: { category: wrong_order/rj-revise: { category: not_in_this_step/' wrong-scenes
mutate "場面 2 の文面の参照を替えた写し" "${C}" 's/rejection: rj-to-all/rejection: rj-nis/' wrong-scenes
mutate "確認の誤答の解説を外した写し" "${C}" 's/, on_wrong: rj-q8-unsent//' wrong-scenes
mutate "拒否の規則に効果を足した写し" "${C}" 's/rejection: rj-skip }/rejection: rj-skip, effects: [] }/' reject-keeps-rows
mutate "拒否の次の行動を空にした写し" "${W}" 's/next_action: 日時が 1 つに決まる期限を選び直します。/next_action: ""/' validate

T="tests/text-checks.json"
mutate "文面の確認の次の行動を替えた写し" "${T}" 's/読む人が動ける答えを選び直します。/読む人が動ける答えを選びます。/' text-checks
mutate "教材の拒否の文面を替えた写し" "${W}" 's/範囲が広いと、読む人は終わりが見えません。/範囲が広いと、読む人は迷います。/' text-checks
mutate "文面の確認の文面 ID を替えた写し" "${T}" 's/"rejection": "rj-q1-feel"/"rejection": "rj-miss-q"/' text-checks
mutate "確認の誤答の解説の参照を外した写し（場面 1）" "${C}" 's/, on_wrong: rj-q1-code//' text-checks
mutate "誤入力の ID を改名した写し" "tests/playthrough.yaml" 's/id: w-q1-feel/id: w-q1-feel-x/' error-inputs-sync
mutate "誤入力の期待を替えた写し" "tests/playthrough.yaml" 's/expect: "rejected:wrong_order"/expect: "rejected:not_in_this_step"/' test
mutate "除外の場面を替えた写し" "${T}" 's/"scene": 5,/"scene": 2,/' text-checks
mutate "R-21 の基準値を変えた写し" "tests/baseline.txt" 's/^r21.empty=0/r21.empty=1/' baseline

# TASK-747: 基準の IR・形式 B・学習目標・基準値。教材の中身を変える変異は ir-match も不合格にする（mutate は指定の項目の行だけを見る）。
mutate "基準の IR の 1 か所を変えた写し" "tests/ir.json" 's/"id": "review-request"/"id": "review-requestx"/' ir-match
mutate "学習目標の定義に LO-3 を足した写し" "docs/requirements.md" "s/^| LO-2 | \(.*\)\$/| LO-2 | \1${BS}${NL}| LO-3 | 仮 |/" learning-goals
mutate "Step の lo を未定義の LO-3 にした写し" "${C}" 's/lo: \[LO-1\]$/lo: [LO-3]/' learning-goals
mutate "学習目標の基準値を替えた写し" "tests/baseline.txt" 's/^learning_goals=2/learning_goals=3/' baseline
mutate "IR のバイト数の基準値を替えた写し" "tests/baseline.txt" 's/^ir.bytes=.*/ir.bytes=1/' baseline

# mutate_fs <名前> <期待する FAIL の項目> <操作>: 写しに対してファイル操作（削除・末尾の改行を落とす）を行う。
mutate_fs() {
  local name="$1" item="$2" op="$3" dir
  n=$((n + 1))
  dir="${WORK}/m${n}"
  cp -R "${SRC}" "${dir}"
  case "${op}" in
    rm-ir) rm "${dir}/tests/ir.json" ;;
    rm-body) rm "${dir}/chapters/ch-request/st-1.md" ;;
    no-eol)
      printf '%s' "$(cat "${dir}/tests/ir.json")" >"${dir}/tests/ir.new"
      mv "${dir}/tests/ir.new" "${dir}/tests/ir.json"
      ;;
  esac
  expect_rc "${name}は 1" 1 --pack "${dir}"
  if printf '%s\n' "${OUT}" | grep -q "^FAIL ${item}:"; then ok "${name}は ${item} で不合格"; else ng "${name} (no FAIL ${item} line)"; fi
}
mutate_fs "基準の IR を消した写し" ir-match rm-ir
mutate_fs "基準の IR の末尾の改行を落とした写し" ir-match no-eol
mutate_fs "本文の Markdown を 1 つ消した写し" format-b rm-body

if [ "${FAILS}" -ne 0 ]; then
  echo "NG: ${FAILS} case(s) differ from expectation" >&2
  exit 1
fi
echo "OK: test-check-second-pack"
