# pack-defects

欠陥を 1 つだけ埋めた教材パックと、その土台の正しいパックです。`validate` が欠陥を期待の場所で報告することを、
`crates/pack/tests/defect_packs.rs` が確かめます（TASK-56・#415、仕様は PACK-29）。同じ試験が、正しいパックの
誤検出が 0 件であることも確かめます（#417）。
題材は架空の備品メモで、本 repo で新しく書きました（spec repo の教材は写していません）。

## 構成

```text
pack-defects/
├── base/format-a/   正しいパック（形式 A: TOML）
├── base/format-b/   同じ教材（形式 B: YAML）。IR は形式 A とバイト一致する
├── v1/{a,b}/D01〜D12/   欠陥ごとの差し替えファイル（パックの根からの相対パスで 1 ファイルだけ置く）
├── base/format-b-v1_1/  語彙 v1.1 用の正しいパック（形式 B だけ。上の土台とは独立）
└── v1_1/b/D13〜D23/     語彙 v1.1 の欠陥ごとの差し替えファイル（同じ作り）
```

試験は、`base/format-<x>` を一時ディレクトリへ写し、その上に `v1/<x>/<ID>` を重ねて欠陥パックを組み立てます。
24 の完全な写しを持たないのは、土台を直したときに直す場所を 1 か所にするためです。
語彙 v1.1 の 11 種（#416）は、`base/format-b-v1_1` に `v1_1/b/<ID>` を重ねて組み立てます。

## 語彙 v1.1 の 11 種の欠陥（TASK-56 2/3・#416）

| ID | 埋め方 | 期待のコード | 期待の場所（項目） | 重ねるファイル |
|---|---|---|---|---|
| D13 | 誤答の選択肢の `on_wrong` を存在しない拒否にする | `E-REF-REJECTION` | ch-2 の章・`step:st-2-7` | 章 |
| D14 | 正答の選択肢に `on_wrong` を足す | `E-STRUCTURE` | ch-1 の章・`step:st-1-4` | 章 |
| D15 | 未知の枠名を書く | `E-SLOT-UNKNOWN` | `workshop.yaml`・`rejection:rj-default-unknown-command` | `workshop.yaml` |
| D16 | `wrong_order` の文面に busy 専用の枠を書く | `E-SLOT-CONTEXT` | `workshop.yaml`・`rejection:rj-default-wrong-order` | `workshop.yaml` |
| D17 | 代替の文が要る枠に代替を書かない | `E-SLOT-NO-FALLBACK` | `workshop.yaml`・`rejection:rj-default-unknown-command` | `workshop.yaml` |
| D18 | `step_do` を使う文面が出うる Step から `do` を外す | `E-SLOT-NEEDS-DO` | ch-2 の章・`step:st-2-3` | 章 |
| D19 | 枠を閉じない | `E-SLOT-SYNTAX` | `workshop.yaml`・`rejection:rj-default-busy` | `workshop.yaml` |
| D20 | `body_levels` を存在しないファイルにする | `E-REF-BODY` | ch-13 の章・`step:st-13-2` | 章 |
| D21 | 水準 B の本文に `skill` 種のコマンド名を書く | `E-GUIDANCE-LEAK` | ch-13 の章・`step:st-13-2` | 水準 B の本文 |
| D22 | ヒントを水準 B・C だけに出し、水準 A で 0 件にする | `E-HINT-HIDDEN` | ch-13 の章・`step:st-13-2` | `workshop.yaml` |
| D23 | 開始点の `guidance` を未定義の水準にする | `E-PARSE` | ch-13 の章・`scope` | 章 |

判定は上の 12 種と同じです。11 種の定義は spec 側（private）で固定されており、ここには ID と要旨だけを置きます。

- 形式 B だけです（spec の定義が形式 B 限定のため）。
- 土台を別に置いたのは、既存の土台に v1.1 の要素を足すと、D01〜D12 の 24 の差し替えファイルが足した要素を
  巻き戻すためです。章のディレクトリ名・題材・D21 で書くコマンド名は本 repo で新しく書いた架空のもので、
  欠陥の定義・期待のコード・期待の場所は変えていません。
- 今の必須の検出は `validate` の 8 種（D13〜D20）で、8/8 です。D21〜D23 は検査器の側が TASK-49（#495・#496）の
  持ち分で未実装のため、検出の試験を `#[ignore]` で置いてあります。11/11 は TASK-49 の完了後に外して確かめます。
  D21〜D23 のパックが期待外の診断を出さないことは、今の試験で確かめています。

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

## 正しいパックの誤検出 0 件（TASK-56 3/3・#417）

正しいパックに診断が 1 件も出ないことを、コードを問わず `defect_packs.rs` の `pack29_correct_packs_*` が確かめます。

| パック | 形式 | `validate` | `test` の段の静的な検査 | CLI `test` |
|---|---|---|---|---|
| `base/format-a` | A | 対象 | 対象 | 対象（`#[ignore]`） |
| `base/format-b` | B | 対象 | 対象 | 対象（`#[ignore]`） |
| `base/format-b-v1_1` | B（語彙 v1.1） | 対象 | 対象外 | 対象外 |

- `test` の段の静的な検査は、`E-PLAY-INPUT-CLASS`（`check_play_inputs`）と誤入力の `expect` の読み取り
  （`check_error_input_expects`）です。教材テストが読めずに空になる経路を取り違えないよう、教材テストが読めること・
  誤入力が 3 件読めることも確かめます。
- `base/format-b-v1_1` は `tests/` を持たず、`test` の段の検査が空振りになるため対象外です。教材テストを足して
  対象に加えることは、本 PR では行っていません。
- 2026-10-18 時点の結果は、`validate` が 0 件／3 パック、`test` の静的な検査が 0 件／2 パックです。
- CLI `test`（模範解答の完走・誤入力の分類を含む）の誤検出 0 件は、`test` が TASK-404（#648）で未実装のため
  `#[ignore]` で置いてあります。完了後に外して確かめます。

## 試験の限界

2026-10-18 時点で、確認した事実と未確認の点を分けて記録します。仕様（private）は ID とパスで参照し、本文は写しません。

- 欠陥の作成者と、検査器の作成者は、同じ AI のエージェントです。作成者の思い込みが欠陥と検査器の両方に入りうる
  ため、検査の盲点を試験では見つけられません。独立した第三者による欠陥の作成は行っていません
  （PACK-29 の証拠の種類、AUTHOR-16、`03-poc/README.md` の 5 節）。仕様が補う手段として挙げる人の試行・公開前の
  第三者レビューは TASK-425 の持ち分で、本 PR では行っていません。
- 試験が示すのは、1 つずつ埋めた欠陥を期待の場所で検出したことだけです。複数の欠陥の組み合わせと、試していない
  種類の欠陥は対象外です。静的検査で拾えない種類（PACK-30・D-43 の範囲）は教材テストで補う方針で、本試験の
  対象外です。
- 誤検出 0 件は、本 repo で書いた小さな架空の土台についての結果です。実際の教材での誤検出 0 件は示しません。
- 判定はコード・ファイル・項目の一致で、診断の文面の質は評価しません。副次の診断は成否に入れません。
- 未完了: D09・D10（TASK-404・TASK-398）、D21〜D23（TASK-49）、CLI `test` の誤検出 0 件（TASK-404）は
  `#[ignore]` です。現在の必須の検出は 20/24 と 8/11 で、親 #414 の 24/24・11/11 は未達です。
- D12 は仕様の定義と違い `ir_format` を 3 で埋めています（「仕様との差」を参照）。
- 人の確認・承認は得ていません（TASK-56 に人間の承認点はありません）。
