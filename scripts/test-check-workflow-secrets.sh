#!/usr/bin/env bash
# scripts/check-workflow-secrets.sh 自体の回帰テスト（`make test-check-workflow-secrets` から呼ばれる）。
#
# 「secret の参照を足すと検査が失敗する」ことを、一時ファイルを --file で検査して自動で確かめる。
# 作業ツリーは変えない。ネットワークは使わない。
#
# 出力は TAP 風（scripts/hooks/test-hooks.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

CHECK="${DIR}/check-workflow-secrets.sh"
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

# run_case <name> <expected-rc> <content>: content を一時ファイルに書いて検査する。
run_case() {
  local f="${TMP_DIR}/case.yml" rc=0
  printf '%s\n' "$3" > "${f}"
  "${CHECK}" --file "${f}" >/dev/null 2>&1 || rc=$?
  expect "$1" "$2" "${rc}"
}

echo "TAP version 13"
run_case "no reference passes" 0 $'jobs:\n  a:\n    steps:\n      - run: make test'
run_case "comment mentioning secrets. is ignored" 0 $'# secrets.X is not used here\njobs: {}'
run_case "secrets.X is detected" 1 $'env:\n  T: ${{ secrets.TOKEN }}'
run_case "secrets[...] is detected" 1 $'env:\n  T: ${{ secrets["TOKEN"] }}'
run_case "secrets: inherit is detected" 1 $'jobs:\n  a:\n    secrets: inherit'
run_case "github.token is detected" 1 $'env:\n  T: ${{ github.token }}'

rc=0
"${CHECK}" --file "${TMP_DIR}/missing.yml" >/dev/null 2>&1 || rc=$?
expect "missing file exits 2" 2 "${rc}"

rc=0
"${CHECK}" --bogus >/dev/null 2>&1 || rc=$?
expect "unknown argument exits 2" 2 "${rc}"

rc=0
"${CHECK}" --file >/dev/null 2>&1 || rc=$?
expect "--file without value exits 2" 2 "${rc}"

echo "1..$((passed + failed))"
echo "# pass ${passed} / fail ${failed}"
[ "${failed}" -eq 0 ]
