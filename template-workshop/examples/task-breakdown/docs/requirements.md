# 第 3 教材「タスクの分解と進行管理」の記録

- 文書の版: 1
- 実施日: 2026-11-03
- 対応するタスク: TASK-763 1/3（#2011）。この記録は、同じタスクの 2/3（#2012。章立てと Step）と 3/3（#2013。エンジンの差分 0 の確認と書けなかった箇所）が追記します
- 仕様は private のため、本文を転記せず ID（PACK-22・SECOND-6・ART-17・H-3・AUTHOR-7 など）で参照します

この記録の目的は、3 本目の教材が左エリアに使う `task_tree`（タスクの木）と `card_list`（カードの一覧）のビューが、今のエンジンでどこまで動くかを、エンジンを変えずに確かめた結果を残すことです。3 本目は性質の違う教材（行を指す操作が中心）として、公開の資料と CLI だけで書きます。

## 1. 置き場の決定

| 項目 | 決定 | 理由・注 |
|---|---|---|
| ディレクトリ名 | `template-workshop/examples/task-breakdown/` | 題材を表す英小文字とハイフンだけの名前で、第 2 教材 `review-request` と同じ並びにそろえる |
| 本体の `pack.id`（予定） | `task-breakdown` | #2012 で本体を書くときに使う。確認用の `pack.id` は `task-breakdown-probe` |
| 確認用の教材パック | `task-breakdown/probes/views/` | 独立した教材パックの根。根の `task-breakdown/` には #2012 が本体を書く |
| 画面の確認の設定 | `tools/browser/choice-journey/task-breakdown-views.json`（と `task-breakdown-tree.json`・`task-breakdown-cards.json`） | 既存の道具（`make check-choice-journey`）に設定を足しただけで、道具のコードは変えていない |

- `probes/` を含む根を `validate` しても `probes/` が無視されるかは、確かめていません。#2012 で根を作ったときに確かめます
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
- 行の属性（`title`・`state`・`detail`）の名前は、画面が読む約束がまだ文書に無い（`docs/views.md` の 4 節）。行の文面に頼らず、状態は Step の本文と拒否の文面でも伝える
- 根の `validate` が `probes/` を無視することを確かめる
