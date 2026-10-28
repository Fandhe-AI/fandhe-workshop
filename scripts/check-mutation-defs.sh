#!/usr/bin/env bash
# 変異の定義の静的な検査（`make check` から常に呼ばれる）。変異のビルドも cargo も使わない。
#
# 変異の定義（crates/*/tests/mutations/*.mutation）は、コードの変更（処理の移動・rustfmt の折り返し・試験の改名）で
# 黙って古くなる。動的な検査（check-mutations.sh）は全件で約 2 時間かかり CI に入らないため、古さに気づくのが遅れる。
# そこで次の 2 点を、全件について数秒で確かめる。
#   1. find が target にちょうど 1 回現れること（数え方は check-mutations.sh と同じ lib-mutations.sh の count_occurrences）
#   2. expect-fail の名前が、対象 crate の試験ターゲット（tests/<test>.rs または tests/<test>/main.rs）に実在すること
# 違反は最初の 1 件で止めず、全件を調べてからすべて列挙して失敗する。
#
# 既知の近似: 名前の照合は `fn <name>(` と `mod <name> {` の字句の照合である。#[test] の有無・mod の入れ子の位置・
# 同名の別関数は区別しない。tests/common/ は探さない。元の状態で試験が通ること、変異がコンパイルでき検出されることは、
# 従来どおり動的な検査（check-mutations.sh）の担当である。
#
# 終了コード: 0 = 違反なし / 1 = 古い定義がある / 2 = 定義の形式の不正・誤用・検査を完了できない（fail-closed）。
# 回帰テストは scripts/test-check-mutation-defs.sh。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd awk || exit 2
require_cmd grep || exit 2
require_cmd mktemp || exit 2

REAL_ROOT="$(cd "${ROOT_DIR}" && pwd -P)"
# shellcheck source=./lib-mutations.sh
source "${DIR}/lib-mutations.sh"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT
PKG_MAP="${TMP_DIR}/packages.txt"
VIOLATIONS="${TMP_DIR}/violations.txt"
: >"${PKG_MAP}"
: >"${VIOLATIONS}"

# package 名 → crate のディレクトリ名の表を 1 回だけ作る（"<package> <dir>" の行）。
for manifest in "${REAL_ROOT}"/crates/*/Cargo.toml; do
  [ -f "${manifest}" ] || continue
  crate_dir="$(basename "$(dirname "${manifest}")")"
  pkg="$(awk '
    /^\[/ { in_pkg = ($0 == "[package]"); next }
    in_pkg && /^name[[:space:]]*=/ {
      v = $0; sub(/^[^"]*"/, "", v); sub(/".*$/, "", v); print v; exit
    }
  ' "${manifest}")"
  [ -n "${pkg}" ] || die "${manifest}: no [package] name"
  printf '%s %s\n' "${pkg}" "${crate_dir}" >>"${PKG_MAP}"
done

DEFS=()
for f in "${REAL_ROOT}"/crates/*/tests/mutations/*.mutation; do
  [ -f "${f}" ] && DEFS+=("${f}")
done
[ "${#DEFS[@]}" -gt 0 ] || die "no mutation definitions found under crates/*/tests/mutations/"

# violation <message>: 違反を 1 行記録する（最後にまとめて列挙する）。
violation() {
  printf '%s\n' "$*" >>"${VIOLATIONS}"
}

names_total=0
for def in "${DEFS[@]}"; do
  parse_def "${def}"
  rel="${def#"${REAL_ROOT}"/}"

  occ="$(count_occurrences "${REAL_ROOT}/${M_TARGET}" "${M_FIND}")"
  if [ "${occ}" != "1" ]; then
    violation "${rel}: find occurs ${occ} times in ${M_TARGET} (expected 1)"
  fi

  matches="$(awk -v p="${M_PACKAGE}" '$1 == p { print $2 }' "${PKG_MAP}")"
  case "${matches}" in
    "") die "${rel}: unknown package: ${M_PACKAGE}" ;;
    *$'\n'*) die "${rel}: package name is ambiguous: ${M_PACKAGE}" ;;
  esac
  tests_dir="${REAL_ROOT}/crates/${matches}/tests"
  src="${tests_dir}/${M_TEST}.rs"
  [ -f "${src}" ] || src="${tests_dir}/${M_TEST}/main.rs"
  [ -f "${src}" ] || die "${rel}: test target source not found: crates/${matches}/tests/${M_TEST}.rs"

  for name in "${M_EXPECT[@]}"; do
    names_total=$((names_total + 1))
    rest="${name}"
    leaf="${name##*::}"
    # 前置きの mod（a::b::leaf の a・b）→ 末尾の fn の順に、固定文字列で探す。
    while [ "${rest}" != "${leaf}" ]; do
      seg="${rest%%::*}"
      rest="${rest#*::}"
      grep -Fq -- "mod ${seg} {" "${src}" || violation "${rel}: expect-fail not found in ${src#"${REAL_ROOT}"/}: ${name}"
    done
    grep -Fq -- "fn ${leaf}(" "${src}" || violation "${rel}: expect-fail not found in ${src#"${REAL_ROOT}"/}: ${name}"
  done
done

if [ -s "${VIOLATIONS}" ]; then
  while IFS= read -r line; do
    echo "error: ${line}" >&2
  done <"${VIOLATIONS}"
  echo "error: $(wc -l <"${VIOLATIONS}" | tr -d ' ') stale mutation definition problem(s)" >&2
  exit 1
fi

log "mutation definitions: ${#DEFS[@]} files, ${names_total} expect-fail names, all consistent"
