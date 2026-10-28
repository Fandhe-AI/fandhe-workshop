#!/usr/bin/env bash
# 配信する設定の WASM の gzip -9 の大きさが 1 MB 以下であることを確かめる CI の関門（make check-wasm-size・CI の browser-test から呼ばれる）。
#
# 使い方: scripts/check-wasm-size.sh [--result <json-file>] [--require-wasm-opt]
#   引数なし: TEST_HOOKS を空に固定して tools/measure/wasm-size.sh で測り、その結果を判定する。SKIP_WASM_OPT は呼び出し元の値を渡す。
#   --result: 測定せず、渡された結果 JSON（fandhe-workshop/measure/v1）だけを判定する（回帰テスト scripts/test-check-wasm-size.sh 用。CI は渡さない）。
#   --require-wasm-opt: DLV-4・TASK-107 2/2。wasm-opt 適用済み（baseline.wasm_opt.applied が真偽値の true）の結果だけを判定し、未適用なら 2 で断る。
#     判定を厳しくする側にしか働かず、環境変数では有効にできない。CI は従来どおり未適用の値で判定し、この厳格な判定は手元で実行する（docs/measurement.md §1）。
#
# 上限の出典: TASK-78 2/2・BUILD-3・DLV-27（1 MB = 1,048,576 bytes。R-33 の提案値）。上限は定数で、環境変数や引数では変えられない
# （PR が workflow の env だけで関門を緩められないようにするため）。
# wasm-opt は CI に入れない（TASK-101 2/2 と同じ判断。供給網の面を増やさない）。SKIP_WASM_OPT=1 の未適用の値は適用後より大きい見込みで、
# その値が上限以下なら適用後も以下になる側の判定である（経験則であり保証ではない。基点の wasm_opt を標準出力の JSON に残す）。
#
# 出力: 標準エラーに 1 行の結果（超過時は error 行）。標準出力に、判定に使った JSON へ gate {limit_bytes, verdict} を足したものを出す。
# 終了コード: 0 = 上限以下 / 1 = 超過、または測定の失敗 / 2 = 誤用か判定不能（様式の欠落・試験の設定・wasm_gzip の欠落や重複。fail-closed）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
# shellcheck source=../tools/measure/lib.sh
source "${ROOT_DIR}/tools/measure/lib.sh"

readonly LIMIT_BYTES=1048576

require_cmd jq
require_cmd gzip
require_cmd git

RESULT_FILE=""
REQUIRE_WASM_OPT=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --result)
      if [ "$#" -lt 2 ]; then
        echo "usage: $(basename "$0") [--result <json-file>] [--require-wasm-opt]" >&2
        exit 2
      fi
      RESULT_FILE="$2"
      shift 2
      ;;
    --require-wasm-opt)
      REQUIRE_WASM_OPT=1
      shift
      ;;
    *)
      echo "usage: $(basename "$0") [--result <json-file>] [--require-wasm-opt]" >&2
      exit 2
      ;;
  esac
done

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-check-wasm-size.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

if [ -z "${RESULT_FILE}" ]; then
  # 標準出力の JSON はファイルで受け、終了状態を先に確かめる（パイプの左辺に置くと失敗を取りこぼす）。
  RESULT_FILE="${TMP}/result.json"
  RC=0
  TEST_HOOKS="" "${ROOT_DIR}/tools/measure/wasm-size.sh" >"${RESULT_FILE}" || RC=$?
  if [ "${RC}" -ne 0 ]; then
    echo "error: wasm-size measurement failed (exit ${RC})" >&2
    if [ "${RC}" -eq 2 ]; then exit 2; fi
    exit 1
  fi
fi

if [ ! -r "${RESULT_FILE}" ]; then
  echo "error: cannot read result file: ${RESULT_FILE}" >&2
  exit 2
fi

# 様式の検査。1 つでも外れたら判定しない。
if ! measure_validate "${RESULT_FILE}"; then
  echo "error: result is malformed or missing baseline fields; cannot judge" >&2
  exit 2
fi

# 試験の設定の値で関門を通さない。配信する設定・release・機能の追加なしだけを受け付ける。
if ! jq -e -s '.[0].baseline.features
    | .config == "delivery" and .profile == "release" and .cargo_features == []' \
  "${RESULT_FILE}" >/dev/null 2>&1; then
  echo "error: result is not from the delivery configuration (release, no extra features); cannot judge" >&2
  exit 2
fi

# DLV-4 の前提（wasm-opt 適用）を満たさない値では合格を出さない。型は真偽値の true だけを受け付ける（文字列の "true" は不可）。
if [ "${REQUIRE_WASM_OPT}" = "1" ] && ! jq -e -s '.[0].baseline.wasm_opt.applied == true' "${RESULT_FILE}" >/dev/null 2>&1; then
  echo "error: result was measured without wasm-opt (DLV-4 requires wasm-opt applied); cannot judge" >&2
  exit 2
fi

# wasm_gzip はちょうど 1 件で、単位は bytes、値は 0 以上 2^53-1 以下の整数。
# 1e20 のような値は jq が 1e+20 と出し、bash の整数比較がエラーになる（if 内では set -e が働かず合格になる）ため、ここで断る。
if ! jq -e -s '[.[0].measurements[] | select(.name == "wasm_gzip")]
    | length == 1 and .[0].unit == "bytes"
      and (.[0].value | type == "number" and . == floor and . >= 0 and . <= 9007199254740991)' \
  "${RESULT_FILE}" >/dev/null 2>&1; then
  echo "error: result needs exactly one wasm_gzip measurement in bytes with an integer value in 0..2^53-1; cannot judge" >&2
  exit 2
fi

GZ="$(jq -s -r '.[0].measurements[] | select(.name == "wasm_gzip") | .value' "${RESULT_FILE}")"
SHA="$(jq -s -r '.[0].baseline.commit.sha' "${RESULT_FILE}")"
DIRTY="$(jq -s -r '.[0].baseline.commit.dirty' "${RESULT_FILE}")"
OPT_APPLIED="$(jq -s -r '.[0].baseline.wasm_opt.applied' "${RESULT_FILE}")"
OPT_DETAIL="$(jq -s -r '.[0].baseline.wasm_opt.detail' "${RESULT_FILE}")"

if [ "${OPT_APPLIED}" = "true" ]; then OPT_TEXT="applied"; else OPT_TEXT="not applied"; fi
SUMMARY="wasm gzip -9 ${GZ} bytes (limit ${LIMIT_BYTES}) / commit ${SHA} (dirty: ${DIRTY}) / wasm-opt: ${OPT_TEXT} (${OPT_DETAIL}) / config: delivery"

# 比較に渡せる形（桁数の上限つきの 10 進整数）でなければ判定しない（fail-closed）。
if ! [[ "${GZ}" =~ ^[0-9]{1,16}$ ]]; then
  echo "error: cannot compare wasm_gzip value '${GZ}' with the limit; cannot judge" >&2
  exit 2
fi
VERDICT="pass"
if [ "${GZ}" -gt "${LIMIT_BYTES}" ]; then VERDICT="fail"; fi

# 基点つきの値をログに残すため、判定に使った JSON に gate を足して標準出力へ出す。
jq -s --argjson limit "${LIMIT_BYTES}" --arg verdict "${VERDICT}" \
  '.[0] + {gate: {limit_bytes: $limit, verdict: $verdict}}' "${RESULT_FILE}"

if [ "${VERDICT}" = "fail" ]; then
  echo "error: ${SUMMARY}" >&2
  echo "error: exceeds the limit by $((GZ - LIMIT_BYTES)) bytes" >&2
  exit 1
fi
log "${SUMMARY}"
