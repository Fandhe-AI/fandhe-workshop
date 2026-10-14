#!/usr/bin/env bash
# Rust の native 試験を流す（`make test` から呼ばれる）。
#
# `--workspace` を付けて、既定では除外している web の native 試験も含める（default-members は web を含まない）。
# wasm32 でのブラウザ試験はここに含まない（TASK-28）。CI では ci.yml の rust-test ジョブが呼ぶ。
#
# 終了コード: 0 = 全試験が通過 / 1 = 失敗 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo

cd "${ROOT_DIR}"
cargo test --locked --workspace
