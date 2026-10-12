---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
description: >
  Rust のコードを書く・変える際の共通規約。ワークスペースの lint 設定（missing_docs・clippy の unsafe と
  #[allow] の理由）・unsafe_code の禁止・エラー処理・依存の版固定と lockfile・crate 間の依存の向き・
  整形と検証コマンドを規定する。ビルドプロファイルと target/ の扱いは rust-build-profile.md、
  コメントの形式は code-comment-style.md の担当で、重複する項目はそちらを正とする。
---

# Rust コーディング規約

`[profile]`・`target/` の掃除・CI キャッシュへの影響は `.claude/rules/rust-build-profile.md`、コメントの形式は
`.claude/rules/code-comment-style.md` が定める。本ファイルは、Rust のコード全般に共通する書き方だけを定める。
crate 構成と依存の向きの正は `docs/architecture.md`、レビュー観点は `AGENTS.md` の「エンジンの実装」。

## 基本形

- edition・version・license などは `[workspace.package]` から継承する（`version.workspace = true`）
- 各 crate の `Cargo.toml` に `[lints] workspace = true` を置き、ルートの `[workspace.lints]` に従わせる
  （lint の水準を crate ごとに緩めない）
- 依存は `[workspace.dependencies]` に版を宣言し、各 crate では `dep.workspace = true` で参照する。
  版は `1.0.229` のように具体的に書き（`*` や範囲の緩い指定にしない）、`Cargo.lock` をコミットして実際に使う版を固定する。
  CI・スクリプトでは `--locked` を付ける
- 公開 API を持つ crate・モジュールの入口に `//!`、公開項目に `///` を付ける（`missing_docs` が警告する）
- `unsafe` は原則書かない。バイナリ・ライブラリの入口に `#![forbid(unsafe_code)]` を置く。必要になった場合は
  理由を `// SAFETY:` に書く（`undocumented_unsafe_blocks` が警告する）
- `#[allow(...)]` には `reason = "..."` を付ける（`allow_attributes_without_reason` が警告する）。
  crate 単位・モジュール単位の広い抑止は避け、対象を最小にする

## エラー処理

- 本番コードで `unwrap()` / `expect()` / `panic!` を使わない。失敗しうる処理は `Result` で返す。
  例外として使う場合は、失敗しない理由をコメントに書く
- 教材データ・利用者入力は信頼できない入力として扱う。読み込み失敗は握りつぶさず、どこの何が不正かを
  エラーに含めて返す（`.claude/rules/security.md`）
- ライブラリ crate のエラーは型で表す。バイナリの入口でだけ終了コードと標準エラー出力へ変換する
- エラー・usage の出力は英語（`error: ...`）。文書・コメントは日本語（`.claude/rules/japanese-style.md`）

## 依存の向き

- crate 間の依存は `docs/architecture.md` の向きに従う。`make check-deps`
  （`scripts/check-crate-deps.sh`）が機械で検査する
- `core` は教材の種類や実行環境（ブラウザ・ファイル I/O）を知らない。エンジンは特定の教材の知識を持たない
- 新しい外部依存を足す前に、既存の依存や標準ライブラリで足りないかを確認する。足す場合は
  `[workspace.dependencies]` に理由が分かる形で追加し、wasm32 で動かす crate には wasm 非対応の依存を入れない

## 整形と検証

- 整形は rustfmt の既定に従う（`make fmt`）。設定を変えて既定から外さない
- clippy の警告は `-D warnings` でエラー扱いにする（`make lint`）。警告を残さない
- 変更後は `make lint`・`make test`・`make check-deps` を通す。`web` を含む変更は wasm32 向け clippy も
  `make lint` に含まれる
- テストは対象と同じ crate に置く。ファイルシステムやネットワークに依存するテストは、一時ディレクトリを使い、
  リポジトリの作業ツリーを書き換えない
