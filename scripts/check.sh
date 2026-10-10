#!/usr/bin/env bash
# 非破壊の静的チェック。ソース・設定は変更しない。
# - editorconfig-checker: .editorconfig の宣言（改行・インデント・末尾空白等）への準拠
# - shellcheck: scripts/ 配下のシェルスクリプト
# ツールが無い場合は「未検査で成功」にせず失敗させる。
# 言語固有のチェック（fmt --check・lint・test 等）は派生リポジトリで追記する。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd editorconfig-checker
require_cmd shellcheck

cd "${ROOT_DIR}"

log "editorconfig-checker (exclusions: .editorconfig-checker.json)"
editorconfig-checker

log "shellcheck scripts/**/*.sh"
find scripts -type f -name '*.sh' -print0 | xargs -0 shellcheck
