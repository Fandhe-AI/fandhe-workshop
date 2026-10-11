---
paths:
  - "**/*.sh"
  - ".shellcheckrc"
description: >
  シェルスクリプト（bash）を書く・変える際の共通規約。bash 3.2 互換・クォート・エラー処理・shellcheck の
  扱い・出力の書き分けを規定する。scripts/*.sh 固有の規約は makefile.md、scripts/hooks/** 固有の規約
  （fail-closed・git diff の扱い等）は git-hooks.md の担当で、重複する項目はそちらを正とする。
---

# シェルスクリプト コーディング規約

`scripts/*.sh` の構成・ターゲットの契約は `.claude/rules/makefile.md`、フックスクリプトの書き方は
`.claude/rules/git-hooks.md` が定める。本ファイルは、どのシェルスクリプトにも共通する書き方だけを定める。

## 基本形

- 先頭は `#!/usr/bin/env bash` と `set -euo pipefail`
- 冒頭コメントに、役割・呼び出し元・終了コードを書く（`.claude/rules/code-comment-style.md`）
- 実行権限（100755）を付ける
- インデントは 2 スペース（`.editorconfig`）。整形ツール（shfmt）は採用していない

## bash 3.2 互換

macOS 標準の bash 3.2 でも動く書き方にする（CI の `test-hooks-macos` が macOS で検証する）。

- 使わない: `mapfile` / `readarray`・連想配列（`declare -A`）・`${var,,}` / `${var^^}`・`&>>`・`coproc`
- 空配列の `"${arr[@]}"` は `set -u` で未定義扱いになるため、要素数を確認してから展開する
- GNU 固有のオプション（`sed -i` の引数なし・`grep -P`・`readlink -f`・`date -d` 等）に依存しない

## 変数とクォート

- 変数展開は `"${var}"` の形でクォートする（既存スクリプトに合わせる）
- 関数内の変数は `local` で宣言する。`local var="$(cmd)"` は終了状態を隠すため、宣言と代入を分ける
- `eval` は使わない。外部から来た文字列をコマンドとして組み立てない
- パスを扱う一覧は NUL 区切り（`find -print0 | xargs -0`・`git ... -z`）にする
- スクリプトの位置は `"$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"` で解決し、cwd に依存しない

## エラー処理

- コマンドの失敗を `|| true` で握りつぶさない。許容する場合は、理由をコメントに書く
- パイプの左辺・`$(...)` に置いたコマンドの失敗は見えなくなる。結果が判定に効く場合は、一時ファイルか変数へ
  受けて終了状態を確認する
- 誤用（未知の引数等）は終了コード 2、検査の失敗は 1 とし、冒頭コメントに終了コードの意味を書く
- 必要なツールが無い場合は、未検査のまま成功にせず失敗させる。導入は試みない

## 出力

- エラー・usage は英語で標準エラーへ出す（`error: ...` / `usage: ...`）。コメントは日本語で書く
- 進行状況は `log`（`scripts/lib.sh`）で標準エラーへ出し、標準出力は結果のために空けておく
- 秘密情報の値を出力しない（検出時はファイル名だけを出す）

## shellcheck

- `make check` が `scripts/**/*.sh` を、pre-commit が staged の `*.sh` を検査する。警告を残さない
- 抑止する場合は対象行の直前に置き、理由を同じ行に書く: `# shellcheck disable=SC2034 # source した側で参照する`
- `source` 先は `# shellcheck source=./lib.sh` で示す（`.shellcheckrc` の `source-path=SCRIPTDIR` で解決される）
- ファイル単位・リポジトリ単位の抑止は追加しない
