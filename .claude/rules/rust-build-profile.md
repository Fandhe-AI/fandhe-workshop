---
paths:
  - "Cargo.toml"
  - "**/Cargo.toml"
  - ".cargo/**"
  - "rust-toolchain"
  - "rust-toolchain.toml"
description: >
  Rust（Cargo）を導入・設定する際の方針。ルート Cargo.toml に依存クレートの debuginfo を行番号のみにする
  dev プロファイルを入れること（効く範囲・失うもの・ルートに書く理由・副作用）、README に置く
  cargo sweep による target/ の掃除手順と worktree ごとの肥大化への注意、[profile]・RUSTFLAGS・
  CARGO_PROFILE_* 等を変えるときの CI キャッシュのキーへの影響の確認を規定する。
---

# Rust ビルド設定（Cargo）編集ルール

## 目的

本リポジトリは現時点で Rust を含まないが、Rust を導入したとき（ルートの `Cargo.toml`・`.cargo/`・
`rust-toolchain.toml` を作成・編集したとき）に本ルールが読み込まれる。

Rust の `target/` は依存クレートの debuginfo で膨らみ、組織内の他リポではローカルの `target/` が 20GB を超えて
ディスクを圧迫した（2026-10-11 時点で fandhe-db 25GB・rust-ai-library 23GB）。また、ビルド設定の変更は CI の
`target/` キャッシュのキーに入らないことがあり、気付かないまま依存を毎回全再ビルドし続ける。
Rust を導入するときは次の 3 点に従う。

## ルート `Cargo.toml` の dev プロファイル

ワークスペースのルートの `Cargo.toml` に、次の設定をコメントごと入れる（なぜ入っているかを `Cargo.toml` だけで読めるようにする）。

```toml
# 依存クレート（workspace メンバー以外）のデバッグ情報を行番号のみにする。
# panic 位置・バックトレース・プロファイラの行番号は残り、失うのはデバッガで依存の
# 変数・型を見る機能だけ。自分たちのクレートは full debuginfo のまま。
[profile.dev.package."*"]
debug = "line-tables-only"
```

- **効く範囲**: `"*"` は workspace メンバー以外（依存クレート）にだけ効く。`dev` を継承する `[profile.test]` と
  `inherits = "dev"` のカスタムプロファイルにも効く。`release`・`bench` は変わらない
- **失うもの**: デバッガ（lldb・gdb）で依存クレートの変数・型を見る機能だけ。依存の中をデバッガで追うときは、
  `[profile.dev.package.<crate>] debug = true` で対象のクレートだけ戻す（クレート名の指定は `"*"` より優先される）
- **ルートに書く理由**: プロファイルはワークスペースのルートの `Cargo.toml` の設定だけが使われ、メンバーの
  `Cargo.toml` に書いた `[profile]` は警告付きで無視される
- **副作用**: 依存のビルドスクリプト・proc-macro にも効き（`[profile.dev.build-override]` より優先される）、
  依存のコンパイル設定と `-C metadata` が変わる。入れた直後は依存が全再ビルドされ、CI の `target/` キャッシュは
  古い内容のまま使えなくなる（後述の「CI キャッシュへの影響」）
- `.claude/rules/github-actions.md` の「サイズを抑える」にある CI 限定の `CARGO_PROFILE_DEV_DEBUG=line-tables-only` は、
  メンバーを含む全クレートを行番号のみにする CI 向けの縮小策で、本設定（ローカル開発でもメンバーは full のまま）とは別物。併用してよい

## README の開発環境節: `target/` の掃除

README のセットアップ（開発環境）の節に、`target/` の掃除の段落を置く。例:

```markdown
`target/` は放置すると数十 GB になります。14 日以上使っていないビルド成果物は
`cargo sweep --time 14` で削除してください（導入: `cargo install cargo-sweep`）。
worktree はそれぞれ別の `target/` を持つため、不要になった worktree は `git worktree remove` で
`target/` ごと消し、残している worktree は `cargo sweep -r --time 14 .claude/worktrees` でまとめて掃除します。
```

- `cargo sweep --time 14` は組織内で 1 回に 9.5 GiB 減らした実績がある（2026-10-11、fandhe-db）
- worktree（`implement-issue-tree` 等が `.claude/worktrees/` に作るもの）は、それぞれが本体と同じ規模の `target/` を持ち、
  肥大化の原因になる。`cargo sweep -r` は `.` で始まるディレクトリを辿らないため、`.claude/worktrees` はパスを明示して渡す
- worktree の配置先が `.claude/worktrees/` 以外なら、例の該当パスを実際の配置先に合わせる

## CI キャッシュへの影響

次のいずれかを変えると、CI の `target/` キャッシュのキーが変わらないまま中身が使えなくなることがある。

- ルートの `Cargo.toml` の `[profile]`（上の dev プロファイルの導入・変更を含む）
- `RUSTFLAGS`・`CARGO_PROFILE_*` 等の env
- `.cargo/config.toml`・`rust-toolchain.toml`
- CI のビルドコマンド・フラグ

変更する PR では、CI のキャッシュの方式ごとに対処が必要かを確かめ、PR 本文に結果（対処した内容、または不要な理由）を書く。
方式ごとの判断と対処（版セグメントの引き上げ・`Swatinem/rust-cache` の `prefix-key`・`rust-base-ci.yml` のマージ後の削除）は
`.claude/rules/github-actions.md` の「キャッシュ方針」→「キーの設計」に従う。
self-hosted runner の永続 `CARGO_TARGET_DIR` を使い GitHub のキャッシュを使わないジョブは、依存を 1 回再ビルドするだけで対処は要らない。
