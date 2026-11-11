#!/usr/bin/env bash
# 反映の変換中の除外の対照の検査（make test-ime-sync-control から呼ばれる。TASK-355 2/2 / FEND-10 の変異 M2）。
#
# crates/web/src/glue.rs の sync_draft の先頭にある変換中の除外（`if is_composing() {`）を、常に偽の分岐に置き換えた写しを
# ビルドし、tools/browser/ime-reload-flood.test.mjs が固定の印 IME-COMPOSING-LOST つきで失敗することを確かめる。
# これで、試験が空振りでなく、除外の退行を実際に検出できると言える。検出する場面は S-M2b（変換中に送信の書き戻しが頼まれる場面。#2121）。
#
# 方式は test-chapter-key-control.sh と同じで、作業ツリーは書き換えない。mktemp -d の中に必要なファイルを写し、
# 写した glue.rs の 1 か所を文字どおりの一致（awk の index）で置き換える。一致が 1 回でなければ止める。
# FIND は glue.rs の除外の行と合わせる。除外の書き方を変えたら、この FIND も直す。
# on_input の isComposing の判定だけを外す変異は、sync_draft の除外が二重に守るため生き残る見込みで、対象にしない。
# 前提は test-browser.sh と同じ（npm ci・Playwright の Chromium・wasm-bindgen CLI）。
# 依存を一時ディレクトリで冷えた状態からビルドするため遅い。make check・フック・CI には入れない。
#
# 終了コード: 0 = 対照が成立（正しい理由で失敗した） / 1 = 対照が不成立（除外を外しても試験が通った） /
#             2 = 誤用・前提の不足・別の理由での失敗（合格ではない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

for cmd in node cargo wasm-bindgen mktemp awk grep cp rm mv; do
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

TARGET="crates/web/src/glue.rs"
FIND='    if is_composing() {'
REPLACE='    if false {'

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-ime-sync-control.XXXXXX")"
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

# 健全性の確認: 置き換えた写しに置換後の行が入り、元の行が残っていないこと（置き換えの空振りを防ぐ）。
grep -Fq "${REPLACE}" "${COPY}/${TARGET}" || die "the replacement is missing in the copy"
if grep -Fq "${FIND}" "${COPY}/${TARGET}"; then die "the original exclusion is still in the copy"; fi

log "building the copy with test hooks (cold dependencies; this is slow)"
(
  cd "${COPY}"
  CARGO_TARGET_DIR="${TMP}/target" TEST_HOOKS=1 SKIP_WASM_OPT=1 OUT_DIR="${COPY}/dist/web-test" \
    "${COPY}/tools/build-web.sh" >/dev/null
) || die "the build of the copy failed"

OUT="${TMP}/out.txt"
rc=0
FANDHE_WEB_TEST_DIR="${COPY}/dist/web-test" node --test \
  "${ROOT_DIR}/tools/browser/ime-reload-flood.test.mjs" >"${OUT}" 2>&1 || rc=$?

if [ "${rc}" -eq 0 ]; then
  tail -n 30 "${OUT}" >&2
  echo "error: control not established: the test passed without the composing exclusion" >&2
  exit 1
fi
if grep -Fq 'mount failed' "${OUT}"; then
  tail -n 30 "${OUT}" >&2
  die "the test failed for another reason (mount failed)"
fi
if ! grep -Fq 'IME-COMPOSING-LOST' "${OUT}"; then
  tail -n 30 "${OUT}" >&2
  die "the test failed for another reason (no IME-COMPOSING-LOST mark)"
fi
grep -E '^(✖|not ok)' "${OUT}" | head -n 20 || true
log "control established: without the composing exclusion the test fails with IME-COMPOSING-LOST"
