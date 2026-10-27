# 測定の結果の様式

```text
決定日: 2026-10-14
対象: TASK-32（WASM の gzip の大きさ・初回表示・再描画の測定と一括実行。様式と 3 つの測定を扱う）
```

## 1. 位置づけ

- 測定の結果に付ける「基点」と、結果の JSON の様式を定める
- 基点の違う値どうしで増減を論じると誤った結論を招く。過去の PoC の記録には、commit・`wasm-opt` の有無・圧縮の設定が違う値が
  混在していた。結果の側で基点を必須にして、この事故を構造的に防ぐ
- 測るものは 3 つ。WASM の `gzip -9` の大きさ、最小の画面（Hello）の初回表示の時間、再描画の時間である
- CI の関門（WASM 1 MB）は `scripts/check-wasm-size.sh`（`make check-wasm-size`。CI の `browser-test`）にあり、`tools/measure/wasm-size.sh` の結果を判定する。上限は 1,048,576 bytes（DLV-27）で、`SKIP_WASM_OPT=1` の基点で判定する。ジョブの追加はしていない

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
| 再描画 | `tools/measure/redraw.sh` | 試験の設定（`debug_rerender` が要る） |
| 再描画 1 回（ログ 100〜119 項目。TASK-223 1/2） | `tools/measure/log-redraw.sh` | 試験の設定。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本 |
| 項目数ごとの再描画の傾き（ログ 100〜4,000 項目の 12 段階。TASK-232 1/3） | `tools/measure/log-slope.sh` | 試験の設定。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本 |
| 2 周分の通しの再描画（世代と Step をまたぐ 6・48 グループ × 行の数 500・1,200・2,000。TASK-232 3/3） | `tools/measure/log-rounds.sh` | 試験の設定。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本 |
| 入力から描画まで（ログ 120〜158 項目。TASK-223 2/2） | `tools/measure/input-paint.sh` | 試験の設定。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本 |
| 左のビューの部品単位の再描画（固定データの画面。TASK-485 1/3。ART-4） | `tools/measure/view-redraw.sh` | 試験の設定（代役の部品 `view_fixture` が要る）。`make measure` の束には入れない。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本 |

- release の設定で `tools/build-web.sh` を一時ディレクトリにビルドする（`dist/` は上書きしない）。結果の JSON を標準出力に 1 つだけ出し、
  進行状況とビルドログは標準エラーへ出す
- `SKIP_WASM_OPT=1` で `wasm-opt` なしを測る。`make measure` は 3 つへ同じ値を渡す
- 終了コード: 0 = 成功 / 1 = ビルド・測定・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用
- 基点の 1 つでも欠けた結果は出さない（fail-closed）。出力は `tools/measure/lib.sh` の `measure_emit`（単体）と
  `measure_emit_run`（束）だけが行う。3 つのうち 1 つでも失敗するか、実行中に HEAD や作業ツリーが動けば、束は出ない
- 遅く、ブラウザを要する。`make check`・フック・CI には入れない（`make measure` 全体について。WASM の大きさの関門 `make check-wasm-size` だけは CI で動く）
- 回帰テストは `make test-measure-wasm-size`（単体の様式）と `make test-measure`（束・fail-closed・一括実行）

## 3. 基点の 4 項目

| 項目 | JSON のパス | 内容 |
|---|---|---|
| commit | `baseline.commit` | `sha`（40 桁の 16 進）と `dirty`（未コミットの変更の有無） |
| `wasm-opt` の有無 | `baseline.wasm_opt` | `applied`（真偽）と `detail`（版、または未適用の理由） |
| 機能の範囲 | `baseline.features` | `config`（`delivery` / `test`）・`cargo_features`・`profile`（`release`） |
| 圧縮の設定 | `baseline.compression` | `command` と `implementation`（下表） |

次の 2 項目は任意で、様式の版は上げない（v1 の検証は必須の 4 項目だけを見る）。`log-redraw`・`log-slope`・`log-rounds`・`input-paint` の結果では `pack` と `device` が、`view-redraw` の結果では `data` と `device` が必須で、欠けたら結果を出さない。

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
  - Hello には操作が無いため、PoC の TTI（最初の操作への応答）は測らない。操作の入る画面が加わった時点で見直す
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
- `baseline.commit.dirty` が `true` の値は、基点として比較に使わない（どの commit の値か特定できない）
- `wasm-opt` が無い環境でもビルドは成功し、`applied: false` として記録される。未適用の値を適用済みの値と比べない
- 左のビューの部品単位の再描画（固定データの画面。出典: `docs/spec/03-poc/artifact-view-parts/README.md` と `testdata/artifact-view-parts/test-data/README.md` の「計測の定義」、ART-4。TASK-485 1/3）
  - 試験の設定の代役の部品（`view_fixture`）を、回線・CPU の制限なしで配信する。測るのは `window.__fandheTest.viewFixtureDispatch` の直前と直後の `performance.now()` の差（同期処理。click の委譲と同じ `dispatch_and_apply` を通り、update・`view()`・keyed list の差分・DOM への反映を含む。click の委譲の属性の読み取り、レイアウトと描画は含まない）
  - データ: 文書 200 行・差分 500 行・タスク 50 件（判定の対象）と、ファイルツリー 200 件・カード 20 件・チェック 20 件・アプリのデモ 9 行（参考）。写しの出所は `crates/web/tests/data/artifact_view_scale/README.md`
  - 組: (b) は 7 種すべて、(a) は契約が許す 5 種の計 12 組。文書 (a)・差分 (a) は契約が拒否するため測らない（`conditions.excluded`）。操作は `toggle`（同じビューの前後を交互に切り替える）と `switch`（別のビューから対象のビューへ切り替える。前後は後に固定。相手は種類の並びで 1 つ前の、同じ変種を許す種類で、`conditions.switch_from` に残す）の 2 種で、どちらも判定に使う
  - 1 試行は新しい context で、規模のデータへ差し替えてから、12 組 × 2 操作 × 20 回を流す。10 試行で組ごとに 200 値。試行 i は組の開始位置を i だけずらす。操作の間に 20ms 待つ
  - 各操作の直後に、dispatch が受理された、選んだ一覧の項目数が期待値（前・後。Node 側で JSON の行から数える。文書は手で数えた 80 / 97）と一致する、ほかの一覧が空で `hidden`、通知が出ていないことを確かめる。外れたら失敗にする
  - 統計と丸めは再描画と同じ。判定の規則は「ビューごとに、`toggle` と `switch` の両方で 95 パーセンタイルが 16ms 以内の変種が 1 つ以上あれば閾値内」（`conditions.judgement`。判定の対象 3 種と参考 4 種を分けて持つ）。超えても失敗にせず、標準エラーに within / over を出す。合否は `docs/testing.md` の記録に書く
  - 値は代役の部品の画面のもので、本体の `view()` の値ではない（ART-26 は別のイシュー）。CPU 4 倍（ART-15）も別のイシュー（`conditions.scope_note`）

- 配信する設定（`delivery`）の値と試験の設定（`test`）の値を比べない。再描画の値は試験の設定の値であり、配信する設定の値と比べない
- 回線条件・ブラウザの版・圧縮の実装が違う値を比べない。大きさ（GNU `gzip -9`）と初回表示・再描画（Node の zlib のレベル 9）は
  圧縮の実装が違うため、圧縮の設定が違う値どうしは比べない
- 教材（`baseline.pack`）・端末（`baseline.device`）が違う値どうしを比べない。`log-redraw`・`log-slope`・`log-rounds`・`input-paint` の値は最小教材の値で、教材全体を代表しない
- `view-redraw` の値は固定データ（`baseline.data`）の代役の部品の画面の値で、教材の値・本体の画面の値と比べない
- Hello の値は最小の画面の値で、教材全体の値ではない
- 記録に値を残すときは、基点の 4 項目を併記する

## 7. 将来の拡張

- JS glue・`index.html` など他の配信物の大きさは、`measurements[]` に足すだけでよい（様式の変更は不要）
- ビルド情報はビルドログの行（`config` / `wasm-opt` / `wasm size`）から読む。解析は `tools/measure/lib.sh` の `measure_build` に集約してある。
  機械可読の出力を `build-web.sh` に足す案は、ログの形式が変わって解析が壊れるようになった時点で検討する
- 大きさと初回表示は配信する設定を 2 回ビルドする（cargo のキャッシュが効くため差は数秒）。1 回のビルドの使い回しは後続の改善とする
- CPU 4 倍の減速・制限なしの対照・Chrome の原本での測定は、記録だけの条件として後続で扱う
- 保存の遅延（変更から保存先への書き込みまで。SAVE-7。TASK-318）はブラウザ試験 `save-latency` で測り、記録は `docs/testing.md` 5.168 にある。束（`measure-run/v1`）は `results` を 3 件に固定しているため入れていない。束へ統合するときは様式の版上げが要る。統計の定義（中央値・nearest-rank の 95 パーセンタイル）は `tools/measure/browser.mjs` と共有している
- `log-redraw`・`log-slope`・`log-rounds`・`input-paint`・`view-redraw` は束（`results` を 3 件に固定）に入れていない。束へ統合するときは様式の版上げが要る
- 操作の入る画面での操作可能まで（TTI）は、後続の画面のタスクで扱う
