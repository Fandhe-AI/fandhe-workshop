#!/usr/bin/env bash
# tools/update-l2.mjs（L2 の配布物の更新。TASK-116）の回帰テストを流す（`make test-update-l2` と CI の browser-test ジョブから呼ばれる）。
#
# 試験は合成の配信物・実行ファイルと mkdtemp の一時ディレクトリだけを使い、ビルド・通信・作業ツリーの変更を行わない。
# zip の中身の確認と、更新スクリプトの旧 zip の展開に unzip を使う。
# 終了コード: 0 = 全試験が通過 / 1 = 失敗 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd node
require_cmd unzip

node --test "${ROOT_DIR}/tools/update-l2.test.mjs"
