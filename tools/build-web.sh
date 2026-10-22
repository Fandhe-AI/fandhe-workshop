#!/usr/bin/env bash
# web crate を wasm にビルドし、配るファイル一式を OUT_DIR に組み立てる（make build-web から呼ばれる）。
#
# 使い方: tools/build-web.sh [release|debug]（既定は release）
#
# 設定は 2 つ。
#   配信する設定（TEST_HOOKS が空）: feature なし。試験用の口（debug_* の書き出し・test-hooks.js・meta）を含めない。
#   試験の設定（TEST_HOOKS=1）: feature test-hooks あり。ブラウザ試験（TASK-28 以降）が使う。
#
# 環境変数:
#   TEST_HOOKS     空 = 配信する設定 / 1 = 試験の設定。それ以外は誤用（TEST_HOOKS=0 が試験の設定になる事故を防ぐ）
#   IR_FILE        指定したときだけ ir/main.json として同梱する。既定の IR は持たない
#   OUT_DIR        出力先。既定は dist/web（試験の設定は dist/web-test）。消して作り直すため dist/ 配下か、
#                  存在しない・空のディレクトリに限る
#   SKIP_WASM_OPT  1 なら wasm-opt を適用しない
#
# 終了コード: 0 = 成功 / 1 = ビルドまたは検証の失敗 / 2 = 誤用。
# wasm-bindgen CLI の版は Cargo.lock と一致させる（docs/toolchain.md）。回帰テストは scripts/test-build-web.sh。
# scripts/ と同じ規約（bash 3.2 互換・英語のエラー）に従い、shellcheck も make check が行う。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../scripts/lib.sh
source "${DIR}/../scripts/lib.sh"

usage() {
  echo "usage: build-web.sh [release|debug]" >&2
  exit 2
}

[ "$#" -le 1 ] || usage
PROFILE="${1:-release}"
case "${PROFILE}" in
  release | debug) ;;
  *) usage ;;
esac

case "${TEST_HOOKS:-}" in
  "") CONFIG="delivery"; DEFAULT_OUT="dist/web" ;;
  1) CONFIG="test"; DEFAULT_OUT="dist/web-test" ;;
  *)
    echo "error: TEST_HOOKS must be empty or 1 (got: ${TEST_HOOKS})" >&2
    exit 2
    ;;
esac

# 未設定のときだけ既定値を使う（:- だと OUT_DIR= の空値が既定値に化け、誤用を見逃す）。
OUT_DIR="${OUT_DIR-${ROOT_DIR}/${DEFAULT_OUT}}"
if [ -z "${OUT_DIR}" ]; then
  echo "error: OUT_DIR must not be empty" >&2
  exit 2
fi

# IR_FILE は呼び出し元の cwd 基準で確認し、以降の cd に影響されないよう絶対パスへ解決しておく。
IR_FILE="${IR_FILE:-}"
IR_ABS=""
if [ -n "${IR_FILE}" ]; then
  if [ ! -f "${IR_FILE}" ]; then
    echo "error: IR_FILE is not a regular file: ${IR_FILE}" >&2
    exit 2
  fi
  IR_ABS="${IR_FILE}"
  # ファイル自体がシンボリックリンクなら実体まで辿る（出力先配下への迂回を見逃さないため）。
  while [ -L "${IR_ABS}" ]; do
    LINK_TARGET="$(readlink "${IR_ABS}")"
    case "${LINK_TARGET}" in
      /*) IR_ABS="${LINK_TARGET}" ;;
      *) IR_ABS="$(dirname "${IR_ABS}")/${LINK_TARGET}" ;;
    esac
  done
  IR_ABS="$(cd "$(dirname "${IR_ABS}")" && pwd -P)/$(basename "${IR_ABS}")"
fi

# 出力先は丸ごと消すため、消してよい場所に限る（fail-closed）。
# 親を正規化してから判定し、.. や symlink で dist/ の外へ出る指定を通さない。
OUT_BASE="$(basename "${OUT_DIR}")"
# 末尾が . や .. だと親の正規化だけでは dist/ の外（dist/.. はリポジトリルート）を指すため拒否する。
case "${OUT_BASE}" in
  . | .. | /)
    echo "error: OUT_DIR must not end with '.', '..' or '/': ${OUT_DIR}" >&2
    exit 2
    ;;
esac
OUT_PARENT="$(dirname "${OUT_DIR}")"
# 既定の出力先の親（dist/）は無ければ作る。それ以外の親は作らない。
if [ "${OUT_PARENT}" = "${ROOT_DIR}/dist" ]; then
  mkdir -p "${OUT_PARENT}"
fi
if [ ! -d "${OUT_PARENT}" ]; then
  echo "error: parent of OUT_DIR does not exist: ${OUT_PARENT}" >&2
  exit 2
fi
OUT_ABS="$(cd "${OUT_PARENT}" && pwd -P)/${OUT_BASE}"
ROOT_REAL="$(cd "${ROOT_DIR}" && pwd -P)"
# 出力先は丸ごと消すため、入力の IR が出力先の配下にあると削除で入力が消える。削除の前に拒否する。
if [ -n "${IR_ABS}" ]; then
  case "${IR_ABS}" in
    "${OUT_ABS}" | "${OUT_ABS}"/*)
      echo "error: IR_FILE must not be OUT_DIR or inside it (it would be deleted): ${IR_ABS}" >&2
      exit 2
      ;;
  esac
fi
# 既存の通常ファイル（シンボリックリンクを含む）は出力ディレクトリにできない。dist/ 配下でも拒否する。
if { [ -e "${OUT_ABS}" ] || [ -L "${OUT_ABS}" ]; } && [ ! -d "${OUT_ABS}" ]; then
  echo "error: OUT_DIR exists and is not a directory: ${OUT_ABS}" >&2
  exit 2
fi
case "${OUT_ABS}" in
  "${ROOT_REAL}/dist/"?*) ;;
  *)
    if [ -e "${OUT_ABS}" ] && { [ ! -d "${OUT_ABS}" ] || [ -n "$(ls -A "${OUT_ABS}")" ]; }; then
      echo "error: refusing to remove OUT_DIR outside dist/ unless it is empty or absent: ${OUT_ABS}" >&2
      exit 2
    fi
    ;;
esac

require_cmd cargo
require_cmd wasm-bindgen
require_cmd jq

cd "${ROOT_DIR}"

# wasm-bindgen の CLI と crate の版が違うと glue が壊れるため、Cargo.lock と一致しなければ止める。
LOCK_VERSIONS="$(awk '/^name = "wasm-bindgen"$/ {getline; gsub(/version = |"/, ""); print}' Cargo.lock)"
if [ "$(printf '%s\n' "${LOCK_VERSIONS}" | grep -c .)" -ne 1 ]; then
  echo "error: Cargo.lock must contain exactly one wasm-bindgen entry" >&2
  exit 1
fi
CLI_VERSION="$(wasm-bindgen --version | awk '{print $2}')"
if [ "${CLI_VERSION}" != "${LOCK_VERSIONS}" ]; then
  echo "error: wasm-bindgen CLI ${CLI_VERSION} does not match Cargo.lock ${LOCK_VERSIONS}" >&2
  echo "fix: cargo install wasm-bindgen-cli --version ${LOCK_VERSIONS} --locked" >&2
  exit 1
fi

CARGO_ARGS=(build --locked -p fandhe-workshop-web --target wasm32-unknown-unknown)
PROFILE_DIR="debug"
if [ "${PROFILE}" = "release" ]; then
  CARGO_ARGS+=(--release)
  PROFILE_DIR="release"
fi
if [ "${CONFIG}" = "test" ]; then
  CARGO_ARGS+=(--features test-hooks)
fi

log "cargo ${CARGO_ARGS[*]}"
cargo "${CARGO_ARGS[@]}"

# CARGO_TARGET_DIR や設定ファイルによる target の移動に追従する。
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | jq -r .target_directory)"
ARTIFACT="${TARGET_DIR}/wasm32-unknown-unknown/${PROFILE_DIR}/fandhe_workshop_web.wasm"
if [ ! -f "${ARTIFACT}" ]; then
  echo "error: build artifact not found: ${ARTIFACT}" >&2
  exit 1
fi

rm -rf "${OUT_ABS}"
mkdir -p "${OUT_ABS}/wasm"
wasm-bindgen --target web --out-dir "${OUT_ABS}/wasm" --out-name fandhe_workshop_web "${ARTIFACT}"

STATIC_DIR="${ROOT_DIR}/crates/web/static"
cp "${STATIC_DIR}/app.js" "${STATIC_DIR}/app.css" "${STATIC_DIR}/icon.svg" "${OUT_ABS}/"
# index.html の印は、試験の設定では meta に置き換え、配信する設定では行ごと消す。
# 同梱の IR の印は、配信する設定で IR_FILE があるときだけ meta にする（app.js はこの meta があるときだけ ./ir/main.json を取得する。
# 試験の設定は試験データの教材を埋め込むため meta を置かない。TASK-112 2/2）。
if [ "${CONFIG}" = "test" ]; then
  sed -e 's|<!--fandhe:test-hooks-->|<meta name="fandhe-test-hooks" content="1">|' \
    -e '/<!--fandhe:ir-->/d' \
    "${STATIC_DIR}/index.html" >"${OUT_ABS}/index.html"
  cp "${STATIC_DIR}/test-hooks.js" "${OUT_ABS}/"
else
  if [ -n "${IR_ABS}" ]; then
    IR_SED='s|<!--fandhe:ir-->|<meta name="fandhe-ir" content="1">|'
  else
    IR_SED='/<!--fandhe:ir-->/d'
  fi
  sed -e '/<!--fandhe:test-hooks-->/d' -e "${IR_SED}" \
    "${STATIC_DIR}/index.html" >"${OUT_ABS}/index.html"
  rm -f "${OUT_ABS}"/wasm/*.d.ts
fi

if [ -n "${IR_ABS}" ]; then
  mkdir -p "${OUT_ABS}/ir"
  cp "${IR_ABS}" "${OUT_ABS}/ir/main.json"
fi

WASM_OUT="${OUT_ABS}/wasm/fandhe_workshop_web_bg.wasm"
WASM_OPT_STATUS="not applied (non-release profile)"
if [ "${PROFILE}" = "release" ]; then
  if [ "${SKIP_WASM_OPT:-}" = "1" ]; then
    WASM_OPT_STATUS="not applied (SKIP_WASM_OPT=1)"
  elif command -v wasm-opt >/dev/null 2>&1; then
    TMP_WASM="$(mktemp "${TMPDIR:-/tmp}/fandhe-wasm-opt.XXXXXX")"
    trap 'rm -f "${TMP_WASM}"' EXIT
    # 失敗を黙って未適用にせず、ビルド全体の失敗にする。
    wasm-opt -Os "${WASM_OUT}" -o "${TMP_WASM}"
    cat "${TMP_WASM}" >"${WASM_OUT}"
    WASM_OPT_STATUS="applied ($(wasm-opt --version))"
  else
    WASM_OPT_STATUS="not applied (wasm-opt not found)"
  fi
fi

RAW_SIZE="$(wc -c <"${WASM_OUT}" | tr -d ' ')"
GZ_SIZE="$(gzip -9 -c "${WASM_OUT}" | wc -c | tr -d ' ')"
log "config: ${CONFIG} / profile: ${PROFILE}"
log "wasm-opt: ${WASM_OPT_STATUS}"
log "wasm size: raw ${RAW_SIZE} bytes, gzip -9 ${GZ_SIZE} bytes"
case "${OUT_ABS}" in
  "${ROOT_REAL}/"*) log "output: ${OUT_ABS#"${ROOT_REAL}"/}" ;;
  *) log "output: ${OUT_ABS}" ;;
esac
