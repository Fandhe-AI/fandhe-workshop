#!/usr/bin/env bash
# 固定ファイルの版が docs/toolchain.md の表と一致することを検査する（`make check-toolchain` から呼ばれる）。
#
# 正は docs/toolchain.md の固定値の表。次を照合する。
#   - rust-toolchain.toml: channel・targets・components
#   - Cargo.toml の [workspace.dependencies] と Cargo.lock の wasm-bindgen（CLI との一致の前提）
#   - package.json・package-lock.json: Node の engines と Playwright（範囲記号なし）
#   - package-lock.json の取得元が npm の公式 registry で、integrity があること
#   - .npmrc の engine-strict
#   - README.md の「前提」節の表（docs/toolchain.md の表の写し）と、セットアップの wasm-bindgen CLI の導入行
# 手元に導入された CLI の版は見ない（それは doctor.sh の役割）。道具なしで CI でも流せる純粋なファイル間の検査にする。
#
# 使い方: scripts/check-toolchain.sh [--root <dir>]
#   --root は回帰テスト（scripts/test-check-toolchain.sh）が一時コピーを検査するために使う。
# 終了コード: 0 = 一致 / 1 = 不一致あり / 2 = 誤用、または検査を完了できない
#   （ファイルが無い・読めない、表のキーが 0 行か複数行、jq の失敗）。2 でも合格にしない（fail-closed）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

TARGET_ROOT="${ROOT_DIR}"
if [ "$#" -eq 2 ] && [ "$1" = "--root" ]; then
  TARGET_ROOT="$2"
elif [ "$#" -ne 0 ]; then
  echo "usage: $(basename "$0") [--root <dir>]" >&2
  exit 2
fi

require_cmd jq || exit 2

DOC="${TARGET_ROOT}/docs/toolchain.md"
README="${TARGET_ROOT}/README.md"
violations=0
SECTION=""
SECTION_DIR=""
trap '[ -z "${SECTION_DIR}" ] || rm -rf "${SECTION_DIR}"' EXIT

fail() {
  echo "error: $*" >&2
  violations=$((violations + 1))
}

# need_file <path>: 読めなければ検査不能として exit 2。
need_file() {
  if [ ! -r "$1" ]; then
    echo "error: cannot read $1" >&2
    exit 2
  fi
}

# doc_value <key>: 表から値を取る。取れなければ検査不能として exit 2。
doc_value() {
  local v
  if ! v="$(toolchain_value "${DOC}" "$1")"; then
    echo "error: docs/toolchain.md must have exactly one non-empty row for key: $1" >&2
    exit 2
  fi
  printf '%s\n' "${v}"
}

# jq_str <file> <filter>: 文字列の値を返す（無ければ空）。jq の失敗は検査不能として exit 2。
jq_str() {
  local out
  if ! out="$(jq -r "$2 // empty" "$1")"; then
    echo "error: cannot parse $1" >&2
    exit 2
  fi
  printf '%s' "${out}"
}

need_file "${DOC}"
need_file "${README}"
for f in rust-toolchain.toml Cargo.toml Cargo.lock package.json package-lock.json .npmrc; do
  need_file "${TARGET_ROOT}/${f}"
done

RUST="$(doc_value rust)"
RUST_TARGET="$(doc_value rust-target)"
WASM_BINDGEN="$(doc_value wasm-bindgen)"
NODE="$(doc_value node)"
PLAYWRIGHT="$(doc_value playwright)"

# README.md の「前提」節（見出し `## 前提` から次の `## ` の手前まで）。節の外の表（MCP の表など）と
# キーが衝突しないよう、節だけを一時ファイルに抜き出して読む。見出しがちょうど 1 つでなければ検査不能。
if [ "$(grep -cx '## 前提' "${README}")" -ne 1 ]; then
  echo "error: README.md must have exactly one '## 前提' heading" >&2
  exit 2
fi
SECTION_DIR="$(mktemp -d)"
SECTION="${SECTION_DIR}/prerequisites.md"
awk '/^## / { in_sec = ($0 == "## 前提"); next } in_sec { print }' "${README}" >"${SECTION}"

# readme_check <key> <docs の値>: README の表の値を docs/toolchain.md と照合する。行が 0 行・複数行・空なら exit 2。
readme_check() {
  local v
  if ! v="$(toolchain_value "${SECTION}" "$1")"; then
    echo "error: README.md 前提 must have exactly one non-empty row for key: $1" >&2
    exit 2
  fi
  [ "${v}" = "$2" ] || fail "README.md 前提 ${1} is '${v}', docs/toolchain.md says '$2'"
}
readme_check rust "${RUST}"
readme_check rust-target "${RUST_TARGET}"
readme_check wasm-bindgen "${WASM_BINDGEN}"
readme_check node "${NODE}"
readme_check playwright "${PLAYWRIGHT}"
if ! grep -qF "cargo install wasm-bindgen-cli --version ${WASM_BINDGEN} --locked" "${README}"; then
  fail "README.md must contain the line: cargo install wasm-bindgen-cli --version ${WASM_BINDGEN} --locked"
fi

# rust-toolchain.toml
RT="${TARGET_ROOT}/rust-toolchain.toml"
channel="$(awk -F'"' '/^channel[ \t]*=/ { print $2 }' "${RT}")"
if [ "${channel}" != "${RUST}" ]; then
  fail "rust-toolchain.toml channel is '${channel}', docs/toolchain.md says '${RUST}'"
fi
if ! grep -E '^targets[[:space:]]*=' "${RT}" | grep -qF "\"${RUST_TARGET}\""; then
  fail "rust-toolchain.toml targets does not include ${RUST_TARGET}"
fi
for comp in rustfmt clippy; do
  if ! grep -E '^components[[:space:]]*=' "${RT}" | grep -qF "\"${comp}\""; then
    fail "rust-toolchain.toml components does not include ${comp}"
  fi
done

# Cargo.toml の [workspace.dependencies]
manifest_wb="$(awk '
  /^\[/ { in_deps = ($0 == "[workspace.dependencies]") }
  in_deps && /^wasm-bindgen[ \t]*=/ { print }
' "${TARGET_ROOT}/Cargo.toml")"
if [ "${manifest_wb}" != "wasm-bindgen = \"=${WASM_BINDGEN}\"" ]; then
  fail "Cargo.toml [workspace.dependencies] wasm-bindgen must be exactly: wasm-bindgen = \"=${WASM_BINDGEN}\""
fi

# Cargo.lock の wasm-bindgen（ちょうど 1 つで、版が一致）
lock_versions="$(awk '
  /^name = "wasm-bindgen"$/ { want = 1; next }
  want && /^version = / { gsub(/"/, "", $3); print $3; want = 0 }
  /^$/ { want = 0 }
' "${TARGET_ROOT}/Cargo.lock")"
if [ -z "${lock_versions}" ]; then
  fail "Cargo.lock has no wasm-bindgen entry"
elif [ "${lock_versions}" != "${WASM_BINDGEN}" ]; then
  fail "Cargo.lock wasm-bindgen is '$(printf '%s' "${lock_versions}" | tr '\n' ' ')', docs/toolchain.md says '${WASM_BINDGEN}' (exactly one entry required)"
fi

# package.json
PKG="${TARGET_ROOT}/package.json"
LOCK="${TARGET_ROOT}/package-lock.json"
v="$(jq_str "${PKG}" '.engines.node')"
[ "${v}" = "${NODE}" ] || fail "package.json engines.node is '${v}', docs/toolchain.md says '${NODE}'"
v="$(jq_str "${PKG}" '.devDependencies.playwright')"
[ "${v}" = "${PLAYWRIGHT}" ] || fail "package.json devDependencies.playwright is '${v}', docs/toolchain.md says '${PLAYWRIGHT}'"

# package-lock.json
v="$(jq_str "${LOCK}" '.packages[""].devDependencies.playwright')"
[ "${v}" = "${PLAYWRIGHT}" ] || fail "package-lock.json root playwright is '${v}', docs/toolchain.md says '${PLAYWRIGHT}'"
for pkgname in playwright playwright-core; do
  v="$(jq_str "${LOCK}" ".packages[\"node_modules/${pkgname}\"].version")"
  [ "${v}" = "${PLAYWRIGHT}" ] || fail "package-lock.json node_modules/${pkgname} is '${v}', docs/toolchain.md says '${PLAYWRIGHT}'"
done

# 取得元と完全性（npm の公式 registry の URL と integrity が全パッケージにあること）
if ! bad="$(jq -r '.packages | to_entries[]
    | select(.key != "" and ((.value.link // false) | not))
    | select(((.value.resolved // "") | startswith("https://registry.npmjs.org/") | not) or ((.value.integrity // "") == ""))
    | .key' "${LOCK}")"; then
  echo "error: cannot parse package-lock.json" >&2
  exit 2
fi
if [ -n "${bad}" ]; then
  fail "package-lock.json packages without an npm registry URL or integrity: $(printf '%s' "${bad}" | tr '\n' ' ')"
fi

# .npmrc
if ! grep -qx 'engine-strict=true' "${TARGET_ROOT}/.npmrc"; then
  fail ".npmrc must contain engine-strict=true"
fi

if [ "${violations}" -ne 0 ]; then
  echo "toolchain check failed: ${violations} violation(s)" >&2
  exit 1
fi
echo "toolchain check passed (rust ${RUST}, wasm-bindgen ${WASM_BINDGEN}, node ${NODE}, playwright ${PLAYWRIGHT})"
