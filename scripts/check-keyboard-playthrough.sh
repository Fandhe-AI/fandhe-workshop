#!/usr/bin/env bash
# 教材パックの通し 1 本を、配信する設定の画面でキーボードだけで最後まで流す（`make check-keyboard-playthrough` から呼ばれる。TASK-92 2/2。BUILD-17）。
#
# 目的: 教材 repo の教材（通し pt-ch0-15 など）を、エンジン側の道具（tools/browser/keyboard-playthrough-run.mjs）でそのまま流す入口。
# Tab・Enter・Space・Shift+Enter・Esc と文字の入力だけで通しが完走し、マウスの click が 0 回であることを確かめる。
# 教材固有の知識は持たない。通しの ID は引数で受け、既定値を持たない。遅い（ビルドとブラウザが要る）ので PR の CI・make test-browser には入れない。
#
# 処理の順
#   1. 引数・前提コマンドの確認（誤用は終了コード 2。ビルドや起動の前に止める）
#   2. FANDHE_PACK_BIN が無ければ scripts/build-pack-bin.sh で fandhe-pack を作る
#   3. validate が診断 0 件であることを確かめる
#   4. 一時ディレクトリへ compile し、IR 同梱の配信する設定（TEST_HOOKS なし）をビルドする
#   5. crates/pack/tests/playthrough_emit.rs の emit_playthrough_for_browser で通しを JSON にする
#   6. node tools/browser/keyboard-playthrough-run.mjs を呼ぶ
#
# 使い方: scripts/check-keyboard-playthrough.sh <pack-dir> <playthrough-id>
# 環境変数: FANDHE_PACK_BIN（既存のものを使う）・FANDHE_BROWSER_CHANNEL（chromium | chrome）・SKIP_WASM_OPT（ビルドに渡る）
# 終了コード: 0 = 合格 / 1 = 不合格・ビルドの失敗 / 2 = 誤用・道具が無い
# 回帰テスト: scripts/test-check-keyboard-playthrough.sh
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

usage() {
  echo "usage: $(basename "$0") <pack-dir> <playthrough-id>" >&2
  exit 2
}

[ "$#" -eq 2 ] || usage
PACK="$1"
ID="$2"
if [ ! -d "${PACK}" ]; then
  echo "error: pack directory not found: ${PACK}" >&2
  exit 2
fi
# ID は教材テストの安全な識別子の形だけ（環境変数と JSON を通るため）。
if ! printf '%s' "${ID}" | grep -Eq '^[A-Za-z0-9._-]{1,64}$'; then
  echo "error: playthrough id must match [A-Za-z0-9._-]{1,64}" >&2
  exit 2
fi
case "${FANDHE_BROWSER_CHANNEL:-}" in
  '' | chromium | chrome) ;;
  *)
    echo "error: FANDHE_BROWSER_CHANNEL must be chromium or chrome" >&2
    exit 2
    ;;
esac

for cmd in node cargo wasm-bindgen jq mktemp; do
  require_cmd "${cmd}" || exit 2
done

# 一時ディレクトリを作る前は空。空のまま終わった場合は何も消さない。
TMP=""
trap '[ -z "${TMP}" ] || rm -rf "${TMP}"' EXIT
TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-keyboard-playthrough.XXXXXX")"

if [ -z "${FANDHE_PACK_BIN:-}" ]; then
  log "building fandhe-pack (debug)"
  FANDHE_PACK_BIN="$("${DIR}/build-pack-bin.sh")"
fi

log "validating the pack"
"${FANDHE_PACK_BIN}" validate "${PACK}" >/dev/null || {
  echo "error: validate reported diagnostics" >&2
  exit 1
}

log "compiling and building the delivery web (IR_FILE, no test hooks)"
"${FANDHE_PACK_BIN}" compile "${PACK}" -o "${TMP}/ir.json" >/dev/null
TEST_HOOKS="" OUT_DIR="${TMP}/web" IR_FILE="${TMP}/ir.json" SKIP_WASM_OPT="${SKIP_WASM_OPT:-}" \
  "${ROOT_DIR}/tools/build-web.sh" >/dev/null

log "writing the playthrough as JSON"
PACK_ABS="$(cd "${PACK}" && pwd)"
FANDHE_PLAYTHROUGH_PACK="${PACK_ABS}" FANDHE_PLAYTHROUGH_ID="${ID}" FANDHE_PLAYTHROUGH_OUT="${TMP}/pt" \
  cargo test --locked -q -p fandhe-workshop-pack --test playthrough_emit -- --ignored --exact emit_playthrough_for_browser >/dev/null
[ -s "${TMP}/pt/playthrough.json" ] || {
  echo "error: playthrough.json is missing or empty" >&2
  exit 1
}

rc=0
FANDHE_WEB_PACK_DIR="${TMP}/web" FANDHE_PACK_IR="${TMP}/ir.json" FANDHE_PLAYTHROUGH_JSON="${TMP}/pt/playthrough.json" \
  node "${ROOT_DIR}/tools/browser/keyboard-playthrough-run.mjs" || rc=$?
exit "${rc}"
