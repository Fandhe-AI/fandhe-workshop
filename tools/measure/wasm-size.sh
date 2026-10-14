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
# 終了コード: 0 = 成功 / 1 = ビルド・測定・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用。
# build-web.sh のログ行（config / wasm-opt / wasm size）を解析する。各行が厳密に 1 行でなければ止まる（fail-closed）。
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
SHA_BEFORE="$(jq -r '.sha' <<<"${COMMIT_JSON}")"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-measure.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

log "building (release) via tools/build-web.sh"
if ! OUT_DIR="${TMP}/web" IR_FILE="" "${ROOT_DIR}/tools/build-web.sh" release >"${TMP}/build.log" 2>&1; then
  cat "${TMP}/build.log" >&2
  echo "error: build-web.sh failed" >&2
  exit 1
fi
cat "${TMP}/build.log" >&2

# one_line <prefix>: ログからその接頭辞で始まる行をちょうど 1 行取り出し、接頭辞を除いた残りを返す。
one_line() {
  local prefix="$1" n
  n="$(grep -c -F -- "[build-web.sh] ${prefix}" "${TMP}/build.log" || true)"
  if [ "${n}" != "1" ]; then
    echo "error: expected exactly one '${prefix}' line in the build log (got ${n})" >&2
    return 1
  fi
  grep -F -- "[build-web.sh] ${prefix}" "${TMP}/build.log" | sed -e "s/^\[build-web.sh\] ${prefix} *//"
}

CONFIG_LINE="$(one_line "config:")" || exit 1
WASM_OPT_LINE="$(one_line "wasm-opt:")" || exit 1
SIZE_LINE="$(one_line "wasm size:")" || exit 1

# 選択（\|）は BSD sed（macOS 標準）の BRE で動かないため、case で完全一致を判定する。
case "${CONFIG_LINE}" in
  "delivery / profile: release") CONFIG="delivery" ;;
  "test / profile: release") CONFIG="test" ;;
  *)
    echo "error: unexpected config line in the build log" >&2
    exit 1
    ;;
esac

case "${WASM_OPT_LINE}" in
  "not applied ("*")") WASM_OPT_APPLIED=false; WASM_OPT_DETAIL="${WASM_OPT_LINE#not applied (}" ;;
  "applied ("*")") WASM_OPT_APPLIED=true; WASM_OPT_DETAIL="${WASM_OPT_LINE#applied (}" ;;
  *)
    echo "error: unexpected wasm-opt line in the build log" >&2
    exit 1
    ;;
esac
WASM_OPT_DETAIL="${WASM_OPT_DETAIL%)}"

LOG_RAW="$(sed -n 's/^raw \([0-9][0-9]*\) bytes, gzip -9 \([0-9][0-9]*\) bytes$/\1/p' <<<"${SIZE_LINE}")"
LOG_GZ="$(sed -n 's/^raw \([0-9][0-9]*\) bytes, gzip -9 \([0-9][0-9]*\) bytes$/\2/p' <<<"${SIZE_LINE}")"
if [ -z "${LOG_RAW}" ] || [ -z "${LOG_GZ}" ]; then
  echo "error: unexpected wasm size line in the build log" >&2
  exit 1
fi

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

# 測定中に commit と作業ツリーの状態（dirty）が動いていないことを確かめる。
# 開始後の編集で、変更済みのソースに元の SHA と dirty:false が付くのを防ぐ。
COMMIT_JSON_AFTER="$(measure_commit_json)" || COMMIT_JSON_AFTER=""
if [ -z "${COMMIT_JSON_AFTER}" ] || [ "$(jq -r '.sha' <<<"${COMMIT_JSON_AFTER}")" != "${SHA_BEFORE}" ]; then
  echo "error: HEAD moved during the measurement" >&2
  exit 1
fi
if [ "$(jq -c . <<<"${COMMIT_JSON}")" != "$(jq -c . <<<"${COMMIT_JSON_AFTER}")" ]; then
  echo "error: working tree state changed during the measurement" >&2
  exit 1
fi

if [ "${CONFIG}" = "test" ]; then
  CARGO_FEATURES='["test-hooks"]'
else
  CARGO_FEATURES='[]'
fi
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
