# 第 2 教材「レビューの頼み方」の要件の記録

版 6（2026-11-03）。TASK-742（#1973）で作りました。TASK-745 で Step 群と模範解答を、TASK-746 1/2 で誤答の 8 場面を、TASK-746 2/2 で誤入力の通しと文面の確認を足しました。

この文書は、第 2 教材の本実装（TASK-743〜TASK-756）が始まる前に、題材・制約・Step 一覧・学習目標を固定するための記録です。後続のタスクはこの記録を引いて作り、足していきます。特に TASK-755 は AR-8 の読みの記録の確認、TASK-756 は AR-12 の読みの記録の確認と設定値の試験との結びつけを担当します。

## 置き場の決定

| 項目 | 決定 |
|---|---|
| 例のディレクトリ（教材パックの根） | `template-workshop/examples/review-request/` |
| この記録 | `template-workshop/examples/review-request/docs/requirements.md` |
| 教材パックの本体（後続のタスク） | 同じディレクトリの直下（`workshop.yaml`・`chapters/`・`fixtures/`・`tests/`）。TASK-743 で骨格（`workshop.yaml`・`chapters/ch-request/`・`tests/playthrough.yaml`）を置きました。残りは後続のタスクが足します。TASK-744 で `fixtures/draft.md` と文書のビューの 5 行（`review/status`・`review/to`・`review/scope`・`review/focus`・`review/due`）を足しました |

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
- AR-12 は、コマンドを持たない形（`command_prefix: ""`・`commands: {}`）で満たすと読みます。決定は D-93（2026-11-02。TASK-7）で、記録は「AR-12 満たした（解釈つき）」です。
- 採らなかった読みは、`/` 以外の接頭辞を実際に使う読みです。採っていれば `commands` に 1 件以上が要り、SECOND-7（入力欄なし）・SECOND-9（コマンドなし）の期待が成り立たなくなります。採った読みでは SECOND-7・SECOND-9 への影響は無く、教材は変えていません。
- AR-12 の設定値は `make check-second-pack` の項目 `profile`・`commands`・`ar12-reading` が固定します。`ar12-reading` は、この表の行と `workshop.yaml`・IR の設定値の一致を見ます。読みの是非は人の決定（D-93）に依り、試験が確かめるのは記録と設定値の一致だけです。
- G-4・H-3 は忠実さの限界です（`docs/known-gaps.md`）。G-4 は語彙 1.4（EP-13）で本文を出せるようになりましたが、第 2 教材を本文を出す形に書き直すか（SECOND-14）は未決で、TASK-755 の担当とします。
- TASK-744 では G-4・H-3 を 5 行の属性の形で受け入れました（D-31）。初期の属性は `title`・`state`・`detail` の 3 つで、行の中身の書き換えは TASK-745 が担当します。

## 7a. TASK-745 の記録（2026-11-02）

- st-2〜st-8 の正しい経路を書きました。各 Step は選ぶたびに `review/to`・`scope`・`focus`・`due` の `detail`・`state` を `set_attr` で変え、`review/status` の `detail`・`state` を段階ごとに変えます。誤答の選択肢・規則による拒否・`when: chose` は TASK-746 の担当です。
- 模範解答 `pt-main`・`pt-alt` は各 16 入力で、宛先・範囲・見てほしい点・期限・見直しの 5 か所で別の選び方をします。終わりの宛先と期限の行は別の文面になります。
- 「14 入力」の読み: 各模範解答の 16 入力のうち、5 行の属性を変える入力が 14 と読みます（データ側の数え方）。試作の 14 は画面の観察の数え方で、そのままは写しません。
- `fandhe-pack test` は `asserts` を判定しないため（AUTHOR-4）、行の変化の期待は `tests/row-asserts.json` に置き、`scripts/check-second-pack.sh` が IR と照合します。`test` が `asserts` を判定するようになったら `tests/playthrough.yaml` へ移します。
- 限界: `test` も画面も開始点の `effects` を適用しないため、実行時の世界状態では行の変化を観察できません。画面の確認は TASK-749 です。
- 件数の基準値は `tests/baseline.txt` に置きます。

## 7b. TASK-746 1/2 の記録（2026-11-02）

- 8 場面の規則と文面を足しました。対応は次のとおりです（`chapter.yaml` の規則・`workshop.yaml` の文面）。
  - 場面 1: st-1・3・4・5・7・8 の `correct: false` の選択肢 11 個の `on_wrong`（`rj-q1-*`・`rj-q3-*`・`rj-q4-*`・`rj-q5-*`・`rj-q7-*`・`rj-q8-unsent`）
  - 場面 2: st-2 の `r-all`（`rj-to-all`）
  - 場面 3: st-2・st-6 の `on_miss`（`rj-miss-to`・`rj-miss-due`。TASK-745 で既に当たっている）
  - 場面 4: st-5 の `r-ui-to`・`r-ui-scope`・`r-ui-focus`（`rj-row-filled`）
  - 場面 5: st-6 の `r-tomorrow-large`（`when: chose st-3 opt-diff-tests`。`rj-due-short`。`r-tomorrow` より前）
  - 場面 6: st-6 の `r-asap`（`rj-due-vague`。`bad_arg_shape`）
  - 場面 7: st-7 の `r-skip`（`rj-skip`）
  - 場面 8: st-8 の `r-revise`（`rj-revise`。`wrong_order`）
- 拒否の規則は `effects` を持たず、拒否のあと左の行は変わりません（データ上の宣言。エンジンの拒否も状態を変えません）。
- 試作にあって 8 場面に入らない拒否（`opt-boss`・`opt-repo`・`opt-everything`・`opt-taste`・ヒント）は足していません。正は SECOND-12 の 8 場面です。
- 限界: 場面 5 は、今のエンジンが入力の判定で `when` を評価しないため、宣言だけで動きません（`docs/testing.md` 5.443）。
- 2/2 への申し送り（試作の `expect_rejection` の欄が `E-PARSE` になること）は 7c で扱いました。

## 7c. TASK-746 2/2 の記録（2026-11-02）

- `tests/playthrough.yaml` の `error_inputs` に 27 件を書きました。場面 1（確認の誤答 11 件）・2・3（`point_at` の外れ 3 件）・4（3 件）・6・7・8 と、8 場面の外の補助 5 件（宣言外の行・他の Step の選択肢・確認の形に合わない入力・取り消し・拒否の後の選び直し）です。すべて `test` が期待どおりの分類で通します。
- 文面の確認は `tests/text-checks.json` に置きました。`fandhe-pack test` の `text_checks:` を書くと終了コードが必ず 1 になるためです。`test` が判定できるようになったら `playthrough.yaml` へ移し、このファイルを消します。
- 場面 5 は誤入力に入れていません。今のエンジンは入力の判定で規則の `when` を評価しないため、`r-tomorrow-large` の入力が拒否にならず、受け付けられます。誤った挙動を正として固定しないよう、`text-checks.json` の `excluded` に理由コード `engine-when-not-evaluated` で記録し、宣言は `wrong-scenes` が固定します。エンジンの結線（Rust の変更）は W-20 に反するため、ここでは直しません。扱いは人の判断です。
- 文面の確認は IR 上の文面と、エンジンの文面の選び方を写した静的な判定（`scripts/second-pack-judge.jq`）の照合です。実行時にエンジンが返した文面ではありません。

## 7d. TASK-747 の記録（2026-11-03）

- 基準の IR を `tests/ir.json` に置いた。`fandhe-pack compile <pack> -o tests/ir.json` の出力そのもので、手で編集しない。教材を変えたら同じコマンドで書き直し、`tests/baseline.txt` の `ir.bytes` も直す
- `scripts/check-second-pack.sh` の `ir-match`・`format-b`・`learning-goals` が、IR のバイト一致・形式 B の構成・学習目標 2 つの一致を確かめる。CI の教材テスト（TASK-102）は `FANDHE_PACK_BIN` を渡してこのスクリプトを呼べばよい
- 仕様の PoC-5 の値（誤入力 28/28・入力の総数 503）と、この教材の値（27/27・235）は違う。場面 5 を `engine-when-not-evaluated` で除外した設計どおりで、基準値は `tests/baseline.txt` である

## 8. 照合の記録（2026-11-02）

- [x] SECOND-4: Step が 8 つ、学習目標が 2 つ。形式 B で書く
- [x] SECOND-5: 宛先（st-2）・範囲（st-3）・見てほしい点（st-4）・期限（st-6）を選ぶ Step がある
- [x] SECOND-12: 8 場面がすべて Step に割り当たっている（「6. 誤答の 8 場面」）
- [x] AR-8・AR-12 の読みと決定の日付を書いた（「7. 要件の読みと日付」）
- [x] TASK-756: AR-12 の行の読み・日付と `workshop.yaml`・IR の設定値が一致する（`ar12-reading`。2026-11-02）
- [x] TASK-747: `compile` の IR が `tests/ir.json` とバイト一致し、形式 B の構成・学習目標 2 つが一致する（`ir-match`・`format-b`・`learning-goals`。2026-11-03）
- [x] TASK-753: 使った拡張点が公開の一覧にあり、掲示板教材の写しとの重なりが EP-6・EP-7 だけで、拒否の文面の一致が 0 件（`ep-used`・`ep-public`・`ep-record`・`label-field`・`board-ep-overlap`・`board-rejections`・`no-rounds`・`input-kinds`。2026-11-03）
- [x] 試作の `chapter.yaml` の Step の ID・題・`input.kind`・`confirm.kind`・`lo`・最終 Step と突き合わせた

## 7e. TASK-413 1/2 の記録（2026-11-03）

- 公開の資料だけで別の作者が書いた第 2 教材の再現を、`docs/second-pack-repro/` に置いた。この教材パックは変えていない
- 作者の写しは `validate` 0 件・`test` 模範解答 2/2・誤入力 8/8（入力の総数 107）。場面 5 を通せない理由は 7c と同じ（`SECOND-12（条件つきの拒否）`）で、文面の確認は `test` が判定しないため未判定（AUTHOR-4）
- 記録は `docs/testing.md` の 5.449

## 7f. TASK-413 2/2 の記録（2026-11-03）

- 作者の写しの IR と再 `compile` の IR のバイト一致を `docs/second-pack-repro/` の検査に足し、書けなかった箇所を `docs/known-gaps.md` に記録した。この教材パックは変えていない
- 記録は `docs/testing.md` の 5.451

## 7g. TASK-753 の記録（2026-11-03）

使った拡張点は次の 6 つです。`fandhe-pack vocab` の出力と一致させ、`scripts/check-second-pack.sh` の `ep-record` が集合の一致を確かめます。EP-9 は `vocab` が数えるので、YAML の検索はしません。

| ID | 名前 | 語彙の版 | 使った所 |
|---|---|---|---|
| EP-6 | `confirm.answer.option_on_wrong` | v1.1 | st-1・st-3・st-4・st-5・st-7・st-8 |
| EP-7 | `text.slot` | v1.1 | 全 8 Step の `do` |
| EP-10 | `pack.profile` | v1.3 | 教材パック全体 |
| EP-11 | `pack.appearance` | v1.3 | 教材パック全体 |
| EP-12 | `pack.left_pane` | v1.3 | 教材パック全体 |
| `confirm.options[].label` | 選択肢の表示名 | v1.3 | 確認の選択肢すべて（拡張点の番号を持たず、`vocab` は数えない） |

- 公開の一覧は `docs/vocabulary/vocabulary.md` の 2 節で、現在は EP-1〜EP-13 です。使った 5 つはすべて含まれます。EP-13 は使いません（語彙 1.3）
- 比べる相手は、掲示板教材の公開の写し `testdata/module-reuse-rounds/test-data/pack` にしました。private の仕様 repo の掲示板教材の文面は、この repo に写しません。教材 repo の掲示板教材ができたら `--board-pack` で比べ直します
- 実測（2026-11-03）: 拒否の文面の異なりは 51 件（この教材）と 66 件（写し）で、一致は 0 件です。重なる拡張点は EP-6・EP-7 だけです。基本の語彙は入力の種類 `choice`・`ui_action` とビューの種類 `document` などが重なり、写しにだけ `command`・`text` などがあります
- 限界: 自動の照合が覆うのは写しにある文面だけです。仕様 repo の掲示板教材との手元の一度きりの照合（2026-11-03・仕様 repo の commit `29bfe1c`）でも一致は 0 件でしたが、試験にはしていません。`docs/examples/choice-only-pack` とは 6 件が一致します。これは SECOND-2・SECOND-17 が認める限界なので、不合格にしません
- 記録は `docs/testing.md` の 5.453
