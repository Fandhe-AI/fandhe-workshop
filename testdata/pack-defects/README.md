# pack-defects

欠陥を 1 つだけ埋めた教材パックと、その土台の正しいパックです。`validate` が欠陥を期待の場所で報告することを、
`crates/pack/tests/defect_packs.rs` が確かめます（TASK-56・#415、仕様は PACK-29）。
題材は架空の備品メモで、本 repo で新しく書きました（spec repo の教材は写していません）。

## 構成

```text
pack-defects/
├── base/format-a/   正しいパック（形式 A: TOML）
├── base/format-b/   同じ教材（形式 B: YAML）。IR は形式 A とバイト一致する
└── v1/{a,b}/D01〜D12/   欠陥ごとの差し替えファイル（パックの根からの相対パスで 1 ファイルだけ置く）
```

試験は、`base/format-<x>` を一時ディレクトリへ写し、その上に `v1/<x>/<ID>` を重ねて欠陥パックを組み立てます。
24 の完全な写しを持たないのは、土台を直したときに直す場所を 1 か所にするためです。
`v1/` の段は、語彙 v1.1 の欠陥（#416）を並べるために空けてあります。

## 12 種の欠陥

| ID | 埋め方 | 期待のコード | 期待の場所（項目） |
|---|---|---|---|
| D01 | 遷移先を存在しない Step にする | `E-REF-STEP` | 章・`step:st-2-4` |
| D02 | 存在しないビューを参照する | `E-REF-VIEW` | 章・`step:st-2-1` |
| D03 | 存在しない練習データを参照する | `E-REF-FIXTURE` | 章・`step:st-2-2` |
| D04 | 存在しないヒントを参照する | `E-REF-HINT` | 章・`step:st-2-3` |
| D05 | どこからも遷移されない Step を足す | `E-UNREACHABLE` | 章・`step:st-2-orphan` |
| D06 | 終わりでも遷移先もない Step を、条件つきの辺から足す | `E-DEAD-END` | 章・`step:st-2-stub` |
| D07 | 内側のモジュールから外側を呼び、巡回させる | `E-MODULE-CYCLE` | 内側のモジュール・`step:u-loop` |
| D08 | 拒否の文面の `next_action` を空にする | `E-EMPTY-NEXT-ACTION` | `workshop.*`・`rejection:rj-missing-arg` |
| D09 | 模範解答の画面操作を存在しない要素にする | `E-PLAY-INCOMPLETE` | `tests/playthrough.*`・`playthrough:pt-main` |
| D10 | 誤入力の期待する分類を誤らせる | `E-PLAY-CATEGORY-MISMATCH` | `tests/playthrough.*`・`error_input:e03` |
| D11 | 入力を持つ Step から確認を取り除く | `E-NO-CONFIRM` | 章・`step:st-2-7` |
| D12 | `pack.ir_format` を 3 にする | `E-FORMAT-VERSION` | `workshop.*`・`pack:ir_format` |

検出の判定は、コード・ファイル・項目の 3 つがすべて一致する診断が 1 件以上あることです。副次の診断は成否に入れません。
12 種の定義は spec 側（private）で固定されています。ここには ID とパスで参照するだけで、本文は転記していません。

## 仕様との差

- D12 は、spec の定義では `ir_format` を 2 にします。本 repo は版 1 と 2 の両方を読む（D-52）ため、2 では欠陥に
  なりません。そのため 3 にしています。欠陥の名前・期待のコード・期待の場所は変えていません。
- D09・D10 は `fandhe-pack test` でしか見つからない欠陥です。`test`（TASK-404・#648）と `E-PLAY-*` の扱い
  （TASK-398・#468）が未実装のため、検出の試験は `#[ignore]` で置いてあります。現時点で確かめているのは
  静的な 10 種 × 2 形式 = 20 パックで、24/24 は両タスクの完了後に `#[ignore]` を外して確かめます。
  D09・D10 のパックが静的検査を通る（他の欠陥を混ぜていない）ことは、今の試験で確かめています。
