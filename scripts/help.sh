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

  help        scripts/help.sh                この一覧を表示する
  doctor      scripts/doctor.sh              開発環境を診断する（読み取りのみ。何も導入・修復しない）
  setup       scripts/setup.sh               Git hooks 有効化と .env 雛形の配置（再実行しても安全）
  check       scripts/check.sh               editorconfig-checker + shellcheck + testdata の参照検査（ソースは変更しない）
  test-hooks  scripts/hooks/test-hooks.sh    フックスクリプト自体の回帰テスト（bash + git のみ）
  lint        scripts/lint.sh                Rust の fmt check + clippy + web の wasm32 clippy（ソースは変更しない）
  fmt         cargo fmt --all                Rust のソースを rustfmt で整形する（ソースを変更する）
  test        scripts/test.sh                Rust の native 試験（web を含む全 crate）
  check-deps  scripts/check-crate-deps.sh    crate 間の依存の向きを cargo metadata で検査する
  test-check-deps  scripts/test-check-crate-deps.sh  依存の向きの検査自体の回帰テスト
  check-testdata  scripts/check-testdata-refs.sh  repo の外への相対参照が 0 件であることを検査する
  test-check-testdata  scripts/test-check-testdata-refs.sh  参照の検査自体の回帰テスト
  check-toolchain  scripts/check-toolchain.sh  固定ファイルの版と docs/toolchain.md の一致を検査する
  test-check-toolchain  scripts/test-check-toolchain.sh  ツールチェーンの一致の検査自体の回帰テスト
  check-workflow-secrets  scripts/check-workflow-secrets.sh  ci.yml が secret を参照していないことを検査する
  test-check-workflow-secrets  scripts/test-check-workflow-secrets.sh  secret 参照の検査自体の回帰テスト
  check-mutations  scripts/check-mutations.sh  変異注入の検査（誤りを入れると対応する試験が失敗することの確認。遅い）
  test-check-mutations  scripts/test-check-mutations.sh  変異注入の検査自体の回帰テスト

Git hooks（lefthook.yml から呼ばれる。直接実行も可）:

  scripts/hooks/secret-scan.sh          staged 差分の簡易シークレット検知
  scripts/hooks/commit-msg-check.sh F   コミットメッセージ F の Conventional Commits 検証

`make <target>` は上記と同じコマンドを呼ぶだけの薄い入口であり、
処理の定義は scripts/ 側にのみ存在する。
EOF
