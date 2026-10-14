#!/usr/bin/env bash
# tools/serve.mjs（サブパス配信サーバ）の試験を流す（`make test-serve` から呼ばれる）。
#
# 試験は一時ディレクトリの固定データだけを使い、dist/ や build-web の出力には依存しない。
# CI への組み込みはブラウザ試験の導入（TASK-28 2/2 以降）と合わせて扱う。
#
# 終了コード: 0 = 全試験が通過 / 1 = 失敗 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd node

node --test "${ROOT_DIR}/tools/serve.test.mjs"
