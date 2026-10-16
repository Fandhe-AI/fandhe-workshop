# 公開 API の契約の文書

```text
公開 API の契約の文書の版: 3
対象の契約の版（CONTRACT_VERSION）: 3
決定日: 2026-10-15
状態: 固定（契約）
```

## 1. 位置づけと版

- 本文書は、core crate（`fandhe_workshop_core`）の公開 API（`Session`・`apply()`・入力・イベント・Step の状態・`Projection`・宛先）の契約を、一か所で見られるようにした一覧と、机上の照合の記録である（TASK-204 3/3）。表示・保存・入力判定・案内の担当が、rustdoc を順に読まなくても全体をつかめるようにする
- 正は Rust の型と rustdoc である。本文書は一覧であり、欄や variant の詳細は再定義しない。食い違いを見つけたら、型を正として本文書を直す
- 「公開 API の契約の文書の版」は本文書の版（整数）である。型の版 `CONTRACT_VERSION`（`crates/core/src/lib.rs`）とは別に管理し、冒頭の囲みに両方を書く
- 型の欄・variant、関数の署名、世代の規則の表を変えたら、`CONTRACT_VERSION` を上げる（`lib.rs` の rustdoc と同じ規則）。そのときは本文書の版も上げ、変更履歴に記録する
- 変更が要るときは、仕様（`docs/spec` の 04）を先に直し、版上げの契約タスクを立てる
- 仕様は private のため、本文を転記せず ID（EVT-6、EVT-8 など）で参照する
- 状態変化の提供元の境界の版（`provider::PROVIDER_CONTRACT_VERSION`）は別に管理する。本文書の対象外である
- 冒頭の版と世代の表（3 節）は、`crates/core/tests/engine_api_doc.rs` が型と照合する

## 2. 型の一覧

学習目標の確認の記録の型と導出（`fandhe_workshop_contract::lo_check`、TASK-291）は contract の型で、`LO_CHECK_CONTRACT_VERSION` で別に版を管理する。本文書の型の一覧と `CONTRACT_VERSION` の対象外である。

キー判定表 `decide` の入出力の型（`fandhe_workshop_contract::key_decide`、TASK-257）も contract の型で、`KEY_DECIDE_CONTRACT_VERSION` で別に版を管理する。本文書の型の一覧と `CONTRACT_VERSION` の対象外である。

型は `fandhe_workshop_core::<モジュール>::<型>` で参照する。crate 直下には `Session`・`SessionState`・`apply`・`Projection`・`CONTRACT_VERSION` を再エクスポートしている。

| モジュール | 主な型・関数 | 役割 | 主な呼び出し元 |
|---|---|---|---|
| `addr` | `Generation`・`Root`・`StepUid`・`StepUidError`・`Stamp`・`TickAddr` | 世代、実体の根、Step の実体 ID、イベントの時点の印、進行の宛先 | web・pack・保存 |
| `input` | `Op`・`Input`・`Control`（`OpenHint`・`SetGuidance` を含む）・`Decision`・`CheckpointId`・`GenerationRule` | `apply()` が受ける操作と、世代の規則の表 | web（入力の受け口）・保存（履歴の `ops`） |
| `event` | `Event`（`HintOpened`・`Judged` を含む）・`IgnoreReason`・`RefuseReason`（`NoHint` を含む）・`ControlKind`・`Cause` | `apply()` が返すイベント列の 1 件 | 表示・保存・案内 |
| `judge` | `judge_input`・`judge_tick`・`judged_event`・`JudgeOutcome`・`JudgeDetail`・`JudgeError` | 入力・進行の判定結果の統合（6 種のどれか 1 つ）と、`Event::Judged` の材料。型は contract の `judgment` が正で、版は `JUDGMENT_CONTRACT_VERSION` が別に持つ。`judge` の関数は `CONTRACT_VERSION` の署名の対象に含まない（`Op::Input` は `apply()` に結線済み。`Op::Tick` は宛先が一致する進行を TASK-210 2/3 が結線済みで、食い違いの無視の記録は 3/3） | `apply()` の分岐（TASK-207・TASK-210）・教材テスト（pack） |
| `step_status` | `StepStatus` | Step の状態の 5 値 | 表示・保存 |
| `world` | `World`・`Element` | 世界状態（経路から要素への写像） | 表示・保存 |
| `session` | `Session`・`SessionState`（`hints_opened` を含む）・`RunState`（`guidance_level` を含む）・`Busy`・`Slice`・`EntryCopy`・`Checkpoint`・`Engine`・`apply` | 実行環境と遷移関数 | web・保存 |
| `projection` | `Projection`・`LogRow`・`Mark`・`MarkKind`・`ReviewItem`・`ReviewChange` | イベント列から導く表示用の見え方 | 表示 |

- 案内レベルとヒントの値の型（`GuidanceLevel`・`visible_hints`）は `fandhe_workshop_contract::guidance` にあり、`GUIDANCE_CONTRACT_VERSION` で別に管理する。本文書の対象外である
- `Session::new`・`Session::from_state`・`Session::apply` が実行環境の入口である。状態を明示して呼ぶ関数が `apply(ir, state, op)` である
- 利用者由来の文字列（`Input` の文字列、`Changed` の値、`Warning::message`）は信頼できない。HTML へ埋め込むときのエスケープは web 層の責務であり、core は値を保持するだけである

## 3. 世代の変わり方の表

世代は `Generation` で表し、初期値は `Generation::INITIAL`（1）である。減らない。`Generation::next` はあふれたとき `None` を返し、panic しない（呼び出し側は世代を変えない側に倒す）。

次の表は `Op::generation_rule()` の戻り値を、EVT-6 の (1)〜(6) と対応づけたものである。第 1 列は固定の記法で書いており、試験が読む。同じ行頭の記法の表を、ほかに置かない。

| 操作 | 規則 | EVT-6 | 世代が変わるときのイベント |
|---|---|---|---|
| `Op::Input(_)` | `Keep` | (1) | なし |
| `Op::Input(Input::Cancel)` | `Keep` | (1) | なし |
| `Op::Tick(_)` | `Keep` | (2) | なし |
| `Op::Control(Control::Stop)` | `IncrementIfProcessing` | (3) | `Stopped` の `new_generation` |
| `Op::Control(Control::GoTo(_))` | `IncrementOnSuccess` | (4) | `Moved` の `new_generation` |
| `Op::Control(Control::Save)` | `Keep` | (5) | なし（`CheckpointSaved` は `new_generation` を持たない） |
| `Op::Control(Control::Restore(_))` | `IncrementOnSuccess` | (6) | `Restored` の `new_generation` |

脚注の操作（EVT-6 の 6 項目の外）。

| 操作 | 規則 | EVT-6 | 世代が変わるときのイベント |
|---|---|---|---|
| `Op::Control(Control::Begin(_))` | `IncrementOnSuccess` | 範囲外（試作の設計） | `Begun` の `new_generation` |
| `Op::Control(Control::OpenHint)` | `Keep` | 範囲外（GUIDE-17） | なし（`HintOpened` は `new_generation` を持たない） |
| `Op::Control(Control::SetGuidance(_))` | `Keep` | 範囲外（GUIDE-3） | なし（イベントを出さない） |

- `Op::Input` は連打（再送）でも 1 件ごとの `Op::Input` であり、`Keep` のままである
- `Control::Stop` は処理中（`RunState.busy` が `Some`）のときだけ世代を上げる。処理中でない停止は `Input::Cancel` として扱い、世代を変えない（EVT-10）
- `IncrementOnSuccess` は、拒否（`Event::Refused`）や無視（`Event::Ignored`）では変えない
- 実際の増やし方の実装は TASK-210・TASK-211・TASK-213・TASK-214・TASK-217 が持つ
- 表は `session::apply` の関門が強制する（TASK-210 1/3）。`Keep` の操作は分岐が世代を動かしても戻し、そのほかも世代を減らさず増分は 1 までにする

## 4. 宛先の形

- `Generation`: `u64` の新しい型。直列化は透過（数値そのもの）
- `StepUid`: `main/<Step>`（章）、`<モジュール ID>@<実体名>/<Step>`、ネストした `<根>/<モジュール ID>@<実体名>/<Step>`。根と Step は最後の `/` で分ける。`Root` は `StepUid::root()` で取り出す
- `StepUid::parse` は外から来た値の検査口であり、拒否を `Result` で返して panic しない。拒否の種類は `Empty`・`TooLong`・`InvalidChar`・`SlashCount`・`EmptyPart`・`BadRoot` の 6 種である。`StepUidError` の文面は固定で、入力値を埋め込まない
- `Stamp { generation, step }`: すべてのイベントが `at` に持つ、その時点の世代と Step
- `TickAddr { generation, step, index }`: 進行（`Op::Tick`）の宛先。EVT-8 の 3 要素（世代・Step の実体 ID・何番目の進行か）に当たる。`index` は 0 始まりである
- 照合の規則: 3 要素がすべて現在の状態（`RunState.generation`・`RunState.current`・`RunState.busy` の `done`）と一致したときだけ進む。食い違いはコマンドの分類より先に判定し、状態を変えずに `Event::Ignored` を 1 件返す。`at` は操作前の世代と現在の Step（宛先の値ではない）で、現在の Step が無いときは記録しない（実装済み、TASK-210 3/3）

## 5. 版 3 で固定しないもの

次は意図して版 3 に含めない。足すときは版上げの契約タスクとする。

- 早送りの入・切（EVT-27）
- 実体ごとの案内レベル（`use.guidance`）とヒントの上限 3 段の定数。D-55 で語彙・エンジンに入れないと決めた。3 段は教材側の約束である
- 案内レベルの切り替えのイベント化と履歴での扱い（GUIDE-16）。TASK-307・TASK-308 が決める。版 2 の `Control::SetGuidance` はイベントを出さない
- ヒントの使用回数を増やす処理と、周回をまたぐ合計（GUIDE-7）。型の置き場（`SessionState.hints_opened`）だけを版 2 で固定した。振る舞いは TASK-295・TASK-297 が持つ
- `Op::Tick` での `Judged`（進行・無視）の発行位置。宛先が一致する進行は `Judged` を出さず `Progress` だけを出すと決めた（TASK-210 2/3、版 3 のサンプルの形に一致し、`CONTRACT_VERSION` を上げずに済む）。宛先の食い違いは `Judged{ignored}` を出さず `Event::Ignored` を 1 件だけ出すと決めた（TASK-210 3/3、サンプル e07 の形のまま、版は据え置き）。`judge::judge_tick` は純関数だけを置く
- `Event::Refused.rejection`（`Option<String>`）の型の置き換えと、問い返しの待ちの状態（EP-4 `clarifying`）。足すときは版上げにする
- コマンドの拒否の分類（`unknown_command`・`wrong_order`・`not_supported`・`not_in_this_step`）と処理中の `busy` は、TASK-268・TASK-269 で `command_class`・`judge` に確定した。型と署名は変えないため、版 3 のままである
- 下書きの操作と再読み込み（SAVE-4）。再読み込みで世代を上げる規則は、3 節の表に含まれない。`apply()` の `Op` には含めず、履歴の入力の列の型（`fandhe_workshop_contract::history::HistoryOp`、TASK-309）が持つ

## 6. 机上の照合の記録

```text
照合日: 2026-10-15
照合した main: 7b311a1
照合した spec（submodule の記録）: 159702f
```

型・rustdoc・規則の表を読んで、受け入れ条件（EVT-6 の (1)〜(6) と EVT-8 の 3 要素が型から読み取れること）を照合した。spec の本文は private のため、本記録は ID と要旨のみで書いている。実行して確かめたものではなく、読んだ結果である。

「結果」は次の 3 値に限る。

- `読み取れる`: 型・rustdoc・規則の表だけで要件が分かる
- `型は許す・実装待ち`: 型は要件を表せるが、振る舞いは担当タスクが実装する
- `所見あり`: 型からは読み取れない、またはあいまい。対処は本件の範囲外である

| 項目 | 型から読み取れる場所 | 結果 | 補足 |
|---|---|---|---|
| EVT-6 (1) 学習者の入力は世代を変えない | `Op::Input(_)` が `Keep`（`input.rs`）。`Input::Cancel` も同じ | 読み取れる | 連打も 1 件ごとの `Op::Input` で、`Keep` のままである |
| EVT-6 (2) 進行は世代を変えない | `Op::Tick(_)` が `Keep` | 読み取れる | |
| EVT-6 (3) 処理中の停止は世代を 1 つ上げる | `Control::Stop` が `IncrementIfProcessing`。処理中は `RunState.busy`。`Event::Stopped.new_generation` | 読み取れる | 処理中でない停止は `Input::Cancel` 扱い（`Control::Stop` の rustdoc、EVT-10） |
| EVT-6 (4) 移動は成功時だけ世代を上げる | `Control::GoTo` が `IncrementOnSuccess`。`Event::Moved.new_generation`。失敗は `Event::Refused` の `op: ControlKind::GoTo` | 読み取れる | |
| EVT-6 (5) 保存は世代を変えない | `Control::Save` が `Keep`。`Event::CheckpointSaved` は `new_generation` を持たない | 読み取れる | |
| EVT-6 (6) 復帰は成功時だけ世代を上げ、復帰先は写しで世代だけ新しい | `Control::Restore` が `IncrementOnSuccess`。`Checkpoint.run`（保存時の写し）と `Event::Restored.new_generation` | 世代の規則は読み取れる。写しを戻して世代だけ新しくする処理は型は許す・実装待ち | 後者は TASK-214 が実装する |
| EVT-6 の外: 周回の開始 | `Control::Begin` が `IncrementOnSuccess`。`Event::Begun.new_generation` | 読み取れる | 6 項目に数えない（試作の設計）。3 節の脚注の表で分けた |
| EVT-8 の 3 要素 | `TickAddr { generation, step, index }`（`addr.rs`）。現在の側は `RunState.generation`・`RunState.current`・`Busy.done` | 読み取れる | |
| EVT-8 (a) 一致したら進行が 1 つ進む | `Event::Progress { index, total }`・`Busy.done` | 実装済み（`tick` モジュール、TASK-210 2/3）。同じ操作の列から同じ状態とイベント列が再現される（EVT-16、`tick_progress` の `p16_replay_is_deterministic`） | |
| EVT-8 (b) 食い違いは状態を変えず「無視」を 1 件 | `Event::Ignored { at, reason }`・`IgnoreReason` | 実装済み（`tick` モジュール、TASK-210 3/3）。`tick_progress` の `mismatched_tick_records_one_ignored` ほか | |
| EVT-8 (b) の場合ごとの理由の割り当て | 古い世代・移動前の Step は `IgnoreReason::Stale` | 決定済み | 同じ番号の二重の届きは `Stale`（世代・Step・番号のいずれかが古い）、処理中でなければ `NoProcessing` とした（TASK-210 3/3）。専用の理由の追加は版上げの契約タスク |
| EVT-8 の判定順（宛先の照合を先に） | `TickAddr` と `IgnoreReason` の rustdoc | 実装済み（`judge_tick` が先に照合、TASK-210 3/3） | |
| GUIDE-17 現在の案内レベルで出せる n 段まで 1 段ずつ開ける | `Control::OpenHint`、`Event::HintOpened { opened, total }`、`SessionState.hints_opened`、`guidance::visible_hints` の `len()` | 型は許す・実装待ち | TASK-295・TASK-297 |
| GUIDE-17 n 段を超える操作と現在の Step が無い操作は `NoHint` で拒否 | `RefuseReason::NoHint`、`ControlKind::OpenHint` | 型は許す・実装待ち | TASK-295 |
| GUIDE-17 ヒントを開いても世代は変わらない | `OpenHint` が `Keep`、`HintOpened` が `new_generation` を持たない | 読み取れる | |
| JUDGE-3 入力 1 件の判定結果は 6 種のどれか 1 つで、`Judged` として出る | `Event::Judged { at, judgment }`・contract の `Judgment::kind()`（全域）・`judge::judge_input` | 読み取れる（型と純関数）。`Op::Input` は `apply()` に結線済み | 発行位置は `InputReceived` の直後（`Op::Input`）。`apply()` は判定を 1 回だけ行い、その結果に従って状態を変える（拒否・問い返しは状態を変えない）。判定できない宣言と、受付・確認の判定の後に遷移が続かない場合は `Judged` を置かず `Warning` を出す。取り消しの受付の規則（`on.cancel`）は、受付の処理を TASK-212 が結線するまで同じく `Warning` だけにする。進行の位置は TASK-210 |
| GUIDE-3 案内レベルは実行全体で 1 つ | `RunState.guidance_level`（`Slice` に置かない）、`Control::SetGuidance` | 読み取れる | 切り替えの処理は TASK-294 が実装する |

## 7. 契約適合試験とサンプル

表示・保存・入力判定・案内の担当が `apply()` へ結線するときの合否の基準である（TASK-205）。次のものから成る。

| 物 | 置き場 | 役割 |
|---|---|---|
| サンプル | `testdata/contract/event-sequence.json`（サンプルの版 3、対象は `CONTRACT_VERSION` 3） | 21 件の操作の前後の状態と期待のイベント列 |
| サンプルの説明 | `testdata/contract/README.md` | 欄の意味・版の規則 |
| 形の試験 | `crates/core/tests/contract_sample.rs` | サンプルが core の型で読めること、連なり、網羅 |
| 適合試験 | `crates/core/tests/contract_conformance.rs` | 下表の 3 層 |
| 読む部品 | `crates/core/tests/common/contract_sample.rs` | サンプルを core の型で読む |

適合試験は 3 層に分ける。`apply()` が骨格の間も意味のある層と、結線の合否を決める厳密な層を分け、厳密な層の判定を弱めて通すことはしない。

| 層 | 確かめること | 現在 | CI |
|---|---|---|---|
| サンプルと規則の表 | サンプルの世代の差が 3 節の表と EVT-6 の (1)〜(6) に合う | 通る | 載る |
| 実際の `apply()` の不変条件 | EVT-22（同じ入力列で状態とイベント列が一致）、3 つの入口の一致、世代が減らない・増分は 1 まで | 通る | 載る |
| 厳密な照合 | 連鎖して流し、各操作の状態とイベント列がサンプルと一致する（`sample_replay_matches_apply`） | 先頭 5 件（e01〜e05、入力の受付と一致する進行）は通る（`sample_replay_prefix_matches_apply`、CI に載る）。全件は最初の食い違いが e06（停止、TASK-211）で `#[ignore]`。連鎖で届かない e09〜e13（一致する進行の連鎖、TASK-210 2/3）は前の状態から `sample_ticks_entry_matches_apply` で、e15（確認による達成、TASK-207 2/3）は前の状態から単独で照合する（`sample_confirm_entry_matches_apply`、CI に載る）。e16（確認を省く明示の Step の受付と同時の達成、TASK-208）も単独で照合する（`sample_skip_confirm_entry_matches_apply`、CI に載る）。e07（古い世代の進行の無視、TASK-210 3/3）も単独で照合する（`sample_stale_tick_entry_matches_apply`、CI に載る） | 先頭 5 件と e07・e09〜e13・e15・e16 の単独照合が載る |
| 比較器の自己確認 | 期待どおりの提供元は合格、1 か所壊した提供元は最初の食い違いの操作 ID で失敗する | 通る | 載る |

実行コマンド:

- 通常: `make test`（CI の `rust-test` が流す）
- 結線の確認: `cargo test --locked -p fandhe-workshop-core --test contract_conformance -- --include-ignored`

統合するタスクの完了条件には、「`contract_conformance` が `--include-ignored` で通る」と書く。`apply()` の最後の分岐
（TASK-207・210・211・213・214・215）を埋めるタスクが、`sample_replay_matches_apply` の `#[ignore]` を外す。
サンプルの版の規則（契約を変えたら版を上げる）は `testdata/contract/README.md` の「版の規則」に従う。

## 8. 変更履歴

| 版 | 日付 | 変更 | 理由 |
|---|---|---|---|
| 1 | 2026-10-15 | 初版。型の一覧・世代の表・宛先の形・机上の照合の記録 | TASK-204 3/3 |
| 2 | 2026-10-15 | `Control::OpenHint`・`Control::SetGuidance`、`Event::HintOpened`、`RefuseReason::NoHint`、`ControlKind::OpenHint`、`RunState.guidance_level`、`SessionState.hints_opened` を追加。世代の表に 2 行、照合の記録に GUIDE-17・GUIDE-3 を追加。`CONTRACT_VERSION` を 1 から 2 へ | TASK-290 |
| 3 | 2026-10-15 | `Event::Judged` を追加。`judge` モジュール（判定結果の統合）を一覧に追加。5 節に `Tick` の発行位置などの未決を、6 節に JUDGE-3 の照合を追加。`CONTRACT_VERSION` を 2 から 3 へ | TASK-267 |

- 2026-10-15: 7 節（契約適合試験とサンプル）を追加した。型と世代の表は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-205 2/2。版 2 への引き上げは上の表のとおり TASK-290）
- 2026-10-15: `Projection` の導出の中身（ログ行・変化の印・復習・復帰の復習）を実装した。公開の型・署名・直列化の形は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-215 2/2）
- 2026-10-15: e16 の単独照合を追加した。型・直列化の形は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-208）
- 2026-10-16: 食い違う進行を `Ignored` 1 件で記録するようにした（`tick` モジュール）。型・表・署名は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-210 3/3）
- 2026-10-16: 宛先が一致する進行 `Op::Tick` を `apply()` に結線した（`tick` モジュール）。型・表・署名は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-210 2/3）
- 2026-10-16: 世代の表を `session::apply` の関門で強制した。型・表・署名は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-210 1/3）
