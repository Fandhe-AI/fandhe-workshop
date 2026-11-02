#!/usr/bin/env bash
# 入力欄のない教材の完走の道具を、設定ファイルで流す（`make check-choice-journey` と CI の browser-test ジョブから呼ばれる。TASK-748 2/2）。
#
# 目的: 教材固有の値（操作の列・期待・派生版の書き換え）を設定ファイルに分けた道具（tools/browser/choice-journey-run.mjs）を、
# node:test の外から 1 コマンドで呼べるようにする。設定ファイルを差し替えるだけで別の教材にも使える。
# 手順の実体は Node の CLI にあり、ここは前提の確認・ビルド・環境変数の受け渡しだけを行う（二重定義をしない）。
#
# 処理の順
#   1. 引数と環境の確認（誤用は終了コード 2。ビルドや起動の前に止める）
#   2. FANDHE_PACK_BIN が無ければ scripts/build-pack-bin.sh で fandhe-pack を作る
#   3. FANDHE_WEB_AUTHOR_DIR が無ければ IR 同梱の配信する設定（TEST_HOOKS なし）を一時ディレクトリへ作る。
#      同梱する IR は index.html の meta を出させるためだけに使い、画面に出す IR は設定ごとに道具が差し替える
#   4. node tools/browser/choice-journey-run.mjs を呼ぶ
#
# 使い方: scripts/check-choice-journey.sh [--config <file>]...
#   --config  設定ファイル（複数可）。省略時は tools/browser/choice-journey/*.json の全件
# 環境変数: FANDHE_PACK_BIN・FANDHE_WEB_AUTHOR_DIR（どちらも既存のものを使う。無ければ上のとおり作る）・
#           FANDHE_BROWSER_CHANNEL（chromium | chrome）・SKIP_WASM_OPT（ビルドに渡る）
# 終了コード: 0 = 全件合格 / 1 = 不合格・ビルドの失敗 / 2 = 誤用・道具が無い
# 回帰テスト: scripts/test-check-choice-journey.sh
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

usage() {
  echo "usage: $(basename "$0") [--config <file>]..." >&2
  exit 2
}

CONFIGS=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --config)
      [ "$#" -ge 2 ] || usage
      CONFIGS+=("$2")
      shift 2
      ;;
    *) usage ;;
  esac
done

if [ "${#CONFIGS[@]}" -eq 0 ]; then
  for f in "${ROOT_DIR}"/tools/browser/choice-journey/*.json; do
    [ -f "${f}" ] || continue
    CONFIGS+=("${f}")
  done
fi
if [ "${#CONFIGS[@]}" -eq 0 ]; then
  echo "error: no config files found" >&2
  exit 2
fi

# 設定のパスは相対の親（..）を含めない。存在する通常のファイルだけを渡す（値はそのまま node の引数になるため）。
ARGS=()
for c in "${CONFIGS[@]}"; do
  case "${c}" in
    -* | */../* | ../* | */.. | ..)
      echo "error: unsafe config path" >&2
      exit 2
      ;;
  esac
  if [ ! -f "${c}" ]; then
    echo "error: config file not found: ${c}" >&2
    exit 2
  fi
  ARGS+=(--config "${c}")
done

case "${FANDHE_BROWSER_CHANNEL:-}" in
  '' | chromium | chrome) ;;
  *)
    echo "error: FANDHE_BROWSER_CHANNEL must be chromium or chrome" >&2
    exit 2
    ;;
esac

require_cmd node || exit 2

# 一時ディレクトリを作る前は空。空のまま終わった場合は何も消さない（下の trap は [ -n ] で守る）。
TMP=""
trap '[ -z "${TMP}" ] || rm -rf "${TMP}"' EXIT

if [ -z "${FANDHE_PACK_BIN:-}" ]; then
  log "building fandhe-pack (debug)"
  FANDHE_PACK_BIN="$("${DIR}/build-pack-bin.sh")"
fi
export FANDHE_PACK_BIN

if [ -z "${FANDHE_WEB_AUTHOR_DIR:-}" ]; then
  for cmd in cargo wasm-bindgen jq mktemp; do
    require_cmd "${cmd}" || exit 2
  done
  TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-choice-journey.XXXXXX")"
  log "building the delivery web (IR_FILE, no test hooks) into a temporary directory"
  "${FANDHE_PACK_BIN}" compile "${ROOT_DIR}/crates/web/tests/data/ch0_3_pack" -o "${TMP}/ir.json" >/dev/null
  TEST_HOOKS="" OUT_DIR="${TMP}/web-author" IR_FILE="${TMP}/ir.json" SKIP_WASM_OPT="${SKIP_WASM_OPT:-}" \
    "${ROOT_DIR}/tools/build-web.sh" >/dev/null
  FANDHE_WEB_AUTHOR_DIR="${TMP}/web-author"
fi
export FANDHE_WEB_AUTHOR_DIR

rc=0
node "${ROOT_DIR}/tools/browser/choice-journey-run.mjs" "${ARGS[@]}" || rc=$?
exit "${rc}"
