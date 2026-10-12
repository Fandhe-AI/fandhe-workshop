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
# Rust 向けの lint / fmt / test / check-deps と、web の wasm ビルド（build-web）を持つ。
# 中身の無いターゲット（`@true` 等）は呼び出し元が終了コードを信用してしまうため置かない。

.DEFAULT_GOAL := help

.PHONY: help doctor setup check test-hooks lint fmt test check-deps test-check-deps check-toolchain test-check-toolchain build-web test-build-web

help: ## このヘルプを表示する（Make 非依存の一覧: scripts/help.sh）
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z0-9_-]+:.*## /{printf "  \033[36mmake %-10s\033[0m %s\n", $$1, $$2}' $(firstword $(MAKEFILE_LIST))

doctor: ## 開発環境を診断する（読み取りのみ・何も導入しない）。直接: scripts/doctor.sh
	@scripts/doctor.sh

setup: ## Git hooks を有効化し .env を雛形から作成する（再実行可）。直接: scripts/setup.sh
	@scripts/setup.sh

check: ## editorconfig-checker + shellcheck（ソースは変更しない）。直接: scripts/check.sh
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

check-toolchain: ## 固定ファイルの版と docs/toolchain.md の一致を検査する。直接: scripts/check-toolchain.sh
	@scripts/check-toolchain.sh

test-check-toolchain: ## ツールチェーンの一致の検査自体の回帰テスト。直接: scripts/test-check-toolchain.sh
	@scripts/test-check-toolchain.sh

build-web: ## web を wasm にビルドして dist/web に配る一式を作る（TEST_HOOKS=1 は試験用）。直接: tools/build-web.sh
	@tools/build-web.sh

test-build-web: ## build-web の受入基準（配信物に試験用の口が無い等）を実ビルドで検査する。直接: scripts/test-build-web.sh
	@scripts/test-build-web.sh
