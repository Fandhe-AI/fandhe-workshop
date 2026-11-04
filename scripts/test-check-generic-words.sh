#!/usr/bin/env bash
# tools/check-generic-words.mjs（教材固有の語の検査）の回帰テストを流す（`make test-check-generic-words` から呼ばれる）。
#
# 試験は mkdtemp の一時ディレクトリに組み立てた小さな crates/ を使い、作業ツリーを変えない。実 repo に対しては読むだけである。
# CI の browser-test ジョブが check-generic-words とともに流す（#2034）。make check・lefthook には組み込まない（docs/testing.md 5.464）。
#
# 終了コード: 0 = 全試験が通過 / 1 = 失敗 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd node

node --test "${ROOT_DIR}/tools/check-generic-words.test.mjs"
