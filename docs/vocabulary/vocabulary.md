# 語彙の文書

```text
語彙の文書の版: 4
決定日: 2026-10-21
状態: 固定（契約）
```

## 1. 位置づけと版

- 本文書は、教材パックが使う語彙の全体（語彙 v1〜v1.5）を、語ごとの語彙の版つきで固定する契約である。名前つき拡張点の名前・種別・語彙の版もここで固定する（TASK-48、TASK-194、PACK-8〜PACK-23）
- 「語彙の文書の版」は本文書の版（整数）であり、語ごとの「語彙の版」（`v1`・`v1.1` など）とは別物である。IR の形式の版 `ir_format`（TASK-41）とも別に管理する
- 版 1 は名前つき拡張点 EP-1〜EP-5（語彙 v1）だけを固定した。版 2 は加算で、EP-6〜EP-12、拡張点の番号を持たない `confirm.options[].label`、条件・状態変化・入力方式などを含む語彙 v1〜v1.3 の全語（7 節）、版と拡張点の対応表（8 節）を足す
- 版 3 は加算で、EP-13（`view.document_body`）と語彙 v1.4 の 3 語（`views.body` と値 2 つ）を足す（TASK-66 1/2、PACK-38、D-50・D-51）。文書のビューが練習データの本文を出す宣言である
- 版 4 は加算で、語彙 v1.5 の 2 語（`input.revise.candidates` と `input.revise.candidates[].label`）を足す。承認の「直す」の誤った対象の候補と表示名の宣言で、拡張点の番号は持たない（TASK-62 2/2、PACK-42、D-69）。v1.5 は D-69 で PACK-34（保存・書き出しの語。TASK-64 1/2）の語とまとめると決めた版である。PACK-34 の語は、v1.5 の行へ語を足す形で加え、文書の版は上げるが語彙の版は上げない
- TASK-62〜TASK-72 の語彙は、TASK-66 の文書ビューの本文の語（v1.4）と TASK-62 の「直す」の候補の語（v1.5）を除き、本版に含めない。足すときは新しい語彙の版の行（v1.5 の語は既存の行）として加え、文書の版を上げる
- 変更規則: 固定した行は書き換えない・消さない・改名しない。足すときは新しい語彙の版の行として加え、文書の版を上げ、変更履歴に記録する
- 名前を変える必要が出た場合は、仕様（`docs/spec` の 04）を先に直し、版上げの契約タスクを立てる
- 仕様は private のため、本文を転記せず ID（PACK-14、EXT-8）で参照する

## 2. 名前つき拡張点（公開の一覧）

教材パックからは、名前で参照するだけで任意のコードは書けない（EXT-8）。次の表が公開の一覧であり、名前・種別・語彙の版の正である。

| ID | 名前 | 種別 | 語彙の版 | 書き方（要旨） | 汎用である理由（要旨） |
|---|---|---|---|---|---|
| EP-1 | `input.wait` | `input` | `v1` | 入力方式 `input: { kind: wait }` と `processing`。進行イベントの完了で受付になり、受講者の入力は要らない | 処理中の表示を見て待つ場面は、シェルの長い処理でもビルドの待ちでも現れる |
| EP-2 | `input.cancel` | `input` | `v1` | 規則の `on: { cancel: true }`。取り消し・停止の入力はどの Step にも届くが、意味を持つのはこの規則を書いた Step だけ | 端末風・フォーム風のどちらの教材にも「やめる」操作がある |
| EP-3 | `condition.param` | `condition` | `v1` | 条件 `{ param: { name, eq } }`。モジュールの引数との比較 | モジュールの再利用の一般的な仕組み |
| EP-4 | `condition.clarifying` | `condition` | `v1` | 条件 `{ clarifying: <問い返しの ID> }`。問い返しの待ちかどうか | 聞き返されたかどうかは、対話のある教材なら共通 |
| EP-5 | `view.card_list` | `view` | `v1` | ビューの種類 `card_list`。要素の `state` 属性を持つ状態つきのカード | 付箋・チケット・カードを並べる教材で共通 |
| EP-6 | `confirm.answer.option_on_wrong` | `confirm` | `v1.1` | 選択式の確認 `answer` の誤答の選択肢ごとに、理由の文面（`confirm.options[].on_wrong`）を書く | 選択式の確認を持つ教材なら、誤答ごとの理由を示したい場面が共通に現れる |
| EP-7 | `text.slot` | `text` | `v1.1` | 文面に差し込む枠。名前は閉じた集合で、式やスクリプトは入らない | 拒否や案内の文面に、入力の語や今の Step を差し込みたい場面は教材によらない |
| EP-8 | `guidance.variant` | `guidance` | `v1.1` | 案内レベル（A・B・C）による本文とヒントの出し分け | 初心者向けと経験者向けで案内の厚みを変える教材で共通 |
| EP-9 | `starts.requires` | `starts` | `v1.2` | モジュールの開始条件（`starts[].requires`）と、満たさないときの文面（`starts[].unmet`） | 前の学習を終えていることを前提とする教材で共通 |
| EP-10 | `pack.profile` | `pack` | `v1.3` | コマンドの接頭辞と入力欄の有無 | 入力の作法は教材ごとに選べてよい、共通の設定 |
| EP-11 | `pack.appearance` | `pack` | `v1.3` | 外観の選択。閉じた 4 値で、見た目だけを変える | 読みやすさの要請は教材によらない |
| EP-12 | `pack.left_pane` | `pack` | `v1.3` | 世界状態の一覧とビューの切り替えボタンの表示の絞り込み | 画面の情報量を絞りたい教材で共通 |
| EP-13 | `view.document_body` | `view` | `v1.4` | 文書のビューが、行の要素の練習データの本文を出す宣言（`views.<ID>.body: shown`）。省略は `hidden` | 文書・手順書・メモの中身を見せる教材は、題材によらず共通 |

- 書き方は要旨である。欄の型と評価の意味の正は、各実装タスク（TASK-42・TASK-44・TASK-46・TASK-49・TASK-51・TASK-52・TASK-53）とする。本文書が固定するのは、名前・種別・語彙の版である
- 試験は、表のうち行頭が `EP-` の行を数える。この表以外に、行頭が `EP-` の表の行を置かない

## 3. 名前の形

- 名前は `<種別>.<語>` の形で、英小文字・数字・`_` と、区切りの `.` で書く
- 版 2 から、先頭の `.` までが種別、残りは `.` で区切った 1 つ以上の語とする。EP-6 の `confirm.answer.option_on_wrong` が 3 段であるため、版 1 の「`.` は 1 つだけ」の規則を広げた。版 1 の名前は 2 段で、そのまま規則に合う
- 種別は、v1 では `input`・`condition`・`view`、v1.1 以降で `confirm`・`text`・`guidance`・`starts`・`pack` が加わる
- 名前の先頭は、種別の列と一致させる

## 4. 汎用名の検査

- 拡張点の名前は、特定の教材（掲示板・Claude Code・GitHub など）に固有の語を含まず、他の教材にも共通の語でなければならない（PACK-14）
- 照合の対象は、表の名前の列だけである。書き方・理由の列は対象外とする
- 照合の方法と固有の語の一覧は、付録 A に置く。試験は一覧を本文書から読み、`crates/pack/tests/vocabulary_extension_points.rs` が名前と照合する
- 照合は 2 種類である
  - `部分一致`: 名前を英小文字にした全体に、語が部分文字列として現れたら違反とする。固有名詞が対象で、連結した形（`githubcard` など）も捕まえる
  - `語の一致`: 名前を `.`・`_`・`-` で分割した各語と完全に一致したら違反とする。一般語に近い語が対象で、部分一致にすると将来の汎用名を誤って拒否するため、完全一致に限る

## 5. 固定・実装・公開の順

1. 本文書に、名前・語彙の版・種別を固定する。汎用である理由を書き、固有の語の照合を通す
2. 実装する。EP-1・EP-2 は TASK-44、EP-3・EP-4 は TASK-42 が担う。EP-5 のビューは表示側のタスクが担う
3. 公開の一覧（2 節の表）を、実装済みの語として外部に示す

この順を、TASK-189（拡張点の追加の手続き、EXT-8）の入力とする。手続き本体（診断コードの追加、既存の教材パックが同じ IR になることの確認を含む）は `docs/extension-point-procedure.md`（TASK-189）に書いた。

## 6. 本文書を入力にするタスク

| タスク | 入力として使う内容 |
|---|---|
| TASK-42 | EP-3・EP-4（条件の語彙） |
| TASK-44 | EP-1・EP-2（入力方式の語彙） |
| TASK-189 | 5 節の順（拡張点の追加の手続き） |
| TASK-194 | 本文書の拡張（語彙 v1〜v1.3 の文書化） |
| TASK-42 | 7 節の `condition` の語（EP-3・EP-4 を含む） |
| TASK-43 | 7 節の `effect` の語 |
| TASK-44 | 7 節の `input` の語と、`rejection`・`result` の語 |
| TASK-45 | 7 節の `confirm` と `rejection` の語（判定結果の全体は JUDGE-3 を参照） |
| TASK-46 | 7 節の `slot` の語と `confirm.options[].on_wrong`（EP-6・EP-7） |
| TASK-47 | 7 節の `confirm` の語と `confirm.options[].label` |
| TASK-49 | 7 節の案内レベルの語（EP-8） |
| TASK-50 | 7 節の `module` の語（`use` など） |
| TASK-51 | 7 節の開始条件の語（EP-9） |
| TASK-52 | 7 節の `pack.profile` の語（EP-10） |
| TASK-53 | 7 節の `pack.appearance`・`pack.left_pane` の語（EP-11・EP-12） |
| TASK-66 | 2 節の EP-13・7 節の `views.body` の語 |
| TASK-59 | 7 節の語の一覧（宣言と使用の一致の検査の対象） |
| TASK-256 | 7 節の `result`・`rejection` の語 |
| TASK-408 | `vocab` コマンドの数え上げ |

## 7. 語の一覧

語彙 v1〜v1.5 の語を、語ごとの語彙の版つきで固定する。（種別, 語）の組が一意のキーである。試験は、この表と `crates/pack/src/vocab.rs` の表の一致を確かめる。

- 種別の列は分類のキーであり、2 節の拡張点の種別とは別物である
- 語の列の `a.b` は欄の経路、`x=y` は欄 `x` の値 `y` を指す
- 拡張点の列が `—` の語は、拡張点の番号を持たない
- `rejection` の `free_text_unjudged` は受付に付く分類であり、拒否ではない（PACK-11）
- `confirm.options[].label` は、入力方式 `choice` の `options[].label` とは別の欄である（PACK-23）

| 種別 | 語 | 語彙の版 | 拡張点 | 出所 | 書き方（要旨） |
|---|---|---|---|---|---|
| `input` | `command` | `v1` | — | PACK-10 | コマンドと引数で受け付ける入力方式 |
| `input` | `text` | `v1` | — | PACK-10 | 複数行の自然文を、空でないことと文字数だけで受け付ける入力方式 |
| `input` | `choice` | `v1` | — | PACK-10 | 選択肢から選ぶ入力方式 |
| `input` | `ui_action` | `v1` | — | PACK-10 | 画面の操作を入力とする入力方式 |
| `input` | `approval` | `v1` | — | PACK-10 | 承認・差し戻しを入力とする入力方式 |
| `input` | `wait` | `v1` | EP-1 | PACK-10 | 受講者の入力を要さず、進行の完了で受付になる待機 |
| `input` | `cancel` | `v1` | EP-2 | PACK-10 | 取り消し・停止の入力を規則で受ける |
| `condition` | `step` | `v1` | — | PACK-8 | Step の状態を見る条件 |
| `condition` | `flag` | `v1` | — | PACK-8 | フラグの有無を見る条件 |
| `condition` | `world_has` | `v1` | — | PACK-8 | 世界状態に要素があるかを見る条件 |
| `condition` | `world_attr` | `v1` | — | PACK-8 | 要素の属性の一致を見る条件 |
| `condition` | `count` | `v1` | — | PACK-8 | 件数の比較の条件 |
| `condition` | `chose` | `v1` | — | PACK-8 | 選んだ選択肢を見る条件 |
| `condition` | `all` | `v1` | — | PACK-8 | 子の条件がすべて成り立つ論理 |
| `condition` | `any` | `v1` | — | PACK-8 | 子の条件のどれかが成り立つ論理 |
| `condition` | `not` | `v1` | — | PACK-8 | 子の条件の否定 |
| `condition` | `param` | `v1` | EP-3 | PACK-8 | モジュールの引数との比較 |
| `condition` | `clarifying` | `v1` | EP-4 | PACK-8 | 問い返しの待ちかどうか |
| `effect` | `put` | `v1` | — | PACK-9 | 要素を追加する状態変化 |
| `effect` | `remove` | `v1` | — | PACK-9 | 要素を削除する状態変化 |
| `effect` | `replace` | `v1` | — | PACK-9 | 要素を置き換える状態変化 |
| `effect` | `set_attr` | `v1` | — | PACK-9 | 属性を変える状態変化 |
| `effect` | `set_flag` | `v1` | — | PACK-9 | 実体ごとのフラグを立てる |
| `effect` | `clear_flag` | `v1` | — | PACK-9 | 実体ごとのフラグを下ろす |
| `effect` | `set_flag_global` | `v1` | — | PACK-9 | 教材全体のフラグを立てる |
| `effect` | `processing` | `v1` | — | PACK-9 | 処理中の進行を表す状態変化 |
| `effect` | `when` | `v1` | — | PACK-9 | 条件がそろったときだけ変化を適用する |
| `confirm` | `point_at` | `v1` | — | PACK-13 | 画面の要素を指して確認する |
| `confirm` | `answer` | `v1` | — | PACK-13 | 選んで答えて確認する |
| `confirm` | `approve` | `v1` | — | PACK-13 | 承認して確認する |
| `confirm` | `none` | `v1` | — | PACK-13 | 確認を省く（理由を添える） |
| `field` | `confirm.reason` | `v1` | — | PACK-13 | 確認を省く理由を書く欄 |
| `result` | `accepted` | `v1` | — | PACK-11 | 受付を期待する教材テストの結果 |
| `result` | `rejected` | `v1` | — | PACK-11 | 拒否を期待する教材テストの結果 |
| `result` | `clarified` | `v1` | — | PACK-11 | 問い返しを期待する教材テストの結果 |
| `rejection` | `unknown_command` | `v1` | — | PACK-11 | 未知のコマンドの分類 |
| `rejection` | `wrong_order` | `v1` | — | PACK-11 | 順序違いの分類 |
| `rejection` | `missing_arg` | `v1` | — | PACK-11 | 引数不足の分類 |
| `rejection` | `bad_arg_shape` | `v1` | — | PACK-11 | 引数の形が違う分類 |
| `rejection` | `busy` | `v1` | — | PACK-11 | 処理中の分類 |
| `rejection` | `not_in_this_step` | `v1` | — | PACK-11 | この Step では使わない分類 |
| `rejection` | `not_supported` | `v1` | — | PACK-11 | 教材未対応の分類 |
| `rejection` | `free_text_unjudged` | `v1` | — | PACK-11 | 自然文の内容は判定しない分類（受付に付き、拒否ではない） |
| `field` | `confirm.on_wrong` | `v1` | — | PACK-15 | Step の既定の誤答の文面 |
| `field` | `view.card_list` | `v1` | EP-5 | PACK-14 | 状態つきのカードを並べるビューの種類 |
| `module` | `use` | `v1` | — | PACK-19 | モジュールを呼ぶ欄 |
| `module` | `module` | `v1` | — | PACK-19 | モジュールの定義 |
| `module` | `args` | `v1` | — | PACK-19 | 呼び出しで渡す引数 |
| `module` | `params` | `v1` | — | PACK-19 | モジュールが受ける引数の宣言 |
| `module` | `starts` | `v1` | — | PACK-19 | モジュールの開始点の宣言 |
| `field` | `confirm.options[].on_wrong` | `v1.1` | EP-6 | PACK-15 | 誤答の選択肢ごとの文面 |
| `slot` | `input_head` | `v1.1` | EP-7 | PACK-16 | 入力の先頭の語を文面へ差し込む枠 |
| `slot` | `nearest_command` | `v1.1` | EP-7 | PACK-16 | 近い候補のコマンドを差し込む枠 |
| `slot` | `step_title` | `v1.1` | EP-7 | PACK-16 | 今の Step の題を差し込む枠 |
| `slot` | `step_do` | `v1.1` | EP-7 | PACK-16 | 今やるべきことを差し込む枠 |
| `slot` | `remaining` | `v1.1` | EP-7 | PACK-16 | 残りの数を差し込む枠 |
| `slot` | `total` | `v1.1` | EP-7 | PACK-16 | 全体の数を差し込む枠 |
| `field` | `do` | `v1.1` | EP-7 | PACK-16 | Step の今やるべきことの文 |
| `field` | `pack.guidance_levels` | `v1.1` | EP-8 | PACK-17 | 案内レベルと隠す語の宣言 |
| `field` | `starts[].guidance` | `v1.1` | EP-8 | PACK-17 | 開始点ごとの案内レベル |
| `field` | `body_levels` | `v1.1` | EP-8 | PACK-17 | 水準ごとの本文 |
| `field` | `hints.levels` | `v1.1` | EP-8 | PACK-17 | ヒントを出す水準 |
| `value` | `guidance=A` | `v1.1` | EP-8 | PACK-17 | 案内レベルの水準の値 |
| `value` | `guidance=B` | `v1.1` | EP-8 | PACK-17 | 案内レベルの水準の値 |
| `value` | `guidance=C` | `v1.1` | EP-8 | PACK-17 | 案内レベルの水準の値 |
| `field` | `starts[].requires` | `v1.2` | EP-9 | PACK-18 | 開始条件（前提とする世界状態） |
| `field` | `starts[].unmet` | `v1.2` | EP-9 | PACK-18 | 開始できないときの文面 |
| `field` | `pack.profile.command_prefix` | `v1.3` | EP-10 | PACK-20 | コマンドの接頭辞 |
| `field` | `pack.profile.input_field` | `v1.3` | EP-10 | PACK-20 | 入力欄の有無 |
| `value` | `pack.profile.input_field=shown` | `v1.3` | EP-10 | PACK-20 | 入力欄の値 |
| `value` | `pack.profile.input_field=hidden` | `v1.3` | EP-10 | PACK-20 | 入力欄の値 |
| `field` | `pack.appearance.theme` | `v1.3` | EP-11 | PACK-21 | 外観の選択（見た目だけを変える） |
| `value` | `pack.appearance.theme=standard` | `v1.3` | EP-11 | PACK-21 | 外観の値（閉じた集合） |
| `value` | `pack.appearance.theme=mono` | `v1.3` | EP-11 | PACK-21 | 外観の値（閉じた集合） |
| `value` | `pack.appearance.theme=high_contrast` | `v1.3` | EP-11 | PACK-21 | 外観の値（閉じた集合） |
| `value` | `pack.appearance.theme=reading` | `v1.3` | EP-11 | PACK-21 | 外観の値（閉じた集合） |
| `field` | `pack.left_pane.world_list` | `v1.3` | EP-12 | PACK-22 | 世界状態の一覧の表示 |
| `field` | `pack.left_pane.view_tabs` | `v1.3` | EP-12 | PACK-22 | ビューの切り替えボタンの表示 |
| `value` | `pack.left_pane.world_list=shown` | `v1.3` | EP-12 | PACK-22 | 左エリアの値 |
| `value` | `pack.left_pane.world_list=hidden` | `v1.3` | EP-12 | PACK-22 | 左エリアの値 |
| `value` | `pack.left_pane.view_tabs=all` | `v1.3` | EP-12 | PACK-22 | 左エリアの値 |
| `value` | `pack.left_pane.view_tabs=declared` | `v1.3` | EP-12 | PACK-22 | 左エリアの値 |
| `value` | `pack.left_pane.view_tabs=hidden` | `v1.3` | EP-12 | PACK-22 | 左エリアの値 |
| `field` | `confirm.options[].label` | `v1.3` | — | PACK-23 | 選択肢の表示名（入力方式 choice の options[].label とは別の欄。拡張点の番号なし） |
| `field` | `views.body` | `v1.4` | EP-13 | PACK-38 | 文書のビューが練習データの本文を出すか |
| `value` | `views.body=shown` | `v1.4` | EP-13 | PACK-38 | 本文の値（閉じた集合） |
| `value` | `views.body=hidden` | `v1.4` | EP-13 | PACK-38 | 本文の値（閉じた集合。省略と同じ） |
| `field` | `input.revise.candidates` | `v1.5` | — | PACK-42 | 承認の「直す」の対象の候補（正しい対象と誤った候補を並べる順）。`approval` の入力だけが持つ |
| `field` | `input.revise.candidates[].label` | `v1.5` | — | PACK-42 | 候補の表示名（`confirm.options[].label` と同じ型。省略は ID） |

## 8. 版と拡張点の対応表

| 語彙の版 | 拡張点 | 番号なしの語 | 出所 |
|---|---|---|---|
| `v1` | EP-1・EP-2・EP-3・EP-4・EP-5 | — | PACK-14 |
| `v1.1` | EP-6・EP-7・EP-8 | — | PACK-15〜PACK-17 |
| `v1.2` | EP-9 | — | PACK-18 |
| `v1.3` | EP-10・EP-11・EP-12 | `confirm.options[].label` | PACK-20〜PACK-23 |
| `v1.4` | EP-13 | — | PACK-38 |
| `v1.5` | — | `input.revise.candidates`・`input.revise.candidates[].label` | PACK-42 |

- v1.1〜v1.3 は加算だけで、IR の形式の版 `ir_format` は 1 のままである（PACK-24）
- v1.4 も加算だけである。`views` は値を通す欄のため、IR の型は変わらない
- v1.5 も加算だけである。`input` は未型付けの値のまま IR に通るため、IR の型は変わらない。拡張点の番号は持たない（`confirm.options[].label` と同じ）
- 語彙の版の上限は `v1.5` である。それ以降の語彙は、新しい語彙の版の行として版上げで足す

## 9. 仕様との照合表

仕様 04 の PACK-8〜PACK-23 が挙げる語が、7 節に欠けていないことの照合である。試験は、各行の語がすべて 7 節の語の列にあることを確かめる。

| 仕様の ID | 7 節の語 |
|---|---|
| PACK-8 | `step`・`flag`・`world_has`・`world_attr`・`count`・`chose`・`all`・`any`・`not`・`param`・`clarifying` |
| PACK-9 | `put`・`remove`・`replace`・`set_attr`・`set_flag`・`clear_flag`・`set_flag_global`・`processing`・`when` |
| PACK-10 | `command`・`text`・`choice`・`ui_action`・`approval`・`wait`・`cancel` |
| PACK-11 | `accepted`・`rejected`・`clarified`・`unknown_command`・`wrong_order`・`missing_arg`・`bad_arg_shape`・`busy`・`not_in_this_step`・`not_supported`・`free_text_unjudged` |
| PACK-12 | 語なし（診断は TASK-195） |
| PACK-13 | `point_at`・`answer`・`approve`・`none`・`confirm.reason` |
| PACK-14 | `view.card_list`（EP-1〜EP-4 は PACK-10・PACK-8 の `wait`・`cancel`・`param`・`clarifying`） |
| PACK-15 | `confirm.on_wrong`・`confirm.options[].on_wrong` |
| PACK-16 | `input_head`・`nearest_command`・`step_title`・`step_do`・`remaining`・`total`・`do` |
| PACK-17 | `pack.guidance_levels`・`starts[].guidance`・`body_levels`・`hints.levels`・`guidance=A`・`guidance=B`・`guidance=C` |
| PACK-18 | `starts[].requires`・`starts[].unmet` |
| PACK-19 | `use`・`module`・`args`・`params`・`starts` |
| PACK-20 | `pack.profile.command_prefix`・`pack.profile.input_field`・`pack.profile.input_field=shown`・`pack.profile.input_field=hidden` |
| PACK-21 | `pack.appearance.theme`・`pack.appearance.theme=standard`・`pack.appearance.theme=mono`・`pack.appearance.theme=high_contrast`・`pack.appearance.theme=reading` |
| PACK-22 | `pack.left_pane.world_list`・`pack.left_pane.view_tabs`・`pack.left_pane.world_list=shown`・`pack.left_pane.world_list=hidden`・`pack.left_pane.view_tabs=all`・`pack.left_pane.view_tabs=declared`・`pack.left_pane.view_tabs=hidden` |
| PACK-23 | `confirm.options[].label` |
| PACK-38 | `views.body`・`views.body=shown`・`views.body=hidden` |
| PACK-42 | `input.revise.candidates`・`input.revise.candidates[].label` |

## 10. 本文書に入れないもの

- 診断コード（`E-*`）: TASK-195 の一覧が正である。本文書は ID で参照するだけにする
- 受講者の入力の値を経路に差し込む語（PACK-39）: 足さないと決まった（D-50）。限界と回避は `docs/known-gaps.md`
- `move`・ディレクトリ要素（PACK-40）、自然文の内容で規則を分ける語（PACK-41）、進行の間隔の宣言欄（PACK-44）: 検討中、または足さないと決まった項目である
- `pack.vocab` の宣言と使用の一致の検査: 検査は TASK-59 が持つ。`pack.vocab` は宣言の欄であり、語には数えない（PACK-31）
- 判定結果の残り（確認・進行・無視）: 入力判定の契約（TASK-256、JUDGE-3）が持つ
- ビューの種類（EP-5 を除く）: 画面の契約が持つ
- TASK-62〜TASK-72 で決まる語彙（TASK-66 の文書ビューの本文の語と、TASK-62 の「直す」の候補の語は 7 節に足した）: 決まった後に、新しい語彙の版の行として足す

## 付録 A. 固有の語の一覧

汎用名の検査の正である。1 行に 1 語を書く（試験が 1 行 1 語で読む）。照合の値は `部分一致`・`語の一致` のどちらかで、それ以外の値は試験が失敗する。

| 出所 | 語 | 照合 |
|---|---|---|
| 掲示板 | `bbs` | 部分一致 |
| 掲示板 | `board` | 語の一致 |
| 掲示板 | `forum` | 語の一致 |
| 掲示板 | `thread` | 語の一致 |
| 掲示板 | `post` | 語の一致 |
| 掲示板 | `reply` | 語の一致 |
| Claude Code | `claude` | 部分一致 |
| Claude Code | `anthropic` | 部分一致 |
| Claude Code | `skill` | 語の一致 |
| Claude Code | `subagent` | 語の一致 |
| Claude Code | `mcp` | 語の一致 |
| Claude Code | `slash` | 語の一致 |
| GitHub | `github` | 部分一致 |
| GitHub | `gh` | 語の一致 |
| GitHub | `pr` | 語の一致 |
| GitHub | `pull` | 語の一致 |
| GitHub | `issue` | 語の一致 |
| GitHub | `repo` | 語の一致 |
| GitHub | `fork` | 語の一致 |

- 一覧は、誤検出を避けるため意図的に保守的にしている。`commit`・`branch` など git 一般の語は GitHub に固有ではないため入れない
- 一覧を変える（語を足す・照合を変える）ことは、文書の版上げとして扱う

## 変更履歴

| 版 | 日付 | 変更 | 理由 |
|---|---|---|---|
| 1 | 2026-10-15 | 初版。EP-1〜EP-5 を語彙 v1 として固定 | TASK-48、PACK-14 |
| 2 | 2026-10-15 | EP-6〜EP-12・`confirm.options[].label`・語彙 v1〜v1.3 の語の一覧・版と拡張点の対応表を加算。名前の形を 2 段以上に拡張 | TASK-194、PACK-8〜PACK-23 |
| 3 | 2026-10-18 | EP-13（`view.document_body`）と語彙 v1.4 の 3 語（`views.body` と値 2 つ）を加算。TASK-66 1/2 | TASK-66、PACK-38、D-50・D-51 |
| 4 | 2026-10-21 | 語彙 v1.5 の 2 語（`input.revise.candidates`・`input.revise.candidates[].label`）を加算。拡張点の番号なし。TASK-62 2/2 | TASK-62、PACK-42、D-69 |

2026-10-19: 5 節に手続きの文書（TASK-189）への参照を足した。固定した行は変わらないため、文書の版は 3 のまま据え置く。
