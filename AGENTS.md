# AGENTS.md

## 文書の位置づけ

本リポジトリ（fandhe-workshop）で作業するすべての AI エージェント・開発者、および ai-review（provider: codex）による
PR 自動レビュー（`.github/workflows/ai-review.yml`。Fandhe-AI/actions の reusable workflow を呼び出す薄い wrapper）が
共通で用いる**レビュー観点集**である。ai-review の既定 prompt は、PR の base コミットにある本書をレビュー基準として
読み込む。本書の変更は、マージ後の次の PR から有効になる。

各観点の一次情報源は `CLAUDE.md`・`.claude/rules/`・`README.md`・各ファイル冒頭のコメントであり、本書は重複記載を
避けて要点と参照だけをまとめる。本書と一次情報源が食い違う場合は、一次情報源を正とする。

本リポジトリは public で、現時点の中身は開発基盤（Make・シェルスクリプト・lefthook・GitHub Actions）と Claude Code の
設定である。エンジンの実装（言語固有のコード）は未導入のため、実装コードに関する観点は導入後に適用する。

## 優先度の定義

| 優先度 | 意味 | 扱い |
|---|---|---|
| P0 | 秘密情報の漏えい・脆弱性・検査やゲートの無効化に直結する | 修正必須。`codex / review` が失敗し、マージできない |
| P1 | 設計原則・運用規約への違反 | 修正必須。P0 と同じく `codex / review` が失敗し、マージできない |
| P2 | 提案。可読性・保守性・文書の追随の改善 | 任意。ジョブは失敗しない |

- ai-review は P0・P1 の指摘が 1 件でもあるとジョブを失敗させる（`block-priorities` の既定。P0・P1 は対象から外せない）。
  したがって P1 は「見送ってよい推奨」ではなく、**マージ前に解消が必要な指摘**として付ける
- P0 と P1 の違いは影響の性質であり、マージ可否ではない。P0 は公開リポジトリでの実害（漏えい・未検査のマージ）に
  直結するもの、P1 は規約違反で、放置すると基盤の一貫性が崩れるものに付ける
- 指摘が誤りだと判断した場合は、根拠が読み取れるようにコード・コメント・一次情報源を直して再レビューを受ける。
  ゲートの設定を弱めて通さない
- 好みの問題や、規約に根拠の無い改善案には P2 を付ける。ここに列挙の無い一般的な品質問題は、既定の重要度判断に従う

## 観点 1: セキュリティ

一次情報源は `.claude/rules/security.md`・`.claude/rules/git-hooks.md`・`.claude/rules/github-actions.md`。

### 秘密情報

- トークン・鍵・パスワード・認証情報入り URL の、コード・設定・ルール・文書・テストへの混入: **P0**
- `.env` 系ファイルの追加（雛形の `.env.example` を除く）、`.gitignore` の `.env` 除外の削除: **P0**
- `.mcp.json`・`.claude/settings.json` への値の直書き（環境変数参照にしていない）: **P0**
- `.env.example` への実値らしき値の記載（キー名と空値・ダミー値以外）: **P1**
- 検出した秘密情報の値そのものを、ログ・エラー出力・PR 本文へ出す実装: **P0**

### 検査・ゲートの無効化（fail-open）

- `scripts/hooks/secret-scan.sh`・`scripts/hooks/commit-msg-check.sh` で、検査自体の失敗を「検出なし」として扱う変更
  （終了状態 2 以上の握りつぶし・`|| true`・冒頭の自己テストの削除・`LC_ALL=C` の削除・`git diff` の
  `--no-color --no-ext-diff --no-textconv` 等の削除）: **P0**
- `ci.yml` の `ci-complete` の弱体化（`needs` からのジョブの除外・`skipped` を許容する条件の拡大・`if: always()` の
  削除・`continue-on-error` の追加）: **P0**
- ジョブを追加したのに `ci-complete` の `needs` へ追加していない（未検査のままマージ可能になる）: **P0**
- `--no-verify`・`LEFTHOOK=0` を使う手順・スクリプト・hook の追加: **P0**
- `ai-review.yml` の `skip-branch-prefixes` の拡大、接頭辞以外の条件によるレビューの skip の新設: **P0**
- `scripts/hooks/test-hooks.sh` の検出ケースの削除・期待値の弱体化: **P1**
- ツールが無い場合に未検査のまま成功させる変更（`require_cmd` の削除等）: **P1**

### GitHub Actions

- PR タイトル・ブランチ名・Issue 本文など利用者が決められる値を、`run:` へ `${{ }}` で直接展開する: **P0**
- `pull_request_target` の導入、fork PR のコードが self-hosted runner で動く経路の追加: **P0**
- self-hosted runner の新規利用（`ai-review.yml` の `codex / review` は既存の例外）: **P1**
- `permissions` の過剰な付与、ワークフローのトップレベル `permissions` の削除、`actions/checkout` の
  `persist-credentials: false` の削除: **P1**
- サードパーティ action のコミット SHA 固定の解除、ツール導入の版固定・SHA256 検証の削除: **P1**
- `npx` で実行するパッケージの版を exact で固定していない（`.mcp.json` 等）: **P1**

### シェル

- 利用者が決められる値・外部から取得した文字列を、`eval` やクォート無しの展開でコマンドに渡す: **P0**
- 上記に当たらない未クォートの変数展開・`eval` の使用: **P1**

### エンジンの実装（導入後に適用）

教材データは、エンジンから見て信頼できない入力として扱う。

- 教材データ・利用者入力を、エスケープせずに HTML・シェル・クエリへ埋め込む: **P0**
- 教材データ中のパスで、意図した範囲の外を読み書きできる（パストラバーサル）: **P0**
- 教材データ・利用者入力から組み立てた URL を、宛先の制限なく取得する: **P1**
- 依存の版を固定していない・lockfile をコミットしていない: **P1**

## 観点 2: アーキテクチャ・設計整合

一次情報源は `CLAUDE.md`・`.claude/rules/makefile.md`・`git-hooks.md`・`github-actions.md`・`coding-shell.md`・
`rust-build-profile.md`。

### エンジンと教材の分離

- エンジンは特定の教材の知識を持たない。特定の教材の章・Step・文言・判定条件を、エンジン側にハードコードする: **P1**
- 仕様（`docs/spec/`。private の submodule）と食い違う実装を、理由の説明なしに入れる: **P1**

### 入口は複数でも、処理の定義は一つ

- `Makefile` のレシピに複数行のシェル・`if` / `for`・ロジックを書く（`scripts/*.sh` の 1 行呼び出しにしていない）: **P1**
- 中身の無いターゲット（`@true` 等）の追加: **P1**
- 同じ検査を CI・lefthook・Makefile に重ねて定義する（CI が `make <target>` / `scripts/*.sh` を呼ばず独自に実装する）: **P1**
- GNU Make 3.82 以降でしか動かない機能（`.ONESHELL`・`.RECIPEPREFIX` 等）の使用: **P1**

### ターゲットの契約

- `check` がソース・設定を変更する、`doctor` が導入・修復を行う、`setup` が既存の `.env` を上書きする、
  `test-hooks` が作業ツリーを変更する: **P1**
- 既存のターゲット名・引数・終了コードを、呼び出し元（`ci.yml`・`lefthook.yml`・README）を更新せずに変える: **P1**

### Git hooks

- テスト・ビルド・リポジトリ全体の lint など、staged 限定で 1 秒未満に収まらない検査をフックへ追加する: **P1**
- `pre-commit` の `parallel: true` の解除、`{staged_files}`・`glob`・`exclude` による対象の絞り込みの削除: **P1**
- `scripts/hooks/` を変更したのに、`scripts/hooks/test-hooks.sh` に回帰ケースを追加していない: **P1**

### CI

- 既存ワークフローへのジョブ追加で済む内容に、新しいワークフローを追加する: **P1**
- 必須チェックを含むワークフローに、ワークフロー単位の `paths` / `paths-ignore` を付ける
  （未実行のチェックが pending のまま残り、マージできなくなる）: **P1**
- ジョブの `name:` の変更・ジョブの追加と削除を、ruleset の必須チェックとの整合に触れずに行う: **P1**
- キャッシュ方針への違反（PR からの保存・再帰 glob のキー・効果の実測なし）: **P1**
- `timeout-minutes` の無いジョブ、PR との二重実行になる発火条件（全ブランチへの `push`）: **P1**

### シェルスクリプト

- macOS 標準の bash 3.2 で動かない書き方（`mapfile`・連想配列・`${var,,}`・空配列の `"${arr[@]}"` 等）: **P1**
- `#!/usr/bin/env bash` と `set -euo pipefail` の欠落、`scripts/lib.sh` の `reject_args`・`require_cmd` を使わない
  新規スクリプト: **P1**
- 理由の書かれていない `# shellcheck disable`、ファイル単位・リポジトリ単位の抑止の追加: **P1**

### 編集しないパス

- `.agents/skills/**`・`.claude/skills/**` の手編集（`skills-lock.json` の更新を伴わない変更）: **P1**
- `docs/spec/` の参照先を、理由の説明なしに変える・`update = none` を外す: **P1**

### Claude Code の設定

- Agent の `model:` に、`CLAUDE.md` の model 配分表に無い alias を書く: **P1**
- 調査・レビュー・検証用の Agent（`explorer`・`reference-researcher`・`reviewer`・`security-auditor`・`test-runner`）へ
  `Edit`・`Write` を与える: **P1**
- 特定のパスにしか関係しないルールを、`paths:` を付けずに常時読み込みにする: **P2**

### Rust の導入時

- `.claude/rules/rust-build-profile.md` に反する導入（ルートの `Cargo.toml` に dev プロファイルが無い・ビルド設定を
  変えたのに CI キャッシュのキーへの影響を確認していない）: **P1**

## 観点 3: 再利用・アセット化

本リポジトリの開発基盤は `Fandhe-AI/template-dev` から作成しており、エンジンは教材から独立した汎用 OSS として
配布する。どちらも「他へ持ち出せること」を前提に確認する。

- スクリプトに、リポジトリ名・絶対パス・個人の環境に依存する値をハードコードする（`scripts/lib.sh` の `ROOT_DIR` を
  使わず cwd に依存する実装を含む）: **P1**
- 教材データの形式（スキーマ）を変える変更で、仕様・文書・サンプルの更新が伴っていない: **P1**
- エンジンの汎用部分と、教材ごとに差し替える拡張点が分かれていない（導入後に適用）: **P2**
- 版・SHA256 などの値を複数箇所に重ねて定義する（`ci.yml` の `env` のような単一の宣言点を崩す）: **P2**
- テンプレートへ還元できる汎用的な改善に、本リポジトリ固有の事情を混ぜている: **P2**
- 構成・コマンド・CI・ルール・Agent を変えたのに、`README.md`・`CLAUDE.md`・`scripts/help.sh` が追随していない: **P2**
- ファイル冒頭のコメントに、役割・呼び出し元・終了コードが無い（`.claude/rules/code-comment-style.md`）: **P2**

## リポジトリ固有の観点

- **Conventional Commits（日本語）**: コミット件名・PR タイトルは `<type>[(<scope>)][!]: <日本語の要約>`
  （`.claude/rules/conventional-commits.md`）。形式は CI が検証するため、指摘するのは type の選び方と
  1 コミット 1 目的の逸脱に限る: **P2**
- **日本語スタイル**: 文書・コメントは日本語、スクリプトのエラー・usage の出力は英語
  （`.claude/rules/japanese-style.md`）。相対的な日付（「昨日」等）を文書に残さない: **P2**
- **計測値の日付**: 時間予算・棚卸し結果などの計測値を、計測日なしで書く・変える: **P2**
- **スコープ外の混入**: PR の目的と無関係な修正を混ぜる（`.claude/rules/out-of-scope-tracking.md`）: **P2**

## 運用上の注記

- `.editorconfig` への準拠・shellcheck の警告・コミット件名と PR タイトルの形式・フックスクリプトの回帰テストは、
  既存の CI（`ci.yml`）が機械判定する。本レビューで重ねて指摘しなくてよい
- 本書の基準を弱める変更（P0・P1 の項目の削除・降格）は、それ自体を P1 として指摘し、一次情報源
  （`.claude/rules/` 等）の変更と整合しているかを確認する
- `update-external.yml` が作る日次の同期 PR（ブランチ名が `chore/skills-update-`・`chore/submodule-update-` で始まる）は、
  上流の取り込みそのものでありこの PR では直せないため、ai-review の対象から外している
- エンジンの実装言語を導入する PR では、コーディング規約（`.claude/rules/coding-<lang>.md`）の追加と同時に、
  本書の「エンジンの実装」の項目を、その言語・構成に合わせて具体化する

## Fandhe-AI/actions の参照方式（`@latest`）

`Fandhe-AI/actions`（組織内の上流リポジトリ）の reusable workflow は、可変タグ `@latest` で参照する
（`.claude/rules/github-actions.md` のジョブ追加チェックリスト）。本書の「コミット SHA 固定」の観点は
**サードパーティの action にだけ**適用し、`Fandhe-AI/actions` への `@latest` 参照を指摘しない。
