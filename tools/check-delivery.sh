#!/usr/bin/env bash
# 配信する設定の配信物に試験用の口が無いことの検査（SHELL-35・BUILD-27。`make check-delivery` から呼ばれる）。
#
# 目的: `make build-web`（配信する設定）が作る dist/web に、試験用の口（debug_* の書き出し・test-hooks.js・
# .d.ts・test-hooks の meta）が入っていないことを機械で保証する（TASK-81 の 2/2）。
# 先に `make build-web` で配信物を作っておく。CI では browser-test ジョブの `make test-build-web` が流す。
#
# 失敗にする検査（すべて固定の文字列・名前で、配信物の読み取りだけを行う。ファイルは変更・削除しない）:
#   1. WASM の書き出しに mount_app がある（書き出しの取得が空振りしていないことの錨）
#   2. WASM の書き出しに debug_ で始まる名前が無い
#   3. JS の glue（wasm/fandhe_workshop_web.js）に debug_ が無い
#   4. index.html に fandhe-test-hooks（meta）と fandhe:test-hooks（印）が無い
#   5. test-hooks.js が配信物に無い
#   6. 配信物のどこにも *.d.ts が無い
#   7. WASM のバイト列（`name` セクションを除く）に debug_ と raw-trigger が無い
#   8. 配信物の中にシンボリックリンクが無い（find -type f はリンクを列挙せず、名前の検査（5・6）をすり抜けるため。
#      tools/serve.mjs は配信ルート内のリンク先を配信するので、リンクは到達可能な経路になる）
#   9a. WASM のバイト列に代役の受け口の名 fandhe-test-receptor-probe が無い（TASK-191 2/2）
#   9. WASM のバイト列に隠し要素の名前 tick-trigger・esc-trigger・complete-trigger が無い（TASK-349 2/2）
#  10. 配信する JS・HTML（glue の JS・app.js・index.html）に、上の 3 名と data-payload が無い（TASK-349 2/2）
#  11. index.html と app.js に <style 要素と style= 属性が無い（TASK-469 1/2。SAFE-15 の静的な層）。
#      app.css の存在を必須にする（欠けていれば 2）。WASM のバイト列と glue の JS は、依存由来の文字列で 0 件に
#      ならない可能性があるため数えず、描画後の DOM の数え上げ（ブラウザ試験）に任せる。
# 7 の WASM のバイト列は、`name` セクション（関数名のデバッグ情報）を除いて数える（tools/wasm-scan.mjs）。2026-10-30 に
# SKIP_WASM_OPT=1 と wasm-opt 適用の両方の release wasm で実測し、debug_ は `name` セクションの中にだけ 2 件あり
# （SKIP_WASM_OPT=1 のとき。debug_struct 等の core 由来）、他のセクションは 0 件だった。そのため wasm-opt の有無に依らず
# 判定でき、CI（wasm-opt なし）でも流せる。除くのは正確な名前 `name` のカスタムセクションだけで、他は数える（fail-closed）。
# 9・10 は 2026-10-16 に配信する設定の release 一式で実測し、いずれも 0 件だった（WASM の data-payload は 1 件）。
#
# TASK-82 で足した項目（2026-10-30 に試験の設定との差を実測。試験の設定には有る印だけを選んだ）:
#  12. WASM（`name` を除く）に Action::Raw の接頭辞（load-fixture:・open-history:・load-ir:・opening-override:）・
#      試験用 IR の印（min-3step・ch0-3-run・ch4-9-run）・試験用クエリの印（reload-points・receptor=）・keydown-probe が無い
#  13. WASM のバイト列の全体に、試験用の関数の代表名 debug_state_json・debug_draft・debug_next_tick・debug_rerender が無い
#  14. glue の JS に keydown-probe・receptor_activation が無い。glue・app.js・index.html に fixture=・receptor= が無い
#  15. ir/ の下に ir/main.json 以外が無く、ir/main.json 以外の *.json・名前に min-3step を含むファイルが無い
#      （拒否する一覧で判定する。許可する一覧にすると、任意の追加ファイルを持つ合成の配信物が通らなくなる）
#
# 対照（--control <試験の設定の dist>）: 指定したときだけ、上の印が試験の設定には有ることを確かめ、検査が空振りして
# いないことを示す（debug_ の書き出しが 5 件以上で代表の 4 名が有る・WASM の各印・glue の debug_・fandhe-test-hooks の meta・
# test-hooks.js と .d.ts）。欠けがあれば `CONTROL-MISSING:` を出して 2 にする（合否を決められない）。tick/esc/complete-trigger
# と fandhe:test-hooks は試験の設定でも 0 件の廃止済みの口なので対照に入れない（注入の回帰テストで検出力を示す）。
# BUILD-27 の「debug_ 5 件」は PoC 期の数で、実装には 37 件ある。件数は固定せず `info:` に出す。
#
# data-payload を WASM のバイト列で数えない理由（TASK-349 の「data-payload の経路 0 件」の解釈）: 左の成果物の指せる行
# （data-action="ui"。artifact_view の PAYLOAD_ATTR）が正当に出力する属性名であり、同じ文字列リテラルは 1 つに
# まとめられるため、件数では新しい経路を検出できない。「経路」は、進行の宛先（tick・complete・esc）を data-payload に
# 書いて click する形と定義し、WASM の外の層で塞ぐ。配信する JS・HTML は 10 で 0 件にし、ソースは
# crates/web/tests/no_hidden_triggers.rs（native の静的検査）が、許可した箇所の件数固定で守る。
#
# scripts/test-build-web.sh は、配信する設定のすべてのビルドに対してこの検査を流す（(a) の同種の項目は本検査に一本化済み）。
#
# 使い方: tools/check-delivery.sh [--control <試験の設定の dist>] [<dist dir>]（既定は <repo>/dist/web）
# 終了コード: 0 = 試験用の口なし / 1 = 見つかった / 2 = 誤用、または検査を完了できない
#   （ディレクトリや必須ファイルが無い・node が無い・書き出しの取得や WASM の解析に失敗・書き出しが空・対照に印が欠けた）。
#   2 でも合格にしない（fail-closed）。
#   回帰テストは scripts/test-check-delivery.sh（`make test-check-delivery`。実ビルドを行うため遅い）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../scripts/lib.sh
source "${DIR}/../scripts/lib.sh"

usage() {
  echo "usage: $(basename "$0") [--control <test config dist>] [<dist dir>]" >&2
  exit 2
}
CONTROL=""
POSITIONAL=0
DIST=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --control)
      [ "$#" -ge 2 ] || usage
      [ -z "${CONTROL}" ] || usage
      CONTROL="$2"
      [ -n "${CONTROL}" ] || usage
      shift 2
      ;;
    --*) usage ;;
    *)
      [ "${POSITIONAL}" -eq 0 ] || usage
      POSITIONAL=1
      DIST="$1"
      shift
      ;;
  esac
done
DIST="${DIST:-${ROOT_DIR}/dist/web}"
SCAN="${DIR}/wasm-scan.mjs"

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
# WASM のバイト列の判定は tools/wasm-scan.mjs（`name` セクションを除く）で数える。出力は変数へ受けて終了状態を確かめ、
# 解析に失敗したら 2 で止める（失敗を「0 件」と読み替えない）。
# scan_wasm <WASM> <パターン>...: `パターン<TAB>件数` の行を出す。
scan_wasm() {
  local wasm="$1" out rc=0
  shift
  out="$(node "${SCAN}" "${wasm}" "$@")" || rc=$?
  if [ "${rc}" -ne 0 ]; then
    echo "error: could not scan wasm ${wasm}" >&2
    exit 2
  fi
  printf '%s\n' "${out}"
}
# count_of <scan の出力> <パターン>: パターンの件数。行が無ければ 2 で止める。
count_of() {
  local line
  line="$(printf '%s\n' "$1" | awk -F '\t' -v p="$2" '$1 == p { print $2; exit }')"
  case "${line}" in
    '' | *[!0-9]*)
      echo "error: no scan result for $2" >&2
      exit 2
      ;;
    *) printf '%s' "${line}" ;;
  esac
}
# wasm_absent <説明> <scan の出力> <パターン>: 件数 0 だけを合格にする。
wasm_absent() {
  local n
  n="$(count_of "$2" "$3")"
  if [ "${n}" -eq 0 ]; then ok "$1"; else violation "$1"; fi
}

WASM_PATTERNS="debug_ raw-trigger tick-trigger esc-trigger complete-trigger fandhe-test-receptor-probe"
WASM_PATTERNS="${WASM_PATTERNS} load-fixture: open-history: load-ir: opening-override: min-3step ch0-3-run ch4-9-run"
WASM_PATTERNS="${WASM_PATTERNS} reload-points receptor= keydown-probe"
# shellcheck disable=SC2086 # 固定のパターン列を単語分割して渡す
SCAN_OUT="$(scan_wasm "${WASM}" ${WASM_PATTERNS})"
wasm_absent "no debug_ bytes in wasm" "${SCAN_OUT}" 'debug_'
wasm_absent "no raw-trigger bytes in wasm" "${SCAN_OUT}" 'raw-trigger'
wasm_absent "no tick-trigger bytes in wasm" "${SCAN_OUT}" 'tick-trigger'
wasm_absent "no esc-trigger bytes in wasm" "${SCAN_OUT}" 'esc-trigger'
wasm_absent "no receptor probe name bytes in wasm" "${SCAN_OUT}" 'fandhe-test-receptor-probe'
wasm_absent "no complete-trigger bytes in wasm" "${SCAN_OUT}" 'complete-trigger'
for pat in load-fixture: open-history: load-ir: opening-override: min-3step ch0-3-run ch4-9-run reload-points receptor= keydown-probe; do
  wasm_absent "no ${pat} bytes in wasm" "${SCAN_OUT}" "${pat}"
done
# 試験用の関数の代表名は `name` セクションも含む全体で数える（wasm-opt の有無に依らず、試験用の関数名が残らない保険）。
for pat in debug_state_json debug_draft debug_next_tick debug_rerender; do
  no_match "no ${pat} in wasm (whole bytes)" -aq "${pat}" "${WASM}"
done
# glue の JS の試験用の印、JS・HTML の試験用クエリの印。
no_match "no keydown-probe in glue js" -q 'keydown-probe' "${GLUE_JS}"
no_match "no receptor_activation in glue js" -q 'receptor_activation' "${GLUE_JS}"
for f in "${GLUE_JS}" "${APP_JS}" "${INDEX}"; do
  for pat in 'fixture=' 'receptor='; do
    no_match "no ${pat} in $(basename "${f}")" -q "${pat}" "${f}"
  done
done

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

# run_control: --control の対照。DIST と同じ実体、または必須ファイルが無い場合は誤用として 2。
run_control() {
  local cdir cdist cwasm cglue cindex missing=0 n out exp_out
  if [ ! -d "${CONTROL}" ]; then
    echo "error: control directory not found: ${CONTROL}" >&2
    exit 2
  fi
  cdir="$(cd "${CONTROL}" && pwd -P)"
  cdist="$(cd "${DIST}" && pwd -P)"
  if [ "${cdir}" = "${cdist}" ]; then
    echo "error: control directory is the same as the checked directory" >&2
    exit 2
  fi
  cwasm="${cdir}/wasm/fandhe_workshop_web_bg.wasm"
  cglue="${cdir}/wasm/fandhe_workshop_web.js"
  cindex="${cdir}/index.html"
  for f in "${cwasm}" "${cglue}" "${cindex}"; do
    if [ ! -f "${f}" ]; then
      echo "error: control file not found: ${f}" >&2
      exit 2
    fi
  done
  control_has() { echo "ok: control has $1"; }
  control_missing() {
    echo "CONTROL-MISSING: $1"
    missing=$((missing + 1))
  }
  exp_out="$(
    node -e '
      const fs = require("fs");
      const m = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
      for (const e of WebAssembly.Module.exports(m)) console.log(e.name);
    ' "${cwasm}"
  )" || {
    echo "error: could not read control wasm exports" >&2
    exit 2
  }
  n="$(printf '%s\n' "${exp_out}" | grep -c -e '^debug_' || true)"
  echo "info: control debug_ exports = ${n}"
  if [ "${n}" -ge 5 ]; then control_has "debug_ exports (>= 5)"; else control_missing "debug_ exports (>= 5)"; fi
  for name in debug_state_json debug_draft debug_next_tick debug_rerender; do
    if printf '%s\n' "${exp_out}" | grep -q -x -e "${name}"; then control_has "${name} export"; else control_missing "${name} export"; fi
  done
  # shellcheck disable=SC2086 # 固定のパターン列を単語分割して渡す
  out="$(scan_wasm "${cwasm}" ${WASM_PATTERNS})"
  for pat in debug_ raw-trigger load-fixture: open-history: load-ir: opening-override: min-3step ch0-3-run ch4-9-run reload-points receptor= keydown-probe fandhe-test-receptor-probe; do
    if [ "$(count_of "${out}" "${pat}")" -gt 0 ]; then control_has "${pat} in wasm"; else control_missing "${pat} in wasm"; fi
  done
  if grep -q -e 'debug_' -- "${cglue}"; then control_has "debug_ in glue js"; else control_missing "debug_ in glue js"; fi
  if grep -q -e 'fandhe-test-hooks' -- "${cindex}"; then control_has "fandhe-test-hooks meta"; else control_missing "fandhe-test-hooks meta"; fi
  if [ -f "${cdir}/test-hooks.js" ]; then control_has "test-hooks.js"; else control_missing "test-hooks.js"; fi
  if [ -n "$(find "${cdir}" -name '*.d.ts' -type f)" ]; then control_has ".d.ts"; else control_missing ".d.ts"; fi
  if [ "${missing}" -ne 0 ]; then
    echo "error: control config lacks ${missing} expected mark(s); the check may be a no-op" >&2
    exit 2
  fi
}

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
IR_EXTRA=""
JSON_EXTRA=""
FIXTURE_NAMED=""
while IFS= read -r line; do
  case "${line}" in
    */test-hooks.js) HOOKS_JS="${HOOKS_JS} ${line}" ;;
    *.d.ts) DTS="${DTS} ${line}" ;;
    *) ;;
  esac
  # 試験用 IR・試験用の置き場は拒否する一覧で判定する（許可する一覧にはしない）。
  case "${line}" in
    ./ir/main.json) ;;
    ./ir/*) IR_EXTRA="${IR_EXTRA} ${line}" ;;
    *.json) JSON_EXTRA="${JSON_EXTRA} ${line}" ;;
    *) ;;
  esac
  case "${line}" in
    *min-3step*) FIXTURE_NAMED="${FIXTURE_NAMED} ${line}" ;;
    *) ;;
  esac
done <<EOF_FILES
${FILES}
EOF_FILES
if [ -z "${HOOKS_JS}" ]; then ok "no test-hooks.js"; else violation "test-hooks.js found:${HOOKS_JS}"; fi
if [ -z "${DTS}" ]; then ok "no .d.ts"; else violation ".d.ts found:${DTS}"; fi

if [ -z "${IR_EXTRA}" ]; then ok "no extra file under ir/"; else violation "extra file under ir/:${IR_EXTRA}"; fi
if [ -z "${JSON_EXTRA}" ]; then ok "no test IR json"; else violation "json file found:${JSON_EXTRA}"; fi
if [ -z "${FIXTURE_NAMED}" ]; then ok "no min-3step file"; else violation "min-3step file found:${FIXTURE_NAMED}"; fi

# 対照: 試験の設定の dist に同じ印が有ることを確かめる（検査が空振りしていないことの証明）。欠けたら合否を決められず 2。
if [ -n "${CONTROL}" ]; then
  run_control
fi

echo "files:"
printf '%s\n' "${FILES}" | while IFS= read -r line; do echo "  ${line}"; done

if [ "${VIOLATIONS}" -eq 0 ]; then
  echo "RESULT: ok"
  exit 0
fi
echo "RESULT: violations found (${VIOLATIONS})"
exit 1
