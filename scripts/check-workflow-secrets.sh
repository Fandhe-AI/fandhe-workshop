#!/usr/bin/env bash
# ビルド・試験のワークフロー（既定は .github/workflows/ci.yml）が secret を参照していないことを検査する
# （`make check-workflow-secrets` から呼ばれる）。
#
# 理由: ci.yml は pull_request で動くため、secret を参照すると PR の起点から secret に届く経路になる。
# secret を扱ってよいのは、PR の起点を持たない配信側（deploy.yml）だけにする（TASK-33）。
#
# 検出対象は、Actions 式 `${{ ... }}` の中（複数行にまたがる式を含む）にある次の参照と、`secrets: inherit`。
#   - `secrets` コンテキスト（`secrets.X`・`secrets['X']`・`secrets . X`・`toJSON(secrets)`。大文字小文字・空白を問わない）
#   - `github.token`・`github['token']`（空白・大文字小文字を問わない）と、`toJSON(github)` のような github 全体の参照
# コメント行も除外しない。YAML のコメントと、run のブロックスカラー（文字列。中の式は評価される）を
# 行頭の `#` だけでは区別できないため、fail-closed で全行を対象にする。式の外の文章（`secrets を参照しない` 等）は対象外。
#
# 使い方: scripts/check-workflow-secrets.sh [--file <path>]
#   --file は回帰テスト（scripts/test-check-workflow-secrets.sh）が一時ファイルを検査するために使う。
# 終了コード: 0 = 参照 0 件 / 1 = 参照あり / 2 = 誤用、または検査を完了できない（ファイルが無い・読めない・awk の失敗）。
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

require_cmd awk || exit 2

if [ ! -r "${TARGET}" ] || [ ! -f "${TARGET}" ]; then
  echo "error: cannot read ${TARGET}" >&2
  exit 2
fi

# awk で式を取り出して判定する。式の開始行番号と式の本文（複数行は空白で連結）を `行:本文` で出す。
# 引用符は八進エスケープ（\047 = ' / \042 = "）で書き、シェルのクォートと衝突させない。
# shellcheck disable=SC2016 # awk のプログラムなので、シェルに展開させない
AWK_PROG='
function check(body, start,    lb) {
  lb = tolower(body)
  if (lb ~ /(^|[^a-z0-9_-])secrets([^a-z0-9_-]|$)/ ||
      lb ~ /(^|[^a-z0-9_-])github[ \t]*(\.[ \t]*token|\[[ \t]*[\047\042]token)/ ||
      lb ~ /(^|[^a-z0-9_-])github[ \t]*([),]|$)/) {
    print start ":" body
  }
}
{
  line = $0
  if (tolower(line) ~ /secrets[ \t]*:[ \t]*inherit/) {
    print NR ":" line
  }
  while (length(line) > 0) {
    if (!inexpr) {
      p = index(line, "${{")
      if (p == 0) break
      line = substr(line, p + 3)
      inexpr = 1
      buf = ""
      start = NR
    } else {
      p = index(line, "}}")
      if (p == 0) { buf = buf line " "; break }
      buf = buf substr(line, 1, p - 1)
      line = substr(line, p + 2)
      inexpr = 0
      check(buf, start)
    }
  }
}
END {
  if (inexpr) check(buf, start)
}
'

rc=0
hits="$(awk "${AWK_PROG}" "${TARGET}")" || rc=$?
if [ "${rc}" -ne 0 ]; then
  echo "error: awk failed on ${TARGET}" >&2
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
