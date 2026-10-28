#!/usr/bin/env bash
# scripts/check-license.sh 自体の回帰テスト（`make test-check-license` から呼ばれる）。
#
# 「license を崩す・本文のファイルを消す／取り違えると検査が失敗する」ことを、手作業ではなくここで確かめる。
# 作業ツリーは変えない。Cargo.toml・Cargo.lock・crates/・LICENSE-* を mktemp -d の一時ディレクトリへ写して改変する。
# 置換が効かなかったケースは not ok にする（偽の合格を防ぐ）。cargo の失敗・member 0 件は偽の cargo で再現する。
#
# 出力は TAP 風（scripts/test-check-crate-deps.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo
require_cmd jq
require_cmd mktemp
require_cmd rm

CHECK="${DIR}/check-license.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0

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

# fresh_copy: 作業ツリーの関連ファイルを一時ディレクトリへ写し、そのパスを出力する。
fresh_copy() {
  local dest
  dest="$(mktemp -d "${TMP_DIR}/case.XXXXXX")"
  cp "${ROOT_DIR}/Cargo.toml" "${ROOT_DIR}/Cargo.lock" "${ROOT_DIR}/LICENSE-MIT" "${ROOT_DIR}/LICENSE-APACHE" "${dest}/"
  cp -R "${ROOT_DIR}/crates" "${dest}/crates"
  echo "${dest}"
}

# run_check <dir>: 一時コピーを検査し、終了コードを RC に入れる（失敗しても止まらない）。
run_check() {
  RC=0
  "${CHECK}" --manifest-path "$1/Cargo.toml" >/dev/null 2>"${TMP_DIR}/stderr.txt" || RC=$?
}

# replace_case <name> <expected-rc> <file> <from-regex> <to>
#   file 内の <from-regex> に一致する行を <to> に置き換えて検査する。一致する行が無ければ not ok。
replace_case() {
  local name="$1" want="$2" file="$3" from="$4" to="$5" dir
  dir="$(fresh_copy)"
  if ! grep -q -- "${from}" "${dir}/${file}"; then
    echo "not ok - ${name} (mutation did not apply)"
    failed=$((failed + 1))
    return 0
  fi
  awk -v from="${from}" -v to="${to}" '$0 ~ from { if (to != "") print to; next } { print }' \
    "${dir}/${file}" >"${dir}/${file}.new"
  mv "${dir}/${file}.new" "${dir}/${file}"
  run_check "${dir}"
  expect "${name}" "${want}" "${RC}"
}

# 1. 実際のワークスペースと、改変しない一時コピーは違反 0 件
RC=0
"${CHECK}" >/dev/null 2>&1 || RC=$?
expect "real workspace passes" 0 "${RC}"
dir="$(fresh_copy)"
run_check "${dir}"
expect "unmodified copy passes" 0 "${RC}"

# 2. license の崩れ
replace_case "workspace license changed to MIT" 1 Cargo.toml '^license = "MIT OR Apache-2.0"$' 'license = "MIT"'
replace_case "workspace license case changed" 1 Cargo.toml '^license = "MIT OR Apache-2.0"$' 'license = "MIT or Apache-2.0"'
replace_case "one crate overrides license" 1 crates/pack/Cargo.toml '^license.workspace = true$' 'license = "Apache-2.0"'
replace_case "one crate drops license" 1 crates/pack/Cargo.toml '^license.workspace = true$' ''
replace_case "one crate uses license-file" 1 crates/pack/Cargo.toml '^license.workspace = true$' 'license-file = "LICENSE-MIT"'

# 3. 本文のファイル
dir="$(fresh_copy)"
rm "${dir}/LICENSE-MIT"
run_check "${dir}"
expect "LICENSE-MIT missing" 1 "${RC}"
dir="$(fresh_copy)"
rm "${dir}/LICENSE-APACHE"
run_check "${dir}"
expect "LICENSE-APACHE missing" 1 "${RC}"
dir="$(fresh_copy)"
: >"${dir}/LICENSE-MIT"
run_check "${dir}"
expect "LICENSE-MIT empty" 1 "${RC}"
dir="$(fresh_copy)"
cp "${ROOT_DIR}/LICENSE-MIT" "${dir}/LICENSE-APACHE"
run_check "${dir}"
expect "LICENSE-APACHE swapped with MIT text" 1 "${RC}"

# 4. 改変して失敗したコピーへ元のファイルを戻すと通る
dir="$(fresh_copy)"
rm "${dir}/LICENSE-MIT"
run_check "${dir}"
expect "mutate-and-revert: mutated copy is rejected" 1 "${RC}"
cp "${ROOT_DIR}/LICENSE-MIT" "${dir}/LICENSE-MIT"
run_check "${dir}"
expect "mutate-and-revert: restored copy passes" 0 "${RC}"

# 5. 偽の cargo（検査を完了できない場合は合格にしない）
FAKE_BIN="${TMP_DIR}/bin"
mkdir -p "${FAKE_BIN}"
cat >"${FAKE_BIN}/cargo" <<'EOF'
#!/usr/bin/env bash
case "${FAKE_MODE}" in
  fail) echo "error: boom" >&2; exit 101 ;;
  empty) echo '{"packages":[],"workspace_members":[],"workspace_root":"/nonexistent"}' ;;
esac
EOF
chmod 755 "${FAKE_BIN}/cargo"
RC=0
FAKE_MODE=fail PATH="${FAKE_BIN}:${PATH}" "${CHECK}" >/dev/null 2>&1 || RC=$?
expect "cargo failure fails closed" 2 "${RC}"
RC=0
FAKE_MODE=empty PATH="${FAKE_BIN}:${PATH}" "${CHECK}" >/dev/null 2>&1 || RC=$?
expect "zero members fails closed" 2 "${RC}"

# 6. 誤用
RC=0
"${CHECK}" --unknown >/dev/null 2>&1 || RC=$?
expect "unknown argument is a usage error" 2 "${RC}"

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
