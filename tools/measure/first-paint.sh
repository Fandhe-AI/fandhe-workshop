#!/usr/bin/env bash
# 最小の画面（Hello）の初回表示の時間を、回線 10Mbps・遅延 40ms に絞った Chromium で測る（tools/measure/all.sh から呼ばれる）。
#
# 使い方: tools/measure/first-paint.sh（引数なし）
#
# 配信する設定を release でビルドし、tools/measure/browser.mjs --kind first-paint で測って、基点つきの結果の JSON を
# 標準出力に 1 つだけ出す。進行状況とログは標準エラーへ出す。様式・定義・条件は docs/measurement.md、出力は lib.sh の measure_emit だけが行う。
# 前提: npm ci && npx playwright install chromium（Playwright は package-lock.json で固定済み）。
#
# 環境変数:
#   SKIP_WASM_OPT  1 なら wasm-opt を適用しない（build-web.sh へ渡す）
#   TEST_HOOKS     空に固定する（初回表示は配信する設定で測る。呼び出し元の値は引き継がない）
#
# 終了コード: 0 = 成功 / 1 = ビルド・測定・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../../scripts/lib.sh
source "${DIR}/../../scripts/lib.sh"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd git
require_cmd jq
require_cmd node

COMMIT_JSON="$(measure_commit_json)" || {
  echo "error: cannot determine the baseline commit" >&2
  exit 1
}

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-measure.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

RC=0
TEST_HOOKS="" measure_browser_result first-paint "${COMMIT_JSON}" "${TMP}" || RC=$?
exit "${RC}"
