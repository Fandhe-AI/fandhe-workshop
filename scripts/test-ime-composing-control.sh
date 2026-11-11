#!/usr/bin/env bash
# 変換中の判定の変異 M1・M3 のブラウザでの再現（make test-ime-composing-control から呼ばれる。#2123。JUDGE-14・JUDGE-16）。
#
# crates/contract/src/key_decide.rs の変換中の除外（Enter・Tab・Escape・Process）から、M1 では Enter を、M3 では Escape を外した写しを
# 1 つずつビルドし、tools/browser/ime-sentences.test.mjs を流す。定義の正は crates/contract/tests/mutations/ の
# JUDGE14-2.mutation（M1）と JUDGE16-1.mutation（M3）で、FIND・REPLACE はそこから読む（ここに文字列を重ねて書かない）。
#   M1: 誤送信の件数つきの印 IME-MIS-SEND で失敗すれば成立。
#   M3: 全件合格（生き残り）が記録どおり。上流 fandhe-frontend-wasm-full の action_from_keydown が変換中の keydown を先に除くため、
#       契約の Esc の除外を外してもブラウザでは観察できない。これは「検出の対照」ではなく「生き残りの記録の再現」で、
#       M3 が検出されたら記録（docs/testing.md 5.503）が古い。Esc の除外の検出先は native の試験（JUDGE16-1〜3）である。
# 上流の is_composing_keydown を外す変異は、上流 crate が repo の外にあり写しでは変えられないため、対象にしない。
#
# 方式は test-ime-sync-control.sh と同じで、作業ツリーは書き換えない。mktemp -d の中に必要なファイルを写し、
# 文字どおりの一致（awk の index）で 1 か所だけ置き換える。ビルドは配信する設定と試験の設定の 2 つ（試験が両方を要る）で、
# CARGO_TARGET_DIR は 2 変異・2 設定で共有する。前提は test-browser.sh と同じ（npm ci・Playwright の Chromium・wasm-bindgen CLI）。
# 遅いため make check・フック・CI には入れない（判定の回帰テストは test-lib-ime-control.sh）。
#
# 終了コード: 0 = M1 が印つきで失敗し M3 が記録どおり生き残った / 1 = M1 が不成立（Enter を外しても試験が通った） /
#             2 = 誤用・前提の不足・ビルドの失敗・別の理由での失敗 / 3 = M3 が記録と食い違う（検出された）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
REAL_ROOT="$(cd "${ROOT_DIR}" && pwd -P)"
# shellcheck source=./lib-mutations.sh
source "${DIR}/lib-mutations.sh"
# shellcheck source=./lib-ime-control.sh
source "${DIR}/lib-ime-control.sh"

reject_args "$(basename "$0")" "$@"

for cmd in node cargo wasm-bindgen mktemp mkdir awk grep cp rm mv; do
  require_cmd "${cmd}" || exit 2
done
if [[ ! -d "${ROOT_DIR}/node_modules/playwright" ]]; then
  echo "error: playwright is not installed; run npm ci && npx playwright install chromium" >&2
  exit 2
fi

DEF_DIR="${ROOT_DIR}/crates/contract/tests/mutations"
load_variant "${DEF_DIR}/JUDGE14-2.mutation"
M1_TARGET="${M_TARGET}" M1_FIND="${M_FIND}" M1_REPLACE="${M_REPLACE}"
load_variant "${DEF_DIR}/JUDGE16-1.mutation"
M3_TARGET="${M_TARGET}" M3_FIND="${M_FIND}" M3_REPLACE="${M_REPLACE}"
[ "${M1_TARGET}" = "${M3_TARGET}" ] || die "M1 and M3 must target the same file"
TARGET="${M1_TARGET}"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-ime-composing-control.XXXXXX")"
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

# apply_variant <find> <replace>: 写しの target を原本から戻し、1 か所だけ置き換える。空振りは止める。
apply_variant() {
  local find="$1" repl="$2" occurrences
  cp "${ROOT_DIR}/${TARGET}" "${COPY}/${TARGET}"
  occurrences="$(count_occurrences "${COPY}/${TARGET}" "${find}")"
  [ "${occurrences}" = "1" ] || die "the line to replace must occur exactly once in ${TARGET} (found ${occurrences})"
  FIND="${find}" REPLACE="${repl}" awk '
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
  grep -Fq -- "${repl}" "${COPY}/${TARGET}" || die "the replacement is missing in the copy"
  if grep -Fq -- "${find}" "${COPY}/${TARGET}"; then die "the original line is still in the copy"; fi
}

# run_variant <name>: 2 つの設定をビルドして ime-sentences を流し、終了コードを RUN_RC に、出力を ${TMP}/out-<name>.txt に残す。
run_variant() {
  local name="$1"
  log "building ${name} (web and test-hooks configurations; the first build is cold and slow)"
  (
    # || die の左辺のサブシェルでは set -e が効かないため、各コマンドに明示の失敗処理を付ける。
    cd "${COPY}" || exit 1
    CARGO_TARGET_DIR="${TMP}/target" TEST_HOOKS="" SKIP_WASM_OPT=1 OUT_DIR="${COPY}/dist/web" \
      "${COPY}/tools/build-web.sh" >/dev/null || exit 1
    CARGO_TARGET_DIR="${TMP}/target" TEST_HOOKS=1 SKIP_WASM_OPT=1 OUT_DIR="${COPY}/dist/web-test" \
      "${COPY}/tools/build-web.sh" >/dev/null || exit 1
  ) || die "the build of ${name} failed"
  RUN_RC=0
  FANDHE_WEB_DIR="${COPY}/dist/web" FANDHE_WEB_TEST_DIR="${COPY}/dist/web-test" node --test \
    "${ROOT_DIR}/tools/browser/ime-sentences.test.mjs" >"${TMP}/out-${name}.txt" 2>&1 || RUN_RC=$?
}

apply_variant "${M1_FIND}" "${M1_REPLACE}"
run_variant m1
m1="$(judge_m1 "${RUN_RC}" "${TMP}/out-m1.txt")"
if [ "${m1}" = "established" ]; then
  grep -E '^(✖|not ok)' "${TMP}/out-m1.txt" | head -n 20 || true
  grep -E "${IME_MIS_SEND_RE}" "${TMP}/out-m1.txt" | head -n 6 || true
else
  tail -n 30 "${TMP}/out-m1.txt" >&2
fi

apply_variant "${M3_FIND}" "${M3_REPLACE}"
run_variant m3
m3="$(judge_m3 "${RUN_RC}" "${TMP}/out-m3.txt")"
if [ "${m3}" != "survived" ]; then
  tail -n 30 "${TMP}/out-m3.txt" >&2
fi

log "M1: ${m1} (expected: established) / M3: ${m3} (expected: survived, see docs/testing.md 5.503)"
rc="$(combine_rc "${m1}" "${m3}")"
if [ "${rc}" -eq 3 ]; then
  echo "error: M3 was detected in the browser; the recorded limit in docs/testing.md 5.503 is out of date" >&2
fi
exit "${rc}"
