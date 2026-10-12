#!/usr/bin/env bash
# ビルド・試験のワークフロー（既定は .github/workflows/ci.yml）が secret を参照していないことを検査する
# （`make check-workflow-secrets` から呼ばれる）。
#
# 理由: ci.yml は pull_request で動くため、secret を参照すると PR の起点から secret に届く経路になる。
# secret を扱ってよいのは、PR の起点を持たない配信側（deploy.yml）だけにする（TASK-33）。
# コメント行（先頭の空白に続く `#`）は数えない。次の参照を検出する: `secrets.`・`secrets[`・
# `secrets: inherit`・`github.token`。
#
# 使い方: scripts/check-workflow-secrets.sh [--file <path>]
#   --file は回帰テスト（scripts/test-check-workflow-secrets.sh）が一時ファイルを検査するために使う。
# 終了コード: 0 = 参照 0 件 / 1 = 参照あり / 2 = 誤用、または検査を完了できない（ファイルが無い・読めない）。
#   2 でも合格にしない（fail-closed）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

TARGET="${ROOT_DIR}/.github/workflows/ci.yml"
if [ "$#" -eq 2 ] && [ "$1" = "--file" ]; then
  TARGET="$2"
elif [ "$#" -ne 0 ]; then
  echo "usage: $(basename "$0") [--file <path>]" >&2
  exit 2
fi

require_cmd grep || exit 2

if [ ! -r "${TARGET}" ] || [ ! -f "${TARGET}" ]; then
  echo "error: cannot read ${TARGET}" >&2
  exit 2
fi

PATTERN='secrets\.|secrets\[|secrets:[[:space:]]*inherit|github\.token'

# コメント行を除いた行だけを対象にする。grep の終了コード 1（一致なし）は正常、2 以上は検査不能。
rc=0
hits="$(grep -nvE '^[[:space:]]*#' "${TARGET}" | grep -E "^[0-9]+:.*(${PATTERN})")" || rc=$?
if [ "${rc}" -gt 1 ]; then
  echo "error: grep failed on ${TARGET}" >&2
  exit 2
fi

if [ -n "${hits}" ]; then
  while IFS= read -r line; do
    echo "error: ${TARGET}:${line}" >&2
  done <<EOT
${hits}
EOT
  echo "workflow secret check failed: the build/test workflow must not reference secrets" >&2
  exit 1
fi
echo "workflow secret check passed (0 references in ${TARGET})"
