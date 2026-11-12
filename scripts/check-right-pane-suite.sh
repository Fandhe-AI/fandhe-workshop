#!/usr/bin/env bash
# 右の画面の試験の束（tools/browser/suites/right-pane.txt）の定義の静的な検査。
#
# なぜ要るか: 束は、入力欄のキー処理の受け持ちを変えても右の画面の操作の結果（TERM-1〜TERM-14・TERM-23）が
#   変わらないことを守る回帰（TASK-458。TERM-40）である。定義から項目が抜ける・試験ファイルが消える・CI で流れなくなると、
#   守っているつもりで守れていない状態になる。遅いブラウザ試験を流す前に、定義の整合を cargo・node・ブラウザ無しで確かめる。
#
# 呼び出し元: scripts/check.sh（make check）、scripts/test-right-pane.sh（実行の前）、make check-right-pane-suite。
#   回帰テストは scripts/test-check-right-pane-suite.sh。
# 必須の集合（TERM の ID と TASK の出所）は、定義ファイルの行を消しても要件が消えないよう、このスクリプトに固定する。
#
# 検査（違反は理由を標準エラーに出し、最後に終了コード 1）:
#   R1 書式          各行は空行・コメント・file|（4 欄）・term|（5 欄）のどれか
#   R2 ファイル名    ^[a-z0-9-]+\.test\.mjs$ で、tools/browser/ に実在し、重複しない
#   R3 CI で流れる   各ファイルが scripts/test-browser.sh に "${ROOT_DIR}/tools/browser/<名前>" の形で載っている
#   R4 screen        出所は必須の TASK のどれか。全 TASK が 1 件以上持ち、ファイルの本文に出所の TASK-nnn がある
#   R5 helper        screen が直接 import する ./X.mjs に X.test.mjs があれば helper として載り、import されない helper は余り
#   R6 term          ID の集合が必須の 15 件とちょうど一致。種類は auto・proxy。ファイル欄は束の screen。
#                    auto の各ファイルの本文にその ID（数字の境界つき）がある。proxy は補足が空でない
#   R7 前段の不足    束のファイルと直接 import する部品が読む FANDHE_* が、回帰の入口が渡す環境変数に収まる
#
# 終了コード: 0 = 一致 / 1 = ずれ / 2 = 誤用（引数あり）・判定不能（定義ファイル・test-browser.sh が無い等）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
for cmd in awk grep sort; do
  require_cmd "${cmd}" || exit 2
done

DEF="${ROOT_DIR}/tools/browser/suites/right-pane.txt"
BDIR="${ROOT_DIR}/tools/browser"
TEST_BROWSER="${ROOT_DIR}/scripts/test-browser.sh"
for f in "${DEF}" "${TEST_BROWSER}"; do
  [ -f "${f}" ] || {
    echo "error: not found: ${f}" >&2
    exit 2
  }
done

REQUIRED_TERMS="TERM-1 TERM-2 TERM-3 TERM-4 TERM-5 TERM-6 TERM-7 TERM-8 TERM-9 TERM-10 TERM-11 TERM-12 TERM-13 TERM-14 TERM-23"
REQUIRED_TASKS="TASK-438 TASK-439 TASK-440 TASK-441 TASK-442 TASK-450 TASK-451 TASK-452"
# 回帰の入口（scripts/test-right-pane.sh）が node へ渡す（または既定で足りる）環境変数。
ALLOWED_ENV="FANDHE_WEB_DIR FANDHE_WEB_TEST_DIR FANDHE_BROWSER_CHANNEL FANDHE_PACK_BIN FANDHE_PACK_PREVIEW_BIN"

fail=0
err() {
  echo "error: $*" >&2
  fail=1
}
in_list() { # in_list <word> <space-separated list>
  case " $2 " in *" $1 "*) return 0 ;; esac
  return 1
}

FILES=""   # 載っている試験ファイル名（空白区切り）
SCREENS="" # screen の名前
HELPERS="" # helper の "<名前>=<部品>"
SCREEN_TASKS=""
TERMS="" # 載っている TERM の ID

# --- 1 回目: 書式（R1）と file 行（R2・R4 の出所の形・R5 の種類）の読み取り ---
lineno=0
while IFS= read -r line || [ -n "${line}" ]; do
  lineno=$((lineno + 1))
  case "${line}" in '' | '#'*) continue ;; esac
  nf="$(printf '%s' "${line}" | awk -F'|' '{print NF}')"
  IFS='|' read -r kind a b c _ <<<"${line}"
  case "${kind}" in
    file)
      if [ "${nf}" != 4 ]; then
        err "line ${lineno}: file| needs 4 fields"
        continue
      fi
      name="${a}"
      if ! printf '%s' "${name}" | grep -Eq '^[a-z0-9-]+\.test\.mjs$'; then
        err "line ${lineno}: invalid test file name: ${name}"
        continue
      fi
      if in_list "${name}" "${FILES}"; then
        err "line ${lineno}: duplicate file: ${name}"
        continue
      fi
      FILES="${FILES} ${name}"
      case "${b}" in
        screen)
          SCREENS="${SCREENS} ${name}"
          if in_list "${c}" "${REQUIRED_TASKS}"; then
            SCREEN_TASKS="${SCREEN_TASKS} ${c}"
          else
            err "line ${lineno}: screen source is not a required task: ${c}"
          fi
          ;;
        helper) HELPERS="${HELPERS} ${name}=${c}" ;;
        *) err "line ${lineno}: file kind must be screen or helper: ${b}" ;;
      esac
      ;;
    term)
      if [ "${nf}" != 5 ]; then
        err "line ${lineno}: term| needs 5 fields"
        continue
      fi
      if in_list "${a}" "${TERMS}"; then
        err "line ${lineno}: duplicate term: ${a}"
      fi
      TERMS="${TERMS} ${a}"
      ;;
    *) err "line ${lineno}: unknown line kind" ;;
  esac
done <"${DEF}"

# --- R2 実在・R3 CI で流れる・R4 本文の出所 ---
for name in ${FILES}; do
  [ -f "${BDIR}/${name}" ] || err "R2: not found in tools/browser: ${name}"
  grep -Fq "\"\${ROOT_DIR}/tools/browser/${name}\"" "${TEST_BROWSER}" ||
    err "R3: not run by scripts/test-browser.sh: ${name}"
done
lineno=0
while IFS= read -r line || [ -n "${line}" ]; do
  lineno=$((lineno + 1))
  case "${line}" in file\|*) ;; *) continue ;; esac
  IFS='|' read -r _ a b c _ <<<"${line}"
  if [ "${b}" = screen ] && [ -f "${BDIR}/${a}" ]; then
    grep -Eq "${c}([^0-9]|$)" "${BDIR}/${a}" ||
      err "R4: ${a} does not mention its source ${c}"
  fi
done <"${DEF}"
for t in ${REQUIRED_TASKS}; do
  in_list "${t}" "${SCREEN_TASKS}" || err "R4: no screen file for ${t}"
done

# --- R5 helper ---
IMPORTED=""
for s in ${SCREENS}; do
  [ -f "${BDIR}/${s}" ] || continue
  # import が 1 件も無い（grep が 1 を返す）場合は空でよい。
  xs="$(grep -oE "from '\./[a-z0-9-]+\.mjs'" "${BDIR}/${s}" | sed -E "s/from '\.\/(.*)'/\1/" | sort -u || true)"
  for x in ${xs}; do
    in_list "${x}" "${IMPORTED}" || IMPORTED="${IMPORTED} ${x}"
  done
done
for x in ${IMPORTED}; do
  base="${x%.mjs}"
  if [ -f "${BDIR}/${base}.test.mjs" ]; then
    in_list "${base}.test.mjs=${x}" "${HELPERS}" ||
      err "R5: helper self-check missing from the suite: ${base}.test.mjs (imported ${x})"
  fi
done
for h in ${HELPERS}; do
  src="${h#*=}"
  nm="${h%%=*}"
  if [ "${nm}" != "${src%.mjs}.test.mjs" ]; then
    err "R5: helper ${nm} must be the self-check of ${src}"
  elif ! in_list "${src}" "${IMPORTED}"; then
    err "R5: helper ${nm} is not imported by any screen file"
  fi
done

# --- R6 term ---
for t in ${REQUIRED_TERMS}; do
  in_list "${t}" "${TERMS}" || err "R6: missing term: ${t}"
done
for t in ${TERMS}; do
  in_list "${t}" "${REQUIRED_TERMS}" || err "R6: unexpected term: ${t}"
done
lineno=0
while IFS= read -r line || [ -n "${line}" ]; do
  lineno=$((lineno + 1))
  case "${line}" in term\|*) ;; *) continue ;; esac
  [ "$(printf '%s' "${line}" | awk -F'|' '{print NF}')" = 5 ] || continue
  IFS='|' read -r _ id kind tfiles note <<<"${line}"
  case "${kind}" in
    auto) ;;
    proxy) [ -n "${note}" ] || err "R6: line ${lineno}: proxy needs a note: ${id}" ;;
    *)
      err "R6: line ${lineno}: kind must be auto or proxy: ${id}"
      continue
      ;;
  esac
  [ -n "${tfiles}" ] || err "R6: line ${lineno}: no test file: ${id}"
  OLDIFS="${IFS}"
  IFS=','
  for tf in ${tfiles}; do
    IFS="${OLDIFS}"
    if ! in_list "${tf}" "${SCREENS}"; then
      err "R6: line ${lineno}: ${id} refers to a file that is not a screen file of the suite: ${tf}"
    elif [ "${kind}" = auto ] && [ -f "${BDIR}/${tf}" ] && ! grep -Eq "${id}([^0-9]|$)" "${BDIR}/${tf}"; then
      err "R6: line ${lineno}: ${tf} does not mention ${id}"
    fi
    IFS=','
  done
  IFS="${OLDIFS}"
done <"${DEF}"

# --- R7 前段の不足 ---
CHECKED=""
for h in ${HELPERS}; do CHECKED="${CHECKED} ${h%%=*}"; done
for f in ${SCREENS} ${CHECKED} ${IMPORTED}; do
  [ -f "${BDIR}/${f}" ] || continue
  vs="$(grep -ohE 'FANDHE_[A-Z0-9_]+' "${BDIR}/${f}" | sort -u || true)" # 読む変数が無ければ空でよい
  for v in ${vs}; do
    in_list "${v}" "${ALLOWED_ENV}" ||
      err "R7: ${f} reads ${v}, which scripts/test-right-pane.sh does not provide"
  done
done

if [ "${fail}" != 0 ]; then
  exit 1
fi
log "right-pane suite definition ok"
