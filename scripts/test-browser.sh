#!/usr/bin/env bash
# ブラウザ試験（Playwright の共通ヘルパ）を流す（`make test-browser` から呼ばれる）。
#
# 配信設定の web（TEST_HOOKS なし）と試験の設定の web（TEST_HOOKS=1）を一時ディレクトリへビルドし、
# tools/browser/*.test.mjs を node:test で実行する。配信設定は FANDHE_WEB_DIR、試験の設定は FANDHE_WEB_TEST_DIR で渡す。
# 共通ヘルパの試験（harness・safety。安全性の計数の対照）と、実際の画面の試験（smoke-hello・keydown-action・esc-attr・input-limit・paste・paste-notice（上限の案内文と貼り付けの観察。TERM-15・TERM-19）・learner-text・enter-submit・key-invariants（Enter 系・Tab 系のキーの不変条件の基準。FEND-21。TASK-351）・submit-wiring・tab-complete・shift-tab・tab-cycle・esc-key・json-depth・json-load-order・doc-links・doc-tree・tick-interval・three-views・log-lines-xss・reject-lines・choice-answer（選択肢の誤答・選び直し・達成。TERM-24）・approval-buttons（承認のボタンと対象つきの直す。TERM-27）・approval-flow（承認の操作のログ文言と CH-2 相当の完走。TERM-27）・input-ports（入力方式ごとに選択肢・承認のボタンが切り替わり、ui_action の Step は左の行のクリックで受付になる。TERM-23。TASK-450 1/3）・input-field-hidden（入力欄なしの教材で入力欄・送信・補完の候補・OS の選択が 0 件で、停止と選択肢・画面操作・承認・確認は出て最後まで進む。PACK-20・TERM-23。TASK-450 2/3）・ch0-3-run（CH-0〜CH-3 の形の 20 Step を `?fixture=ch0-3` で 1 つのページに開いて最後まで進め、CH-1 の 6 Step を含む区間（st-0-1 から st-2-1）をキーボードだけで完走する。TERM-23。TASK-450 3/3）・tree-attrs・list-parent-attrs・confirm-burst・stop-move-ignore・esc-outcomes（Esc の画面の結果。TERM-7。処理中は候補が開いていても停止し候補も閉じる・処理中でなく候補が開いていれば閉じるだけ・どちらでもなければ取り消しの入力で、受け付けない Step では拒否の行と次の行動が出る。TERM-8）・reload-ignore・reload-notice・restore-cancel・candidates・step-status（受付済み（確認待ち）の表示と確認を省く Step の受付と同時の達成。TERM-31。TASK-435 1/2）・confirm-status（指す・答える・承認するの 3 種の確認の後にだけ達成が出る。TERM-31。TASK-435 2/2）・history-export・history-import・artifact-view-check（左のビュー共通の検査器の自己確認。TASK-471）・artifact-view-controls（検査器の負の対照 3 種。印を消す・部品を項目の外に置く・innerHTML。TASK-471）・diff-view（差分 (b) の本体。インデント・行番号と記号の列・2 層の印。TASK-475 1/2。(a) の対照とインデントが畳まれる観察。TASK-475 2/2）・doc-view（文書 (b) の本体。見出し・一覧・表・引用・行内のコードの構造が保たれる。TASK-476 1/2。`-`/`+` の組・(a) の対照と構造喪失の観察。TASK-476 2/2）・card-view（カード (a)・(b) の本体と 20 件の規模。(a) は card_basic と状態・ラベルの tag で印は内側の div、(b) は article と marker で印は li 自身。ART-1・ART-17。TASK-477）・check-view（チェックの一覧 (a)・(b) の本体と 20 件の規模。(a) は rich_text・記号の span・状態の tag で印は内側の div、(b) は li 自身が印の宿主で marker のバッジ。状態は記号つきの文字。ART-1・ART-17。TASK-478）・app-demo-view（アプリのデモのワイヤー (a)・(b) の本体。(a) は text・tag・rich_text と input・button・card_basic で描き印は内側の div、(b) は headless-ui の field（読み取り専用）と自作の span・button で描き印は li 自身で role・aria- を出さない。タブの li は app.css で横に並ぶ。ART-1・ART-17。TASK-479）・point-rows（7 種のビューの指せる行が 4 属性を組で持ち、宛先の無い行は持たない。指せる行はフォーカスを受け 24x24 以上。クリックの効果は確かめない。ART-11。TASK-480 1/2）・point-click（指せる行のクリックが指す操作として受け付けられ、表示だけの行は何も受け付けない。アプリのデモの名札の行 screen/top だけが指せる行になる。ART-11。TASK-480 2/2）・point-keys（指せる行にフォーカスして Enter・Space を押すと click と同じ入力になる。押しっぱなしは 1 回・修飾キー付きは何もしない。A11Y-18・FEND-13。TASK-358 1/2）・keyboard-confirm（Tab・文字・Enter・Space だけで、確認を含む Step を達成まで進める。製品の左の成果物の行が指せる行になる。CSP あり・なし。A11Y-18・FEND-13。TASK-358 2/2）・task-tree（タスクツリー (a)・(b) の本体。(b) は木の属性、(a) はチェックと深さのクラス、どちらも状態の記号と文字。TASK-474）・artifact-mark-toggle（前後の切り替えで行も項目の内側も置き換わらないこと・印の文字を消す対照・切り替えて戻す対照。TASK-469 2/2）・view-switch（ビューの切り替え・更新で根の要素とフォーカスが保たれる・選択の属性の 101 回・部品を項目の外に置く対照。ART-5・ART-12。TASK-470 2/2）・ime（CDP による IME の模擬のヘルパの自己確認）・ime-input（入力欄と IME の層 3。BUILD-10。誤送信 0・送信漏れ 0。CDP の模擬）・history-versions（保存・復元・書き出し・版の層 3。BUILD-10。往復・読み込み後の保存と復帰・版の判定・壊れた履歴の拒否）・reload-fresh（保存済みの履歴が無い場合と未操作の再読み込みで新しく始まり世代が上がらない。操作後は復元される対照つき。SAVE-3・SAVE-28）・submit-disabled（送信ボタンが無効なときの Enter は 0 件・有効なときは click と同じ。FEND-14。TASK-352 2/3）・reload-points（8 Step の教材を `?fixture=reload-points` で起動の前に選び、実際の `page.reload()` で 15 地点の一致と完走した世界状態の一致を確かめる。TASK-319 3/3。SAVE-3）。SAFE-9・SAFE-12・EVT-3・EVT-7・EVT-8・EVT-9・EVT-10・EVT-11・EVT-12・EVT-18・SAVE-6・FEND-15・TERM-1・TERM-2・TERM-21）、
# 実際の画面への対照の自己確認（safety-control。SAFE-8。CSP あり・なし）を含む。
# 前提: `npm ci` と `npx playwright install chromium` の済み。導入はここでは試みない。
# Chrome の原本で流すときは FANDHE_BROWSER_CHANNEL=chrome を付ける（既定は同梱 Chromium）。
# CI では browser-test ジョブが流す（TASK-79。Playwright とブラウザの導入はジョブ側の step が行う）。
#
# 終了コード: 0 = 全試験が通過 / 1 = 失敗・前提の不足 / 2 = 誤用（引数あり・不正な channel）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

case "${FANDHE_BROWSER_CHANNEL:-}" in
  '' | chromium | chrome) ;;
  *)
    echo "error: FANDHE_BROWSER_CHANNEL must be chromium or chrome" >&2
    exit 2
    ;;
esac

for cmd in node cargo wasm-bindgen jq mktemp rm; do
  require_cmd "${cmd}"
done

if [[ ! -d "${ROOT_DIR}/node_modules/playwright" ]]; then
  echo "error: playwright is not installed; run npm ci && npx playwright install chromium" >&2
  exit 1
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-browser.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

log "building web into a temporary directory"
# 親環境の TEST_HOOKS を継承すると試験設定が生成されるため、空にして配信設定へ固定する。
TEST_HOOKS="" OUT_DIR="${TMP}/web" SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/build-web.sh" >/dev/null

log "building web (test hooks) into a temporary directory"
TEST_HOOKS=1 OUT_DIR="${TMP}/web-test" SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/build-web.sh" >/dev/null

FANDHE_WEB_DIR="${TMP}/web" FANDHE_WEB_TEST_DIR="${TMP}/web-test" node --test \
  "${ROOT_DIR}/tools/browser/harness.test.mjs" \
  "${ROOT_DIR}/tools/browser/safety.test.mjs" \
  "${ROOT_DIR}/tools/browser/safety-control.test.mjs" \
  "${ROOT_DIR}/tools/browser/smoke-hello.test.mjs" \
  "${ROOT_DIR}/tools/browser/keydown-action.test.mjs" \
  "${ROOT_DIR}/tools/browser/esc-attr.test.mjs" \
  "${ROOT_DIR}/tools/browser/input-limit.test.mjs" \
  "${ROOT_DIR}/tools/browser/paste.test.mjs" \
  "${ROOT_DIR}/tools/browser/paste-notice.test.mjs" \
  "${ROOT_DIR}/tools/browser/learner-text.test.mjs" \
  "${ROOT_DIR}/tools/browser/enter-submit.test.mjs" \
  "${ROOT_DIR}/tools/browser/key-invariants.test.mjs" \
  "${ROOT_DIR}/tools/browser/field-space.test.mjs" \
  "${ROOT_DIR}/tools/browser/submit-disabled.test.mjs" \
  "${ROOT_DIR}/tools/browser/submit-wiring.test.mjs" \
  "${ROOT_DIR}/tools/browser/tab-complete.test.mjs" \
  "${ROOT_DIR}/tools/browser/shift-tab.test.mjs" \
  "${ROOT_DIR}/tools/browser/tab-cycle.test.mjs" \
  "${ROOT_DIR}/tools/browser/esc-key.test.mjs" \
  "${ROOT_DIR}/tools/browser/json-depth.test.mjs" \
  "${ROOT_DIR}/tools/browser/json-load-order.test.mjs" \
  "${ROOT_DIR}/tools/browser/doc-links.test.mjs" \
  "${ROOT_DIR}/tools/browser/doc-tree.test.mjs" \
  "${ROOT_DIR}/tools/browser/tick-interval.test.mjs" \
  "${ROOT_DIR}/tools/browser/tick-dispatch.test.mjs" \
  "${ROOT_DIR}/tools/browser/three-views.test.mjs" \
  "${ROOT_DIR}/tools/browser/xss-cases.test.mjs" \
  "${ROOT_DIR}/tools/browser/log-lines-xss.test.mjs" \
  "${ROOT_DIR}/tools/browser/error-inputs.test.mjs" \
  "${ROOT_DIR}/tools/browser/reject-lines.test.mjs" \
  "${ROOT_DIR}/tools/browser/choice-answer.test.mjs" \
  "${ROOT_DIR}/tools/browser/approval-buttons.test.mjs" \
  "${ROOT_DIR}/tools/browser/approval-flow.test.mjs" \
  "${ROOT_DIR}/tools/browser/input-ports.test.mjs" \
  "${ROOT_DIR}/tools/browser/input-field-hidden.test.mjs" \
  "${ROOT_DIR}/tools/browser/ch0-3-run.test.mjs" \
  "${ROOT_DIR}/tools/browser/tree-attrs.test.mjs" \
  "${ROOT_DIR}/tools/browser/list-parent-attrs.test.mjs" \
  "${ROOT_DIR}/tools/browser/confirm-burst.test.mjs" \
  "${ROOT_DIR}/tools/browser/stop-move-ignore.test.mjs" \
  "${ROOT_DIR}/tools/browser/reload-ignore.test.mjs" \
  "${ROOT_DIR}/tools/browser/reload-notice.test.mjs" \
  "${ROOT_DIR}/tools/browser/restore-cancel.test.mjs" \
  "${ROOT_DIR}/tools/browser/candidates.test.mjs" \
  "${ROOT_DIR}/tools/browser/storage-select.test.mjs" \
  "${ROOT_DIR}/tools/browser/artifact-view-check.test.mjs" \
  "${ROOT_DIR}/tools/browser/artifact-view-controls.test.mjs" \
  "${ROOT_DIR}/tools/browser/artifact-mark-toggle.test.mjs" \
  "${ROOT_DIR}/tools/browser/file-tree-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/task-tree.test.mjs" \
  "${ROOT_DIR}/tools/browser/diff-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/doc-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/card-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/check-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/app-demo-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/point-rows.test.mjs" \
  "${ROOT_DIR}/tools/browser/point-click.test.mjs" \
  "${ROOT_DIR}/tools/browser/point-keys.test.mjs" \
  "${ROOT_DIR}/tools/browser/keyboard-confirm.test.mjs" \
  "${ROOT_DIR}/tools/browser/view-switch.test.mjs" \
  "${ROOT_DIR}/tools/browser/storage-leave.test.mjs" \
  "${ROOT_DIR}/tools/browser/idb-versionchange.test.mjs" \
  "${ROOT_DIR}/tools/browser/history-export.test.mjs" \
  "${ROOT_DIR}/tools/browser/history-import.test.mjs" \
  "${ROOT_DIR}/tools/browser/run-status.test.mjs" \
  "${ROOT_DIR}/tools/browser/step-status.test.mjs" \
  "${ROOT_DIR}/tools/browser/confirm-status.test.mjs" \
  "${ROOT_DIR}/tools/browser/stop-redo.test.mjs" \
  "${ROOT_DIR}/tools/browser/esc-outcomes.test.mjs" \
  "${ROOT_DIR}/tools/browser/ime.test.mjs" \
  "${ROOT_DIR}/tools/browser/ime-input.test.mjs" \
  "${ROOT_DIR}/tools/browser/history-versions.test.mjs" \
  "${ROOT_DIR}/tools/browser/reload-fresh.test.mjs" \
  "${ROOT_DIR}/tools/browser/reload-points.test.mjs"
