---
description: >
  調査・設計フェーズの委譲原則。main の役割、subagent へ委譲する基準、パスごとの調査先と適用ルールの
  切り替え表、編集しないパスを規定する。作成・編集フェーズの委譲先は delegation-impl.md の担当。
---

# 委譲ルール（調査・設計）

## 目的

main のコンテキストは、ユーザーとの対話・計画・判断のために残す。ファイルの読み込みやログは subagent 側で
消費させ、main には結論だけを戻す。

## main の役割

- ユーザーの意図の確認、計画の作成、承認の取得
- subagent への依頼の作成と、戻ってきた結果の統合・検証
- コミット・PR 作成など、ユーザーの承認が要る操作の実行

## 委譲する基準

| 状況 | 対応 |
|---|---|
| 場所が分かっている 1〜2 ファイルの確認 | main が直接読む |
| 複数ファイル・複数ディレクトリにまたがる調査 | `explorer` へ委譲する |
| 外部ツール・言語・サービスの仕様確認 | `reference-researcher` へ委譲する |
| 独立した複数の調査 | 1 回の応答で並列に委譲する |
| 複数領域にまたがる設計判断 | main が行う（必要なら最上位 tier の model を使う） |

委譲した調査を main が重ねて実行しない。結果を待って統合する。

## 依頼に含めること

subagent は依頼文に書かれたことしか知らない。次を必ず渡す。

- 目的と、知りたい結論の形（一覧・有無・比較など）
- 対象のパスと、既に分かっていること・除外済みのこと
- 読むべきルール（下表）
- 変更の可否（調査は読み取り専用）

## パスベース切り替え表

| パス | 調査 | 適用ルール |
|---|---|---|
| `Makefile`・`scripts/*.sh` | `explorer` | `makefile.md`・`coding-shell.md` |
| `lefthook.yml`・`scripts/hooks/**` | `explorer` | `git-hooks.md`・`coding-shell.md` |
| `.github/workflows/**` | `explorer` | `github-actions.md` |
| `.claude/rules/**`・`.claude/agents/**`・`.claude/settings.json`・`.mcp.json` | `explorer` | 本ファイル・`delegation-impl.md` |
| `Cargo.toml`・`.cargo/**`・`rust-toolchain.toml`（未導入） | `explorer` | `rust-build-profile.md` |
| `docs/spec/**` | `explorer` | 読み取りのみ（下記） |
| 外部仕様（Rust・fandhe-frontend・Playwright・GitHub 等） | `reference-researcher` | `.claude/skills/` のリファレンススキルを先に引く |

## 編集しないパス

| パス | 理由 | 変更したい場合 |
|---|---|---|
| `.agents/skills/**`・`.claude/skills/**` | `skills-lock.json` で管理する外部取得物（`update-external.yml` が日次で追従する） | `contribute-skill` で upstream へ送る |
| `docs/spec/**` | private の submodule（`Fandhe-AI/fandhe-workshop-spec`） | spec リポジトリ側で変更する |
| `.env` | 秘密情報を含むローカルファイル。読み取りもしない | キー名の追加は `.env.example` に行う |
| `LICENSE-MIT`・`LICENSE-APACHE` | ライセンス本文 | ユーザーに確認する |

## 承認が要る操作

- `implement-issue` は、計画（`_/local-plans/`）をユーザーが承認してから実装する
- コミット・push・PR 作成・Issue 起票は、ユーザーが依頼した場合に行う
- 既存ファイルの削除・大きな書き換えは、対象を確認してから行う
