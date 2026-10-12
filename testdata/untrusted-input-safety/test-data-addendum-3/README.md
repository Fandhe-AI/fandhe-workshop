# 追補 3: PoC-7 core（手順 5〜7）の試験データ

手順 1 で固定した `test-data/oversize/generate.sh` は書き換えません。足りないものだけを、実行より前にここへ固定します。

| ファイル | 内容 | 使う手順 |
|---------|------|---------|
| `make-addendum3.py` | 下の 2 つを決定的に生成する道具 | 5・7 |
| `cycle-ir/cycle-enter-self.json` | `use` の Step が自分自身を呼び出し先にする IR（`enter` の自己ループ）。開始は `s-enter-self` | 7 |
| `cycle-ir/cycle-enter-2.json` | `use` の Step 2 つが互いを呼び出し先にする IR（`enter` の 2 周期）。開始は `s-enter-2` | 7 |
| `cycle-ir/cycle-exit-2.json` | `final` の Step 2 つが `exit_to` で互いを指す IR。確認を省く Step で、入力 1 回で達成の連鎖が巡回する。開始は `s-exit-2`。起動したあと自然文を 1 回送る | 7 |
| `cycle-ir/cycle-d07-flat.json` | PoC-3 の欠陥 D07（`merge-gate` の最後の Step が `impl-run` を呼ぶ）の閉じた辺を、IR の辺として直接書いた IR（基は PoC-3 の golden IR）。開始は `s-d07` | 7 |
| `make-addendum3.py padded-ir <バイト数>` | 標準出力へ、ちょうど指定のバイト数の IR（PoC-1 の最小 IR の `pack.title` を `x` で埋める）。教材パックの合計 20MB・IR の大きさの上限の水準に使う | 5 |

## テストの中で作るもの（レシピだけを固定。生成物はコミットしない）

- **履歴の `ops` が多いもの**: 最小 IR で `Session` を作り、指定の種類の操作を N 回適用した `History` を書き出す。種類は 4 つ: `reload`（`Session::reloaded`）、`stale_tick`（宛先が合わない `tick`。無視のイベントが 1 件ずつ残る）、`draft_reload`（`set_draft` と `reloaded` の交互）、`text_draft`（20,000 文字の下書きを `set_draft` してから `reloaded` を交互に N/2 回）。N は 10,000・50,000・100,000・200,000。
- **上限を超えた `ops` の履歴**: 上の履歴の `ops` だけを上限 +1 件にした JSON（写しは元のまま）。上限の判定は適用し直しの前なので、写しが合わなくてよい。
- **教材パックの合計・Markdown 1 ファイル**: `engine-event-consistency/test-data/min-3step-pack/` を作業ディレクトリへ写し、`generate.sh pack` と `generate.sh markdown` の出力を足す。
- **深い入れ子・巨大な配列**: `generate.sh nested 10000`、`generate.sh bigarray 1000000`（IR の読み込み口と履歴の読み込み口の両方へ）。IR の中の配列（`nodes`）が巨大な場合は、`{"…":[` の形で包んで作る（レシピはテストのコードに書く）。
