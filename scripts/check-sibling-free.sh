#!/usr/bin/env bash
# 兄弟の 03-poc/・prototype/ と private の docs/spec が無い「まっさらな」配置であることを検査する
# （`make check-sibling-free`・CI の rust-test／browser-test から呼ばれる。BUILD-14・TASK-89）。
#
# 目的: 試験が兄弟の PoC に黙って依存していても、手元に兄弟があると通ってしまう。CI と clean-room では
# 兄弟が無いことを前提として明示し、前提が崩れた環境で合格を出さない。
# 手元で docs/spec を取得済みの開発者は 1 になる。これは期待どおりで、CI と clean-room の前提確認用である。
#
# 使い方: scripts/check-sibling-free.sh [--root <dir>]（既定は repo のルート。--root は回帰テスト用）
# 検査: (1) <root> の親ディレクトリに 03-poc と prototype が存在しない（シンボリックリンクを含む）
#       (2) <root>/docs/spec が存在しないか空
# 読むだけで、書き込み・削除はしない。
# 終了コード: 0 = 前提を満たす / 1 = 兄弟か spec の中身がある、または spec を列挙できない / 2 = 誤用。回帰テストは scripts/test-check-sibling-free.sh。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

ROOT="${ROOT_DIR}"
case "$#" in
  0) ;;
  2)
    if [ "$1" != "--root" ]; then
      echo "usage: check-sibling-free.sh [--root <dir>]" >&2
      exit 2
    fi
    ROOT="$2"
    ;;
  *)
    echo "usage: check-sibling-free.sh [--root <dir>]" >&2
    exit 2
    ;;
esac
if [ ! -d "${ROOT}" ]; then
  echo "error: --root is not a directory: ${ROOT}" >&2
  exit 2
fi
ROOT="$(cd "${ROOT}" && pwd -P)"
PARENT="$(dirname "${ROOT}")"
UP=".."

rc=0
for name in 03-poc prototype; do
  if [ -e "${PARENT}/${name}" ] || [ -L "${PARENT}/${name}" ]; then
    echo "error: sibling-present: ${UP}/${name}" >&2
    rc=1
  fi
done
if [ -e "${ROOT}/docs/spec" ] || [ -L "${ROOT}/docs/spec" ]; then
  if [ ! -d "${ROOT}/docs/spec" ]; then
    echo "error: spec-present: docs/spec" >&2
    rc=1
  else
    # 列挙の失敗を「空」と取り違えない（fail-closed）。$(...) を条件式に直接置くと終了状態が隠れる
    spec_list=""
    if ! spec_list="$(ls -A "${ROOT}/docs/spec")"; then
      echo "error: spec-unreadable: docs/spec" >&2
      exit 1
    fi
    if [ -n "${spec_list}" ]; then
      echo "error: spec-present: docs/spec" >&2
      rc=1
    fi
  fi
fi
if [ "${rc}" -eq 0 ]; then
  log "ok: no sibling 03-poc/prototype and no docs/spec content"
fi
exit "${rc}"
