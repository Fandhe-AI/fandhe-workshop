#!/usr/bin/env bash
# fandhe-pack の debug ビルドを行い、実行ファイルの絶対パスを標準出力へ 1 行だけ書く。
#
# 目的: ブラウザ試験の前提バイナリの位置を、`target/debug/fandhe-pack` の直書きにしない（BUILD-16。
# PoC-11 の F-10。CARGO_TARGET_DIR を別にすると ENOENT で止まった）。位置は cargo 自身の出力から解決するため、
# CARGO_TARGET_DIR・.cargo/config の build.target-dir のどちらにも追従する。
# 呼び出し元: scripts/test-browser.sh（結果を FANDHE_PACK_BIN として node:test へ渡す）。回帰テストは test-build-pack-bin.sh。
# ログ・診断は stderr のみ。解決できない場合は未検査の成功にせず失敗する（fail-closed）。
#
# 終了コード: 0 = 成功 / 1 = ビルド失敗・位置の解決失敗 / 2 = 誤用（引数あり）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo
require_cmd jq

cd "${ROOT_DIR}"

# cargo の標準出力（JSON）を先に変数へ取る。ビルド失敗を set -e で検出するため、パイプにしない。
messages="$(cargo build --locked -p fandhe-workshop-pack --bin fandhe-pack --message-format=json-render-diagnostics)"

bin="$(printf '%s\n' "${messages}" | jq -r 'select(.reason == "compiler-artifact" and .target.name == "fandhe-pack" and .executable != null) | .executable')"

if [ -z "${bin}" ] || [ "$(printf '%s\n' "${bin}" | wc -l | tr -d ' ')" != "1" ]; then
  echo "error: could not resolve a single fandhe-pack executable from cargo output" >&2
  exit 1
fi
if [ ! -f "${bin}" ] || [ ! -x "${bin}" ]; then
  echo "error: resolved fandhe-pack is not an executable file: ${bin}" >&2
  exit 1
fi

printf '%s\n' "${bin}"
