# 第 3 教材「タスクの分解と進行管理」の記録

- 文書の版: 3
- 実施日: 2026-11-03
- 対応するタスク: TASK-763 1/3（#2011）と 2/3（#2012。章立てと Step。7 節）。3/3（#2013。エンジンの差分 0 の確認と書けなかった箇所。8 節）もこの記録に追記しました
- 仕様は private のため、本文を転記せず ID（PACK-22・SECOND-6・ART-17・H-3・AUTHOR-7 など）で参照します

この記録の目的は、3 本目の教材が左エリアに使う `task_tree`（タスクの木）と `card_list`（カードの一覧）のビューが、今のエンジンでどこまで動くかを、エンジンを変えずに確かめた結果を残すことです。3 本目は性質の違う教材（行を指す操作が中心）として、公開の資料と CLI だけで書きます。

## 1. 置き場の決定

| 項目 | 決定 | 理由・注 |
|---|---|---|
| ディレクトリ名 | `template-workshop/examples/task-breakdown/` | 題材を表す英小文字とハイフンだけの名前で、第 2 教材 `review-request` と同じ並びにそろえる |
| 本体の `pack.id`（予定） | `task-breakdown` | #2012 で本体を書くときに使う。確認用の `pack.id` は `task-breakdown-probe` |
| 確認用の教材パック | `task-breakdown/probes/views/` | 独立した教材パックの根。根の `task-breakdown/` には #2012 が本体を書く |
| 画面の確認の設定 | `tools/browser/choice-journey/task-breakdown-views.json`（と `task-breakdown-tree.json`・`task-breakdown-cards.json`） | 既存の道具（`make check-choice-journey`）に設定を足しただけで、道具のコードは変えていない |

- 根に本体を置いて `validate`・`test` を流すと、`probes/` は読まれません（章 1・Step の定義 8 と出て、確認用パックの章を数えない）。2/3 で確かめました（7 節）
- 置き場は TASK-769 で変わる可能性があります

## 2. 確かめに使った資料

エンジンのソース（`crates/**`）は開いていません。使ったのは次の公開の資料・例・道具の設定だけです。

- 資料: `docs/views.md`・`docs/pack-authoring.md`・`docs/schema-format-b.md`・`docs/cli-guide.md`・`docs/cli.md`・`docs/known-gaps.md`・`docs/diagnostics.md`・`docs/testing.md`（5.437・5.447）
- 例: `docs/examples/choice-only-pack`・`template-workshop/examples/review-request`
- 道具の設定: `tools/browser/choice-journey/review-request-derived.json`（形式の手本）、`tools/browser/choice-journey-config.mjs` の照合の部分（どの欄が何を測るかを知るため。教材側の道具のコードで、エンジンではない）

## 3. 確認用の教材パック

`probes/views/` は 4 つの Step で、2 つのビューそれぞれの行を指します。

| Step | ビュー | 入力 | 確認 | 確かめること |
|---|---|---|---|---|
| `st-start` | `v-tree`（`task_tree`） | 選択肢 1 つ | 答え | 行を置く（理由は 5 節の限界 1） |
| `st-tree` | `v-tree` | 画面操作。子の行は受付、親の行は拒否 | `point_at` | 木の行を指して受付・拒否・確認まで届くか |
| `st-cards` | `v-cards`（`card_list`） | 画面操作。未着手のカードは受付、完了のカードは拒否 | `point_at` | カードの行を指して受付・拒否・確認まで届くか |
| `st-wrap` | `v-tree` | 選択肢 1 つ | 答え | 終了の時点のビューをそろえる |

設定値は、`world_list: shown`（`hidden` では行が 0 件になり指せない。PACK-22）、`view_tabs: hidden`（Step ごとにビューを 1 つにする）、`input_field: hidden`、`vocab: "1.3"` です。行は 5 つで、木が `plan/launch`（親）・`plan/launch/design`・`plan/launch/build`（子）、カードが `board/design`（未着手）・`board/build`（完了）です。属性は `title`・`state`・`detail` を置きました。題材は架空の作業です。

## 4. 確認の結果

使ったコマンド（いずれも 2026-11-03、手元の Linux、`fandhe-pack` は debug ビルド）と最終行・判定は次のとおりです。

| コマンド | 結果 |
|---|---|
| `fandhe-pack validate template-workshop/examples/task-breakdown/probes/views` | `OK: 診断 0 件（形式 B(YAML)、章 1、モジュール 0、Step の定義 4）`、終了コード 0 |
| `fandhe-pack test` 同上 | 模範解答 1/1 完走、誤入力 2/2 が期待どおり、入った Step の実体 4/4、終了コード 0 |
| `node --test tools/browser/choice-journey-config.test.mjs` | 172 件合格 |
| `make check-choice-journey CHOICE_JOURNEY_CONFIG=<設定>`（`SKIP_WASM_OPT=1`） | 設定 3 本それぞれが CSP あり・なしの 2 通りとも合格（`completed=true`、行 5 件、外部通信 0、ページ内エラー 0） |

根拠の種類は、CLI（`validate`・`test`）と、自動ブラウザ（`check-choice-journey`。DOM だけの観察。人は画面を見ていない）です。

### 4.1 `task_tree`

| 区分 | 結果 | 確かめた方法・根拠の種類 |
|---|---|---|
| 書ける | `views` に `kind: task_tree` を宣言し、Step の `views` と宛先 `targets` に使える。診断 0 件 | `validate`（CLI） |
| 表示される | 経路の行 5 件と、変化の印の記号と文字が出る。切り替えのボタンは 0 件。設定 `task-breakdown-tree`（両方のビューを `task_tree` に差し替えた派生）で、`data-view` は全時点で `task_tree` のみ | 自動ブラウザ（`visibleViews`・`rowPaths`・`leftSwitchAbsent`・印の照合） |
| 指せる | 子の行を指すと受付になり、確認の `point_at` で達成になる。親の行を指すと拒否になる（`rejected:not_in_this_step`）。実クリックの `row` 操作が全時点で届く | `test`（CLI）。自動ブラウザ（`stopped=none`） |
| 書けない・表示されない | 親子の入れ子、行の文字（`title`・`state`・`detail`）が画面に出るかは、確かめていない（未確認。道具は経路・印・記号だけを測る） | 資料の記述（`docs/views.md` の 1 節。製品の画面は種類ごとの描画部品へ未結線）。AI による画面の観察は行っていない |

### 4.2 `card_list`

| 区分 | 結果 | 確かめた方法・根拠の種類 |
|---|---|---|
| 書ける | `kind: card_list` を宣言し、使える。診断 0 件 | `validate`（CLI） |
| 表示される | 経路の行 5 件と印が出る。設定 `task-breakdown-cards`（両方のビューを `card_list` に差し替えた派生）で、`data-view` は全時点で `card_list` のみ | 自動ブラウザ |
| 指せる | 未着手のカードは受付、完了のカードは拒否、確認の `point_at` で達成。実クリックが届く | `test`（CLI）。自動ブラウザ |
| 書けない・表示されない | カードの状態（`state`）の見た目、カード固有の部品は、確かめていない（未確認。4.1 と同じ理由） | 資料の記述 |

補足として、2 つのビューを 1 つの教材で順に使う設定 `task-breakdown-views` も合格しました。この設定は `visibleViews` を照合していません（道具が全時点で同じ集合を要求するため）。そのため、混在の教材で各 Step に見えるビューの種類は、`task-breakdown-tree`・`task-breakdown-cards` の 2 つの派生で個別に確かめています。

## 5. 書けない範囲と限界

### 5.1 否定の確認

`mktemp -d` の中に確認用パックの写しを作り、1 か所ずつ変えて `validate` を流しました（写しは消しました）。

| 変えた箇所 | 結果 |
|---|---|
| `kind` の綴りの誤り（`task-tree`・`cardlist`） | `validate` は診断 0 件、`compile` も終了コード 0。綴りの誤りは止まらない（`docs/views.md` の 2 節のとおり）。画面側が完全一致で読み、一覧にない綴りは黙って飛ばす |
| `view_tabs: hidden` で、1 つの Step が `views: [v-tree, v-cards]` を宣言 | `E-LEFT-MULTI-VIEW`（宣言した Step の数だけ） |
| 宛先のビューを、その Step の画面に出ないビューにする | `E-LEFT-TARGET-VIEW` |
| `put` していない経路を宛先にする | `E-LEFT-TARGET-ROW` |
| `view_tabs: declared` にして 2 つのビューを宣言 | 診断 0 件（宣言そのものは書ける。画面での切り替えの観察は、していない） |
| `world_list: hidden` | 診断 0 件（`validate` は止めない。画面では行が 0 件になり指せない。PACK-22。この写しを画面では流していない） |

### 5.2 限界

1. 開始点（`starts`）の `effects` は画面が適用しない。最初に `check-choice-journey` を流したとき、行が 0 件のまま `main/st-tree:entered` で止まった（`no-target`）。このため、行を最初の Step の受付の `effects` で置く形にした。`docs/known-gaps.md` の PACK-22 の欄にも同じ限界の記述がある（TASK-217 が未着手）。3 本目の本体も、行は最初の選択肢の受付で置く
2. `kind` の綴りの誤りが `validate`・`compile` で止まらない。既存の ID は見つけていない。#2013 で `docs/known-gaps.md` に足す候補
3. 製品の画面は、7 種とも種類ごとの描画部品に結線されていない（`docs/views.md` の 1 節）。行の文字・入れ子・カードの状態の見た目は、この確認では観察していない。SECOND-6（行の文面）・ART-17 と同じ形になる可能性が高いが、未確認
4. 道具は、1 つの設定の中で全時点に同じ `visibleViews` を要求する。Step ごとにビューが違う教材は、1 設定では照合できない（上の補足の迂回を使った）
5. 人は画面を見ていない。試験と教材の作成者が同じ AI である。Playwright MCP による行の中身の観察（preview）は、していない

## 6. #2012 への申し送り

- 使える形: 1 Step に 1 ビュー、`world_list: shown`、`view_tabs: hidden`、宛先は `put` した経路、行は最初の選択肢の受付の `effects` で置く、親の行は拒否・子の行は受付で `set_attr` する、確認は `point_at`
- 避ける形: 開始点の `effects` に頼る、`world_list: hidden`、1 Step に複数のビューを宣言（`view_tabs: hidden` のとき）、ビューの切り替えを前提にした Step
- 行の属性（`title`・`state`・`detail`）の名前は、画面が読む約束がまだ文書に無かった（`docs/views.md` の 4 節）。行の文面に頼らず、状態は Step の本文と拒否の文面でも伝える（2026-11-05 の追記: 版 5 の 4 節に仮置きの読み方が入った。ただし日本語の状態語は閉じた語彙の外でバッジが「不明」になる（`docs/known-gaps.md`）ため、この対応は続ける）
- 根の `validate` が `probes/` を無視することは、2/3 で確かめた（7 節）

## 7. 章立てと Step（TASK-763 2/3。#2012）

本体の教材パックを根（`template-workshop/examples/task-breakdown/`）に書きました。エンジン（`crates/`・`Cargo.toml`・`Cargo.lock`）は変えておらず、ソースも開いていません。使った資料は 2 節と同じ範囲です。

### 7.1 題材の場面と学習目標

架空の作業「アプリの公開」を、小さなタスクに分け、状態（未着手・進行中・完了）を 1 つずつ動かす場面です。

| ID | 学習目標 |
|---|---|
| LO-1 | 大きな作業を、一度に終えられる大きさのタスクに分け、ほかに頼らない順に並べられる |
| LO-2 | 進行中のタスクを絞りながら、タスクの状態（未着手・進行中・完了）を順に動かせる |

### 7.2 章立てと設定値

| 項目 | 決定 | 理由 |
|---|---|---|
| 章 | 1 つ（`ch-tasks`）。開始点も 1 つ（`s-1`） | 開始点の `effects` を画面が適用しない（5.2 の限界 1）ため、2 つ目の開始点で行を作れない。章をまたぐ世界状態の引き継ぎは未確認の領域のため入れない |
| Step 数 | 8 | 第 2 教材にそろえる |
| `pack.id`・題 | `task-breakdown`・「タスクの分解と進行管理」 | 1 節の予定どおり |
| 設定値 | `guidance: A`・`vocab: "1.3"`・`command_prefix: ""`・`input_field: hidden`・`commands: {}`・`world_list: shown`・`view_tabs: hidden`・外観は省略 | 確認用パックと同じ。行を指すため `world_list` は `shown` |
| ビュー | `v-tree`（`task_tree`）・`v-cards`（`card_list`）。1 Step に 1 ビュー | `E-LEFT-MULTI-VIEW` を避ける |
| 行 | `plan/release`（親）と子 3 行 `plan/release/data`・`screen`・`publish`。カードも同じ経路の行を指す | 下の 7.5 |
| 練習データ | `fixtures/tasks.md`（【架空】の印つき） | `E-FICTIONAL-MARK` |

### 7.3 Step 一覧

「受付」は受付の規則の `effects` による行の変化、「完了条件」は `confirm` です。拒否はすべて分類 `not_in_this_step` で、Step ごとに個別の文面（`message` と `next_action`）を持ちます。ヒントは全 Step に 1〜2 段（段 1 は考え方、段 2 は答えに近い指示）です。

| ID | 題 | ビュー | 入力 | 受付と状態変化 | 拒否 | 完了条件 | LO |
|---|---|---|---|---|---|---|---|
| st-1 | 大きな作業を確かめる | `v-tree` | `choice` 1 択 | 4 行を `put`（親は「未分解」、子は「未着手」） | なし | `answer`。誤答 1 | LO-1 |
| st-2 | 作業を分ける | `v-tree` | `choice` 3 択 | 3 つに分ける: 親を「分解済み」にする | 分けない、全部並べる | `answer`。誤答 1 | LO-1 |
| st-3 | 最初のタスクを決める | `v-tree` | `ui_action` 4 行 | data を指す: data の説明を「最初に始めます」にする | screen・publish・親 | `point_at` data | LO-1 |
| st-4 | カードを進行中にする | `v-cards` | `ui_action` 2 行 | data を指す: 「進行中」にする | screen | `answer`。誤答 1 | LO-2 |
| st-5 | 終わったカードを完了にする | `v-cards` | `ui_action` 2 行 | data を指す: 「完了」にする | screen（未着手） | `point_at` data | LO-2 |
| st-6 | 次のカードを進行中にする | `v-cards` | `ui_action` 3 行 | screen を指す: 「進行中」にする | publish、data（完了済み） | `point_at` screen | LO-2 |
| st-7 | 残りを木で確かめる | `v-tree` | `ui_action` 4 行 | screen を指す: 「完了」にし、親の説明を「残りは公開だけです」にする | publish、親、data | `answer`。誤答 1 | LO-1・LO-2 |
| st-8 | ふりかえる（`final`） | `v-tree` | `choice` 1 択 | なし | なし | `answer`。誤答 1 | LO-1・LO-2 |

行を指す `ui_action` は st-3〜st-7 の 5 つで、行を指す操作が中心です。`choice` は行を置く st-1、分け方を選ぶ st-2、読むだけの st-8 に限りました。誤答の場面は 18 件で、拒否の規則が 12（st-2 に 2、st-3 に 3、st-4・st-5 に各 1、st-6 に 2、st-7 に 3）、確認の誤答が 5（st-1・2・4・7・8）、`point_at` の外れが 1（st-3）です。すべて `error_inputs` に 1 件ずつ書き、`test` が 18/18 で合格しました。

### 7.4 H-3 と文面

選んだ値を文中・経路・属性に差し込めない（H-3）ため、行ごとに規則を列挙しました。行の属性（`title`・`state`・`detail`）を画面が読む約束が書いた時点では文書に無かった（`docs/views.md` の 4 節）ため、状態の変化は各 Step の本文と拒否の文面にも書きました。2026-11-05 に版 5 の 4 節へ仮置きの読み方が入りましたが、日本語の状態語は閉じた語彙の外でバッジが「不明」になる（`docs/known-gaps.md`）ため、この書き方は続けています。親を完了にする Step は設けていません（子の「公開する」が最後まで未着手のため、親は「分解済み」のままです。st-7 は、子が残るうちは親を完了にしないことを答えさせます）。

### 7.5 確かめた未確認点

| 点 | 結果 |
|---|---|
| 根に `probes/` があっても `validate`・`test` が読まないか | 読まない。章 1・Step の定義 8 と出て、確認用パックの章を数えない。`probes/views` を単独で流した結果も 1/3 と同じ（診断 0 件・4/4） |
| カードの宛先に木と同じ経路を使えるか | 使える。診断 0 件。`test` も全件合格 |
| 規則の `effects` で置いた行を、後の Step の規則の `set_attr` が見るか | 見る（`validate`・`test` と画面の通しで確認）。ただし `test` は `set_attr` の値を照合しないため、効果の値の正しさは試験で守れていない（7.7） |
| `on_achieved`・`next[].when` が置いた行を見るか | 使っていない（未確認のまま。#2013 への申し送り） |

### 7.6 実行したコマンドと結果

日付 2026-11-03、手元の Linux、`fandhe-pack` は debug ビルドです。

| コマンド | 結果 |
|---|---|
| `fandhe-pack validate template-workshop/examples/task-breakdown` | `OK: 診断 0 件（形式 B(YAML)、章 1、モジュール 0、Step の定義 8）`、終了コード 0 |
| `fandhe-pack test` 同上 | 模範解答 1/1 完走、誤入力 18/18 が期待どおり、入力の総数 164、R-22 の確認なしで達成 0 件、R-21 の「次の行動」が空 0 件、入った Step の実体 8/8、終了コード 0 |
| `fandhe-pack validate`・`test` を `probes/views` に | 診断 0 件、4/4（1/3 と同じ） |
| `make check-choice-journey CHOICE_JOURNEY_CONFIG=tools/browser/choice-journey/task-breakdown.json`（`SKIP_WASM_OPT=1`） | CSP あり・なしとも合格（`completed=true`、行 4 件、外部通信 0、ページ内エラー 0） |
| `node --test tools/browser/choice-journey-config.test.mjs` | 成功 |

根拠の種類は CLI（`validate`・`test`）と自動ブラウザ（DOM だけの観察）です。人は画面を見ていません。

### 7.7 限界と #2013 への申し送り

- 行の文字（`title`・`state`・`detail`）が画面に出るか、`task_tree` の入れ子、カードの状態の見た目は未確認（5.2 の限界 3）。画面の通しは経路と印しか測らない
- `fandhe-pack test` は効果の値（`set_attr`）を照合しない。語彙の検査も `rules[].outcome.effects` を通らない（`docs/schema-format-b.md` の 6 節）。このため状態の遷移の正しさは、書いた文面との突き合わせが人手になる
- `starts[].effects` を画面が適用しない（PACK-22・TASK-217）ことを避けた。章をまたぐ形、`on_achieved`、`next[].when` は使っていない
- 1 つの設定では Step ごとのビューの種類を照合できない（5.2 の限界 4）。この設定も `visibleViews` を書いていない
- `kind` の綴りの誤りが止まらない件（5.2 の限界 2）は 3/3 で `docs/known-gaps.md` に足す候補
- 試験と教材の作成者が同じ AI である。人は画面を見ていない

## 8. エンジンの差分 0 と書けなかった箇所（TASK-763 3/3。#2013）

### 8.1 エンジンの差分 0

数える範囲（`crates/`・直下の `Cargo.toml`・`Cargo.lock`）の差分は 0 行でした。固定した commit（`7dbc58a161a62faeef6a1aad221f8c947a1521a5`）・測った HEAD（`ae3cc2bed398908035120e13fdca27f1202072fa`）・SHA-256 の一覧（1,628 件）・再測定の手順は `docs/engine-zero-diff/task-breakdown/README.md` にあります（2026-11-03）。分類の表は空で、人の承認が要る分類はありません。

### 8.2 受け入れ条件の再実行（2026-11-03、手元の Linux、debug ビルド）

| コマンド | 結果 |
|---|---|
| `fandhe-pack validate template-workshop/examples/task-breakdown` | `OK: 診断 0 件（形式 B(YAML)、章 1、モジュール 0、Step の定義 8）`、終了コード 0 |
| `fandhe-pack test` 同上 | 模範解答 1/1、誤入力 18/18、入った Step の実体 8/8、終了コード 0 |
| 同じ 2 つを `probes/views` に | 診断 0 件、1/1・2/2・4/4、終了コード 0 |

### 8.3 書けなかった箇所・確かめていない箇所

| 箇所 | 記録 |
|---|---|
| `kind` の綴りの誤りが `validate`・`compile` で止まらない（5.1・5.2 の限界 2） | `docs/known-gaps.md` の 「ビューの綴り（専用の ID なし）」の行。未対応 |
| 開始点の `effects` を画面が適用しない。行は最初の選択肢の受付で置いた（5.2 の限界 1） | PACK-22 の行（TASK-217 が未着手） |
| 行の文字・入れ子・カードの状態の見た目は未確認（5.2 の限界 3、7.7） | SECOND-6（行の文面）の行に、第 3 教材でも未確認と追記。ART-17 にも関連する |
| `test` が `set_attr` の値を照合しない。第 3 教材には第 2 教材の `row-asserts.json` に当たる照合が無い（7.7） | AUTHOR-4 の節に追記。経過措置のまま |
| 選んだ値を差し込めず、行ごとに規則を列挙した（7.4） | H-3 |
| 1 つの設定では Step ごとの `visibleViews` を照合できない（5.2 の限界 4） | 道具の限界で、エンジンの語彙の不足ではない。この記録と `docs/testing.md` 5.459 にだけ書く |
| `on_achieved`・`next[].when` が置いた行を見るか、章をまたぐ形 | 使っていないため確かめていない（不足としては書かない） |

### 8.4 根拠の種類と限界

根拠は CLI（`validate`・`test`）と差分・SHA-256 の照合です。人は画面を見ていません。教材と試験の作者が同じ AI です。
