#!/usr/bin/env bash
# 巨大な文字列の IR を開いたときの renderer の RSS の最大値と crash の件数を、基点つきで測って JSON を出す（TASK-163 1/2。SAFE-25・D-45）。
#
# 使い方: tools/measure/ir-memory.sh（引数なし。make measure-ir-memory から呼ばれる）
#
# 配信する設定（TEST_HOOKS を空に固定。試験用の口なし）に min-3step.json を ir/main.json として同梱してビルドし、普通の IR・
# 1,990 万文字の pack.title・1,990 万文字のヒントの本文の 3 通りを開いて、renderer の RSS の最大値（KiB）を測る。巨大な値は
# string_too_long で断られるため、測る値は断った後（対策後）の値である。基準端末 A での実測と記録は #860、手順は docs/measurement.md §4c。
# 基点は commit・教材（min-3step.json）・端末の 3 つ。端末の基点はホスト名・ユーザー名・パスを含まない。
# `make measure` の束には入れない。結果の JSON は標準出力に 1 つだけ、進行状況とログは標準エラーへ出す。出力は lib.sh の measure_emit だけが行う。
# 前提: npm ci && npx playwright install chromium（Playwright は package-lock.json で固定済み）。
#
# 環境変数:
#   SKIP_WASM_OPT           1 なら wasm-opt を適用しない（build-web.sh へ渡す）
#   FANDHE_BROWSER_CHANNEL  chrome なら Chrome の原本で測る（基準端末 A での人の測定用。未設定は Playwright 同梱の Chromium）
#
# 終了コード: 0 = 成功（crash 0 件） / 1 = ビルド・測定・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用 /
#   3 = 測定は完了し検証にも通ったが crash が 1 件以上（JSON は出す）。
# 回帰テストは scripts/test-measure-ir-memory.sh。
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

IR="${ROOT_DIR}/testdata/engine-event-consistency/test-data/min-3step.json"

COMMIT_JSON="$(measure_commit_json)" || {
  echo "error: cannot determine the baseline commit" >&2
  exit 1
}
PACK_JSON="$(measure_pack_json "${IR}")" || {
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
TEST_HOOKS="" measure_browser_result ir-memory "${COMMIT_JSON}" "${TMP}" "${EXTRA_JSON}" "${IR}" || RC=$?
exit "${RC}"
