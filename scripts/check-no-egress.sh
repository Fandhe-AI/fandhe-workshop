#!/usr/bin/env bash
# 受講者の入力・履歴を外部へ送る手段（通信の機能・学習分析・提出・進捗の同期）と、初回の配布物が持たない機能
# （EXT-1 実 CLI への橋渡し・EXT-2 backend／提出／学習分析・EXT-3 LLM の必須化）が、依存と配信物に無いことを機械検査する
# （SAFE-16・TASK-153、EXT-1〜EXT-3・TASK-185 1/2）。違反の行には区分（[SAFE-16]・[EXT-1 cli-bridge] など）を前置する。
# ブラウザでの通信の計数は tools/browser/egress-count.test.mjs が受け持ち、こちらは静的な層。
# L2 の起動の引数（miniserve の追加機能を有効にしないこと）は tools/build-l2.test.mjs が完全一致で固定しており、ここでは重複して検査しない。
# pack（作成者向け CLI）が拒否の crate を正当に要する場合は、この一覧の見直しとして別の変更で扱う。
#
# 呼び出し元: `make check-no-egress`（deps）・`make check-no-egress-dist`（dist）・CI の rust-lint ジョブ（deps と回帰テスト）・
# scripts/test-build-web.sh（配信する設定の実ビルドへの dist）。回帰テストは scripts/test-check-no-egress.sh。
#
# 使い方:
#   check-no-egress.sh deps [--metadata <native.json> --metadata-wasm <wasm32.json> --package-json <path>]
#     Cargo の解決済みの依存に通信・学習分析の crate が無いこと、wasm32 で解決した web-sys の feature に通信の型が無いこと、
#     package.json に配信へ入りうる `dependencies` が無いことを検査する。引数は回帰テストが合成の入力を渡すためだけに使う。
#   check-no-egress.sh dist <dist-dir>
#     配信する設定の出力（tools/build-web.sh）を読むだけで検査する（変更・削除はしない）。
# 拒否の一覧・許可の件数は下の定数で、環境変数・引数では緩められない（規則の変更はこのファイルの変更として見える）。
#
# 終了コード: 0 = 違反なし / 1 = 違反あり / 2 = 誤用、または検査を完了できない（ツール欠落・入力の破損・錨の欠落）。
# 2 でも合格にしない（fail-closed）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

usage() {
  echo "usage: $(basename "$0") deps [--metadata <json> --metadata-wasm <json> --package-json <path>] | dist <dist-dir>" >&2
  exit 2
}

# 拒否する crate（完全一致と前方一致）。HTTP クライアント・ソケット・学習分析・計測の送信の crate。2026-10-29。
readonly DENY_CRATES_EXACT='reqwest gloo-net gloo-net-macros ureq hyper surf isahc attohttpc ehttp tungstenite tokio-tungstenite ws_stream_wasm web-socket segment mixpanel amplitude rudderstack curl curl-sys'
readonly DENY_CRATES_PREFIX='sentry opentelemetry posthog'
# 初回の配布物が持たない機能の crate（EXT-1〜EXT-3）。行の形は「区分|完全一致（空白区切り）|前方一致（空白区切り）」。名前の列挙で網羅ではない（known-gaps.md）。2026-10-29。
readonly EXT_CRATES='EXT-1 cli-bridge|portable-pty pty-process duct subprocess ssh2 russh openssh interprocess native-messaging tokio-process|wasmer-wasi
EXT-2 backend|axum actix-web actix-http warp rocket tide poem salvo tower-http sqlx diesel rusqlite redis mongodb postgres tokio-postgres oauth2 openidconnect jsonwebtoken|aws-sdk- google-cloud- firebase supabase
EXT-2 analytics|plausible matomo countly|datadog
EXT-3 llm|async-openai openai openai-api-rs anthropic anthropic-sdk ollama-rs llm llm-chain genai langchain-rust ort tch tokenizers rust-bert kalosm mistralrs|candle- llama whisper-'
# 初回の配布物が持たない機能の web-sys feature（完全一致|前方一致）。
readonly EXT_WEBSYS='EXT-1 cli-bridge|Usb UsbDevice Serial SerialPort Hid HidDevice Bluetooth BluetoothDevice FileSystemDirectoryHandle FileSystemFileHandle FileSystemWritableFileStream|
EXT-2 backend|CredentialsContainer Credential PublicKeyCredential PasswordCredential HtmlFormElement HtmlDocument|
EXT-3 llm||Gpu Ml'
# 初回の配布物が持たない機能の WASM import の名前（__wbg_ と末尾のハッシュを除いた完全一致）。
readonly EXT_WASM_IMPORTS='EXT-1 cli-bridge|requestDevice requestPort usb serial hid bluetooth showDirectoryPicker showOpenFilePicker showSaveFilePicker registerProtocolHandler connectNative
EXT-2 backend|credentials cookie set_cookie submit requestSubmit
EXT-3 llm|gpu ml LanguageModel'
# glue JS と app.js の拒否トークン（区分|ERE）。
readonly EXT_JS_PATTERNS='EXT-1 cli-bridge|\bconnectNative\b|chrome\.runtime|navigator\.(usb|serial|hid|bluetooth)\b|show(Directory|OpenFile|SaveFile)Picker|registerProtocolHandler
EXT-2 backend|navigator\.credentials|PublicKeyCredential|document\.cookie|\.submit\(|requestSubmit|mailto:
EXT-2 analytics|\bgtag\b|\bdataLayer\b|\b_paq\b|\bplausible\b|\bmixpanel\b|\bposthog\b|\bSentry\b
EXT-3 llm|\bLanguageModel\b|navigator\.gpu|navigator\.ml\b|(window|self)\.ai\b|\bopenai\b|\banthropic\b|\bollama\b|chat/completions'
# index.html の提出の形（EXT-2 submission）。
readonly EXT_HTML_PATTERN='<form[ >]|mailto:'
# 拒否する web-sys の feature（通信の手段になる型）。Navigator の sendBeacon は feature で区別できないため dist の検査と実行時の計数が受け持つ。
readonly DENY_WEBSYS_FEATURES='Request RequestInit Response Headers FormData XmlHttpRequest XmlHttpRequestUpload WebSocket EventSource RtcPeerConnection RtcDataChannel ServiceWorkerContainer ServiceWorkerRegistration Worker SharedWorker WebTransport PushManager'
# web-sys の解決結果が空振りしていないことの錨（保存先の型）。
readonly ANCHOR_WEBSYS_FEATURES='Storage IdbFactory'
# 配信物の構成要素（ir/main.json は任意）。
readonly DIST_FILES='app.css app.js icon.svg index.html wasm/fandhe_workshop_web.js wasm/fandhe_workshop_web_bg.wasm'
readonly DIST_OPTIONAL='ir/main.json'
# 通信の手段のトークン（glue JS と app.js で 0 件であること）。fetch は別に件数を固定する。
readonly DENY_JS_TOKENS='XMLHttpRequest WebSocket EventSource sendBeacon RTCPeerConnection serviceWorker importScripts SharedWorker WebTransport'
# WASM の import で拒否する名前（wasm-bindgen が付ける接頭辞 __wbg_ と末尾のハッシュを除いた名前の完全一致）。
readonly DENY_WASM_IMPORTS='fetch sendBeacon send XMLHttpRequest WebSocket EventSource RTCPeerConnection serviceWorker Worker set_href assign replace'
# 保存先の import が有ること（空振りの防止）。
readonly ANCHOR_WASM_IMPORTS='setItem indexedDB'
# glue JS の `fetch(` は wasm-bindgen の初期化の 1 件だけ。app.js の `fetch(` は ./ir/main.json の 1 件だけ（2026-10-29 の実測）。
readonly GLUE_FETCH_COUNT=1
readonly APP_FETCH_COUNT=1

VIOLATIONS=0
tmp=""
violation() {
  echo "error: $*" >&2
  VIOLATIONS=$((VIOLATIONS + 1))
}
finish() {
  if [ "${VIOLATIONS}" -ne 0 ]; then
    exit 1
  fi
}

# count_matches <ERE> <file>: 一致する行の数を標準出力へ。grep の 1（不一致）は 0 件、2 以上は検査失敗として 2 で止まる。
count_matches() {
  local n rc=0
  n="$(grep -cE -- "$1" "$2")" || rc=$?
  if [ "${rc}" -eq 1 ]; then
    echo 0
    return 0
  elif [ "${rc}" -ne 0 ]; then
    echo "error: grep failed on $2" >&2
    exit 2
  fi
  echo "${n}"
}

# count_occurrences <ERE> <file>: 一致の出現数（同じ行に複数あっても数える）を標準出力へ。失敗の扱いは count_matches と同じ。
count_occurrences() {
  local n rc=0
  n="$(grep -oE -- "$1" "$2")" || rc=$?
  if [ "${rc}" -eq 1 ]; then
    echo 0
    return 0
  elif [ "${rc}" -ne 0 ]; then
    echo "error: grep failed on $2" >&2
    exit 2
  fi
  printf '%s\n' "${n}" | wc -l | tr -d '[:space:]'
  echo
}

check_deps() {
  local native="" wasm="" pkg="${ROOT_DIR}/package.json"
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --metadata) [ "$#" -ge 2 ] || usage; native="$2"; shift 2 ;;
      --metadata-wasm) [ "$#" -ge 2 ] || usage; wasm="$2"; shift 2 ;;
      --package-json) [ "$#" -ge 2 ] || usage; pkg="$2"; shift 2 ;;
      *) usage ;;
    esac
  done
  require_cmd jq || exit 2
  local line f
  tmp="$(mktemp -d)"
  trap 'rm -rf "${tmp}"' EXIT
  if [ -z "${native}" ] && [ -z "${wasm}" ]; then
    require_cmd cargo || exit 2
    native="${tmp}/native.json"
    wasm="${tmp}/wasm.json"
    # --locked --offline: Cargo.lock を書き換えず通信もしない。
    (cd "${ROOT_DIR}" && cargo metadata --format-version 1 --locked --offline >"${native}" 2>"${tmp}/c1.err") || {
      echo "error: cargo metadata failed" >&2
      head -n 5 "${tmp}/c1.err" >&2
      exit 2
    }
    (cd "${ROOT_DIR}" && cargo metadata --format-version 1 --locked --offline --filter-platform wasm32-unknown-unknown >"${wasm}" 2>"${tmp}/c2.err") || {
      echo "error: cargo metadata (wasm32) failed" >&2
      head -n 5 "${tmp}/c2.err" >&2
      exit 2
    }
  elif [ -z "${native}" ] || [ -z "${wasm}" ]; then
    usage
  fi
  [ -f "${pkg}" ] || { echo "error: package.json not found" >&2; exit 2; }

  jq -e '(.packages | type == "array") and (.packages | length) > 0' "${native}" >/dev/null 2>&1 || { echo "error: unreadable metadata" >&2; exit 2; }
  jq -e '(.resolve.nodes | type == "array") and (.resolve.nodes | length) > 0' "${wasm}" >/dev/null 2>&1 || { echo "error: unreadable wasm32 metadata" >&2; exit 2; }
  jq -e 'type == "object"' "${pkg}" >/dev/null 2>&1 || { echo "error: unreadable package.json" >&2; exit 2; }

  # crate 名。dev 依存・build 依存を含む全パッケージで厳しい側に判定する。
  jq -r --arg exact "${DENY_CRATES_EXACT}" --arg prefix "${DENY_CRATES_PREFIX}" '
    ($exact | split(" ")) as $e | ($prefix | split(" ")) as $p
    | .packages[].name as $n
    | select(($e | index($n)) or ($p | any(. as $x | $n | startswith($x))))
    | "[SAFE-16] denied crate in dependency graph: \($n)"' "${native}" >"${tmp}/v1.txt" || exit 2
  while IFS='|' read -r tag ex pre; do
    [ -n "${tag}" ] || continue
    jq -r --arg tag "${tag}" --arg exact "${ex}" --arg prefix "${pre}" '
      ($exact | split(" ")) as $e | ($prefix | split(" ") | map(select(length > 0))) as $p
      | .packages[].name as $n
      | select(($e | index($n)) or ($p | any(. as $x | $n | startswith($x))))
      | "[\($tag)] denied crate in dependency graph: \($n)"' "${native}" >>"${tmp}/v1.txt" || exit 2
  done <<<"${EXT_CRATES}"
  # web-sys の feature。ノードがちょうど 1 つで、錨の feature を含むこと。
  jq -r '[.resolve.nodes[] | select(.id | test("web-sys[@ ]|#web-sys$"))] | length' "${wasm}" >"${tmp}/n.txt" || exit 2
  if [ "$(cat "${tmp}/n.txt")" != "1" ]; then
    echo "error: expected exactly one web-sys node in the wasm32 resolve (found $(cat "${tmp}/n.txt"))" >&2
    exit 2
  fi
  jq -r --arg deny "${DENY_WEBSYS_FEATURES}" --arg anchor "${ANCHOR_WEBSYS_FEATURES}" '
    [.resolve.nodes[] | select(.id | test("web-sys[@ ]|#web-sys$")) | .features[]] as $f
    | (($anchor | split(" ")) | map(select(. as $a | ($f | index($a)) | not))) as $missing
    | if ($missing | length) > 0 then "ANCHOR-MISSING"
      else ($deny | split(" "))[] | select(. as $d | $f | index($d)) | "denied web-sys feature: \(.)" end' "${wasm}" >"${tmp}/v2.txt" || exit 2
  if ! grep -qx 'ANCHOR-MISSING' "${tmp}/v2.txt"; then
    sed -i.bak 's/^denied web-sys feature/[SAFE-16] denied web-sys feature/' "${tmp}/v2.txt" || exit 2
    while IFS='|' read -r tag ex pre; do
      [ -n "${tag}" ] || continue
      jq -r --arg tag "${tag}" --arg exact "${ex}" --arg prefix "${pre}" '
        [.resolve.nodes[] | select(.id | test("web-sys[@ ]|#web-sys$")) | .features[]] as $f
        | ($exact | split(" ") | map(select(length > 0))) as $e | ($prefix | split(" ") | map(select(length > 0))) as $p
        | $f[] | . as $n
        | select(($e | index($n)) or ($p | any(. as $x | $n | startswith($x))))
        | "[\($tag)] denied web-sys feature: \($n)"' "${wasm}" >>"${tmp}/v2.txt" || exit 2
    done <<<"${EXT_WEBSYS}"
  fi
  if grep -qx 'ANCHOR-MISSING' "${tmp}/v2.txt"; then
    echo "error: web-sys anchor features (${ANCHOR_WEBSYS_FEATURES}) are missing; the check would be vacuous" >&2
    exit 2
  fi
  # package.json: 配信へ入りうる dependencies の欄が無いこと。
  jq -r 'if has("dependencies") then "[SAFE-16] package.json must not have dependencies (devDependencies only)" else empty end' "${pkg}" >"${tmp}/v3.txt" || exit 2
  for f in v1 v2 v3; do
    while IFS= read -r line; do
      [ -n "${line}" ] || continue
      violation "${line}"
    done <"${tmp}/${f}.txt"
  done
  finish
  log "ok: deps: SAFE-16, EXT-1, EXT-2, EXT-3 checked - no denied crate, no denied web-sys feature, no npm dependencies"
}

check_dist() {
  [ "$#" -eq 1 ] || usage
  local d="$1" f tok n imports want
  require_cmd node || exit 2
  if [ ! -d "${d}" ] || [ -L "${d}" ]; then
    echo "error: not a directory: ${d}" >&2
    exit 2
  fi
  for f in ${DIST_FILES}; do
    [ -f "${d}/${f}" ] || { echo "error: missing ${f} in dist" >&2; exit 2; }
  done
  # 1. ファイルの一覧（シンボリックリンクと余分なファイルを拒む）。NUL 区切りで読む。
  want=" ${DIST_FILES} ${DIST_OPTIONAL} "
  while IFS= read -r -d '' f; do
    f="${f#./}"
    case "${want}" in
      *" ${f} "*) ;;
      *) violation "[SAFE-16] unexpected file in dist: ${f}" ;;
    esac
  done < <(cd "${d}" && find . -mindepth 1 \( -type f -o -type l \) -print0)
  if [ -n "$(cd "${d}" && find . -type l -print -quit)" ]; then
    violation "[SAFE-16] symbolic link in dist"
  fi
  # 2. WASM の import。
  imports="$(node -e '
const fs=require("fs");
const m=new WebAssembly.Module(fs.readFileSync(process.argv[1]));
for(const i of WebAssembly.Module.imports(m)) console.log(i.name.replace(/^__wbg_/,"").replace(/_[0-9a-f]{16}$/,""));
' "${d}/wasm/fandhe_workshop_web_bg.wasm")" || { echo "error: cannot read WASM imports" >&2; exit 2; }
  [ -n "${imports}" ] || { echo "error: WASM has no imports; the check would be vacuous" >&2; exit 2; }
  for tok in ${ANCHOR_WASM_IMPORTS}; do
    printf '%s\n' "${imports}" | grep -qxF -- "${tok}" || { echo "error: anchor import ${tok} not found in WASM; the check would be vacuous" >&2; exit 2; }
  done
  for tok in ${DENY_WASM_IMPORTS}; do
    if printf '%s\n' "${imports}" | grep -qxF -- "${tok}"; then
      violation "[SAFE-16] denied WASM import: ${tok}"
    fi
  done
  while IFS='|' read -r tag names; do
    [ -n "${tag}" ] || continue
    for tok in ${names}; do
      if printf '%s\n' "${imports}" | grep -qxF -- "${tok}"; then
        violation "[${tag}] denied WASM import: ${tok}"
      fi
    done
  done <<<"${EXT_WASM_IMPORTS}"
  # 3. glue JS と app.js の通信のトークン。
  for f in wasm/fandhe_workshop_web.js app.js; do
    for tok in ${DENY_JS_TOKENS}; do
      n="$(count_matches "\\b${tok}\\b" "${d}/${f}")"
      [ "${n}" -eq 0 ] || violation "[SAFE-16] ${f} mentions ${tok} (${n} lines)"
    done
    n="$(count_matches '\bnew +Worker\b' "${d}/${f}")"
    [ "${n}" -eq 0 ] || violation "[SAFE-16] ${f} creates a Worker (${n} lines)"
    while IFS='|' read -r tag pat; do
      [ -n "${tag}" ] || continue
      n="$(count_matches "${pat}" "${d}/${f}")"
      [ "${n}" -eq 0 ] || violation "[${tag}] ${f} matches a denied pattern (${n} lines)"
    done <<<"${EXT_JS_PATTERNS}"
  done
  n="$(count_occurrences '\bfetch\(' "${d}/wasm/fandhe_workshop_web.js")"
  [ "${n}" -eq "${GLUE_FETCH_COUNT}" ] || violation "glue js has ${n} fetch( calls (expected ${GLUE_FETCH_COUNT})"
  n="$(count_occurrences '\bfetch\(' "${d}/app.js")"
  [ "${n}" -eq "${APP_FETCH_COUNT}" ] || violation "app.js has ${n} fetch( calls (expected ${APP_FETCH_COUNT})"
  n="$(count_occurrences 'fetch\("\./ir/main\.json"\)' "${d}/app.js")"
  [ "${n}" -eq "${APP_FETCH_COUNT}" ] || violation "app.js fetch argument is not the fixed ./ir/main.json"
  n="$(count_matches "${EXT_HTML_PATTERN}" "${d}/index.html")"
  [ "${n}" -eq 0 ] || violation "[EXT-2 submission] index.html has ${n} lines with a form or mailto"
  # 4. 外部の参照（http(s):// と // 始まり）。
  for f in index.html app.js app.css; do
    n="$(count_matches 'https?://|[("'"'"'= ]//[A-Za-z0-9-]+\.[A-Za-z]' "${d}/${f}")"
    [ "${n}" -eq 0 ] || violation "[SAFE-16] ${f} has ${n} lines with an external reference"
  done
  finish
  log "ok: dist: SAFE-16, EXT-1, EXT-2, EXT-3 checked - only the expected files and no network capability"
}

[ "$#" -ge 1 ] || usage
mode="$1"
shift
case "${mode}" in
  deps) check_deps "$@" ;;
  dist) check_dist "$@" ;;
  *) usage ;;
esac
