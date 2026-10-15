#!/usr/bin/env bash
# 変異注入の検査: 実装に誤りを 1 つ入れると、対応する試験が失敗することを機械で確かめる（`make check-mutations` から呼ばれる）。
#
# 変異の定義は crates/<crate>/tests/mutations/<ID>.mutation（形式・手順・記録の様式は docs/testing.md）。
# 作業ツリーは書き換えない。Cargo.toml・Cargo.lock・rust-toolchain.toml・crates/（あれば testdata/ も）を mktemp -d の一時ディレクトリへ写し、
# 写しの中で 1 か所を置換してから cargo test を流す。変異の find / replace は「データ」として扱い、
# awk の ENVIRON と index() による文字どおりの一致だけで置換する（sed の式・eval・bash のパターン置換は使わない）。
#
# 1 回の実行の流れ:
#   1. 元の状態（変異なし）で対象の試験が通り、expect-fail の名前が実行されていることを確かめる
#   2. 変異ごとに写しをやり直して置換を当て、試験を流す。expect-fail がすべて失敗すれば DETECTED、欠ければ SURVIVED
#   3. 記録の表（Markdown）を標準出力に出す。進行のログは標準エラー出力に出す
#
# 使い方: scripts/check-mutations.sh [definition-file ...]
#   引数が無ければ crates/*/tests/mutations/*.mutation の全件を対象にする。
# 終了コード: 0 = 全件が検出された / 1 = 検出されない変異がある（SURVIVED）/
#   2 = 誤用、定義の不正、元の状態の失敗、コンパイルできない変異（2 でも合格にしない。fail-closed）。
# 回帰テストは scripts/test-check-mutations.sh。CI には含めない（遅い。組み込みは TASK-33・TASK-83）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

# die <message>: 英語の error を出して終了コード 2 で止める。
die() {
  echo "error: $*" >&2
  exit 2
}

require_cmd cargo || exit 2
require_cmd awk || exit 2

REAL_ROOT="$(cd "${ROOT_DIR}" && pwd -P)"

# --- 引数 ---------------------------------------------------------------

DEFS=()
if [ "$#" -gt 0 ]; then
  for arg in "$@"; do
    case "${arg}" in
      *.mutation) ;;
      *)
        echo "usage: $(basename "$0") [definition-file.mutation ...]" >&2
        exit 2
        ;;
    esac
    if [ ! -f "${arg}" ]; then
      echo "usage: $(basename "$0") [definition-file.mutation ...] (not a file: ${arg})" >&2
      exit 2
    fi
    arg_dir="$(cd "$(dirname "${arg}")" && pwd -P)"
    case "${arg_dir}/" in
      "${REAL_ROOT}/"*) ;;
      *)
        echo "usage: $(basename "$0") [definition-file.mutation ...] (outside the repository: ${arg})" >&2
        exit 2
        ;;
    esac
    DEFS+=("${arg_dir}/$(basename "${arg}")")
  done
else
  for f in "${REAL_ROOT}"/crates/*/tests/mutations/*.mutation; do
    [ -f "${f}" ] && DEFS+=("${f}")
  done
  if [ "${#DEFS[@]}" -eq 0 ]; then
    die "no mutation definitions found under crates/*/tests/mutations/"
  fi
fi

# --- 定義の読み取り -------------------------------------------------------

# 1 件分の定義を保持する変数。parse_def が埋める。
M_ID="" M_PACKAGE="" M_TEST="" M_TARGET="" M_FIND="" M_REPLACE="" M_NOTE=""
M_EXPECT=()

# parse_def <file>: 定義を読み、形式を検査する。不正なら終了コード 2。
# 各行は最初の ": " で key と value に分ける。未知のキー・重複・欠落・空の値は拒否する。
parse_def() {
  local file="$1" line key value
  local seen_id=0 seen_package=0 seen_test=0 seen_target=0 seen_find=0 seen_replace=0 seen_note=0
  M_ID="" M_PACKAGE="" M_TEST="" M_TARGET="" M_FIND="" M_REPLACE="" M_NOTE=""
  M_EXPECT=()
  while IFS= read -r line || [ -n "${line}" ]; do
    [ -z "${line}" ] && continue
    case "${line}" in
      *": "*) ;;
      *) die "${file}: malformed line (expected 'key: value'): ${line}" ;;
    esac
    key="${line%%: *}"
    value="${line#*: }"
    [ -n "${value}" ] || die "${file}: empty value for key: ${key}"
    case "${key}" in
      id) [ "${seen_id}" -eq 0 ] || die "${file}: duplicate key: id"; seen_id=1; M_ID="${value}" ;;
      package) [ "${seen_package}" -eq 0 ] || die "${file}: duplicate key: package"; seen_package=1; M_PACKAGE="${value}" ;;
      test) [ "${seen_test}" -eq 0 ] || die "${file}: duplicate key: test"; seen_test=1; M_TEST="${value}" ;;
      target) [ "${seen_target}" -eq 0 ] || die "${file}: duplicate key: target"; seen_target=1; M_TARGET="${value}" ;;
      find) [ "${seen_find}" -eq 0 ] || die "${file}: duplicate key: find"; seen_find=1; M_FIND="${value}" ;;
      replace) [ "${seen_replace}" -eq 0 ] || die "${file}: duplicate key: replace"; seen_replace=1; M_REPLACE="${value}" ;;
      note) [ "${seen_note}" -eq 0 ] || die "${file}: duplicate key: note"; seen_note=1; M_NOTE="${value}" ;;
      expect-fail) M_EXPECT+=("${value}") ;;
      *) die "${file}: unknown key: ${key}" ;;
    esac
  done <"${file}"
  [ "${seen_id}" -eq 1 ] || die "${file}: missing key: id"
  [ "${seen_package}" -eq 1 ] || die "${file}: missing key: package"
  [ "${seen_test}" -eq 1 ] || die "${file}: missing key: test"
  [ "${seen_target}" -eq 1 ] || die "${file}: missing key: target"
  [ "${seen_find}" -eq 1 ] || die "${file}: missing key: find"
  [ "${seen_replace}" -eq 1 ] || die "${file}: missing key: replace"
  [ "${#M_EXPECT[@]}" -gt 0 ] || die "${file}: missing key: expect-fail"
  # package・test は cargo の引数に渡すため、使える文字を絞る。
  printf '%s\n' "${M_PACKAGE}" | grep -Eq '^[a-z0-9-]+$' || die "${file}: invalid package: ${M_PACKAGE}"
  printf '%s\n' "${M_TEST}" | grep -Eq '^[a-z0-9_]+$' || die "${file}: invalid test: ${M_TEST}"
  printf '%s\n' "${M_ID}" | grep -Eq '^[A-Za-z0-9_-]+$' || die "${file}: invalid id: ${M_ID}"
  [ "${M_FIND}" != "${M_REPLACE}" ] || die "${file}: find and replace are identical"
  check_target "${file}" "${M_TARGET}"
}

# check_target <def-file> <target>: repo 根からの相対パスで、repo の中の通常ファイルに限る。
# 絶対パス・".." の成分・symlink（途中を含む）を拒否し、書き換えを一時コピーの中に閉じる。
check_target() {
  local file="$1" target="$2" rest part walk
  case "${target}" in
    /*) die "${file}: target must be a relative path: ${target}" ;;
    "") die "${file}: target is empty" ;;
  esac
  rest="${target}"
  walk="${REAL_ROOT}"
  while [ -n "${rest}" ]; do
    part="${rest%%/*}"
    if [ "${part}" = "${rest}" ]; then
      rest=""
    else
      rest="${rest#*/}"
    fi
    case "${part}" in
      "" | "." | "..") die "${file}: target has an invalid path component: ${target}" ;;
    esac
    walk="${walk}/${part}"
    [ ! -L "${walk}" ] || die "${file}: target passes through a symlink: ${target}"
  done
  [ -f "${REAL_ROOT}/${target}" ] || die "${file}: target is not a regular file: ${target}"
}

# --- 置換 ---------------------------------------------------------------

# count_occurrences <file> <find>: find が file に現れる回数（行ごとの重ならない出現の合計）。
count_occurrences() {
  FIND="$2" awk '
    BEGIN { find = ENVIRON["FIND"]; n = length(find); total = 0 }
    {
      line = $0
      while ((i = index(line, find)) > 0) {
        total++
        line = substr(line, i + n)
      }
    }
    END { print total }
  ' "$1"
}

# replace_once <file> <find> <replace> <out>: 最初の 1 か所だけを置き換えて out に書く。
replace_once() {
  FIND="$2" REPLACE="$3" awk '
    BEGIN { find = ENVIRON["FIND"]; repl = ENVIRON["REPLACE"]; n = length(find); done = 0 }
    {
      if (!done && (i = index($0, find)) > 0) {
        print substr($0, 1, i - 1) repl substr($0, i + n)
        done = 1
      } else {
        print $0
      }
    }
  ' "$1" >"$4"
}

# --- 試験の実行 -----------------------------------------------------------

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT
export CARGO_TARGET_DIR="${TMP_DIR}/target"
BASE="${TMP_DIR}/base"
WORK="${TMP_DIR}/work"
OUT="${TMP_DIR}/out.txt"

# 既定の種・ケース数で決定的に流し、記録を再現できるようにする。
unset FANDHE_PROPTEST_SEED FANDHE_PROPTEST_CASES

# fresh_work: 変更前のコピー（BASE）から WORK を作り直す。
fresh_work() {
  rm -rf "${WORK}"
  cp -R "${BASE}" "${WORK}"
}

# run_test <package> <test>: WORK で試験のターゲット全体を流し、出力を OUT に残す。cargo の終了コードを返す。
run_test() {
  (cd "${WORK}" && cargo test --locked -p "$1" --test "$2") >"${OUT}" 2>&1
}

# join_by_comma <items...>: ", " で連結する。
join_by_comma() {
  local out="" item
  for item in "$@"; do
    if [ -z "${out}" ]; then out="${item}"; else out="${out}, ${item}"; fi
  done
  printf '%s' "${out}"
}

# md_escape <text>: Markdown の表のセルに入れるため、"|" を退避する。
md_escape() {
  printf '%s' "$1" | awk '{ gsub(/\|/, "\\|"); printf "%s", $0 }'
}

# --- 本体 ---------------------------------------------------------------

mkdir -p "${BASE}"
for item in Cargo.toml Cargo.lock rust-toolchain.toml crates; do
  [ -e "${REAL_ROOT}/${item}" ] || die "missing in repository root: ${item}"
  cp -R "${REAL_ROOT}/${item}" "${BASE}/${item}"
done
# testdata/ は試験の入力（spec の写し）。読む試験（web の doc_links 等）が写しの中でも通るよう、あれば写す。
# 無い repo（回帰テストの擬似 repo）では写さない。変異の対象にはならない。
if [ -d "${REAL_ROOT}/testdata" ]; then
  cp -R "${REAL_ROOT}/testdata" "${BASE}/testdata"
fi

# 全定義を先に検査する。不正な定義があれば、試験を流す前に止める。
for def in "${DEFS[@]}"; do
  parse_def "${def}"
  fresh_work
  occ="$(count_occurrences "${WORK}/${M_TARGET}" "${M_FIND}")"
  if [ "${occ}" != "1" ]; then
    die "${def}: 'find' must occur exactly once in ${M_TARGET} (found ${occ})"
  fi
done

COMMIT="$(git -C "${REAL_ROOT}" rev-parse --short HEAD 2>/dev/null || echo unknown)"
if [ -n "$(git -C "${REAL_ROOT}" status --porcelain 2>/dev/null || true)" ]; then
  COMMIT="${COMMIT}-dirty"
fi

ROWS_FILE="${TMP_DIR}/rows.txt"
: >"${ROWS_FILE}"
survived=0

for def in "${DEFS[@]}"; do
  parse_def "${def}"

  # 元の状態の確認: 試験が通り、expect-fail の名前が実行されていること。
  fresh_work
  log "${M_ID}: checking the unmodified state"
  if ! run_test "${M_PACKAGE}" "${M_TEST}"; then
    tail -n 30 "${OUT}" >&2
    die "${M_ID}: the test fails without the mutation (package=${M_PACKAGE} test=${M_TEST})"
  fi
  for name in "${M_EXPECT[@]}"; do
    grep -Fxq "test ${name} ... ok" "${OUT}" ||
      die "${M_ID}: expected test did not run and pass in the unmodified state: ${name}"
  done

  # 置換を当てる。
  fresh_work
  replace_once "${WORK}/${M_TARGET}" "${M_FIND}" "${M_REPLACE}" "${WORK}/${M_TARGET}.mutated"
  mv "${WORK}/${M_TARGET}.mutated" "${WORK}/${M_TARGET}"

  # コンパイルできない変異は、検出に数えない。
  log "${M_ID}: compiling the mutated copy"
  if ! (cd "${WORK}" && cargo test --locked -p "${M_PACKAGE}" --test "${M_TEST}" --no-run) >"${OUT}" 2>&1; then
    tail -n 30 "${OUT}" >&2
    die "${M_ID}: the mutated copy does not compile"
  fi

  log "${M_ID}: running the test with the mutation"
  run_test "${M_PACKAGE}" "${M_TEST}" || true

  failed=()
  while IFS= read -r name; do
    failed+=("${name}")
  done < <(sed -n 's/^test \(.*\) \.\.\. FAILED$/\1/p' "${OUT}")

  missing=0
  for name in "${M_EXPECT[@]}"; do
    found=0
    if [ "${#failed[@]}" -gt 0 ]; then
      for f in "${failed[@]}"; do
        if [ "${f}" = "${name}" ]; then found=1; fi
      done
    fi
    [ "${found}" -eq 1 ] || missing=1
  done
  if [ "${missing}" -eq 0 ]; then result="DETECTED"; else result="SURVIVED"; survived=1; fi

  # 性質テストが失敗した場合は、種・ケース数・最小の反例を控える。
  prop="-"
  info="$(grep -E '^[[:space:]]*cases=[0-9]+ seed=|^[[:space:]]*minimal input:' "${OUT}" | sed 's/^[[:space:]]*//' | awk '{ printf "%s%s", (NR > 1 ? "; " : ""), $0 }' || true)"
  if [ -n "${info}" ]; then prop="${info}"; fi

  failed_text="-"
  if [ "${#failed[@]}" -gt 0 ]; then failed_text="$(join_by_comma "${failed[@]}")"; fi
  note_text="${M_TARGET}"
  if [ -n "${M_NOTE}" ]; then note_text="${M_TARGET}: ${M_NOTE}"; fi
  printf '| %s | %s | %s | %s | %s | %s |\n' \
    "${M_ID}" "$(md_escape "${note_text}")" "$(md_escape "$(join_by_comma "${M_EXPECT[@]}")")" \
    "$(md_escape "${failed_text}")" "${result}" "$(md_escape "${prop}")" >>"${ROWS_FILE}"
  log "${M_ID}: ${result}"
done

cat <<EOF
実行日: $(date +%F)
元にしたコミット: ${COMMIT}
コマンド: scripts/check-mutations.sh$(for a in "$@"; do printf ' %s' "${a}"; done)

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
EOF
cat "${ROWS_FILE}"

if [ "${survived}" -ne 0 ]; then
  echo "error: at least one mutation survived (no expected test failed)" >&2
  exit 1
fi
