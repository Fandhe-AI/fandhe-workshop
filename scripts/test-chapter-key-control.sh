#!/usr/bin/env bash
# 章ごとの保存先の鍵の対照の検査（make test-chapter-key-control から呼ばれる。TASK-326 1/2 / SAVE-14・DLV-8）。
#
# `EngineHost::history_key` を、試作の修正前に当たる固定の鍵（教材にも章にも依らない）に置き換えた写しをビルドし、
# tools/browser/chapter-keys.test.mjs が「先の章の履歴が出た」理由（固定の印 CHAPTER-HISTORY-MIXED）で失敗することを確かめる。
# これで、試験が空振りでなく、鍵の退行を実際に検出できると言える。置き換えは変異 SAVE14-5 と同じである
# （変異は host の層、これはブラウザの層の対照）。FIND は SAVE14-5 の find と同じ文字列で、鍵の式を変えたら
# crates/web/tests/mutations/SAVE14-5.mutation とこの FIND を合わせて直す。
#
# 方式は test-keydown-control.sh と同じで、作業ツリーは書き換えない。mktemp -d の中に必要なファイルを写し、
# 写した crates/web/src/host.rs の 1 か所を文字どおりの一致（awk の index）で置き換える。一致が 1 回でなければ止める。
# 前提は test-browser.sh と同じ（npm ci・Playwright の Chromium・wasm-bindgen CLI）。
# 依存を一時ディレクトリで冷えた状態からビルドするため遅い。make check・フック・CI には入れない。
#
# 終了コード: 0 = 対照が成立（正しい理由で失敗した） / 1 = 対照が不成立（修正前の鍵でも試験が通った） /
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

TARGET="crates/web/src/host.rs"
FIND='Some(HistoryKey::for_pack(scope, &ir.pack.id, recorded.start()))'
REPLACE='Some(HistoryKey::for_pack(scope, "", ""))'

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-chapter-key-control.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT
COPY="${TMP}/copy"
mkdir -p "${COPY}/tools" "${COPY}/scripts"

for item in Cargo.toml Cargo.lock rust-toolchain.toml crates testdata; do
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
' "${COPY}/${TARGET}")"
[ "${occurrences}" = "1" ] || die "the line to replace must occur exactly once in ${TARGET} (found ${occurrences})"

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
' "${COPY}/${TARGET}" >"${COPY}/${TARGET}.new"
mv "${COPY}/${TARGET}.new" "${COPY}/${TARGET}"

# 健全性の確認: 置き換えた写しに、固定の鍵の式が入り、元の式が残っていないこと（置き換えの空振りを防ぐ）。
grep -Fq "${REPLACE}" "${COPY}/${TARGET}" || die "the replacement is missing in the copy"
if grep -Fq "${FIND}" "${COPY}/${TARGET}"; then die "the original key expression is still in the copy"; fi

log "building the copy with test hooks (cold dependencies; this is slow)"
(
  cd "${COPY}"
  CARGO_TARGET_DIR="${TMP}/target" TEST_HOOKS=1 SKIP_WASM_OPT=1 OUT_DIR="${COPY}/dist/web-test" \
    "${COPY}/tools/build-web.sh" >/dev/null
) || die "the build of the copy failed"

OUT="${TMP}/out.txt"
rc=0
FANDHE_WEB_TEST_DIR="${COPY}/dist/web-test" node --test \
  "${ROOT_DIR}/tools/browser/chapter-keys.test.mjs" >"${OUT}" 2>&1 || rc=$?

if [ "${rc}" -eq 0 ]; then
  tail -n 30 "${OUT}" >&2
  echo "error: control not established: the test passed with the pre-fix fixed key" >&2
  exit 1
fi
if grep -Fq 'mount failed' "${OUT}"; then
  tail -n 30 "${OUT}" >&2
  die "the test failed for another reason (mount failed)"
fi
if ! grep -Fq 'CHAPTER-HISTORY-MIXED' "${OUT}"; then
  tail -n 30 "${OUT}" >&2
  die "the test failed for another reason (no CHAPTER-HISTORY-MIXED mark)"
fi
log "control established: with the pre-fix fixed key the test fails with CHAPTER-HISTORY-MIXED"
