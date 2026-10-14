#!/usr/bin/env bash
# action-keydown の対照の検査（make test-keydown-control から呼ばれる。TASK-346 / FEND-1）。
#
# feature action-keydown を外した写しをビルドし、tools/browser/keydown-action.test.mjs が
# 「キーが届かない」理由（固定の印 ACTION-KEYDOWN-NOT-DISPATCHED）で失敗することを確かめる。
# これで、試験が空振りでなく、書き忘れを実際に検出できると言える。
#
# 方式は check-mutations.sh と同じで、作業ツリーは書き換えない。mktemp -d の中に必要なファイルを写し、
# 写した Cargo.toml の 1 行を文字どおりの一致（awk の index）で置き換える。一致が 1 回でなければ止める。
# 前提は test-browser.sh と同じ（npm ci・Playwright の Chromium・wasm-bindgen CLI）。
# 依存を一時ディレクトリで冷えた状態からビルドするため遅い。make check・フック・CI には入れない。
#
# 終了コード: 0 = 対照が成立（正しい理由で失敗した） / 1 = 対照が不成立（feature を外しても試験が通った） /
#             2 = 誤用・前提の不足・別の理由での失敗（合格ではない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

for cmd in node cargo wasm-bindgen jq mktemp awk grep cp rm mv; do
  require_cmd "${cmd}" || exit 2
done
if [[ ! -d "${ROOT_DIR}/node_modules/playwright" ]]; then
  echo "error: playwright is not installed; run npm ci && npx playwright install chromium" >&2
  exit 2
fi

die() {
  echo "error: $*" >&2
  exit 2
}

FIND='features = ["action-keydown"] }'
REPLACE='features = [] }'

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-keydown-control.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT
COPY="${TMP}/copy"
mkdir -p "${COPY}/tools" "${COPY}/scripts"

for item in Cargo.toml Cargo.lock rust-toolchain.toml crates; do
  [ -e "${ROOT_DIR}/${item}" ] || die "missing in repository root: ${item}"
  [ ! -L "${ROOT_DIR}/${item}" ] || die "refusing to copy a symlink: ${item}"
  cp -R "${ROOT_DIR}/${item}" "${COPY}/${item}"
done
cp "${ROOT_DIR}/tools/build-web.sh" "${COPY}/tools/build-web.sh"
cp "${ROOT_DIR}/scripts/lib.sh" "${COPY}/scripts/lib.sh"

# 置換の前に、対象がちょうど 1 回現れることを確かめる（0 回・2 回以上は定義の不整合）。
occurrences="$(FIND="${FIND}" awk '
  BEGIN { find = ENVIRON["FIND"]; n = length(find); total = 0 }
  {
    line = $0
    while ((i = index(line, find)) > 0) { total++; line = substr(line, i + n) }
  }
  END { print total }
' "${COPY}/Cargo.toml")"
[ "${occurrences}" = "1" ] || die "the line to replace must occur exactly once in Cargo.toml (found ${occurrences})"

FIND="${FIND}" REPLACE="${REPLACE}" awk '
  BEGIN { find = ENVIRON["FIND"]; repl = ENVIRON["REPLACE"]; n = length(find); done = 0 }
  {
    if (!done && (i = index($0, find)) > 0) {
      print substr($0, 1, i - 1) repl substr($0, i + n)
      done = 1
    } else {
      print $0
    }
  }
' "${COPY}/Cargo.toml" >"${COPY}/Cargo.toml.new"
mv "${COPY}/Cargo.toml.new" "${COPY}/Cargo.toml"

# 健全性の確認: 解決後の feature から action-keydown が消えていること。消えていなければ対照が空振りしている。
log "checking that the copy resolves without action-keydown"
# パッケージが持つ feature の一覧ではなく、解決後に有効な feature（resolve.nodes）を見る。
enabled="$(cd "${COPY}" && cargo metadata --locked --format-version 1 |
  jq -r '[.resolve.nodes[] | select(.id | test("fandhe-frontend-wasm-full"))] as $n
    | if ($n | length) != 1 then "ERR" else ($n[0].features | map(select(. == "action-keydown")) | length | tostring) end')" ||
  die "cargo metadata failed in the copy"
[ "${enabled}" != "ERR" ] || die "fandhe-frontend-wasm-full was not found exactly once in the copy"
[ "${enabled}" = "0" ] || die "action-keydown is still enabled in the copy (control would be vacuous): ${enabled}"

log "building the copy with test hooks (cold dependencies; this is slow)"
(
  cd "${COPY}"
  CARGO_TARGET_DIR="${TMP}/target" TEST_HOOKS=1 SKIP_WASM_OPT=1 OUT_DIR="${COPY}/dist/web-test" \
    "${COPY}/tools/build-web.sh" >/dev/null
) || die "the build of the copy failed"

OUT="${TMP}/out.txt"
rc=0
FANDHE_WEB_TEST_DIR="${COPY}/dist/web-test" node --test \
  "${ROOT_DIR}/tools/browser/keydown-action.test.mjs" >"${OUT}" 2>&1 || rc=$?

if [ "${rc}" -eq 0 ]; then
  tail -n 30 "${OUT}" >&2
  echo "error: control not established: the test passed without action-keydown" >&2
  exit 1
fi
if grep -Fq 'mount failed' "${OUT}"; then
  tail -n 30 "${OUT}" >&2
  die "the test failed for another reason (mount failed)"
fi
if ! grep -Fq 'ACTION-KEYDOWN-NOT-DISPATCHED' "${OUT}"; then
  tail -n 30 "${OUT}" >&2
  die "the test failed for another reason (no ACTION-KEYDOWN-NOT-DISPATCHED mark)"
fi
log "control established: without action-keydown the test fails with ACTION-KEYDOWN-NOT-DISPATCHED"
