# old_histories_1

旧形式（版 1 の IR・語彙 1.0）の履歴の凍結物です。今のエンジン（EP-13、語彙 1.4 を含む）で生成した試料であり、
EP-13 の追加前のエンジンが書いたものではありません（語彙 1.0 は教材の宣言に由来します）。証明できるのは、
旧形式の履歴を今のエンジンが開くことまでです。旧い履歴が新しい版のエンジンで開くことの試験
（`crates/pack/tests/old_pack_history_open.rs`、EXT-9・D-52・D-53、TASK-190 の 1/2）が読みます。
JSON はコメントを書けないため、説明は本ファイルに置きます。

## ファイル

| ファイル | 内容 |
|---|---|
| `min-3step-v1-progress.json` | 教材 `tests/data/ir_format_1/min-3step-pack.ir.json`（`format_version` 1・語彙の宣言なし＝既定 1.0）で記録した履歴 |

## 作り方（2026-10-19、元にしたコミット 6fc670a）

- 初期状態: 開始点 `s-1` の Step `main/st-1` を `Entered` にした状態（`Control::Begin` は未実装のため）。試験の `initial_at_start` と同じ
- 記録: `RecordedSession` で次の 4 手を記録し、`envelope_for_save()` → `export_bytes` で書き出したバイト列をそのまま置いた
  1. 下書き（`/make-note`）
  2. 受け付けられる送信（`/make-note first-note`）
  3. 処理中のため拒否される送信（`/make-note Bad_Name`。凍結 JSON の category は `busy`）
  4. 末尾の未送信の下書き
- `versions`: `engine` 0.0.0・`ir_format` 1・`vocab` 1.0・`content` 0.0.1
- 生成に使ったコードは試験に残していない（試験は読むだけで、書き戻さない）

## 書き換えない規則

凍結物を書き換えて試験を通してはいけません。書き換えが要るなら、旧い履歴が開かなくなった（互換が壊れた）ことを意味します。
その場合は原因を直すか、版の規則（D-52）に沿った扱いを決めてから、新しい凍結物を別の名前で足します。
