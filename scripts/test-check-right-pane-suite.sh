#!/usr/bin/env bash
# scripts/check-right-pane-suite.sh 自体の回帰テスト（`make test-check-right-pane-suite` から呼ばれる。CI の check ジョブも流す）。
#
# 「束の定義がずれると検査が失敗し、失敗の理由が出る」ことを確かめる。作業ツリーは変えない。
# 検査スクリプトと見本の定義・試験ファイル・test-browser.sh を mktemp -d の疑似 repo へ組み立てて流す。
# 出力は TAP 風。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd mkdir
require_cmd mv
require_cmd sed
require_cmd rm
require_cmd mktemp
require_cmd cp
require_cmd grep

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0

# expect <name> <expected-rc> <actual-rc>
expect() {
  if [ "$2" = "$3" ]; then
    echo "ok - $1 (exit $3)"
    passed=$((passed + 1))
  else
    echo "not ok - $1 (expected exit $2, got $3)"
    failed=$((failed + 1))
  fi
}

# expect_stderr <name> <fixed-string>: 直前の run_check の標準エラーに文字列があること。
expect_stderr() {
  if grep -Fq -- "$2" "${TMP_DIR}/stderr.txt"; then
    echo "ok - $1"
    passed=$((passed + 1))
  else
    echo "not ok - $1 (stderr lacks: $2)"
    failed=$((failed + 1))
  fi
}

REPO="${TMP_DIR}/repo"
BD="${REPO}/tools/browser"
DEFF="${BD}/suites/right-pane.txt"
TB="${REPO}/scripts/test-browser.sh"

# 見本の screen 8 件（TASK ごとに 1 件。本文に出所と TERM の ID を書く）と helper 1 件（部品 h.mjs・自己確認 h.test.mjs）。
# TERM-1 を s438、TERM-10 を s451 に置き、TERM-1 と TERM-10 の区別を確かめられるようにする。
# terms_of <TASK 番号>: 見本の試験ファイルの本文に書く TERM の ID。
terms_of() {
  case "$1" in
    438) echo "TERM-1 TERM-2 TERM-3 TERM-4 TERM-5 TERM-6 TERM-7" ;;
    440) echo "TERM-8" ;;
    441) echo "TERM-9" ;;
    442) echo "TERM-11" ;;
    450) echo "TERM-12 TERM-13 TERM-14 TERM-23" ;;
    451) echo "TERM-10" ;;
    *) echo "TERM-0" ;;
  esac
}

build() {
  rm -rf "${REPO}"
  mkdir -p "${REPO}/scripts" "${BD}/suites"
  cp "${DIR}/check-right-pane-suite.sh" "${DIR}/lib.sh" "${REPO}/scripts/"
  : >"${TB}"
  local n terms name
  for n in 438 439 440 441 442 450 451 452; do
    name="s${n}.test.mjs"
    terms="$(terms_of "${n}")"
    printf "// TASK-%s %s\nimport { x } from './h.mjs';\n" "${n}" "${terms}" >"${BD}/${name}"
    # shellcheck disable=SC2016 # ${ROOT_DIR} は展開せず、test-browser.sh の現物と同じ文字で書く
    printf '  "${ROOT_DIR}/tools/browser/%s" \\\n' "${name}" >>"${TB}"
  done
  printf 'export const x = 1;\n' >"${BD}/h.mjs"
  printf '// h の自己確認\n' >"${BD}/h.test.mjs"
  # shellcheck disable=SC2016 # 同上
  printf '  "${ROOT_DIR}/tools/browser/h.test.mjs" \\\n' >>"${TB}"
  write_def
}

# write_def: 見本の定義を書く（検査は必須の TERM 15 件を固定しているため、本物と同じ 15 件を並べる）。
write_def() {
  {
    echo '# sample'
    for n in 438 439 440 441 442 450 451 452; do echo "file|s${n}.test.mjs|screen|TASK-${n}"; done
    echo 'file|h.test.mjs|helper|h.mjs'
    for t in 1 2 3 5 6 7; do echo "term|TERM-${t}|auto|s438.test.mjs|"; done
    echo 'term|TERM-4|proxy|s438.test.mjs|note'
    echo 'term|TERM-8|auto|s440.test.mjs|'
    echo 'term|TERM-9|auto|s441.test.mjs|'
    echo 'term|TERM-10|auto|s451.test.mjs|'
    echo 'term|TERM-11|auto|s442.test.mjs|'
    echo 'term|TERM-12|proxy|s450.test.mjs|note'
    echo 'term|TERM-13|auto|s450.test.mjs|'
    echo 'term|TERM-14|auto|s450.test.mjs|'
    echo 'term|TERM-23|auto|s450.test.mjs|'
  } >"${DEFF}"
}

run_check() {
  RC=0
  "${REPO}/scripts/check-right-pane-suite.sh" "$@" >/dev/null 2>"${TMP_DIR}/stderr.txt" || RC=$?
}

build
run_check
expect "baseline matches" 0 "${RC}"

# term 行の欠落
build
grep -v '^term|TERM-9|' "${DEFF}" >"${DEFF}.new" && mv "${DEFF}.new" "${DEFF}"
run_check
expect "missing term" 1 "${RC}"
expect_stderr "missing term: id reported" "missing term: TERM-9"

# term 行の重複
build
echo 'term|TERM-9|auto|s441.test.mjs|' >>"${DEFF}"
run_check
expect "duplicate term" 1 "${RC}"
expect_stderr "duplicate term: id reported" "duplicate term: TERM-9"

# 余りの term
build
echo 'term|TERM-99|auto|s441.test.mjs|' >>"${DEFF}"
run_check
expect "unexpected term" 1 "${RC}"
expect_stderr "unexpected term: id reported" "unexpected term: TERM-99"

# 種類の誤り
build
sed 's/^term|TERM-9|auto/term|TERM-9|manual/' "${DEFF}" >"${DEFF}.new" && mv "${DEFF}.new" "${DEFF}"
run_check
expect "invalid term kind" 1 "${RC}"
expect_stderr "invalid kind reported" "kind must be auto or proxy"

# auto のファイルに ID が無い
build
sed 's/^term|TERM-9|auto|s441/term|TERM-9|auto|s440/' "${DEFF}" >"${DEFF}.new" && mv "${DEFF}.new" "${DEFF}"
run_check
expect "auto file lacks the id" 1 "${RC}"
expect_stderr "lacks id reported" "does not mention TERM-9"

# TERM-1 と TERM-10 の区別（s451 の本文は TERM-10 だけ。TERM-1 を s451 に割り当てると不合格）
build
sed 's/^term|TERM-1|auto|s438/term|TERM-1|auto|s451/' "${DEFF}" >"${DEFF}.new" && mv "${DEFF}.new" "${DEFF}"
run_check
expect "TERM-1 does not match TERM-10" 1 "${RC}"
expect_stderr "TERM-1 vs TERM-10 reported" "s451.test.mjs does not mention TERM-1"

# proxy の補足が空
build
sed 's/^term|TERM-4|proxy|s438.test.mjs|note/term|TERM-4|proxy|s438.test.mjs|/' "${DEFF}" >"${DEFF}.new" && mv "${DEFF}.new" "${DEFF}"
run_check
expect "proxy without note" 1 "${RC}"
expect_stderr "proxy note reported" "proxy needs a note"

# term のファイルが screen でない
build
sed 's/^term|TERM-9|auto|s441.test.mjs/term|TERM-9|auto|h.test.mjs/' "${DEFF}" >"${DEFF}.new" && mv "${DEFF}.new" "${DEFF}"
run_check
expect "term refers to a non-screen file" 1 "${RC}"
expect_stderr "non-screen reported" "not a screen file"

# ファイルの不在
build
rm "${BD}/s441.test.mjs"
run_check
expect "test file missing" 1 "${RC}"
expect_stderr "missing file reported" "not found in tools/browser: s441.test.mjs"

# test-browser.sh に載っていない
build
grep -v 's442.test.mjs' "${TB}" >"${TB}.new" && mv "${TB}.new" "${TB}"
run_check
expect "not listed in test-browser.sh" 1 "${RC}"
expect_stderr "not run reported" "not run by scripts/test-browser.sh: s442.test.mjs"

# 不正な名前
build
echo 'file|../x.test.mjs|screen|TASK-438' >>"${DEFF}"
run_check
expect "invalid file name" 1 "${RC}"
expect_stderr "invalid name reported" "invalid test file name"

# 重複したファイル
build
echo 'file|s438.test.mjs|screen|TASK-438' >>"${DEFF}"
run_check
expect "duplicate file" 1 "${RC}"
expect_stderr "duplicate file reported" "duplicate file"

# タスクの screen が欠ける
build
grep -v '^file|s452.test.mjs|' "${DEFF}" >"${DEFF}.new" && mv "${DEFF}.new" "${DEFF}"
run_check
expect "task without a screen file" 1 "${RC}"
expect_stderr "task reported" "no screen file for TASK-452"

# 出所が本文に無い
build
printf '// no source here\n' >"${BD}/s452.test.mjs"
run_check
expect "source task not in the file body" 1 "${RC}"
expect_stderr "source reported" "does not mention its source TASK-452"

# helper の不足
build
grep -v '^file|h.test.mjs|' "${DEFF}" >"${DEFF}.new" && mv "${DEFF}.new" "${DEFF}"
run_check
expect "helper self-check missing" 1 "${RC}"
expect_stderr "helper missing reported" "helper self-check missing"

# helper の余り（どの screen も import しない）
build
for n in 438 439 440 441 442 450 451 452; do
  grep -v "^import" "${BD}/s${n}.test.mjs" >"${BD}/new.tmp" && mv "${BD}/new.tmp" "${BD}/s${n}.test.mjs"
done
run_check
expect "helper not imported by any screen" 1 "${RC}"
expect_stderr "unused helper reported" "not imported by any screen file"

# 許していない環境変数
build
printf "// TASK-450 TERM-12 TERM-13 TERM-14 TERM-23\nimport { x } from './h.mjs';\nconst v = process.env.FANDHE_SECRET_THING;\n" >"${BD}/s450.test.mjs"
run_check
expect "unprovided environment variable" 1 "${RC}"
expect_stderr "env reported" "reads FANDHE_SECRET_THING"

# 書式の崩れ
build
echo 'term|TERM-9|auto' >>"${DEFF}"
run_check
expect "malformed term line" 1 "${RC}"
expect_stderr "malformed reported" "term| needs 5 fields"

build
echo 'bogus line' >>"${DEFF}"
run_check
expect "unknown line kind" 1 "${RC}"
expect_stderr "unknown kind reported" "unknown line kind"

# 誤用・判定不能
build
run_check extra
expect "arguments are rejected" 2 "${RC}"

build
rm "${DEFF}"
run_check
expect "definition file missing is undecidable" 2 "${RC}"

build
rm "${TB}"
run_check
expect "test-browser.sh missing is undecidable" 2 "${RC}"

echo "1..$((passed + failed))"
echo "# passed: ${passed}, failed: ${failed}"
[ "${failed}" = 0 ]
