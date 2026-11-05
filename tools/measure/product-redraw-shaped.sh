#!/usr/bin/env bash
# 本体の画面（製品の ArtifactPart の view()）の左の成果物の再描画の時間を、描画部品の実際の形の行で測り直す（#2074。ART-26）。
#
# 使い方: tools/measure/product-redraw-shaped.sh（引数なし）
#
# 固定データ（artifact-redraw-shaped-<種類>-<件数>。差分の hunk・ファイルツリーの dir と file・入れ子のタスク・文書の本文・アプリのデモの 6 種の kind などの
# 属性つきの行を積み、7 種のビューを宣言した教材）で、選択のボタンの click 1 回の同期処理を測る。操作・件数・閾値（16ms）・判定は product-redraw.sh と同じで、
# 行の形だけが違う。そのため値は product-redraw の値と同じ条件ではなく、直接は比べない（docs/measurement.md。記録は docs/testing.md 5.472）。
# 形の検査（hunk・dir と file・入れ子・本文の区画・kind の混在）が外れたら失敗にする。16ms を超えても失敗にせず値を出す。
# 基点は commit・教材（min-3step.json。pack.id だけを変えて使う）・端末の 3 つ。
# `make measure` の束には入れない。結果の JSON は標準出力に 1 つだけ、進行状況とログは標準エラーへ出す。出力は lib.sh の measure_emit だけが行う。
# 前提: npm ci && npx playwright install chromium（Playwright は package-lock.json で固定済み）。
#
# 環境変数:
#   SKIP_WASM_OPT           1 なら wasm-opt を適用しない（build-web.sh へ渡す）
#   TEST_HOOKS              1 に固定する（呼び出し元の値は引き継がない）
#   FANDHE_BROWSER_CHANNEL  chrome なら Chrome の原本で測る（基準端末 A での人の測定用。未設定は Playwright 同梱の Chromium）
#
# 終了コード: 0 = 成功 / 1 = ビルド・測定・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用。
# 回帰テストは scripts/test-measure.sh の (h)。
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
TEST_HOOKS=1 measure_browser_result product-redraw-shaped "${COMMIT_JSON}" "${TMP}" "${EXTRA_JSON}" || RC=$?
exit "${RC}"
