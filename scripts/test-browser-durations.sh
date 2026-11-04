#!/usr/bin/env bash
# tools/browser-durations.mjs（試験ファイルごとの所要時間の集計）の回帰テストを流す（`make test-browser-durations` から呼ばれる）。
#
# 試験は合成の junit XML を一時ディレクトリに書くだけで、ブラウザもビルドも使わない。
# CI の browser-test ジョブが check-generic-words の回帰テストとともに流す（#2037）。make check・lefthook には組み込まない。
#
# 終了コード: 0 = 全試験が通過 / 1 = 失敗 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd node

node --test "${ROOT_DIR}/tools/browser-durations.test.mjs"
