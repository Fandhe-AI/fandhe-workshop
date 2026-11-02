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

2026-11-02 に、D-92（TASK-738。SECOND-17）で `choice-only-pack/` を残すと決めました。手戻りが 0 であること、汎用性の中心の主張（SECOND-1 のエンジンの差分 0）には例の有無が効かないこと、似ている点の一部は形式 B と語彙が決めるため題材を離しても似かたは 0 にならないことが理由です。例の教材ファイルは変えていません。

### 第 2 教材と似ている点

この例と第 2 教材は、仕様の PoC-5 の代替案 (a) の比較（SECOND-2）で、次の点が同じです。

- `pack` の欄の並び（`id`・`title`・`version`・`ir_format`・`guidance`・`vocab`・`profile`・`appearance`・`left_pane`）と、`ir_format`・`guidance`・`vocab` の値
- 外観と左の画面の値（`appearance.theme: reading`・`left_pane.world_list: hidden`・`view_tabs: hidden`）
- 拒否の文面の ID の付け方（8 分類それぞれに `rj-unknown` などの同じ ID を付け、`defaults` の対応も同じ）

Step・規則・選択肢の ID とファイル名は重なりません。

### 形式 B と語彙が決める部分

| 似ている点 | 形式 B と語彙が決める部分 | 決めない部分（作者の選択） |
|---|---|---|
| `pack` の欄 | 欄の名前・型・必須（`docs/schema-format-b.md` の 3.1 節。未知の欄は `E-PARSE`）。`profile`・`appearance`・`left_pane` を使うと `vocab` は 1.3 以上の宣言が要る（`E-VOCAB-MISMATCH`） | 並びの順（読み込みは順序を見ない。3.1 節の表の順に書くと同じ並びになる）。省略できる欄を書くか |
| 外観と左の画面 | 値の候補（`docs/operation-profile.md`・`docs/pack-authoring.md`） | 候補からどれを選ぶか。`reading`・`hidden` は、選んだ値が同じ |
| 拒否の ID | 分類の 8 語と、`defaults` が分類から文面の ID への表であること | ID の文字列（`rj-*`）の付け方と、8 分類すべてに文面を書くか（網羅は検査しない） |

形式が決めるのは名前と値の集合までで、同じ値を選んだかどうかは作者の選択です。

### 限界

似ている点の一部は形式と語彙が決めるため、題材を離しても似かたは 0 にならません。一方で、ID の付け方・値の選び方・並びの順は、この例に倣った可能性があります。例が第 2 教材の成否に効いた可能性は否定できません。この限界を受け入れて、例を残しました（D-92）。

### 画面での限界

この例は `validate`・`test` は通りますが、画面ではそのままでは最後の Step まで進みません（`world_list: hidden` が行を作らない PACK-22 と、画面が開始点の `effects` を適用しない経過措置のため。記録は `docs/testing.md` の 5.436）。例の構造は変えていません。

### 全ファイルの SHA-256

`MANIFEST.sha256` は、このディレクトリの全ファイルの SHA-256 です。ファイルを直したら作り直します（試験が食い違いを検出します）。

```text
cd docs/examples
find . -type f ! -name MANIFEST.sha256 | sed 's#^\./##' | LC_ALL=C sort | xargs sha256sum > MANIFEST.sha256
```
