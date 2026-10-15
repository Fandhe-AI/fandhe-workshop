# CLAUDE.md

Claude Code がこのリポジトリで作業するときの要約。人向けの詳しい説明（セットアップ・コマンド・CI・MCP）は
`README.md` に置き、ここでは重複させない。

## Overview

fandhe-workshop は、体験型教材を作成・実行する汎用 OSS エンジンの実装リポジトリ（public）。
教材作成者が章・Step・入力条件・状態変化・完了条件・ヒントをデータとして書くと、ブラウザの中で、ターミナル風の
操作に応じて成果物が変わる教材として動くことを目指す。エンジンは特定の教材の知識を持たない。

- 開発基盤は `Fandhe-AI/template-dev` から作成した。現時点の中身は Make・シェルスクリプト・lefthook・GitHub Actions
- Cargo ワークスペース（4 crate）を導入済み。pack crate は読み込み層と `fandhe-pack compile <pack-dir>`・`compare <pack-dir-a> <pack-dir-b>` を実装済みで、他は骨格中心（2026-10-15 時点）。Rust のコーディング規約は `coding-rust.md`。エンジンの実装・Rust 向けの builder Agent は、タスクに沿って追加する
- 仕様は private の submodule `docs/spec`（`Fandhe-AI/fandhe-workshop-spec`）。取得手順は README の「仕様」

## Repository Structure

```text
.
├── CLAUDE.md                 # 本ファイル
├── AGENTS.md                 # レビュー観点集（ai-review が PR の base コミットから読む。P0 / P1 / P2）
├── README.md                 # 人向けの説明（構成・セットアップ・コマンド・CI・MCP）
├── Cargo.toml / Cargo.lock   # Cargo ワークスペース（仮想 manifest）と固定した依存の版
├── crates/                   # contract / core / pack / web の 4 crate（pack は compile・compare まで実装済み）
├── testdata/                 # 試験データ（spec repo の写し。出所は testdata/README.md。整形ツールの対象外）
├── rust-toolchain.toml       # Rust の版・component・wasm32 ターゲットの固定
├── package.json / package-lock.json / .npmrc  # Node・Playwright の版の固定（ブラウザ試験用）
├── docs/toolchain.md         # 固定した版と前提の一か所の正（check-toolchain が一致を検査）
├── docs/measurement.md       # 測定の結果の様式（基点の 4 項目と比較の規則。実行は make measure）
├── docs/testing.md           # 性質テストと変異注入の手順・記録の様式（変異の定義は crates/*/tests/mutations/）
├── Makefile                  # scripts/ を呼ぶだけの薄い入口（make help で一覧）
├── lefthook.yml              # Git hooks 定義（pre-commit / commit-msg）
├── tools/                    # spec が置き場を指定したスクリプト（build-web.sh・check-deps.sh・serve.mjs・browser/・measure/。規約・shellcheck は scripts/ と同じ）
├── scripts/
│   ├── lib.sh                # 共有ヘルパー（require_cmd / reject_args / log）
│   ├── help.sh / doctor.sh / setup.sh / check.sh
│   ├── test-keydown-control.sh # action-keydown を外した対照の検査（make test-keydown-control）
│   ├── check-mutations.sh    # 変異注入の検査（回帰テストは test-check-mutations.sh）
│   ├── check-toolchain.sh    # 固定ファイルと docs/toolchain.md の版の一致の検査（回帰テストは test-check-toolchain.sh）
│   ├── check-workflow-secrets.sh # ci.yml が secret を参照しないことの検査（回帰テストは test-check-workflow-secrets.sh）
│   ├── ci-rust-toolchain.sh  # CI で導入された rustc の版と docs/toolchain.md の照合
│   ├── ci-changes.sh         # CI の test-hooks-macos の実行要否の判定
│   └── hooks/                # lefthook から呼ばれるフックの実体と回帰テスト
├── .github/workflows/
│   ├── ci.yml                # changes / check / rust-lint / rust-test / test-hooks / test-hooks-macos / pr-title / ci-complete
│   ├── deploy.yml            # 配信の骨格（workflow_dispatch のみ・permissions {}・未実装で失敗。TASK-121 で実装）
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
├── CONTRIBUTING.md           # 貢献ガイド（人向けの入口）
├── docs/glossary.md          # 用語集（章・Step・IR・語彙など）
├── docs/architecture.md      # 実装 repo の構成設計（crate の境界・依存の向き・型の置き場。版つきの契約）
├── docs/spec/                # 仕様（private submodule。update = none）
└── _/                        # スキルが使うローカル状態（git 管理外。_/local-plans/ 等）
```

## コマンド

| コマンド | 内容 |
|---|---|
| `make help` | ターゲット一覧 |
| `make doctor` | 開発環境の診断（読み取りのみ） |
| `make setup` | Git hooks の有効化と `.env` の作成（再実行可） |
| `make check` | editorconfig-checker + shellcheck + testdata の参照検査（ソースは変更しない） |
| `make test-hooks` | `scripts/hooks/` の回帰テスト |
| `make lint` | Rust の fmt check + clippy + web の wasm32 clippy（ソースは変更しない） |
| `make fmt` | Rust の整形（ソースを変更する） |
| `make test` | Rust の native 試験（web を含む全 crate） |
| `make check-deps` | crate 間の依存の向きの検査（`cargo metadata` + jq） |
| `make test-check-deps` | 依存の向きの検査自体の回帰テスト |
| `make check-testdata` | repo の外への相対参照が 0 件であることの検査 |
| `make test-check-testdata` | 参照の検査自体の回帰テスト |
| `make check-dom-reads` | web crate が DOM の値を読まないことの検査（`tools/check-deps.sh`。例外は件数固定。crate 間の依存は `make check-deps`） |
| `make test-check-dom-reads` | DOM 読み取りの検査自体の回帰テスト |
| `make serve` | `dist/web` をサブパスの下で gzip つきで配る（`tools/serve.mjs`） |
| `make test-serve` | 配信サーバの試験（`node:test`） |
| `make test-browser` | ブラウザ試験の共通ヘルパ（`tools/browser/`）・最小の画面・keydown の属性の試験・対照の自己確認（SAFE-8。CSP あり・なし。Playwright。`npm ci` と `npx playwright install chromium` が前提。CI 未組み込み） |
| `make test-keydown-control` | `action-keydown` を外した写しで keydown の試験が正しい理由で失敗することの対照（遅く、CI・フックには入れない） |
| `make check-toolchain` | 固定ファイルの版と `docs/toolchain.md` の一致の検査 |
| `make test-check-toolchain` | ツールチェーンの一致の検査自体の回帰テスト |
| `make check-workflow-secrets` | `ci.yml` が secret を参照していないことの検査 |
| `make test-check-workflow-secrets` | secret 参照の検査自体の回帰テスト |
| `make build-web` | web を wasm にビルドして `dist/web` に配る一式を作る（`TEST_HOOKS=1` で試験の設定） |
| `make test-build-web` | `build-web` の受入基準（配信物に試験用の口が無い等）を実ビルドで検査する |
| `make check-mutations` | 変異注入の検査（誤りを入れると対応する試験が失敗することの確認。遅く、CI・フックには入れない） |
| `make test-check-mutations` | 変異注入の検査自体の回帰テスト |
| `make measure-wasm-size` | WASM の `gzip -9` の大きさを基点つきで測り JSON を出す（様式は `docs/measurement.md`） |
| `make test-measure-wasm-size` | 測定の様式の検証と実ビルドの回帰テスト（遅く、CI・フックには入れない） |
| `make measure` | 3 つの測定（WASM の gzip・初回表示・再描画）を基点つきで流し、束の JSON を出す（遅く、ブラウザを要する） |
| `make test-measure` | 一括測定の束の検証・fail-closed・実行の回帰テスト（遅く、ブラウザを要し、CI・フックには入れない） |

変更後は `make check` を通す。Rust を変えたら `make lint`・`make test`・`make check-deps` も通す。版の固定ファイルを変えたら `make check-toolchain` も通す。

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
| `README.md`・`CONTRIBUTING.md`・`CLAUDE.md`・`docs/architecture.md`・`docs/glossary.md` | `explorer` | `docs-writer` | `japanese-style.md` |
| `Cargo.toml`・`**/*.rs` | `explorer` | 未定義（着手時に `update-claude` で builder を追加） | `coding-rust.md`・`rust-build-profile.md` |
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
| `coding-rust.md` | `**/*.rs`・`**/Cargo.toml` | workspace lint・unsafe と `#[allow]` の理由・エラー処理・依存の版固定と向き・検証コマンド |
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
- 自動マージ（`autoMerge: true`）は、ruleset `main-protection` が PR HEAD の全チェックを必須にしていることを前提に
  成立する（`.claude/rules/github-actions.md` の「必須チェック」）。起動時は外部 App のチェックを宣言する

    ```json
    { "parent": 42, "repo": "Fandhe-AI/fandhe-workshop", "autoMerge": true,
      "externalChecks": [{ "app": "cursor", "context": "Cursor Bugbot" }] }
    ```

- CI のジョブを追加・改名・削除すると、必須チェックと HEAD のチェックがずれて自動マージが辞退する。ruleset を同じ
  タイミングで更新する

## main ブランチの保護

- `main` へ直接 push はできない（ruleset `main-protection`）。変更は必ずブランチを切り、PR を経由する
- マージは squash のみ。必須チェック 10 件の成功と、レビュースレッドの解決が条件（承認数は 0）
- ruleset・リポジトリ設定の内容は `.claude/rules/github-actions.md`、変更手順は `setup-repo-guards` スキル
