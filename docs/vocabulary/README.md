# 語彙の資料の索引

教材の作成者が、教材パックで使える語と名前つき拡張点（EP-1〜EP-13）を探すための索引です。語と版の正は、同じディレクトリの `vocabulary.md` です。この索引は、作成者が書き方の資料へ移るための道案内で、語の版や名前を新しく定めません。

- 文書の版: 1
- 確認した日: 2026-10-20
- 仕様は private のため、本文を転記せず ID（PACK-8〜PACK-23・PACK-38・AUTHOR-12・AUTHOR-13・AUTHOR-17・D-21・D-64）で参照します

## 1. 読み方

| 知りたいこと | 読む場所 |
|---|---|
| 語の一覧と、語ごとの語彙の版 | `vocabulary.md` の 7 節 |
| 名前つき拡張点の名前・種別・語彙の版 | `vocabulary.md` の 2 節 |
| 語彙の版と拡張点の対応 | `vocabulary.md` の 8 節 |
| 語彙 v1〜v1.3 の写し | 別にありません。`vocabulary.md` が v1〜v1.5 の全体を、語ごとの版つきで固定しています |
| 欄の名前・型・必須 | `docs/schema-format-b.md` |
| 書き方の詳細 | `docs/pack-authoring.md` |
| 拡張点を足す手続き | `docs/extension-point-procedure.md` |

- `vocabulary.md` は固定の契約です（D-21）。固定した行は書き換えず、足すときは新しい語彙の版の行として加えます。この索引は `vocabulary.md` の本文を変えず、写しを持ちません。食い違うときは `vocabulary.md` が正です
- 教材固有の語は、拡張点の名前に含められません（汎用名の検査。`vocabulary.md` の 4 節）

## 2. `pack.vocab`（語彙の版の宣言）

- `pack.vocab` には、使っている語の中で最も新しい語彙の版以上を、`主.副` で書きます（例: `"1.4"`）。引用符で囲みます
- エンジンが読める語彙の上限は 1.5 です。宣言が上限を超える履歴は開きません（`vocab_newer`）
- 宣言より新しい版の拡張点（EP-9〜EP-13 と、確認の選択肢の `label`、承認の `input.revise.candidates`、実行環境の操作 `env_action`）を使うと、`validate` が拡張点ごとに 1 件の `E-VOCAB-MISMATCH` を報告します（PACK-31）。語彙 1.1 までの語（EP-1〜EP-8）は、この検査の対象ではありません
- 詳しい規約は `docs/pack-authoring.md` の「`pack.vocab` の書き方」「`pack.vocab` の宣言と使用の一致」を見てください

## 3. 名前つき拡張点の索引

名前・種別・語彙の版は `vocabulary.md` の 2 節の表が正です。次の表は、書き方の資料と、作成者への注意を足した道案内です。

| ID | 名前 | 語彙の版 | 書き方の資料 | 作成者への注意 |
|---|---|---|---|---|
| EP-1 | `input.wait` | v1 | `docs/pack-authoring.md` の「入力方式」「処理中の進行」 | `processing` が 1 件以上ないと `E-STRUCTURE`。進行の間隔は宣言できません（PACK-44） |
| EP-2 | `input.cancel` | v1 | `docs/pack-authoring.md` の「入力方式」 | `kind: cancel` はありません。規則の `on: { cancel: true }` で受けます |
| EP-3 | `condition.param` | v1 | `docs/schema-format-b.md` の 7 節 | モジュールの引数との比較です |
| EP-4 | `condition.clarifying` | v1 | `docs/schema-format-b.md` の 7 節 | 問い返しの待ちかどうかを見ます |
| EP-5 | `view.card_list` | v1 | `docs/views.md` の 2 節 | `kind: card_list` の綴りは検査されません。綴りを確かめてください |
| EP-6 | `confirm.answer.option_on_wrong` | v1.1 | `docs/pack-authoring.md` の「確認」 | 正答の選択肢に書くと `E-STRUCTURE` です |
| EP-7 | `text.slot` | v1.1 | `docs/pack-authoring.md` の「差し込みの枠」 | **`{{...}}` を含む値は引用符で囲みます。** flow 形式の中、および値が `{{` で始まる位置の引用符なしは `E-PARSE` です（`docs/schema-format-b.md` の 9 節。AUTHOR-13） |
| EP-8 | `guidance.variant` | v1.1 | `docs/pack-authoring.md` | `pack.guidance_levels`・`body_levels`・ヒントの `levels`・開始点の `guidance` で出し分けます。`vocab` は `body_levels` とヒントの `levels` だけを数えます |
| EP-9 | `starts.requires` | v1.2 | `docs/schema-format-b.md` の 4 節 | `requires` または `unmet` を持つ開始点が、`vocab` の EP-9 の数え方の対象です（TASK-409。`use` の Step を指す開始点は数えません） |
| EP-10 | `pack.profile` | v1.3 | `docs/operation-profile.md` の 1 節 | `command_prefix` と `input_field`。既定から変えたときだけ `E-PROFILE-*` を検査します |
| EP-11 | `pack.appearance` | v1.3 | `docs/operation-profile.md` の 2 節 | 外観は閉じた 4 値（`standard`・`mono`・`high_contrast`・`reading`）で、見た目だけを変えます。意味は 2 節にあります。2026-10-20 時点では、外観ごとの見た目は画面に結線されていません |
| EP-12 | `pack.left_pane` | v1.3 | `docs/pack-authoring.md` の「左エリア」、`docs/views.md` の 5 節 | `E-LEFT-*` は近似の検査です。2026-10-20 時点では、画面への結線は後続です |
| EP-13 | `view.document_body` | v1.4 | `docs/pack-authoring.md` の「文書のビューの本文」、`docs/views.md` の 3 節 | `body` を書く教材は `pack.vocab` に 1.4 以上を宣言します |

## 4. 拡張点の番号を持たない語

- `confirm.options[].label`（v1.3、PACK-23）: 確認 `answer` の選択肢の表示名です。入力方式 `choice` の `options[].label` とは別の欄で、取り違えると宣言と使用の検査がずれます。`vocab` コマンドは `label` を数えません（AUTHOR-17）。出力の形は `docs/cli.md` の 12 節と 13 節にあります
- `input.revise.candidates` と `input.revise.candidates[].label`（v1.5、PACK-42）: 承認の「直す」の誤った対象の候補と表示名です。どの候補が正しいかは宣言せず、規則の `on: { approval: revise, target }` が決めます。`vocab` コマンドは数えません。書き方は `docs/pack-authoring.md` の「承認の『直す』の候補と表示名」にあります
- `env_action`・`input.action`・`input.action=save`・`input.action=export`（v1.5、PACK-34）: 保存・書き出しの操作を Step の入力として宣言する語です。値は `save` と `export` の 2 つだけで、操作が成功したら受付になります。受講者の値は持ちません。`export` は実行時の結線が未実装です（`docs/known-gaps.md`）。`vocab` コマンドは数えません。書き方は `docs/pack-authoring.md` の「保存・書き出しを入力とする Step」にあります
- `pack.vocab` 自体は宣言の欄で、語には数えません（PACK-31）

## 5. 足さないと決まっている語

| 話題 | 状態 | 資料 |
|---|---|---|
| 受講者が選んだ値の、文中・経路・属性への差し込み | 足さないと決定（PACK-39、D-50） | `docs/known-gaps.md` の H-3 |
| 1 Step の `approval` での承認 → 差し戻し → 再承認 | 足さないと決定（PACK-32、D-69） | `docs/pack-authoring.md` の「承認の差し戻しと再承認」 |
| 進行の間隔の宣言欄 | 足さないと決定（PACK-44、D-43） | `docs/pack-authoring.md` の「処理中の進行」 |
| 入力のない確認だけの Step を表す語（読むだけの Step） | 足さないと決定（PACK-35、D-69） | `docs/pack-authoring.md` の「読むだけの Step」 |
| 確認 `answer` に複数の問いを持たせる語（複数の対象の分類） | 足さないと決定（PACK-33、D-69） | `docs/pack-authoring.md` の「複数の対象を 1 つずつ分類する確認」 |
| 誤答の拒否に前後比較のビューを結ぶ語 | 足さないと決定（PACK-36、D-69） | `docs/pack-authoring.md` の「誤答の拒否で前後の比較を案内する」 |
| 自然文の内容で規則を分ける語（判定語） | 足さないと決定（PACK-41、D-43） | `docs/known-gaps.md` の H-2 |

## 6. この索引の保守

- 拡張点を足したときは、`vocabulary.md` の表と版を先に直し、その後でこの索引の表に行を足します（手続きは `docs/extension-point-procedure.md`）
- `vocabulary.md` の試験は、`vocabulary.md` の表だけを読みます。この索引には、行頭が `EP-` の表の行を置いても試験に影響しませんが、`vocabulary.md` の側には増やさないでください
