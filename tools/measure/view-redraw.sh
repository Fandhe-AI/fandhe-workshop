#!/usr/bin/env bash
# 左の成果物ビューの部品単位の再描画の時間を、試験の設定（feature test-hooks）の Chromium で測る（TASK-485 1/3。ART-4）。
#
# 使い方: tools/measure/view-redraw.sh（引数なし）
#
# 代役の部品（view_fixture）の画面で、文書 200 行・差分 500 行・タスク 50 件（判定の対象）と、ファイルツリー 200 件・カード 20 件・
# チェック 20 件・アプリのデモ 9 行（参考）の、toggle と switch の 1 回の同期処理（viewFixtureDispatch）を測る。10 試行 × 20 操作 = 200 値。
# 本体の view() での測定（ART-26）と CPU 4 倍（ART-15）は別のイシュー。値は固定データの画面のもので、教材の値と比べない（docs/measurement.md）。
# 基点は commit・固定データ（出所と写しの SHA-256）・端末の 3 つ。16ms を超えても失敗にせず値を出す（合否は docs/testing.md に書く）。
# `make measure` の束には入れない。結果の JSON は標準出力に 1 つだけ、進行状況とログは標準エラーへ出す。出力は lib.sh の measure_emit だけが行う。
# 前提: npm ci && npx playwright install chromium（Playwright は package-lock.json で固定済み）。
#
# 環境変数:
#   SKIP_WASM_OPT           1 なら wasm-opt を適用しない（build-web.sh へ渡す）
#   TEST_HOOKS              1 に固定する（呼び出し元の値は引き継がない）
#   FANDHE_BROWSER_CHANNEL  chrome なら Chrome の原本で測る（基準端末 A での人の測定用。未設定は Playwright 同梱の Chromium）
#
# 終了コード: 0 = 成功 / 1 = ビルド・測定・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用。
# 回帰テストは scripts/test-measure.sh の (g)。
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
DATA_JSON="$(measure_data_json)" || {
  echo "error: cannot determine the baseline data" >&2
  exit 1
}
DEVICE_JSON="$(measure_device_json)" || {
  echo "error: cannot determine the baseline device" >&2
  exit 1
}
EXTRA_JSON="$(jq -n --argjson data "${DATA_JSON}" --argjson device "${DEVICE_JSON}" '{data: $data, device: $device}')"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-measure.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

RC=0
TEST_HOOKS=1 measure_browser_result view-redraw "${COMMIT_JSON}" "${TMP}" "${EXTRA_JSON}" || RC=$?
exit "${RC}"
