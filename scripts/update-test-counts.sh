#!/usr/bin/env bash
# 試験の件数の期待値（scripts/expected-test-counts.txt）を、実際の試験の結果から書き直す（`make update-test-counts` から呼ばれる）。
#
# なぜ存在するか: 版上げ PR の受け入れ確認 (2)（FEND-17・D-63・TASK-101）は、repo に固定した期待値と件数を比べる。
# 期待値は版を上げる「前」に、別の PR でこのスクリプトで更新する（版上げ PR の中で書き換えると CI が失敗する）。
# make test（Rust の native 試験）と make test-browser（Playwright。npm ci と npx playwright install chromium が前提）を
# 実際に流すため遅い。どちらかが失敗・欠けた場合は、期待値を書き換えずに失敗する（fail-closed）。
#
# 終了コード: 0 = 更新 / 1 = 試験の失敗・前提の不足 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
for cmd in git date mktemp rm mv tee; do
  require_cmd "${cmd}"
done

cd "${ROOT_DIR}"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-update-counts.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

# 解析側が ANSI を取り除くため、色の有無は結果に影響しない。pipefail により試験の失敗はここで中断する。
log "running scripts/test.sh"
"${DIR}/test.sh" 2>&1 | tee "${TMP}/rust.log" >/dev/null
log "running scripts/test-browser.sh"
FANDHE_TEST_TAP_OUT="${TMP}/browser.tap" "${DIR}/test-browser.sh" >/dev/null

"${DIR}/check-frontend-bump.sh" normalize rust "${TMP}/rust.log" >"${TMP}/rust.norm"
"${DIR}/check-frontend-bump.sh" normalize browser "${TMP}/browser.tap" >"${TMP}/browser.norm"

commit="$(git rev-parse --short HEAD)"
today="$(date +%Y-%m-%d)"
{
  echo "# 試験の件数の期待値（FEND-17 の受け入れ確認 (2)・D-63・TASK-101）。更新日: ${today}・元にしたコミット: ${commit}"
  echo "# 更新は \`make update-test-counts\`（遅い。ブラウザ試験も流す）。版上げ PR の中では変えない（CI が失敗する）。版を上げる前に別の PR で更新する。"
  echo "# 書式: rust|<試験の束>|passed=<n>|ignored=<n>。同じ束の記述が crate をまたいで重複する場合は合算する（限界）。"
  cat "${TMP}/rust.norm" "${TMP}/browser.norm"
} >"${TMP}/expected.out"
mv "${TMP}/expected.out" "${DIR}/expected-test-counts.txt"
log "updated scripts/expected-test-counts.txt"
