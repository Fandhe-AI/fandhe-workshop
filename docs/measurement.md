# 測定の結果の様式

```text
決定日: 2026-10-14
対象: TASK-32（WASM の gzip の大きさ・初回表示・再描画の測定。本文書は 1/2 で gzip の大きさと様式を扱う）
```

## 1. 位置づけ

- 測定の結果に付ける「基点」と、結果の JSON の様式を定める
- 基点の違う値どうしで増減を論じると誤った結論を招く。過去の PoC の記録には、commit・`wasm-opt` の有無・圧縮の設定が違う値が
  混在していた。結果の側で基点を必須にして、この事故を構造的に防ぐ
- 初回表示・再描画の測定は #92 で同じ様式に足す。CI の関門（WASM 1 MB）は TASK-78 の担当で、本文書の範囲外である

## 2. 実行

```bash
make measure-wasm-size            # 直接: tools/measure/wasm-size.sh
make measure-wasm-size | jq .
```

- release の設定で `tools/build-web.sh` を一時ディレクトリにビルドし、`gzip -9 -c` の大きさを測る（`dist/` は上書きしない）
- 結果の JSON を標準出力に 1 つだけ出す。進行状況とビルドログは標準エラーへ出す
- `TEST_HOOKS=1` で試験の設定、`SKIP_WASM_OPT=1` で `wasm-opt` なしを測る
- 終了コード: 0 = 成功 / 1 = ビルド・測定・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用
- 基点の 1 つでも欠けた結果は出さない（fail-closed）。出力は `tools/measure/lib.sh` の `measure_emit` だけが行う
- 回帰テストは `make test-measure-wasm-size`。実ビルドを含むため遅く、`make check`・フック・CI には入れない

## 3. 基点の 4 項目

| 項目 | JSON のパス | 内容 |
|---|---|---|
| commit | `baseline.commit` | `sha`（40 桁の 16 進）と `dirty`（未コミットの変更の有無） |
| `wasm-opt` の有無 | `baseline.wasm_opt` | `applied`（真偽）と `detail`（版、または未適用の理由） |
| 機能の範囲 | `baseline.features` | `config`（`delivery` / `test`）・`cargo_features`・`profile`（`release`） |
| 圧縮の設定 | `baseline.compression` | `command`（`gzip -9 -c`）と `implementation`（`gzip --version` の 1 行目） |

## 4. 結果の様式（`schema`: `fandhe-workshop/measure/v1`）

| パス | 型 | 内容 |
|---|---|---|
| `schema` | 文字列 | 様式の版。読む側が確かめる |
| `measured_at` | 文字列 | 測定時刻（UTC、`YYYY-MM-DDTHH:MM:SSZ`） |
| `baseline.*` | — | 前節の 4 項目 |
| `toolchain.rustc`・`toolchain.wasm_bindgen` | 文字列 | 再現の手掛かり。基点の 4 項目には含めない |
| `measurements[]` | 配列（1 件以上） | `{ name, file, unit, value }`。`value` は正の整数。現状は `wasm_raw` と `wasm_gzip`（単位は `bytes`） |

検証は `measure_validate`（`tools/measure/lib.sh`）が行い、`schema` の一致・基点の各項目の存在と型・`measurements` の値を確かめる。

## 5. 比較の規則

- 基点が 1 項目でも違う値どうしで、増減を論じない
- `baseline.commit.dirty` が `true` の値は、基点として比較に使わない（どの commit の値か特定できない）
- `wasm-opt` が無い環境でもビルドは成功し、`applied: false` として記録される。未適用の値を適用済みの値と比べない
- 配信する設定（`delivery`）の値と試験の設定（`test`）の値を比べない
- 記録に値を残すときは、基点の 4 項目を併記する

## 6. 将来の拡張

- JS glue・`index.html` など他の配信物の大きさは、`measurements[]` に足すだけでよい（様式の変更は不要）
- ビルド情報はビルドログの行（`config` / `wasm-opt` / `wasm size`）から読む。測定が増えて解析が重複する場合は、
  `build-web.sh` に機械可読の出力を足す案を検討する
