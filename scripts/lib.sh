#!/usr/bin/env bash
# scripts/*.sh が共有するヘルパー。直接実行せず source して使う:
#   DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
#   source "${DIR}/lib.sh"
#
# 呼び出し元は `set -euo pipefail` を引き継ぐため、未定義変数・失敗した
# コマンド・パイプライン途中の失敗はその場でスクリプトを中断し、終了コードが
# そのまま呼び出し元（人・`make`・lefthook）へ伝播する。
set -euo pipefail

# 本ファイルの親ディレクトリ（= リポジトリルート）を cwd に依存せず解決する。
# `scripts/check.sh` を直接叩いても `make check` 経由でも同じ対象を扱うための前提。
repo_root() {
  local lib_dir
  lib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  (cd "${lib_dir}/.." && pwd)
}

ROOT_DIR="$(repo_root)"
# shellcheck disable=SC2034 # source した側のスクリプトで参照する
readonly ROOT_DIR

# require_cmd <name>: PATH 上に無ければ明確なメッセージで失敗する。
# 導入は一切試みない（導入手順の案内は doctor.sh / README の役割）。
require_cmd() {
  local cmd="$1"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "error: required command not found on PATH: ${cmd} (run \`make doctor\` for details)" >&2
    return 1
  fi
}

# reject_args <script-name> "$@": 引数を取らないスクリプトで未知の入力を
# 黙って無視せず、usage を出して終了コード 2（誤用）で止める。
reject_args() {
  local script_name="$1"
  shift
  if [ "$#" -ne 0 ]; then
    echo "usage: ${script_name} (no arguments)" >&2
    exit 2
  fi
}

log() {
  echo "[$(basename "$0")] $*" >&2
}

# toolchain_value <docs/toolchain.md のパス> <key>: 固定値の表（`| `key` | `値` | ...`）から値を 1 つ取り出す。
# 固定した版の正は docs/toolchain.md であり、check-toolchain.sh と doctor.sh が同じ読み取りを共有する。
# キーが 0 行・複数行・値が空のときは、何も出力せず非 0 を返す（fail-closed）。値は比較にだけ使い、実行しない。
toolchain_value() {
  local doc="$1" key="$2" out
  [ -r "${doc}" ] || return 1
  out="$(awk -F'|' -v key="${key}" '
    function trim(s) { gsub(/^[ \t]+|[ \t]+$/, "", s); return s }
    NF >= 4 && trim($2) == "`" key "`" {
      v = trim($3)
      gsub(/`/, "", v)
      print v
    }
  ' "${doc}")" || return 1
  [ -n "${out}" ] || return 1
  case "${out}" in *$'\n'*) return 1 ;; esac
  printf '%s\n' "${out}"
}
