#!/usr/bin/env bash
# tools/check-internal-info.mjs（社内固有の情報・認証情報の照合。TASK-139）の回帰テストを流す（`make test-check-internal-info` から呼ばれる）。
#
# 試験は mkdtemp の一時 repo に組み立てた小さな repo を使い、作業ツリーを変えない。認証情報の形の照合は
# crates/pack/examples/secret_scan_files.rs を呼ぶため、先にビルドして試験を速くする。
# CI・make check・lefthook には組み込まない（docs/testing.md）。
#
# 終了コード: 0 = 全試験が通過 / 1 = 失敗 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd node
require_cmd cargo

cargo build --locked --quiet -p fandhe-workshop-pack --example secret_scan_files
node --test "${ROOT_DIR}/tools/check-internal-info.test.mjs"
