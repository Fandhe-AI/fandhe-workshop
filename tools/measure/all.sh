#!/usr/bin/env bash
# 3 つの測定（WASM の gzip・初回表示・再描画）を 1 つのコマンドで流し、基点つきの束を出す（make measure から呼ばれる）。
#
# 使い方: tools/measure/all.sh（引数なし）
#
# wasm-size.sh・first-paint.sh・redraw.sh を順に実行し、各結果（様式 v1）を fandhe-workshop/measure-run/v1 の束にまとめて
# 標準出力に 1 つだけ出す。大きさと初回表示は配信する設定、再描画は試験の設定（debug_rerender が要る）で測るため、
# 基点の機能の範囲は結果ごとに持つ。出力は lib.sh の measure_emit_run だけが行い、束の検証に通らなければ何も出さない（fail-closed）。
# 3 つのうち 1 つでも失敗するか、実行中に HEAD や作業ツリーが動いたら、標準出力は空で終了する。
# 前提: npm ci && npx playwright install chromium。遅く、ブラウザを要するため make check・フック・CI には入れない。
#
# 環境変数: SKIP_WASM_OPT（1 なら wasm-opt を適用しない。3 つへ同じ値を渡す）。TEST_HOOKS は各スクリプトが固定する。
#
# 終了コード: 0 = 成功 / 1 = いずれかの測定・基点の収集・束の検証の失敗（標準出力は空） / 2 = 誤用（子の 2 もそのまま返す）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../../scripts/lib.sh
source "${DIR}/../../scripts/lib.sh"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd git
require_cmd jq
require_cmd node

COMMIT_JSON="$(measure_commit_json)" || {
  echo "error: cannot determine the baseline commit" >&2
  exit 1
}

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-measure.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

# run_one <名前> <スクリプト>: 子の標準出力を一時ファイルへ受ける。失敗は終了コードを保ったまま止める（2 は 2 のまま）。
run_one() {
  local name="$1" script="$2" rc=0
  log "running ${name}"
  # TEST_HOOKS は呼び出し元の値ではなく、各スクリプトが固定する（wasm-size.sh だけは環境をそのまま使うため空を明示する）。
  case "${name}" in
    wasm-size) TEST_HOOKS="" "${script}" >"${TMP}/${name}.json" || rc=$? ;;
    *) "${script}" >"${TMP}/${name}.json" || rc=$? ;;
  esac
  if [ "${rc}" -ne 0 ]; then
    echo "error: ${name} failed" >&2
    return "${rc}"
  fi
}

run_one wasm-size "${DIR}/wasm-size.sh" || exit $?
run_one first-paint "${DIR}/first-paint.sh" || exit $?
run_one redraw "${DIR}/redraw.sh" || exit $?

measure_assert_commit_unchanged "${COMMIT_JSON}" || exit 1

jq -n \
  --slurpfile a "${TMP}/wasm-size.json" \
  --slurpfile b "${TMP}/first-paint.json" \
  --slurpfile c "${TMP}/redraw.json" \
  '{schema: "fandhe-workshop/measure-run/v1", results: [$a[0], $b[0], $c[0]]}' >"${TMP}/run.json" || {
  echo "error: could not assemble the run result" >&2
  exit 1
}

measure_emit_run "${TMP}/run.json"
