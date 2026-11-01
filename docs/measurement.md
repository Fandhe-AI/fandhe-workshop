# 測定の結果の様式

```text
決定日: 2026-10-14
対象: TASK-108（IR つきの直接リンクの初回表示。§8）・TASK-32（WASM の gzip の大きさ・初回表示・再描画の測定と一括実行。様式と 3 つの測定を扱う）
```

## 1. 位置づけ

- 測定の結果に付ける「基点」と、結果の JSON の様式を定める
- 基点の違う値どうしで増減を論じると誤った結論を招く。過去の PoC の記録には、commit・`wasm-opt` の有無・圧縮の設定が違う値が
  混在していた。結果の側で基点を必須にして、この事故を構造的に防ぐ
- 測るものは 3 つ。WASM の `gzip -9` の大きさ、最小の画面（Hello）の初回表示の時間、再描画の時間である
- CI の関門（WASM 1 MB）は `scripts/check-wasm-size.sh`（`make check-wasm-size`。CI の `browser-test`）にあり、`tools/measure/wasm-size.sh` の結果を判定する。上限は 1,048,576 bytes（DLV-27）で、CI は `SKIP_WASM_OPT=1` の基点で判定する。DLV-4（`wasm-opt` 適用後）の厳格な判定は手元で `scripts/check-wasm-size.sh --require-wasm-opt` を直接実行する（未適用の結果は終了コード 2。TASK-107 2/2。記録は `docs/testing.md` 5.356）。ジョブの追加はしていない。DLV-4 の関門が参照する値（そろえた基点の値）と旧い値の一覧は `docs/testing.md` 5.361（TASK-126）

## 2. 実行

```bash
npm ci && npx playwright install chromium   # 初回のみ（初回表示・再描画に必要）
make measure                                # 直接: tools/measure/all.sh
make measure | jq .
```

| 目的 | コマンド | 設定 |
|---|---|---|
| 3 つを一括で流す | `make measure`（`tools/measure/all.sh`） | 下の 3 つ |
| WASM の大きさ | `make measure-wasm-size`（`tools/measure/wasm-size.sh`） | 配信する設定（`TEST_HOOKS=1` で試験の設定も測れる） |
| 初回表示 | `tools/measure/first-paint.sh` | 配信する設定 |
| 初回表示（IR つきの直接リンクで操作できるまで。TASK-108。DLV-5） | `make measure-first-paint-direct`（`tools/measure/first-paint-direct.sh`） | 配信する設定に IR を同梱。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本。3 秒を超えたら終了コード 3（JSON は出す）。手順は §8 |
| 再描画 | `tools/measure/redraw.sh` | 試験の設定（`debug_rerender` が要る） |
| 再描画 1 回（ログ 100〜119 項目。TASK-223 1/2） | `tools/measure/log-redraw.sh` | 試験の設定。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本 |
| 項目数ごとの再描画の傾き（ログ 100〜4,000 項目の 12 段階。TASK-232 1/3） | `tools/measure/log-slope.sh` | 試験の設定。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本 |
| 2 周分の通しの再描画（世代と Step をまたぐ 6・48 グループ × 行の数 500・1,200・2,000。TASK-232 3/3） | `tools/measure/log-rounds.sh` | 試験の設定。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本 |
| 入力から描画まで（ログ 120〜158 項目。TASK-223 2/2） | `tools/measure/input-paint.sh` | 試験の設定。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本 |
| 左のビューの部品単位の再描画（固定データの画面。TASK-485 1/3。ART-4） | `tools/measure/view-redraw.sh` | 試験の設定（代役の部品 `view_fixture` が要る）。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本 |
| 本体の画面の左のビューの再描画（TASK-485 2/3。ART-26） | `tools/measure/product-redraw.sh` | 試験の設定（`?fixture=artifact-redraw-<n>` が要る）。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本 |

- release の設定で `tools/build-web.sh` を一時ディレクトリにビルドする（`dist/` は上書きしない）。結果の JSON を標準出力に 1 つだけ出し、
  進行状況とビルドログは標準エラーへ出す
- `SKIP_WASM_OPT=1` で `wasm-opt` なしを測る。`make measure` は 3 つへ同じ値を渡す
- 終了コード: 0 = 成功 / 1 = ビルド・測定・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用
- 基点の 1 つでも欠けた結果は出さない（fail-closed）。出力は `tools/measure/lib.sh` の `measure_emit`（単体）と
  `measure_emit_run`（束）だけが行う。3 つのうち 1 つでも失敗するか、実行中に HEAD や作業ツリーが動けば、束は出ない
- 遅く、ブラウザを要する。`make check`・フック・CI には入れない（`make measure` 全体について。WASM の大きさの関門 `make check-wasm-size` だけは CI で動く）
- 回帰テストは `make test-measure-wasm-size`（単体の様式）と `make test-measure`（束・fail-closed・一括実行）

2 周分の履歴の大きさ（TASK-216。EVT-13）は native の試験の出力を包む別の様式（§4a。`fandhe-workshop/measure-native/v1`）で、
`make measure` の束には入れない。

```bash
make measure-history-size | jq .            # 直接: tools/measure/history-size.sh
```

- 実体は `crates/pack/tests/history_size_rounds.rs`（`HISTORY-SIZE ` の行を 1 行出す）。`tools/measure/history-size.sh` がそれを基点つきの結果に包む
- 終了コード: 0 = 成功 / 1 = 試験・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用。出力は `lib.sh` の `measure_emit_native` だけが行う
- 回帰テストは `make test-measure-result` の (d)（組み立てと、基点の欠け・整数でない値・値の連結を断る負の対照）

## 3. 基点の 4 項目

| 項目 | JSON のパス | 内容 |
|---|---|---|
| commit | `baseline.commit` | `sha`（40 桁の 16 進）と `dirty`（未コミットの変更の有無） |
| `wasm-opt` の有無 | `baseline.wasm_opt` | `applied`（真偽）と `detail`（版、または未適用の理由） |
| 機能の範囲 | `baseline.features` | `config`（`delivery` / `test`）・`cargo_features`・`profile`（`release`） |
| 圧縮の設定 | `baseline.compression` | `command` と `implementation`（下表） |

次の 2 項目は任意で、様式の版は上げない（v1 の検証は必須の 4 項目だけを見る）。`log-redraw`・`log-slope`・`input-paint`・`product-redraw` の結果では `pack` と `device` が、`view-redraw` の結果では `data` と `device` が必須で、欠けたら結果を出さない。

| 項目 | JSON のパス | 内容 |
|---|---|---|
| 教材 | `baseline.pack` | `id`・`version`・`nodes`（Step の数）・`sha256`（IR ファイルの SHA-256） |
| 固定データ | `baseline.data` | `view-redraw` が教材の代わりに持つ。`id`・`manifest_sha256`（`testdata/artifact-view-parts/test-data.sha256` の SHA-256）・`files`（埋め込んだ規模のデータの写し 7 ファイルの名前ごとの SHA-256） |
| 端末 | `baseline.device` | `os`・`os_release`・`arch`・`cpu`・`logical_cores`・`memory_bytes`・`apple_silicon_macos`（基準端末 A の目印）。ホスト名・ユーザー名・パスは入れない |

| 測定 | `compression.command` | `compression.implementation` |
|---|---|---|
| WASM の大きさ | `gzip -9 -c` | `gzip --version` の 1 行目（GNU gzip） |
| 初回表示・再描画 | `tools/serve.mjs gzip level 9 (Content-Encoding: gzip)` | `node <版> zlib <版>` |

## 4. 結果の様式（`schema`: `fandhe-workshop/measure/v1`）

| パス | 型 | 内容 |
|---|---|---|
| `schema` | 文字列 | 様式の版。読む側が確かめる |
| `measured_at` | 文字列 | 測定時刻（UTC、`YYYY-MM-DDTHH:MM:SSZ`） |
| `baseline.*` | — | 前節の 4 項目 |
| `toolchain.rustc`・`toolchain.wasm_bindgen` | 文字列 | 再現の手掛かり。基点の 4 項目には含めない |
| `toolchain.browser`・`toolchain.playwright` | 文字列 | 初回表示・再描画のみ。`chromium <版>` と Playwright の版。任意の項目で、様式の版は上げない |
| `conditions` | 対象 | 初回表示・再描画のみ。回線・キャッシュ・試行数などの再現の手掛かり。基点の 4 項目には含めない |
| `measurements[]` | 配列（1 件以上） | `{ name, file, unit, value }`。`value` は正の整数 |

測定の名前と単位

| 測定 | `name` | `unit` | 内容 |
|---|---|---|---|
| WASM の大きさ | `wasm_raw`・`wasm_gzip` | `bytes` | `.wasm` の生の大きさと `gzip -9` の大きさ |
| 初回表示 | `first_paint_mounted_median`・`first_paint_mounted_max` | `us` | ナビゲーション開始から `html[data-mounted="true"]` になるまで（Hello が DOM に出た時点）の中央値と最大 |
| 初回表示 | `first_contentful_paint_median` | `us` | First Contentful Paint の中央値 |
| 初回表示 | `first_paint_wasm_transfer` | `bytes` | `.wasm` の転送量（`encodedBodySize`） |
| 再描画 | `redraw_median`・`redraw_p95`・`redraw_max` | `us` | `Runtime::rerender` による root 配下の再構築 1 回あたりの時間 |
| 再描画 1 回（ログ 100〜119 項目） | `log_redraw_median`・`log_redraw_p95`・`log_redraw_max` | `us` | 隠し要素の click 1 回の同期的な処理（イベントの委譲・状態の更新・DOM への反映）の時間 |
| 項目数ごとの再描画の傾き | `log_slope_<4 桁の項目数>_median`・`_p95`・`_max`（12 段階 × 3 = 36 件） | `us` | `log_redraw_*` と同じ量を、ログの長さ 100・200・400・600・800・900・1100・1200・1600・2000・3000・4000 の各段階で測った値。傾きは `value` が正の整数という様式の制約のため含めず、記録の文書で値から計算する |
| 2 周分の通しの再描画 | `log_rounds_g<2 桁のグループ数>_<4 桁の行の数>_median`・`_p95`・`_max`（グループ数 6・48 × 行の数 500・1200・2000 × 3 = 18 件） | `us` | `log_slope_*` と同じ量を、`go_to` で (世代, Step) の別のグループを作った 2 周分の通しで測った値 |

検証は `measure_validate`（`tools/measure/lib.sh`）が行い、`schema` の一致・基点の各項目の存在と型・`measurements` の値を確かめる。

### 測定の条件

- 初回表示（出典: `docs/spec/02-poc-plan.md` §7 の条件）
  - 回線は CDP の `Network.emulateNetworkConditions` で下り・上り 10Mbps・遅延 40ms に絞る。CPU の減速はしない。キャッシュは無効にする
  - 試行ごとに新しい context で 10 回測り、中央値と最大を記録する
  - ポーリングではなく、ページ内の `MutationObserver` が `data-mounted` の変化の時点の `performance.now()` を記録する。`data-mounted="error"` は失敗にする
  - 制限が効いていることを確かめる。`.wasm` の応答が `Content-Encoding: gzip` であること、各試行のマウント時間が
    `.wasm` の転送量を 10Mbps で受け取る最短時間以上であること。どちらかを満たさなければ失敗にする
  - Hello には操作が無いため、PoC の TTI（最初の操作への応答）は測らない。操作できるまでは下の `first-paint-direct` で測る
- 初回表示（IR つきの直接リンク。`first-paint-direct`。TASK-108。DLV-5）。`first-paint` は置き換えず残す（束の必須項目で、Hello の値の系列を保つため）
  - 教材は `crates/web/tests/data/ch0_3/ir.json`（20 Step の形の写し。本物の教材ではない）に、`pack.id` の付け替え（`measure-first-paint-direct`）と開始点 `s-1-1` の追加だけを jq で加えた写し。`IR_FILE` で配信する設定へ同梱し、サブパスの下で配る
  - 開くのは `<配信元>/subpath/?start=s-1-1`。条件は `first-paint` と同じ（下り・上り 10Mbps・遅延 40ms・キャッシュ無効・CPU の減速なし・試行ごとに新しい context で 10 回）
  - 「操作できる」とは、次がすべて真になった最初の時点（ナビゲーション開始からの `performance.now()`）である。`data-mounted="true"`、`run-status` が `idle`、`step-title` が開始点の節点の題と一致、`step-status` が「受付済み（確認待ち）」でなくログにその Step の行が無い、入力の口（`choice-btn`・`approval-btn`・`draft-input` と `submit-input` の組）が接続済み・大きさあり・`disabled` でない状態で 1 つ以上ある。ページ内の `MutationObserver` で捉える
  - 自己検査（1 つでも外れたら失敗で標準出力は空）。`.wasm` の応答が 1 件で gzip、`ir/main.json` の要求が 1 件で 200 かつ転送量が正、操作できるまでの時間が wasm と IR の転送量を 10Mbps で受け取る最短時間以上、配信元以外への要求・`pageerror`・枠の通知が 0 件、`window.__fandheTest` が無い、次の Step の題が出ていない
  - 判定は中央値と最大の両方が 3,000,000 us 以下（ちょうどは以内）。超えても JSON は出し、終了コードは 3。形の不正・測定の失敗は 1。判定の正は `lib.sh` の `measure_first_paint_direct_verdict`（`values_us` から計算し直して照合する）
  - 測定名: `first_paint_direct_operable_median`・`_operable_max`（判定値）、`first_paint_direct_mounted_median`（参考）、`first_paint_direct_wasm_transfer`・`first_paint_direct_ir_transfer`（bytes）。`conditions` に回線・CPU・キャッシュ・viewport・`browser.channel`・`browser.headless`・開始点・10 回の値（`values_us`）・`threshold_us`・`judgement`・`scope_note` を残す
  - 基点の `device` は区分だけ（`class`: `A` は Apple シリコンの macOS、それ以外は `other`。`os`・`arch`・`apple_silicon_macos`）。CPU の型番・メモリの量・ホスト名は公開に向かないため入れない。`class: A` は基準端末 A であることの証明ではなく、記録する人が確かめる
- 再描画
  - 試験の設定のビルドを、回線の制限なし・CPU の減速なしで配信する。10 回の試行のそれぞれで 20 サンプルを取る（計 200 値）
  - 1 サンプルは `window.__fandheTest.rerender()` を 200 回連続で呼ぶ時間を 200 で割った 1 回あたりの値。`performance.now()` の分解能より十分大きくするため
  - 各サンプルで、旧 root が DOM から外れ、新しい root が `Hello` を持つことを確かめる。何もしない再描画を「速い」と誤読しないため。戻り値が `false`（未マウントなど）なら失敗にする
  - 中央値は中央 2 値の平均、95 パーセンタイルは nearest-rank（`s[ceil(0.95n)-1]`）。PoC の `measure-poc1.mjs` と同じ定義である
  - 値は整数のマイクロ秒に丸める。0 に丸まる値は失敗にする（1 へ繰り上げない）

- 2 周分の通しの再描画（TASK-232 3/3。EVT-26。出典の目安: `docs/spec/03-poc/save-restore-versions/measurements/history-size.csv`）
  - `min-3step` の試験の設定を、回線・CPU の制限なしで配信する。ログは `go_to`（logLen が 2 増え、`li` は増えない）で st-1→st-2→st-3 を巡回して、6 または 48 のグループ（世代 × Step）にし、拒否される入力（`text: x`。1 回でログと `li` が 2 件増える）を各グループへ割り当てる（端数は古い側へ）。行の総数 R は測る前の logLen
  - 段階（G × R）ごとに新しい context で 5 試行 × 40 操作 = 200 値。測るのは最新のグループ（st-3）での `el.click()` の同期区間で、サンプルの間に 20ms 待つ。判定は代表値 R = 500 の p95（16,000 us）
  - 各操作の直後に、logLen の増え方、畳みの前は `li` の増え方、最後のデータ行の変化、Step の位置、`[data-list-error]`・`[data-list-limit]` が無いこと、`li` が 1,000 以下であることを確かめる。外れたら失敗にする。窓の中で `li` が 1,000 をまたがないことは計画の純関数 `planRoundsStage` が検査する
  - 16ms を超えても失敗にせず、標準エラーに within / over を出す。値は合成の 2 周分のもので、本物の教材の行の構成を表さない（`conditions.scope_note`）

- 再描画 1 回（ログ 100〜119 項目。出典: `docs/spec/03-poc/engine-event-consistency/env.md`、EVT-24）
  - 既定の最小教材（`min-3step`）の試験の設定を、回線・CPU の制限なしで配信する。操作は見えない `raw-trigger` に拒否される入力（`/make-note Bad_Name`）を載せた click で、1 回でログが 2 件増える
  - 1 試行は新しい context で、時間を測らずにログを 100 件にしてから、10 操作を 1 つずつ測る（ログは 100〜118 の間）。20 試行で計 200 値
  - 測るのは `el.click()` の直前と直後の `performance.now()` の差だけ（ページ内で完結）。レイアウトと描画は含まない。サンプルの間に 20ms 待つ
  - 各操作の直後に、操作前のログが窓（100〜119）の中、操作後が前より 2 件多い、ログの `li` が click から戻った時点で 2 件増えている、`[data-list-error]` が無いことを確かめる。外れたら失敗にする
  - 統計と丸めは再描画と同じ。16ms（`conditions.threshold_p95_us`）を超えても失敗にせず、標準エラーに within / over を出す。合否は `docs/testing.md` の記録に書く
  - 値は最小教材のもので、教材全体を代表しない（`conditions.scope_note`）

- 項目数ごとの再描画の傾き（ログ 100〜4,000 項目。TASK-232 1/3。EVT-26・D-47）
  - `log-redraw` と同じ量・同じ教材・同じ操作を、ログの長さの 12 段階で測る。1 試行（新しい context）の中で段階を昇順に進め、各段階まで時間を測らずに埋めてから 20 操作を 1 つずつ測る（窓は `[n, n + 38]`。900 段は 900〜938、1100 段は 1100〜1138 で、約 1,000 項目の畳みをまたがない）。10 試行で段階ごとに計 200 値。操作の間に 20ms 待つ
  - 各操作の直後に、窓の中・操作後が 2 件多い・`[data-list-error]` が無い・ログの `li` が 1,000 以下・畳みの前（`li[data-fold]` が 0 件）は `li` が 2 件増える・最後のデータ行の `data-event` が変わる・Step が動かない、を確かめる。外れたら失敗にする。畳みの前後の `li` の数は `conditions.rows_observed` に残す
  - 統計と丸めは再描画と同じ。16ms を超えても失敗にせず、段階ごとに within / over を標準エラーに出す。傾き（最小二乗）は `measurements[]` に入れず、`docs/testing.md` の記録で値から計算する
  - 最小教材は Step が 1 つのため、畳みは先頭側だけが働く。複数 Step にまたがるログの畳み方の費用は表さない（`conditions.scope_note`）

- 入力から描画まで（ログ 120〜158 項目。出典: 同じ `env.md` の「計測の条件」、EVT-24）
  - 既定の最小教材（`min-3step`）の試験の設定を、回線・CPU の制限なしで配信する。操作は送信ボタン（`submit-input`）の CDP の信頼された click（`page.click`）。入力欄に拒否される `/make-note Bad_Name` を 1 回入れておくと、拒否された下書きは欄に残るため、同じ本文が毎回送られ、1 回でログが 2 件増える
  - PoC は停止ボタンの click を使ったが、現行の実装では停止は処理中でなければ状態を変えない（`crates/web/src/hello.rs`）ため、ログへ項目を追加する操作に当たらない。そのため送信に置き換えた
  - 1 試行は新しい context で、時間を測らずにログを 120 件にしてから、下書きを入れて 200ms 待ち、20 操作を 1 つずつ測る（操作前のログは 120, 122, …, 158）。10 試行で計 200 値（`log-redraw` の 20 × 10 とは逆の配分）。操作の間に 30ms 待つ
  - 起点は click のキャプチャ段階の受け取りの時刻（参考値 `input_paint_capture_p95`）と `event.timeStamp`（判定値）。終点は次の `requestAnimationFrame` の中の `setTimeout(0)`。判定には値が大きくなる `timeStamp` 起点を使う
  - 各操作で、`isTrusted` が真、操作前のログが窓の中、操作後が 2 件多く `li` も 2 件増える、Step が動かない、下書きが変わらない、`[data-list-error]` が無い、`fromEvent >= fromCapture >= 0` を確かめる。外れたら失敗にする。1 操作の完了待ちには 10 秒の上限を置く
  - 統計と丸めは再描画と同じ。100ms（`conditions.threshold_p95_us`）を超えても失敗にせず、標準エラーに within / over を出す。合否は `docs/testing.md` の記録に書く
  - 値は最小教材のもので、教材全体を代表しない（`conditions.scope_note`）

## 4a. native の大きさの様式（`schema`: `fandhe-workshop/measure-native/v1`）

大きさは端末・wasm-opt・圧縮に依らないため、v1 の必須欄（`wasm_opt`・`features`・`compression`）を偽の値で埋めず、別の様式にした。端末（`device`）も入れない。

| パス | 型 | 内容 |
|---|---|---|
| `schema` | 文字列 | `fandhe-workshop/measure-native/v1` |
| `measured_at` | 文字列 | 測定の時刻（UTC） |
| `baseline.commit` | `{sha, dirty}` | v1 と同じ。`dirty: true` の値は記録に使わない |
| `baseline.cargo_profile` | 文字列 | `test` |
| `baseline.pack` | 欄の組 | `id`・`version`・`nodes`・`ir_sha256`（compile の出力のバイト列）・`content_hash`・`source` |
| `baseline.ops_sequence` | 欄の組 | `id`（`rounds-plan-a-v1`）・`ops`（件数）・`ops_sha256`（`ops` の JSON）・`goto_substitutions`（`GoTo` で飛ばした数。1 周目 3・2 周目 2 の 5） |
| `toolchain.rustc` | 文字列 | `rustc -V` |
| `conditions` | 欄の組 | `condition: "A"`・`checkpoints_at`・`scope_note` |
| `measurements[]` | `{name, unit, value}` | 値は正の整数 |

測定名: `history_total_bytes`・`history_ops_bytes`・`history_snapshot_bytes`・`history_changed_bytes`・`history_checkpoint_bytes`（`bytes`）、`history_ops`・`history_log_entries`・`history_changed_entries`・`history_checkpoints`（`count`）、`history_changed_ratio_ppm`（`ppm`。`changed_bytes * 1,000,000 / total_bytes` の切り捨て）。

測り方の定義:

- 対象は書き出しの包み（`export_bytes` の compact JSON。`ops` と `snapshot`）。`total_bytes` はそのバイト数
- `Changed` のバイト数は `snapshot.log` の `Changed` を 1 件ずつ JSON にした長さの合計（`synth-inputs.md` §3 と同じ定義）
- 道筋は `module-reuse-rounds` の模範解答の 2 周分に、各周の終わりのチェックポイントを足した条件 A の形。誤入力は入れない
- 未実装の部品のため `GoTo` で飛ばす場所がある（`docs/testing.md` 5.258・5.260）。飛ばした分、`Changed` の件数は真の通しと違いうる

## 4b. 右のログの項目数の様式（TASK-454 1/2。`schema`: `fandhe-workshop/measure-native/v1`）

様式は 4a と同じで、`baseline.ops_sequence` は同じ `rounds-plan-a-v1`（`ops_sha256` が `make measure-history-size` と一致すれば同じ操作列を測っている）。違う欄だけを示す。

| パス | 内容 |
|---|---|
| `conditions` | `condition: "A"`・`target`（`LIST_ITEMS_TARGET` = 1,000）・`within_target`（最大が目安以下か）・`folded_steps`（畳みが起きた手の数）・`limit_marks`（安全弁・上流の拒否の印の数）・`scope_note` |
| `measurements[]` | 値は正の整数。0 になりうる量は `conditions` に置く |

測定名（単位はすべて `count`）: `log_rounds_items_max`（手ごとの描画で数えた `part-log-list` の `<li` の最大）・`log_rounds_items_round2_end`・`log_rounds_events_round2_end`（項目数とは別の量）・`log_rounds_groups_round2_end`（`data-gen` と `data-step` の組の数）・`log_rounds_steps_observed`（観察した手の数）。

```bash
make measure-log-items | jq .               # 直接: tools/measure/log-items.sh
```

- 実体は `crates/web/tests/log_rounds_count.rs`（`LOG-ROUNDS ` の行を 1 行出す）。native の描画結果を数えており、ブラウザの DOM の値ではない
- 終了コード: 0 = 成功 / 1 = 試験・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用。`make measure` の束には入れない
- 比較の規則: `log_rounds_*` を `log-rounds.sh`（合成のログの再描画の時間）と比べない。`ops_sha256` が違う値どうしを比べない
- 回帰テストは `make test-measure-result` の (e)。記録は `docs/testing.md` 5.349

## 5. 一括実行の束（`schema`: `fandhe-workshop/measure-run/v1`）

`make measure` の出力。大きさと初回表示は配信する設定、再描画は試験の設定で測るため、基点の機能の範囲は結果ごとに違う。
そのため、v1 の結果を 3 件並べた束にする。

```json
{ "schema": "fandhe-workshop/measure-run/v1", "results": [ <v1 の結果> × 3 ] }
```

- `results` はちょうど 3 件で、順に WASM の大きさ・初回表示・再描画
- 各要素は v1 の検証を満たす。`measurements[].name` に `wasm_gzip`・`first_paint_mounted_median`・`redraw_median` が 1 件ずつある
- 3 件の `baseline.commit`（`sha` と `dirty`）が一致する。同じ commit で測ったことの保証である
- 検証は `measure_validate_run` が行い、通らなければ何も出さない

## 6. 比較の規則

- 基点が 1 項目でも違う値どうしで、増減を論じない
- `first-paint-direct` の値を Hello の `first_paint_mounted_*` と比べない。開始点・教材・`browser.channel`・`headless` が違う値も比べない。`device.class` が `other` の値は DLV-5 の記録に使わない
- `baseline.commit.dirty` が `true` の値は、基点として比較に使わない（どの commit の値か特定できない）
- `wasm-opt` が無い環境でもビルドは成功し、`applied: false` として記録される。未適用の値を適用済みの値と比べない
- spec に残る旧い WASM の大きさの値（PoC の基点）と、`docs/testing.md` 5.361 のそろえた基点の値を比べない（基点つきの一覧は 5.361）
- 左のビューの部品単位の再描画（固定データの画面。出典: `docs/spec/03-poc/artifact-view-parts/README.md` と `testdata/artifact-view-parts/test-data/README.md` の「計測の定義」、ART-4。TASK-485 1/3）
  - 試験の設定の代役の部品（`view_fixture`）を、回線・CPU の制限なしで配信する。測るのは `window.__fandheTest.viewFixtureDispatch` の直前と直後の `performance.now()` の差（同期処理。click の委譲と同じ `dispatch_and_apply` を通り、update・`view()`・keyed list の差分・DOM への反映を含む。click の委譲の属性の読み取り、レイアウトと描画は含まない）
  - データ: 文書 200 行・差分 500 行・タスク 50 件（判定の対象）と、ファイルツリー 200 件・カード 20 件・チェック 20 件・アプリのデモ 9 行（参考）。写しの出所は `crates/web/tests/data/artifact_view_scale/README.md`
  - 組: (b) は 7 種すべて、(a) は契約が許す 5 種の計 12 組。文書 (a)・差分 (a) は契約が拒否するため測らない（`conditions.excluded`）。操作は `toggle`（同じビューの前後を交互に切り替える）と `switch`（別のビューから対象のビューへ切り替える。前後は後に固定。相手は種類の並びで 1 つ前の、同じ変種を許す種類で、`conditions.switch_from` に残す）の 2 種で、どちらも判定に使う
  - 1 試行は新しい context で、規模のデータへ差し替えてから、12 組 × 2 操作 × 20 回を流す。10 試行で組ごとに 200 値。試行 i は組の開始位置を i だけずらす。操作の間に 20ms 待つ
  - 各操作の直後に、dispatch が受理された、選んだ一覧の項目数が期待値（前・後。Node 側で JSON の行から数える。文書は手で数えた 80 / 97）と一致する、ほかの一覧が空で `hidden`、通知が出ていないことを確かめる。外れたら失敗にする
  - 統計と丸めは再描画と同じ。判定の規則は「ビューごとに、`toggle` と `switch` の両方で 95 パーセンタイルが 16ms 以内の変種が 1 つ以上あれば閾値内」（`conditions.judgement`。判定の対象 3 種と参考 4 種を分けて持つ）。超えても失敗にせず、標準エラーに within / over を出す。合否は `docs/testing.md` の記録に書く
  - 値は代役の部品の画面のもので、本体の `view()` の値ではない（ART-26 は別のイシュー）。CPU 4 倍（ART-15）は記録だけの別の種別（下の「CPU 4 倍の記録」）
- CPU 4 倍の記録（ART-15。TASK-485 3/3）。記録だけで、判定に使わない
  - 種別は `view-redraw-cpu4x`（代役の部品の画面。文書 200 行・差分 500 行・タスク 50 件の `toggle` と `switch`。8 系列）と `product-redraw-cpu4x`（本体の画面。`switch` だけ。3 件）。入口は `tools/measure/view-redraw-cpu4x.sh`・`product-redraw-cpu4x.sh`。測る量・10 試行 × 20 操作・統計は減速なしの種別と同じ
  - 減速は CDP の `Emulation.setCPUThrottlingRate`（率は 4 の完全一致だけ）で、ページの読み込みと画面の mount が済んだ後、測る操作の前に掛ける。固定の仕事量の整数演算の時間を減速の前後で測り、比が 2 未満なら失敗にする（減速が効かないまま速い値を 4 倍の値として残さないため）。試行ごとの比の最小・最大は `conditions.cpu_calibration` に残す
  - 結果は判定の欄（`conditions.judgement`・`judged`・`within`）を持たない。`conditions.record_only: true` と、`conditions.record_only_cpu4x`（種類ごとの p95 と、参考の 16ms 超過の印 `over_threshold`）に記録する。16ms は参考で、合否に使わない。`conditions.sample_diff_rows` に教材の sample の差分の大きさを添える
  - 測定名は `view_redraw_cpu4x_…`・`product_redraw_cpu4x_…`。`make measure` の束には入れない

- 配信する設定（`delivery`）の値と試験の設定（`test`）の値を比べない。再描画の値は試験の設定の値であり、配信する設定の値と比べない
- 回線条件・ブラウザの版・圧縮の実装が違う値を比べない。大きさ（GNU `gzip -9`）と初回表示・再描画（Node の zlib のレベル 9）は
  圧縮の実装が違うため、圧縮の設定が違う値どうしは比べない
- 教材（`baseline.pack`）・端末（`baseline.device`）が違う値どうしを比べない。`log-redraw`・`log-slope`・`log-rounds`・`input-paint` の値は最小教材の値で、教材全体を代表しない
- `view-redraw` の値は固定データ（`baseline.data`）の代役の部品の画面の値で、教材の値・本体の画面の値と比べない
- `conditions.cpu_throttle` が違う値どうしを比べない。CPU 4 倍の値（`view-redraw-cpu4x`・`product-redraw-cpu4x`）は記録だけで、判定に使わず、減速なしの値とも比べない
- `product-redraw` の値は本体の画面の `switch`（選択のボタンの click の同期区間。委譲を含む）の値で、`view-redraw` の値（委譲を含まない）・教材の値と比べない。種類ごとの描画部品が製品に結線されるまでは経路の行の値である（`docs/testing.md` 5.318）
- `history_*` の値（native の様式）は、PoC-6 の CSV の値と直接比べない（履歴の形が違い、誤入力が無く、`GoTo` で飛ばしているため）。WASM・ブラウザの側の値とも比べない。`baseline.ops_sequence.ops_sha256` が違う値どうしを比べない
- Hello の値は最小の画面の値で、教材全体の値ではない
- 記録に値を残すときは、基点の 4 項目を併記する

## 7. 将来の拡張

- TASK-336（SAVE-26）は実施済み。`crates/pack/tests/history_size_conditions.rs` が計画に誤入力・`Control::Save`・下書きを差し込み、3 条件 × 種 5 の 15 本で 5MB 以下を判定する（記録は `docs/testing.md` の 5.344）。`HISTORY-SIZE-CONDITIONS ` の行の値も PoC-6 の CSV と直接は比べない。`make measure` の束には入れていない
- TASK-337（SAVE-27）は、本物の CH-0〜CH-15 が無いため章の形の写しと 2 周分の代役で実施した（`crates/web/tests/history_size_all_chapters.rs`。記録は `docs/testing.md` の 5.345）。本物の全章では未実施で、`HISTORY-SIZE-ALL-CHAPTERS ` の行は PoC-6 の CSV・5.344 の値と直接は比べない。`make measure` の束には入れていない

- JS glue・`index.html` など他の配信物の大きさは、`measurements[]` に足すだけでよい（様式の変更は不要）
- ビルド情報はビルドログの行（`config` / `wasm-opt` / `wasm size`）から読む。解析は `tools/measure/lib.sh` の `measure_build` に集約してある。
  機械可読の出力を `build-web.sh` に足す案は、ログの形式が変わって解析が壊れるようになった時点で検討する
- 大きさと初回表示は配信する設定を 2 回ビルドする（cargo のキャッシュが効くため差は数秒）。1 回のビルドの使い回しは後続の改善とする
- 左のビューの CPU 4 倍は TASK-485 3/3 で扱った（§4）。制限なしの対照・Chrome の原本での測定は、記録だけの条件として後続で扱う
- 保存の遅延（変更から保存先への書き込みまで。SAVE-7。TASK-318）はブラウザ試験 `save-latency` で測り、記録は `docs/testing.md` 5.168 にある。束（`measure-run/v1`）は `results` を 3 件に固定しているため入れていない。束へ統合するときは様式の版上げが要る。統計の定義（中央値・nearest-rank の 95 パーセンタイル）は `tools/measure/browser.mjs` と共有している
- `log-redraw`・`log-slope`・`input-paint`・`view-redraw`・`product-redraw` は束（`results` を 3 件に固定）に入れていない。束へ統合するときは様式の版上げが要る
- 操作できるまでの初回表示は `first-paint-direct` で扱った（§4・§8）。基準端末 A での実測と記録は #833、CI の関門（目安にとどめるか失敗にするか）は人の承認点、公開サイトでの測定は DLV-6・TASK-123 で扱う。`make measure` の束への統合は様式の版上げが要る

## 8. 基準端末 A での初回表示の測定（DLV-5。#833 の手順。TASK-108 の道具）

`first-paint-direct`（§4）を、基準端末 A（Apple シリコンの Mac）の Chrome の原本で人が流して記録するための手順と様式である。この節を足した時点では実測の記録は無い（手元の Linux・Chromium の確認は `docs/testing.md` 5.432）。

### 前提（導入するもの）

- Rust（`rust-toolchain.toml` の版）・`wasm32-unknown-unknown` ターゲット・wasm-bindgen CLI・`wasm-opt`（DLV-4 の適用後の値を記録するため）・jq・Node（`docs/toolchain.md`）
- `npm ci`（Playwright は `package-lock.json` で固定）。Chrome の原本を使うため `FANDHE_BROWSER_CHANNEL=chrome` を付ける（Chromium の導入は不要）
- 作業ツリーを clean にし、測る commit を決めてから流す（`baseline.commit.dirty` が `true` の値は記録に使わない）

### 手順

```bash
out="$(mktemp -d)/first-paint-direct.json"
FANDHE_BROWSER_CHANNEL=chrome tools/measure/first-paint-direct.sh > "$out"; echo $?
```

- 結果は作業ツリーの外（上の `mktemp -d`）に保存する。リポジトリ直下に未追跡のファイルができると `baseline.commit.dirty` が `true` になるため
- 終了コードを記録するときは、スクリプトを直接呼ぶ。`make` 経由では、レシピの終了コード 1 と 3 がどちらも 2 に変わり、測定の失敗と 3 秒の超過を区別できない
- 終了コード 0 は 3 秒以内、3 は測れたが超過、1 は測定の失敗（標準出力は空）。`make measure-first-paint-direct` 経由でも JSON は同じだが、終了コードは失敗と超過のどちらも 2 になる
- 流している間は、ほかの重い処理を止める。10 回の値が揃わない・ばらつきが大きいときは、条件を変えず流し直す前に値を記録に残す
- 貼ってもらうのは、標準出力の JSON の全体と終了コードだけである。標準エラー（ビルドログ）にはパスが出るため、失敗の調査で要るときだけ貼る

### 記録の様式

置き場所は `docs/testing.md` の 5.x 節（番号は記録するときの空きを使う）。欄は次のとおり。

| 欄 | 内容 |
|---|---|
| 測定日 | `YYYY-MM-DD` |
| 基点 commit | `baseline.commit.sha`。`dirty` が `false` であること |
| 端末 | `baseline.device.class`（`A`）。基準端末 A であることを記録者が確かめた旨 |
| wasm-opt | `baseline.wasm_opt.applied` と `detail` |
| ブラウザ | `toolchain.browser`・`conditions.browser.channel`（`chrome`）・`headless`・`toolchain.playwright` |
| 条件 | 回線・遅延・CPU の減速・キャッシュ（`conditions` のとおり） |
| 開始点 | `conditions.start` |
| 値 | `values_us` の 10 件・中央値・最大・判定（以内／超過）・終了コード |
| 記録者 | 測った人 |
| 結果の JSON | 全体（折りたたみまたはコードブロック） |

値は手元のサブパス配信のもので、公開サイトの値ではない（DLV-6）。教材は CH-0〜CH-3 の形の写しで、本物の教材での値ではない。
