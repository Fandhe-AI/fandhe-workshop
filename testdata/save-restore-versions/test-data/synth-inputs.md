# 合成した 2 周分の入力列（手順 5。実装の前に固定）

## 1. 作り方

- 元は PoC-4 の模範解答（`../../module-reuse-rounds/test-data/pack/tests/playthrough.yaml` の `pt-rounds`）と同じ道筋を通る、誘導つきの操作（`tests/poc6_common/mod.rs` の `guided`。PoC-4 の `poc4_common` の写し）。1 周目は `s-r1` から完走、`Control::Begin("s-r2")`、2 周目を完走する。
- **誤入力を 3 割**混ぜる: 誘導つきの 1 手を行う前に、乱数（`proptest-seed.txt` の ChaCha。種を `0x00` 詰めの 8 バイトの通し番号で分ける）で 0〜1 を引き、0.3 未満なら誤入力を 1 つ先に送る。誤入力の候補（毎回、一様に選ぶ）:
  1. `Command("/advance-phase")`
  2. `Command("/no-such-command")`
  3. `Command("/new-idea")`（引数なし）
  4. `Choice("no-such-option")`
  5. `Ui { view: "v-files", path: "nonexistent/path" }`
  6. `Text("短")`
- 平均して、模範解答の 1 手につき 0.3 回の誤入力（全体の入力の約 23%）が入る。「3 割」は**模範解答の手に対する誤入力の割合**と定義する（入力の全体に対しては約 23%）。この定義は実施の前に固定する。
- 履歴らしさのための付加操作（容量の上限側を見るため）: 誘導つきの 6 手ごとに `Control::Save`（チェックポイント）、4 手ごとに `Control::OpenHint`（開けるなら）、12 手ごとに `Draft` の入力。付加操作は条件 B で入れる。

## 2. 条件

| 条件 | 内容 |
|------|------|
| A（計画どおり） | 模範解答＋誤入力 3 割。付加操作なし。チェックポイントは 2 周で 2 件（1 周目の終わり、2 周目の終わり） |
| B（上限側） | A に付加操作（6 手ごとのチェックポイント保存、ヒント、下書き）を足す |
| C（最悪側） | B で、全部の入力の直後にチェックポイントを保存する（1 手ごとの `Save`） |

判定は A で行う（計画の手順 5）。B・C は余裕を見るための記録で、判定に使わない。種は条件ごとに別（通し番号 `A=1`、`B=2`、`C=3`）。各条件で乱数の種を 5 つ（通し番号の下位）変え、5 回の大きさの最小・中央値・最大を記録する。

## 3. 測るもの（`../measurements/history-size.csv`。列は `condition,seed,ops,log_entries,total_bytes,changed_entries,changed_bytes,ops_bytes,snapshot_bytes,checkpoints,checkpoint_bytes`）

- 履歴ファイル全体の JSON のバイト数。
- `Changed` のイベントの数と、そのバイト数（ログの中の `Changed` を 1 件ずつ JSON にした長さの合計）。
- `ops` の部分、`snapshot` の部分、チェックポイントの部分のバイト数。
- 5MB（5 × 1024 × 1024 バイト）との比。
