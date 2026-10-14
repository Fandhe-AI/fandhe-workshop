#!/usr/bin/env bash
# ブラウザ試験（Playwright の共通ヘルパ）を流す（`make test-browser` から呼ばれる）。
#
# 配信設定の web を一時ディレクトリへビルドし（TEST_HOOKS なし）、tools/browser/*.test.mjs を node:test で実行する。
# 共通ヘルパの試験（harness・safety。安全性の計数の対照）と、実際の画面の試験（smoke-hello。CSP あり・なし）、
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

FANDHE_WEB_DIR="${TMP}/web" node --test \
  "${ROOT_DIR}/tools/browser/harness.test.mjs" \
  "${ROOT_DIR}/tools/browser/safety.test.mjs" \
  "${ROOT_DIR}/tools/browser/safety-control.test.mjs" \
  "${ROOT_DIR}/tools/browser/smoke-hello.test.mjs"
