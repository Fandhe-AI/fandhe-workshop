# 規模の検査用データ

`cards-20.json` は、カードのビュー（TASK-477 2/2）を 20 件の規模で検査するためのデータです。

## 出所

`testdata/artifact-view-parts/test-data/generate.mjs` が出力する `cards-20.json`（乱数の種は固定）を、整形して写したものです。
20 行で、全行が前後の両方を持ち、変更は 8 行、印なしは 12 行です。ラベルは全行で一定です。

## testdata に置かない理由

`testdata/` は spec の写しで、SHA-256 で固定しています。実装側が足すデータは混ぜず、ここに置きます。

## 整合の確かめ方

`tools/browser/card-view.test.mjs` の CV8 が、`generate.mjs` を一時ディレクトリへ流し、出力と本ファイルを `JSON.parse` で比べます。
バイト列は比べません（本ファイルは整形し、末尾に改行を持つため）。

## 使われ方

- `crates/web/src/view_fixture.rs` が `include_bytes!` で読みます（`test-hooks` の下だけ。配信物には入りません）
- `crates/web/tests/card_view_b_contract.rs` が構成を確かめます

## チェックの一覧（TASK-478）

`checks-20.json` は、チェックの一覧のビュー（TASK-478）を 20 件の規模で検査するためのデータです。

- 出所: `generate.mjs` が出力する `checks-20.json` を整形して写したものです
- 内訳: 20 行で、全行が前後の両方を持ちます。補足が全行で変わるため、後の印は変更 20・追加 0・削除 0・印なし 0 です
- 整合の確かめ方: `tools/browser/check-view.test.mjs` の CK8 が、`generate.mjs` の出力と本ファイルを `JSON.parse` で比べます
- 使われ方: `crates/web/src/view_fixture.rs` が `include_bytes!` で読み（`test-hooks` の下だけ）、`crates/web/tests/check_view_b_contract.rs` が構成を確かめます
