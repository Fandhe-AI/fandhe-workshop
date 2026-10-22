#!/usr/bin/env bash
# L2 の配布物を Rust のない模擬環境で展開・起動し、自動ブラウザで CH-0〜CH-1 を完走する試験を流す
# （`make test-l2-run` と CI の browser-test ジョブから呼ばれる。TASK-112 2/2。D-58・D-71・DLV-13）。
# あわせて、待ち受けが 127.0.0.1 だけで他のアドレスからの接続が拒否されること、起動とマウントの時間の記録を試験が行う
# （TASK-113。DLV-14。待ち受けの一覧は Linux では /proc/net/tcp*、macOS では /usr/sbin/lsof で読む）。
#
# 流れ: 配信する設定の web（release・同梱の IR あり）をビルド → tools/build-l2.mjs で zip を作る →
# tools/browser/l2-run.test.mjs が mkdtemp の下へ展開し、PATH=/usr/bin:/bin だけで ./start.sh を起動して 3 回通す。
# 遅い（web の release ビルドを専用の target で行う）ため、make test-browser・フックには入れない。
#
# 環境変数:
#   L2_SERVER  必須。miniserve の実行ファイル。sha256 は build-l2.mjs が tools/l2/miniserve.sha256 の固定値と照合する
#              （取得はここでは行わない。CI の step が取得して照合する）
# 専用の target にする理由: WASM の名前情報を strip = symbols で落とす必要があるため。tools/check-delivery.sh（build-l2.mjs が
# 呼ぶ）は WASM のバイト列に debug_ が無いことを求めるが、wasm-opt を使わない CI のビルドには名前（debug_struct 等）が残る。
# CARGO_PROFILE_RELEASE_STRIP は本スクリプトのビルドだけに効かせ、共有の target のキャッシュを壊さない。
#
# 終了コード: 0 = 3/3 通過 / 1 = 失敗・前提の不足 / 2 = 誤用（引数あり・L2_SERVER なし・未対応の OS）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

if [ -z "${L2_SERVER:-}" ]; then
  echo "error: L2_SERVER must point to the miniserve executable" >&2
  exit 2
fi

# 模擬環境の試験は、取得済みの miniserve と同じ OS・CPU の配布物だけを作る。
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) PLATFORM="linux-x64" ;;
  Darwin-arm64) PLATFORM="macos-arm64" ;;
  *)
    echo "error: unsupported platform: $(uname -s)-$(uname -m)" >&2
    exit 2
    ;;
esac

for cmd in node unzip cargo wasm-bindgen jq mktemp rm; do
  require_cmd "${cmd}"
done

if [[ ! -d "${ROOT_DIR}/node_modules/playwright" ]]; then
  echo "error: playwright is not installed; run npm ci && npx playwright install chromium" >&2
  exit 1
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-l2-run.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

IR="${ROOT_DIR}/crates/web/tests/data/ch0_3/ir.json"

log "building web (delivery, release, bundled IR) into a temporary directory"
CARGO_PROFILE_RELEASE_STRIP=symbols CARGO_TARGET_DIR="${TMP}/cargo-target" \
  TEST_HOOKS="" SKIP_WASM_OPT=1 OUT_DIR="${TMP}/site" IR_FILE="${IR}" \
  "${ROOT_DIR}/tools/build-web.sh" release >/dev/null

log "building the L2 zip (${PLATFORM})"
mkdir -p "${TMP}/out"
L2_SITE="${TMP}/site" L2_OUT="${TMP}/out" L2_PLATFORM="${PLATFORM}" node "${ROOT_DIR}/tools/build-l2.mjs" >/dev/null

log "running the mock-environment run"
FANDHE_L2_ZIP="${TMP}/out/fandhe-workshop-l2-${PLATFORM}.zip" FANDHE_L2_IR="${IR}" \
  node --test "${ROOT_DIR}/tools/browser/l2-run.test.mjs"
