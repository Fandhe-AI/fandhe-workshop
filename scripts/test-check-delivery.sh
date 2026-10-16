#!/usr/bin/env bash
# tools/check-delivery.sh（配信検査）自体の回帰テスト（`make test-check-delivery` から呼ばれる。実ビルドを行うため遅い）。
#
# 確かめること: 配信する設定は 0、試験の設定は 1（検査が空振りしていないことの対照）、配信物の写しに
# 試験用の口を 1 種類ずつ足すと 1、検査を完了できない入力（欠落・誤用）は 2。
# 作業ツリーは変えない。ビルドの出力と写しは mktemp -d の一時ディレクトリにだけ書き、終了時に消す。
# 前提: wasm-bindgen（docs/toolchain.md の版）と node が使えること。無ければ失敗する（成功扱いにしない）。
#
# 出力は TAP 風（scripts/test-check-dom-reads.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり / 2 = 誤用。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd node
require_cmd mktemp
require_cmd cp
require_cmd rm
require_cmd mkdir

CHECK="${ROOT_DIR}/tools/check-delivery.sh"
BUILD="${ROOT_DIR}/tools/build-web.sh"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-check-delivery.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0

# expect <name> <expected-rc> <command...>: コマンドの終了状態が期待と一致することを確かめる。
expect() {
  local name="$1" want="$2" rc=0
  shift 2
  "$@" >/dev/null 2>&1 || rc=$?
  if [ "${rc}" -eq "${want}" ]; then
    echo "ok - ${name} (exit ${rc})"
    passed=$((passed + 1))
  else
    echo "not ok - ${name} (expected exit ${want}, got ${rc})"
    failed=$((failed + 1))
  fi
}

# copy_tree <name>: 配信する設定の写しを作り、そのパスを出力する。
copy_tree() {
  local dest="${TMP_DIR}/$1"
  mkdir -p "${dest}"
  cp -R "${DELIVERY}/." "${dest}/"
  printf '%s' "${dest}"
}

DELIVERY="${TMP_DIR}/delivery"
TESTCFG="${TMP_DIR}/test"
log "building delivery config"
OUT_DIR="${DELIVERY}" "${BUILD}" >/dev/null
log "building test config"
OUT_DIR="${TESTCFG}" TEST_HOOKS=1 "${BUILD}" >/dev/null

expect "delivery config passes" 0 "${CHECK}" "${DELIVERY}"
expect "test config is rejected" 1 "${CHECK}" "${TESTCFG}"

T="$(copy_tree with-hooks-js)"
echo '// hooks' >"${T}/test-hooks.js"
expect "added test-hooks.js is detected" 1 "${CHECK}" "${T}"

T="$(copy_tree with-dts)"
echo 'export {};' >"${T}/wasm/extra.d.ts"
expect "added .d.ts is detected" 1 "${CHECK}" "${T}"

T="$(copy_tree with-link-hooks-js)"
echo '// hooks' >"${TMP_DIR}/link-target.js"
ln -s "${TMP_DIR}/link-target.js" "${T}/test-hooks.js"
expect "symlinked test-hooks.js is detected" 1 "${CHECK}" "${T}"

T="$(copy_tree with-link-dts)"
echo 'export {};' >"${TMP_DIR}/link-target.d.ts"
ln -s "${TMP_DIR}/link-target.d.ts" "${T}/wasm/extra.d.ts"
expect "symlinked .d.ts is detected" 1 "${CHECK}" "${T}"

T="$(copy_tree with-link-dir)"
mkdir "${TMP_DIR}/link-dir"
ln -s "${TMP_DIR}/link-dir" "${T}/linked"
expect "symlinked directory is detected" 1 "${CHECK}" "${T}"

T="$(copy_tree with-meta)"
echo '<meta name="fandhe-test-hooks" content="1">' >>"${T}/index.html"
expect "added test-hooks meta is detected" 1 "${CHECK}" "${T}"

T="$(copy_tree with-marker)"
echo '<!-- fandhe:test-hooks -->' >>"${T}/index.html"
expect "added test-hooks marker is detected" 1 "${CHECK}" "${T}"

T="$(copy_tree with-debug-js)"
echo '// debug_state_json' >>"${T}/wasm/fandhe_workshop_web.js"
expect "added debug_ in glue js is detected" 1 "${CHECK}" "${T}"

# inject_wasm <wasm> <文字列>: 末尾にカスタムセクション（id 0）を足す。単なる追記だと解析に失敗して exit 2 になり、
# 文字列の検出（exit 1）を確かめられないため、正しいセクションにする。パスと文字列は process.argv で渡す。
inject_wasm() {
  node -e '
    const fs = require("fs");
    const leb = (n) => { const o = []; do { let b = n & 0x7f; n >>>= 7; if (n) b |= 0x80; o.push(b); } while (n); return o; };
    const name = Buffer.from("probe");
    const body = Buffer.from(process.argv[2]);
    const payload = Buffer.concat([Buffer.from(leb(name.length)), name, body]);
    const sec = Buffer.concat([Buffer.from([0]), Buffer.from(leb(payload.length)), payload]);
    fs.appendFileSync(process.argv[1], sec);
  ' "$1" "$2"
}

T="$(copy_tree wasm-control)"
inject_wasm "${T}/wasm/fandhe_workshop_web_bg.wasm" "plain-marker"
expect "wasm with a valid custom section still passes" 0 "${CHECK}" "${T}"
for name in raw-trigger tick-trigger esc-trigger complete-trigger; do
  T="$(copy_tree "wasm-${name}")"
  inject_wasm "${T}/wasm/fandhe_workshop_web_bg.wasm" "${name}"
  expect "added ${name} in wasm is detected" 1 "${CHECK}" "${T}"
done

T="$(copy_tree with-payload-glue)"
echo '// data-payload' >>"${T}/wasm/fandhe_workshop_web.js"
expect "added data-payload in glue js is detected" 1 "${CHECK}" "${T}"

T="$(copy_tree with-payload-app)"
echo '// data-payload' >>"${T}/app.js"
expect "added data-payload in app.js is detected" 1 "${CHECK}" "${T}"

T="$(copy_tree with-trigger-html)"
echo '<button data-testid="tick-trigger"></button>' >>"${T}/index.html"
expect "added tick-trigger in index.html is detected" 1 "${CHECK}" "${T}"

T="$(copy_tree without-app-js)"
rm -f "${T}/app.js"
expect "missing app.js is exit 2" 2 "${CHECK}" "${T}"

expect "missing directory is exit 2" 2 "${CHECK}" "${TMP_DIR}/missing"
echo plain >"${TMP_DIR}/file"
expect "regular file is exit 2" 2 "${CHECK}" "${TMP_DIR}/file"
T="$(copy_tree without-wasm)"
rm -f "${T}/wasm/fandhe_workshop_web_bg.wasm"
expect "missing wasm is exit 2" 2 "${CHECK}" "${T}"
T="$(copy_tree broken-wasm)"
echo 'not wasm' >"${T}/wasm/fandhe_workshop_web_bg.wasm"
expect "unreadable wasm exports is exit 2" 2 "${CHECK}" "${T}"
expect "two arguments is exit 2" 2 "${CHECK}" "${DELIVERY}" "${TESTCFG}"

echo "1..$((passed + failed))"
echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
