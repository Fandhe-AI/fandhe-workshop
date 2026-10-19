#!/usr/bin/env bash
# tools/inventory-deps.mjs（依存の棚卸し）の回帰テストを流す（`make test-inventory-deps` から呼ばれる）。
#
# 試験は loopback の stub サーバと mkdtemp の一時ディレクトリを使い、実 crates.io には通信せず、作業ツリーを変えない。
# CI・make check・lefthook には組み込まない（失敗したら止めるガードにする変更は人間の承認点。docs/testing.md）。
#
# 終了コード: 0 = 全試験が通過 / 1 = 失敗 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd node

node --test "${ROOT_DIR}/tools/inventory-deps.test.mjs"
