#!/usr/bin/env bash
# staged 差分に対する簡易シークレット検知（lefthook pre-commit から呼ばれる）。
#
# 保守的なヒューリスティックであり網羅的なスキャナの代替ではない。
# 検知対象:
#   1. .env 系ファイルの追加・変更（.env.example / .sample / .template は雛形として除外）
#   2. ベンダー固有トークン形式・秘密鍵ヘッダー・認証情報入り接続文字列
#   3. API キー / シークレット / トークン / パスワードらしき代入（引用符付き 16 文字以上の英数字値）
#
# .agents/skills/・.claude/skills/ は skills-lock.json で管理する外部取得のスキル文書で、
# 説明用の例示値（接続文字列のサンプル等）を多数含むため 2・3 の走査対象から除外する。
#
# 検出時は値を端末・ログへ出さないよう、該当ファイル名のみを報告する。
# 誤検知時はダミー値・環境変数参照へ書き換えるか、対象行をコミットから外す。
# `--no-verify` によるバイパスは行わない。
set -euo pipefail

high_pattern='sk-[A-Za-z0-9_-]{20,}|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,}|AIza[0-9A-Za-z_-]{35}|BEGIN (RSA |EC |OPENSSH |PGP )?PRIVATE KEY|[a-zA-Z][a-zA-Z0-9+.-]*://[^[:space:]/@:]+:[^[:space:]/@]+@'
assign_pattern='(api[_-]?key|secret|token|password)[[:space:]]*[:=][[:space:]]*["'"'"'][A-Za-z0-9]{16,}'

# looks_secret <line>: いずれかのパターンに一致すれば 0。
# 判定は bash の [[ =~ ]]（POSIX ERE）で行い、grep の実装差・終了コードの取り違えによる
# fail-open 経路を持たない。代入パターンのみ大文字小文字を区別しない。
looks_secret() {
  local line="$1" rc=1
  if [[ "${line}" =~ ${high_pattern} ]]; then
    return 0
  fi
  shopt -s nocasematch
  if [[ "${line}" =~ ${assign_pattern} ]]; then
    rc=0
  fi
  shopt -u nocasematch
  return "${rc}"
}

# 自己テスト: 正規表現が実装差で機能しない環境では検査結果を信用できないため止める。
# 一致させたい例示値は実行時に組み立て、本ファイル自体のコミットが検知されないようにする。
q='"' at='@' dash='-----'
# shellcheck disable=SC2016 # ${DB_PASSWORD} は展開させない例示値
if ! looks_secret "+password = ${q}Hunter2Hunter2Hunter2${q}" \
  || ! looks_secret "+API_KEY: ${q}abcdefghijklmnop1234${q}" \
  || ! looks_secret "+db: postgres://user:pass${at}host:5432/db" \
  || ! looks_secret "+${dash}BEGIN OPENSSH PRIVATE"" KEY${dash}" \
  || looks_secret '+password = "${DB_PASSWORD}"' \
  || looks_secret '+see https://example.com/path'; then
  echo "secret-scan: self-test failed (regex engine mismatch?); aborting" >&2
  exit 2
fi

# サブディレクトリから直接実行しても走査範囲が狭まらないよう、リポジトリルートで実行する
cd "$(git rev-parse --show-toplevel)"

# 1. .env 系ファイル
env_files=""
while IFS= read -r path; do
  [ -z "${path}" ] && continue
  if [[ "${path}" =~ (^|/)\.env(\..+)?$ ]] && ! [[ "${path}" =~ \.(example|sample|template)$ ]]; then
    env_files+="  ${path}"$'\n'
  fi
done <<< "$(git diff --cached --name-only --diff-filter=ACMR)"

if [ -n "${env_files}" ]; then
  echo "commit blocked: .env-like file(s) staged for commit:" >&2
  printf '%s' "${env_files}" >&2
  echo "unstage it (e.g. \`git restore --staged <file>\`) if this was unintentional." >&2
  exit 1
fi

# 2・3. 追加行のみを走査する（削除行を対象にすると漏洩済み値の除去コミットを妨げる）
diff_text="$(git diff --cached -U0 --diff-filter=ACMR -- . \
  ':(exclude).agents/skills/**' ':(exclude).claude/skills/**' ':(exclude)*.lock')"

current_file=""
flagged_files=""
while IFS= read -r line; do
  case "${line}" in
    '+++ b/'*) current_file="${line#+++ b/}" ;;
    '+++'*) ;;
    '+'*)
      if looks_secret "${line}" && [[ "${flagged_files}" != *"  ${current_file}"$'\n'* ]]; then
        flagged_files+="  ${current_file}"$'\n'
      fi
      ;;
  esac
done <<< "${diff_text}"

if [ -n "${flagged_files}" ]; then
  echo "commit blocked: staged diff looks like it contains a hardcoded secret in:" >&2
  printf '%s' "${flagged_files}" >&2
  echo "replace it with a placeholder / env var reference, or move it out of the diff." >&2
  exit 1
fi

exit 0
