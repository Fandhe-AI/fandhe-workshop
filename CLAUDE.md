# CLAUDE.md

Claude Code がこのリポジトリで作業するときの要約。人向けの詳しい説明（セットアップ・コマンド・CI・MCP）は
`README.md` に置き、ここでは重複させない。

## Overview

fandhe-workshop は、体験型教材を作成・実行する汎用 OSS エンジンの実装リポジトリ（public）。
教材作成者が章・Step・入力条件・状態変化・完了条件・ヒントをデータとして書くと、ブラウザの中で、ターミナル風の
操作に応じて成果物が変わる教材として動くことを目指す。エンジンは特定の教材の知識を持たない。

- 開発基盤は `Fandhe-AI/template-dev` から作成した。現時点の中身は Make・シェルスクリプト・lefthook・GitHub Actions
- **言語固有の設定・実装（Rust 等）は未導入**。実装着手の指示後に、タスクに沿って追加する
- 仕様は private の submodule `docs/spec`（`Fandhe-AI/fandhe-workshop-spec`）。取得手順は README の「仕様」

## Repository Structure

```text
.
├── CLAUDE.md                 # 本ファイル
├── README.md                 # 人向けの説明（構成・セットアップ・コマンド・CI・MCP）
├── Makefile                  # scripts/ を呼ぶだけの薄い入口（make help で一覧）
├── lefthook.yml              # Git hooks 定義（pre-commit / commit-msg）
├── scripts/
│   ├── lib.sh                # 共有ヘルパー（require_cmd / reject_args / log）
│   ├── help.sh / doctor.sh / setup.sh / check.sh
│   ├── ci-changes.sh         # CI の test-hooks 対象 OS の判定
│   └── hooks/                # lefthook から呼ばれるフックの実体と回帰テスト
├── .github/workflows/
│   ├── ci.yml                # check / test-hooks / pr-title / ci-complete
│   ├── ai-review.yml         # codex による PR 自動レビュー
│   └── update-external.yml   # スキル・submodule の日次追従 PR
├── .claude/
│   ├── agents/<category>/    # Sub-agents（research / implement / testing / quality / docs）
│   ├── rules/                # ルール（一部は paths で読み込み条件を限定）
│   ├── settings.json         # 共有設定（SessionStart hook）
│   ├── skills/               # .agents/skills/ への symlink
│   └── workflows/            # implement-issue-tree.js（スキル側の実体への symlink）
├── .agents/skills/           # スキルの実体（skills-lock.json で管理する外部取得物）
├── skills-lock.json
├── .mcp.json                 # プロジェクト共有 MCP サーバー（Playwright MCP）
├── docs/spec/                # 仕様（private submodule。update = none）
└── _/                        # スキルが使うローカル状態（git 管理外。_/local-plans/ 等）
```

## コマンド

| コマンド | 内容 |
|---|---|
| `make help` | ターゲット一覧 |
| `make doctor` | 開発環境の診断（読み取りのみ） |
| `make setup` | Git hooks の有効化と `.env` の作成（再実行可） |
| `make check` | editorconfig-checker + shellcheck（ソースは変更しない） |
| `make test-hooks` | `scripts/hooks/` の回帰テスト |

build・test・lint など言語固有のターゲットはまだ無い。変更後は `make check` を通す。

## 委譲方針（必読）

main のコンテキストは、対話・計画・判断のために残す。調査と実装は subagent へ委譲し、結論だけを受け取る。
詳細は `.claude/rules/delegation.md`（調査・設計）と `.claude/rules/delegation-impl.md`（作成・編集）。

### main の役割

- ユーザーの意図の確認・計画の作成・承認の取得
- subagent への依頼と、結果の統合・検証
- コミット・PR 作成など、承認が要る操作の実行
- 1 ファイル・数行で済む修正は、main が直接行ってよい

### パスベース切り替え表

| パス | 調査 | 実装 | 適用ルール |
|---|---|---|---|
| `Makefile`・`scripts/*.sh` | `explorer` | `infra-builder` | `makefile.md`・`coding-shell.md` |
| `lefthook.yml`・`scripts/hooks/**` | `explorer` | `infra-builder` | `git-hooks.md`・`coding-shell.md` |
| `.github/workflows/**` | `explorer` | `infra-builder` | `github-actions.md` |
| `.claude/rules/**`・`.claude/agents/**`・`.claude/settings.json`・`.mcp.json` | `explorer` | `dotclaude-author` | `delegation.md`・`security.md` |
| `README.md`・`CLAUDE.md` | `explorer` | `docs-writer` | `japanese-style.md` |
| `Cargo.toml`・`**/*.rs`（未導入） | `explorer` | 未定義（着手時に `update-claude` で builder を追加） | `rust-build-profile.md` |
| 外部仕様の確認 | `reference-researcher` | — | `.claude/skills/` のリファレンススキル |
| `.agents/skills/**`・`.claude/skills/**` | — | 編集しない（`contribute-skill` で upstream へ） | — |
| `docs/spec/**` | `explorer` | 編集しない（spec リポジトリ側で変更） | — |

実装後は `test-runner`（検証）→ `reviewer`（レビュー）→ `security-auditor`（PR 作成前）の順に委譲する。

### model 配分表

| 用途 | model tier | alias |
|---|---|---|
| 複雑な横断判断・アーキテクチャ設計 | 最上位 tier | `opus` |
| 調査・生成・実装・レビュー | 標準 tier | `sonnet` |
| 機械的な集計・検査の実行・ドキュメント更新 | 軽量 tier | `haiku` |

Agent の `model:` には、この表の alias だけを書く。最上位 tier は、大規模な設計や複数領域にまたがる判断に限って使う。

## Sub-agents

定義は `.claude/agents/<category>/<name>.md`。

| カテゴリ | subagent_type | model | 役割 |
|---|---|---|---|
| research | `explorer` | `sonnet` | コードベース横断の調査（読み取り専用） |
| research | `reference-researcher` | `sonnet` | 外部仕様・リファレンススキルの調査（読み取り専用） |
| implement | `infra-builder` | `sonnet` | Makefile・scripts・lefthook・GitHub Actions の実装 |
| implement | `dotclaude-author` | `sonnet` | `.claude/rules`・`.claude/agents`・`settings.json`・`.mcp.json` の作成・編集 |
| testing | `test-runner` | `haiku` | `make check`・`make test-hooks` の実行と結果の要約 |
| quality | `reviewer` | `sonnet` | 差分のレビュー（読み取り専用） |
| quality | `security-auditor` | `sonnet` | 秘密情報・インジェクション・CI の権限と供給網の監査（読み取り専用） |
| docs | `docs-writer` | `haiku` | `README.md`・`CLAUDE.md` の更新 |

エンジンの実装に対応する builder（`rust-builder` 等）と `e2e-runner` は未定義。実装着手時に追加する。

## Rules

定義は `.claude/rules/*.md`。「読み込み」が「常時」のものは毎セッション読み込まれ、それ以外は該当パスの編集時のみ
読み込まれる。

| ファイル | 読み込み | 内容 |
|---|---|---|
| `delegation.md` | 常時 | 調査・設計フェーズの委譲原則・パスベース切り替え・編集しないパス |
| `delegation-impl.md` | 常時 | 作成・編集フェーズの委譲マッピング・実装後の流れ |
| `security.md` | 常時 | 秘密情報の混入防止・インジェクション・供給網・OWASP Top 10 |
| `japanese-style.md` | 常時 | 日本語の文体・表記 |
| `conventional-commits.md` | 常時 | コミット件名・PR タイトルの形式と検証の仕組み |
| `out-of-scope-tracking.md` | 常時 | スコープ外の事項を Issue に記録する流れ |
| `coding-shell.md` | `**/*.sh`・`.shellcheckrc` | bash 3.2 互換・クォート・エラー処理・shellcheck |
| `code-comment-style.md` | `**/*.sh`・`Makefile`・`lefthook.yml`・`.github/workflows/**`・`**/*.rs`・`**/*.toml` | コメントに残す内容とファイル種別ごとの形式 |
| `makefile.md` | `Makefile`・`scripts/*.sh` | 薄い入口の方針・ターゲット追加手順・スクリプトの規約 |
| `git-hooks.md` | `lefthook.yml`・`lefthook-local.yml`・`scripts/hooks/**` | フックの時間予算・CI との役割分担・fail-closed |
| `github-actions.md` | `.github/workflows/**` | runner 方針・必須チェック・発火条件・キャッシュ方針 |
| `rust-build-profile.md` | `Cargo.toml`・`**/Cargo.toml`・`.cargo/**`・`rust-toolchain`・`rust-toolchain.toml` | Rust 導入時の dev プロファイル・`target/` の掃除・CI キャッシュへの影響 |

## Current Skills

`skills-lock.json` で管理する外部取得物（実体は `.agents/skills/`）。直接編集せず、更新は `update-external.yml`
の日次 PR に任せる。

### 作業スキル（`Fandhe-AI/agent-cli-skills`・16 件）

| 用途 | スキル |
|---|---|
| 計画 | `create-plan` |
| Issue | `create-issue`・`create-issue-tree`・`update-issue-tree` |
| 実装 | `implement-issue`・`implement-issue-tree` |
| レビュー | `implement-review`・`implement-review-pr` |
| コミット・PR | `create-commit`・`create-pr` |
| ドキュメント・コメント | `update-docs`・`comment-code` |
| `.claude/` 体系・リポジトリ設定 | `init-claude`・`update-claude`・`setup-repo-guards` |
| スキルの upstream への貢献 | `contribute-skill` |

### リファレンススキル（`Fandhe-AI/agent-reference-skills`・10 件）

`rust`・`fandhe-frontend`・`playwright`・`make`・`lefthook`・`github-docs`・`editorconfig`・`commitlint`・
`anthropic-claude-code`・`openai-codex`

## Conventions

- **言語**: ユーザーとのやりとり・ドキュメント・コメントは日本語（`japanese-style.md`）
- **Conventional Commits**: コミット件名と PR タイトルは `<type>[(<scope>)][!]: <要約>`（`conventional-commits.md`）
- **フックを回避しない**: `--no-verify`・`LEFTHOOK=0` は使わない。拒否されたら原因を直す
- **承認フロー**: `implement-issue` は計画（`_/local-plans/`）の承認後に実装する。コミット・push・PR・Issue の
  作成は、ユーザーが依頼した場合に行う
- **セキュリティ**: PR 作成前にセキュリティレビューを行い、問題があれば PR を作らない（`security.md`）。
  `.env` は読まない・出力しない
- **二重定義をしない**: 処理の実体は `scripts/` に置き、Makefile・lefthook・CI からはそれを呼ぶ
- **スコープ**: 依頼の範囲外で見つけた問題は直さず、Issue に記録する（`out-of-scope-tracking.md`）
- **検証**: 変更後は `make check` を通す。`scripts/hooks/` を変更したら `make test-hooks` も通す
- **ドキュメントの同期**: 構成・コマンド・CI・ルール・Agent を変えたら、`README.md` と本ファイルの表も更新する

## hooks（settings.json）

| hook | 内容 |
|---|---|
| SessionStart | 日本語・委譲・Conventional Commits・フック回避の禁止・計画承認・`.env` を読まないことのリマインダーを表示する |
| PostToolUse | 設定していない。整形ツール（shfmt 等）を採用しておらず、検査は `make check` と Git hooks が行う |

言語のフォーマッタ（rustfmt 等）を導入したら、PostToolUse の自動整形を検討する。個人用の設定は
`.claude/settings.local.json`（git 管理外）に置く。

## implement-issue-tree の前提

- `gh` CLI が認証済みであること（`gh auth status`）
- `.claude/workflows/implement-issue-tree.js` は、スキル側の実体
  （`../skills/implement-issue-tree/scripts/implement-issue-tree.js`）への相対 symlink。コピーに置き換えない
  （スキルの更新が届かなくなる）
- 一時 worktree は `.claude/worktrees/`、ツリーの状態は `_/issue-trees/` に置かれる（どちらも git 管理外）
