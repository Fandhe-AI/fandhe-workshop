#!/usr/bin/env bash
# scripts/check-sibling-free.sh 自体の回帰テスト（`make test-check-sibling-free` と CI の check から呼ばれる）。
# mktemp -d の下に parent/repo を作って判定する。作業ツリーは変えない。禁止語は実行時に組み立てる。
# 出力は TAP 風。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd mktemp
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT
CHECK="${DIR}/check-sibling-free.sh"
poc="03-""poc"
up=".."

passed=0
failed=0
# run_case <name> <expected-rc> <expected-stderr-substring or ""> <args...>
run_case() {
  local name="$1" want="$2" needle="$3" rc=0
  shift 3
  "${CHECK}" "$@" >/dev/null 2>"${TMP_DIR}/err.txt" || rc=$?
  if [ "${rc}" = "${want}" ] && { [ -z "${needle}" ] || grep -q -- "${needle}" "${TMP_DIR}/err.txt"; }; then
    echo "ok - ${name} (exit ${rc})"
    passed=$((passed + 1))
  else
    echo "not ok - ${name} (expected exit ${want}, got ${rc})"
    failed=$((failed + 1))
  fi
}
# fresh <name>: parent/repo を作ってその repo のパスを出す。
fresh() {
  mkdir -p "${TMP_DIR}/$1/repo/docs"
  echo "${TMP_DIR}/$1/repo"
}

r="$(fresh clean)"
run_case "clean" 0 "" --root "${r}"
r="$(fresh emptyspec)"
mkdir "${r}/docs/spec"
run_case "empty docs/spec" 0 "" --root "${r}"
r="$(fresh poc)"
mkdir "${r}/${up}/${poc}"
run_case "sibling 03-poc" 1 "sibling-present: ${up}/${poc}" --root "${r}"
r="$(fresh proto)"
mkdir "${r}/${up}/prototype"
run_case "sibling prototype" 1 "sibling-present: ${up}/prototype" --root "${r}"
r="$(fresh dangling)"
ln -s "${TMP_DIR}/nonexistent-target" "${r}/${up}/${poc}"
run_case "dangling symlink sibling" 1 "sibling-present" --root "${r}"
r="$(fresh spec)"
mkdir "${r}/docs/spec"
echo x >"${r}/docs/spec/a.md"
run_case "docs/spec with a file" 1 "spec-present" --root "${r}"
r="$(fresh unreadable)"
mkdir "${r}/docs/spec"
chmod 000 "${r}/docs/spec"
if [ -r "${r}/docs/spec" ]; then
  echo "ok - unreadable docs/spec (skipped: permissions not enforced)"
  passed=$((passed + 1))
else
  run_case "unreadable docs/spec" 1 "spec-unreadable" --root "${r}"
fi
chmod 755 "${r}/docs/spec"
run_case "unknown argument" 2 "usage" --bogus
run_case "missing root dir" 2 "not a directory" --root "${TMP_DIR}/nope"
run_case "root without value" 2 "usage" --root

echo "passed: ${passed}, failed: ${failed}"
[ "${failed}" -eq 0 ]
