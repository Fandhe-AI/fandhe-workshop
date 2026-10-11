#!/usr/bin/env bash
# CI 用: 変更ファイルから、フックの回帰テストを実行する OS を決め、GITHUB_OUTPUT へ書く
# （ci.yml の changes ジョブから呼ばれる）。
# 出力: test-hooks-os=<OS 名の JSON 配列文字列>（["ubuntu-latest"] か ["ubuntu-latest","macos-latest"]）。
# ci.yml の test-hooks-macos ジョブの実行可否（if:）と、ci-complete が test-hooks-macos の skipped を
# 許容するかの判定が、この出力を読む。キー名・形式は ci.yml との契約のため、変える場合は同時に直す。
# macOS ランナーは起動待ちが長く課金も高いため、フック関連の変更が無い場合は ubuntu のみにする。
# fail-safe: 差分を取得できない・判定できない場合は macOS も含める（検査を減らす側へ倒さない）。
# 前提: actions/checkout を fetch-depth: 2 で実行済み（pull_request では merge commit の HEAD^1 = base）。
# pull_request 以外（main への push 等）は複数コミットを含み得るため、常に macOS も含める。
# 入力: 環境変数 CI_EVENT_NAME（ci.yml が github.event_name を渡す）。未設定は pull_request 以外と同じ扱い。
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

if [ "${CI_EVENT_NAME:-}" != "pull_request" ]; then
  log "event=${CI_EVENT_NAME:-unknown}; running on all OSes"
elif git diff --name-only --no-renames HEAD^1 HEAD > "${changed_file}" 2>/dev/null; then
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
