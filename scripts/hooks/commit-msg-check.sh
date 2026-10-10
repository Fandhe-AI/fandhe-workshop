#!/usr/bin/env bash
# Conventional Commits 形式をシェル正規表現で検証する commit-msg フック本体。
# npm 依存の commitlint は導入せず、依存を増やさない。
#
# 呼び出し元: lefthook.yml の commit-msg（第 1 引数にメッセージファイルのパスが渡る）。
#
# 許容形式:
#   <type>[(<scope>)][!]: <要約>
#   例: feat(auth): ソーシャルログイン機能を追加
#       feat(api)!: レスポンス形式を変更
#       chore: 依存を更新
# type は feat/fix/docs/style/refactor/perf/test/build/ci/chore/revert に限定する。
# scope は英小文字・数字・ハイフンのみ。複数領域にまたがる変更では省略可。
#
# Merge / Revert（git 既定文言）・fixup! / squash! は git 標準操作・作業中コミットを
# 不必要に止めないため素通りさせる。
set -euo pipefail

msg_file="${1:-}"
if [ -z "${msg_file}" ] || [ ! -f "${msg_file}" ]; then
  echo "commit-msg-check: commit message file not found: ${msg_file}" >&2
  exit 1
fi

# コメント行（`#` 始まり）と空行を除いた最初の行を件名として扱う。
subject=""
while IFS= read -r line || [ -n "${line}" ]; do
  case "${line}" in
    '#'*) continue ;;
    '') continue ;;
    *) subject="${line}"; break ;;
  esac
done < "${msg_file}"

if [ -z "${subject}" ]; then
  echo "commit-msg-check: empty commit message subject" >&2
  exit 1
fi

case "${subject}" in
  Merge\ *|Revert\ *|fixup!\ *|squash!\ *)
    exit 0
    ;;
esac

pattern='^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-z0-9-]+\))?!?: .+'
if [[ "${subject}" =~ ${pattern} ]]; then
  exit 0
fi

cat >&2 <<EOF
commit-msg-check: commit message does not follow Conventional Commits format.

  subject: ${subject}

expected: <type>[(<scope>)][!]: <summary>
  type  = feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert
  scope = lowercase letters, digits, hyphens (e.g. auth, api, docs-site); optional

example: feat(auth): ソーシャルログイン機能を追加
EOF
exit 1
