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
run_case "plain text mentioning secrets outside an expression passes" 0 $'# secrets.X is not used here\njobs: {}'
run_case "expression in a comment is detected (fail-closed)" 1 $'# ${{ secrets.X }}\njobs: {}'
run_case "secrets in a run block scalar is detected" 1 $'jobs:\n  a:\n    steps:\n      - run: |\n          # note\n          echo "${{ secrets.TOKEN }}"'
run_case "secrets with spaces in subscript is detected" 1 $'env:\n  T: ${{ secrets [ \'TOKEN\' ] }}'
run_case "secrets single-quoted subscript is detected" 1 $'env:\n  T: ${{ secrets[\'TOKEN\'] }}'
run_case "uppercase SECRETS is detected" 1 $'env:\n  T: ${{ SECRETS.TOKEN }}'
run_case "toJSON(secrets) is detected" 1 $'env:\n  T: ${{ toJSON(secrets) }}'
run_case "secrets in a multi-line expression is detected" 1 $'env:\n  T: ${{\n    secrets.TOKEN\n  }}'
run_case "github[token] subscript is detected" 1 $'env:\n  T: ${{ github[\'token\'] }}'
run_case "GITHUB.TOKEN with spaces is detected" 1 $'env:\n  T: ${{ GitHub . Token }}'
run_case "toJSON(github) is detected" 1 $'env:\n  T: ${{ toJSON(github) }}'
run_case "github.event_name passes" 0 $'if: ${{ github.event_name == \'push\' && github.ref == \'refs/heads/main\' }}'
run_case "github[event_name] passes" 0 $'if: ${{ github[\'event_name\'] == \'push\' }}'
run_case "secrets.X is detected" 1 $'env:\n  T: ${{ secrets.TOKEN }}'
run_case "secrets[...] is detected" 1 $'env:\n  T: ${{ secrets["TOKEN"] }}'
run_case "secrets: inherit is detected" 1 $'jobs:\n  a:\n    secrets: inherit'
run_case "github.token is detected" 1 $'env:\n  T: ${{ github.token }}'
run_case "secrets after '}}' in a string literal is detected" 1 $'env:\n  T: ${{ \'}}\' != \'\' && secrets.TOKEN || \'\' }}'
run_case "escaped quote then '}}' in a string literal is detected" 1 $'env:\n  T: ${{ \'it\'\'s }}\' != \'\' && secrets.TOKEN }}'
run_case "'}}' in a string literal alone passes" 0 $'env:\n  T: ${{ \'}}\' != \'\' }}'
run_case "bare if with github.token is detected" 1 $'jobs:\n  a:\n    if: github.token != \'\''
run_case "bare if with toJSON(github) is detected" 1 $'jobs:\n  a:\n    steps:\n      - if: toJSON(github)\n        run: x'
run_case "bare if with secrets is detected" 1 $'jobs:\n  a:\n    if: secrets.X != \'\''
run_case "bare if in a block scalar is detected" 1 $'jobs:\n  a:\n    if: >-\n      github.event_name == \'push\' &&\n      github.token != \'\'\n    steps: []'
run_case "bare if with github.event_name passes" 0 $'jobs:\n  a:\n    if: github.event_name == \'pull_request\'\n    steps: []'
run_case "bare if followed by a sibling mentioning secrets text passes" 0 $'jobs:\n  a:\n    if: always()\n    name: secrets rotation'

run_case "toJSON(github.*) is detected" 1 $'env:\n  T: ${{ toJSON(github.*) }}'
run_case "github.* with spaces is detected" 1 $'env:\n  T: ${{ toJSON(github . *) }}'
run_case "quoted secrets value inherit is detected" 1 $'jobs:\n  a:\n    secrets: \'inherit\''
run_case "quoted secrets key inherit is detected" 1 $'jobs:\n  a:\n    \'secrets\': inherit'
run_case "double-quoted secrets inherit is detected" 1 $'jobs:\n  a:\n    "secrets": "inherit"'
run_case "quoted if key with github.token is detected" 1 $'jobs:\n  a:\n    \'if\': github.token != \'\''
run_case "multi-line expression in bare if with secret after close is detected" 1 $'jobs:\n  a:\n    if: ${{ github.event_name == \'push\' &&\n      true }} && secrets.X != \'\'\n    steps: []'
run_case "quoted if followed by sibling passes" 0 $'jobs:\n  a:\n    \'if\': always()\n    name: secrets rotation'

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
