# ch0_3_pack

CH-0〜CH-3 の形の 20 Step を、形式 B（YAML と Markdown）の教材パックとして書いた試験用の教材です。TASK-427 1/2 の試験が使います。

- ブラウザ: `tools/browser/author-preview.test.mjs`（`fandhe-pack compile` の出力を `IR_FILE` に渡して配信物を作り、サブパスの下で CH-0〜CH-3 を通す。記録は `docs/testing.md` の 5.292）
- ビルド: `scripts/test-browser.sh`

## 出所と位置づけ

教材 repo の CH-0〜CH-3 はまだ無く（TASK-642・TASK-643 が未完了）、本物の写しではありません。手書きの IR（`../ch0_3/ir.json`）の 20 節点を 1 対 1 で YAML に写し、作成者の手順（compile → 配信）を実物で流せる形にしました。
文面は `../ch0_3/README.md` と同じ方針で、架空で汎用の題材（メモ・カード・ファイル）です。上流の文面と教材固有の語は入れていません。

## IR をコミットしない理由

作成者の手順そのもの（`fandhe-pack compile` → `IR_FILE=… tools/build-web.sh`）を流すため、IR は試験のたびに compile で作ります。

## `../ch0_3/ir.json` との差

| 項目 | `ch0_3/ir.json` | この教材 |
|---|---|---|
| `pack.id` | `ch0-3-run` | `ch0-3-author`（保存先の鍵を分ける） |
| `pack.ir_format` | 1 | 2（本文の木 `body_doc` を出す） |
| 各章の最後の Step（`st-0-2c`・`st-1-6`・`st-2-8`・`st-3-2`） | `st-3-2` 以外は次の章へ続く `next` | `final: true` で `next` なし（他の章を指す `next` は `E-REF-STEP` になるため） |
| `starts` | `s-0-1` の 1 件 | 各章の先頭の 4 件（`s-0-1`・`s-1-1`・`s-2-1`・`s-3-1`）。章の移動は「次の章へ進む」で行う |
| 本文 | 名前だけ | 全 Step に短い `.md`。先頭の見出しはその Step の題 |

それ以外（Step の ID・題・`do`・入力・規則・確認・拒否・処理中・`views`）は同じです。試験の P4 が、この対応を構造の要約で照合します。
片方を変えたら、もう片方も揃えてください。

## 写していないもの

`../ch0_3/README.md` の「写していないもの」と同じです。加えて、教材テスト（`tests/playthrough.yaml`）は置いていません。
