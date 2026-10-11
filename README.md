# fandhe-workshop

体験型教材を作成・実行する汎用 OSS エンジンの実装リポジトリです。
教材作成者が章・Step・入力条件・状態変化・完了条件・ヒントをデータとして書くと、
ブラウザの中で、ターミナル風の操作に応じて成果物（ファイル・差分・Issue・PR／CI の状態など）が変わる教材として動くことを目指します。
エンジンは特定の教材の知識を持たず、教材は別のリポジトリで作成します。

開発基盤は [Fandhe-AI/template-dev](https://github.com/Fandhe-AI/template-dev) から作成しています。
言語固有の設定・実装は、実装着手の指示後にタスクに沿って追加します。

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
| `.claude/rules/` | Claude Code のルール（`git-hooks.md`: `lefthook.yml`・`scripts/hooks/` 編集時のみ読み込まれる、フックの時間予算と CI との役割分担、`makefile.md`: `Makefile`・`scripts/*.sh` 編集時のみ読み込まれる、薄い入口の方針とターゲット追加手順、`github-actions.md`: `.github/workflows/` 編集時のみ読み込まれる、runner 方針・必須チェック・発火条件・ジョブ追加チェックリスト・キャッシュ方針、`rust-build-profile.md`: `Cargo.toml`・`.cargo/`・`rust-toolchain.toml` 編集時のみ読み込まれる、Rust 導入時の dev プロファイル・`target/` の掃除・CI キャッシュへの影響。このほか、委譲・セキュリティ・日本語スタイル・Conventional Commits・スコープ外の追跡・シェルのコーディング規約・コメント規約の共通ルールがあり、一覧と読み込み条件は `CLAUDE.md` の「Rules」に記載） |
| `CLAUDE.md` | Claude Code 向けの要約（構成・委譲方針・model 配分・Sub-agents／Rules／Skills の一覧） |
| `AGENTS.md` | AI エージェントと PR 自動レビュー（ai-review）が共通で使うレビュー観点集（セキュリティ・設計整合・再利用の 3 観点と優先度 P0 / P1 / P2）。PR の base コミットの内容が基準になるため、変更はマージ後の次の PR から有効 |
| `.claude/agents/` | Claude Code の Sub-agents 定義（`research` / `implement` / `testing` / `quality` / `docs` のカテゴリ別。一覧は `CLAUDE.md` の「Sub-agents」） |
| `.claude/settings.json` | Claude Code のプロジェクト共有設定（SessionStart hook のリマインダー。個人設定は git 管理外の `.claude/settings.local.json`） |
| `.claude/workflows/` | `implement-issue-tree` の workflow 定義（スキル側の実体への相対 symlink） |
| `Makefile` | `scripts/` を呼ぶだけの薄い入口（`make help` で一覧） |
| `scripts/` | 処理の実体（`help` / `doctor` / `setup` / `check`）。Make なしでも直接実行できる |
| `scripts/hooks/` | lefthook から呼ばれる Git hooks の実体 |
| `lefthook.yml` | Git hooks 定義（pre-commit / commit-msg） |
| `.editorconfig`・`.editorconfig-checker.json` | 文字コード・改行・インデントの宣言と、その検査の除外設定（ライセンス本文は除外） |
| `.shellcheckrc` | shellcheck が `source` 先（`scripts/lib.sh`）を追って検査するための設定 |
| `.envrc`・`.env.example` | direnv による `.env` の読み込みと、その雛形 |
| `.mcp.json` | Claude Code のプロジェクト共有 MCP サーバー定義（Playwright MCP。後述の「MCP サーバー」） |
| `docs/spec/` | 仕様・フェーズ文書（private の submodule。前述の「仕様」） |
| `.gitmodules` | submodule 定義（`docs/spec` を `update = none` で登録） |
| `LICENSE-MIT`・`LICENSE-APACHE` | ライセンス（前述の「ライセンス」） |
| `.github/workflows/` | CI・AI PR レビュー・外部ソース自動追従（後述の「CI」） |

## 必要なツール

| ツール | 用途 | 必須 |
|---|---|---|
| [GNU Make](https://www.gnu.org/software/make/) 3.81+ | `make` 入口 | ✓ |
| [lefthook](https://lefthook.dev/) | Git hooks | ✓ |
| [editorconfig-checker](https://github.com/editorconfig-checker/editorconfig-checker) | `.editorconfig` 準拠チェック | ✓ |
| [ShellCheck](https://www.shellcheck.net/) | シェルスクリプトの lint | ✓ |
| [direnv](https://direnv.net/) | `.env` の自動読み込み | 任意 |
| [Node.js](https://nodejs.org/)（`npx`） | Playwright MCP の起動（後述の「MCP サーバー」） | 任意 |

macOS（Homebrew）の例:

```bash
brew install lefthook editorconfig-checker shellcheck direnv
```

`make doctor` で導入状況を確認できます（読み取りのみで、何も導入・変更しません）。

## セットアップ

```bash
make doctor   # 必要なツールが揃っているか確認する
make setup    # Git hooks を有効化し、.env が無ければ .env.example から作成する
direnv allow  # direnv を使う場合のみ。.envrc の読み込みを許可する
```

`make setup` は何度実行しても安全です（既存の `.env` は上書きしません）。

## コマンド

| コマンド | 直接実行 | 内容 |
|---|---|---|
| `make help` | `scripts/help.sh` | 操作の一覧を表示する |
| `make doctor` | `scripts/doctor.sh` | 開発環境を診断する |
| `make setup` | `scripts/setup.sh` | Git hooks 有効化と `.env` 雛形の配置 |
| `make check` | `scripts/check.sh` | editorconfig-checker + shellcheck（ソースは変更しない） |
| `make test-hooks` | `scripts/hooks/test-hooks.sh` | フックスクリプト自体の回帰テスト（一時 git リポジトリで検出・許可ケースを検証。bash + git のみ） |

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
| `ci.yml` | `check`: ローカル・hooks と同じ `make check` を実行する（editorconfig-checker / shellcheck はバージョン固定 + SHA256 検証で導入）。`changes`: 変更ファイルから、フックの回帰テストを macOS でも実行するかを決める（`scripts/ci-changes.sh`。PR でフック関連の変更が無ければ ubuntu のみ、push・判定不能時は macOS も含める）。`test-hooks`: フックスクリプト自体の回帰テスト（`make test-hooks`。ubuntu-latest で常に実行する）。`test-hooks-macos`: 同じテストの macOS 版（macos-latest。標準の bash 3.2 でも実行する）。`changes` の結果で不要な場合は macOS ランナーを起動せず、チェックは skipped として報告される。`pr-title`: PR タイトルを commit-msg フックと同じスクリプトで検証する（squash merge でコミット件名になるため）。`ci-complete`: 全ジョブ結果の集約（`test-hooks-macos` の skipped は、`changes` の結果と一致する場合だけ許容する） |
| `ai-review.yml` | Fandhe-AI/actions の ai-review（codex）による PR 自動レビュー。Actions variable `CODEX_HOME_DIR` が未設定の間は skip される |
| `update-external.yml` | エージェントスキル（`skills-lock.json`）と submodule（`.gitmodules`）の日次自動追従 PR。secrets は org の `SUBMODULE_PAT` を使う（`SKILLS_PAT` 未登録時は共通側が `SUBMODULE_PAT` へフォールバック）。作成する PR には `dependencies` / `automated` ラベルが付く |

- `main` は ruleset `main-protection` で保護しています（2026-10-11 適用）。直接 push はできず、変更はすべて PR を経由します。
  マージ方式は squash のみで、必須チェックがすべて成功し、レビュースレッドがすべて解決済みであることが条件です（承認数は 0）
- ruleset の必須チェック（required status checks）には、PR の HEAD に報告される全チェック（`ci.yml` の全ジョブ・ai-review の `codex / *`・Cursor Bugbot）を登録しています。
  `implement-issue-tree` の自動マージは、必須に無いチェックが HEAD に 1 件でもある場合と、必須のチェックが HEAD に報告されない場合に辞退するためです。
  方針の詳細は `.claude/rules/github-actions.md` の「必須チェック」、ruleset・マージ設定の導入は `setup-repo-guards` スキルの手順に従います
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
