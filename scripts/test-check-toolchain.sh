#!/usr/bin/env bash
# scripts/check-toolchain.sh 自体の回帰テスト（`make test-check-toolchain` から呼ばれる）。
#
# 「固定ファイルの版を docs/toolchain.md とずらすと検査が失敗する」ことを、手作業ではなくここで自動で確かめる。
# 作業ツリーは変えない。固定ファイルと docs/toolchain.md を mktemp -d の一時ディレクトリへ写して改変し、
# check-toolchain.sh を --root で向ける。ネットワークは使わない。
#
# 出力は TAP 風（scripts/hooks/test-hooks.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd jq

CHECK="${DIR}/check-toolchain.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0
case_no=0
# 表のセルはバッククォートで囲まれている。シングルクォート内に書けないため変数に入れる。
BT='`'

# expect <name> <expected-rc> <actual-rc>
expect() {
  if [ "$2" = "$3" ]; then
    echo "ok - $1 (exit $3)"
    passed=$((passed + 1))
  else
    echo "not ok - $1 (expected exit $2, got $3)"
    failed=$((failed + 1))
  fi
}

# fresh_copy: 検査対象のファイルを一時ディレクトリへ写し、そのパスを出力する。
fresh_copy() {
  case_no=$((case_no + 1))
  local dest="${TMP_DIR}/case-${case_no}"
  mkdir -p "${dest}/docs"
  cp "${ROOT_DIR}/rust-toolchain.toml" "${ROOT_DIR}/Cargo.toml" "${ROOT_DIR}/Cargo.lock" \
    "${ROOT_DIR}/package.json" "${ROOT_DIR}/package-lock.json" "${ROOT_DIR}/.npmrc" "${dest}/"
  cp "${ROOT_DIR}/docs/toolchain.md" "${dest}/docs/toolchain.md"
  echo "${dest}"
}

# subst <file> <from> <to>: 固定文字列を置換する（sed -i の BSD/GNU 差を避けて一時ファイル経由にする）。
# 置換前の文字列が無ければテスト自体が壊れているので、その場で失敗させる。
subst() {
  if ! grep -qF -- "$2" "$1"; then
    echo "not ok - test setup: '$2' not found in $1"
    failed=$((failed + 1))
    return 1
  fi
  local content from="$2" to="$3"
  content="$(cat "$1")"
  printf '%s\n' "${content/"${from}"/${to}}" >"$1.new"
  mv "$1.new" "$1"
}

# run_check <dir>: 一時コピーを検査し、終了コードを RC に入れる（失敗しても止まらない）。
run_check() {
  RC=0
  "${CHECK}" --root "$1" >/dev/null 2>"${TMP_DIR}/stderr.txt" || RC=$?
}

# mutate_case <name> <expected-rc> <file> <from> <to>
mutate_case() {
  local dir
  dir="$(fresh_copy)"
  if subst "${dir}/$3" "$4" "$5"; then
    run_check "${dir}"
    expect "$1" "$2" "${RC}"
  fi
}

# 1. 実際のリポジトリは一致
RC=0
"${CHECK}" >/dev/null 2>&1 || RC=$?
expect "real repository passes" 0 "${RC}"

dir="$(fresh_copy)"
run_check "${dir}"
expect "unmodified copy passes" 0 "${RC}"

# 2. 固定ファイルをずらすと失敗する
mutate_case "rust channel mismatch is rejected" 1 rust-toolchain.toml 'channel = "1.98.1"' 'channel = "1.98.0"'
mutate_case "missing wasm32 target is rejected" 1 rust-toolchain.toml '"wasm32-unknown-unknown"' '"x86_64-unknown-linux-gnu"'
mutate_case "missing clippy component is rejected" 1 rust-toolchain.toml '"clippy"' '"miri"'
mutate_case "Cargo.toml wasm-bindgen range is rejected" 1 Cargo.toml 'wasm-bindgen = "=0.2.128"' 'wasm-bindgen = "0.2.128"'
mutate_case "Cargo.lock wasm-bindgen version drift is rejected" 1 Cargo.lock 'name = "wasm-bindgen"
version = "0.2.128"' 'name = "wasm-bindgen"
version = "0.2.129"'
mutate_case "package.json playwright range is rejected" 1 package.json '"playwright": "1.63.0"' '"playwright": "^1.63.0"'
mutate_case "package.json engines.node mismatch is rejected" 1 package.json '">=24 <25"' '">=22"'
mutate_case "package-lock playwright version drift is rejected" 1 package-lock.json '"node_modules/playwright": {
      "version": "1.63.0"' '"node_modules/playwright": {
      "version": "1.62.0"'
mutate_case "package-lock foreign registry is rejected" 1 package-lock.json 'https://registry.npmjs.org/playwright/' 'https://example.invalid/playwright/'
mutate_case ".npmrc without engine-strict is rejected" 1 .npmrc 'engine-strict=true' 'engine-strict=false'

# 3. 文書側を壊すと、検査不能として失敗する（fail-closed）
mutate_case "docs row with a different rust version is rejected" 1 docs/toolchain.md "| ${BT}rust${BT} | ${BT}1.98.1${BT} |" "| ${BT}rust${BT} | ${BT}1.98.2${BT} |"

dir="$(fresh_copy)"
rm "${dir}/Cargo.lock"
run_check "${dir}"
expect "missing Cargo.lock fails closed" 2 "${RC}"

dir="$(fresh_copy)"
rm "${dir}/.npmrc"
run_check "${dir}"
expect "missing .npmrc fails closed" 2 "${RC}"

dir="$(fresh_copy)"
rm "${dir}/docs/toolchain.md"
run_check "${dir}"
expect "missing docs/toolchain.md fails closed" 2 "${RC}"

dir="$(fresh_copy)"
grep -F "| ${BT}rust${BT} |" "${dir}/docs/toolchain.md" >"${dir}/dup-row.txt"
cat "${dir}/dup-row.txt" >>"${dir}/docs/toolchain.md"
run_check "${dir}"
expect "duplicated docs key fails closed" 2 "${RC}"

dir="$(fresh_copy)"
grep -vF "| ${BT}playwright${BT} |" "${dir}/docs/toolchain.md" >"${dir}/docs/toolchain.md.new"
mv "${dir}/docs/toolchain.md.new" "${dir}/docs/toolchain.md"
run_check "${dir}"
expect "missing docs key fails closed" 2 "${RC}"

dir="$(fresh_copy)"
printf '{ broken' >"${dir}/package.json"
run_check "${dir}"
expect "invalid package.json fails closed" 2 "${RC}"

# 4. 誤用
RC=0
"${CHECK}" --unknown >/dev/null 2>&1 || RC=$?
expect "unknown argument is a usage error" 2 "${RC}"

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
