# 開発用 Makefile。
#
# Make は scripts/*.sh を呼ぶだけの薄い入口であり、処理の定義は scripts/ 側にのみ置く
# （各レシピは 1 行の呼び出しに限定し、ここでチェック内容を二重定義しない）。
# Make を使わない場合の同等コマンドは scripts/help.sh を参照。
#
# GNU Make 3.81（macOS 標準）で動作確認済み。3.82+ 専用機能（.ONESHELL・.RECIPEPREFIX 等）は使わない。
#
# 変更時は .claude/rules/makefile.md に従う。
#
# 使い方: `make help`（既定ターゲット）でターゲット一覧を表示する。
# 一覧は下記の `## ` コメントから生成されるため、`## ` の無いターゲットは表示されない。
# ターゲットを追加・改名・削除したら scripts/help.sh も同じ変更で更新すること。
#
# Rust 向けの lint / fmt / test / check-deps / check-dom-reads / check-mutations と、web の wasm ビルド（build-web）、ワークフローの secret 参照の検査、教材固有の語の検査（check-generic-words。CI 未組み込み）、依存の棚卸し（inventory-deps。通信が要る。CI 未組み込み）を持つ。
# 中身の無いターゲット（`@true` 等）は呼び出し元が終了コードを信用してしまうため置かない。

.DEFAULT_GOAL := help

.PHONY: help doctor setup check test-hooks lint fmt test check-deps test-check-deps check-testdata test-check-testdata check-dom-reads test-check-dom-reads check-toolchain test-check-toolchain check-workflow-secrets test-check-workflow-secrets check-fork-ci test-check-fork-ci build-web test-build-web check-delivery test-check-delivery serve test-serve test-browser test-build-pack-bin test-keydown-control check-mutations test-check-mutations measure-wasm-size test-measure-wasm-size measure test-measure check-generic-words test-check-generic-words inventory-deps test-inventory-deps

# 説明文の開始桁を揃えるため、1 パス目で最長のターゲット名の幅を求め、2 パス目で出力する
# （同じファイルを 2 回読む）。幅を固定しないので、長い名前を足しても再びずれない。
help: ## このヘルプを表示する（Make 非依存の一覧: scripts/help.sh）
	@awk 'BEGIN {FS = ":.*## "} NR == FNR {if ($$0 ~ /^[a-zA-Z0-9_-]+:.*## / && length($$1) > w) w = length($$1); next} /^[a-zA-Z0-9_-]+:.*## / {printf "  \033[36mmake %-" w "s\033[0m %s\n", $$1, $$2}' $(firstword $(MAKEFILE_LIST)) $(firstword $(MAKEFILE_LIST))

doctor: ## 開発環境を診断する（読み取りのみ・何も導入しない）。直接: scripts/doctor.sh
	@scripts/doctor.sh

setup: ## Git hooks を有効化し .env を雛形から作成する（再実行可）。直接: scripts/setup.sh
	@scripts/setup.sh

check: ## editorconfig-checker + shellcheck + testdata の参照検査（ソースは変更しない）。直接: scripts/check.sh
	@scripts/check.sh

test-hooks: ## scripts/hooks/ のフックスクリプト自体の回帰テスト（bash + git のみ）。直接: scripts/hooks/test-hooks.sh
	@scripts/hooks/test-hooks.sh

lint: ## Rust の fmt check + clippy + web の wasm32 clippy（ソースは変更しない）。直接: scripts/lint.sh
	@scripts/lint.sh

fmt: ## Rust のソースを rustfmt で整形する（ソースを変更する）。直接: cargo fmt --all
	@cargo fmt --all

test: ## Rust の native 試験（web を含む全 crate）。直接: scripts/test.sh
	@scripts/test.sh

check-deps: ## crate 間の依存の向きを cargo metadata で検査する。直接: scripts/check-crate-deps.sh
	@scripts/check-crate-deps.sh

test-check-deps: ## 依存の向きの検査自体の回帰テスト。直接: scripts/test-check-crate-deps.sh
	@scripts/test-check-crate-deps.sh

check-testdata: ## repo の外への相対参照が 0 件であることを検査する。直接: scripts/check-testdata-refs.sh
	@scripts/check-testdata-refs.sh

test-check-testdata: ## 参照の検査自体の回帰テスト。直接: scripts/test-check-testdata-refs.sh
	@scripts/test-check-testdata-refs.sh

check-dom-reads: ## web crate が DOM の値を読まないこと（例外は件数固定）を検査する。直接: tools/check-deps.sh
	@tools/check-deps.sh

test-check-dom-reads: ## DOM 読み取りの検査自体の回帰テスト。直接: scripts/test-check-dom-reads.sh
	@scripts/test-check-dom-reads.sh

check-toolchain: ## 固定ファイルの版と docs/toolchain.md の一致を検査する。直接: scripts/check-toolchain.sh
	@scripts/check-toolchain.sh

test-check-toolchain: ## ツールチェーンの一致の検査自体の回帰テスト。直接: scripts/test-check-toolchain.sh
	@scripts/test-check-toolchain.sh

check-workflow-secrets: ## ci.yml が secret を参照していないことを検査する。直接: scripts/check-workflow-secrets.sh
	@scripts/check-workflow-secrets.sh

test-check-workflow-secrets: ## secret 参照の検査自体の回帰テスト。直接: scripts/test-check-workflow-secrets.sh
	@scripts/test-check-workflow-secrets.sh

check-fork-ci: ## fork が secret 無しでも CI を通せる構成であることをパーサで検査する（A〜C。D は --pack-workflow 指定時のみ。ruby・jq・cargo が必要）。直接: scripts/check-fork-ci.sh
	@scripts/check-fork-ci.sh

test-check-fork-ci: ## fork CI 検査自体の回帰テスト。直接: scripts/test-check-fork-ci.sh
	@scripts/test-check-fork-ci.sh

build-web: ## web を wasm にビルドして dist/web に配る一式を作る（TEST_HOOKS=1 は試験用）。直接: tools/build-web.sh
	@tools/build-web.sh

test-build-web: ## build-web の受入基準（配信物に試験用の口が無い等）を実ビルドで検査する。直接: scripts/test-build-web.sh
	@scripts/test-build-web.sh

check-delivery: ## 配信物（dist/web。先に make build-web）に試験用の口が無いことを検査する。直接: tools/check-delivery.sh
	@tools/check-delivery.sh

test-check-delivery: ## 配信検査自体の回帰テスト（実ビルドを行うため遅い）。直接: scripts/test-check-delivery.sh
	@scripts/test-check-delivery.sh

serve: ## dist/web をサブパスの下で gzip つきで配る（引数は直接渡す）。直接: node tools/serve.mjs
	@node tools/serve.mjs

test-serve: ## 配信サーバ（tools/serve.mjs）の試験（node:test）。直接: scripts/test-serve.sh
	@scripts/test-serve.sh

check-generic-words: ## エンジンの crate のコードに教材固有の語（付録 A）が無いことを検査する（CI 未組み込み）。直接: node tools/check-generic-words.mjs
	@node tools/check-generic-words.mjs

test-check-generic-words: ## 教材固有の語の検査自体の回帰テスト（node:test）。直接: scripts/test-check-generic-words.sh
	@scripts/test-check-generic-words.sh

test-browser: ## 共通ヘルパで最小の画面をサブパス配信し、対照の自己確認も行うブラウザ試験（Playwright）。直接: scripts/test-browser.sh
	@scripts/test-browser.sh

test-build-pack-bin: ## fandhe-pack の前提バイナリの解決が CARGO_TARGET_DIR に追従することの回帰テスト（遅い）。直接: scripts/test-build-pack-bin.sh
	@scripts/test-build-pack-bin.sh

test-keydown-control: ## action-keydown を外した写しで keydown の試験が正しい理由で失敗することの対照（遅い）。直接: scripts/test-keydown-control.sh
	@scripts/test-keydown-control.sh

check-mutations: ## 変異注入の検査（誤りを入れると対応する試験が失敗することの確認。遅い）。直接: scripts/check-mutations.sh
	@scripts/check-mutations.sh

test-check-mutations: ## 変異注入の検査自体の回帰テスト。直接: scripts/test-check-mutations.sh
	@scripts/test-check-mutations.sh

measure-wasm-size: ## WASM の gzip -9 の大きさを基点つきで測り JSON を出す。直接: tools/measure/wasm-size.sh
	@tools/measure/wasm-size.sh

test-measure-wasm-size: ## wasm-size の様式検証と実ビルドの回帰テスト（遅い）。直接: scripts/test-measure-wasm-size.sh
	@scripts/test-measure-wasm-size.sh

measure: ## 3 つの測定（WASM の gzip・初回表示・再描画）を基点つきで流し JSON を出す（遅い・要ブラウザ）。直接: tools/measure/all.sh
	@tools/measure/all.sh

test-measure: ## 一括測定の様式検証と実行の回帰テスト（遅い・要ブラウザ）。直接: scripts/test-measure.sh
	@scripts/test-measure.sh

inventory-deps: ## Cargo.lock の依存の取り下げ・所有者の数を crates.io で調べる（通信が要り数分かかる。CI 未組み込み）。直接: node tools/inventory-deps.mjs
	@node tools/inventory-deps.mjs

test-inventory-deps: ## 依存の棚卸し自体の回帰テスト（node:test。通信なし）。直接: scripts/test-inventory-deps.sh
	@scripts/test-inventory-deps.sh
