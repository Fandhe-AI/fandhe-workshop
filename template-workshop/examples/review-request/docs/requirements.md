# 第 2 教材「レビューの頼み方」の要件の記録

版 1（2026-11-02）。TASK-742（#1973）で作りました。

この文書は、第 2 教材の本実装（TASK-743〜TASK-756）が始まる前に、題材・制約・Step 一覧・学習目標を固定するための記録です。後続のタスクはこの記録を引いて作り、足していきます。特に TASK-755 は AR-8 の読みの記録の確認、TASK-756 は AR-12 の読みの記録の確認と設定値の試験との結びつけを担当します。

## 置き場の決定

| 項目 | 決定 |
|---|---|
| 例のディレクトリ（教材パックの根） | `template-workshop/examples/review-request/` |
| この記録 | `template-workshop/examples/review-request/docs/requirements.md` |
| 教材パックの本体（後続のタスク） | 同じディレクトリの直下（`workshop.yaml`・`chapters/`・`fixtures/`・`tests/`）。このタスクでは作りません |

- 第 2 教材をエンジン repo の雛形の例として置くことは D-91（2026-11-02）で決まり、SECOND-18 が置き場を「雛形のディレクトリの配下の例のディレクトリ」と定めています。名前は着手時に決める扱いだったため、ここで決めます。
- `review-request` は試作の `pack.id` と同じです。公開の例 `docs/examples/<name>/` と同じく、例のディレクトリがそのまま教材パックの根になります。
- パックの根の下に `docs/` を置いても、読み込みには影響しません。`fandhe-pack` が読むのは `workshop.yaml`・`chapters/`・`tests/`・参照される `fixtures/` だけです。
- 限界: TASK-769 が雛形の構成を固定するときに、場所が変わる可能性があります。第 2 教材のタスク群（TASK-743〜TASK-756）のあいだは、この名前で固定します。

## 正の順序

本実装の正は、04 の SECOND-4〜SECOND-12 です。試作の要件（AR-1〜AR-20）と試作のパック（`prototype/packs/review-request/`）は出発点であり、参考です。両者が食い違うときは、04 の記述に従います。

仕様は private の repo にあるため、この記録には本文を転記せず、ID（SECOND-n・D-n・AR-n・TASK-n）で参照します。

## 1. 題材

題材は、レビューを頼むときに、相手・頼む範囲・見てほしい点・期限を選び、依頼文の下書きを整えることです（AR-1・SECOND-5）。

- 場面（架空）: ログイン画面の入力チェックを直した後で、レビューを頼みます。社内固有の情報は含めません。
- AR-1 の読み: 依頼文の下書きは、文書のビューの 5 行の属性（`review/status`・`to`・`scope`・`focus`・`due`。状態・宛先・範囲・見てほしい点・期限）で満たすと読みます（D-31）。この読みは、作者を起動する前には決めておらず、結果を見た後に決めたものです。

## 2. 制約

| 制約 | 対応する AR | 対応する 04 の ID | 本実装での設定値（後続のタスクが書く値） |
|---|---|---|---|
| 選択肢（`choice`）と画面操作（`ui_action`）だけで進む。コマンドも自然文の入力も使わない | AR-2・AR-11 | SECOND-7 | `input_field: hidden` |
| 左は文書のビューだけ。世界状態の一覧とビューの切り替えを出さない | AR-9・AR-10 | SECOND-8 | `world_list: hidden`・`view_tabs: hidden`・全 Step が `views: [v-doc]`・ビューの種類は `document` だけ |
| コマンドを持たない | AR-12 | SECOND-9・SECOND-22 | `command_prefix: ""`・`commands: {}` |
| Claude Code 風の外観を使わない | AR-13 | SECOND-10 | `pack.appearance.theme: reading`（見た目の人の評価は SECOND-16 で検討中） |
| GitHub 風のビュー・6 工程・2 周・掲示板・シェル風の操作を使わない | AR-14〜AR-18 | SECOND-11 | 章 1 つ・`starts` 1 つ・掲示板の語彙なし |
| 形式 B で書く | AR-19 | SECOND-4 | `workshop.yaml`・`chapters/<章>/chapter.yaml`・本文の Markdown・`tests/playthrough.yaml`・練習データ |
| 置き場 | AR-20 | D-91・SECOND-18 | 「置き場の決定」の例のディレクトリ（試作の場所は製品の置き場ではない） |
| 所要は 10 分程度 | AR-3 | SECOND-13 | 本文に書くだけ（検討中。測らない） |
| 誤答への応答を 3 種以上の場面で使い、拒否の分類の名前を書く | AR-6・AR-7 | SECOND-12 | 「6. 誤答の 8 場面」 |
| Step は 6〜10、学習目標は 2 つ | AR-4・AR-5 | SECOND-4 | Step 8 つ・学習目標 2 つ |

AR-1〜AR-20 の要件の文面は、この記録では転記せず、対応の一覧にとどめます。

## 3. 学習目標（2 つ。AR-5・SECOND-4）

| ID | 学習目標 |
|---|---|
| LO-1 | 読む人が迷わないように、宛先と頼む範囲を絞れる |
| LO-2 | 見てほしい点と期限を、1 つに決まる形で書ける |

## 4. Step 一覧（8 つ。AR-4 の 6〜10 の範囲・SECOND-4）

試作のパックを出発点にします。「入力」は `input.kind`、「確認」は `confirm.kind` です。

| ID | 題 | 入力 | 確認 | LO | 変わる行 | 04 の ID |
|---|---|---|---|---|---|---|
| st-1 | 状況を確認する | `choice`（1 択「下書きを作り始める」） | `answer` | LO-1・LO-2 | `review/status` | SECOND-4・SECOND-7・SECOND-12 |
| st-2 | 宛先を選ぶ | `choice` | `point_at` | LO-1 | `to`・`status` | SECOND-5・SECOND-6・SECOND-12 |
| st-3 | 頼む範囲を選ぶ | `choice` | `answer` | LO-1 | `scope`・`status` | SECOND-5・SECOND-6・SECOND-12 |
| st-4 | 見てほしい点を選ぶ | `choice` | `answer` | LO-2 | `focus`・`status` | SECOND-5・SECOND-6・SECOND-12 |
| st-5 | 空いている行を探す | `ui_action` | `answer` | LO-2 | `due` | SECOND-7・SECOND-12 |
| st-6 | 期限を選ぶ | `choice` | `point_at` | LO-2 | `due`・`status` | SECOND-5・SECOND-6・SECOND-12 |
| st-7 | 送る前に見直す | `choice` | `answer` | LO-1・LO-2 | `status`（見直し済み） | SECOND-6・SECOND-12 |
| st-8 | 送る（最終 Step） | `choice` | `answer` | LO-1・LO-2 | `status`（送信可） | SECOND-6・SECOND-12 |

注: 全 Step が SECOND-4・SECOND-8（`views: [v-doc]`）・SECOND-11 に当たります。st-1 は開始の入口（SECOND-7 の「選択肢のボタン 1 つ」）、st-5 は画面操作（`ui_action`）の Step です。

## 5. 宛先・範囲・見てほしい点・期限を選ぶ Step（SECOND-5）

宛先は st-2、範囲は st-3、見てほしい点は st-4、期限は st-6 です。選ぶたびに該当の行の内容が変わり（SECOND-6）、変わった行には変化の印が付きます。

## 6. 誤答の 8 場面（SECOND-12・AR-7）

| # | 場面 | Step | 拒否の分類 |
|---|---|---|---|
| 1 | 確認の誤答の解説 | st-1・st-3・st-4・st-5・st-7・st-8（`answer`） | `not_in_this_step` |
| 2 | 規則による拒否（チーム全員） | st-2 | `not_in_this_step` |
| 3 | `point_at` の外れ | st-2・st-6 | `not_in_this_step` |
| 4 | `ui_action` で埋まった行を押す | st-5 | `not_in_this_step` |
| 5 | 条件つきの拒否（差分と関連テストの後の「明日の 17 時まで」） | st-6（`when: chose`） | `not_in_this_step` |
| 6 | 「できるだけ早く」 | st-6 | `bad_arg_shape` |
| 7 | 見直さずに送る | st-7 | `not_in_this_step` |
| 8 | 「直す」 | st-8 | `wrong_order` |

- 拒否のあと、左の行は変わらないことを前提にします。確かめるのは TASK-746・TASK-750 です。
- 選択肢だけで進む教材では、当たる拒否の分類が限られます。

## 7. 要件の読みと日付

| 要件 | 読み | 決定 | 記録 | 観察 |
|---|---|---|---|---|
| AR-1 | 5 行の属性で満たす | D-31 | 結果を見た後に決めた読み | SECOND-5 |
| AR-8 | 文書のビューに出る変化（属性の変化と変化の印）を含むと読み、本文の表示は求めない | D-93（2026-11-02）・SECOND-21 | AR-8 満たした | SECOND-6（14 入力） |
| AR-12 | コマンドを持たない形（`command_prefix: ""`・`commands: {}`）で満たすと読む | D-93（2026-11-02）・SECOND-22 | AR-12 満たした（解釈つき） | SECOND-9 |

- AR-8 の「題の変化」は、14 入力では観察していません。作者の自己申告「部分的（G-4・H-3）」とは、読みの違いとして残します。
- AR-12 の設定値を固定する試験は TASK-756 が担当します。
- G-4・H-3 は忠実さの限界です（`docs/known-gaps.md`）。G-4 は語彙 1.4（EP-13）で本文を出せるようになりましたが、第 2 教材を本文を出す形に書き直すか（SECOND-14）は未決で、TASK-755 の担当とします。

## 8. 照合の記録（2026-11-02）

- [x] SECOND-4: Step が 8 つ、学習目標が 2 つ。形式 B で書く
- [x] SECOND-5: 宛先（st-2）・範囲（st-3）・見てほしい点（st-4）・期限（st-6）を選ぶ Step がある
- [x] SECOND-12: 8 場面がすべて Step に割り当たっている（「6. 誤答の 8 場面」）
- [x] AR-8・AR-12 の読みと決定の日付を書いた（「7. 要件の読みと日付」）
- [x] 試作の `chapter.yaml` の Step の ID・題・`input.kind`・`confirm.kind`・`lo`・最終 Step と突き合わせた
