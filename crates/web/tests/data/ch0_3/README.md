# ch0_3

CH-0〜CH-3 の形を写した 20 Step の試験用の教材（`ir.json`）です。TASK-450 3/3 の試験が使います。

- native: `crates/web/tests/ch0_3_run_host.rs`
- ブラウザ: `tools/browser/ch0-3-run.test.mjs`（試験の設定の画面を `?fixture=ch0-3` で開く。選択は `crates/web/src/test_fixture.rs`）

`fixed_sequence_ch0_1/ir.json` は、この `ir.json` の凍結時点（2026-10-22）のバイト写しです。写しは追随させません（`fixed_sequence_ch0_1/README.md`）。

## 出所と位置づけ

教材 repo の CH-0〜CH-3 はまだ無く（TASK-642 が未完了）、試作の教材パックの公開の可否も決まっていません（`testdata/README.md`）。
このため、本物の写しではなく、章ごとの Step 数・入力方式・確認の種類・拒否・問い返し・補完・処理中の**形**だけを写しました。
文面は架空で汎用の題材（メモ・カード・ファイル）にしてあり、上流の文面と教材固有の語は入れていません。
`commands`・`rejections`・`clarifications`・`defaults` は `testdata/input-field-ime/test-data/ch1-pack.json`（PoC-2 の CH-1 の 6 Step）から写し、足りない文面を足しました。

| 章 | Step 数 | 形 |
|---|---|---|
| CH-0 | 4 | 選択肢（3 件）と、答える確認 3 つ |
| CH-1 | 6 | `ch1-pack.json` の 6 Step と同じ形（指す・補完・複数行・問い返しと取り消し・処理中・選択肢と答える確認） |
| CH-2 | 8 | 名前つきのコマンド・表示名の選択・承認・自然文・処理中・対象つきの「直す」・画面操作 |
| CH-3 | 2 | 選択肢の誤答と指す確認、最後の承認 |

## 作り方の規則

- 節点は `scope: main` に線形につなぎ、`next` は無条件の 1 本だけにします。最後の `main/st-3-2` は `next: []` です。章をまたぐ遷移も、同じ無条件の `next` です
- `starts` は `main/st-0-1` の 1 件だけです。開始状態は `test_fixture.rs` が `starts[0].node` に合わせます
- `pack.id` は `ch0-3-run` です（既定の `min-3step` と保存先の鍵を分けるため）
- 各 Step は `views` を 1 件だけ宣言します（CH-0・CH-1 は `v-cards`、CH-2・CH-3 は `v-files`）。確認の `point_at` と入力の `ui_action` の宛先は、その Step の宣言と一致させます
- 指す対象の行は、その Step に入る前、または同じ Step の受付の効果で作ります。`on_enter` と `starts[].effects` は使いません。core が適用しないためです
- 規則の `when` は使いません。`ch1-pack.json` の st-1-4 の取り消しの規則は `when: {clarifying: …}` を持ちますが、core は `when` を評価しないため、`when` なしで書き直しました
- 処理中は CH-1 の st-1-5（4 進行）と CH-2 の st-2-5（2 進行）だけに置きます

## 写していないもの

- 教材の文面（本文・`do` の詳細）。`body` は名前だけで、本文は読み込みません
- 遷移先の `on_enter`・入口の写しの作り直し・章の完了・チェックポイントの復帰。core にまだ無いか、製品の画面に口がありません
- 規則の `when`（上記）
- 処理中の Esc による停止。先頭以外の Step では入口の写しが無く、停止が警告で無視されます（`docs/testing.md` の 5.117）
