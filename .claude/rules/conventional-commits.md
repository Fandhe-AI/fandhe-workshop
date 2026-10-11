---
description: >
  Conventional Commits の詳細規約。コミット件名と PR タイトルの形式・type の選び方・scope・破壊的変更の
  表記・検証の仕組み（commit-msg フックと CI の pr-title ジョブ）を規定する。形式の正は
  scripts/hooks/commit-msg-check.sh。
---

# Conventional Commits 規約

## 形式

```text
<type>[(<scope>)][!]: <要約>
```

```text
feat(auth): ソーシャルログイン機能を追加
feat(api)!: レスポンス形式を変更
docs(rules): GitHub Actions のキャッシュ方針を追加
chore: 依存を更新
```

- 形式の正は `scripts/hooks/commit-msg-check.sh` の正規表現。本ファイルと食い違う場合はスクリプトに従う
- 要約は日本語で書く。`: ` の後ろに半角スペースを 1 つ置き、文末に句点を付けない
- **PR タイトルも同じ形式にする**（squash merge でコミット件名になる）

## type

次の 11 種類のみ使える。

| type | 使う場面 |
|---|---|
| `feat` | 機能の追加 |
| `fix` | 不具合の修正 |
| `docs` | ドキュメント・ルール（`README.md`・`CLAUDE.md`・`.claude/rules/`）だけの変更 |
| `style` | 動作を変えない整形（空白・改行等） |
| `refactor` | 動作を変えない構造の変更 |
| `perf` | 性能の改善 |
| `test` | テストの追加・修正 |
| `build` | ビルド・依存に関わる変更 |
| `ci` | `.github/workflows/` など CI の変更 |
| `chore` | 上記に当たらない雑務（スキルの追加・設定の更新等） |
| `revert` | 変更の取り消し |

迷ったら、利用者から見た変化で選ぶ（機能が増えるなら `feat`、壊れていたものが直るなら `fix`）。

## scope

- 英小文字・数字・ハイフンのみ（`[a-z0-9-]+`）。例: `rules`・`hooks`・`ci`・`init-claude`
- 複数の領域にまたがる変更では省略してよい
- 大文字・スラッシュ・アンダースコア・カンマは使えない（フックが拒否する）

## 破壊的変更

`:` の直前に `!` を付ける（`feat(api)!: ...`）。本文に、何が変わるか・移行方法を書く。

## 本文

- 件名の後に空行を 1 つ置く
- 「何を」より「なぜ」を書く。差分を読めば分かる内容を繰り返さない
- 関連する Issue は `Closes #123`・`Refs #123` で示す

## 1 コミットの単位

- 1 つの目的に絞る。無関係な変更は別コミット・別 PR に分ける（`out-of-scope-tracking.md`）
- 整形だけの変更と、動作を変える変更を混ぜない

## 検証の仕組み

| 場面 | 検証 |
|---|---|
| ローカルのコミット | lefthook の commit-msg が `scripts/hooks/commit-msg-check.sh` を実行する（`make setup` 済みの環境） |
| PR | CI の `pr-title` ジョブが、PR タイトルを同じスクリプトで検証する |

- `Merge ...`・`Revert ...`（git の既定文言）・`fixup! ...`・`squash! ...` は検証を素通りする
- フックで拒否された場合は、メッセージを直して再コミットする。`--no-verify`・`LEFTHOOK=0` で回避しない
- type の追加など形式を変える場合は、スクリプト・`scripts/hooks/test-hooks.sh` の回帰ケース・本ファイルを
  同じ変更で更新する

## 作成手順

コミットは `create-commit`、PR は `create-pr` スキルで作成する。どちらもユーザーが依頼した場合に行う。
