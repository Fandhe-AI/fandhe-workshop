#!/usr/bin/env bash
# WASM の gzip -9 の大きさを、基点（commit・wasm-opt・機能の範囲・圧縮の設定）つきで測る（make measure-wasm-size から呼ばれる）。
#
# 使い方: tools/measure/wasm-size.sh（引数なし）
#
# release の設定で tools/build-web.sh を一時ディレクトリへビルドし（dist/ は上書きしない）、結果の JSON を標準出力に 1 つだけ出す。
# 進行状況とビルドログは標準エラーへ出す。様式の定義は docs/measurement.md、出力は tools/measure/lib.sh の measure_emit だけが行う。
#
# 環境変数:
#   TEST_HOOKS     空 = 配信する設定 / 1 = 試験の設定（build-web.sh へそのまま渡し、検証も任せる）
#   SKIP_WASM_OPT  1 なら wasm-opt を適用しない（build-web.sh へ渡す）
#   IR_FILE        呼び出し元の値を引き継がない（ビルド設定を一定にするため空で渡す）
#
# 終了コード: 0 = 成功 / 1 = ビルド・測定・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用（build-web.sh の終了コード 2 も引き継ぐ）。
# ビルドとログ解析（config / wasm-opt / wasm size。各行が厳密に 1 行でなければ止まる）は lib.sh の measure_build が行う。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../../scripts/lib.sh
source "${DIR}/../../scripts/lib.sh"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd git
require_cmd jq
require_cmd gzip

# 基点の commit を先に集める。集められなければビルドせずに止まる。
COMMIT_JSON="$(measure_commit_json)" || {
  echo "error: cannot determine the baseline commit" >&2
  exit 1
}

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-measure.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

# build-web.sh の誤用（TEST_HOOKS=0 など）の 2 は 2 のまま返す。それ以外の失敗は 1。
BUILD_RC=0
measure_build "${TMP}" || BUILD_RC=$?
if [ "${BUILD_RC}" -ne 0 ]; then exit "${BUILD_RC}"; fi
CONFIG="${MEASURE_CONFIG}"
WASM_OPT_APPLIED="${MEASURE_WASM_OPT_APPLIED}"
WASM_OPT_DETAIL="${MEASURE_WASM_OPT_DETAIL}"
LOG_RAW="${MEASURE_LOG_RAW}"
LOG_GZ="${MEASURE_LOG_GZ}"

# 自分でも測り、ログの値と突き合わせる（取り違えの検出）。
WASM_REL="wasm/fandhe_workshop_web_bg.wasm"
WASM_FILE="${TMP}/web/${WASM_REL}"
[ -f "${WASM_FILE}" ] || {
  echo "error: wasm file not found in the build output" >&2
  exit 1
}
RAW="$(wc -c <"${WASM_FILE}" | tr -d ' ')"
GZ="$(gzip -9 -c "${WASM_FILE}" | wc -c | tr -d ' ')"
if [ "${RAW}" != "${LOG_RAW}" ] || [ "${GZ}" != "${LOG_GZ}" ]; then
  echo "error: measured size differs from the build log (raw ${RAW}/${LOG_RAW}, gzip ${GZ}/${LOG_GZ})" >&2
  exit 1
fi

measure_assert_commit_unchanged "${COMMIT_JSON}" || exit 1

CARGO_FEATURES="$(measure_features_json "${CONFIG}")" || exit 1
GZIP_IMPL="$(gzip --version 2>&1 | head -n 1)" || GZIP_IMPL=""
RUSTC_V="$(rustc -V 2>/dev/null || true)"
WBG_V="$(wasm-bindgen --version 2>/dev/null || true)"

jq -n \
  --arg measured_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --argjson commit "${COMMIT_JSON}" \
  --argjson applied "${WASM_OPT_APPLIED}" \
  --arg detail "${WASM_OPT_DETAIL}" \
  --arg config "${CONFIG}" \
  --argjson features "${CARGO_FEATURES}" \
  --arg impl "${GZIP_IMPL}" \
  --arg rustc "${RUSTC_V}" \
  --arg wbg "${WBG_V}" \
  --arg file "${WASM_REL}" \
  --argjson raw "${RAW}" \
  --argjson gz "${GZ}" \
  '{
    schema: "fandhe-workshop/measure/v1",
    measured_at: $measured_at,
    baseline: {
      commit: $commit,
      wasm_opt: {applied: $applied, detail: $detail},
      features: {config: $config, cargo_features: $features, profile: "release"},
      compression: {command: "gzip -9 -c", implementation: $impl}
    },
    toolchain: {rustc: $rustc, wasm_bindgen: $wbg},
    measurements: [
      {name: "wasm_raw", file: $file, unit: "bytes", value: $raw},
      {name: "wasm_gzip", file: $file, unit: "bytes", value: $gz}
    ]
  }' >"${TMP}/result.json"

measure_emit "${TMP}/result.json"
