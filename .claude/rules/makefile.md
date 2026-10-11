---
paths:
  - "Makefile"
  - "scripts/*.sh"
description: >
  Makefile・scripts/ を編集する際の方針。Make は scripts/*.sh を呼ぶだけの薄い入口とし、処理の定義は
  scripts/ に置く。ターゲットの追加・改名・削除の手順、スクリプトの書き方、各ターゲットの契約を規定する。
  scripts/hooks/** は git-hooks.md の担当。
---

# Makefile・scripts 編集ルール

詳細な設計指針は make スキル（`.claude/skills/make`）を参照する。本ファイルはこのリポジトリ固有の要点のみを定める。
`scripts/hooks/**`（lefthook から呼ばれるフックスクリプト）は `.claude/rules/git-hooks.md` が担当する。

## 基本方針

- Makefile は薄い入口。レシピは `@scripts/<name>.sh` の 1 行呼び出し、または言語ネイティブコマンドの 1 行呼び出しのみ
- 複数行シェル・`if` / `for`・`curl`・環境依存の処理は `scripts/` の bash スクリプトへ置く
- 中身の無いターゲット（`@true` 等）は置かない（呼び出し元が終了コードを信用してしまう）
- GNU Make 3.81（macOS 標準）互換を保つ。`.ONESHELL`・`.RECIPEPREFIX` 等の 3.82+ 専用機能は使わない
- 同じ処理を CI・lefthook・Makefile に二重定義しない。CI も `make <target>` か `scripts/*.sh` を呼ぶ
- 現在のターゲット: `help`（既定）・`doctor`・`setup`・`check`・`test-hooks`

## ターゲットの追加・改名・削除の手順

1. `.PHONY` に追加・更新する
2. ターゲット行末に `## 説明` コメントを付ける（`make help` はこのコメントから生成される）
3. `scripts/help.sh` の一覧を同じ変更で更新する（自動同期の仕組みは無い）
4. 呼び出し元を洗い出して更新する

   ```bash
   grep -rn "make " .github .claude/rules docs README.md CLAUDE.md AGENTS.md lefthook.yml scripts
   ```

   CI（`.github/workflows/ci.yml`）・lefthook・README のターゲット表が対象
5. `make help` と `make -n <target>` で確認し、`make check` を通す

既存のターゲット名・引数・環境変数・終了コードは呼び出し元が依存しているため、原則維持する。

## 新規スクリプトの規約

- 先頭は `#!/usr/bin/env bash` と `set -euo pipefail`、`scripts/lib.sh` を `source` して `reject_args` / `require_cmd` / `log` を使う
- 実行権限（100755）を付け、`shellcheck` を通す（`make check` が `scripts/**/*.sh` を検査する）
- エラー・usage の出力は英語（`error: ...` / `usage: ...`）、コメントは日本語（既存スクリプトに合わせる）
- 引数を取らないスクリプトは `reject_args` で未知の引数を終了コード 2 で拒否する
- ツールが無い場合は「未検査で成功」にせず失敗させる。導入は試みない（案内は `doctor.sh` / README）
- macOS 標準の bash 3.2 で動く書き方にする

## ターゲットの契約

- `check`: ソース・設定を変更しない（editorconfig-checker + shellcheck）。fmt の自動修正は別ターゲットに分ける
- `doctor`: 読み取りのみ。何も導入・修復しない
- `setup`: 再実行しても安全（既存の `.env` を上書きしない）
- `test-hooks`: 一時 git リポジトリ内で完結し、作業ツリーを変更しない
- 将来 `clean` を追加する場合は、成果物ディレクトリのみを消し、共有・ユーザー管理のディレクトリ（`.env`・`.git`・`docs/spec` 等）を消さない
