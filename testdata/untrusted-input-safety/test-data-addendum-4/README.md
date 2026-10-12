# 追補 4: PoC-7 web の回（手順 3・4・5〜7 のブラウザ・8・9）の試験データ

実行より前に固定します。固定済みの `test-data/`・追補 1〜3 は書き換えません。時刻と SHA-256 は `../env.md`。

| ファイル | 内容 | 使う手順 |
|---------|------|---------|
| `secret-shapes.json` | `test-data/secret-like.md` の 10 形の部品（そのまま）、期待する形の ID、対照 N-01・N-02、追加の対照 N-03〜N-06（過剰な検出がないかの記録。成功基準には含めない）。完成した値は書かない | 9 |
| `extra-cases.json` | 手順 3 の E6 の位置（クエリの各名前・ハッシュ）、`?ir=` に入れる固定の値 5 件、`data-payload` の書き換えの対象と値 | 3 |

## 入り口へのペイロードの入れ方（実行より前の取り決め）

`test-data/xss-cases.md` の「入り口」の欄に挙がるケースだけを、その入り口に入れます（E1〜E6）。

| 入り口 | 入れる位置 | 件数 |
|--------|-----------|-----|
| E1（教材パックの Markdown） | 今の本体の経路: 教材パックの Step の本文（`.md`）のすべてへ、そのケースのペイロードを書き、`fandhe-pack compile` で IR にし、IR に本文の中身が入らないこと（ペイロードの文字列が IR に出ない）と、画面に本文の中身が出ないことを数える。方式 (c) の木: `prototype/experiments/markdown-compare/` の変種 c を、手順 2 と同じ道具で 49 件流し直す | 49 |
| E2（構造定義の文字列） | IR の表示される文字列の欄（`pack.title`・`step.title`・`step.do`・ヒント `text`・拒否の `message`/`next_action`・問い返しの `message`/`how_to_answer`・選択肢の `label`・`refs[].name`/`link`）のすべてへ同じペイロードを入れた IR（掲示板教材〔形式 B〕の IR を基にする）を開く。加えて、ID 類（コマンド名・ヒントの ID・選択肢の ID）は、IR の中の同じ文字列の全出現を置き換えた IR（参照の整合を保つ）。構造定義の YAML に入れる経路は XSS-55（アンカー）と数件で `fandhe-pack compile` を通す | 55 |
| E3（練習データ） | 世界状態に置く欄: `put/path`、`put/attrs/*` の値、`set_attr/value`（練習データの本文は IR に入らず参照のパスだけなので、本文の経路は E1 と同じ扱い） | 50 |
| E4（履歴の読み込み） | 本体が書き出した履歴の JSON の文字列の欄（`ops` の入力・下書き、`snapshot` の写し、`start`、`pack_id`、`versions.content`）のすべてへペイロードを入れて、履歴を読み込む口（ファイル選択と、保存先からの復元）に渡す | 14 |
| E5（入力欄） | 入力欄へ入力して送信（拒否・問い返しの文面への `{{input_head}}` の差し込みを含む。語の先頭に置く形と置かない形）。加えて `extra-cases.json` の `e5_data_payload` の書き換え | 49 |
| E6（URL のクエリ・ハッシュ） | `extra-cases.json` の位置 × `e6_payloads`、`ir_param_fixed` | 5＋上記 |

数えるもの: 記録用の関数（`window.__poc7_hit`）の呼び出し、配信元（`127.0.0.1`）以外への通信の試み（`route` で中止して数える）、CSP の違反（`securitypolicyviolation`）、`dialog`・popup・ページの例外・タブの異常終了（`crash`）、DOM の危険な要素・`on*` 属性・`http(s)` 以外の URL。

## 手順 5〜7（ブラウザ）の水準と測り方（実行より前の取り決め）

- **IR（20MB）**: `make-addendum3.py padded-ir` で、ちょうど 20,971,520 バイト・1 バイト超・10 倍（209,715,200 バイト）。静的サーバから `?ir=` で取りに行く。
- **履歴（10MB）**: ちょうど 10,485,760・1 バイト超・10 倍（104,857,600 バイト）。`generate.sh history` の出力（手順 1 で固定。履歴の形ではないので、大きさの上限が先に効くかを見る）を使う。履歴の形の大きなもの（`ops` の件数の試験）は、本体が書き出した履歴の `ops` を複製して作る。読み込みの口は 2 つ: ファイル選択（`setInputFiles`）と、保存先（IndexedDB）からの復元。
- **入力 1 回**: 20,000・20,001・200,000 文字（`generate.sh input`）。
- **応答時間の定義**: (a) データが JS の手に渡った（取得を終えた、またはファイルを読んだ）時点から、理由の文字が DOM に出るまで。(b) 画面の移動の開始から理由が出るまで（取得と WASM の起動を含む）。判定は (a)。(b) も記録する。
- **メモリ**: WASM の線形メモリ（`memory.buffer.byteLength`）と、JS のヒープ（`performance.memory.usedJSHeapSize`）を、読み込みの前後で記録する。
- **深い入れ子・巨大な配列**: `generate.sh nested 10000`、`generate.sh bigarray 1000000`、`generate.sh bigarray 5000000`（IR と履歴の両方の口に）。加えて、形の合う IR で配列が巨大なもの（`step.lo` の文字列を 100 万・500 万個入れる。IR の大きさの上限の中に収まる範囲）を、本体の読み込みに渡す。
- **巡回**: 追補 3 の `cycle-ir/` の 4 本を `?ir=` で開く。`cycle-exit-2` は起動のあと自然文を 1 回送る。

## 手順 8（CSP）の取り決め

- CSP は計画の案をそのまま `<meta http-equiv="Content-Security-Policy">` に入れる: `default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; img-src 'self'`。
- 配信する設定（試験用の読み出し口なし）と試験の設定の両方で、全 Step の自動操作を流す。`<meta>` で指定できない項目（`frame-ancestors`・`report-uri`・`sandbox`）は、別のページで `<meta>` に入れてブラウザの応答（コンソールの警告・無視）を記録する。
- 誘導の表は、既存の自動ブラウザテスト（`browser-test-poc6.mjs` の `guided`、`browser-test-poc2.mjs` の CH-1 の進め方）と同じものを使う。
- 「全 Step」は、最小教材（3 Step）、CH-1 の教材（6 Step）、掲示板教材の CH-2（8 Step）の誘導つきの操作で、到達した Step の数を数える。到達しなかった Step は未確認として記録する。

## 手順 9（秘密情報らしい警告）の取り決め

- `secret-shapes.json` の 10 形を入力欄に入力し、保存の前に警告が出ること、警告が出ている間は保存先に書かれないこと、「保存する」を選ぶと書かれること、「この入力を残さない」を選ぶと書かれないことを確かめる。対照 N-01・N-02 で警告が出ないこと。
- ログ・画面の記録・スクリーンショットには、形の ID と警告の有無だけを残す。
