# template-dev

Fandhe-AI の開発リポジトリ用テンプレートです。言語に依存しない開発基盤
（Claude Code / Codex のスキル体系・Git hooks・EditorConfig・Make の入口・環境変数の雛形）を提供し、
派生リポジトリで言語固有の設定を追加して使います。

## 構成

| パス | 役割 |
|---|---|
| `.agents/skills/`・`.claude/skills/` | エージェント用スキル（`skills-lock.json` で管理する外部取得物。`.claude/skills/` は `.agents/skills/` への symlink） |
| `Makefile` | `scripts/` を呼ぶだけの薄い入口（`make help` で一覧） |
| `scripts/` | 処理の実体（`help` / `doctor` / `setup` / `check`）。Make なしでも直接実行できる |
| `scripts/hooks/` | lefthook から呼ばれる Git hooks の実体 |
| `lefthook.yml` | Git hooks 定義（pre-commit / commit-msg） |
| `.editorconfig`・`.editorconfig-checker.json` | 文字コード・改行・インデントの宣言と、その検査の除外設定 |
| `.shellcheckrc` | shellcheck が `source` 先（`scripts/lib.sh`）を追って検査するための設定 |
| `.envrc`・`.env.example` | direnv による `.env` の読み込みと、その雛形 |
| `.mcp.json` | Claude Code のプロジェクト共有 MCP サーバー定義（ベースは空） |
| `.gitmodules` | submodule 定義（ベースは空。記入例をコメントで記載） |
| `.github/workflows/` | CI・AI PR レビュー・外部ソース自動追従（後述の「CI」） |

## 必要なツール

| ツール | 用途 | 必須 |
|---|---|---|
| [GNU Make](https://www.gnu.org/software/make/) 3.81+ | `make` 入口 | ✓ |
| [lefthook](https://lefthook.dev/) | Git hooks | ✓ |
| [editorconfig-checker](https://github.com/editorconfig-checker/editorconfig-checker) | `.editorconfig` 準拠チェック | ✓ |
| [ShellCheck](https://www.shellcheck.net/) | シェルスクリプトの lint | ✓ |
| [direnv](https://direnv.net/) | `.env` の自動読み込み | 任意 |

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

## CI

| ワークフロー | 内容 |
|---|---|
| `ci.yml` | `check`: ローカル・hooks と同じ `make check` を実行する（editorconfig-checker / shellcheck はバージョン固定 + SHA256 検証で導入）。`pr-title`: PR タイトルを commit-msg フックと同じスクリプトで検証する（squash merge でコミット件名になるため）。`ci-complete`: 全ジョブ結果の集約 |
| `ai-review.yml` | Fandhe-AI/actions の ai-review（codex）による PR 自動レビュー。Actions variable `CODEX_HOME_DIR` が未設定の間は skip される |
| `update-external.yml` | エージェントスキル（`skills-lock.json`）と submodule（`.gitmodules`）の日次自動追従 PR。secrets は org の `SUBMODULE_PAT` を使う（`SKILLS_PAT` 未登録時は共通側が `SUBMODULE_PAT` へフォールバック）。作成する PR には `dependencies` / `automated` ラベルが付く |

- ruleset の required status checks には `ci-complete`（と ai-review の `codex / *`）を登録する。
  ruleset・マージ設定の導入は `setup-repo-guards` スキルの手順に従う
- CI にジョブを追加したら `ci-complete` の `needs` にも必ず追加する

## MCP サーバー

`.mcp.json` はベースとして空（`"mcpServers": {}`）にしてある。派生リポジトリで必要なサーバーを追加する。
API キー等は値を直接書かず `"${EXAMPLE_API_KEY:-}"` のように環境変数参照とし、実値は `.env`（direnv 経由）に置く。

## 環境変数

- 雛形は `.env.example`（キー名と空値・ダミー値のみ）、実値は `.env`（git 管理外）に置きます
- direnv を導入している場合、`.envrc` が `.env` をシェルへ読み込みます
- 個人用の追加設定は `.envrc.local`（git 管理外）に置けます

## 派生リポジトリでの拡張

1. **言語固有のコマンド**: `scripts/` に処理（例: `scripts/verify.sh`）を追加し、`Makefile` に 1 行で呼ぶターゲットと `## ` コメントを足し、`scripts/help.sh` の一覧も更新する（中身の無いターゲット `@true` 等は置かない）
2. **`make check` の拡張**: `scripts/check.sh` に fmt チェック・lint 等を追記する（自動修正は別ターゲットに分ける）
3. **hooks の追加**: `lefthook.yml` に job を追加し、実体が長くなる場合は `scripts/hooks/` へ切り出す
4. **EditorConfig**: 言語ごとのセクション（例: `[*.rs] indent_size = 4`）を `.editorconfig` に追加する
5. **`.gitignore`**: ビルド成果物（`/target`・`/dist`・`node_modules/` 等）を追加する
