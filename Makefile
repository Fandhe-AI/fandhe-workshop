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
# Rust 向けの lint / fmt / test / check-deps / check-dom-reads / check-mutations と、web の wasm ビルド（build-web）、L2 の配布物の作成（build-l2）、ワークフローの secret 参照の検査、ビルド・試験の経路の棚卸し（check-build-inventory。CI 未組み込み）、教材固有の語の検査（check-generic-words。CI 未組み込み）、公開前の社内固有の情報・認証情報の照合（check-internal-info。CI 未組み込み）、依存の棚卸し（inventory-deps。通信が要る。CI 未組み込み）、影響する Step の洗い出しの回帰テスト（test-upstream-impact。CI 未組み込み）を持つ。
# 中身の無いターゲット（`@true` 等）は呼び出し元が終了コードを信用してしまうため置かない。

.DEFAULT_GOAL := help

.PHONY: help doctor setup check test-hooks lint fmt test check-deps test-check-deps check-testdata test-check-testdata check-sibling-free test-check-sibling-freecheck-mutation-defs test-check-mutation-defs check-dom-reads test-check-dom-reads check-toolchain test-check-toolchain check-workflow-secrets test-check-workflow-secrets check-fork-ci test-check-fork-ci build-web test-build-web build-l2 test-build-l2 test-l2-run check-delivery test-check-delivery serve test-serve test-browser test-build-pack-bin test-keydown-control test-chapter-key-control check-mutations test-check-mutations check-wasm-size test-check-wasm-size measure-wasm-size test-measure-wasm-size measure measure-history-size measure-log-items test-measure test-measure-result clean-room test-clean-room check-build-inventory test-check-build-inventory check-generic-words test-check-generic-words check-internal-info check-internal-info-secrets test-check-internal-info inventory-deps test-inventory-deps test-upstream-impact check-frontend-bump test-check-frontend-bump update-test-counts

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

check-sibling-free: ## 兄弟の 03-poc/prototype と docs/spec の中身が無いことを検査する（BUILD-14。spec 取得済みの手元では 1 になる）。直接: scripts/check-sibling-free.sh
	@scripts/check-sibling-free.sh

test-check-sibling-free: ## 兄弟なしの前提の検査自体の回帰テスト。直接: scripts/test-check-sibling-free.sh
	@scripts/test-check-sibling-free.sh

check-mutation-defs: ## 変異の定義の find の一意性と expect-fail の名前の実在を静的に検査する（cargo 不要）。直接: scripts/check-mutation-defs.sh
	@scripts/check-mutation-defs.sh

test-check-mutation-defs: ## 上の静的検査自体の回帰テスト。直接: scripts/test-check-mutation-defs.sh
	@scripts/test-check-mutation-defs.sh

check-dom-reads: ## web crate が DOM の値を読まないこと（例外は件数固定）を検査する。直接: tools/check-deps.sh
	@tools/check-deps.sh

test-check-dom-reads: ## DOM 読み取りの検査自体の回帰テスト。直接: scripts/test-check-dom-reads.sh
	@scripts/test-check-dom-reads.sh

check-frontend-bump: ## 版上げ PR の受け入れ確認 (4)(5) の関門（feature の明示と、版上げ PR で変えてはいけないファイル）を検査する。直接: scripts/check-frontend-bump.sh guard
	@scripts/check-frontend-bump.sh guard

test-check-frontend-bump: ## 版上げの受け入れ確認の関門自体の回帰テスト（模擬の版上げ）。直接: scripts/test-check-frontend-bump.sh
	@scripts/test-check-frontend-bump.sh

update-test-counts: ## 試験の件数の期待値（scripts/expected-test-counts.txt）を実測で書き直す（遅い。ブラウザ試験も流す）。直接: scripts/update-test-counts.sh
	@scripts/update-test-counts.sh

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

build-l2: ## L2 の配布物（zip）を作る。L2_SERVER に miniserve の実行ファイルが要る（先に make build-web）。直接: node tools/build-l2.mjs
	@node tools/build-l2.mjs

test-build-l2: ## build-l2 の回帰テスト（合成の配信物。ビルド・通信なし）。直接: scripts/test-build-l2.sh
	@scripts/test-build-l2.sh

test-l2-run: ## L2 の配布物を Rust のない模擬環境（PATH=/usr/bin:/bin）で展開・起動し、ブラウザで CH-0〜CH-1 を 3 回完走し、待ち受けが 127.0.0.1 だけで他のアドレスから拒否されること・起動とマウントの時間・L1 と L2 の 3 配信先 × 3 回の 9 回で観察・snapshot・history が一致すること（DLV-15）と、凍結した IR と組の固定の入力列での比較・履歴の ops に揺れる欄が無いこと（TASK-114 2/2）・L1 と L2 の 2 か所と別名 localhost のオリジンの分離（DLV-16。TASK-115）を確かめる（L2_SERVER 必須。遅く、CI の browser-test のみ）。直接: scripts/test-l2-run.sh
	@scripts/test-l2-run.sh

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

# WORDS は環境変数 FANDHE_TERM_LIST としてシェルの値で渡す（Make の展開でコマンド文字列へ入れない）。private の spec repo の語の一覧のパス。空なら語の照合を黙って飛ばさず、tools 側が 2 で止める。
check-internal-info: ## 社内固有の語（WORDS=<一覧のパス>）と認証情報の形を照合する（CI 未組み込み）。直接: node tools/check-internal-info.mjs --list <file>
	@FANDHE_TERM_LIST="$$WORDS" node tools/check-internal-info.mjs

check-internal-info-secrets: ## 認証情報の形だけを照合する（語の照合は行わない。CI 未組み込み）。直接: node tools/check-internal-info.mjs --secrets-only
	@node tools/check-internal-info.mjs --secrets-only

test-check-internal-info: ## 公開前の照合自体の回帰テスト（node:test）。直接: scripts/test-check-internal-info.sh
	@scripts/test-check-internal-info.sh

test-browser: ## 共通ヘルパで最小の画面をサブパス配信し、対照の自己確認も行うブラウザ試験（Playwright）。直接: scripts/test-browser.sh
	@scripts/test-browser.sh

test-build-pack-bin: ## fandhe-pack の前提バイナリの解決が CARGO_TARGET_DIR に追従することの回帰テスト（遅い）。直接: scripts/test-build-pack-bin.sh
	@scripts/test-build-pack-bin.sh

test-keydown-control: ## action-keydown を外した写しで keydown の試験が正しい理由で失敗することの対照（遅い）。直接: scripts/test-keydown-control.sh
	@scripts/test-keydown-control.sh

test-chapter-key-control: ## 鍵を修正前の固定の文字列にした写しで章の試験が正しい理由で失敗することの対照（遅い）。直接: scripts/test-chapter-key-control.sh
	@scripts/test-chapter-key-control.sh

check-mutations: ## 変異注入の検査（誤りを入れると対応する試験が失敗することの確認。遅い）。直接: scripts/check-mutations.sh
	@scripts/check-mutations.sh

test-check-mutations: ## 変異注入の検査自体の回帰テスト。直接: scripts/test-check-mutations.sh
	@scripts/test-check-mutations.sh

check-wasm-size: ## 配信する設定の WASM の gzip -9 が 1 MB 以下であることの関門（CI の browser-test。遅い）。直接: scripts/check-wasm-size.sh
	@scripts/check-wasm-size.sh

test-check-wasm-size: ## 大きさの関門自体の回帰テスト（ビルドなし）。直接: scripts/test-check-wasm-size.sh
	@scripts/test-check-wasm-size.sh

measure-wasm-size: ## WASM の gzip -9 の大きさを基点つきで測り JSON を出す。直接: tools/measure/wasm-size.sh
	@tools/measure/wasm-size.sh

test-measure-wasm-size: ## wasm-size の様式検証と実ビルドの回帰テスト（遅い）。直接: scripts/test-measure-wasm-size.sh
	@scripts/test-measure-wasm-size.sh

measure: ## 3 つの測定（WASM の gzip・初回表示・再描画）を基点つきで流し JSON を出す（遅い・要ブラウザ）。直接: tools/measure/all.sh
	@tools/measure/all.sh

measure-history-size: ## 2 周分の履歴の大きさと Changed の割合を基点つきで測り JSON を出す（TASK-216）。直接: tools/measure/history-size.sh
	@tools/measure/history-size.sh

measure-log-items: ## 2 周分の通しでの右のログの項目数（最大）を基点つきで測り JSON を出す（TASK-454）。直接: tools/measure/log-items.sh
	@tools/measure/log-items.sh

test-measure: ## 一括測定の様式検証と実行の回帰テスト（遅い・要ブラウザ）。直接: scripts/test-measure.sh
	@scripts/test-measure.sh

test-measure-result: ## 測定の結果の組み立てが jq 1.7 の文法でも通ることを確かめる（速い・ビルドなし）。直接: scripts/test-measure-result.sh
	@scripts/test-measure-result.sh

clean-room: ## まっさらなコンテナ（公開イメージ・認証情報なし）で cargo test を既定と --workspace で流して判定する（遅い・docker と通信が要る。CI 未組み込み）。直接: tools/clean-room.sh
	@tools/clean-room.sh

test-clean-room: ## まっさらなコンテナの試験の判定自体の回帰テスト（docker なし。CI の check）。直接: scripts/test-clean-room.sh
	@scripts/test-clean-room.sh

check-build-inventory: ## ビルド・試験の経路の棚卸し（Cargo.lock の private な依存 0 件・secret らしい語の参照 0 件）。通信なし。CI 未組み込み。直接: scripts/check-build-inventory.sh
	@scripts/check-build-inventory.sh

test-check-build-inventory: ## 上の棚卸し検査自体の回帰テスト（見本の repo。CI の check）。直接: scripts/test-check-build-inventory.sh
	@scripts/test-check-build-inventory.sh

inventory-deps: ## Cargo.lock の依存の取り下げ・所有者の数を crates.io で調べる（通信が要り数分かかる。CI 未組み込み）。直接: node tools/inventory-deps.mjs
	@node tools/inventory-deps.mjs

test-inventory-deps: ## 依存の棚卸し自体の回帰テスト（node:test。通信なし）。直接: scripts/test-inventory-deps.sh
	@scripts/test-inventory-deps.sh

test-upstream-impact: ## 影響する Step の洗い出し道具の回帰テスト（node:test。通信なし）。道具は直接: node tools/upstream-impact.mjs --help
	@scripts/test-upstream-impact.sh
