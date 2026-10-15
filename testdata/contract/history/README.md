# contract/history

履歴の包み `HistoryEnvelope`（`HISTORY_CONTRACT_VERSION` = 1）のサンプルです。書き出し→読み込みの往復でログの並びと位置が
変わらないことの基準になります（TASK-309 3/3）。保存・読み込みの担当（TASK-174・TASK-310・TASK-315・TASK-330）が参照します。
JSON はコメントを書けないため、説明は本ファイルに置きます。

- サンプル: `empty.json`・`two-rounds.json`・`draft-rejected.json`（サンプルの版 1）
- 読む部品: `crates/core/tests/common/history_sample.rs`
- 適合試験: `crates/core/tests/history_envelope_conformance.rs`（core の型で往復・バイト一致・性質テスト・比較器の自己確認）
- 補助の試験: `crates/contract/tests/history_envelope_samples.rs`（core に依存せず、最上位のキーの順・`ops` のタグ・必須欄を確かめる）
- 変異: `crates/core/tests/mutations/SAVE2-1.mutation`（読み込みでログの並びを変える。PoC-6 の X-6 に当たる。記録は `docs/testing.md` の 5.7）
- 型と JSON の形の表: `crates/contract/src/history.rs` の rustdoc

spec は private のため、本文を写さず ID（SAVE-2・SAVE-17・SAVE-18・SAVE-24・D-52・D-54・TASK-9・TASK-309）で参照します。

## サンプル

教材は `engine-event-consistency/test-data/min-3step.json`（`pack_id` は `min-3step`、`start` は `s-1`）です。
`versions` は `HistoryVersions` の形で、`content_hash` は `content_hash()` で IR から求めた値です（試験が同じ関数で照合します）。

| ファイル | 内容 |
|---|---|
| `empty.json` | 空の履歴。`ops` は `[]`、`snapshot` は開始直後の状態（ログは空、世代 1、入口の写しは `log_len` 0） |
| `two-rounds.json` | 2 周分の履歴。1 周目を `ChapterCompleted` まで進め、`Control::Begin("s-1")` で 2 周目へ入って再び完了し、最後に `Restore(1)` で 1 周目のチェックポイントへ戻る。`submit`・`reload`・`Save`・`Restore` を含む |
| `draft-rejected.json` | 下書きと拒否を含む履歴。形の誤りで拒否された送信（`InputReceived` だけで `Accepted` が続かない）、存在しない番号への `Restore` の `Refused`、末尾の未送信の `draft` を含む |

## 欄の意味

| 欄 | 意味 |
|---|---|
| `format` | 形式名。`fandhe-workshop-history` |
| `history_format` | 包みの形の版。1 |
| `pack_id`・`start` | 教材の ID と最初の開始点の ID |
| `versions` | 4 軸の版の欄（`engine`・`ir_format`・`vocab`・`content`・`content_hash`・`refs`） |
| `ops` | 入力の列。`submit`・`draft`・`reload`・`input`・`tick`・`control` の 6 種だけ。順を変えない |
| `snapshot` | 実行環境の全体の写し（core の `SessionState`）。ログの位置は `log_len` が指す |

`snapshot` の中の形は core の `CONTRACT_VERSION`（`docs/engine-api.md`）が正です。サンプルは `to_vec_pretty` に
末尾の改行を 1 つ足した形で、試験がバイト一致を確かめます。`HistoryEnvelope` は未知の欄を拒否するため、
`event-sequence.json` のようにサンプルの版を欄へ埋め込めません。版は下の変更履歴の表で管理し、試験は
`format`・`history_format` を定数と照合します。

## 版の規則

- `HISTORY_CONTRACT_VERSION` を上げたら、サンプルを新しい形で書き直し、サンプルの版を上げます
- イベントの並び・欄、`ops` のタグ、`snapshot` の形を変えたら、サンプルの版を上げます
- 旧版は残さず、下の変更履歴に記録します
- 欄を足す必要が出たら、黙って足さず、契約の版上げの課題にします

## 主張しないこと

- `ops` を `apply()` へ適用し直した結果が `snapshot` と一致することは主張しません。`apply()` が骨格の間は確かめられず、
  照合は TASK-174・TASK-310・TASK-315 の責務です。サンプルのイベントと状態は、TASK-205 のサンプルから手で組み立てています
- 拒否理由コードと読み込みの判定の順序（TASK-310）、長さ・件数の上限（読み込み口）は扱いません

## サンプルが決めた細目

TASK-205 の `event-sequence.json` の並べ方と細目に揃えています。次は、それに無く本サンプルが決めたものです。
担当タスク（TASK-214、TASK-233〜239）が別の決定をした場合は、版上げの契約タスクにします。

- Enter の送信は `{"submit": …}` だけで残り、直前に同じ文字列の `{"draft": …}` が並びます（SAVE-24）。送信後は `drafts` から外れます
- 未送信の下書きは `snapshot.drafts` に根ごとに残り、書き出しに入ります（SAVE-18）
- `CheckpointSaved.log_len` は、保存のイベントを足す前のログの長さ（そのイベントの添字）です。`checkpoints[id].log_len` と一致します
- `Restored.log_len` は、復帰先のチェックポイントの `log_len` です。ログは復帰しても削られません
- 入口の写しの `log_len` は、最後の `Entered` の添字です。開始直後で `Entered` が無いときは 0 です
- 2 周目は、`Control::Begin` が `Begun` と `Entered` を出し、実行の状態を開始直後へ戻します（世代は進みます）
- `ChapterCompleted` は、章の最後の Step を達成した操作のイベントの最後に並びます
- 2 周目のチェックポイントの ID は、1 周目の次の番号（2）です

## 机上の照合

TASK-9（D-52）の決定と、スキーマ（型・サンプル）の欄を対応させました。食い違う欄は 0 件です（2026-10-15）。

| 決定・振る舞い | スキーマ上の表れ | 確かめる試験 |
|---|---|---|
| SAVE-2: 包みは写しと 4 軸の版を持ち、ログの並びを変えない | `HistoryEnvelope`。`ops` は `Vec`、`log` は写しの中の `Vec` | `samples_roundtrip_preserves_log_order_and_positions`・`property_roundtrip_preserves_log_order` |
| SAVE-17・D-52: 書き出しは `ops` と写しの両方。`ops` だけは拒否 | `snapshot` は必須の欄。`EXPORT_FORM` は `OpsAndSnapshot` | `history_envelope_samples` の `ops_only_and_unknown_fields_are_rejected` |
| SAVE-18: 未送信の下書きは書き出しに入る | 末尾の `{"draft": …}` と `snapshot.drafts`。`EXPORT_INCLUDES_DRAFT` | `sample_kinds_are_as_named` |
| SAVE-24: Enter の送信は `submit` だけ | `ops` のタグは 6 種。Enter 専用のタグは無い | `submit_is_the_only_send_record`・`ops_use_only_the_six_tags` |
| D-54: 早送りは `ops` に入れない | 早送りのタグは無い。`OPS_RECORDS_FAST_FORWARD` は `false` | `submit_is_the_only_send_record` |

## 変更履歴

| サンプルの版 | 日付 | 内容 |
|---|---|---|
| 1 | 2026-10-15 | 新規（TASK-309 3/3）。空・2 周分・下書きと拒否の 3 本 |
