#!/usr/bin/env bash
# CI で導入された rustc の版が docs/toolchain.md の固定値と一致することを確かめる
# （.github/workflows/ci.yml の rust-lint / rust-test から呼ばれる）。
#
# 理由: runner の rustup の版によっては rust-toolchain.toml を読まない、または自動導入しない場合がある。
# `make check-toolchain` はファイルどうしの照合だけで、実際に導入された版は見ないため、ここで補う。
# 版が違うまま lint・試験を通すと、固定していない版で検査したことになる。
#
# 終了コード: 0 = 一致 / 1 = 不一致 / 2 = 誤用、または検査を完了できない（rustc が無い・表が読めない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd rustc || exit 2
require_cmd cargo || exit 2

if ! expected="$(toolchain_value "${ROOT_DIR}/docs/toolchain.md" rust)"; then
  echo "error: docs/toolchain.md must have exactly one non-empty row for key: rust" >&2
  exit 2
fi

# リポジトリルートで実行し、rust-toolchain.toml が選ぶ版を見る。
cd "${ROOT_DIR}"
rustc --version
cargo --version
rustup show active-toolchain || true
rustup target list --installed || true

actual="$(rustc --version | awk '{ print $2 }')"
if [ "${actual}" != "${expected}" ]; then
  echo "::error::rustc ${actual} != ${expected} (docs/toolchain.md)"
  exit 1
fi
echo "rust toolchain ok (${actual})"
