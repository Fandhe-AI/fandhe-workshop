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

  help                         scripts/help.sh                         この一覧を表示する
  doctor                       scripts/doctor.sh                       開発環境を診断する（読み取りのみ。何も導入・修復しない）
  setup                        scripts/setup.sh                        Git hooks 有効化と .env 雛形の配置（再実行しても安全）
  check                        scripts/check.sh                        editorconfig-checker + shellcheck + testdata の参照検査（ソースは変更しない）
  test-hooks                   scripts/hooks/test-hooks.sh             フックスクリプト自体の回帰テスト（bash + git のみ）
  lint                         scripts/lint.sh                         Rust の fmt check + clippy + web の wasm32 clippy（ソースは変更しない）
  fmt                          cargo fmt --all                         Rust のソースを rustfmt で整形する（ソースを変更する）
  test                         scripts/test.sh                         Rust の native 試験（web を含む全 crate）
  check-deps                   scripts/check-crate-deps.sh             crate 間の依存の向きを cargo metadata で検査する
  test-check-deps              scripts/test-check-crate-deps.sh        依存の向きの検査自体の回帰テスト
  check-testdata               scripts/check-testdata-refs.sh          repo の外への相対参照が 0 件であることを検査する
  test-check-testdata          scripts/test-check-testdata-refs.sh     参照の検査自体の回帰テスト
  check-dom-reads              tools/check-deps.sh                     web crate が DOM の値を読まないこと（例外は件数固定）を検査する
  test-check-dom-reads         scripts/test-check-dom-reads.sh         DOM 読み取りの検査自体の回帰テスト
  check-frontend-bump          scripts/check-frontend-bump.sh          版上げ PR の受け入れ確認 (4)(5) の関門（feature の明示・変えてはいけないファイル）を検査する（(3) は CI の wasm-gzip）
  test-check-frontend-bump     scripts/test-check-frontend-bump.sh     版上げの受け入れ確認の関門自体の回帰テスト（模擬の版上げ）
  update-test-counts           scripts/update-test-counts.sh           試験の件数の期待値を実測で書き直す（遅い。ブラウザ試験も流す）
  check-toolchain              scripts/check-toolchain.sh              固定ファイルの版と docs/toolchain.md の一致を検査する
  test-check-toolchain         scripts/test-check-toolchain.sh         ツールチェーンの一致の検査自体の回帰テスト
  check-workflow-secrets       scripts/check-workflow-secrets.sh       ci.yml が secret を参照していないことを検査する
  test-check-workflow-secrets  scripts/test-check-workflow-secrets.sh  secret 参照の検査自体の回帰テスト
  check-fork-ci                scripts/check-fork-ci.sh                fork が secret 無しでも CI を通せる構成であることをパーサで検査する（A〜C。D は教材 repo 用で指定時のみ）
  test-check-fork-ci           scripts/test-check-fork-ci.sh           fork CI 検査自体の回帰テスト
  build-web                    tools/build-web.sh                      web を wasm にビルドして dist/web に配る一式を作る（TEST_HOOKS=1 は試験用）
  test-build-web               scripts/test-build-web.sh               build-web の受入基準を実ビルドで検査する
  check-delivery               tools/check-delivery.sh                 配信物（先に make build-web）に試験用の口が無いことを検査する
  test-check-delivery          scripts/test-check-delivery.sh          配信検査自体の回帰テスト（実ビルドを行うため遅い）
  serve                        node tools/serve.mjs                    dist/web をサブパスの下で gzip つきで配る（--dir --base --port --no-gzip --gzip-level）
  test-serve                   scripts/test-serve.sh                   配信サーバ（tools/serve.mjs）の試験（node:test）
  check-generic-words          tools/check-generic-words.mjs           エンジンの crate のコードに教材固有の語（語彙の付録 A）が無いことを検査する（CI 未組み込み）
  test-check-generic-words     scripts/test-check-generic-words.sh     教材固有の語の検査自体の回帰テスト
  inventory-deps               tools/inventory-deps.mjs                Cargo.lock の依存の取り下げ・所有者の数を調べる（通信が要り数分かかる。CI 未組み込み）
  test-inventory-deps          scripts/test-inventory-deps.sh          依存の棚卸し自体の回帰テスト（通信なし）
  test-browser                 scripts/test-browser.sh                 ブラウザ試験（Playwright の共通ヘルパ + 最小の画面 + keydown + 対照の自己確認。FANDHE_BROWSER_CHANNEL=chrome で原本）
  test-build-pack-bin          scripts/test-build-pack-bin.sh          fandhe-pack の前提バイナリの解決が CARGO_TARGET_DIR に追従することの回帰テスト（遅い）
  test-keydown-control         scripts/test-keydown-control.sh         action-keydown を外した写しで keydown の試験が正しい理由で失敗することの対照（遅い）
  check-mutations              scripts/check-mutations.sh              変異注入の検査（誤りを入れると対応する試験が失敗することの確認。遅い）
  test-check-mutations         scripts/test-check-mutations.sh         変異注入の検査自体の回帰テスト
  check-wasm-size              scripts/check-wasm-size.sh              配信する設定の WASM の gzip -9 が 1 MB 以下であることの関門（遅い。CI の browser-test）
  test-check-wasm-size         scripts/test-check-wasm-size.sh         大きさの関門自体の回帰テスト（ビルドなし）
  measure-wasm-size            tools/measure/wasm-size.sh              WASM の gzip -9 の大きさを基点つきで測り JSON を出す
  test-measure-wasm-size       scripts/test-measure-wasm-size.sh       wasm-size の様式検証と実ビルドの回帰テスト（遅い）
  measure                      tools/measure/all.sh                    3 つの測定（WASM の gzip・初回表示・再描画）を基点つきで流し JSON を出す（遅い・要ブラウザ）
  test-measure                 scripts/test-measure.sh                 一括測定の様式検証と実行の回帰テスト（遅い・要ブラウザ）

Git hooks（lefthook.yml から呼ばれる。直接実行も可）:

  scripts/hooks/secret-scan.sh          staged 差分の簡易シークレット検知
  scripts/hooks/commit-msg-check.sh F   コミットメッセージ F の Conventional Commits 検証

`make <target>` は上記と同じコマンドを呼ぶだけの薄い入口であり、
処理の定義は scripts/ 側にのみ存在する。
EOF
