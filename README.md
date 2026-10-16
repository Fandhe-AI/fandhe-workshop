# fandhe-workshop

体験型教材を作成・実行する汎用 OSS エンジンの実装リポジトリです。
教材作成者が章・Step・入力条件・状態変化・完了条件・ヒントをデータとして書くと、
ブラウザの中で、ターミナル風の操作に応じて成果物（ファイル・差分・Issue・PR／CI の状態など）が変わる教材として動くことを目指します。
エンジンは特定の教材の知識を持たず、教材は別のリポジトリで作成します。

開発基盤は [Fandhe-AI/template-dev](https://github.com/Fandhe-AI/template-dev) から作成しています。
言語固有の設定・実装は、実装着手の指示後にタスクに沿って追加します。

## 概要

教材作成者が書いたデータ（教材パック）を、ビルド時に IR へコンパイルし、実行時はエンジン（中核）が IR だけを読んで動かす 3 層の構成です。

| 層 | 内容 |
|---|---|
| 教材パック | 章・Step・入力条件・状態変化・完了条件・ヒントを、Markdown と構造定義で書いたもの |
| IR | 教材パックをビルド時に検証・コンパイルして作る JSON。実行時に読まれるのはこれだけ |
| エンジン（中核） | IR を読み、ターミナル風の操作に応じて成果物の状態を進める。特定の教材の知識を持たない |

用語は [用語集](./docs/glossary.md) にまとめています。
2026-10-15 時点では、Cargo ワークスペースの 4 crate のうち pack crate に、教材パックの読み込み層と `fandhe-pack compile <pack-dir>` と `compare <pack-dir-a> <pack-dir-b>` を実装済みです（TASK-39）。エンジン（core）の実行側と web はまだ骨格です。このリポジトリには開発基盤・構成設計（[docs/architecture.md](./docs/architecture.md)）・ワークスペースがあります。

## 仮称の一覧

次の名前はすべて仮称です。確定した名前に変えるときは、この表と [docs/architecture.md](./docs/architecture.md) を同じ変更で直します。

| 対象 | 仮称 | 状態（2026-10-12 時点） |
|---|---|---|
| エンジンのリポジトリ | `fandhe-workshop` | 本リポジトリ。public で作成済み。製品名は未確定 |
| 教材のリポジトリ | `fandhe-tutorials` | 未確認（GitHub から参照できなかった。作成済みかどうかは不明） |
| 外部向けの雛形 | `template-workshop` | 初回は本リポジトリ内のディレクトリに置く方針（[docs/architecture.md](./docs/architecture.md) の第 8 節）。独立リポジトリにする時期は未定 |
| crate（4 つ） | `fandhe-workshop-contract`・`fandhe-workshop-core`・`fandhe-workshop-pack`・`fandhe-workshop-web` | `crates/` 配下。pack に読み込み層と `compile`・`compare` を実装済みで、他は骨格中心（2026-10-15 時点）。役割は [docs/architecture.md](./docs/architecture.md) の第 3 節 |
| 作成者向け CLI | `fandhe-pack` | pack crate に置く。`compile <pack-dir>`（IR の JSON を標準出力へ書く）と `compare <pack-dir-a> <pack-dir-b>`（IR のバイト一致を判定する）を実装済み。外部仕様は未固定 |

## 仕様（`docs/spec`）

仕様・フェーズ文書は private リポジトリ `Fandhe-AI/fandhe-workshop-spec` を `docs/spec` に
submodule として置いています（閲覧には権限が必要です）。

認証の無い環境での clone が失敗しないよう `.gitmodules` で `update = none` を指定しているため、
`git clone --recurse-submodules` や `git submodule update --init` では取得されません。
spec リポジトリへの読み取り権限がある環境で、次のように明示的に取得します。

```bash
git -c submodule.docs/spec.update=checkout submodule update --init docs/spec
```

参照の更新は `update-external.yml` が日次で PR を作成します（後述の「CI」）。

## ライセンス

MIT OR Apache-2.0 のデュアルライセンスです（[LICENSE-MIT](./LICENSE-MIT) / [LICENSE-APACHE](./LICENSE-APACHE)）。

## 構成

| パス | 役割 |
|---|---|
| `.agents/skills/`・`.claude/skills/` | エージェント用スキル（`skills-lock.json` で管理する外部取得物。`.claude/skills/` は `.agents/skills/` への symlink） |
| `.claude/rules/` | Claude Code のルール（`git-hooks.md`: `lefthook.yml`・`scripts/hooks/` 編集時のみ読み込まれる、フックの時間予算と CI との役割分担、`makefile.md`: `Makefile`・`scripts/*.sh` 編集時のみ読み込まれる、薄い入口の方針とターゲット追加手順、`github-actions.md`: `.github/workflows/` 編集時のみ読み込まれる、runner 方針・必須チェック・発火条件・ジョブ追加チェックリスト・キャッシュ方針、`rust-build-profile.md`: `Cargo.toml`・`.cargo/`・`rust-toolchain.toml` 編集時のみ読み込まれる、Rust 導入時の dev プロファイル・`target/` の掃除・CI キャッシュへの影響。このほか、委譲・セキュリティ・日本語スタイル・Conventional Commits・スコープ外の追跡・シェルと Rust（`coding-rust.md`: `**/*.rs`・`**/Cargo.toml` 編集時のみ読み込まれる）のコーディング規約・コメント規約の共通ルールがあり、一覧と読み込み条件は `CLAUDE.md` の「Rules」に記載） |
| `CLAUDE.md` | Claude Code 向けの要約（構成・委譲方針・model 配分・Sub-agents／Rules／Skills の一覧） |
| `AGENTS.md` | AI エージェントと PR 自動レビュー（ai-review）が共通で使うレビュー観点集（セキュリティ・設計整合・再利用の 3 観点と優先度 P0 / P1 / P2）。PR の base コミットの内容が基準になるため、変更はマージ後の次の PR から有効 |
| `.claude/agents/` | Claude Code の Sub-agents 定義（`research` / `implement` / `testing` / `quality` / `docs` のカテゴリ別。一覧は `CLAUDE.md` の「Sub-agents」） |
| `.claude/settings.json` | Claude Code のプロジェクト共有設定（SessionStart hook のリマインダー。個人設定は git 管理外の `.claude/settings.local.json`） |
| `.claude/workflows/` | `implement-issue-tree` の workflow 定義（スキル側の実体への相対 symlink） |
| `Cargo.toml`・`Cargo.lock` | Cargo ワークスペース（仮想 manifest。lint の設定・dev プロファイル・release プロファイルを持つ）と、固定した依存の版 |
| `crates/` | 4 crate（`contract`・`core`・`pack`・`web`）の骨格。`default-members` は `web` を含まない |
| `testdata/` | 試験データの置き場。spec repo の試験データを出所付きで写したもの（出所・規則は `testdata/README.md`）。整形ツールの対象外 |
| `rust-toolchain.toml`・`package.json`・`package-lock.json`・`.npmrc` | ツールチェーンの版の固定（Rust・Node・Playwright。値の正は `docs/toolchain.md`） |
| `Makefile` | `scripts/` を呼ぶだけの薄い入口（`make help` で一覧） |
| `scripts/` | 処理の実体（`help` / `doctor` / `setup` / `check` / `lint` / `test` / `check-crate-deps` / `test-check-crate-deps` / `check-testdata-refs` / `test-check-testdata-refs` / `test-check-dom-reads` / `test-check-delivery` / `check-toolchain` / `test-check-toolchain` / `check-workflow-secrets` / `test-check-workflow-secrets` / `ci-rust-toolchain` / `check-mutations` / `test-check-mutations`）。Make なしでも直接実行できる |
| `tools/` | spec が置き場を指定したスクリプト（`build-web.sh`・`check-deps.sh`・`serve.mjs`・`measure/`）。規約と shellcheck は `scripts/` と同じ |
| `scripts/hooks/` | lefthook から呼ばれる Git hooks の実体 |
| `lefthook.yml` | Git hooks 定義（pre-commit / commit-msg） |
| `.editorconfig`・`.editorconfig-checker.json` | 文字コード・改行・インデントの宣言と、その検査の除外設定（ライセンス本文は除外） |
| `.shellcheckrc` | shellcheck が `source` 先（`scripts/lib.sh`）を追って検査するための設定 |
| `.envrc`・`.env.example` | direnv による `.env` の読み込みと、その雛形 |
| `.mcp.json` | Claude Code のプロジェクト共有 MCP サーバー定義（Playwright MCP。後述の「MCP サーバー」） |
| `CONTRIBUTING.md` | 貢献ガイド（開発の流れ・検査・コミット規約・セキュリティ・ライセンス） |
| `docs/glossary.md` | 用語集（章・Step・IR・語彙など） |
| `docs/architecture.md` | 実装 repo の構成設計（版 1）。crate の境界・依存の向き・公開 API の型の置き場・feature・試験データと雛形の置き場を固定する契約。crate 名は仮称 |
| `docs/testing.md` | 性質テストと変異注入の手順、変異注入の記録の様式。変異の定義は `crates/*/tests/mutations/`、検査は `make check-mutations` |
| `docs/measurement.md` | 測定の結果の様式（版 1）。基点の 4 項目と、基点の違う値を比べない規則。実行は `make measure`（大きさのみは `make measure-wasm-size`） |
| `docs/toolchain.md` | ツールチェーンの固定（版 1）。固定した版と前提の一か所の正。固定ファイルとの一致は `make check-toolchain` が検査する |
| `docs/vocabulary/vocabulary.md` | 語彙の文書（版 2）。語彙 v1〜v1.3 の語ごとの版・名前つき拡張点 EP-1〜EP-12・版と拡張点の対応表・固有の語の一覧を固定する契約。文書と pack crate の表（`crates/pack/src/vocab.rs`）の一致は `crates/pack/tests/vocabulary_terms.rs`、汎用名の検査は `crates/pack/tests/vocabulary_extension_points.rs` |
| `docs/engine-api.md` | 公開 API の契約の文書（版 1）。core の型の一覧・世代の変わり方の表（EVT-6）・宛先の形（EVT-8）・机上の照合の記録。冒頭の契約の版と世代の表は `crates/core/tests/engine_api_doc.rs` が型と照合する |
| `docs/diagnostics.md` | 診断コードの一覧（版 1）。04 の PACK-2〜PACK-28 に現れる 32 件の名前・出る条件・出所・区分を固定する契約。`E-LIMIT-*` は参照のみ。実装との一致の検査は TASK-400 |
| `docs/pack-authoring.md` | 教材の作成者向けの文書。版と保存済みの履歴（本文だけの修正は `content_hash` を変えず、主版の上げ忘れは履歴の版の判定に現れない）。主版を上げる条件と `pack.vocab` の規約は TASK-179 で追加予定。根拠の試験は `crates/core/tests/version_record.rs`・`crates/contract/tests/fixed_histories.rs` |
| `docs/spec/` | 仕様・フェーズ文書（private の submodule。前述の「仕様」） |
| `.gitmodules` | submodule 定義（`docs/spec` を `update = none` で登録） |
| `LICENSE-MIT`・`LICENSE-APACHE` | ライセンス（前述の「ライセンス」） |
| `.github/workflows/` | CI・AI PR レビュー・外部ソース自動追従（後述の「CI」） |

## 前提

### エンジンのビルドの前提

ツールの版は固定済みです。固定した値の正は [docs/toolchain.md](./docs/toolchain.md) の一か所に置き、ここには転記しません（二重定義を避けるため）。固定ファイルとの一致は `make check-toolchain` が検査します。

| 項目 | 固定するファイル | 備考 |
|---|---|---|
| Rust のツールチェーン | `rust-toolchain.toml` | rustup が版・rustfmt・clippy を導入します。`target/` の掃除手順は「セットアップ」に書いています |
| `wasm32` ターゲット | `rust-toolchain.toml` | ブラウザ向けのビルドと、`make lint` の wasm32 向け clippy に使います |
| wasm-bindgen CLI | `Cargo.toml`・`Cargo.lock` の `wasm-bindgen` と同じ版 | ビルドスクリプト（TASK-27）から必須になります。現時点は任意です |
| Node.js | `package.json` の `engines`・`.npmrc` | ブラウザテストの実行に使います |
| Playwright | `package.json`・`package-lock.json` | ブラウザテストに使います |
| Chrome または Chromium | 固定しません | Playwright が同梱する Chromium を使います（理由は docs/toolchain.md） |
| 試験データの置き場所 | 方針は固定済み | 2 つ以上の crate が使うデータは `testdata/`、1 つの crate だけが使う小さな入力はその crate の `tests/` に置きます（[docs/architecture.md](./docs/architecture.md) の第 8 節）。未決なのは、契約のサンプル・golden の配置の細目だけです |

### 必要なツール

| ツール | 用途 | 必須 |
|---|---|---|
| [GNU Make](https://www.gnu.org/software/make/) 3.81+ | `make` 入口 | ✓ |
| [lefthook](https://lefthook.dev/) | Git hooks | ✓ |
| [editorconfig-checker](https://github.com/editorconfig-checker/editorconfig-checker) | `.editorconfig` 準拠チェック | ✓ |
| [ShellCheck](https://www.shellcheck.net/) | シェルスクリプトの lint | ✓ |
| [rustup](https://rustup.rs/) 経由の [Rust](https://www.rust-lang.org/)（cargo・rustfmt・clippy・`wasm32` ターゲット） | `make lint`・`make test`・`make fmt`。`rust-toolchain.toml` を自動で読む | ✓ |
| [jq](https://jqlang.github.io/jq/) | `make check-deps`（crate 間の依存の向きの検査） | ✓ |
| [direnv](https://direnv.net/) | `.env` の自動読み込み | 任意 |
| wasm-bindgen CLI | wasm のビルド（TASK-27 以降） | 任意 |
| [Node.js](https://nodejs.org/) 24 系（`npm`・`npx`） | ブラウザテスト（Playwright）と Playwright MCP の起動（後述の「MCP サーバー」） | 任意 |

macOS（Homebrew）の例:

```bash
brew install lefthook editorconfig-checker shellcheck direnv
```

`make doctor` で導入状況を確認できます（読み取りのみで、何も導入・変更しません）。

## セットアップ

```bash
make doctor   # 必要なツールが揃っているか確認する
rustup toolchain install   # rust-toolchain.toml の版・component・ターゲットを導入する
npm ci        # ブラウザテストを使う場合のみ。package-lock.json の版を導入する
make setup    # Git hooks を有効化し、.env が無ければ .env.example から作成する
direnv allow  # direnv を使う場合のみ。.envrc の読み込みを許可する
```

`make setup` は何度実行しても安全です（既存の `.env` は上書きしません）。

`target/` は放置すると数十 GB になります。14 日以上使っていないビルド成果物は
`cargo sweep --time 14` で削除してください（導入: `cargo install cargo-sweep`）。
worktree はそれぞれ別の `target/` を持つため、不要になった worktree は `git worktree remove` で
`target/` ごと消し、残している worktree は `cargo sweep -r --time 14 .claude/worktrees` でまとめて掃除します。

## ビルドとテスト

現在できる検査は `make check`・`make test-hooks`・`make lint`・`make test`・`make check-deps`・`make test-check-deps`・`make check-dom-reads`・`make test-check-dom-reads` です。内容は次の「コマンド」の表を参照してください。

Cargo ワークスペースは導入済みです。pack crate は `compile <pack-dir>` と `compare` を実装済みで、現在の制約は、出力先の `-o` が未実装（標準出力のみ）・`use` によるモジュール展開をしない・他のコマンド（`validate`・`test`・`vocab`）が未実装であることです。それ以外の crate は骨格です。wasm32 のビルドとブラウザテストの手順は、後続のタスクで書きます。

### ビルド

ネイティブのビルドは `cargo build --locked` です（`default-members` の contract・core・pack だけを対象にします）。web を含めるときは `--workspace` を付けます。wasm32 向けのビルドは後述します。

### native のテスト

- `make test`（`cargo test --locked --workspace`）: web を含む全 crate の native 試験を流します
- `cargo test --locked`: `default-members` だけを流します（web は含みません）
- `make lint`: `cargo fmt --check`・clippy（警告をエラー扱い）・web の wasm32 向け clippy（feature なしと `test-hooks` の 2 通り）を流します。ソースは変更しません
- `make fmt`: rustfmt で整形します（ソースを変更します）
- `make check-deps`: crate 間の依存の向きを `cargo metadata` で検査します。`make test-check-deps` は、その検査自体の回帰テストです
- `make check-mutations`: 変異注入の検査です。作業ツリーは書き換えず、一時コピーに誤りを 1 つ入れて試験が失敗することを確かめます（手順と記録の様式は [docs/testing.md](./docs/testing.md)）。遅いため `make check` やフックには含めません。`make test-check-mutations` は、その検査自体の回帰テストです
- `make check-dom-reads`: web crate のソースが DOM の値を読む API（`.value()`・`get_attribute`・`text_content` 等）を呼ばないことを `tools/check-deps.sh` で検査します。例外（`glue.rs` の `field.value()`）は件数を固定し、増減すると失敗します。crate 間の依存の向きを見る `make check-deps` とは別の検査です。`make test-check-dom-reads` は、その検査自体の回帰テストです
- `make check-testdata`: repo の外（兄弟の `03-poc/`・`prototype/` など）への相対参照が 0 件であることを検査します（`make check` にも含まれます）。`make test-check-testdata` は、その検査自体の回帰テストです

### wasm32 のビルド

（後続のタスクで記入します）

### ブラウザテスト

共通ヘルパ（`tools/browser/harness.mjs`）は、ブラウザの起動・通信の計数・CSP 違反の計数・ページ内エラーの収集を担います。安全性の試験（TASK-143）向けに、記録用の関数（`__poc7_hit`）の呼び出し・ダイアログ・ポップアップ・タブの異常終了の計数、画面の走査（危険な要素・`on*` 属性・`style` 属性・http(s) 以外の URL・外部 http(s)）、CSP の meta だけを除いた一時の写しでの配信（CSP なし）も担います。計数のキー名は `safetyCounts()` が返す固定のもので、後続の安全性の試験がそのまま使います。

```bash
npm ci                              # package-lock.json の版を導入する
npx playwright install chromium     # 同梱の Chromium を導入する
make test-browser                   # web（配信設定と試験の設定）を一時ディレクトリへビルドし、試験を流す
```

既定は Playwright 同梱の Chromium です。Google Chrome の原本で流すときは `FANDHE_BROWSER_CHANNEL=chrome make test-browser` とします（値は `chromium` か `chrome` だけです。Chrome が無い場合は Chromium へ切り替えず失敗します）。

## コマンド

| コマンド | 直接実行 | 内容 |
|---|---|---|
| `make help` | `scripts/help.sh` | 操作の一覧を表示する |
| `make doctor` | `scripts/doctor.sh` | 開発環境を診断する |
| `make setup` | `scripts/setup.sh` | Git hooks 有効化と `.env` 雛形の配置 |
| `make check` | `scripts/check.sh` | editorconfig-checker + shellcheck + testdata の参照検査（ソースは変更しない） |
| `make test-hooks` | `scripts/hooks/test-hooks.sh` | フックスクリプト自体の回帰テスト（一時 git リポジトリで検出・許可ケースを検証。bash + git のみ） |
| `make lint` | `scripts/lint.sh` | Rust の fmt check + clippy + web の wasm32 向け clippy（ソースは変更しない） |
| `make fmt` | `cargo fmt --all` | Rust のソースを rustfmt で整形する（ソースを変更する） |
| `make test` | `scripts/test.sh` | Rust の native 試験（web を含む全 crate） |
| `make check-deps` | `scripts/check-crate-deps.sh` | crate 間の依存の向きを `cargo metadata` で検査する |
| `make test-check-deps` | `scripts/test-check-crate-deps.sh` | 依存の向きの検査自体の回帰テスト |
| `make check-testdata` | `scripts/check-testdata-refs.sh` | repo の外への相対参照が 0 件であることの検査 |
| `make test-check-testdata` | `scripts/test-check-testdata-refs.sh` | 参照の検査自体の回帰テスト |
| `make check-dom-reads` | `tools/check-deps.sh` | web crate が DOM の値を読まないこと（例外は件数固定）の検査 |
| `make test-check-dom-reads` | `scripts/test-check-dom-reads.sh` | DOM 読み取りの検査自体の回帰テスト |
| `make check-toolchain` | `scripts/check-toolchain.sh` | 固定ファイルの版と `docs/toolchain.md` の一致を検査する |
| `make test-check-toolchain` | `scripts/test-check-toolchain.sh` | ツールチェーンの一致の検査自体の回帰テスト |
| `make check-workflow-secrets` | `scripts/check-workflow-secrets.sh` | `ci.yml` が secret を参照していないことを検査する |
| `make test-check-workflow-secrets` | `scripts/test-check-workflow-secrets.sh` | secret 参照の検査自体の回帰テスト |
| `make serve` | `node tools/serve.mjs` | `dist/web` をサブパス（既定 `/subpath/`、`--base` で任意の名前）の下で gzip つきで配る。例: `make build-web` の後に `node tools/serve.mjs --base /my-site/` を実行し、`http://127.0.0.1:8765/my-site/` を開く |
| `make test-serve` | `scripts/test-serve.sh` | 配信サーバの試験（Node の `node:test`） |
| `make test-browser` | `scripts/test-browser.sh` | 共通ヘルパ（`tools/browser/`）の試験・安全性の計数の対照と、配信設定の web をサブパス配信した最小の画面の試験・対照を挿入した自己確認（SAFE-8。CSP あり・なし）、試験の設定（`TEST_HOOKS=1`）の web で keydown の属性が届くことの試験、配信設定の web で Enter の送信が送信ボタンの click と同じ記録になることの試験（Playwright）。CI には未組み込みです |
| `make test-keydown-control` | `scripts/test-keydown-control.sh` | `action-keydown` を外した写しで keydown の試験が「キーが届かない」理由で失敗することの対照。依存を冷えた状態からビルドするため遅く、CI・フックには入れません |
| `make build-web` | `tools/build-web.sh` | web を wasm にビルドして `dist/web` に配る一式を作る（`TEST_HOOKS=1` で試験の設定、`IR_FILE` で IR を同梱） |
| `make test-build-web` | `scripts/test-build-web.sh` | `build-web` の受入基準を実ビルドで検査する |
| `make check-delivery` | `tools/check-delivery.sh` | 配信物（`dist/web`。先に `make build-web`）に試験用の口が無いことの検査。CI には未組み込み |
| `make test-check-delivery` | `scripts/test-check-delivery.sh` | 配信検査自体の回帰テスト（実ビルドを行うため遅い） |
| `make check-mutations` | `scripts/check-mutations.sh` | 変異注入の検査（誤りを入れると対応する試験が失敗することの確認。遅く、CI・フックには入れない） |
| `make test-check-mutations` | `scripts/test-check-mutations.sh` | 変異注入の検査自体の回帰テスト |
| `make measure-wasm-size` | `tools/measure/wasm-size.sh` | WASM の `gzip -9` の大きさを基点つきで測り、JSON を標準出力へ出す（様式は [docs/measurement.md](./docs/measurement.md)） |
| `make test-measure-wasm-size` | `scripts/test-measure-wasm-size.sh` | 結果の様式の検証と実ビルドの回帰テスト（遅く、CI・フックには入れない） |
| `make measure` | `tools/measure/all.sh` | 3 つの測定（WASM の `gzip -9`・初回表示・再描画）を基点つきで流し、束の JSON を標準出力へ出す。要 `npm ci` と `npx playwright install chromium`（様式は [docs/measurement.md](./docs/measurement.md)） |
| `make test-measure` | `scripts/test-measure.sh` | 束の検証・fail-closed・一括実行の回帰テスト（遅く、ブラウザを要し、CI・フックには入れない） |

## Git hooks

`make setup` 後、以下が自動実行されます。

- **pre-commit**
  - 簡易シークレット検知（`.env` 系ファイルの追加、トークン形式・秘密鍵・認証情報入り URL・ハードコード値）。
    保守的なヒューリスティックであり、網羅的なスキャナの代替ではありません
  - staged ファイルの editorconfig-checker
  - staged の `*.sh` に対する shellcheck
- **commit-msg**: [Conventional Commits](https://www.conventionalcommits.org/ja/) 形式の検証
  （`<type>[(<scope>)][!]: <要約>`、type は `feat` `fix` `docs` `style` `refactor` `perf` `test` `build` `ci` `chore` `revert`）

hooks に引っかかった場合は原因を修正してから再コミットしてください。`--no-verify` によるバイパスは行いません。

開発速度を落とさないため、hooks には staged ファイル限定で一瞬で終わる検査と、コミット単位でしか意味がない検査
（シークレット検知・コミットメッセージ）だけを置きます。テスト・ビルド・全体 lint など重い処理や CI と重複する検査は
CI（`.github/workflows/ci.yml`）で行います。時間予算（pre-commit 合計 1 秒未満）と判断手順は `.claude/rules/git-hooks.md` に定義しています。

フックスクリプト（`scripts/hooks/*.sh`）自体のテストもフックには置かず、CI の `test-hooks`・`test-hooks-macos` ジョブ（`make test-hooks`）で
Linux・macOS（標準の bash 3.2 を含む）について実行します。`scripts/hooks/` を変更したらローカルでも `make test-hooks` を実行してください。

## CI

| ワークフロー | 内容 |
|---|---|
| `ci.yml` | `check`: ローカル・hooks と同じ `make check` を実行する（editorconfig-checker / shellcheck はバージョン固定 + SHA256 検証で導入）。あわせて `make check-toolchain` と `make check-workflow-secrets`（と各回帰テスト）も流す。`rust-lint`: `make lint`・`make check-deps`・`make test-check-deps`・`make check-dom-reads`・`make test-check-dom-reads`（`rust-toolchain.toml` の版を rustup で導入し、導入された版を `scripts/ci-rust-toolchain.sh` で照合する）。`rust-test`: `make test`（`cargo test --locked --workspace`）がネイティブのビルドと試験を兼ねる。2 つの Rust ジョブはキャッシュを使わない（効果を実測してから足す）。`changes`: 変更ファイルから、フックの回帰テストを macOS でも実行するかを決める（`scripts/ci-changes.sh`。PR でフック関連の変更が無ければ ubuntu のみ、push・判定不能時は macOS も含める）。`test-hooks`: フックスクリプト自体の回帰テスト（`make test-hooks`。ubuntu-latest で常に実行する）。`test-hooks-macos`: 同じテストの macOS 版（macos-latest。標準の bash 3.2 でも実行する）。`changes` の結果で不要な場合は macOS ランナーを起動せず、チェックは skipped として報告される。`pr-title`: PR タイトルを commit-msg フックと同じスクリプトで検証する（squash merge でコミット件名になるため）。`ci-complete`: 全ジョブ結果の集約（`test-hooks-macos` の skipped は、`changes` の結果と一致する場合だけ許容する） |
| `deploy.yml` | 配信の骨格（TASK-33）。`workflow_dispatch` だけで動き、`permissions: {}`。中身は未実装で、実行すると失敗して止まる（TASK-121 で作る）。secret を参照するビルド・試験を `ci.yml` に置かないため、配信は PR の起点を持たない別ワークフローに分けている |
| `ai-review.yml` | Fandhe-AI/actions の ai-review（codex）による PR 自動レビュー。Actions variable `CODEX_HOME_DIR` が未設定の間は skip される |
| `update-external.yml` | エージェントスキル（`skills-lock.json`）と submodule（`.gitmodules`）の日次自動追従 PR。secrets は org の `SUBMODULE_PAT` を使う（`SKILLS_PAT` 未登録時は共通側が `SUBMODULE_PAT` へフォールバック）。作成する PR には `dependencies` / `automated` ラベルが付く |

- `main` は ruleset `main-protection` で保護しています（2026-10-11 適用）。直接 push はできず、変更はすべて PR を経由します。
  マージ方式は squash のみで、必須チェックがすべて成功し、レビュースレッドがすべて解決済みであることが条件です（承認数は 0）
- ruleset の必須チェック（required status checks）には、PR の HEAD に報告される全チェック（`ci.yml` の全ジョブ・ai-review の `codex / *`・Cursor Bugbot）を登録しています。
  `implement-issue-tree` の自動マージは、必須に無いチェックが HEAD に 1 件でもある場合と、必須のチェックが HEAD に報告されない場合に辞退するためです。
  方針の詳細は `.claude/rules/github-actions.md` の「必須チェック」、ruleset・マージ設定の導入は `setup-repo-guards` スキルの手順に従います
- 2026-10-14 に、`rust lint (fmt + clippy + check-deps)` と `rust build + test (--workspace)` の 2 件（発行元は github-actions）を必須チェックに加え、合計 12 件にしました。`deploy.yml` の `deploy` は PR に報告されないため、必須に入れません。詳細は `.claude/rules/github-actions.md` の「必須チェック」です
- 条件付きのジョブは、ジョブ単位の `if:` で制御し、不要な場合もチェック自体は skipped として報告させます（例: `test-hooks-macos`）
- CI のジョブを追加・改名・削除したら、`ci-complete` の `needs` と ruleset の必須チェックを同じタイミングで更新します

## MCP サーバー

`.mcp.json` に、Claude Code のプロジェクト共有 MCP サーバーを定義しています。

| サーバー | 内容 |
|---|---|
| `playwright` | [Playwright MCP](https://github.com/microsoft/playwright-mcp)（`@playwright/mcp`、版は `.mcp.json` で固定）。エージェントがブラウザを操作して画面を確認する。`npx` で起動するため Node.js が必要 |

- ブラウザは Playwright の chromium を使います。未導入の場合は `npx -y playwright install chromium` などで導入します
- ブラウザのプロファイルは `_/playwright/profile`、スクリーンショット等の出力は `_/playwright/output` に置かれます（`_/` は git 管理外）
- 初回は Claude Code がプロジェクトの MCP サーバーを使うかどうかの承認を求めます

サーバーを追加する場合、API キー等は値を直接書かず `"${EXAMPLE_API_KEY:-}"` のように環境変数参照とし、実値は `.env`（direnv 経由）に置く。

## 環境変数

- 雛形は `.env.example`（キー名と空値・ダミー値のみ）、実値は `.env`（git 管理外）に置きます
- direnv を導入している場合、`.envrc` が `.env` をシェルへ読み込みます
- 個人用の追加設定は `.envrc.local`（git 管理外）に置けます

## 派生リポジトリでの拡張

1. **言語固有のコマンド**: `scripts/` に処理（例: `scripts/verify.sh`）を追加し、`Makefile` に 1 行で呼ぶターゲットと `## ` コメントを足し、`scripts/help.sh` の一覧も更新する（中身の無いターゲット `@true` 等は置かない）
2. **`make check` の拡張**: `scripts/check.sh` に fmt チェック・lint 等を追記する（自動修正は別ターゲットに分ける）
3. **hooks の追加**: `.claude/rules/git-hooks.md` の時間予算と判断手順に従う。staged 限定で 1 秒未満の検査のみ `lefthook.yml` に job を追加し（実体が長くなる場合は `scripts/hooks/` へ切り出す）、テスト・ビルド等は `ci.yml` にジョブを追加して `ci-complete` の `needs` と ruleset の必須チェックにも加える
4. **EditorConfig**: 言語ごとのセクション（例: `[*.rs] indent_size = 4`）を `.editorconfig` に追加する
5. **`.gitignore`**: ビルド成果物（`/target`・`/dist`・`node_modules/` 等）を追加する
6. **Rust の導入**: `.claude/rules/rust-build-profile.md` に従い、ルートの `Cargo.toml` に依存クレートの debuginfo を行番号のみにする dev プロファイルを入れ、README のセットアップ節に `cargo sweep` による `target/` の掃除手順を書く。ビルド設定（`[profile]`・`RUSTFLAGS`・`CARGO_PROFILE_*` 等）を変えるときは CI キャッシュのキーへの影響を確認する（`.claude/rules/github-actions.md` の「キャッシュ方針」）

## 貢献

貢献の流れは [CONTRIBUTING.md](./CONTRIBUTING.md)、用語は [用語集](./docs/glossary.md) を参照してください。
