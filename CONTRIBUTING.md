# 貢献ガイド

fandhe-workshop への貢献を歓迎します。このリポジトリは public です。Issue・PR・CI のログはすべて公開されます。

仕様は private のリポジトリ（submodule の `docs/spec`）にあり、外部の方は読めません。
Issue と PR の本文だけで、何を・なぜ変えるのかが伝わるように書いてください。

## 開発の流れ

1. 関連する Issue を確認します。無ければ、変更の目的を書いた Issue を作ります
2. `main` からブランチを作成します。`main` へ直接 push はできません
3. 変更をコミットし、PR を作成します
4. 必須チェックがすべて成功し、レビューのスレッドがすべて解決されたらマージされます。マージ方式は squash のみです

CI の内容は [README](./README.md) の「CI」を参照してください。

## 環境の準備

[README](./README.md) の「必要なツール」と「セットアップ」に従ってください。

```bash
make doctor   # 必要なツールが揃っているか確認する
make setup    # Git hooks を有効化する
```

## 変更前の検査

```bash
make check        # editorconfig-checker + shellcheck
make test-hooks   # scripts/hooks/ を変更したときに実行する
```

`fandhe-frontend` の版を上げる場合は、`docs/testing.md` の 6.6.2 の手順に従ってください（受け入れ確認 (1)〜(5) は版上げ PR の CI が確かめます）。

## コミットと PR タイトル

コミットの件名と PR タイトルは、Conventional Commits の形式で書きます。

```text
<type>[(<scope>)][!]: <要約>
```

- type は `feat` `fix` `docs` `style` `refactor` `perf` `test` `build` `ci` `chore` `revert` のいずれかです
- 要約は日本語で書き、文末に句点を付けません
- 形式の詳細は `.claude/rules/conventional-commits.md` を参照してください
- フックで拒否された場合は、原因を直して再コミットしてください。`--no-verify` や `LEFTHOOK=0` で回避しないでください

## 言語と文体

ドキュメントとコメントは日本語で書きます。スクリプトのエラー出力と usage は英語です。
文体と表記は `.claude/rules/japanese-style.md` を参照してください。
用語は [用語集](./docs/glossary.md) にまとめています。

## セキュリティ

- 秘密情報（トークン・鍵・パスワード）と `.env` をコミットしないでください
- 未修正の脆弱性の詳細を、公開の Issue や PR に書かないでください
- 脆弱性の報告用の窓口は、現時点で整備されていません。整備するまでは、詳細を伏せた Issue でメンテナーに連絡を求めてください。これは暫定の案内であり、GitHub の Private vulnerability reporting を有効にするか `SECURITY.md` を作った時点で置き換えます

## ライセンス

このリポジトリは MIT OR Apache-2.0 のデュアルライセンスです（[LICENSE-MIT](./LICENSE-MIT) / [LICENSE-APACHE](./LICENSE-APACHE)）。
特に明示しない限り、貢献として提出した内容は、同じ条件のデュアルライセンスで提供されたものとみなします。

## AI エージェントで作業する場合

エージェント向けの要約は [CLAUDE.md](./CLAUDE.md)、レビュー観点は [AGENTS.md](./AGENTS.md) にあります。
