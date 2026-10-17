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

## アプリのデモの名札（TASK-480 2/2）

`appdemo-nameplate.json` は、アプリのデモのビューで名札の行（世界状態の経路 `screen/top`）が指せる行になることを検査するためのデータです。

- 出所: spec の試作の教材 CH-0 の ST-0.1 の `on_enter` が置く 3 つの要素（`screen/top`・`screen/left`・`screen/right`）の `title` と `role` を写したものです。`role` が `heading` の行は kind `heading`、`item` の行は kind `item` にしています
- 内訳: 3 行で、前後は同じ中身です（印は付きません）。指せる行は `screen/top` だけで、`screen/left`・`screen/right` は表示だけの負の対照です（`view_fixture.rs` の宛先の規則 `Only`）
- `testdata` に置かない理由: 上と同じです。spec の写しではなく、実装側が足すデータです
- 使われ方: `crates/web/src/view_fixture.rs` が `include_bytes!` で読み（`test-hooks` の下だけ）、`fixture_data` の payload `appdemo:nameplate` で差し替えます。`tools/browser/point-click.test.mjs` の PC3 が使います
