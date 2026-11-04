#!/usr/bin/env bash
# 変異の定義（crates/*/tests/mutations/*.mutation）を読む共有の関数。source 専用で、直接は実行しない。
# check-mutations.sh（動的な実行）と check-mutation-defs.sh（静的な検査）が同じ読み方・同じ数え方を使うよう、
# 定義の形式の検査（parse_def）・target の検査（check_target）・出現回数の数え方（count_occurrences）を一か所に置く。
#
# 前提: 呼び出し側が `set -euo pipefail` を有効にし、lib.sh を source し、REAL_ROOT（repo 根の実パス）を
# 定めてから、このファイルを source する。parse_def の形式違反は die（終了コード 2）で止める（fail-closed）。

# shellcheck disable=SC2034 # M_* は source した側（check-mutations.sh・check-mutation-defs.sh）が読む
# die <message>: 英語の error を出して終了コード 2 で止める。
die() {
  echo "error: $*" >&2
  exit 2
}

# 1 件分の定義を保持する変数。parse_def が埋める。
M_ID="" M_PACKAGE="" M_TEST="" M_TARGET="" M_FIND="" M_REPLACE="" M_NOTE="" M_FEATURES=""
M_EXPECT=()

# parse_def <file>: 定義を読み、形式を検査する。不正なら終了コード 2。
# 各行は最初の ": " で key と value に分ける。未知のキー・重複・欠落・空の値は拒否する。
parse_def() {
  local file="$1" line key value
  local seen_id=0 seen_package=0 seen_test=0 seen_target=0 seen_find=0 seen_replace=0 seen_note=0
  M_ID="" M_PACKAGE="" M_TEST="" M_TARGET="" M_FIND="" M_REPLACE="" M_NOTE="" M_FEATURES=""
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
      features) [ -z "${M_FEATURES}" ] || die "${file}: duplicate key: features"; M_FEATURES="${value}" ;;
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
  # features は任意（試験の設定の feature の背後にある試験を対象にするとき）。cargo の --features に渡す。
  if [ -n "${M_FEATURES}" ]; then
    printf '%s\n' "${M_FEATURES}" | grep -Eq '^[a-z0-9-]+$' || die "${file}: invalid features: ${M_FEATURES}"
  fi
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
