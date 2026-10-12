#!/usr/bin/env bash
# Rust のソースを変えない検査（`make lint` から呼ばれる）。
#
# rustfmt の check、clippy（警告をエラー扱い）、web の wasm32 向け clippy（feature なしと test-hooks の 2 通り）を流す。
# wasm32-unknown-unknown のターゲットが無いと cargo が失敗する。黙って skip せず、そのまま失敗させる
# （ターゲットと版は rust-toolchain.toml で固定する。docs/toolchain.md）。
# 整形の自動修正は `make fmt`（ここでは何も書き換えない）。CI では ci.yml の rust-lint ジョブが呼ぶ（feature の網羅は TASK-80）。
#
# 終了コード: 0 = 全検査が通過 / 1 = いずれかが失敗 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo

cd "${ROOT_DIR}"

log "cargo fmt --check"
cargo fmt --all --check

log "cargo clippy (workspace, all targets)"
cargo clippy --workspace --all-targets --locked -- -D warnings

log "cargo clippy (web, wasm32)"
cargo clippy -p fandhe-workshop-web --target wasm32-unknown-unknown --locked -- -D warnings

log "cargo clippy (web, wasm32, test-hooks)"
cargo clippy -p fandhe-workshop-web --target wasm32-unknown-unknown --locked --features test-hooks -- -D warnings
