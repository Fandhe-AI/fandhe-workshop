# contract

公開 API の契約（`CONTRACT_VERSION` = 3）に対する、イベント列のサンプルです。表示・保存・入力判定・案内の担当が
`apply()` へ結線するときの合否の基準になります（TASK-205）。JSON はコメントを書けないため、説明は本ファイルに置きます。

- サンプル: `event-sequence.json`（版 3）
- 読む部品: `crates/core/tests/common/contract_sample.rs`
- 形の試験: `crates/core/tests/contract_sample.rs`（TASK-205 1/2）
- 適合試験: `crates/core/tests/contract_conformance.rs`（TASK-205 2/2）。EVT-22 の決定性と EVT-6 の世代を確かめます。サンプルを連鎖して流す厳密な照合は、`apply()` が骨格の間は失敗するため `#[ignore]` で、結線の確認は `cargo test --locked -p fandhe-workshop-core --test contract_conformance -- --include-ignored` で行います（`docs/engine-api.md` の 7 節）

spec は private のため、本文を写さず ID（EVT-6・EVT-10・EVT-22・TASK-205）で参照します。

履歴の包み（`HistoryEnvelope`）のサンプルは `history/` にあります（説明は `history/README.md`、TASK-309 3/3）。

## 構成

教材は `engine-event-consistency/test-data/min-3step.json`（最小 3 Step の IR）です。`ir` 欄に、`testdata/` からの
相対パスで書いてあります。操作は 21 件（`e01` から `e21`）で、前の操作の `after` が次の `before` になります。

| 欄 | 意味 |
|---|---|
| `sample_version` | サンプルの版（整数） |
| `contract_version` | 対象の `CONTRACT_VERSION` |
| `ir` | 教材の IR |
| `initial` | 最初の `SessionState` 全体（世代 1、現在の Step は `main/st-1`、ログは空） |
| `entries[].id` | `e01` から始まる連番 |
| `entries[].kinds` | 操作の種類。`accept`（受付）・`reject`（拒否）・`confirm`（確認）・`achieve`（達成）・`progress`（進行）・`stop`（停止）・`move`（移動）・`save`（保存）・`restore`（復帰）のうち該当するもの |
| `entries[].evt6` | EVT-6 の (1)〜(6) のどれか（1〜6） |
| `entries[].note` | 日本語の短い説明 |
| `entries[].op` | `apply()` へ渡す `Op` |
| `entries[].before` / `after` | 適用前後の `RunState` |
| `entries[].events` | 期待のイベント列（順序つき） |

前後の状態に `RunState` を使うのは、契約の型を変えたときにサンプルが読めなくなり、版上げを強制できるためです。
`log` は `events` の累積、`checkpoints` は `CheckpointSaved` から分かります。

## 版の規則

- `CONTRACT_VERSION` を上げたら、`sample_version` を上げ、`contract_version` を合わせます。形の試験が食い違いを検出します
- イベントの順序・欄を変えたら、`sample_version` を上げます
- 旧版は残さず、下の変更履歴に記録します

## サンプルが決めた細目

根拠は試作（PoC-1）のイベントの順序です。担当タスク（TASK-207・210・211・213・214・215）が別の決定をした場合は、
版上げの契約タスクにします。

- `Op::Input` は、`InputReceived` の直後に `Judged` を 1 件だけ置きます（`Accepted`・効果・`Achieved` より前。TASK-267）。判定できない宣言、受付・確認の判定の後に遷移が続かない場合、取り消しの受付の規則（受付の処理は TASK-212 が結線する）では、`Judged` を置かず `Warning` を出します。結果は 6 種のどれか 1 つで、拒否・問い返しには空でない次の行動が付きます
- 取り消し（`Input::Cancel`）は、教材の規則に `on.cancel` が無ければ `not_in_this_step` の拒否になります（EVT-10。e21）
- 宛先が一致する `Tick` は `Judged` を出さず `Progress` だけを出します（TASK-210 2/3 が決めました）。宛先が食い違う `Tick` は `Judged` を出さず `Ignored` を 1 件だけ出します（TASK-210 3/3 が決めました。サンプルの形のままです）。`Tick` の項目は版 2 のままです
- `Changed` は操作のイベントの最後に、1 経路 1 件で並びます。`at` は効果を適用した Step の時点で、無ければ操作後の世代と現在の Step です
- `Stopped`・`Moved`・`Restored` の `at` は操作前の時点です
- `Progress.index` は 0 始まりです。`Tick` は `InputReceived` を出しません
- `CheckpointSaved.log_len` は、保存のイベントを足す前のログの長さです
- 処理中でない停止は `InputReceived{cancel}` で、世代を変えません（EVT-10）
- 宛先が古い進行は `Ignored{stale}` を 1 件だけ出し、状態を変えません
- `Restored.owner` は復帰後の現在の Step です

開始（`Control::Begin`）は含めていません。現在の Step が無い時点の `Stamp` の書き方を、版 1 の型が決めていないためです。
また `Begin` は EVT-6 の (1)〜(6) の外です。

## 変更履歴

| サンプルの版 | 日付 | 内容 |
|---|---|---|
| 1 | 2026-10-15 | 新規（TASK-205 1/2）。21 操作で 9 種と EVT-6 の (1)〜(6) を覆う |
| 2 | 2026-10-15 | `CONTRACT_VERSION` 2 に追従（TASK-290）。`RunState.guidance_level`（全件 `A`）と `SessionState.hints_opened`（空）を足した。操作・イベントは変えない |
| 3 | 2026-10-15 | `CONTRACT_VERSION` 3 に追従（TASK-267）。入力の項目（e01・e02・e03・e08・e15・e16・e21）の `InputReceived` の直後に `Judged` を足した。`Tick`・`Control` の項目は変えない |
