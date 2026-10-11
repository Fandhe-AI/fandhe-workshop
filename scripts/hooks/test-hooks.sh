#!/usr/bin/env bash
# scripts/hooks/ のフックスクリプト自体の回帰テスト（CI の test-hooks ジョブ・`make test-hooks`）。
#
# 一時ディレクトリに使い捨ての git リポジトリを作り、secret-scan.sh の検出・許可ケースと
# commit-msg-check.sh の OK / NG ケースを実際のスクリプトで検証する。
# 外部依存は bash と git のみ。フックではなく CI で実行する（.claude/rules/git-hooks.md）。
#
# 回帰ケースは過去に見つかった fail-open の不具合クラスに対応する:
#   (a) 検査エラーを「一致なし」と同一視する
#   (b) 非 UTF-8 の内容でロケール依存の照合が一致しなくなる
#   (c) hunk 内の `++ b/...` で始まる追加行を `+++ b/` ヘッダと誤認する
#   (d) 利用者の git 設定（色・外部 diff・textconv・prefix）で diff の出力形式が変わる
#   (e) git の失敗がパイプ・here-string に隠れる
#
# フックは本スクリプトを実行している bash（${BASH}）で起動する。macOS 標準の bash 3.2 での
# 互換性は `/bin/bash scripts/hooks/test-hooks.sh` で確認できる（本スクリプトも 3.2 互換で書く）。
#
# 検出させる例示値はすべて実行時に組み立て、本ファイル自体のコミットが secret-scan に
# 検知されないようにする。
set -euo pipefail

if [ "$#" -ne 0 ]; then
  echo "usage: $(basename "$0") (no arguments)" >&2
  exit 2
fi

HOOKS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SECRET_SCAN="${HOOKS_DIR}/secret-scan.sh"
COMMIT_MSG_CHECK="${HOOKS_DIR}/commit-msg-check.sh"
HOOK_BASH="${BASH}"

# テストハーネス自身の文字列比較はバイト単位で行う。フックは下の UTF-8 ロケールで起動する。
export LC_ALL=C

# (b) の回帰を実際に再現させるため、フックは UTF-8 ロケールで起動する（フック側が内部で
# C ロケールへ切り替えることを検証する）。見つからない環境では検証にならないため失敗させる。
# （`locale -a | grep -q` は grep の早期終了で locale が SIGPIPE になり pipefail で偽になり得るため、
# 一覧を変数へ受けてから照合する）
available_locales="$(locale -a 2>/dev/null || true)"
utf8_locale=""
for candidate in C.UTF-8 C.utf8 en_US.UTF-8 en_US.utf8; do
  if [[ $'\n'"${available_locales}"$'\n' == *$'\n'"${candidate}"$'\n'* ]]; then
    utf8_locale="${candidate}"
    break
  fi
done
if [ -z "${utf8_locale}" ]; then
  echo "test-hooks: no UTF-8 locale available (need C.UTF-8 or en_US.UTF-8)" >&2
  exit 1
fi

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/test-hooks.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT

# 開発者・CI ランナーの git 設定（署名・色・フック・テンプレート）から隔離する。
# lefthook 経由で起動された場合に備え、親リポジトリを指す環境変数も外す。
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_OBJECT_DIRECTORY \
  GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_CONFIG_PARAMETERS GIT_CONFIG_COUNT
export HOME="${work_dir}/home"
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=hook-test GIT_AUTHOR_EMAIL=hook-test@example.invalid
export GIT_COMMITTER_NAME=hook-test GIT_COMMITTER_EMAIL=hook-test@example.invalid
mkdir -p "${HOME}"

# (e) 用: 指定した引数を含む git 呼び出しだけを失敗させるシム。実体の git は先に解決しておく。
real_git="$(command -v git)"
shim_dir="${work_dir}/shim"
mkdir -p "${shim_dir}"
cat > "${shim_dir}/git" <<EOF
#!/bin/sh
for arg in "\$@"; do
  if [ "\${arg}" = "\${FAIL_GIT_ARG:-}" ]; then
    echo "git shim: forced failure on \${arg}" >&2
    exit 128
  fi
done
exec "${real_git}" "\$@"
EOF
chmod +x "${shim_dir}/git"

# 例示値（実行時に組み立てる）
q='"'
at='@'
dash='-----'
token="gh""p_$(printf 'A1b2C3d4E5f6G7h8I9j0K1l2')"
password_value="Hunter2Hunter2Hunter2"
password_line="password = ${q}${password_value}${q}"
dsn_line="db: postgres://app:s3cr3tpass${at}db.internal:5432/app"
key_line="${dash}BEGIN OPENSSH PRIVATE"" KEY${dash}"
# 出力に値そのものが含まれていないことを確認する対象
leak_needles="${token}
${password_value}
s3cr3tpass"

passed=0
failed=0
repo_seq=0
repo=""

pass() {
  passed=$((passed + 1))
  echo "ok - $1"
}

fail() {
  failed=$((failed + 1))
  echo "not ok - $1"
  shift
  local detail
  for detail in "$@"; do
    echo "    ${detail}"
  done
}

# new_repo: 空コミット 1 つを持つ使い捨てリポジトリを作り、cwd をそこへ移す。
new_repo() {
  repo_seq=$((repo_seq + 1))
  repo="${work_dir}/repo-${repo_seq}"
  mkdir -p "${repo}"
  cd "${repo}"
  git -c init.defaultBranch=main init -q
  git commit -q --allow-empty -m "chore: init"
}

# check_scan <説明> <期待する終了コード> [報告されるべきファイル名]
# cwd で secret-scan.sh を UTF-8 ロケールで実行し、終了コード・報告ファイル名・値の非出力を検証する。
check_scan() {
  local desc="$1" want="$2" want_file="${3:-}" rc=0 out needle
  LC_ALL="${utf8_locale}" "${HOOK_BASH}" "${SECRET_SCAN}" > "${work_dir}/out" 2>&1 || rc=$?
  out="$(cat "${work_dir}/out")"
  if [ "${rc}" -ne "${want}" ]; then
    fail "secret-scan: ${desc}" "expected exit ${want}, got ${rc}" "output: ${out}"
    return 0
  fi
  if [ -n "${want_file}" ] && [[ "${out}" != *$'\n'"  ${want_file}"$'\n'* ]]; then
    fail "secret-scan: ${desc}" "expected report of file: ${want_file}" "output: ${out}"
    return 0
  fi
  while IFS= read -r needle; do
    if [[ "${out}" == *"${needle}"* ]]; then
      fail "secret-scan: ${desc}" "secret value leaked to output"
      return 0
    fi
  done <<< "${leak_needles}"
  pass "secret-scan: ${desc}"
}

# stage <path> <内容（printf の書式ではなくそのまま書き込む）>
stage() {
  mkdir -p "$(dirname "$1")"
  printf '%s\n' "$2" > "$1"
  git add -- "$1"
}

echo "# hooks under test run with: ${HOOK_BASH} (${BASH_VERSION}), locale ${utf8_locale}"

# ---------------------------------------------------------------- secret-scan: 検出
new_repo; stage src/app.txt "x = ${token}"
check_scan "vendor token in new file" 1 src/app.txt

new_repo; stage config.yml "${password_line}"
check_scan "quoted password assignment" 1 config.yml

new_repo; stage config.yml "API_KEY: ${q}abcdefghijklmnop1234${q}"
check_scan "api key assignment (case-insensitive)" 1 config.yml

new_repo; stage config.yml "${dsn_line}"
check_scan "connection string with credentials" 1 config.yml

new_repo; stage id.txt "${key_line}"
check_scan "private key header" 1 id.txt

new_repo; stage notes.txt "first"; git commit -q -m "chore: add"
printf '%s\n%s\n' "first" "x = ${token}" > notes.txt; git add notes.txt
check_scan "token added to existing file" 1 notes.txt

new_repo; stage .env "A=1"
check_scan ".env staged" 1 .env

new_repo; stage config/.env.local "A=1"
check_scan ".env.local in subdirectory staged" 1 config/.env.local

new_repo; stage "日本語/.env" "A=1"
check_scan ".env under non-ASCII directory (core.quotePath)" 1 "日本語/.env"

new_repo; stage "dir with space/app.txt" "x = ${token}"
check_scan "file name with spaces is reported correctly" 1 "dir with space/app.txt"

new_repo; stage src/app.txt "x = ${token}"; mkdir -p sub/dir; cd sub/dir
check_scan "run from a subdirectory scans the whole index" 1 src/app.txt

new_repo; mkdir -p .agents/skills/x; stage .agents/skills/x/doc.md "x = ${token}"
git commit -q -m "chore: vendored"; mkdir -p src; git mv .agents/skills/x/doc.md src/doc.md
check_scan "file moved out of an excluded path" 1 src/doc.md

# (b) 非 UTF-8 のバイトと同じ行にあるトークン
new_repo; stage latin1.txt "caf"$'\xe9'" ${token}"
check_scan "(b) Latin-1 byte on the same line" 1 latin1.txt

new_repo; stage bytes.txt $'\xff\xfe'" ${token}"
check_scan "(b) invalid UTF-8 prefix on the same line" 1 bytes.txt

# (c) 追加行の本文が `++` で始まる（diff 上は `+++...` になる）
new_repo; stage app.txt "++ b/other.txt ${token}"
check_scan "(c) added line starting with '++ b/' is scanned" 1 app.txt

new_repo; stage app.txt "++${token}"
check_scan "(c) added line starting with '++' is scanned" 1 app.txt

new_repo; printf '%s\n%s\n' "++ b/decoy.txt" "x = ${token}" > app.txt; git add app.txt
check_scan "(c) '++ b/' line in a hunk does not change the reported file" 1 app.txt

# (d) 利用者の git 設定
new_repo; git config color.diff always; stage app.txt "x = ${token}"
check_scan "(d) color.diff=always" 1 app.txt

new_repo; git config color.ui always; stage app.txt "x = ${token}"
check_scan "(d) color.ui=always" 1 app.txt

new_repo; git config diff.external true; stage app.txt "x = ${token}"
check_scan "(d) diff.external" 1 app.txt

new_repo; printf '%s\n' '*.txt diff=hide' > .gitattributes; git config diff.hide.textconv 'sh -c :'
stage app.txt "x = ${token}"
check_scan "(d) textconv driver" 1 app.txt

new_repo; git config diff.noprefix true; stage app.txt "x = ${token}"
check_scan "(d) diff.noprefix keeps file attribution" 1 app.txt

new_repo; git config diff.mnemonicPrefix true; stage app.txt "x = ${token}"
check_scan "(d) diff.mnemonicPrefix keeps file attribution" 1 app.txt

new_repo; git config diff.renames copies; mkdir -p .claude/skills/x
stage .claude/skills/x/doc.md "x = ${token}"; git commit -q -m "chore: vendored"
cp .claude/skills/x/doc.md copy.md; git add copy.md
check_scan "(d) diff.renames=copies does not hide a copied file" 1 copy.md

# ---------------------------------------------------------------- secret-scan: 許可
new_repo
check_scan "nothing staged" 0

new_repo; stage app.txt "hello"
check_scan "clean file" 0

new_repo; stage .env.example "A=changeme"; stage .env.sample "A=1"; stage .env.template "A=1"
check_scan ".env.example / .sample / .template" 0

new_repo
# shellcheck disable=SC2016 # ${DB_PASSWORD} は展開させない例示値
stage config.yml 'password = "${DB_PASSWORD}"'
check_scan "env var placeholder" 0

new_repo; stage README.md "see https://example.com/path and user${at}example.com"
check_scan "URL without credentials" 0

new_repo; stage .agents/skills/x/SKILL.md "x = ${token}"; stage .claude/skills/y/SKILL.md "${dsn_line}"
stage deps.lock "x = ${token}"
check_scan "excluded paths (.agents/skills, .claude/skills, *.lock)" 0

new_repo; stage leaked.txt "$(printf '%s\n%s' "keep" "x = ${token}")"; git commit -q -m "chore: leak"
printf '%s\n' "keep" > leaked.txt; git add leaked.txt
check_scan "removing a leaked secret is allowed" 0

new_repo; stage leaked.txt "x = ${token}"; git commit -q -m "chore: leak"; git rm -q leaked.txt
check_scan "deleting a file with a secret is allowed" 0

# ---------------------------------------------------------------- secret-scan: 検査エラー（fail-closed）
# (e) git の失敗を「対象なし」として合格させない
new_repo; stage .env "A=1"
PATH="${shim_dir}:${PATH}" FAIL_GIT_ARG=--name-only check_scan "(e) failure listing staged files" 2

new_repo; stage app.txt "x = ${token}"
PATH="${shim_dir}:${PATH}" FAIL_GIT_ARG=-U0 check_scan "(e) failure reading staged diff" 2

new_repo; stage app.txt "hello"
PATH="${shim_dir}:${PATH}" FAIL_GIT_ARG=--show-toplevel check_scan "(e) failure resolving repository root" 2

mkdir -p "${work_dir}/not-a-repo"; cd "${work_dir}/not-a-repo"
GIT_CEILING_DIRECTORIES="${work_dir}" check_scan "(e) outside a git repository" 2

# ---------------------------------------------------------------- commit-msg-check
# check_msg <説明> <期待する終了コード>（メッセージは ${work_dir}/msg に書いておく）
check_msg() {
  local desc="$1" want="$2" rc=0 out
  LC_ALL="${utf8_locale}" "${HOOK_BASH}" "${COMMIT_MSG_CHECK}" "${work_dir}/msg" > "${work_dir}/out" 2>&1 || rc=$?
  out="$(cat "${work_dir}/out")"
  if [ "${rc}" -ne "${want}" ]; then
    fail "commit-msg: ${desc}" "expected exit ${want}, got ${rc}" "output: ${out}"
    return 0
  fi
  pass "commit-msg: ${desc}"
}

msg_ok() {
  printf '%s\n' "$1" > "${work_dir}/msg"
  check_msg "OK: $(printf '%q' "$1")" 0
}

msg_ng() {
  printf '%s\n' "$1" > "${work_dir}/msg"
  check_msg "NG: $(printf '%q' "$1")" 1
}

for type in feat fix docs style refactor perf test build ci chore revert; do
  msg_ok "${type}: summary"
done
msg_ok "feat(auth): ソーシャルログイン機能を追加"
msg_ok "feat(api)!: レスポンス形式を変更"
msg_ok "fix(docs-site2): typo"
msg_ok "feat: caf"$'\xe9'" (Latin-1 subject)"
msg_ok "chore: CRLF line"$'\r'
msg_ok $'\n# comment\n\nfeat: subject after comments and blank lines'
msg_ok "Merge branch 'main' into topic"
msg_ok "Revert \"feat: something\""
msg_ok "fixup! feat: something"
msg_ok "squash! feat: something"
printf 'feat: no trailing newline' > "${work_dir}/msg"
check_msg "OK: subject without trailing newline" 0

msg_ng "bad message"
msg_ng "Feat: capitalized type"
msg_ng "feat(Auth): uppercase scope"
msg_ng "feat(a b): space in scope"
msg_ng "feat:missing space"
msg_ng "feat: "
msg_ng "feature: unknown type"
msg_ng "# only a comment"
: > "${work_dir}/msg"
check_msg "NG: empty file" 1

rc=0
LC_ALL="${utf8_locale}" "${HOOK_BASH}" "${COMMIT_MSG_CHECK}" "${work_dir}/missing" > /dev/null 2>&1 || rc=$?
if [ "${rc}" -eq 1 ]; then pass "commit-msg: NG: missing message file"; else fail "commit-msg: NG: missing message file" "expected exit 1, got ${rc}"; fi

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
