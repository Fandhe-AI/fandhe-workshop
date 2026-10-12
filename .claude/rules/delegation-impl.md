---
description: >
  作成・編集フェーズの委譲マッピング。パスごとの実装担当 Agent、main が直接編集してよい範囲、
  実装後の検証・レビューの流れ、実装依頼に含める内容を規定する。調査フェーズは delegation.md の担当。
---

# 委譲ルール（作成・編集）

調査フェーズの原則と、編集しないパスは `.claude/rules/delegation.md` を参照する。

## 実装の委譲マッピング

| パス | 実装 | 適用ルール |
|---|---|---|
| `Makefile`・`scripts/*.sh` | `infra-builder` | `makefile.md`・`coding-shell.md` |
| `lefthook.yml`・`scripts/hooks/**` | `infra-builder` | `git-hooks.md`・`coding-shell.md` |
| `.github/workflows/**` | `infra-builder` | `github-actions.md` |
| `.editorconfig`・`.editorconfig-checker.json`・`.shellcheckrc`・`.gitignore`・`.gitmodules` | `infra-builder` | 各ファイル冒頭のコメント |
| `.claude/rules/**`・`.claude/agents/**`・`.claude/settings.json`・`.mcp.json` | `dotclaude-author` | `security.md` |
| `README.md`・`CONTRIBUTING.md`・`CLAUDE.md`・`docs/architecture.md`・`docs/glossary.md` | `docs-writer` | `japanese-style.md` |
| `Cargo.toml`・`**/*.rs` などエンジンの実装（未導入） | 専用の builder が未定義（下記） | `rust-build-profile.md` |

### エンジンの実装を始めるとき

言語固有の実装は、実装着手の指示後に追加する（README）。着手時に `update-claude` を実行し、技術レイヤに合わせた
builder Agent（例: `rust-builder`）・コーディング規約（例: `coding-rust.md`）・`e2e-runner` を追加してから
委譲する。追加までの間は、main が計画を作り、汎用の subagent へ適用ルールを明示して委譲する。

## main が直接編集してよい範囲

- 1 ファイル・数行で、読むべき箇所が既に main のコンテキストにある修正
- ユーザーとの対話の中で、その場で直すよう求められた小さな修正

それ以外は委譲する。迷ったら委譲する。

## 実装依頼に含めること

- 目的と、完了の条件（何が動けば終わりか）
- 変更するファイルと、変更しないファイル
- 読むべきルール（上表）と、参照する計画（`_/local-plans/<name>.md`）
- 実行する検証コマンド
- コミット・push をするかどうか（指示が無ければしない）

## 実装後の流れ

1. **検証**: `test-runner` へ `make check` を依頼する。`scripts/hooks/`・`lefthook.yml` を変更した場合は
    `make test-hooks` も依頼する
2. **レビュー**: `reviewer` へ差分のレビューを依頼する（`implement-review`）
3. **セキュリティ**: PR 作成前に `security-auditor` へ監査を依頼する。要修正があれば PR を作らない
4. **ドキュメント**: 構成・コマンド・CI・ルール・Agent が変わった場合は `docs-writer` へ `README.md`・`CLAUDE.md`・`docs/architecture.md`
    の更新を依頼する（`update-docs`）
5. **コミット・PR**: main が `create-commit`・`create-pr` で行う（`conventional-commits.md`）

独立した実装は並列に委譲してよい。同じファイルを触る実装は直列にする。

## 結果の扱い

- subagent の「成功」は鵜呑みにせず、検証コマンドの結果と差分（`git diff --stat`）で確認する
- 失敗・未実行の検証は、そのままユーザーへ報告する
- 範囲外として報告された事項は `out-of-scope-tracking.md` に従って扱う
