# 教材パックの例

教材の作成者が、構造の書き方を手元で試すための最小の教材パック 2 つです（AUTHOR-12）。題材は、教材とは無関係の仮のものです。

| 例 | 進み方 | 示す書き方 |
|---|---|---|
| `minimal-pack/` | コマンド（`/hello`・`/swap`）で進み、最後の確認だけが画面操作 | 開始点・`put`・`remove` の状態変化・`point_at` の確認・誤入力の教材テスト |
| `choice-only-pack/` | 選択肢と画面操作だけで進む | `pack.profile`（入力欄なし）・`pack.appearance`・`pack.left_pane`・`answer` の選択肢の `label` |

## 読む順と試し方

1. `docs/schema-format-b.md` で欄・型・必須を確かめる
2. `minimal-pack/` を読み、`workshop.yaml`、`chapters/*/chapter.yaml`、`tests/playthrough.yaml` の順に見る
3. リポジトリのルートで、次を実行する（実行ファイルの用意は `docs/cli-guide.md` の 2 節）

```text
target/debug/fandhe-pack validate docs/examples/minimal-pack
target/debug/fandhe-pack test docs/examples/minimal-pack
```

どちらの例も、`validate` は診断 0 件、`test` は全件合格で終了コード 0 になります。この 2 点は、試験（`crates/pack/tests/examples_pack.rs`）が CI で守っています。

## 出所と、写した後に変えた点

出所は、仕様の PoC-5 の代替案 (a) の `public/examples/` です。そのまま通らなかったため、次の点だけを変えています。

| 例 | 変えた点 | 理由 |
|---|---|---|
| `minimal-pack` | 最後の Step の確認を `confirm: none` から `point_at` に替え、`views: [v-files]` を足した | `test` は、章の完了を「最後の入力が `achieved` であること」で判定する（`docs/cli.md` の 12.1 節の経過措置）。`confirm: none` の Step は `accepted` で終わり、完走にならない |
| `minimal-pack` | 教材テストの `asserts` を消し、末尾に `{ ui: "v-files shelf/b.dat", expect: achieved }` を足した | `asserts` は今の `test` が判定せず、定義があるだけで終了コード 1 になる。世界状態は教材テストでは確かめず、`point_at` が `shelf/b.dat` の存在を確かめる |
| `choice-only-pack` | `pack.vocab` のコメントを、`validate` が `E-VOCAB-MISMATCH` で検査する説明に直した | 以前の「検査されない」は事実と違う |
| `choice-only-pack` | `fixtures/note.md` の説明を、`body` を省略した場合の説明に直した | `views.<ID>.body: shown`（語彙 1.4 の EP-13。`docs/views.md`）で本文を出せる |

core が章の完了を結線すれば、`confirm: none` で終わる形と `asserts` が使える可能性があります。そのときは、これらの例を見直します。

## `choice-only-pack` の扱い

`choice-only-pack/` を残すか、削るか、変えるかは、TASK-738（SECOND-17）が決めます。決まるまで、構造は変えません。`MANIFEST.sha256` は、このディレクトリの全ファイルの SHA-256 です。ファイルを直したら作り直します（試験が食い違いを検出します）。

```text
cd docs/examples
find . -type f ! -name MANIFEST.sha256 | sed 's#^\./##' | LC_ALL=C sort | xargs sha256sum > MANIFEST.sha256
```
