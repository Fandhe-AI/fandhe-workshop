# ビュー（左エリアの成果物の見せ方）

画面の左エリアは、教材の世界状態（ファイル・文書・カードなど）が操作に応じて変わる様子を見せます。この文書は、教材の作成者が、ビューを宣言し、左エリアの見え方を絞り、画面上の要素を「指す」入力の宛先を書くために知っておくことをまとめます（AUTHOR-12）。

- 文書の版: 5
- 確認した日: 2026-11-05
- 仕様は private のため、本文を転記せず ID（ART-2・ART-3・ART-5・ART-11・ART-12・PACK-22・PACK-38・PACK-48・D-49・D-50・D-51・D-64・D-100）で参照します
- 書き方の詳細は `docs/pack-authoring.md`（「左エリア」「文書のビューの本文」）が正です。この文書は、ビューの種類と、画面の実装の進み具合を加えて整理します

## 1. 先に知っておくこと（2026-11-05 時点の実装の状態）

ビューの種類ごとの描画部品は、7 種とも `crates/web/src/` に実装があり、検査器の代役の部品（`view_fixture`）で試験されています。製品の画面（`engine_parts::ArtifactPart::view`）は、2026-11-05 までに 7 種すべてを種類ごとの描画部品へ結線しました（#2062〜#2068）。確認できた事実は次のとおりです。

| 項目 | 状態 | 根拠 |
|---|---|---|
| 7 種の綴りと、Step が宣言したビューから種類を決める規則 | 実装済み | `crates/web/src/artifact_view.rs`・`view_select.rs`・`crates/contract/src/view_kind.rs`（綴りの正） |
| 綴りの検査（`E-VIEW-KIND-UNKNOWN`） | 実装済み（2026-11-03） | `crates/pack/src/view_kind.rs`・`crates/contract/src/view_kind.rs`・`crates/pack/tests/view_kind.rs` |
| 7 種ごとの描画部品（ファイルツリー・文書・差分・タスクツリー・カード・チェック・アプリのデモ） | 結線済み（2026-11-05。#2062〜#2068）。製品は後の状態（`Phase::After`）の変種 (b) だけを出す | `engine_parts.rs` の `ArtifactPart::view`（種類の `match`）、`crates/web/src/product_rows.rs`（振り分け）、各 `*_view.rs` の `product_items`、`crates/web/tests/product_rows.rs` と `tests/data/product_view_html/*.html`、`tools/browser/choice-journey/review-request-*.json`・`task-breakdown*.json` |
| 製品の画面が見せる左エリア | 種類ごとの行（読み方は 4 節の仮置き）。ファイルツリーは `dir`・`file` のラベルと補足、文書は題・状態・補足の 3 つの文字（`body: shown` のときは練習データの本文の区画も）、差分は行番号・記号つきの `pre`、タスクツリーはラベル・補足・状態のバッジ、カードはラベル・状態のバッジ・補足、チェックは記号つきの状態、アプリのデモは `kind` ごとのワイヤー。木の種類（ファイルツリー・タスクツリー）は入れ子の属性も付く。行の検査に通らない行は、経路だけの行（`path_rows`）に退く（fail-closed）。前後の切り替え（ART-2）の口は無く、変種 (a) は製品から呼ばれない。変化の印は行単位のまま（ART-17） | `engine_parts.rs`・`product_rows.rs`・`document_element_rows.rs`・`task_tree_product.rs`・`card_list_product.rs`・`file_tree_product.rs`・`diff_product.rs`・`check_list_product.rs`・`app_demo_product.rs`（いずれも `crates/web/tests/`） |
| 選択のボタン（ビューの切り替え） | `view_tabs` に従う（5 節）。既定の `all` は 7 種すべてを出して選べる | `view_select.rs`・`engine_parts.rs`（2026-10-25。TASK-481 1/2） |
| `pack.left_pane` の画面への結線 | 結線済み（2026-10-25）。ホストが設定を読み、読めない設定は既定（`shown`・`all`）で動く | `host.rs`・`crates/web/tests/left_pane_view.rs`・`tools/browser/left-pane.test.mjs` |
| 人が画面で見て確かめた件数 | 0 件 | 未確認（結線の後の画面の確認は #2060 で行う） |

この文書の 2 節以降の「画面での見え方」は、契約と部品の実装に基づく説明で、結線の後の製品の画面で人が確かめたものではありません。教材の書き方（宣言・検査）は結線の前後で変わりません。

文書のビューは、他の 6 種と読み方が違います。製品は、世界状態の要素ごとの行（`document_view.rs` の `doc_element_item`）に題・状態・補足と本文の区画を出します。ブロックを行にする部品（`doc_item`・`doc_items`・変種 (a)）は製品から使いません。ブロックの行の識別子（文書の経路 + `#` + 位置）が、「指す」入力の宛先（世界状態の経路。6 節）と食い違い、対応が決まっていないためです（`docs/testing.md` の 5.111 の判断 4）。前後の切り替えの口も製品にはありません。

## 2. ビューの種類（7 種）

`workshop.yaml` の `views.<ID>.kind` に書く綴りです（ART-5、EP-5）。

| `kind` | 見せるもの | 語彙の版 |
|---|---|---|
| `file_tree` | ファイルツリー | 画面の契約（拡張点の番号なし） |
| `document` | 文書。`body: shown` で練習データの本文も出せる（3 節） | 画面の契約。`body` は v1.4（EP-13） |
| `diff` | 差分 | 画面の契約 |
| `task_tree` | タスクのツリー | 画面の契約 |
| `card_list` | 状態つきのカード（EP-5） | v1 |
| `check_list` | チェックの一覧 | 画面の契約 |
| `app_demo` | アプリのデモ | 画面の契約 |

- アプリのデモのワイヤー (b) の欄は常に `type=text` の読み取り専用で、パスワードの欄を出す経路がありません（TASK-492。ART-24）
- `kind` の綴りは、画面側が完全一致で読みます。大文字小文字の違いなど、一覧にない綴りは黙って飛ばします（fail-closed）。綴りを間違えると、`validate`・`compile` が `E-VIEW-KIND-UNKNOWN` で止めます（ビューの ID を示し、書いた値は写しません。2026-11-03、TASK-796、D-97）。`kind` の欠けや文字列でない値は `E-PARSE` です。画面側が一覧にない綴りを描かない動作は変わりません
- 文書と差分は、変種 (a) を使えません（画面側の規則）。作成者が選ぶ欄ではありません
- Step がビューを宣言するときは `views: [ビューの ID, ...]` と書きます。`workshop.yaml` に無い ID は `E-REF-VIEW` です。画面に出るビューは、宣言した順に、種類ごとの最初の 1 つです

```yaml
views:
  v-files: { kind: file_tree }
  v-cards: { kind: card_list }
  v-doc: { kind: document, body: shown }
```

## 3. 文書のビューの本文（EP-13）

`views.<ID>.body` に `shown` を書くと、文書のビューが練習データの本文を出します。省略すると `hidden` で、題・属性・注記だけを出します（PACK-38・D-50・D-51）。値の規則、`pack.vocab` に 1.4 以上を宣言する必要、`replace` で本文を替える書き方は `docs/pack-authoring.md` の「文書のビューの本文」を見てください。

- 製品の左エリアは、文書のビューを選んでいて `body: shown` を宣言していれば、行の要素の `fixture` に対応する練習データの本文を行の中に出します（ART-18。2026-10-24 時点）。`body` を省略した・`hidden` の教材、`fixture` の無い要素では本文を出しません。出さないときに理由の文は出しません。限界は `docs/known-gaps.md` の ART-18 にあります
- 受講者が選んだ値を本文に差し込むことはできません（H-3。`docs/known-gaps.md`）

## 4. 要素の属性と練習データ

左エリアの行は、世界状態の要素（`put`・`replace` で置いたもの）から作ります。

- 行の ID は、世界状態の経路そのものです。要素を置いた経路が、指せる行の宛先になります
- 行が持つ中身は、ラベル・補足・状態・タグです（`crates/web/src/artifact_view.rs` の `Cell`）。要素のどの属性名を、どの中身として読むかは、spec ではまだ確定していません。次の読み方は、結線（#2062〜#2068）が置いた仮置きです（2026-11-05 時点。確定は spec の判断）。語彙の外の値は、既知の状態へ寄せず固定の表示にします（fail-closed）。

| 項目 | 仮置きの読み方 |
|---|---|
| 属性の読み | `title` をラベル（無ければ世界状態の経路）、`detail` を補足、`state` を状態として読む。`tags` は常に空（`product_rows.rs` の `read_cell`） |
| 深さ | 経路の `/` の数（`path_depth`） |
| 行の種類（`data-kind`） | ファイルツリー: ほかの行の経路が「この経路 + `/`」で始まれば `dir`、そうでなければ `file`（途中のディレクトリの行は合成しない）。差分: `state` が `hunk` なら `hunk`、それ以外は `line`（`detail` が「旧:新」の形でなければ行番号は空）。アプリのデモ: 属性 `kind` を 6 語（`heading`・`tab`・`field`・`button`・`item`・`text`）で読み、無い値・語彙の外の値は文字のワイヤー（`text`）。欄は常に `type=text` の読み取り専用。タスクツリー `task`・カード `card`・チェック `check`・文書 `item` は固定 |
| 状態の語彙（完全一致） | タスク: `open`・`progress`・`done`（外は「不明」、記号 `?`）。カード: `draft`・`open`・`merged`・`closed`（外は「不明」）。チェック: `success`・`failure`・`running`・`pending`・`skipped`（外は「? 不明」）。差分: `ctx`・`add`・`del`・`hunk`（外は `?`）。アプリのデモ: 空・`active`・`disabled`（外は「（不明）」）。文書: 語彙に読まず、文字のまま出す |

第 3 教材の日本語の状態語（`未着手`・`進行中`・`完了`・`未分解`・`分解済み`）は語彙の外のため、バッジが「不明」になります（`docs/known-gaps.md` の「ビューの状態の語彙」）。教材の値は文字ノードとして出し、HTML としては解釈しません（SAFE-11）。
- 練習データ（`fixtures`）の `kind` は、pack crate が値を検査しない文字列です。練習データの種別の約束も、2026-10-20 時点では決まった規則を確認できていません（未確認）
- 要素の `fixture` 属性が、文書のビューの本文を決めます（3 節）

## 5. 左エリアの絞り込み（EP-12、`pack.left_pane`）

`pack.left_pane` の 2 項目（`world_list`・`view_tabs`）と値の意味は `docs/pack-authoring.md` の「左エリア」の表が正です。画面への効き方は次のとおり整理できます。

| 設定 | 契約が決めること | 2026-10-25 時点の画面 |
|---|---|---|
| `world_list: hidden` | 世界の一覧を出さない | 結線済み。世界状態の行を作らず、7 本の一覧はどれも項目 0 件になる（下の限界を参照） |
| `view_tabs: declared` | Step が宣言したビューだけに絞る | 宣言が 2 種以上の Step でだけ宣言した種類のボタンを出し、宣言外の選択は無視する |
| `view_tabs: hidden` | 切り替えを出さず、Step ごとに 1 つのビューだけを見せる | 結線済み。ボタンを出さず（親に `hidden`）、宣言の先頭のビューだけを見せる。それ以外の選択は無視する |
| `view_tabs: all` | すべてのビューの切り替えを出す | 結線済み。7 種のボタンを出し、すべて選べる。宣言に無い種類の一覧は、世界状態の行を出すだけで指せる行にならない |

設定と矛盾する Step の検査は、`validate` が `E-LEFT-*` で行います。

限界: 結線の後も、`world_list: hidden` にすると、指せる行と文書ビューの本文も消えます。`world_list: hidden` のとき `engine_parts.rs` は世界状態の行を作らず、7 種の `product_items` はどれも世界状態の行から作るためです。これらを使う教材では `world_list: hidden` を使わないでください（`docs/known-gaps.md` の PACK-22）。

| コード | 知らせること |
|---|---|
| `E-LEFT-MULTI-VIEW` | `view_tabs: hidden` なのに、Step が 2 つ以上のビューを宣言している |
| `E-LEFT-TARGET-VIEW` | 操作の宛先のビューが、その Step の画面に出ない |
| `E-LEFT-TARGET-ROW` | 宛先の `path` が、どの `put`・`replace` にも現れない |

- `E-LEFT-*` は、設定を既定から変えたときだけ検査します（`E-LEFT-TARGET-ROW` は `world_list` だけの変更も含みます）
- **近似です**。`E-LEFT-TARGET-ROW` は「その経路がどこかで世界状態に置かれる」という必要条件だけを調べます。画面でしか分からない矛盾（順序に依る不在、条件で通らない枝など）は報告しません。教材テストの模範解答で補ってください
- `views` を宣言しない Step の既定のビューは、試作の仮置きです（決まった規則は後続のタスクで定めます）。宣言する Step に揃えると、この仮置きに依らず書けます

## 6. 「指す」入力の宛先

画面の要素を指す入力（`input.kind: ui_action` と、確認の `point_at`）は、`targets` に `view`（ビューの ID）と `path`（世界状態の経路）を書きます。

```yaml
input:
  kind: ui_action
  targets: [{ view: v-cards, path: cards/sample }]
```

- 宛先の `path` は、`put` または `replace` で世界状態に置いた経路にします。置かれない経路を宛先にすると、設定が既定でないときは `E-LEFT-TARGET-ROW` で知らせます。既定のときの扱いは検査されません（未確認）
- `view` が `workshop.yaml` の `views` に無いと `E-REF-VIEW` です
- 製品の画面は、選ばれたビューの宣言 ID が引ける行を、仮置きの規則ですべて指せる行にしています（TASK-358 2/2。確定は後続のタスク）。経路が長すぎる、または空の行は、表示だけになります
- 受講者が指した経路の値は、規則の照合にだけ使われます。世界状態の経路や属性には入りません（H-3）

## 7. 表示件数の上限

- 1 つのリストの項目数は、上流の安全弁として 4,096 件です。超えると、先頭の 4,095 件と「上限超過: 先頭 … 件だけ表示（全 … 件）」の状態の行を出します（黙って切り詰めません）。件数以外の誤りでは通知の行になります。分割や畳み方の細目は、画面のタスクが決めます（`crates/web/src/list_policy.rs`）。左の成果物の 4,095・4,096・4,097 件は、ブラウザ試験で確かめています（`docs/testing.md` 5.295）
- 文書の差分は、片側のブロック数が上のリストの上限を超えると突き合わせを行いません（`artifact_doc_rows.rs`）
- 目安は 1 リスト 1,000 件です。左の成果物は、目安を超えると古い Step で変えた行を要約の 1 行に畳みます（最新の Step の行は畳みません）。基準端末での暫定値で、仕様は検討中です（FEND-22、D-46）
- 教材パックの大きさの上限は `docs/schema-format-b.md` の 2 節を見てください

## 8. 根拠の試験

| 試験 | 確かめること |
|---|---|
| `crates/pack/tests/left_pane.rs` | 左エリアの設定の読み込みと `E-PARSE`、`E-LEFT-*` の条件と報告先 |
| `crates/web/tests/left_pane_view.rs`・`tools/browser/left-pane.test.mjs` | 6 通りの設定でのボタン・選択の親・行の件数、宣言外の選択の無視、Step の移動での先頭への復帰、読めない設定の既定への倒し |
| `crates/pack/tests/view_body.rs` | `body: shown` の宣言・不正な値・語彙の版の不一致 |
| `crates/web/tests/artifact_view_contract.rs` | 7 種の綴り・束縛点・指せる行の属性の契約 |
| `crates/web/tests/artifact_view_sample.rs` | 7 本のリストを 1 部品に置く形 |
| `crates/web/tests/product_rows.rs`・`document_element_rows.rs`・`task_tree_product.rs`・`card_list_product.rs`・`file_tree_product.rs`・`diff_product.rs`・`check_list_product.rs`・`app_demo_product.rs` | 製品の画面の 7 種の行（期待 HTML は `tests/data/product_view_html/*.html`）。語彙の外の値の固定の表示、経路の行への退避、パスワードの欄が出ないこと（`app_demo_product.rs`） |
| `tools/browser/choice-journey/*.json` の `texts`・`rowTexts` | 製品の画面の行の文字・種類・入れ子（`review-request-derived*.json`・`task-breakdown*.json`。記録は `docs/testing.md` の 5.469） |
| `tools/browser/` の `artifact-view-check.mjs` を使う試験 | 変化の印・行の同一性・危険な要素が 0 件であること（代役の部品に対して） |
