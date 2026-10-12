#!/usr/bin/env bash
# scripts/check-testdata-refs.sh 自体の回帰テスト（`make test-check-testdata` から呼ばれる）。
#
# 「repo の外・兄弟ディレクトリへの相対参照を足すと検査が失敗する」ことを、手作業ではなくここで確かめる。
# 作業ツリーは変えない。検査スクリプトと小さな testdata/ を mktemp -d の一時ディレクトリへ組み立てて改変する。
# 禁止語は実行時に組み立て、このファイル自体が検査（コード側の規則）に一致しないようにする。
#
# 出力は TAP 風（scripts/test-check-crate-deps.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

CHECK_NAME="check-testdata-refs.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

sibling="03-""poc"
dots="../../../../"

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

# fresh_tree <name>: 最小の repo 構成を作り、そのパスを出力する。
fresh_tree() {
  local dest="${TMP_DIR}/$1"
  mkdir -p "${dest}/scripts" "${dest}/testdata/poc-a/test-data/sub" "${dest}/crates/pack/tests"
  cp "${DIR}/lib.sh" "${DIR}/${CHECK_NAME}" "${dest}/scripts/"
  printf '{"ref": "../data.json"}\n' >"${dest}/testdata/poc-a/test-data/sub/a.json"
  printf 'fn main() {}\n' >"${dest}/crates/pack/tests/t.rs"
  echo "${dest}"
}

# run_check <tree>: 検査を実行し、終了コードを RC に入れる（失敗しても止まらない）。
run_check() {
  RC=0
  "$1/scripts/${CHECK_NAME}" >/dev/null 2>"${TMP_DIR}/stderr.txt" || RC=$?
}

# 1. 正常なデータ（repo の内側へ解決する参照と、無関係なパス遡りの試験値）は通る
tree="$(fresh_tree ok)"
printf 'payload ../../../XSS-54/etc/passwd and ../../XSS-IR3\n' >"${tree}/testdata/poc-a/test-data/sub/b.md"
run_check "${tree}"
expect "valid testdata passes" 0 "${RC}"

# 2. 兄弟の 03-poc を指す参照は失敗する
tree="$(fresh_tree sibling)"
printf '{"ref": "%s%s/x/y.json"}\n' "${dots}" "${sibling}" >"${tree}/testdata/poc-a/test-data/sub/c.json"
run_check "${tree}"
expect "sibling reference in testdata is rejected" 1 "${RC}"

# 3. repo のルートより上へ出る参照は失敗する
tree="$(fresh_tree escape)"
printf '{"ref": "../../../../../outside"}\n' >"${tree}/testdata/poc-a/test-data/sub/d.json"
run_check "${tree}"
expect "reference escaping the root is rejected" 1 "${RC}"

# 4. コード側の相対参照は失敗する
tree="$(fresh_tree code)"
printf 'const P: &str = "../../%s/x";\n' "${sibling}" >"${tree}/crates/pack/tests/t.rs"
run_check "${tree}"
expect "sibling reference in code is rejected" 1 "${RC}"

# 5. testdata/ が無いときは成功にしない
tree="$(fresh_tree missing)"
rm -rf "${tree}/testdata"
run_check "${tree}"
expect "missing testdata fails closed" 2 "${RC}"

# 6. 誤用
RC=0
"${DIR}/${CHECK_NAME}" --unknown >/dev/null 2>&1 || RC=$?
expect "unknown argument is a usage error" 2 "${RC}"

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
