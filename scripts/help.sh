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
  check-mutation-defs          scripts/check-mutation-defs.sh          変異の定義の find の一意性と expect-fail の名前の実在を静的に検査する
  test-check-mutation-defs     scripts/test-check-mutation-defs.sh     変異の定義の静的検査自体の回帰テスト
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
  build-l2                     tools/build-l2.mjs                      L2 の配布物（zip）を作る（L2_SERVER に miniserve が要る。先に make build-web）
  test-build-l2                scripts/test-build-l2.sh                build-l2 の回帰テスト（合成の配信物。ビルドなし）
  test-l2-run                  scripts/test-l2-run.sh                  L2 の配布物を Rust のない模擬環境で展開・起動し、ブラウザで CH-0〜CH-1 を 3 回完走し、待ち受けが 127.0.0.1 だけであることと起動の時間・L1 と L2 の 9 回の結果の一致（DLV-15）、固定の入力列での比較と履歴の ops に揺れる欄が無いこと（TASK-114 2/2）・L1 と L2 の 2 か所と別名 localhost のオリジンの分離（DLV-16。TASK-115）も確かめる（L2_SERVER 必須。遅い）
  check-delivery               tools/check-delivery.sh                 配信物（先に make build-web）に試験用の口が無いことを検査する
  test-check-delivery          scripts/test-check-delivery.sh          配信検査自体の回帰テスト（実ビルドを行うため遅い）
  serve                        node tools/serve.mjs                    dist/web をサブパスの下で gzip つきで配る（--dir --base --port --no-gzip --gzip-level）
  test-serve                   scripts/test-serve.sh                   配信サーバ（tools/serve.mjs）の試験（node:test）
  check-generic-words          tools/check-generic-words.mjs           エンジンの crate のコードに教材固有の語（語彙の付録 A）が無いことを検査する（CI 未組み込み）
  test-check-generic-words     scripts/test-check-generic-words.sh     教材固有の語の検査自体の回帰テスト
  check-internal-info          tools/check-internal-info.mjs           社内固有の語（WORDS=<一覧のパス>）と認証情報の形を照合する（CI 未組み込み）
  check-internal-info-secrets  tools/check-internal-info.mjs           認証情報の形だけを照合する（語の照合は行わない。CI 未組み込み）
  test-check-internal-info     scripts/test-check-internal-info.sh     公開前の照合自体の回帰テスト
  inventory-deps               tools/inventory-deps.mjs                Cargo.lock の依存の取り下げ・所有者の数を調べる（通信が要り数分かかる。CI 未組み込み）
  test-inventory-deps          scripts/test-inventory-deps.sh          依存の棚卸し自体の回帰テスト（通信なし）
  test-upstream-impact         scripts/test-upstream-impact.sh         影響する Step の洗い出し道具の回帰テスト（通信なし。CI 未組み込み）
  test-browser                 scripts/test-browser.sh                 ブラウザ試験（Playwright の共通ヘルパ + 最小の画面 + keydown + 対照の自己確認。FANDHE_BROWSER_CHANNEL=chrome で原本）
  test-build-pack-bin          scripts/test-build-pack-bin.sh          fandhe-pack の前提バイナリの解決が CARGO_TARGET_DIR に追従することの回帰テスト（遅い）
  test-keydown-control         scripts/test-keydown-control.sh         action-keydown を外した写しで keydown の試験が正しい理由で失敗することの対照（遅い）
  test-chapter-key-control     scripts/test-chapter-key-control.sh     鍵を修正前の固定の文字列にした写しで章の試験が正しい理由で失敗することの対照（遅い）
  check-mutations              scripts/check-mutations.sh              変異注入の検査（誤りを入れると対応する試験が失敗することの確認。遅い）
  test-check-mutations         scripts/test-check-mutations.sh         変異注入の検査自体の回帰テスト
  check-wasm-size              scripts/check-wasm-size.sh              配信する設定の WASM の gzip -9 が 1 MB 以下であることの関門（遅い。CI の browser-test）
  test-check-wasm-size         scripts/test-check-wasm-size.sh         大きさの関門自体の回帰テスト（ビルドなし）
  measure-wasm-size            tools/measure/wasm-size.sh              WASM の gzip -9 の大きさを基点つきで測り JSON を出す
  test-measure-wasm-size       scripts/test-measure-wasm-size.sh       wasm-size の様式検証と実ビルドの回帰テスト（遅い）
  measure                      tools/measure/all.sh                    3 つの測定（WASM の gzip・初回表示・再描画）を基点つきで流し JSON を出す（遅い・要ブラウザ）
  measure-history-size         tools/measure/history-size.sh           2 周分の履歴の大きさと Changed の割合を基点つきで測り JSON を出す
  measure-log-items            tools/measure/log-items.sh              2 周分の通しでの右のログの項目数（最大）を基点つきで測り JSON を出す
  test-measure                 scripts/test-measure.sh                 一括測定の様式検証と実行の回帰テスト（遅い・要ブラウザ）
  test-measure-result          scripts/test-measure-result.sh          測定の結果の組み立てが jq 1.7 の文法でも通ることの確認（速い・ビルドなし）
  clean-room                   tools/clean-room.sh                     まっさらなコンテナで cargo test（既定と --workspace）を流して判定する（遅い・docker と通信が要る。CI 未組み込み）
  test-clean-room              scripts/test-clean-room.sh              まっさらなコンテナの試験の判定自体の回帰テスト（docker なし）
  check-build-inventory        scripts/check-build-inventory.sh        ビルド・試験の経路の棚卸し（private な依存 0 件・secret らしい語 0 件。通信なし。CI 未組み込み）
  test-check-build-inventory   scripts/test-check-build-inventory.sh   上の棚卸し検査自体の回帰テスト（見本の repo）

Git hooks（lefthook.yml から呼ばれる。直接実行も可）:

  scripts/hooks/secret-scan.sh          staged 差分の簡易シークレット検知
  scripts/hooks/commit-msg-check.sh F   コミットメッセージ F の Conventional Commits 検証

`make <target>` は上記と同じコマンドを呼ぶだけの薄い入口であり、
処理の定義は scripts/ 側にのみ存在する。
EOF
