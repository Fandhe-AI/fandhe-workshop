#!/usr/bin/env bash
# エンジンの差分 0 の確認（TASK-752。SECOND-1・W-20・D-91）。
#
# 役割: 第 2 教材をエンジンの Rust を変えずに足せたことを、固定した commit からの差分の行数と、ファイルの
#   SHA-256 の一覧の照合で数えて示す。1 つの教材で差分 0 でも、すべての教材で 0 とは言えない（SECOND-20）。
#   その限界と実測の記録は docs/engine-zero-diff/review-request/README.md に置く。
# 呼び出し元: `make check-engine-zero-diff`（手で流す。CI は回帰テストだけを流す。本体を CI に入れない理由は
#   README の「CI について」）。回帰テストは scripts/test-check-engine-zero-diff.sh。
#
# 数える範囲（判定する）: crates/ の全体（src・tests・static・各 crate の Cargo.toml）・直下の Cargo.toml・Cargo.lock。
# 参考の行（数えないが出す）: rust-toolchain.toml。
# 数えない範囲（INFO の行に行数だけ出す）: template-workshop/（雛形と教材の例）・docs/・scripts/・tools/・
#   testdata/・.github/・その他の直下のファイル。教材の追加はここに現れるのが正しい。
# 注意: 測る対象を変えないため、この検査の道具と試験は crates/ の下に置かない。
#
# 項目（1 項目 1 行。`PASS <名前>` / `FAIL <名前>: <理由>` / `SKIP <名前>: <理由>`）
#   base-commit      base と head が commit に解決でき、base が head の祖先
#   base-before-pack 教材のパスが base に無く、窓の中で最初に足した commit の親が base（0 になる窓を選んでいない確認）
#   manifest-base    base から作った一覧が、記録した一覧とバイト単位で一致
#   manifest-head    head から作った一覧が、記録した一覧と全件一致（changed / missing / extra をパスだけで出す）
#   manifest-count   head の件数が記録した件数と一致
#   diff-lines       git diff の追加と削除の合計が 0（--no-renames。バイナリは 1 件の変更）
#   cargo-toml       直下の Cargo.toml の行数が 0
#   cargo-lock       Cargo.lock の行数が 0
#   crate-manifests  crates/*/Cargo.toml の行数が 0
#   worktree-clean   head が作業ツリーの HEAD のときだけ、範囲に未コミットの変更・未追跡のファイルが無い
#
# 一覧は git の blob から作る（作業ツリーの改行の変換や .gitattributes に左右されない）。形は sha256sum -c と同じ
# `<sha256>  <path>` で、git の木の並びのまま書く。出力はパス・SHA・行数だけで、ファイルの中身は出さない。
#
# 使い方:
#   scripts/check-engine-zero-diff.sh [--root <dir>] [--base <rev>] [--head <rev>] [--manifest <file>]
#                                     [--pack <path>] [--write-manifest]
#   --write-manifest は base から一覧を作って --manifest に書く（判定しない。記録の作り直しに使う）。
#   エンジンが後から変わった場合は、記録した「測った HEAD」の SHA を --head に渡して再測定する。
# 終了コード: 0 = 全項目合格 / 1 = 不合格 / 2 = 誤用・前提の欠け（shallow で base が無い・SHA-256 の道具が無い等。
#   2 も合格にしない）
# 必要な道具: git・awk・sed・cmp・head・wc・mktemp・sha256sum か shasum（cargo・通信は不要）
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
export LC_ALL=C

readonly DEFAULT_BASE="505fef25de073875d814500e76a9e597e4513639"
readonly DEFAULT_MANIFEST="docs/engine-zero-diff/review-request/engine-manifest.sha256"
readonly DEFAULT_PACK="template-workshop/examples/review-request"
# 数える範囲。git のパス指定として使う。
readonly -a SCOPE=(crates Cargo.toml Cargo.lock)

usage() {
  echo "usage: check-engine-zero-diff.sh [--root <dir>] [--base <rev>] [--head <rev>] [--manifest <file>] [--pack <path>] [--write-manifest]" >&2
}

fatal() {
  echo "error: $*" >&2
  exit 2
}

# 先頭が - ・改行・.. を含む値は、git のオプションやパスの抜けとして解釈されうるので断る。
check_value() {
  local name="$1" v="$2" kind="$3"
  [ -n "${v}" ] || fatal "${name} must not be empty"
  case "${v}" in
    -*) fatal "${name} must not start with '-'" ;;
    *$'\n'*) fatal "${name} must not contain a newline" ;;
  esac
  if [ "${kind}" = "rev" ]; then
    case "${v}" in
      *..*) fatal "${name} must not contain '..'" ;;
    esac
  else
    case "${v}" in
      .. | ../* | */../* | */..) fatal "${name} must not contain a '..' component" ;;
    esac
  fi
}

root="${ROOT_DIR}"
base="${DEFAULT_BASE}"
head_rev="HEAD"
manifest=""
pack="${DEFAULT_PACK}"
write_manifest=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --write-manifest)
      write_manifest=1
      shift
      ;;
    --root | --base | --head | --manifest | --pack)
      [ "$#" -ge 2 ] || { usage; exit 2; }
      case "$1" in
        --root) root="$2" ;;
        --base) base="$2" ;;
        --head) head_rev="$2" ;;
        --manifest) manifest="$2" ;;
        --pack) pack="$2" ;;
      esac
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

for c in git awk sed cmp head wc mktemp rm cat mv; do
  require_cmd "${c}" || exit 2
done
sha_cmd=()
if command -v sha256sum >/dev/null 2>&1; then
  sha_cmd=(sha256sum)
elif command -v shasum >/dev/null 2>&1; then
  sha_cmd=(shasum -a 256)
else
  fatal "required command not found on PATH: sha256sum or shasum"
fi

check_value "--root" "${root}" path
check_value "--base" "${base}" rev
check_value "--head" "${head_rev}" rev
check_value "--pack" "${pack}" path
[ -d "${root}" ] || fatal "--root is not a directory"
root="$(cd "${root}" && pwd)"
[ "$(git -C "${root}" rev-parse --is-inside-work-tree 2>/dev/null || true)" = "true" ] \
  || fatal "--root is not a git work tree"
[ -z "$(git -C "${root}" rev-parse --show-prefix 2>/dev/null)" ] \
  || fatal "--root must be the top of the git work tree"

if [ -z "${manifest}" ]; then
  manifest="${root}/${DEFAULT_MANIFEST}"
else
  check_value "--manifest" "${manifest}" path
  case "${manifest}" in
    /*) ;;
    *) manifest="${root}/${manifest}" ;;
  esac
fi

# rev を完全な commit の SHA に解決する。解決できなければ前提の欠け（shallow で無い場合を含む）として 2。
resolve() {
  git -C "${root}" rev-parse --verify --quiet "$1^{commit}" 2>/dev/null \
    || fatal "cannot resolve $2 '$1' to a commit (shallow clone? fetch more history)"
}
base_sha="$(resolve "${base}" "--base")"
head_sha="$(resolve "${head_rev}" "--head")"

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

# build_manifest <sha> <out>: 数える範囲のファイルの SHA-256 の一覧を blob から作る。
# blob 以外（symlink・submodule）があれば <out>.bad に種類とパスを書く。
build_manifest() {
  local rev="$1" out="$2" rec meta path mode type oid h
  : > "${out}"
  : > "${out}.bad"
  git -C "${root}" ls-tree -r -z "${rev}" -- "${SCOPE[@]}" > "${out}.ls" \
    || fatal "git ls-tree failed for ${rev}"
  while IFS= read -r -d '' rec; do
    meta="${rec%%$'\t'*}"
    path="${rec#*$'\t'}"
    read -r mode type oid <<< "${meta}"
    if [ "${type}" != "blob" ] || [ "${mode}" = "120000" ]; then
      printf '%s %s\n' "${type}" "${path}" >> "${out}.bad"
      continue
    fi
    h="$(git -C "${root}" cat-file blob "${oid}" | "${sha_cmd[@]}")" \
      || fatal "cannot hash a blob in ${rev}"
    printf '%s  %s\n' "${h%% *}" "${path}" >> "${out}"
  done < "${out}.ls"
}

if [ "${write_manifest}" = "1" ]; then
  build_manifest "${base_sha}" "${tmp}/base"
  [ ! -s "${tmp}/base.bad" ] || fatal "non-blob entries in scope (see check-engine-zero-diff.sh without --write-manifest)"
  [ -s "${tmp}/base" ] || fatal "no files in scope at base"
  mkdir_dir="$(dirname "${manifest}")"
  [ -d "${mkdir_dir}" ] || fatal "manifest directory does not exist"
  cat "${tmp}/base" > "${tmp}/out"
  mv "${tmp}/out" "${manifest}"
  echo "wrote $(wc -l < "${manifest}" | tr -d ' ') entries from ${base_sha}"
  exit 0
fi

[ -f "${manifest}" ] || fatal "manifest file not found"

failed=0
pass() { echo "PASS $1${2:+ ($2)}"; }
fail() { echo "FAIL $1: $2"; failed=1; }
skip() { echo "SKIP $1: $2"; }

# --- base-commit ---
if git -C "${root}" merge-base --is-ancestor "${base_sha}" "${head_sha}"; then
  pass base-commit "base ${base_sha} is an ancestor of head ${head_sha}"
else
  fail base-commit "base ${base_sha} is not an ancestor of head ${head_sha}"
fi

# --- base-before-pack ---
if [ -n "$(git -C "${root}" ls-tree "${base_sha}" -- "${pack}")" ]; then
  fail base-before-pack "${pack} already exists at base"
else
  first_added="$(git -C "${root}" log --diff-filter=A --reverse --format='%H %P' "${base_sha}..${head_sha}" -- "${pack}" | head -n 1)"
  if [ -z "${first_added}" ]; then
    fail base-before-pack "${pack} was never added between base and head"
  else
    added_sha="${first_added%% *}"
    added_parents="${first_added#* }"
    if [ "${added_parents}" = "${base_sha}" ]; then
      pass base-before-pack "first added in ${added_sha}, whose parent is base"
    else
      fail base-before-pack "first added in ${added_sha}, whose parent is not base"
    fi
  fi
fi

# --- manifest-* ---
rec_count="$(wc -l < "${manifest}" | tr -d ' ')"
build_manifest "${base_sha}" "${tmp}/base"
if [ -s "${tmp}/base.bad" ]; then
  fail manifest-base "non-blob entries in scope at base: $(tr '\n' ';' < "${tmp}/base.bad")"
elif cmp -s "${tmp}/base" "${manifest}"; then
  pass manifest-base "$(wc -l < "${tmp}/base" | tr -d ' ') entries match the record byte for byte"
else
  fail manifest-base "the record differs from the list rebuilt at base (do not edit it by hand)"
fi

build_manifest "${head_sha}" "${tmp}/head"
if [ -s "${tmp}/head.bad" ]; then
  fail manifest-head "non-blob entries in scope at head: $(tr '\n' ';' < "${tmp}/head.bad")"
else
  # 記録（ファイル 1）と head（ファイル 2）をパスで突き合わせ、食い違いをパスだけで出す。
  awk '
    function path(l) { return substr(l, 67) }
    NR == FNR { rec[path($0)] = substr($0, 1, 64); next }
    { p = path($0); seen[p] = 1
      if (!(p in rec)) print "extra " p
      else if (rec[p] != substr($0, 1, 64)) print "changed " p }
    END { for (p in rec) if (!(p in seen)) print "missing " p }
  ' "${manifest}" "${tmp}/head" > "${tmp}/mdiff"
  if [ -s "${tmp}/mdiff" ]; then
    fail manifest-head "$(wc -l < "${tmp}/mdiff" | tr -d ' ') path(s) differ from the record"
    sed 's/^/  /' "${tmp}/mdiff"
  else
    pass manifest-head "all $(wc -l < "${tmp}/head" | tr -d ' ') entries match the record"
  fi
fi
head_count="$(wc -l < "${tmp}/head" | tr -d ' ')"
if [ "${head_count}" = "${rec_count}" ]; then
  pass manifest-count "${head_count} entries at head, ${rec_count} in the record"
else
  fail manifest-count "${head_count} entries at head, ${rec_count} in the record"
fi

# --- diff-* ---
git -C "${root}" diff --numstat --no-renames "${base_sha}" "${head_sha}" -- "${SCOPE[@]}" > "${tmp}/numstat"
git -C "${root}" diff --numstat --no-renames "${base_sha}" "${head_sha}" > "${tmp}/numstat-all"

# sum_lines <awk 条件>: 条件に合うパスの追加と削除の合計（バイナリの - は 1 件の変更として 1 を足す）。
sum_lines() {
  awk -F '\t' -v cond="$1" '
    function match_path(p) {
      if (cond == "all") return 1
      if (cond == "root-toml") return p == "Cargo.toml"
      if (cond == "root-lock") return p == "Cargo.lock"
      if (cond == "crate-toml") return p ~ /^crates\/[^\/]+\/Cargo\.toml$/
      return 0
    }
    match_path($3) { a = ($1 == "-") ? 1 : $1; d = ($2 == "-") ? 1 : $2; s += a + d }
    END { print s + 0 }
  ' "${tmp}/numstat"
}

report_sum() { # <name> <cond> <label>
  local n
  n="$(sum_lines "$2")"
  if [ "${n}" = "0" ]; then
    pass "$1" "0 lines in $3"
  else
    fail "$1" "${n} changed line(s) in $3"
  fi
}
report_sum diff-lines all "crates/, Cargo.toml and Cargo.lock"
if [ "$(sum_lines all)" != "0" ]; then
  sed 's/^/  numstat /' "${tmp}/numstat"
  git -C "${root}" log --format='  commit %h %s' "${base_sha}..${head_sha}" -- "${SCOPE[@]}"
fi
report_sum cargo-toml root-toml "the root Cargo.toml"
report_sum cargo-lock root-lock "Cargo.lock"
report_sum crate-manifests crate-toml "crates/*/Cargo.toml"

# --- worktree-clean ---
if [ "${head_sha}" = "$(git -C "${root}" rev-parse --verify HEAD)" ]; then
  dirty="$(git -C "${root}" status --porcelain --untracked-files=all -- "${SCOPE[@]}")"
  if [ -z "${dirty}" ]; then
    pass worktree-clean "no uncommitted or untracked file in scope"
  else
    fail worktree-clean "uncommitted or untracked file(s) in scope:"
    printf '%s\n' "${dirty}" | sed 's/^/  /'
  fi
else
  skip worktree-clean "head is not the work tree's HEAD, so the work tree was not checked"
fi

# --- INFO（判定に使わない）---
echo "INFO base ${base_sha}"
echo "INFO head ${head_sha}"
echo "INFO window commits $(git -C "${root}" rev-list --count "${base_sha}..${head_sha}")"
ts_lines="$(git -C "${root}" diff --numstat --no-renames "${base_sha}" "${head_sha}" -- rust-toolchain.toml \
  | awk -F '\t' '{ a = ($1 == "-") ? 1 : $1; d = ($2 == "-") ? 1 : $2; s += a + d } END { print s + 0 }')"
echo "INFO reference rust-toolchain.toml ${ts_lines} line(s)"
awk -F '\t' '
  $3 ~ /^crates\// || $3 == "Cargo.toml" || $3 == "Cargo.lock" || $3 == "rust-toolchain.toml" { next }
  { n = split($3, parts, "/"); top = (n > 1) ? parts[1] "/" : "(root files)"
    a = ($1 == "-") ? 1 : $1; d = ($2 == "-") ? 1 : $2
    files[top]++; add[top] += a; del[top] += d }
  END { for (t in files) printf "INFO uncounted %s files=%d added=%d deleted=%d\n", t, files[t], add[t], del[t] }
' "${tmp}/numstat-all" | LC_ALL=C sort

if [ "${failed}" = "0" ]; then
  echo "result: engine diff is 0 lines between ${base_sha} and ${head_sha}"
  exit 0
fi
echo "result: FAIL"
exit 1
