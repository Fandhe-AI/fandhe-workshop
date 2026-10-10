#!/usr/bin/env bash
# ローカル環境を作業可能な状態にする。何度再実行しても安全（冪等）。
# - lefthook install で Git hooks を有効化する
# - .env が無い場合のみ .env.example から作成する（既存の .env は上書きしない）
# ツール本体の導入は行わない（不足時は doctor.sh の結果に従って手動で導入する）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd lefthook

cd "${ROOT_DIR}"

log "lefthook install (enable Git hooks defined in lefthook.yml)"
lefthook install

if [ -e .env ]; then
  log ".env already exists; left untouched"
else
  cp .env.example .env
  log ".env created from .env.example; fill in the values locally"
fi

if command -v direnv >/dev/null 2>&1; then
  log "direnv detected: run \`direnv allow\` once to load .env via .envrc"
fi
