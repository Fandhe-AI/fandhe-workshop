#!/usr/bin/env bash
# scripts/check-no-egress.sh の回帰テスト（`make test-check-no-egress` から呼ばれる。ビルドなし・通信なし）。
#
# 合成のメタデータ・package.json・配信物で、終了コード 0（合格）・1（違反）・2（判定不能）を確かめる。
# 配信物の合成は、import だけを持つ最小の WASM をバイト列から作る（node の WebAssembly を使う）。
# 終了コード: 0 = すべて成功 / 1 = 失敗が 1 件以上。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd node
require_cmd jq

CHECK="${DIR}/check-no-egress.sh"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-check-no-egress.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

FAILURES=0
# expect <説明> <期待する終了状態> <コマンド...>
expect() {
  local desc="$1" want="$2" rc=0
  shift 2
  "$@" >/dev/null 2>&1 || rc=$?
  if [ "${rc}" -eq "${want}" ]; then
    echo "ok: ${desc}" >&2
  else
    echo "FAIL: ${desc} (exit ${rc}, want ${want})" >&2
    FAILURES=$((FAILURES + 1))
  fi
}

# --- deps ---
printf '%s' '{"packages":[{"name":"serde"},{"name":"web-sys"}]}' >"${TMP}/native-ok.json"
printf '%s' '{"packages":[{"name":"serde"},{"name":"reqwest"}]}' >"${TMP}/native-reqwest.json"
printf '%s' '{"packages":[{"name":"serde"},{"name":"sentry-core"}]}' >"${TMP}/native-sentry.json"
mkwasm() { # mkwasm <out> <features の JSON 配列>
  printf '{"resolve":{"nodes":[{"id":"registry+x#web-sys@0.3.0","features":%s},{"id":"registry+x#js-sys@0.3.0","features":[]}]}}' "$2" >"$1"
}
mkwasm "${TMP}/wasm-ok.json" '["Storage","IdbFactory","Navigator"]'
mkwasm "${TMP}/wasm-bad.json" '["Storage","IdbFactory","WebSocket"]'
mkwasm "${TMP}/wasm-noanchor.json" '["Navigator"]'
printf '%s' '{"resolve":{"nodes":[{"id":"registry+x#js-sys@0.3.0","features":[]}]}}' >"${TMP}/wasm-nowebsys.json"
printf '%s' '{"devDependencies":{"playwright":"1.0.0"}}' >"${TMP}/pkg-ok.json"
printf '%s' '{"dependencies":{"x":"1.0.0"}}' >"${TMP}/pkg-bad.json"
printf '%s' '{broken' >"${TMP}/broken.json"

deps() { "${CHECK}" deps --metadata "$1" --metadata-wasm "$2" --package-json "$3"; }
expect "deps: clean input passes" 0 deps "${TMP}/native-ok.json" "${TMP}/wasm-ok.json" "${TMP}/pkg-ok.json"
expect "deps: reqwest is denied" 1 deps "${TMP}/native-reqwest.json" "${TMP}/wasm-ok.json" "${TMP}/pkg-ok.json"
expect "deps: sentry prefix is denied" 1 deps "${TMP}/native-sentry.json" "${TMP}/wasm-ok.json" "${TMP}/pkg-ok.json"
expect "deps: web-sys WebSocket feature is denied" 1 deps "${TMP}/native-ok.json" "${TMP}/wasm-bad.json" "${TMP}/pkg-ok.json"
expect "deps: npm dependencies is denied" 1 deps "${TMP}/native-ok.json" "${TMP}/wasm-ok.json" "${TMP}/pkg-bad.json"
expect "deps: missing anchor features is undecidable" 2 deps "${TMP}/native-ok.json" "${TMP}/wasm-noanchor.json" "${TMP}/pkg-ok.json"
expect "deps: missing web-sys node is undecidable" 2 deps "${TMP}/native-ok.json" "${TMP}/wasm-nowebsys.json" "${TMP}/pkg-ok.json"
expect "deps: broken metadata is undecidable" 2 deps "${TMP}/broken.json" "${TMP}/wasm-ok.json" "${TMP}/pkg-ok.json"
expect "deps: broken package.json is undecidable" 2 deps "${TMP}/native-ok.json" "${TMP}/wasm-ok.json" "${TMP}/broken.json"
expect "deps: only one of the two metadata options is misuse" 2 "${CHECK}" deps --metadata "${TMP}/native-ok.json"
expect "deps: unknown option is misuse" 2 "${CHECK}" deps --bogus
expect "usage: no mode is misuse" 2 "${CHECK}"
expect "usage: unknown mode is misuse" 2 "${CHECK}" nope
expect "deps: the real repository passes" 0 "${CHECK}" deps

# --- dist ---
# mkwasm_file <out> <import 名...>: 関数 import だけを持つ WASM を書く。
mkwasm_file() {
  local out="$1"
  shift
  node -e '
const [out, ...names] = process.argv.slice(1);
const enc = (s) => Buffer.from(s, "utf8");
const u = (n) => { const b = []; do { let x = n & 0x7f; n >>>= 7; if (n) x |= 0x80; b.push(x); } while (n); return Buffer.from(b); };
const str = (s) => Buffer.concat([u(enc(s).length), enc(s)]);
const imports = Buffer.concat([u(names.length), ...names.map((n) => Buffer.concat([str("m"), str(n), Buffer.from([0x00]), u(0)]))]);
const types = Buffer.concat([u(1), Buffer.from([0x60, 0x00, 0x00])]);
const sec = (id, body) => Buffer.concat([Buffer.from([id]), u(body.length), body]);
const wasm = Buffer.concat([Buffer.from([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]), sec(1, types), sec(2, imports)]);
require("fs").writeFileSync(out, wasm);
' "${out}" "$@"
}
mkdist() { # mkdist <dir>: 合格の配信物
  mkdir -p "$1/wasm"
  echo '<!doctype html><title>x</title>' >"$1/index.html"
  echo 'body{}' >"$1/app.css"
  echo '<svg/>' >"$1/icon.svg"
  printf 'const r = await fetch("./ir/main.json");\n' >"$1/app.js"
  printf 'module_or_path = fetch(module_or_path);\n' >"$1/wasm/fandhe_workshop_web.js"
  mkwasm_file "$1/wasm/fandhe_workshop_web_bg.wasm" __wbg_setItem_0123456789abcdef __wbg_indexedDB_0123456789abcdef
}
mkdist "${TMP}/d-ok"
expect "dist: clean dist passes" 0 "${CHECK}" dist "${TMP}/d-ok"

cp -R "${TMP}/d-ok" "${TMP}/d-ir"
mkdir -p "${TMP}/d-ir/ir"
echo '{}' >"${TMP}/d-ir/ir/main.json"
expect "dist: optional ir/main.json passes" 0 "${CHECK}" dist "${TMP}/d-ir"

cp -R "${TMP}/d-ok" "${TMP}/d-extra"
echo 'self.addEventListener("fetch",()=>{})' >"${TMP}/d-extra/sw.js"
expect "dist: extra file is denied" 1 "${CHECK}" dist "${TMP}/d-extra"

cp -R "${TMP}/d-ok" "${TMP}/d-link"
ln -s app.css "${TMP}/d-link/link.css"
expect "dist: symbolic link is denied" 1 "${CHECK}" dist "${TMP}/d-link"

cp -R "${TMP}/d-ok" "${TMP}/d-ws"
echo 'new WebSocket("x")' >>"${TMP}/d-ws/wasm/fandhe_workshop_web.js"
expect "dist: WebSocket in glue is denied" 1 "${CHECK}" dist "${TMP}/d-ws"

cp -R "${TMP}/d-ok" "${TMP}/d-beacon"
echo 'navigator.sendBeacon("x")' >>"${TMP}/d-beacon/app.js"
expect "dist: sendBeacon in app.js is denied" 1 "${CHECK}" dist "${TMP}/d-beacon"

cp -R "${TMP}/d-ok" "${TMP}/d-fetch2"
echo 'await fetch("./ir/main.json");' >>"${TMP}/d-fetch2/app.js"
expect "dist: second fetch in app.js is denied" 1 "${CHECK}" dist "${TMP}/d-fetch2"

cp -R "${TMP}/d-ok" "${TMP}/d-fetcharg"
printf 'const r = await fetch("./other.json");\n' >"${TMP}/d-fetcharg/app.js"
expect "dist: fetch with another argument is denied" 1 "${CHECK}" dist "${TMP}/d-fetcharg"

cp -R "${TMP}/d-ok" "${TMP}/d-url"
echo '<img src="https://example.invalid/a.png">' >>"${TMP}/d-url/index.html"
expect "dist: external URL is denied" 1 "${CHECK}" dist "${TMP}/d-url"

mkdist "${TMP}/d-imp"
mkwasm_file "${TMP}/d-imp/wasm/fandhe_workshop_web_bg.wasm" __wbg_setItem_0123456789abcdef __wbg_indexedDB_0123456789abcdef __wbg_sendBeacon_0123456789abcdef
expect "dist: sendBeacon WASM import is denied" 1 "${CHECK}" dist "${TMP}/d-imp"

mkdist "${TMP}/d-noanchor"
mkwasm_file "${TMP}/d-noanchor/wasm/fandhe_workshop_web_bg.wasm" __wbg_getItem_0123456789abcdef
expect "dist: missing anchor import is undecidable" 2 "${CHECK}" dist "${TMP}/d-noanchor"

cp -R "${TMP}/d-ok" "${TMP}/d-missing"
rm "${TMP}/d-missing/app.css"
expect "dist: missing required file is undecidable" 2 "${CHECK}" dist "${TMP}/d-missing"
expect "dist: missing directory is undecidable" 2 "${CHECK}" dist "${TMP}/nonexistent"
expect "dist: no argument is misuse" 2 "${CHECK}" dist

if [ "${FAILURES}" -ne 0 ]; then
  echo "error: ${FAILURES} check(s) failed" >&2
  exit 1
fi
log "ok: all check-no-egress regression checks passed"
