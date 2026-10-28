#!/usr/bin/env bash
# ビルド・試験の経路の棚卸し検査（TASK-77 2/2。BUILD-2・PoC-11 の手順 1）。
#
# 役割: 公開の依存だけでビルド・試験が成り立つことを、次の 2 つの不変条件（0 件）で機械的に確かめる。
#   A. Cargo.lock の package に private な依存が 0 件（source が crates.io か、source 無しの workspace member だけ）
#   B. ビルド・試験の経路のファイルから、secret らしい語（GITHUB_TOKEN・CARGO_REGISTR…・git-fetch-with-cli・
#      `secrets.`・識別子の末尾の `_TOKEN`・`_PAT`）の参照が 0 件
# 件数そのものは判定しない。標準出力の「記録」の行を docs/testing.md 5.358 に残す。
# 呼び出し元: `make check-build-inventory`（手で流す。CI の関門にする扱いは TASK-96 の承認事項のため未組み込み）。
# 回帰テストは scripts/test-check-build-inventory.sh（CI の check ジョブ）。
#
# 使い方: scripts/check-build-inventory.sh [--root <dir>]
#   --allowlist-file <file> は回帰テストだけが使う差し替え口（既定の実行では使わない）。
# 終了コード: 0 = 両方 0 件 / 1 = 違反または許可リストの古い項目 / 2 = 誤用・検査を完了できない（2 も合格にしない）
# 必要な道具: git・awk・sha256sum か shasum（ruby・cargo・jq・通信は不要。まっさらなコンテナの中でも流せる）
#
# 範囲（git ls-files の追跡ファイルだけ。コミットした内容）
#   含める: .github/workflows/ci.yml・Makefile・scripts/**・tools/**・crates/**・Cargo.toml・
#           rust-toolchain.toml・package.json・.npmrc・lefthook.yml（ビルド・試験・検査の経路）
#   含めない: deploy.yml（配信の経路。secret を使ってよい設計）・update-external.yml（日次追従で SUBMODULE_PAT を
#             正当に使う）・ai-review.yml・docs/・testdata/・.agents/・.claude/・README.md などの文書
#
# 既存の検査との分担（二重定義をしない）
#   - scripts/check-fork-ci.sh A1 は cargo metadata で同じ source 規則を見る。本検査は Cargo.lock を直接読み、
#     cargo・通信なしで内訳つきの棚卸しを出す。crates.io の source 文字列は同スクリプトの CRATES_REGISTRY と同じ。
#   - 同 A4（.cargo/config の拒否）・B3（uses の SHA 固定）は重複させない。uses は件数の記録だけ。
#   - scripts/check-workflow-secrets.sh の Actions 式の構造的な判定は ruby が要るため呼ばない（CI の check ジョブが持つ）。
#   - tools/inventory-deps.mjs（D-58 の取り下げ・所有者数）とは別物。
# 出力: 違反は `path:line:B token` の形だけ（行の本文・source の URL・値は出さない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
export LC_ALL=C

readonly CRATES_REGISTRY="registry+https://github.com/rust-lang/crates.io-index"

usage() {
  echo "usage: check-build-inventory.sh [--root <dir>]" >&2
}

root="${ROOT_DIR}"
allowlist_file=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --root)
      [ "$#" -ge 2 ] || { usage; exit 2; }
      root="$2"
      shift 2
      ;;
    --allowlist-file)
      [ "$#" -ge 2 ] || { usage; exit 2; }
      allowlist_file="$2"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

fatal() {
  echo "error: $*" >&2
  exit 2
}

for c in git awk mktemp rm cp grep cut sed tr wc; do
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

[ -d "${root}" ] || fatal "--root is not a directory"
root="$(cd "${root}" && pwd)"
[ "$(git -C "${root}" rev-parse --is-inside-work-tree 2>/dev/null || true)" = "true" ] \
  || fatal "--root is not a git work tree"
[ -z "$(git -C "${root}" rev-parse --show-prefix 2>/dev/null)" ] \
  || fatal "--root must be the top of the git work tree"
[ -f "${root}/Cargo.lock" ] || fatal "Cargo.lock not found"

# 許可リスト。形は「pair<TAB>path<TAB>語（小文字）」と「exclude<TAB>path」。
# 古い項目（どこにも一致しない・存在しない）は失敗にして、黙って腐らないようにする。
# 語は識別子の末尾の `_token`・`_pat` などの偶然の一致（secret ではない）と、検出の見本・検査自身の説明に限る。
default_allowlist() {
  cat <<'EOF'
# --- 検出の見本を大量に持つ回帰テストと本検査自身はファイル単位で除外する ---
exclude	scripts/check-build-inventory.sh
exclude	scripts/test-check-build-inventory.sh
exclude	scripts/test-check-workflow-secrets.sh
exclude	scripts/test-check-fork-ci.sh
exclude	tools/browser/secret-warning.test.mjs
# --- (path, 語) の完全一致。語は小文字。実装時点（2026-10-28）の HEAD の一覧 ---
# ファイル名の言及（check-workflow-secrets.sh）
pair	.github/workflows/ci.yml	check-workflow-secrets.sh
# ファイル名の言及（check-workflow-secrets.sh）
pair	Makefile	check-workflow-secrets.sh
# ファイル名の言及（check-workflow-secrets.sh）
pair	Makefile	test-check-workflow-secrets.sh
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	crates/core/src/secret_scan.rs	github_pat
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	crates/core/src/secret_scan.rs	github_token
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	crates/core/src/secret_scan.rs	gitlab_token
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	crates/core/src/secret_scan.rs	npm_token
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	crates/core/src/secret_scan.rs	pypi_token
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	crates/core/src/secret_scan.rs	slack_token
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	crates/core/tests/secret_scan.rs	github_token
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	crates/core/tests/secret_scan.rs	gitlab_token
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	crates/core/tests/secret_scan.rs	npm_token
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	crates/core/tests/secret_scan.rs	pypi_token
# 局所変数のフィールド参照（secret ではない）
pair	crates/web/src/a11y.rs	secrets.min
# 識別子の末尾の偶然の一致（キー名・検索パターン・解決の語。secret ではない）
pair	crates/web/src/action.rs	esc_key_token
# 識別子の末尾の偶然の一致（キー名・検索パターン・解決の語。secret ではない）
pair	crates/web/src/action.rs	esc_key_token.to_owned
# 識別子の末尾の偶然の一致（キー名・検索パターン・解決の語。secret ではない）
pair	crates/web/src/engine_parts.rs	bind_attr_token
# 識別子の末尾の偶然の一致（キー名・検索パターン・解決の語。secret ではない）
pair	crates/web/src/host.rs	esc_key_token
# 識別子の末尾の偶然の一致（キー名・検索パターン・解決の語。secret ではない）
pair	crates/web/tests/action_profile_contract.rs	esc_key_token
# 識別子の末尾の偶然の一致（キー名・検索パターン・解決の語。secret ではない）
pair	crates/web/tests/action_profile_contract.rs	esc_key_token.to_owned
# 識別子の末尾の偶然の一致（キー名・検索パターン・解決の語。secret ではない）
pair	crates/web/tests/no_progress.rs	from_state_pat
# ファイル名の言及（check-workflow-secrets.sh）
pair	scripts/check-fork-ci.sh	check-workflow-secrets.sh
# check-fork-ci B2 の説明と判定の文字列
pair	scripts/check-fork-ci.sh	git-fetch-with-cli
# 識別子の末尾の偶然の一致（キー名・検索パターン・解決の語。secret ではない）
pair	scripts/check-testdata-refs.sh	resolve_token
# ファイル名の言及（check-workflow-secrets.sh）
pair	scripts/check-workflow-secrets.sh	check-workflow-secrets.sh
# コメントの例（check-workflow-secrets の説明）
pair	scripts/check-workflow-secrets.sh	secrets.x
# ファイル名の言及（check-workflow-secrets.sh）
pair	scripts/check-workflow-secrets.sh	test-check-workflow-secrets.sh
# ファイル名の言及（check-workflow-secrets.sh）
pair	scripts/help.sh	check-workflow-secrets.sh
# ファイル名の言及（check-workflow-secrets.sh）
pair	scripts/help.sh	test-check-workflow-secrets.sh
# 識別子の末尾の偶然の一致（キー名・検索パターン・解決の語。secret ではない）
pair	tools/check-deps.sh	scan_pat
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	tools/check-generic-words.mjs	github_token
# 秘密情報の形の ID（検出の定義・見本。値ではない）
pair	tools/check-internal-info.test.mjs	github_token
EOF
}

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

if [ -n "${allowlist_file}" ]; then
  [ -r "${allowlist_file}" ] || fatal "allowlist file is not readable"
  cp "${allowlist_file}" "${tmp}/allow.txt"
else
  default_allowlist > "${tmp}/allow.txt"
fi

rc=0

# ---------------------------------------------------------------- A: Cargo.lock
log "A: reading Cargo.lock"
# workspace member の package 名（root の Cargo.toml の members から各 Cargo.toml の [package] name を読む）
members_list="$(awk '
  /^\[workspace\][ \t]*$/ { inws = 1; next }
  /^\[/ { inws = 0; inmem = 0 }
  inws && /^members[ \t]*=[ \t]*\[/ { inmem = 1; next }
  inmem && /^\][ \t]*,?[ \t]*$/ { inmem = 0; next }
  inmem {
    line = $0
    if (match(line, /"[^"]+"/)) { print substr(line, RSTART + 1, RLENGTH - 2) }
  }
' "${root}/Cargo.toml" 2>/dev/null)" || fatal "cannot read workspace members from Cargo.toml"
[ -n "${members_list}" ] || fatal "no workspace members found in Cargo.toml"

# [workspace.package] の version（member が version.workspace = true で継ぐ場合に使う）
ws_version="$(awk '
  /^\[workspace\.package\][ \t]*$/ { inwp = 1; next }
  /^\[/ { inwp = 0 }
  inwp && /^version[ \t]*=[ \t]*"[^"]+"/ {
    match($0, /"[^"]+"/); print substr($0, RSTART + 1, RLENGTH - 2); exit
  }
' "${root}/Cargo.toml")"

# 名前と版の組（name@version）。同名で別版の workspace 外の path 依存を member と見なさないため
member_names=""
while IFS= read -r m; do
  case "${m}" in /* | *..*) fatal "unsupported workspace member path" ;; esac
  [ -f "${root}/${m}/Cargo.toml" ] || fatal "workspace member Cargo.toml not found"
  n="$(awk '
    /^\[package\][ \t]*$/ { inpkg = 1; next }
    /^\[/ { inpkg = 0 }
    inpkg && /^name[ \t]*=[ \t]*"[^"]+"/ {
      match($0, /"[^"]+"/); print substr($0, RSTART + 1, RLENGTH - 2); exit
    }
  ' "${root}/${m}/Cargo.toml")"
  [ -n "${n}" ] || fatal "workspace member has no package name"
  v="$(awk '
    /^\[package\][ \t]*$/ { inpkg = 1; next }
    /^\[/ { inpkg = 0 }
    inpkg && /^version[ \t]*=[ \t]*"[^"]+"/ {
      match($0, /"[^"]+"/); print substr($0, RSTART + 1, RLENGTH - 2); exit
    }
    inpkg && /^version\.workspace[ \t]*=[ \t]*true[ \t]*$/ { print "@WS"; exit }
  ' "${root}/${m}/Cargo.toml")"
  [ "${v}" != "@WS" ] || v="${ws_version}"
  [ -n "${v}" ] || fatal "workspace member has no package version"
  member_names="${member_names}${n}@${v}"$'\n'
done <<EOF
${members_list}
EOF

awk -v registry="${CRATES_REGISTRY}" -v members="${member_names}" '
  function flush(   ok) {
    if (!inpkg) { return }
    total++
    if (name == "" || version == "") { print "FATAL package without name or version near line " pline; return }
    if (source == "") {
      if ((name "@" version) in ismember) { nmember++ } else {
        print "VIOL Cargo.lock:" pline ":A " name "@" version " no source and not a workspace member"
      }
    } else if (source == registry) { ncrates++ }
    else if (source ~ /^git\+/) {
      ngit++; print "VIOL Cargo.lock:" pline ":A " name "@" version " git source is not in the public allowlist"
    } else {
      nother++; print "VIOL Cargo.lock:" pline ":A " name "@" version " source is not crates.io"
    }
    inpkg = 0
  }
  BEGIN {
    n = split(members, arr, "\n")
    for (i = 1; i <= n; i++) { if (arr[i] != "") { ismember[arr[i]] = 1 } }
  }
  {
    if (indeps) {
      if ($0 ~ /^\]$/) { indeps = 0; next }
      if ($0 ~ /^ "[^"]+",?$/) { next }
      print "FATAL unparsable dependencies entry at line " NR; next
    }
    if ($0 ~ /^#/ || $0 ~ /^$/) { next }
    if (!seenver) {
      if ($0 == "version = 4") { seenver = 1; next }
      print "FATAL Cargo.lock lock format is not version 4"; exit
    }
    if ($0 == "[[package]]") { flush(); inpkg = 1; name = ""; version = ""; source = ""; pline = NR; next }
    if (!inpkg) { print "FATAL unexpected content outside a package at line " NR; next }
    if ($0 ~ /^name = "[^"]+"$/) { name = substr($0, 9, length($0) - 9); next }
    if ($0 ~ /^version = "[^"]+"$/) { version = substr($0, 12, length($0) - 12); next }
    if ($0 ~ /^source = "[^"]+"$/) { source = substr($0, 11, length($0) - 11); next }
    if ($0 ~ /^checksum = "[0-9a-f]+"$/) { next }
    if ($0 == "dependencies = [") { indeps = 1; next }
    print "FATAL unknown key or line at line " NR
  }
  END {
    if (indeps) { print "FATAL unterminated dependencies array at end of file" }
    flush()
    if (!seenver) { print "FATAL Cargo.lock lock format is not version 4" }
    print "COUNT " total + 0 " " ncrates + 0 " " nmember + 0 " " ngit + 0 " " nother + 0
  }
' "${root}/Cargo.lock" > "${tmp}/a.out"

if grep -q '^FATAL ' "${tmp}/a.out"; then
  fatal "Cargo.lock: $(grep -m1 '^FATAL ' "${tmp}/a.out" | cut -c7-)"
fi
count_line="$(grep -m1 '^COUNT ' "${tmp}/a.out")" || fatal "Cargo.lock: no count produced"
read -r _ pk_total pk_crates pk_member pk_git pk_other <<EOF
${count_line}
EOF
private_deps="$((pk_total - pk_crates - pk_member))"
if grep -q '^VIOL ' "${tmp}/a.out"; then
  grep '^VIOL ' "${tmp}/a.out" | cut -c6- | sed 's/^/error: /' >&2
  rc=1
fi

# ---------------------------------------------------------------- B: secret らしい語
log "B: scanning tracked build and test files"
in_scope() {
  case "$1" in
    .github/workflows/ci.yml | Makefile | Cargo.toml | rust-toolchain.toml | package.json | .npmrc | lefthook.yml) return 0 ;;
    scripts/* | tools/* | crates/*) return 0 ;;
  esac
  return 1
}

# 一覧の取得は独立した処理にして終了状態を確認する（process substitution では失敗が伝わらない）
git -C "${root}" ls-files -s -z > "${tmp}/files.bin" || fatal "git ls-files failed"
: > "${tmp}/hits.txt"
scanned=0
while IFS= read -r -d '' rec; do
  meta="${rec%%$'\t'*}"
  path="${rec#*$'\t'}"
  in_scope "${path}" || continue
  mode="${meta%% *}"
  if [ "${mode}" = "120000" ] || [ "${mode}" = "160000" ] || [ -L "${root}/${path}" ]; then
    fatal "symlink or submodule inside the scanned scope: ${path}"
  fi
  [ -f "${root}/${path}" ] || fatal "tracked file is missing from the work tree: ${path}"
  if [ "$(tr -d '\0' < "${root}/${path}" | wc -c)" != "$(wc -c < "${root}/${path}")" ]; then
    fatal "binary file inside the scanned scope: ${path}"
  fi
  scanned=$((scanned + 1))
  # 行の本文は出さず、語だけを取り出す。_token・_pat は識別子の末尾でだけ一致（GITHUB_PATH・*_TOKENS は対象外）
  awk -v path="${path}" '
    function endsws(s, suf) { return length(s) > length(suf) && substr(s, length(s) - length(suf) + 1) == suf }
    {
      line = tolower($0)
      nw = split(line, words, /[^a-z0-9_.-]+/)
      for (i = 1; i <= nw; i++) {
        w = words[i]
        if (w == "") { continue }
        hit = 0
        if (index(w, "github_token") || index(w, "cargo_registr") || index(w, "git-fetch-with-cli") || index(w, "secrets.")) { hit = 1 }
        else {
          ns = split(w, sub_, /[.-]+/)
          for (j = 1; j <= ns; j++) { if (endsws(sub_[j], "_token") || endsws(sub_[j], "_pat")) { hit = 1 } }
        }
        if (hit) {
          t = w
          gsub(/^[.-]+|[.-]+$/, "", t)
          print path "\t" NR "\t" t
        }
      }
    }
  ' "${root}/${path}" >> "${tmp}/hits.txt"
done < "${tmp}/files.bin"

awk -F'\t' '
  FILENAME == ALLOW {
    if ($0 ~ /^#/ || $0 == "") { next }
    if ($1 == "pair") { pair[$2 "\t" $3] = 1; npair++; next }
    if ($1 == "exclude") { excl[$2] = 1; nexcl++; next }
    print "FATAL bad allowlist line " FNR; next
  }
  FILENAME == HITS {
    if ($1 in excl) { exused[$1] = 1; next }
    key = $1 "\t" $3
    if (key in pair) { used[key] = 1; nallowed++; next }
    print "VIOL " $1 ":" $2 ":B " $3 " secret-like reference"
    next
  }
  END {
    for (k in pair) { if (!(k in used)) { split(k, kk, "\t"); print "STALE allowlist:" kk[1] ":" kk[2] " stale" } }
    for (e in excl) { if (!(e in exused)) { print "STALE allowlist:" e " excluded file has no match or does not exist" } }
    print "COUNT " npair + 0 " " nexcl + 0 " " nallowed + 0
  }
' ALLOW="${tmp}/allow.txt" HITS="${tmp}/hits.txt" "${tmp}/allow.txt" "${tmp}/hits.txt" > "${tmp}/b.out"

if grep -q '^FATAL ' "${tmp}/b.out"; then
  fatal "allowlist: $(grep -m1 '^FATAL ' "${tmp}/b.out" | cut -c7-)"
fi
secret_refs="$(grep -c '^VIOL ' "${tmp}/b.out" || true)"
stale="$(grep -c '^STALE ' "${tmp}/b.out" || true)"
if [ "${secret_refs}" != "0" ]; then
  grep '^VIOL ' "${tmp}/b.out" | cut -c6- | sed 's/^/error: /' >&2
  rc=1
fi
if [ "${stale}" != "0" ]; then
  grep '^STALE ' "${tmp}/b.out" | cut -c7- | sed 's/^/error: /' >&2
  rc=1
fi
b_count="$(grep -m1 '^COUNT ' "${tmp}/b.out")" || fatal "allowlist: no count produced"
read -r _ al_pairs al_excl al_hits <<EOF
${b_count}
EOF

# ---------------------------------------------------------------- 記録
lock_sha="$("${sha_cmd[@]}" "${root}/Cargo.lock" | cut -d' ' -f1)"
commit="$(git -C "${root}" rev-parse --short HEAD 2>/dev/null || echo unknown)"
uses_count="n/a"
if [ -f "${root}/.github/workflows/ci.yml" ]; then
  uses_count="$(grep -c -E '^[[:space:]]*(-[[:space:]]+)?uses:' "${root}/.github/workflows/ci.yml" || true)"
fi
result="pass"
[ "${rc}" = "0" ] || result="fail"

echo "build inventory: commit=${commit} cargo_lock_sha256=${lock_sha}"
echo "cargo-lock: version=4 packages=${pk_total} crates-io=${pk_crates} workspace-members=${pk_member} git=${pk_git} other=${pk_other}"
echo "private-deps: ${private_deps}"
echo "workflow-uses (ci.yml, record only; pinning is judged by check-fork-ci B3): ${uses_count}"
echo "secret-refs: ${secret_refs} (scanned-files=${scanned} allowlisted-pairs=${al_pairs} allowlisted-hits=${al_hits} excluded-files=${al_excl} stale=${stale})"
echo "result: ${result}"
exit "${rc}"
