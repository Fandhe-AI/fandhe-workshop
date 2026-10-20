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
- 配信する設定（`delivery`）の値と試験の設定（`test`）の値を比べない。再描画の値は試験の設定の値であり、配信する設定の値と比べない
- 回線条件・ブラウザの版・圧縮の実装が違う値を比べない。大きさ（GNU `gzip -9`）と初回表示・再描画（Node の zlib のレベル 9）は
  圧縮の実装が違うため、圧縮の設定が違う値どうしは比べない
- Hello の値は最小の画面の値で、教材全体の値ではない
- 記録に値を残すときは、基点の 4 項目を併記する

## 7. 将来の拡張

- JS glue・`index.html` など他の配信物の大きさは、`measurements[]` に足すだけでよい（様式の変更は不要）
- ビルド情報はビルドログの行（`config` / `wasm-opt` / `wasm size`）から読む。解析は `tools/measure/lib.sh` の `measure_build` に集約してある。
  機械可読の出力を `build-web.sh` に足す案は、ログの形式が変わって解析が壊れるようになった時点で検討する
- 大きさと初回表示は配信する設定を 2 回ビルドする（cargo のキャッシュが効くため差は数秒）。1 回のビルドの使い回しは後続の改善とする
- CPU 4 倍の減速・制限なしの対照・Chrome の原本での測定は、記録だけの条件として後続で扱う
- 保存の遅延（変更から保存先への書き込みまで。SAVE-7。TASK-318）はブラウザ試験 `save-latency` で測り、記録は `docs/testing.md` 5.168 にある。束（`measure-run/v1`）は `results` を 3 件に固定しているため入れていない。束へ統合するときは様式の版上げが要る。統計の定義（中央値・nearest-rank の 95 パーセンタイル）は `tools/measure/browser.mjs` と共有している
- 操作の入る画面での操作可能まで（TTI）と、入力から描画までの時間は、後続の画面のタスクで扱う
