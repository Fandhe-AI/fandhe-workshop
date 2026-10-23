#!/usr/bin/env bash
# ログが 120〜158 項目の状態での入力から描画までの時間を、試験の設定（feature test-hooks）の Chromium で測る（TASK-223 2/2。EVT-24）。
#
# 使い方: tools/measure/input-paint.sh（引数なし）
#
# 既定の最小教材（min-3step）で、送信ボタンの信頼された click（CDP）から次の rAF の中の setTimeout(0) までを測る。
# 値は最小教材のもので、教材全体を代表しない。試験の設定の値であり、配信する設定の値とは比べない（docs/measurement.md）。
# 基点は commit・教材・端末の 3 つ。教材は testdata の min-3step.json（試験の設定が埋め込む IR）から、端末は measure_device_json から集める。
# 100ms を超えても失敗にせず値を出す（合否は記録の文書に書く）。`make measure` の束には入れない。結果の JSON は標準出力に 1 つだけ、
# 進行状況とログは標準エラーへ出す。出力は lib.sh の measure_emit だけが行う。
# 前提: npm ci && npx playwright install chromium（Playwright は package-lock.json で固定済み）。
#
# 環境変数:
#   SKIP_WASM_OPT           1 なら wasm-opt を適用しない（build-web.sh へ渡す）
#   TEST_HOOKS              1 に固定する（呼び出し元の値は引き継がない）
#   FANDHE_BROWSER_CHANNEL  chrome なら Chrome の原本で測る（基準端末 A での人の測定用。未設定は Playwright 同梱の Chromium）
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
PACK_JSON="$(measure_pack_json "${ROOT_DIR}/testdata/engine-event-consistency/test-data/min-3step.json")" || {
  echo "error: cannot determine the baseline pack" >&2
  exit 1
}
DEVICE_JSON="$(measure_device_json)" || {
  echo "error: cannot determine the baseline device" >&2
  exit 1
}
EXTRA_JSON="$(jq -n --argjson pack "${PACK_JSON}" --argjson device "${DEVICE_JSON}" '{pack: $pack, device: $device}')"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-measure.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

RC=0
TEST_HOOKS=1 measure_browser_result input-paint "${COMMIT_JSON}" "${TMP}" "${EXTRA_JSON}" || RC=$?
exit "${RC}"
