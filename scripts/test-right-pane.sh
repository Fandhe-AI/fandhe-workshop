#!/usr/bin/env bash
# 右の画面の試験の束（tools/browser/suites/right-pane.txt。TASK-458。TERM-40）だけを 1 つの回帰として流す
# （`make test-right-pane` から呼ばれる）。入力欄のキー処理の受け持ちを変える作業（TASK-352 など）の前後で、
# 右の画面の操作の結果が変わらないことを短く確かめるための入口である。
#
# scripts/test-browser.sh は変えない（全件の流れと件数の照合に影響させないため）。束のファイルは全件が
# test-browser.sh に載っていることを scripts/check-right-pane-suite.sh（R3）が保証する。
# 前段のビルドは、束が読む配信する設定（FANDHE_WEB_DIR）・試験の設定（FANDHE_WEB_TEST_DIR）・fandhe-pack の 3 つだけ。
# 任意: FANDHE_TEST_TAP_OUT（TAP の出し先）・FANDHE_TEST_JUNIT_OUT（junit の出し先）。test-browser.sh と同じ形で、
#   件数の記録（TASK-458 2/3）が同じ道具で読めるようにする。FANDHE_BROWSER_CHANNEL=chrome で Chrome の原本。
# 遅く、ブラウザを要するため、CI・フックには入れない。
#
# 終了コード: 0 = 全件合格 / 1 = 試験の失敗・前提の不足・定義のずれ / 2 = 誤用（引数あり・不正な channel）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

case "${FANDHE_BROWSER_CHANNEL:-}" in
  '' | chromium | chrome) ;;
  *)
    echo "error: FANDHE_BROWSER_CHANNEL must be chromium or chrome" >&2
    exit 2
    ;;
esac

for cmd in node cargo wasm-bindgen jq mktemp rm awk; do
  require_cmd "${cmd}"
done

if [[ ! -d "${ROOT_DIR}/node_modules/playwright" ]]; then
  echo "error: playwright is not installed; run npm ci && npx playwright install chromium" >&2
  exit 1
fi

# 定義のずれは、遅いビルドの前に止める。
"${DIR}/check-right-pane-suite.sh"

DEF="${ROOT_DIR}/tools/browser/suites/right-pane.txt"
FILES=()
while IFS='|' read -r kind name _; do
  if [[ "${kind}" == "file" ]]; then
    FILES+=("${ROOT_DIR}/tools/browser/${name}")
  fi
done <"${DEF}"
if [[ ${#FILES[@]} -eq 0 ]]; then
  echo "error: no file entries in ${DEF}" >&2
  exit 1
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-right-pane.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

log "building fandhe-pack (debug)"
FANDHE_PACK_BIN="$("${DIR}/build-pack-bin.sh")"
export FANDHE_PACK_BIN

log "building web into a temporary directory"
TEST_HOOKS="" OUT_DIR="${TMP}/web" SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/build-web.sh" >/dev/null

log "building web (test hooks) into a temporary directory"
TEST_HOOKS=1 OUT_DIR="${TMP}/web-test" SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/build-web.sh" >/dev/null

REPORTER_ARGS=()
if [[ -n "${FANDHE_TEST_TAP_OUT:-}" || -n "${FANDHE_TEST_JUNIT_OUT:-}" ]]; then
  REPORTER_ARGS+=(--test-reporter=spec --test-reporter-destination=stdout)
fi
if [[ -n "${FANDHE_TEST_TAP_OUT:-}" ]]; then
  REPORTER_ARGS+=(--test-reporter=tap "--test-reporter-destination=${FANDHE_TEST_TAP_OUT}")
fi
if [[ -n "${FANDHE_TEST_JUNIT_OUT:-}" ]]; then
  REPORTER_ARGS+=(--test-reporter=junit "--test-reporter-destination=${FANDHE_TEST_JUNIT_OUT}")
fi

log "running ${#FILES[@]} right-pane test files"
cd "${ROOT_DIR}"
FANDHE_WEB_DIR="${TMP}/web" FANDHE_WEB_TEST_DIR="${TMP}/web-test" \
  node --test ${REPORTER_ARGS[@]+"${REPORTER_ARGS[@]}"} "${FILES[@]}"
