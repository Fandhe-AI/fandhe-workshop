#!/usr/bin/env bash
# CI 用: 変更ファイルから test-hooks ジョブの対象 OS（matrix）を決め、GITHUB_OUTPUT へ書く。
# macOS ランナーは起動待ちが長く課金も高いため、フック関連の変更が無い場合は ubuntu のみにする。
# fail-safe: 差分を取得できない・判定できない場合は macOS も含める（検査を減らす側へ倒さない）。
# 前提: actions/checkout を fetch-depth: 2 で実行済み（merge commit の HEAD^1 = base）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd git

cd "${ROOT_DIR}"

# macOS 検証が必要なパス: フックスクリプト・lib.sh・lefthook 設定・ci.yml・本スクリプト・Makefile
hook_paths='^(scripts/|lefthook\.yml$|Makefile$|\.github/workflows/ci\.yml$)'

both='["ubuntu-latest","macos-latest"]'
os="${both}"

changed_file="$(mktemp)"
trap 'rm -f "${changed_file}"' EXIT

if git diff --name-only --no-renames HEAD^1 HEAD > "${changed_file}" 2>/dev/null; then
  if [ -s "${changed_file}" ]; then
    # grep の終了状態: 0 = 一致 / 1 = 不一致 / 2 以上 = エラー（エラーは macOS 含む側へ倒す）
    rc=0
    grep -Eq "${hook_paths}" "${changed_file}" || rc=$?
    if [ "${rc}" -eq 1 ]; then
      os='["ubuntu-latest"]'
    fi
  fi
else
  log "warn: git diff failed; running on all OSes"
fi

log "test-hooks os=${os}"
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "test-hooks-os=${os}" >> "${GITHUB_OUTPUT}"
else
  echo "test-hooks-os=${os}"
fi
