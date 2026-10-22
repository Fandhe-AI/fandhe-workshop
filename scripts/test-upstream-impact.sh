#!/usr/bin/env bash
# tools/upstream-impact.mjs（影響する Step の洗い出し）の回帰テストを流す（`make test-upstream-impact` から呼ばれる）。
#
# 試験は mkdtemp の一時ディレクトリだけを使い、通信せず、作業ツリーを変えない。
# CI・make check・lefthook には組み込まない（失敗したら止めるガードにする変更は人間の承認点。docs/testing.md）。
#
# 終了コード: 0 = 全試験が通過 / 1 = 失敗 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd node

node --test "${ROOT_DIR}/tools/upstream-impact.test.mjs"
