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

## 左のビューの再描画の測定（TASK-485 1/3）

`diff-500.json`・`tasks-50.json`・`filetree-200.json`・`doc-large-before.md`・`doc-large-after.md`・`doc-large-before.json`・`doc-large-after.json` は、部品単位の再描画の速さ（ART-4）を測るための規模のデータです。

- 出所: 先の 5 つは `generate.mjs` の出力の写しです。JSON は 1 行のまま末尾に改行を足し、Markdown はバイト一致です。`doc-large-*.json` は、`doc-large-*.md` を `fandhe_workshop_pack::markdown::parse` で木にした `contract::doc::Doc` の JSON です（実行時に Markdown は解釈しないため、ビルド時の出力を写しで持ちます）
- 整形しない理由: 差分 500 行は整形すると約 3 倍になり、リポジトリを不必要に膨らませるためです
- 内訳（前に出る行 / 全行）: 文書 80 ブロック / 97 行（最上位のブロックを突き合わせた後）、差分 479 / 500、タスク 46 / 50、ファイルツリー 191 / 200。`generate.mjs` の出力を数えた値（2026-10-27）で、spec の PoC-8 の表と一致します
- 整合の確かめ方: `view_fixture.rs` の単体試験（木の写しと `markdown::parse` の一致、差し替え後の行の数）と、`tools/browser/view-scale.test.mjs` の VS1（`generate.mjs` の出力との一致）・VS2（画面の項目数）が確かめます
- 使われ方: `crates/web/src/view_fixture.rs` が `include_bytes!`・`include_str!` で読み（`test-hooks` の下だけ。配信物には入りません）、`fixture_data` の payload `filetree:200`・`diff:500`・`tasks:50`・`document:200` で差し替えます。`tools/measure/view-redraw.mjs` と `tools/measure/lib.sh` の `measure_data_json`（SHA-256 を基点に残す）が使います
- `testdata` に置かない理由: 上と同じです（`testdata/` は spec の写しで、大きなファイルは置かれていません）

## アプリのデモの名札（TASK-480 2/2）

`appdemo-nameplate.json` は、アプリのデモのビューで名札の行（世界状態の経路 `screen/top`）が指せる行になることを検査するためのデータです。

- 出所: spec の試作の教材 CH-0 の ST-0.1 の `on_enter` が置く 3 つの要素（`screen/top`・`screen/left`・`screen/right`）の `title` と `role` を写したものです。`role` が `heading` の行は kind `heading`、`item` の行は kind `item` にしています
- 内訳: 3 行で、前後は同じ中身です（印は付きません）。指せる行は `screen/top` だけで、`screen/left`・`screen/right` は表示だけの負の対照です（`view_fixture.rs` の宛先の規則 `Only`）
- `testdata` に置かない理由: 上と同じです。spec の写しではなく、実装側が足すデータです
- 使われ方: `crates/web/src/view_fixture.rs` が `include_bytes!` で読み（`test-hooks` の下だけ）、`fixture_data` の payload `appdemo:nameplate` で差し替えます。`tools/browser/point-click.test.mjs` の PC3 が使います

## アプリのデモの模擬ログイン（TASK-492）

`appdemo-login.json` と `appdemo-login-contrast.json` は、アプリのデモのビューに模擬ログインの画面を渡し、パスワード欄とパスワードを求める文言が 0 件であることを検査するためのデータです（ART-24・ROUND1-25）。

- 出所: 教材 repo の掲示板のデモの定義（TASK-664）がまだ無いため、ART-24 と ROUND1-25 の期待（名前を選ぶだけで入れる）から作った実装側の代役です。本物のデモの写しではありません
- 内訳: `appdemo-login.json` は 7 行（前 6・後 7）です。名前の行 2 件（`login/name-1`・`login/name-2`。kind `item`）、見出し・説明・名前の欄・入るボタン、後だけに出る一覧の行です。`appdemo-login-contrast.json` は、これに label が「パスワード」の欄 `login/secret` を 1 件足した負の対照（前 7・後 8）で、検出器が空振りしていないことを示します
- 指せる行: 名前の行の `login/name-1`・`login/name-2` だけです。それ以外は表示だけの負の対照です（`view_fixture.rs` の宛先の規則 `Only`）
- `testdata` に置かない理由: 上と同じです。spec の写しではなく、実装側が足すデータです
- 使われ方: `crates/web/src/view_fixture.rs` が `include_bytes!` で読み（`test-hooks` の下だけ）、`fixture_data` の payload `appdemo:login`・`appdemo:login-contrast` で差し替えます。`crates/web/tests/app_demo_login_contract.rs` と `tools/browser/app-demo-login.test.mjs` が使います
