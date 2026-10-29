#!/usr/bin/env bash
# 配信する設定の配信物に試験用の口が無いことの検査（SHELL-35・BUILD-27。`make check-delivery` から呼ばれる）。
#
# 目的: `make build-web`（配信する設定）が作る dist/web に、試験用の口（debug_* の書き出し・test-hooks.js・
# .d.ts・test-hooks の meta）が入っていないことを機械で保証する（TASK-81 の 2/2）。
# 先に `make build-web` で配信物を作っておく。CI にはまだ組み込まない（wasm のビルドが要るため）。
#
# 失敗にする検査（すべて固定の文字列・名前で、配信物の読み取りだけを行う。ファイルは変更・削除しない）:
#   1. WASM の書き出しに mount_app がある（書き出しの取得が空振りしていないことの錨）
#   2. WASM の書き出しに debug_ で始まる名前が無い
#   3. JS の glue（wasm/fandhe_workshop_web.js）に debug_ が無い
#   4. index.html に fandhe-test-hooks（meta）と fandhe:test-hooks（印）が無い
#   5. test-hooks.js が配信物に無い
#   6. 配信物のどこにも *.d.ts が無い
#   7. WASM のバイト列に debug_ と raw-trigger が無い
#   8. 配信物の中にシンボリックリンクが無い（find -type f はリンクを列挙せず、名前の検査（5・6）をすり抜けるため。
#      tools/serve.mjs は配信ルート内のリンク先を配信するので、リンクは到達可能な経路になる）
#   9a. WASM のバイト列に代役の受け口の名 fandhe-test-receptor-probe が無い（TASK-191 2/2）
#   9. WASM のバイト列に隠し要素の名前 tick-trigger・esc-trigger・complete-trigger が無い（TASK-349 2/2）
#  10. 配信する JS・HTML（glue の JS・app.js・index.html）に、上の 3 名と data-payload が無い（TASK-349 2/2）
#  11. index.html と app.js に <style 要素と style= 属性が無い（TASK-469 1/2。SAFE-15 の静的な層）。
#      app.css の存在を必須にする（欠けていれば 2）。WASM のバイト列と glue の JS は、依存由来の文字列で 0 件に
#      ならない可能性があるため数えず、描画後の DOM の数え上げ（ブラウザ試験）に任せる。
# 7 は 2026-10-15 に配信する設定の release wasm で実測し、どちらも 0 件だった（std・依存由来の文字列も残らない）。
# 将来 0 件でなくなった場合は、試験の設定との対照つきの検査（TASK-82）へ移す。
# 9・10 は 2026-10-16 に配信する設定の release 一式で実測し、いずれも 0 件だった（WASM の data-payload は 1 件）。
#
# data-payload を WASM のバイト列で数えない理由（TASK-349 の「data-payload の経路 0 件」の解釈）: 左の成果物の指せる行
# （data-action="ui"。artifact_view の PAYLOAD_ATTR）が正当に出力する属性名であり、同じ文字列リテラルは 1 つに
# まとめられるため、件数では新しい経路を検出できない。「経路」は、進行の宛先（tick・complete・esc）を data-payload に
# 書いて click する形と定義し、WASM の外の層で塞ぐ。配信する JS・HTML は 10 で 0 件にし、ソースは
# crates/web/tests/no_hidden_triggers.rs（native の静的検査）が、許可した箇所の件数固定で守る。
#
# 範囲: 試験の設定との対照、試験用クエリ、Action::Raw、試験用 IR の検査は TASK-82 が拡張する。
# scripts/test-build-web.sh の (a) も同種の項目を持つ。一本化は TASK-82 で行う。
#
# 使い方: tools/check-delivery.sh [<dist dir>]（既定は <repo>/dist/web）
# 終了コード: 0 = 試験用の口なし / 1 = 見つかった / 2 = 誤用、または検査を完了できない
#   （ディレクトリや必須ファイルが無い・node が無い・書き出しの取得に失敗・書き出しが空）。2 でも合格にしない（fail-closed）。
#   回帰テストは scripts/test-check-delivery.sh（`make test-check-delivery`。実ビルドを行うため遅い）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../scripts/lib.sh
source "${DIR}/../scripts/lib.sh"

if [ "$#" -gt 1 ]; then
  echo "usage: $(basename "$0") [<dist dir>]" >&2
  exit 2
fi
DIST="${1:-${ROOT_DIR}/dist/web}"

require_cmd node || exit 2
require_cmd grep || exit 2
require_cmd find || exit 2

if [ ! -d "${DIST}" ]; then
  echo "error: dist directory not found: ${DIST} (run \`make build-web\` first)" >&2
  exit 2
fi
WASM="${DIST}/wasm/fandhe_workshop_web_bg.wasm"
GLUE_JS="${DIST}/wasm/fandhe_workshop_web.js"
INDEX="${DIST}/index.html"
APP_JS="${DIST}/app.js"
APP_CSS="${DIST}/app.css"
for f in "${WASM}" "${GLUE_JS}" "${INDEX}" "${APP_JS}" "${APP_CSS}"; do
  if [ ! -f "${f}" ]; then
    echo "error: required file not found: ${f} (run \`make build-web\` first)" >&2
    exit 2
  fi
done

VIOLATIONS=0
ok() {
  echo "ok: $*"
}
violation() {
  echo "FAIL: $*"
  VIOLATIONS=$((VIOLATIONS + 1))
}

# no_match <説明> <grep のオプション> <パターン> <ファイル>: grep の終了状態 1（不一致）だけを合格にする。
# 0 は違反、2 以上は検査を完了できないため 2 で止める（失敗を「該当なし」と読み替えない）。
no_match() {
  local desc="$1" opt="$2" pat="$3" file="$4" rc=0
  grep "${opt}" -e "${pat}" -- "${file}" >/dev/null 2>&1 || rc=$?
  case "${rc}" in
    1) ok "${desc}" ;;
    0) violation "${desc}" ;;
    *)
      echo "error: grep failed (exit ${rc}) on ${file}" >&2
      exit 2
      ;;
  esac
}

# WASM の書き出し名を 1 行ずつ取得する。パスは process.argv で渡し、スクリプト文字列に埋め込まない。
EXPORTS_ERR=0
EXPORTS="$(
  node -e '
    const fs = require("fs");
    const m = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
    for (const e of WebAssembly.Module.exports(m)) console.log(e.name);
  ' "${WASM}"
)" || EXPORTS_ERR=$?
if [ "${EXPORTS_ERR}" -ne 0 ] || [ -z "${EXPORTS}" ]; then
  echo "error: could not read wasm exports from ${WASM}" >&2
  exit 2
fi

EXP_RC=0
printf '%s\n' "${EXPORTS}" | grep -q -x -e 'mount_app' || EXP_RC=$?
case "${EXP_RC}" in
  0) ok "mount_app is exported" ;;
  1)
    echo "error: mount_app is not exported; the export listing may be wrong" >&2
    exit 2
    ;;
  *)
    echo "error: grep failed (exit ${EXP_RC}) on the wasm export list" >&2
    exit 2
    ;;
esac

DBG_RC=0
printf '%s\n' "${EXPORTS}" | grep -q -e '^debug_' || DBG_RC=$?
case "${DBG_RC}" in
  1) ok "no debug_* export in wasm" ;;
  0) violation "debug_* export found in wasm" ;;
  *)
    echo "error: grep failed (exit ${DBG_RC}) on the wasm export list" >&2
    exit 2
    ;;
esac

no_match "no debug_ in glue js" -q 'debug_' "${GLUE_JS}"
no_match "no fandhe-test-hooks meta in index.html" -q 'fandhe-test-hooks' "${INDEX}"
no_match "no test-hooks marker in index.html" -q 'fandhe:test-hooks' "${INDEX}"
no_match "no debug_ bytes in wasm" -aq 'debug_' "${WASM}"
no_match "no raw-trigger bytes in wasm" -aq 'raw-trigger' "${WASM}"
no_match "no tick-trigger bytes in wasm" -aq 'tick-trigger' "${WASM}"
no_match "no esc-trigger bytes in wasm" -aq 'esc-trigger' "${WASM}"
no_match "no receptor probe name bytes in wasm" -aq 'fandhe-test-receptor-probe' "${WASM}"
no_match "no complete-trigger bytes in wasm" -aq 'complete-trigger' "${WASM}"

# 配信する JS・HTML には、隠し要素の名前も data-payload も正当な利用が無い（WASM との違いは冒頭のコメント）。
for f in "${GLUE_JS}" "${APP_JS}" "${INDEX}"; do
  for pat in tick-trigger esc-trigger complete-trigger data-payload; do
    no_match "no ${pat} in $(basename "${f}")" -q "${pat}" "${f}"
  done
done

# スタイルは同一オリジンの外部ファイル app.css だけに置く（SAFE-13・SAFE-15）。静的な層として、配信する HTML・JS に
# <style 要素と style= 属性が無いことを確かめる。描画後の DOM の数え上げはブラウザ試験（collectSafety）が担う。
no_match "no <style element in index.html" -q '<style' "${INDEX}"
no_match "no style= attribute in index.html" -q 'style=' "${INDEX}"
no_match "no <style in app.js" -q '<style' "${APP_JS}"
no_match "no style= in app.js" -q 'style=' "${APP_JS}"

# シンボリックリンクは名前の検査をすり抜けるため、1 件でもあれば失敗にする（リンクを辿らない）。
LINKS_ERR=0
LINKS="$(cd "${DIST}" && find . -type l | LC_ALL=C sort)" || LINKS_ERR=$?
if [ "${LINKS_ERR}" -ne 0 ]; then
  echo "error: could not search symlinks under ${DIST}" >&2
  exit 2
fi
if [ -z "${LINKS}" ]; then ok "no symlink"; else violation "symlink found: $(printf '%s' "${LINKS}" | tr '\n' ' ')"; fi

# 配信物の一覧は find の出力を変数へ受けて終了状態を確かめる。名前の検査は一覧に対して行う。
FILES_ERR=0
FILES="$(cd "${DIST}" && find . -type f | LC_ALL=C sort)" || FILES_ERR=$?
if [ "${FILES_ERR}" -ne 0 ] || [ -z "${FILES}" ]; then
  echo "error: could not list files under ${DIST}" >&2
  exit 2
fi
# 名前の判定は case で行い、grep の失敗を握りつぶさない。
HOOKS_JS=""
DTS=""
while IFS= read -r line; do
  case "${line}" in
    */test-hooks.js) HOOKS_JS="${HOOKS_JS} ${line}" ;;
    *.d.ts) DTS="${DTS} ${line}" ;;
    *) ;;
  esac
done <<EOF_FILES
${FILES}
EOF_FILES
if [ -z "${HOOKS_JS}" ]; then ok "no test-hooks.js"; else violation "test-hooks.js found:${HOOKS_JS}"; fi
if [ -z "${DTS}" ]; then ok "no .d.ts"; else violation ".d.ts found:${DTS}"; fi

echo "files:"
printf '%s\n' "${FILES}" | while IFS= read -r line; do echo "  ${line}"; done

if [ "${VIOLATIONS}" -eq 0 ]; then
  echo "RESULT: ok"
  exit 0
fi
echo "RESULT: violations found (${VIOLATIONS})"
exit 1
