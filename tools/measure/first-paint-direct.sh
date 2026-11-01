#!/usr/bin/env bash
# IR を同梱した配信する設定で、開始点の直接リンクを開いてから「操作できる」状態になるまでを、回線 10Mbps・遅延 40ms に絞った
# Chromium で 10 回測る（TASK-108 の道具。DLV-5）。`make measure` の束には入れない（束の様式は最小の画面の first-paint.sh を必須にしている）。
#
# 使い方: tools/measure/first-paint-direct.sh（引数なし）
#
# 20 Step の形の IR（crates/web/tests/data/ch0_3/ir.json）に、pack.id の付け替えと開始点 s-1-1 の追加だけを jq で加えた写しを作り、
# IR_FILE として配信する設定へ同梱して測る（tools/measure/browser.mjs --kind first-paint-direct）。値は手元のサブパス配信のもので、
# 公開サイトの値ではない（DLV-6）。基点は commit・教材（配った IR の SHA-256）・端末の区分で、端末は区分だけを入れる
# （機種名・CPU の型番・メモリの量は公開に向かないため入れない）。様式・定義・条件・記録の様式は docs/measurement.md。
# 結果の JSON は標準出力に 1 つだけ、進行状況とログは標準エラーへ出す。前提: npm ci && npx playwright install chromium。
#
# 環境変数:
#   SKIP_WASM_OPT           1 なら wasm-opt を適用しない（build-web.sh へ渡す）
#   TEST_HOOKS              空に固定する（配信する設定で測る。呼び出し元の値は引き継がない）
#   FANDHE_BROWSER_CHANNEL  chrome なら Chrome の原本で測る（基準端末 A での人の測定用。未設定は Playwright 同梱の Chromium）
#
# 終了コード: 0 = 3 秒以内 / 3 = 測れたが 3 秒を超えた（JSON は出す） / 1 = ビルド・測定・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用。
# 回帰テスト: scripts/test-measure-result.sh（ビルドなし）・scripts/test-measure.sh（実測）。
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

# IR の写し。保存先の鍵を他と分けるため pack.id を付け替え、開始点 s-1-1（CH-1 の最初の Step）を足す。OUT_DIR（${TMP}/web）の外に置く。
jq -c --arg id "measure-first-paint-direct" \
  '.pack.id = $id | .starts += [{id: "s-1-1", node: "main/st-1-1", effects: []}]' \
  "${ROOT_DIR}/crates/web/tests/data/ch0_3/ir.json" >"${TMP}/ir.json" || {
  echo "error: cannot prepare the IR copy" >&2
  exit 1
}
PACK_JSON="$(measure_pack_json "${TMP}/ir.json")" || {
  echo "error: cannot determine the baseline pack" >&2
  exit 1
}
DEVICE_JSON="$(measure_device_class_json)" || {
  echo "error: cannot determine the baseline device class" >&2
  exit 1
}
EXTRA_JSON="$(jq -n --argjson pack "${PACK_JSON}" --argjson device "${DEVICE_JSON}" '{pack: $pack, device: $device}')"

RC=0
TEST_HOOKS="" measure_browser_result first-paint-direct "${COMMIT_JSON}" "${TMP}" "${EXTRA_JSON}" "${TMP}/ir.json" >"${TMP}/out.json" || RC=$?
if [ "${RC}" -ne 0 ]; then exit "${RC}"; fi

VERDICT=0
measure_first_paint_direct_verdict "${TMP}/out.json" || VERDICT=$?
case "${VERDICT}" in
  0 | 3) ;;
  *) exit 1 ;;
esac
cat "${TMP}/out.json"
if [ "${VERDICT}" -eq 3 ]; then
  echo "error: operable time is over the 3 s threshold (median or max)" >&2
fi
exit "${VERDICT}"
