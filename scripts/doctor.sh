#!/usr/bin/env bash
# 開発環境の診断のみを行う。導入・更新・修復は一切しない。
# 必須ツールが欠けていれば非 0 で終了し、次の行動は呼び出し元（人・`make doctor`）が決める。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

status=0

# check_tool <required|optional> <name> <version-command...>
check_tool() {
  local level="$1" name="$2"
  shift 2
  if command -v "${name}" >/dev/null 2>&1; then
    echo "ok   ${name}: $("$@" 2>&1 | head -n1)"
  elif [ "${level}" = "required" ]; then
    echo "miss ${name}: not found on PATH (required)"
    status=1
  else
    echo "skip ${name}: not found on PATH (optional)"
  fi
}

# rust-toolchain.toml があると、cargo・rustc などの呼び出しが rustup 経由で固定 toolchain の
# 自動導入を起動し得る。診断は何も導入しないため、Rust 系コマンドを呼ぶ前に全体で無効にする。
export RUSTUP_AUTO_INSTALL=0

echo "environment diagnosis for ${ROOT_DIR} (read-only, no changes made)"
check_tool required git git --version
check_tool required make make --version
check_tool required lefthook lefthook version
check_tool required editorconfig-checker editorconfig-checker --version
check_tool required shellcheck sh -c 'shellcheck --version | grep "^version:"'
check_tool required cargo cargo --version
check_tool required rustfmt rustfmt --version
check_tool required cargo-clippy cargo-clippy --version
check_tool required jq jq --version
check_tool optional direnv direnv version
check_tool optional npx npx --version

# 固定した版との照合（期待値は docs/toolchain.md の表。値の転記はしない）。
TOOLCHAIN_DOC="${ROOT_DIR}/docs/toolchain.md"
if ! want_rust="$(toolchain_value "${TOOLCHAIN_DOC}" rust)" ||
  ! want_target="$(toolchain_value "${TOOLCHAIN_DOC}" rust-target)" ||
  ! want_wb="$(toolchain_value "${TOOLCHAIN_DOC}" wasm-bindgen)" ||
  ! want_node="$(toolchain_value "${TOOLCHAIN_DOC}" node)"; then
  echo "miss docs/toolchain.md: cannot read the pinned versions table (required)"
  status=1
else
  have_rust="$(rustc --version 2>/dev/null | awk '{ print $2 }' || true)"
  if [ "${have_rust}" = "${want_rust}" ]; then
    echo "ok   rustc ${want_rust}: matches docs/toolchain.md"
  else
    echo "miss rustc: '${have_rust:-none}' active, docs/toolchain.md pins ${want_rust} (run \`rustup toolchain install\`)"
    status=1
  fi

  # rustup の有無に依らず、有効な toolchain の sysroot にターゲットの標準ライブラリがあるかで判定する
  # （Homebrew の rust など rustup が無い環境でも、make lint が要る wasm32 の欠落を検出するため）。
  sysroot="$(rustc --print sysroot 2>/dev/null || true)"
  if [ -n "${sysroot}" ] && [ -d "${sysroot}/lib/rustlib/${want_target}" ]; then
    echo "ok   ${want_target}: installed"
  else
    echo "miss ${want_target}: not installed (required; \`make lint\` needs it. run \`rustup toolchain install\`)"
    status=1
  fi

  if command -v wasm-bindgen >/dev/null 2>&1; then
    have_wb="$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }' || true)"
    if [ "${have_wb}" = "${want_wb}" ]; then
      echo "ok   wasm-bindgen ${want_wb}: matches docs/toolchain.md"
    else
      echo "warn wasm-bindgen: '${have_wb:-unknown}' installed, docs/toolchain.md pins ${want_wb} (optional until the web build)"
    fi
  else
    echo "skip wasm-bindgen: not found on PATH (optional until the web build)"
  fi

  if command -v node >/dev/null 2>&1; then
    have_node="$(node --version 2>/dev/null || true)"
    node_major="${have_node#v}"
    node_major="${node_major%%.*}"
    # node の範囲（>=24 <25）の下限の major だけを見て、24 系かを確かめる
    want_major="${want_node#>=}"
    want_major="${want_major%% *}"
    if [ "${node_major}" = "${want_major}" ]; then
      echo "ok   node ${have_node}: within ${want_node}"
    else
      echo "warn node: '${have_node:-unknown}' installed, docs/toolchain.md pins ${want_node} (optional; browser tests)"
    fi
  fi
fi

if [ -f "${ROOT_DIR}/.env" ]; then
  echo "ok   .env: present (values are not displayed)"
else
  echo "note .env: not found; \`make setup\` copies .env.example to .env"
fi

if [ "${status}" -ne 0 ]; then
  echo "one or more required tools are missing; see README.md \"必要なツール\" for install instructions" >&2
fi

exit "${status}"
