#!/usr/bin/env bash
# 世代と Step をまたいだ 2 周分の通し（6・48 グループ）で、行の総数 500・1,200・2,000 の再描画 1 回の時間を、試験の設定（feature test-hooks）の Chromium で測る（TASK-232 3/3。EVT-26）。
#
# 使い方: tools/measure/log-rounds.sh（引数なし）
#
# 既定の最小教材（min-3step）を go_to で st-1→st-2→st-3 と巡回し、(世代, Step) の別のグループを作った上で、最新のグループでの隠し要素の click 1 回の同期的な処理（イベントの委譲・状態の更新・DOM への反映）を測る。
# 値は合成の 2 周分のもので、本物の教材の行の構成を表さず、教材全体を代表しない。試験の設定の値であり、配信する設定の値とは比べない（docs/measurement.md）。
# 基点は commit・教材・端末の 3 つ。教材は testdata の min-3step.json（試験の設定が埋め込む IR）から、端末は measure_device_json から集める。
# 段階ごとの値を log_rounds_g<2 桁のグループ数>_<4 桁の行の数>_median|p95|max で出す。16ms を超えても失敗にせず値を出す（合否は記録の文書に書く）。`make measure` の束には入れない。結果の JSON は標準出力に 1 つだけ、
# 進行状況とログは標準エラーへ出す。出力は lib.sh の measure_emit だけが行う。同じ基点の log-slope.sh（1 グループ）と並べて読む。
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
TEST_HOOKS=1 measure_browser_result log-rounds "${COMMIT_JSON}" "${TMP}" "${EXTRA_JSON}" || RC=$?
exit "${RC}"
