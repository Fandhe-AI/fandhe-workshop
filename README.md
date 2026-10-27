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
2026-10-15 時点では、Cargo ワークスペースの 4 crate のうち pack crate に、教材パックの読み込み層と `fandhe-pack compile <pack-dir>`・`compare <pack-dir-a> <pack-dir-b>`・`metrics <pack-dir>` を実装済みです（TASK-39）。エンジン（core）の実行側と web はまだ骨格です。このリポジトリには開発基盤・構成設計（[docs/architecture.md](./docs/architecture.md)）・ワークスペースがあります。

## 仮称の一覧

次の名前はすべて仮称です。確定した名前に変えるときは、この表と [docs/architecture.md](./docs/architecture.md) を同じ変更で直します。

| 対象 | 仮称 | 状態（2026-10-12 時点） |
|---|---|---|
| エンジンのリポジトリ | `fandhe-workshop` | 本リポジトリ。public で作成済み。製品名は未確定 |
| 教材のリポジトリ | `fandhe-tutorials` | 未確認（GitHub から参照できなかった。作成済みかどうかは不明） |
| 外部向けの雛形 | `template-workshop` | 初回は本リポジトリ内のディレクトリに置く方針（[docs/architecture.md](./docs/architecture.md) の第 8 節）。独立リポジトリにする時期は未定 |
| crate（4 つ） | `fandhe-workshop-contract`・`fandhe-workshop-core`・`fandhe-workshop-pack`・`fandhe-workshop-web` | `crates/` 配下。pack に読み込み層と `compile`・`compare`・`metrics`・`vocab` を実装済みで、他は骨格中心（2026-10-15 時点）。役割は [docs/architecture.md](./docs/architecture.md) の第 3 節 |
| 作成者向け CLI | `fandhe-pack` | pack crate に置く。`compile <pack-dir> [--chapters <id>,...]`（IR の JSON を標準出力へ書く。`--chapters` で章の範囲を絞る）・`compare <pack-dir-a> <pack-dir-b>`（IR のバイト一致を判定する）・`metrics <pack-dir>`（ファイルごとの行数・実効行数・バイト数・入れ子の深さを CSV で出す）・`vocab <pack-dir>`（拡張点を使った Step の数え上げ）を実装済み。外部仕様は `docs/cli.md` |

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
| `scripts/` | 処理の実体（`help` / `doctor` / `setup` / `check` / `lint` / `test` / `check-crate-deps` / `test-check-crate-deps` / `check-testdata-refs` / `test-check-testdata-refs` / `test-check-dom-reads` / `test-check-delivery` / `check-toolchain` / `test-check-toolchain` / `check-workflow-secrets` / `test-check-workflow-secrets` / `check-fork-ci` / `test-check-fork-ci` / `ci-rust-toolchain` / `check-mutations` / `test-check-mutations` / `test-check-generic-words` / `test-inventory-deps`）。Make なしでも直接実行できる |
| `tools/` | spec が置き場を指定したスクリプト（`build-web.sh`・`check-deps.sh`・`serve.mjs`・`inventory-deps.mjs`・`upstream-impact.mjs`・`build-l2.mjs`・`l2/`・`measure/`）。規約と shellcheck は `scripts/` と同じ |
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
| `docs/dependency-inventory.md` | 依存の棚卸しの手順と日付つきの結果の記録（BUILD-25・D-58）。実行は `make inventory-deps` |
| `docs/measurement.md` | 測定の結果の様式（版 1）。基点の 4 項目と、基点の違う値を比べない規則。実行は `make measure`（大きさのみは `make measure-wasm-size`） |
| `docs/toolchain.md` | ツールチェーンの固定（版 1）。固定した版と前提の一か所の正。固定ファイルとの一致は `make check-toolchain` が検査する |
| `docs/vocabulary/vocabulary.md` | 語彙の文書（版 7）。語彙 v1〜v1.7 の語ごとの版・名前つき拡張点 EP-1〜EP-13・版と拡張点の対応表・固有の語の一覧を固定する契約。文書と pack crate の表（`crates/pack/src/vocab.rs`）の一致は `crates/pack/tests/vocabulary_terms.rs`、汎用名の検査は `crates/pack/tests/vocabulary_extension_points.rs` |
| `docs/upstream-bump-procedure.md` | 参照元の版を上げる手順書（版 1）。洗い出し・教材テスト・公開前の人の確認と記録・AI の生成物を無検証で公開しないことを固定する（TASK-182 2/2） |
| `docs/extension-point-procedure.md` | 拡張点の追加の手続きの文書（版 1）。汎用であることの説明・語彙の文書の固定・診断コードの追加・公開の一覧への追記・既存の教材パックが同じ IR になることの確認を固定する。実行時の登録の仕組みは作らない（D-53）。形の検査は `crates/pack/tests/extension_point_procedure_doc.rs` |
| `docs/engine-api.md` | 公開 API の契約の文書（版 1）。core の型の一覧・世代の変わり方の表（EVT-6）・宛先の形（EVT-8）・机上の照合の記録。冒頭の契約の版と世代の表は `crates/core/tests/engine_api_doc.rs` が型と照合する |
| `docs/cli-guide.md` | 作成者向けのコマンドの使い方と出力の読み方（版 1）。出力の転記は `crates/pack/tests/examples_pack.rs` が実行結果と照合する（TASK-412） |
| `docs/examples/` | 最小の教材パックの例 2 つ（コマンドで進む例・選択肢と画面操作だけで進む例）と `MANIFEST.sha256`。`validate` 0 件・`test` 全件合格を `examples_pack.rs` が CI で検査する（TASK-412。AUTHOR-12） |
| `docs/diagnostics.md` | 診断コードの一覧（版 19。作成者向けの早見表は 8 節。一覧の正）。04 の PACK-2〜PACK-31 に現れるコードと実装で足した `E-REF-ELEMENT`・`E-VOCAB-MISMATCH`・`E-SECRET-SHAPE`・`E-FICTIONAL-MARK` の 36 件の名前・出る条件・出所・区分を固定する契約。`E-LIMIT-*` は参照のみ。実装との一致は `crates/pack/tests/diagnostics_doc.rs` が CI で検査する（TASK-400） |
| `docs/cli.md` | CLI の外部仕様（版 11）。`fandhe-pack` のバイナリ名・コマンド 6 つと引数の形・使い方・終了コード 0/1/2・診断 1 行・項目の書式・最終行（`OK:`・`NG:`）・診断の出し先・`compile` の出力（`-o`・末尾・バイト単位の決定性）・`metrics` の出力・`vocab` の出力（表・集計行。TASK-399・TASK-407・TASK-402・TASK-408・TASK-409）。文書との一致は `crates/pack/tests/cli_contract_doc.rs`、実行ファイルの挙動は `crates/pack/tests/cli_contract.rs` が検査する |
| `docs/history-format.md` | 履歴の形式の文書（版 1）。包みの 7 欄・4 軸の版・追記だけで増える列（EXT-5・D-53）。冒頭の版と欄の並びは `crates/contract/tests/history_format_doc.rs` が型と照合する |
| `docs/pack-authoring.md` | 教材の作成者向けの文書。版と保存済みの履歴（本文だけの修正は `content_hash` を変えず、主版の上げ忘れは履歴の版の判定に現れない）。主版を上げる条件と `pack.vocab` の書き方の規約（検査は採らず作成者の規約。D-52）。根拠の試験は `crates/core/tests/version_record.rs`・`crates/contract/tests/fixed_histories.rs` |
| `docs/known-gaps.md` | 既知の不足（忠実さの限界）。受講者が選んだ値を差し込めないこと（H-3）と属性を書き換える回避に加え、PoC の記録の H-6・H-7・H-8・AUTHOR-13 の今の状態を記録する。確かめていない ID は明記する。根拠の試験は `crates/pack/tests/chosen_value_gap.rs` ほか |
| `docs/schema-format-b.md` | 構造定義（形式 B）の文書（版 1）。`workshop.yaml`・`chapter.yaml`・`module.yaml`・`tests/playthrough.yaml` の欄・型・必須・既定と、`{{...}}` を引用符で囲む注意書き（AUTHOR-13。試験は `crates/pack/tests/format_b.rs` の `author13_`）。末尾に机上の照合の記録 |
| `docs/vocabulary/README.md` | 語彙の資料の索引（版 1）。EP-1〜EP-13 の書き方の資料と作成者への注意の道案内。語と版の正は `docs/vocabulary/vocabulary.md` |
| `docs/operation-profile.md` | 操作プロファイル（接頭辞・入力欄）と外観で選べるもの・選べないもの、外観 4 値の定義（版 1）。外観ごとの見た目が画面に未反映であることも記録する |
| `docs/views.md` | ビュー 7 種・要素の属性・`pack.left_pane` の効き方・「指す」入力の宛先・表示件数の上限（版 1）。製品の画面への結線の進み具合を記録する |
| `docs/spec/` | 仕様・フェーズ文書（private の submodule。前述の「仕様」） |
| `.gitmodules` | submodule 定義（`docs/spec` を `update = none` で登録） |
| `LICENSE-MIT`・`LICENSE-APACHE` | ライセンス（前述の「ライセンス」） |
| `.github/workflows/` | CI・AI PR レビュー・外部ソース自動追従（後述の「CI」） |

## 前提

### エンジンのビルドの前提

固定した版の正は [docs/toolchain.md](./docs/toolchain.md) の第 2 節の表です。次の表はその写しで、第三者がこの節だけを読んで同じ版を揃えられるようにしています。写しとの一致は `make check-toolchain` が検査します（CI の `check` ジョブでも流れます）。

| キー | 値 | 固定するファイル | 備考 |
|---|---|---|---|
| `rust` | `1.98.1` | `rust-toolchain.toml` の `channel` | rustfmt・clippy も導入されます。`target/` の掃除手順は「セットアップ」に書いています |
| `rust-target` | `wasm32-unknown-unknown` | `rust-toolchain.toml` の `targets` | ブラウザ向けのビルドと、`make lint` の wasm32 向け clippy に使います |
| `wasm-bindgen` | `0.2.128` | `Cargo.toml`・`Cargo.lock` | CLI は `Cargo.lock` の版と同じにします |
| `node` | `>=24 <25` | `package.json` の `engines.node`・`.npmrc` の `engine-strict` | 24 系を使います |
| `playwright` | `1.63.0` | `package.json`・`package-lock.json` | ブラウザテストに使います |

固定しない前提は次のとおりです。

| 項目 | 内容 |
|---|---|
| Chrome または Chromium | 固定しません。Playwright 1.63.0 が同梱する Chromium を使います。Google Chrome の原本で流すときは `FANDHE_BROWSER_CHANNEL=chrome` を付けます（理由は docs/toolchain.md の第 3 節、扱いは「Chrome の原本と Chromium の扱い」） |
| 試験データの置き場所 | 2 つ以上の crate が使うデータは `testdata/`（出所は `testdata/README.md`）、1 つの crate だけが使う小さな入力はその crate の `tests/` に置きます（[docs/architecture.md](./docs/architecture.md) の第 8 節）。repo の外を参照しないことは `make check-testdata` が検査します |
| Python | 不要です。試験用の配信は Node の `tools/serve.mjs` が担います |
| wasm-opt | 任意です（`SKIP_WASM_OPT=1` で省けます） |

### 必要なツール

| ツール | 用途 | 必須 |
|---|---|---|
| [GNU Make](https://www.gnu.org/software/make/) 3.81+ | `make` 入口 | ✓ |
| [lefthook](https://lefthook.dev/) | Git hooks | ✓ |
| [editorconfig-checker](https://github.com/editorconfig-checker/editorconfig-checker) | `.editorconfig` 準拠チェック | ✓ |
| [ShellCheck](https://www.shellcheck.net/) | シェルスクリプトの lint | ✓ |
| [rustup](https://rustup.rs/) 経由の [Rust](https://www.rust-lang.org/)（cargo・rustfmt・clippy・`wasm32` ターゲット） | `make lint`・`make test`・`make fmt`。`rust-toolchain.toml` を自動で読む | ✓ |
| [jq](https://jqlang.github.io/jq/) | `make check-deps`（crate 間の依存の向きの検査）・`tools/measure/*.sh`（`make measure`）。1.7 以上（macOS 同梱で可） | ✓ |
| [direnv](https://direnv.net/) | `.env` の自動読み込み | 任意 |
| wasm-bindgen CLI（`Cargo.lock` と同じ版） | `make build-web`・`make test-browser`。native のビルド・試験だけなら不要 | 任意（wasm のビルドには必須） |
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
cargo install wasm-bindgen-cli --version 0.2.128 --locked   # wasm のビルドとブラウザテストを使う場合のみ
npm ci        # ブラウザテストを使う場合のみ。package-lock.json の版を導入する
npx playwright install chromium   # ブラウザテストを使う場合のみ。同梱の Chromium を導入する
make setup    # Git hooks を有効化し、.env が無ければ .env.example から作成する
direnv allow  # direnv を使う場合のみ。.envrc の読み込みを許可する
```

`make setup` は何度実行しても安全です（既存の `.env` は上書きしません）。

`target/` は放置すると数十 GB になります。14 日以上使っていないビルド成果物は
`cargo sweep --time 14` で削除してください（導入: `cargo install cargo-sweep`）。
worktree はそれぞれ別の `target/` を持つため、不要になった worktree は `git worktree remove` で
`target/` ごと消し、残している worktree は `cargo sweep -r --time 14 .claude/worktrees` でまとめて掃除します。

## ビルドとテスト

流す順は、native のビルド、native のテスト、wasm32 のビルド、ブラウザテストです。ただし `make test-browser` は前提のバイナリ（`fandhe-pack`）と web を自分でビルドするため、先に `make test` を流す必要はありません。現在できる検査は `make check`・`make test-hooks`・`make lint`・`make test`・`make check-deps`・`make test-check-deps`・`make check-dom-reads`・`make test-check-dom-reads` です。内容は次の「コマンド」の表を参照してください。

Cargo ワークスペースは導入済みです。pack crate は `compile <pack-dir>`・`compare`・`metrics`・`vocab`・`validate` を実装済みで、現在の制約は、`use` によるモジュール展開をしない・`test` が最終行と R-22 などの集計をまだ出さない（枠・終了コード・PASS と FAIL の行・集計の 3 行は実装済み）ことです。それ以外の crate は骨格です。wasm32 のビルドとブラウザテストの手順は、後続のタスクで書きます。

### ビルド

ネイティブのビルドは `cargo build --locked` です（`default-members` の contract・core・pack だけを対象にします）。web を含めるときは `--workspace` を付けます。wasm32 向けのビルドは後述します。

### native のテスト

- `make test`（`cargo test --locked --workspace`）: web を含む全 crate の native 試験を流します。CI の `rust-test` ジョブも同じ `--workspace` で流します。web の native 試験は PoC-11 当時は 33 件で、2026-10-19 時点（`b27ca0b`）は 795 件です（`cargo test --locked -p fandhe-workshop-web -- --list` の件数）
- `cargo test --locked`: `default-members` だけを流します（web は含みません）
- `make lint`: `cargo fmt --check`・clippy（警告をエラー扱い）・web の wasm32 向け clippy（feature なしと `test-hooks` の 2 通り）を流します。ソースは変更しません
- `make fmt`: rustfmt で整形します（ソースを変更します）
- `make check-deps`: crate 間の依存の向きを `cargo metadata` で検査します。`make test-check-deps` は、その検査自体の回帰テストです
- `make check-mutations`: 変異注入の検査です。作業ツリーは書き換えず、一時コピーに誤りを 1 つ入れて試験が失敗することを確かめます（手順と記録の様式は [docs/testing.md](./docs/testing.md)）。遅いため `make check` やフックには含めません。`make test-check-mutations` は、その検査自体の回帰テストです
- `make check-dom-reads`: web crate のソースが DOM の値を読む API（`.value()`・`get_attribute`・`text_content` 等）を呼ばないことを `tools/check-deps.sh` で検査します。例外（`glue.rs` の `field.value()`）は件数を固定し、増減すると失敗します。crate 間の依存の向きを見る `make check-deps` とは別の検査です。`make test-check-dom-reads` は、その検査自体の回帰テストです
- `make check-testdata`: repo の外（兄弟の `03-poc/`・`prototype/` など）への相対参照が 0 件であることを検査します（`make check` にも含まれます）。`make test-check-testdata` は、その検査自体の回帰テストです

### wasm32 のビルド

```bash
make build-web                  # 配信の設定（既定は release）。出力は dist/web
TEST_HOOKS=1 make build-web     # 試験の設定。出力は dist/web-test
make serve                      # dist/web をサブパスの下で配る
make check-delivery             # 配信物に試験用の口が無いことを確かめる
```

wasm-bindgen CLI の版が `Cargo.lock` と違うと、ビルドは止まります。詳しい環境変数は `tools/build-web.sh` の冒頭のコメントを参照してください。

### ブラウザテスト

共通ヘルパ（`tools/browser/harness.mjs`）は、ブラウザの起動・通信の計数・CSP 違反の計数・ページ内エラーの収集を担います。安全性の試験（TASK-143）向けに、記録用の関数（`__poc7_hit`）の呼び出し・ダイアログ・ポップアップ・タブの異常終了の計数、画面の走査（危険な要素・`on*` 属性・`style` 属性・http(s) 以外の URL・外部 http(s)）、CSP の meta だけを除いた一時の写しでの配信（CSP なし）も担います。計数のキー名は `safetyCounts()` が返す固定のもので、後続の安全性の試験がそのまま使います。変換中の入力は、CDP による IME の模擬のヘルパ（`tools/browser/ime.mjs`。シミュレーターであり実 IME ではありません）で作ります。

```bash
npm ci                              # package-lock.json の版を導入する
npx playwright install chromium     # 同梱の Chromium を導入する
make test-browser                   # fandhe-pack（debug）と web（配信設定と試験の設定）をビルドし、試験を流す
```

前提のバイナリ `fandhe-pack` の debug ビルドは `make test-browser` が先に行い（`scripts/build-pack-bin.sh`）、位置を環境変数 `FANDHE_PACK_BIN` で試験へ渡します。位置は cargo の出力から解決するため、`CARGO_TARGET_DIR` を変えても止まりません。CI は既定でない target で流し、毎回これを確かめます。`node --test` で試験を直接流すときは、`FANDHE_PACK_BIN`（と `FANDHE_WEB_DIR`・`FANDHE_WEB_TEST_DIR`）を自分で設定してください。未設定は `FANDHE_PACK_BIN` を名指すエラーで失敗します。`npm ci`・`npx playwright install chromium`・wasm-bindgen CLI は、従来どおり自分で用意する前提です。手元で位置の解決だけを確かめるには `make test-build-pack-bin` を使います（遅いため CI には入れていません）。

CI（`ci.yml` の `browser-test` ジョブ、チェック名 `browser test (Chromium)`）は、同じ `make test-browser` を同梱の Chromium（`FANDHE_BROWSER_CHANNEL` なし）で流します。pass／fail 型の試験（`tools/browser/*.test.mjs` の全件。PoC-1・2・6 と語彙の結線に当たる試験を含む）と、最小の画面の試験（`smoke-hello`。web は先に一度だけビルドし、試験自体はビルドし直さない）、サブパス配信の構成要素と相対パスの参照の試験（`delivery-subpath`。DLV-1・DLV-2。TASK-106 1/3）、章への直接リンク 10 件のサブパス配信の試験（`delivery-direct-links`。DLV-1。TASK-106 2/3）、名前の違うサブパス 2 か所とルート配信の試験（`delivery-bases`。DLV-2。TASK-106 3/3）・2 列の骨格の試験（`shell-columns`。SHELL-1。TASK-517 1/2）・幅 960px 未満のタブの切り替えの試験（`shell-tabs`。SHELL-1・A11Y-5。TASK-517 2/2）・4 つの幅で CH-1 の 6 Step を通す試験（`shell-widths`。SHELL-1。TASK-517 2/2）・CH-0 から CH-3 の 20 Step を 1280px と 360px で最後まで通す通しの試験の骨格（`shell-journey`。SHELL-1・SHELL-7・SHELL-8・SHELL-10。TASK-548 1/2。途中にチェックポイント・再読み込み・書き出し→読み込み・保存の失敗を挟む。TASK-548 2/2）、信頼できない入力への安全性の観察の記録（`safety-observe`。観察値は assert せず CI のログに残し、完走して終了コード 0 になることが関門）が対象です。1 本でも失敗すればジョブが失敗し、`ci-complete` が CI 全体を失敗にします。ジョブの手順は次のとおりで、手元でも同じ順で再現できます。

1. 固定した Rust と、SHA256 を検証した wasm-bindgen CLI（`docs/toolchain.md`）を導入する
2. Node.js 24 系を導入する（版は `package.json` の `engines`）
3. `npm ci`
4. `npx playwright install --with-deps chromium`（CI の Ubuntu では OS の依存も入れる）
5. `make test-browser`

失敗したときは、ジョブのログの node:test の出力で、失敗した試験ファイルと検査名を確認します。

既定は Playwright 同梱の Chromium です。Google Chrome の原本で流すときは `FANDHE_BROWSER_CHANNEL=chrome make test-browser` とします（値は `chromium` か `chrome` だけです。Chrome が無い場合は Chromium へ切り替えず失敗します）。

### Chrome の原本と Chromium の扱い（BUILD-20）

ブラウザテストの原本は、Google Chrome の最新の安定版で流します（D-9）。同梱の Chromium は写しです。どちらで流したかは、node:test の診断の行 `browser: channel=... version=...`（`harness.test.mjs`）で分かります。不具合を報告するときは、この channel と版を添えてください。

| 環境 | 流し方 | 結果の位置づけ |
|---|---|---|
| x86_64 の Linux（Chrome の安定版を導入済み） | `FANDHE_BROWSER_CHANNEL=chrome make test-browser` | 原本。2026-10-19 に全件合格を確かめた（記録は `docs/testing.md` 5.128） |
| Linux Arm64 など、Chrome の安定版が無い環境 | 変数を付けずに `make test-browser`（同梱の Chromium） | 参考の確認。`npx playwright install chrome` は `not supported on Linux Arm64` で失敗する |
| macOS など上記以外 | 未確認 | 確かめていないため、位置づけを決めていません |
| CI（`browser test (Chromium)`） | 同梱の Chromium | 関門。原本は CI では流していません（関門にするかは人間の判断待ちです） |

- Chrome の導入は `npx playwright install chrome` です（Ubuntu・Debian の x86_64 だけ。root が要り、既存の `google-chrome` を最新の安定版に入れ替えます）。Google の公式の導入手順でもかまいません
- `chrome` を指定して Chrome が無いときは、Chromium へ切り替えずに起動の失敗で止まります（fail-closed）。値は `chromium` か `chrome` だけで、それ以外は終了コード 2 です
- 原本は、Playwright の版を上げたときと公開（リリース）の前に流し直し、記録を更新する提案です（未承認）

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
| `make check-frontend-bump` | `scripts/check-frontend-bump.sh guard` | 版上げ PR の受け入れ確認 (4)(5)（FEND-17・D-63）の関門。`wasm-full` の `default-features = false` と `action-keydown` の明示を `cargo metadata` で確かめ、`CI_EVENT_NAME=pull_request` の版上げ PR では期待値・`tools/check-deps.sh`・`tools/build-web.sh`・関門自体が base と同じことを求める。(3) の WASM の gzip +10% は CI の `browser-test` が `wasm-gzip` で判定する |
| `make test-check-frontend-bump` | `scripts/test-check-frontend-bump.sh` | 上の関門自体の回帰テスト（一時の repo での模擬の版上げ） |
| `make check-wasm-size` | `scripts/check-wasm-size.sh` | 配信する設定の WASM の `gzip -9` が 1,048,576 bytes（1 MB。DLV-27）以下であることの関門（TASK-78 2/2）。`tools/measure/wasm-size.sh` で測り、基点（commit・`wasm-opt` の有無・設定）つきの JSON をログに残す。CI の `browser-test` が `SKIP_WASM_OPT=1` で流す。遅い |
| `make test-check-wasm-size` | `scripts/test-check-wasm-size.sh` | 上の関門自体の回帰テスト（ビルドなし） |
| `make update-test-counts` | `scripts/update-test-counts.sh` | 試験の件数の期待値 `scripts/expected-test-counts.txt` を実測で書き直す（遅い。ブラウザ試験も流す。版を上げる前に別の PR で行う） |
| `make check-toolchain` | `scripts/check-toolchain.sh` | 固定ファイルの版と `docs/toolchain.md` の一致、README の「前提」の表との一致を検査する |
| `make test-check-toolchain` | `scripts/test-check-toolchain.sh` | ツールチェーンの一致の検査自体の回帰テスト |
| `make check-workflow-secrets` | `scripts/check-workflow-secrets.sh` | `ci.yml` が secret を参照していないことを検査する |
| `make test-check-workflow-secrets` | `scripts/test-check-workflow-secrets.sh` | secret 参照の検査自体の回帰テスト |
| `make check-fork-ci` | `scripts/check-fork-ci.sh` | fork が secret 無しでも CI を通せる構成であることを、パーサで復号した値で検査する（A〜C。教材 repo 向けの D は `--pack-workflow` と `--engine-rev` を渡したときだけ。`ruby`・`jq`・`cargo` が必要） |
| `make test-check-fork-ci` | `scripts/test-check-fork-ci.sh` | fork CI 検査自体の回帰テスト |
| `make serve` | `node tools/serve.mjs` | `dist/web` をサブパス（既定 `/subpath/`、`--base` で任意の名前）の下で gzip つきで配る。例: `make build-web` の後に `node tools/serve.mjs --base /my-site/` を実行し、`http://127.0.0.1:8765/my-site/` を開く |
| `make check-generic-words` | `tools/check-generic-words.mjs` | エンジンの crate のコードに教材固有の語（語彙の付録 A）が無いことの検査（CI 未組み込み） |
| `make test-check-generic-words` | `scripts/test-check-generic-words.sh` | 教材固有の語の検査自体の回帰テスト |
| `make check-internal-info WORDS=<一覧のパス>` | `tools/check-internal-info.mjs` | 社内固有の語（private の spec repo の一覧）と認証情報の形の公開前の照合（CI 未組み込み。記録は `docs/publish-checks.md`） |
| `make check-internal-info-secrets` | `tools/check-internal-info.mjs` | 認証情報の形だけの照合（語の照合は行わない。CI 未組み込み） |
| `make test-check-internal-info` | `scripts/test-check-internal-info.sh` | 上の照合自体の回帰テスト |
| `make inventory-deps` | `tools/inventory-deps.mjs` | `Cargo.lock` の依存の取り下げ・所有者の数を crates.io で調べる（通信が要り数分かかる。CI 未組み込み。手順と記録は `docs/dependency-inventory.md`） |
| `make test-inventory-deps` | `scripts/test-inventory-deps.sh` | 依存の棚卸し自体の回帰テスト（通信なし） |
| `make test-upstream-impact` | `scripts/test-upstream-impact.sh` | 影響する Step の洗い出し道具（`tools/upstream-impact.mjs`。TASK-182）の回帰テスト（通信なし。CI 未組み込み。道具は `node tools/upstream-impact.mjs --help`） |
| `make test-serve` | `scripts/test-serve.sh` | 配信サーバの試験（Node の `node:test`） |
| `make test-browser` | `scripts/test-browser.sh` | 前段で `fandhe-pack` の debug ビルドを行い `FANDHE_PACK_BIN` で渡す（BUILD-16。TASK-91）。共通ヘルパ（`tools/browser/`）の試験・安全性の計数の対照と、配信設定の web をサブパス配信した最小の画面の試験・対照を挿入した自己確認（SAFE-8。CSP あり・なし）、巡回する IR 3 本が遷移 1,000 回で止まり警告が 1 件だけ残ること・巡回しない長い連鎖が 1,000 回ちょうどなら通り 1,001 回で止まることの試験（`transition-limit`。SAFE-23。TASK-160 1/3）と、止めたときに専用の見出しの行・通知欄の理由が出て着地が行に並ばないことの試験（SAFE-34。TASK-160 2/3）、止めた後は操作の前の Step に戻り次の入力が受け付けられることの試験（SAFE-35。TASK-160 3/3）、root の全置換が 0 回で参照をログが 1,000 項目を超える入力で 1 リストが 1,000 項目以下に畳まれ親が同じ要素のままである試験（list-fold。FEND-22。TASK-360 1/2）・artifact-limit（左の成果物の 4,095・4,096・4,097 項目と、境目をまたぐ入力と、古い側の畳みで、状態の行と root の非置換を確かめる試験。ART-14・FEND-22。TASK-484 2/2。`docs/testing.md` 5.295）・保持した 18 要素の同一性と入力欄のフォーカス・入力途中の値が保たれる試験と、全再描画（`rerender`）を起こすと保持した 6 要素が 0 件しか残らずフォーカスが失われる負の対照（element-identity。FEND-3。B-7・B-7n。TASK-348 2/3・3/3）、配信する `index.html` の CSP の meta が charset の直後にあり内容が仕様と一致することの検査（`csp-placement`。両設定。SAFE-13。TASK-151 1/2）、配信・試験の両設定の `index.html`・`app.css` がインラインの script・`style` 属性・`on*` 属性・`data:` と外部の取得を使わず実物の加工の対照で不合格になることの検査（`inline-free`。SAFE-15。TASK-151 2/2）、最小の画面・最小教材と CH-1 の誘導・試験用の教材の全 Step の移動・表示部品 14 通り・CH-4〜CH-9 の形の 17 Step の通し（試験の設定・配信する設定。章ごとの外部通信 0。TASK-674 2/2。`docs/testing.md` 5.285）を CSP あり・なしで流して通信・違反・コンソールの CSP 警告・エラーが 0 件で要求の種類が同じことの試験（`csp-runs`。SAFE-13。TASK-151 2/2）、信頼できない入力への安全性の観察の記録（`safety-observe`。BUILD-4。観察値は assert せず診断の行に残し、完走して終了コード 0 になることを確かめる）、試験の設定（`TEST_HOOKS=1`）の web で keydown の属性が届くことの試験、Esc が 10 場面（処理中か否か × フォーカスなし・root の外のボタン・入力欄・root の中のボタン・main の外の部品）でちょうど 1 回届く esc-once の試験（FEND-6・FEND-8。TASK-353）、配信設定の web で Enter の送信が送信ボタンの click と同じ記録になることと、Shift+Enter の改行・複数行の送信の試験、貼り付け（改行の保持・自動送信 0 件・合成の paste が止められないこと）の試験、上限の案内文と貼り付けの観察（TERM-15・TERM-19。PoC-2 の写しの試料・案内欄の位置・候補が開かないこと）の試験、CDP による IME の模擬のヘルパの自己確認と、入力欄と IME の層 3 の試験（TASK-86 1/2。変換中の誤送信 0 件・確定後の送信漏れ 0 件）と、保存済みの履歴が無い場合と未操作の再読み込みで新しく始まり世代が上がらない試験（TASK-319 1/3。操作後は復元される対照つき）、保存・復元・書き出し・版の層 3 の試験（TASK-86 2/2。往復・読み込み後の保存と復帰・版の判定・壊れた履歴の拒否）・枠の書き出し（ダウンロードのリンク）と読み込み（ファイル入力）の操作を画面の操作だけで通す試験（history-ops。SHELL-15。TASK-531 1/2）・受講の途中の履歴を別のオリジンで読み込むと同じ状態・画面になり、続きの操作の結果と書き出しのバイト列が一致する試験（history-continue。SHELL-15。TASK-531 2/2。`docs/testing.md` 5.244）・変換して開いた履歴で枠の通知欄に通知が 1 件出て閉じると消え、変換なし・表示だけでは出ない試験（SHELL-20。TASK-535 1/2。変換して開いた後の操作の保存が開いた側の新しい版で書かれる。TASK-535 2/2）・版の違う履歴 15 件を開き、種類・理由・軸が期待の表と合い、開く 7 件は写しが一致し、開かない 8 件は差し替えが 0 回である試験（VER-2。TASK-175 2/2）、Esc の画面の結果の試験（TASK-441 1/2・2/2。処理中は候補が開いていても停止して候補も閉じ、処理中でなく候補が開いていれば閉じるだけ、どちらでもなければ取り消しの入力で、受け付けない Step では拒否の行と次の行動が出る）、入力方式ごとに選択肢・承認のボタンが切り替わり画面操作の Step が左の行のクリックで受け付けられる試験（TERM-23。TASK-450 1/3）、入力欄なしの教材で入力欄・送信・補完の候補・OS の選択が 0 件になり、停止と選択肢・画面操作・承認・確認は出て最後まで進む試験（PACK-20。TASK-450 2/3）、CH-0〜CH-3 の形の 20 Step を 1 つのページで最後まで進め、CH-1 をキーボードだけで完走する試験（TERM-23。TASK-450 3/3）・`st-1-4`・`st-2-7` を含む 20 Step の全場面で選択肢のボタンに ID の文字が出る件数が 0 で、`label` を外した対照では検出する試験（choice-labels。TERM-25。TASK-446。`docs/testing.md` 5.271）・「次の章へ進む」だけで CH-0 から CH-3 まで章をまたいで通す試験（next-chapter。SHELL-10・INTRO-3。TASK-526 2/2。`docs/testing.md` 5.227）・CH-3 を最後に使う絞った IR（範囲外の章のノードは残し開始点だけ除く）で CH-3 の完了後に範囲外の章へ入る操作が 0 件で、範囲外の `?start=` は断られ、開始点を足した対照では出る試験（out-of-range-chapter。SHELL-12・D-67。TASK-527。`docs/testing.md` 5.228）・章の飛ばし・選択の部品が全時点で 0 件で章をまたぐ操作は「次の章へ進む」の 0 か 1 個だけの試験（chapter-select-absent。SHELL-11・D-73。TASK-528。`docs/testing.md` 5.240）・未信頼の値（HTML・スクリプト・`javascript:` の形）の表示の試験の骨格と負の対照（view-escape。文書を除く 6 ビュー × 2 変種 × 前後の 24 通りが CSP あり・なしで違反 0・計数 0、不活性の文書の `innerHTML` で危険な要素がちょうど 3 件、表示した文字の `innerHTML` への入れ直しが検出される。ART-6。TASK-482 1/2・2/2。`docs/testing.md` 5.288・5.289）、旧いエンジンが新しい教材（IR）を理由つきで断り、保存済みの履歴が書き換わらない試験（VER-13。TASK-181 2/2）（Playwright）。CI では `browser-test` ジョブが流します・L2 の完走試験が使う DOM だけの観察の較正（l2-journey-calibrate。試験の設定の `stateJson` と各 Step で一致。TASK-112 2/2。`docs/testing.md` 5.239） |
| `make test-build-pack-bin` | `scripts/test-build-pack-bin.sh` | `fandhe-pack` の前提バイナリの解決が `CARGO_TARGET_DIR` の別置きと既定の両方で動くことの回帰テスト（遅く、CI・フックには入れない） |
| `make test-chapter-key-control` | `scripts/test-chapter-key-control.sh` | 鍵を修正前の固定の文字列にした写しで、章の試験（`chapter-keys`）が「先の章の履歴が出た」理由で失敗することの対照。依存を冷えた状態からビルドするため遅く、CI・フックには入れません |
| `make test-keydown-control` | `scripts/test-keydown-control.sh` | `action-keydown` を外した写しで keydown の試験が「キーが届かない」理由で失敗することの対照。依存を冷えた状態からビルドするため遅く、CI・フックには入れません |
| `make build-web` | `tools/build-web.sh` | web を wasm にビルドして `dist/web` に配る一式を作る（`TEST_HOOKS=1` で試験の設定、`IR_FILE` で IR を同梱） |
| `make test-build-web` | `scripts/test-build-web.sh` | `build-web` の受入基準（compile した IR が配信する設定に入ることを含む）を実ビルドで検査する |
| `make build-l2` | `tools/build-l2.mjs` | L2 の配布物（`fandhe-workshop-l2/{start.sh, miniserve, site/}` の zip）を `dist/l2` に作る。`L2_SERVER` に miniserve 0.35.0 の実行ファイルが要り、`tools/l2/miniserve.sha256` の固定値と照合する。`L2_SITE`（既定 `dist/web`）・`L2_OUT`・`L2_PLATFORM`（`macos-arm64` 既定・`linux-x64`） |
| `make test-build-l2` | `scripts/test-build-l2.sh` | `build-l2` の回帰テスト（合成の配信物。`unzip` と node が要る） |
| `make test-l2-run` | `scripts/test-l2-run.sh` | L2 の配布物を Rust のない模擬環境（`PATH=/usr/bin:/bin`）で展開・起動し、Chromium で CH-0〜CH-1 を 3 回完走し、待ち受けが 127.0.0.1 だけで他のアドレスから拒否されること・起動とマウントの時間（記録のみ）、L1（サブパス 2 名）と L2 の 3 配信先 × 3 回の 9 回で操作ごとの観察・`snapshot.json`・`history.json` がバイト単位で一致すること、凍結した IR と組の固定の入力列（`crates/web/tests/data/fixed_sequence_ch0_1/`）での比較と履歴の `ops` に揺れる欄が無いことも確かめる。`L2_SERVER`（miniserve 0.35.0 の実行ファイル）が必須で、web の release ビルドを含むため遅い。`npm ci` と `npx playwright install chromium` が前提（`docs/testing.md` 5.239・5.241b・5.243・5.245b） |
| `make check-delivery` | `tools/check-delivery.sh` | 配信物（`dist/web`。先に `make build-web`）に試験用の口が無いことの検査。CI には未組み込み |
| `make test-check-delivery` | `scripts/test-check-delivery.sh` | 配信検査自体の回帰テスト（実ビルドを行うため遅い） |
| `make check-mutations` | `scripts/check-mutations.sh` | 変異注入の検査（誤りを入れると対応する試験が失敗することの確認。遅く、CI・フックには入れない） |
| `make test-check-mutations` | `scripts/test-check-mutations.sh` | 変異注入の検査自体の回帰テスト |
| `make measure-wasm-size` | `tools/measure/wasm-size.sh` | WASM の `gzip -9` の大きさを基点つきで測り、JSON を標準出力へ出す（様式は [docs/measurement.md](./docs/measurement.md)） |
| `make measure-history-size` | `tools/measure/history-size.sh` | 2 周分の履歴の大きさと `Changed` の割合を基点つきで測り、JSON を標準出力へ出す（様式は `docs/measurement.md` §4a。`make measure` の束には入れない） |
| `make test-measure-wasm-size` | `scripts/test-measure-wasm-size.sh` | 結果の様式の検証と実ビルドの回帰テスト（遅く、CI・フックには入れない） |
| `make measure` | `tools/measure/all.sh` | 3 つの測定（WASM の `gzip -9`・初回表示・再描画）を基点つきで流し、束の JSON を標準出力へ出す。要 `npm ci` と `npx playwright install chromium`（様式は [docs/measurement.md](./docs/measurement.md)） |
| `make test-measure` | `scripts/test-measure.sh` | 束の検証・fail-closed・一括実行の回帰テスト（遅く、ブラウザを要し、CI・フックには入れない） |
| `make test-measure-result` | `scripts/test-measure-result.sh` | 測定の結果の組み立てと基点の収集の回帰テスト（jq 1.7 の文法でも通ることを含む。速く、ビルドなし。CI の `check` ジョブが流す） |

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
| `ci.yml` | `check`: ローカル・hooks と同じ `make check` を実行する（editorconfig-checker / shellcheck はバージョン固定 + SHA256 検証で導入）。あわせて `make check-toolchain`・`make check-workflow-secrets`・`make check-fork-ci`（と各回帰テスト）も流す（`check-fork-ci` のために固定の Rust toolchain も導入する）。`rust-lint`: `make lint`・`make check-deps`・`make test-check-deps`・`make check-dom-reads`・`make test-check-dom-reads`（`rust-toolchain.toml` の版を rustup で導入し、導入された版を `scripts/ci-rust-toolchain.sh` で照合する）。`rust-test`: `make test`（`cargo test --locked --workspace`）がネイティブのビルドと試験を兼ねる。2 つの Rust ジョブはキャッシュを使わない（効果を実測してから足す）。`changes`: 変更ファイルから、フックの回帰テストを macOS でも実行するかを決める（`scripts/ci-changes.sh`。PR でフック関連の変更が無ければ ubuntu のみ、push・判定不能時は macOS も含める）。`test-hooks`: フックスクリプト自体の回帰テスト（`make test-hooks`。ubuntu-latest で常に実行する）。`test-hooks-macos`: 同じテストの macOS 版（macos-latest。標準の bash 3.2 でも実行する）。`changes` の結果で不要な場合は macOS ランナーを起動せず、チェックは skipped として報告される。`browser-test`: `make test-browser`（pass／fail 型の試験と `smoke-hello`）を同梱の Chromium で流す（TASK-79。Node・Playwright の導入を含み、キャッシュは使わない）。`pr-title`: PR タイトルを commit-msg フックと同じスクリプトで検証する（squash merge でコミット件名になるため）。`ci-complete`: 全ジョブ結果の集約（`test-hooks-macos` の skipped は、`changes` の結果と一致する場合だけ許容する） |
| `deploy.yml` | 配信の骨格（TASK-33）。`workflow_dispatch` だけで動き、`permissions: {}`。fork では skip される（`github.repository` が本 repo のときだけ動く）。中身は未実装で、実行すると失敗して止まる（TASK-121 で作る）。secret を参照するビルド・試験を `ci.yml` に置かないため、配信は PR の起点を持たない別ワークフローに分けている |
| `ai-review.yml` | Fandhe-AI/actions の ai-review（codex）による PR 自動レビュー。Actions variable `CODEX_HOME_DIR` が未設定の間は skip される |
| `update-external.yml` | エージェントスキル（`skills-lock.json`）と submodule（`.gitmodules`）の日次自動追従 PR。secrets は org の `SUBMODULE_PAT` を使う（`SKILLS_PAT` 未登録時は共通側が `SUBMODULE_PAT` へフォールバック）。作成する PR には `dependencies` / `automated` ラベルが付く |

- `fandhe-frontend` の版上げ PR は、受け入れ確認 (1)〜(5) を CI で確かめます。判断・事前の PR・外れたときの対処は `docs/testing.md` の 6.6.2 にあります（TASK-363）
- `main` は ruleset `main-protection` で保護しています（2026-10-11 適用）。直接 push はできず、変更はすべて PR を経由します。
  マージ方式は squash のみで、必須チェックがすべて成功し、レビュースレッドがすべて解決済みであることが条件です（承認数は 0）
- ruleset の必須チェック（required status checks）には、PR の HEAD に報告される全チェック（`ci.yml` の全ジョブ・ai-review の `codex / *`・Cursor Bugbot）を登録しています。
  `implement-issue-tree` の自動マージは、必須に無いチェックが HEAD に 1 件でもある場合と、必須のチェックが HEAD に報告されない場合に辞退するためです。
  方針の詳細は `.claude/rules/github-actions.md` の「必須チェック」、ruleset・マージ設定の導入は `setup-repo-guards` スキルの手順に従います
- 2026-10-14 に、`rust lint (fmt + clippy + check-deps)` と `rust build + test (--workspace)` の 2 件（発行元は github-actions）を必須チェックに加え、合計 12 件にしました。`deploy.yml` の `deploy` は PR に報告されないため、必須に入れません。詳細は `.claude/rules/github-actions.md` の「必須チェック」です
- 2026-10-18 に、`browser test (Chromium)`（発行元は github-actions）を必須チェックに加え、合計 13 件にしました
- 条件付きのジョブは、ジョブ単位の `if:` で制御し、不要な場合もチェック自体は skipped として報告させます（例: `test-hooks-macos`）
- CI のジョブを追加・改名・削除したら、`ci-complete` の `needs` と ruleset の必須チェックを同じタイミングで更新します

### fork の CI

第三者が fork して secret を設定せずに CI を流しても、ビルドと試験が通る構成を保ちます。`make check-fork-ci` が、
次の 3 点（A〜C）を静的に検査します。教材 repo 向けの D は、`--pack-workflow` と `--engine-rev` を渡したときだけ動きます。YAML は Psych（ruby 標準）でノード木にし、Cargo の依存元は `cargo metadata --locked`、
`package-lock.json` は `jq`、`.gitmodules` は git 自身の設定パーサで復号し、字句ではなく復号した値で判定します。

- A: private な依存が 0 件で、submodule を取得しない
- B: `ci.yml` からの secret の参照が 0 件で、`uses` は許可した所有者の 40 桁 SHA 固定だけ
- C: `ci.yml` のトップレベルの `permissions` が読み取り専用で、配信（`deploy.yml`）は別ワークフローに切り離され、fork では skip される
- D: 教材 repo の workflow が、固定したエンジンの版（`ENGINE_REV`。C-14）でエンジンを取得する。`ENGINE_REV` は 40 桁の小文字の 16 進を 1 行だけ持つ（D1）。workflow には、次の正準の 3 step で始まるジョブが 1 つ以上必要で（D2・D3）、4 番目以降の step は `actions/checkout`・`git` の語・エンジンの repo・`ENGINE_REV`・エンジンの置き場に触れてはならない（D4）

D の正準の 3 step は、次の 1 つだけです。形が外れたものは拒否します。

1. この repo の `actions/checkout`（40 桁 SHA 固定、`persist-credentials: false` だけを指定する）
2. `ENGINE_REV` を読んで出力する 1 行の step（`id` と `run: echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"` だけを持つ）
3. 2 の出力を `ref` にしたエンジンの `actions/checkout`（`repository`・`ref`・`path`・`persist-credentials: false` だけを指定する）

`git clone` や `cargo install --git` による取得は、正準に含めず一律に拒否します。シェルの意味は解釈せず、復号した値の許可リストで判定します。エンジンは教材 repo 自身のビルド定義（`Cargo.toml` の path 依存・Makefile）から使う前提です。

検査の限界は次のとおりです（2026-10-19 時点）。

- 自分たちが書く構成の誤りを防ぐための検査であり、悪意ある変更を防ぐ境界ではありません
- `run` の中のシェルの意味は解釈しません（`submodule` の語の有無だけを見ます。難読化・変数での組み立て・`eval` は見抜けません）
- `run` から呼ぶスクリプト（`scripts/*.sh`・`make` のターゲット）の中身は解釈しません
- runner・利用者の環境の設定（`~/.npmrc`・`$CARGO_HOME/config.toml`・global の git 設定）は対象外です
- D はエンジンに触れないジョブを検査しません。文字列を組み立てた URL・action の内部の動き・artifact の受け渡しも見抜けません。TMPL-9 で案 B を採る場合の作り直しは TASK-765 の範囲です
- 実際の fork での実行は行っていません。D は試験の見本でだけ確かめており、実際の教材 repo（TASK-36）への適用もしていません（D-29 の行わない確認。2026-10-19 時点）

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
