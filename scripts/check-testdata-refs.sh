#!/usr/bin/env bash
# 実装 repo が repo の外（兄弟の 03-poc/・prototype/ を含む）を相対参照していないことを検査する
# （`make check-testdata`・scripts/check.sh から呼ばれる）。BUILD-14 の「兄弟なしで試験できる」前提の番人。
#
# 字句一致（`03-poc` の grep）は、教材内の架空のパスやパス遡りの試験値を誤検出する。そのため
# testdata/ は、`../` で始まる各トークンをそのファイルの位置から解決して判定する。
#   (a) repo のルートより上へ出る
#   (b) 解決後の先頭の構成要素が 03-poc か prototype
# 宛先が存在するかは見ない（md の記録上のリンクが多数あり、試験のデータ参照ではないため）。
# コード側（crates/・scripts/・tools/）も同じ解決で判定する（(a) はディレクトリ名によらず拒否する）。
# 自分自身の照合に一致しないよう、禁止語は実行時に組み立てる。
#
# 失敗時は `error: <path>: <token>` を標準エラーへ出す（本文は出さない）。
# 終了コード: 0 = 違反なし / 1 = 違反あり / 2 = 検査を完了できない（fail-closed）または誤用。
# 回帰テストは scripts/test-check-testdata-refs.sh。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd grep
require_cmd find
export LC_ALL=C

cd "${ROOT_DIR}"

if [ ! -d testdata ]; then
  echo "error: testdata/ not found" >&2
  exit 2
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

violations=0
banned_a="03-""poc"
banned_b="proto""type"

# resolve_token <file-dir> <token>: ディレクトリ基準でトークンを解決し、違反なら理由を出力する。
# 違反が無ければ何も出力しない。
resolve_token() {
  local base="$1" token="$2" part
  local -a stack
  local n=0
  local -a base_parts=() token_parts=()
  # read -a は '/' で分割するだけでパス名展開をしない（`../*/..` の `*` が cwd のファイル名に化けて
  # 階層の深さが変わり、ルート脱出の判定をすり抜けるのを防ぐ）。
  IFS='/' read -r -a base_parts <<<"${base}"
  IFS='/' read -r -a token_parts <<<"${token}"
  for part in ${base_parts[@]+"${base_parts[@]}"}; do
    stack[n]="${part}"
    n=$((n + 1))
  done
  for part in ${token_parts[@]+"${token_parts[@]}"}; do
    case "${part}" in
      '' | .) ;;
      ..)
        if [ "${n}" -eq 0 ]; then
          echo "escapes the repository root"
          return 0
        fi
        n=$((n - 1))
        ;;
      *)
        stack[n]="${part}"
        n=$((n + 1))
        ;;
    esac
  done
  if [ "${n}" -gt 0 ] && { [ "${stack[0]}" = "${banned_a}" ] || [ "${stack[0]}" = "${banned_b}" ]; }; then
    echo "points to a sibling directory (${stack[0]})"
  fi
}

# scan_file <file>: ファイル中の `../` で始まるトークンを、そのファイルの位置から解決して判定する。
# GNU 専用の grep --null に頼らず、ファイルごとに grep する（BSD grep 互換）。違反は violations に加算する。
scan_file() {
  local file="$1" rc=0 token reason
  grep -IoE -e '\.\./[^[:space:]"'"'"'`)<>]*' "${file}" >"${TMP_DIR}/tokens.txt" || rc=$?
  if [ "${rc}" -ge 2 ]; then
    echo "error: grep failed while scanning ${file} (exit ${rc})" >&2
    exit 2
  fi
  while IFS= read -r token; do
    reason="$(resolve_token "$(dirname "${file}")" "${token}")"
    if [ -n "${reason}" ]; then
      echo "error: ${file}: ${token} (${reason})" >&2
      violations=$((violations + 1))
    fi
  done <"${TMP_DIR}/tokens.txt"
}

# 規則 1: testdata/ のテキストファイル。規則 2: コード側（crates/・scripts/・tools/。target/ を除く）。
# どちらも同じ解決で、repo のルート外・兄弟ディレクトリへの参照を拒否する（ディレクトリ名は問わない）。
scan_dirs=(testdata)
for d in crates scripts tools; do
  if [ -d "${d}" ]; then
    scan_dirs+=("${d}")
  fi
done
find "${scan_dirs[@]}" -name target -prune -o -type f -print0 >"${TMP_DIR}/files.bin"
while IFS= read -r -d '' file; do
  scan_file "${file}"
done <"${TMP_DIR}/files.bin"

if [ "${violations}" -ne 0 ]; then
  echo "error: ${violations} reference(s) outside the repository" >&2
  exit 1
fi
log "no references outside the repository"
