#!/usr/bin/env bash
# Rust のソースを変えない検査（`make lint` から呼ばれる）。
#
# rustfmt の check、clippy（警告をエラー扱い）、web の wasm32 向け clippy（feature なしと test-hooks の 2 通り）を流す。
# 組み合わせはネイティブ 1 + wasm32 2 である。仕様（BUILD-5）が挙げる hello feature は、docs/architecture.md 第 7 節が
# 「製品に持ち込まない」と定めるため web crate に無く、流さない。組み合わせは WASM_FEATURE_SETS の 1 か所で宣言し、
# web crate が宣言した feature が配列に無ければ失敗させる（feature を足したとき網羅の更新漏れで関門が通らないようにする）。
# wasm32-unknown-unknown のターゲットが無いと cargo が失敗する。黙って skip せず、そのまま失敗させる
# （ターゲットと版は rust-toolchain.toml で固定する。docs/toolchain.md）。
# 整形の自動修正は `make fmt`（ここでは何も書き換えない）。CI では ci.yml の rust-lint ジョブが呼ぶ。
#
# 終了コード: 0 = 全検査が通過 / 1 = いずれかが失敗 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo
require_cmd jq

cd "${ROOT_DIR}"

log "cargo fmt --check"
cargo fmt --all --check

log "cargo clippy (workspace, all targets)"
cargo clippy --workspace --all-targets --locked -- -D warnings

# wasm32 向け clippy の feature の組み合わせ（空文字 = feature なし。複数はカンマ区切り）
WASM_FEATURE_SETS=("" "test-hooks")

log "check web features are covered"
declared="$(cargo metadata --no-deps --format-version 1 --locked |
  jq -r '.packages[] | select(.name == "fandhe-workshop-web") | .features | keys[] | select(. != "default")')"
covered="$(printf '%s\n' "${WASM_FEATURE_SETS[@]}" | tr ',' '\n' | sed '/^$/d')"
# 一覧 $1 に含まれ、一覧 $2 に含まれない要素を空白区切りで出す（未クォート展開を避け 1 行ずつ読む）
list_missing() {
  local want="$1" have="$2" f out=""
  while IFS= read -r f; do
    [[ -z "${f}" ]] && continue
    if ! printf '%s\n' "${have}" | grep -qx -- "${f}"; then
      out="${out} ${f}"
    fi
  done < <(printf '%s\n' "${want}")
  printf '%s' "${out}"
}
missing="$(list_missing "${declared}" "${covered}")"
unknown="$(list_missing "${covered}" "${declared}")"
if [[ -n "${missing}${unknown}" ]]; then
  echo "error: web features not covered by lint (update WASM_FEATURE_SETS):${missing}; undeclared in set:${unknown}" >&2
  exit 1
fi

for set in "${WASM_FEATURE_SETS[@]}"; do
  if [[ -z "${set}" ]]; then
    log "cargo clippy (web, wasm32, features: none)"
    cargo clippy -p fandhe-workshop-web --target wasm32-unknown-unknown --locked -- -D warnings
  else
    log "cargo clippy (web, wasm32, features: ${set})"
    cargo clippy -p fandhe-workshop-web --target wasm32-unknown-unknown --locked --features "${set}" -- -D warnings
  fi
done
