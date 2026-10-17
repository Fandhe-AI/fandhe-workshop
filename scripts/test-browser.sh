#!/usr/bin/env bash
# ブラウザ試験（Playwright の共通ヘルパ）を流す（`make test-browser` から呼ばれる）。
#
# 配信設定の web（TEST_HOOKS なし）と試験の設定の web（TEST_HOOKS=1）を一時ディレクトリへビルドし、
# tools/browser/*.test.mjs を node:test で実行する。配信設定は FANDHE_WEB_DIR、試験の設定は FANDHE_WEB_TEST_DIR で渡す。
# 共通ヘルパの試験（harness・safety。安全性の計数の対照）と、実際の画面の試験（smoke-hello・keydown-action・esc-attr・input-limit・paste・paste-notice（上限の案内文と貼り付けの観察。TERM-15・TERM-19）・learner-text・enter-submit・submit-wiring・tab-complete・shift-tab・tab-cycle・esc-key・json-depth・json-load-order・doc-links・doc-tree・tick-interval・three-views・log-lines-xss・reject-lines・choice-answer（選択肢の誤答・選び直し・達成。TERM-24）・approval-buttons（承認のボタンと対象つきの直す。TERM-27）・tree-attrs・list-parent-attrs・confirm-burst・stop-move-ignore・esc-outcomes（Esc の画面の結果。TERM-7。処理中は候補が開いていても停止し候補も閉じる・処理中でなく候補が開いていれば閉じるだけ・どちらでもなければ取り消しの入力で、受け付けない Step では拒否の行と次の行動が出る。TERM-8）・reload-ignore・reload-notice・restore-cancel・candidates・history-export・history-import・artifact-view-check（左のビュー共通の検査器の自己確認。TASK-471）・ime（CDP による IME の模擬のヘルパの自己確認）・ime-input（入力欄と IME の層 3。BUILD-10。誤送信 0・送信漏れ 0。CDP の模擬）・history-versions（保存・復元・書き出し・版の層 3。BUILD-10。往復・読み込み後の保存と復帰・版の判定・壊れた履歴の拒否）。SAFE-9・SAFE-12・EVT-3・EVT-7・EVT-8・EVT-9・EVT-10・EVT-11・EVT-12・EVT-18・SAVE-6・FEND-15・TERM-1・TERM-2・TERM-21）、
# 実際の画面への対照の自己確認（safety-control。SAFE-8。CSP あり・なし）を含む。
# 前提: `npm ci` と `npx playwright install chromium` の済み。導入はここでは試みない。
# Chrome の原本で流すときは FANDHE_BROWSER_CHANNEL=chrome を付ける（既定は同梱 Chromium）。
# CI へは未組み込み（ジョブの追加は必須チェックの更新と合わせて扱う）。
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
  "${ROOT_DIR}/tools/browser/storage-leave.test.mjs" \
  "${ROOT_DIR}/tools/browser/history-export.test.mjs" \
  "${ROOT_DIR}/tools/browser/history-import.test.mjs" \
  "${ROOT_DIR}/tools/browser/run-status.test.mjs" \
  "${ROOT_DIR}/tools/browser/stop-redo.test.mjs" \
  "${ROOT_DIR}/tools/browser/esc-outcomes.test.mjs" \
  "${ROOT_DIR}/tools/browser/ime.test.mjs" \
  "${ROOT_DIR}/tools/browser/ime-input.test.mjs" \
  "${ROOT_DIR}/tools/browser/history-versions.test.mjs"
