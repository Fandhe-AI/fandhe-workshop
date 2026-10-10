#!/usr/bin/env bash
# Make を介さない操作一覧。`make help` は Makefile の `## ` コメントから
# 機械的に生成されるのに対し、本ファイルは手動で保守する一覧であり、
# 両者を自動で同期する仕組みは無い。Makefile のターゲットを追加・改名・
# 削除したら同じ変更で本ファイルも更新すること。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

cat <<'EOF'
このリポジトリの操作と、Make を使わずに直接実行する方法:

  help     scripts/help.sh      この一覧を表示する
  doctor   scripts/doctor.sh    開発環境を診断する（読み取りのみ。何も導入・修復しない）
  setup    scripts/setup.sh     Git hooks 有効化と .env 雛形の配置（再実行しても安全）
  check    scripts/check.sh     editorconfig-checker + shellcheck（ソースは変更しない）

Git hooks（lefthook.yml から呼ばれる。直接実行も可）:

  scripts/hooks/secret-scan.sh          staged 差分の簡易シークレット検知
  scripts/hooks/commit-msg-check.sh F   コミットメッセージ F の Conventional Commits 検証

`make <target>` は上記と同じコマンドを呼ぶだけの薄い入口であり、
処理の定義は scripts/ 側にのみ存在する。
EOF
