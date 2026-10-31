# 公開 API の契約の文書

```text
公開 API の契約の文書の版: 7
対象の契約の版（CONTRACT_VERSION）: 7
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
- 受け口の有効化の関門の版（`receptor_gate::RECEPTOR_GATE_CONTRACT_VERSION`）も別に管理する。本文書の対象外である
- 冒頭の版と世代の表（3 節）は、`crates/core/tests/engine_api_doc.rs` が型と照合する

## 2. 型の一覧

学習目標の確認の記録の型と導出（`fandhe_workshop_contract::lo_check`、TASK-291）は contract の型で、`LO_CHECK_CONTRACT_VERSION` で別に版を管理する。本文書の型の一覧と `CONTRACT_VERSION` の対象外である。

キー判定表 `decide` の入出力の型（`fandhe_workshop_contract::key_decide`、TASK-257 版 1・TASK-281 版 2）も contract の型で、`KEY_DECIDE_CONTRACT_VERSION`（現在 2）で別に版を管理する。本文書の型の一覧と `CONTRACT_VERSION` の対象外である。

型は `fandhe_workshop_core::<モジュール>::<型>` で参照する。crate 直下には `Session`・`SessionState`・`apply`・`Projection`・`CONTRACT_VERSION` を再エクスポートしている。

| モジュール | 主な型・関数 | 役割 | 主な呼び出し元 |
|---|---|---|---|
| `addr` | `Generation`・`Root`・`StepUid`・`StepUidError`・`Stamp`・`TickAddr` | 世代、実体の根、Step の実体 ID、イベントの時点の印、進行の宛先 | web・pack・保存 |
| `input` | `Op`・`Input`・`Control`（`OpenHint`・`SetGuidance` を含む）・`Decision`・`CheckpointId`・`GenerationRule` | `apply()` が受ける操作と、世代の規則の表 | web（入力の受け口）・保存（履歴の `ops`） |
| `event` | `Event`（`HintOpened`・`Judged`・`Reloaded`・`TransitionLimitReached` を含む）・`IgnoreReason`・`RefuseReason`（`NoHint`・`ClosedRound` を含む）・`RefuseNextAction`・`ControlKind`・`Cause` | `apply()` が返すイベント列の 1 件 | 表示・保存・案内 |
| `judge` | `judge_input`・`judge_tick`・`judged_event`・`JudgeOutcome`・`JudgeDetail`・`JudgeError` | 入力・進行の判定結果の統合（6 種のどれか 1 つ）と、`Event::Judged` の材料。型は contract の `judgment` が正で、版は `JUDGMENT_CONTRACT_VERSION` が別に持つ。`judge` の関数は `CONTRACT_VERSION` の署名の対象に含まない（`Op::Input` は `apply()` に結線済み。`Op::Tick` は宛先が一致する進行を TASK-210 2/3 が結線済みで、食い違いの無視の記録は 3/3） | `apply()` の分岐（TASK-207・TASK-210）・教材テスト（pack） |
| `text_fill` | `step_do_at`・`slot_values`・`fill_next_action`・`fill_message`・`filled_text`・`filled_texts_of_log`・`FilledText`・`nearest_command`・`osa_distance`・`NEAREST_MAX_DISTANCE`・`TextFillError` | 拒否・問い返し・注記の文面への実行時の値の差し込み（`filled_texts_of_log` はログから拒否・問い返しの文面を作り直す）。`Judged` の次の行動は差し込みの後の値で、型と署名は変えない。`step_do_at` は案内レベルごとの `do`（`do_levels`。C → B → A）を選ぶ純関数で、`slot_values` の `step_do` と pack の漏れの検査が同じ選択を読む（TASK-70 2/2）。`CONTRACT_VERSION` の署名の対象に含まない | `judge`・教材テスト（pack）・画面（web） |
| `instance` | `root_of`・`owner_of`・`instance_roots`・`slice_of`・`status_of(ir, run, uid)`・`steps_of`・`instance_graph`（`InstanceGraph`・`InstanceGraphError`） | 学習モジュールの実体（根）ごとの状態の読み出しと、状態を書き読みする根の決め方の一本化（MOD-1、TASK-233 1/2）、実体の呼び出しの静的な有向非巡回グラフの確認と根ごとの数（TASK-233 2/2。`load_ir` には結線しない）。`CONTRACT_VERSION` の署名の対象に含まない | `Session`・pack と web の試験 |
| `body_level` | `select_body`・`body_doc_at`・`SelectedBody` | 案内レベル別の本文の木（`Node::body_doc`・`body_levels_doc`）の選択。契約の `GuidanceLevel::fallback_chain` の順（C → B → A）で探し、水準の版が無ければ濃い側へ倒れる。`body_levels_doc` の鍵 `A` と `A`・`B`・`C` 以外の鍵は読まず、木を持たない版 1 の Node は `None`。`CONTRACT_VERSION` の署名の対象に含まない（TASK-292） | 画面の本文の区画（web。結線は SHELL-7 のタスク）・教材テスト（pack） |
| `hint_select` | `hints_at`・`current_hints` | 現在の Step と案内レベルで出せるヒントの ID の一覧（規則は契約の `visible_hints`。GUIDE-2、TASK-293）。`CONTRACT_VERSION` の署名の対象に含まない | ヒントを開く判定（TASK-295）・画面（TASK-530）・教材テスト（pack） |
| `lo_observe` | `answer_phase`・`observe_op`・`observe_history`・`records_from_history`・`AnswerPhase` | 履歴から確認ごとの最初の回答の正誤と開いたヒントの段を `CheckObservation` の列として取り出す（TASK-304 1/3、GUIDE-11）。適用し直しは `history_replay::replay_with`（`replay` の前後に hook を足した口）を通す。前後の比較の観測は作らない。`CONTRACT_VERSION` の署名の対象に含まない | 教材テスト（pack）・講師資料・復習・体験会の集計 |
| `lo_self_reliant` | `SelfRelianceRule`・`CheckKind`・`classify`・`tally`・`count_participants`・`SelfRelianceOutcome`・`SelfRelianceTally`・`ParticipantCount` | 選び直しの手順を持つ選択肢の確認の自力の数え方（TASK-304 2/3、GUIDE-11、TASK-3 の決定 #484）。1・2 段目のヒントの後の正答と選び直しの正答は自力、答えの段（呼び出し側が渡す。0 は拒否）以上を開いた後の正答は自力に数えない。束ねは「すべて自力」の参加者の数で、割合は返さない。`CONTRACT_VERSION` の署名の対象に含まない | 教材テスト（pack）・講師資料・復習・体験会の集計 |
| `step_status` | `StepStatus` | Step の状態の 5 値 | 表示・保存 |
| `world` | `World`・`Element` | 世界状態（経路から要素への写像） | 表示・保存 |
| `session` | `Session`・`SessionState`（`hints_opened` を含む）・`RunState`（`guidance_level` を含む）・`Busy`・`Slice`（`clarifying` を含む）・`EntryCopy`・`Checkpoint`・`Engine`・`apply`・`reload`（`Session::instances`・`slice`・`step_status`・`steps_of`・`current_draft`、`SessionState::draft_root`・`current_draft` の読み取り用メソッドを含む。`RecordedSession::current_draft` も同じ） | 実行環境と遷移関数。再読み込み（`session::reload`・`Session::reload`、SAVE-4）は `apply()` の外にある | web・保存 |
| `projection` | `Projection`・`LogRow`・`Mark`・`MarkKind`・`ReviewItem`・`ReviewChange` | イベント列から導く表示用の見え方 | 表示 |

- 案内レベルとヒントの値の型（`GuidanceLevel`・`visible_hints`）は `fandhe_workshop_contract::guidance` にあり、`GUIDANCE_CONTRACT_VERSION` で別に管理する。本文書の対象外である
- `Session::new`・`Session::from_state`・`Session::apply` が実行環境の入口である（開始点に入った直後の状態で作る `Session::entered_at` は、web の結線（TASK-254）までの暫定の入口。TASK-112 2/2）。状態を明示して呼ぶ関数が `apply(ir, state, op)` である
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
- 実際の増やし方の実装は TASK-210・TASK-211・TASK-213・TASK-214（実装済み）・TASK-217 が持つ
- 再読み込み（`session::reload`、SAVE-4）は `Op` ではないため、表に含めない。現在の Step があれば処理中かどうかに関わらず世代を +1 し、`Reloaded` の `new_generation` に載せる。世代のあふれと現在の Step が無い場合は世代を変えない（TASK-320 1/2）
- 表は `session::apply` の関門が強制する（TASK-210 1/3）。`Keep` の操作は分岐が世代を動かしても戻し、そのほかも世代を減らさず増分は 1 までにする

## 4. 宛先の形

- `Generation`: `u64` の新しい型。直列化は透過（数値そのもの）
- `StepUid`: `main/<Step>`（章）、`<モジュール ID>@<実体名>/<Step>`、ネストした `<根>/<モジュール ID>@<実体名>/<Step>`。根と Step は最後の `/` で分ける。`Root` は `StepUid::root()` で取り出す
- `StepUid::parse` は外から来た値の検査口であり、拒否を `Result` で返して panic しない。拒否の種類は `Empty`・`TooLong`・`InvalidChar`・`SlashCount`・`EmptyPart`・`BadRoot` の 6 種である。`StepUidError` の文面は固定で、入力値を埋め込まない
- `Stamp { generation, step }`: すべてのイベントが `at` に持つ、その時点の世代と Step
- `TickAddr { generation, step, index }`: 進行（`Op::Tick`）の宛先。EVT-8 の 3 要素（世代・Step の実体 ID・何番目の進行か）に当たる。`index` は 0 始まりである
- 照合の規則: 3 要素がすべて現在の状態（`RunState.generation`・`RunState.current`・`RunState.busy` の `done`）と一致したときだけ進む。食い違いはコマンドの分類より先に判定し、状態を変えずに `Event::Ignored` を 1 件返す。`at` は操作前の世代と現在の Step（宛先の値ではない）で、現在の Step が無いときは記録しない（実装済み、TASK-210 3/3）

## 5. 版 7 で固定しないもの

次は意図して版 7 に含めない。足すときは版上げの契約タスクとする。

- 早送りの入・切（EVT-27）
- 実体ごとの案内レベル（`use.guidance`）とヒントの上限 3 段の定数。D-55 で語彙・エンジンに入れないと決めた。3 段は教材側の約束である
- 案内レベルの切り替えのイベント化と履歴での扱い（GUIDE-16）。TASK-307・TASK-308 が決める。版 2 の `Control::SetGuidance` はイベントを出さない
- ヒントの使用回数が停止・復帰・移動で減らないことと送信 0 件は、型や API を足さず試験で固定した（GUIDE-7。TASK-297 2/2。`hint_count_monotone`・`round_hint_counts`・`egress-count` の G3）。型の置き場（`SessionState.hints_opened`）は版 2 で固定し、鍵（Step の実体 ID）と合計の読み取り口（`hints_opened_at`・`hints_opened_in`）は TASK-297 1/2 で足した
- `Op::Tick` での `Judged`（進行・無視）の発行位置。宛先が一致する進行は `Judged` を出さず `Progress` だけを出すと決めた（TASK-210 2/3、版 3 のサンプルの形に一致し、`CONTRACT_VERSION` を上げずに済む）。宛先の食い違いは `Judged{ignored}` を出さず `Event::Ignored` を 1 件だけ出すと決めた（TASK-210 3/3、サンプル e07 の形のまま、版は据え置き）。`judge::judge_tick` は純関数だけを置く
- `Event::Refused.rejection`（`Option<String>`）の型の置き換え。足すときは版上げにする
- 問い返しの待ちの状態（EP-4 `clarifying`）は、欄 `Slice.clarifying` を版 5 で固定した（TASK-42 2/2）。立てる・消す処理と評価器の結線は TASK-207・TASK-267 が持ち、それまで本番の値は常に `None` である
- コマンドの拒否の分類（`unknown_command`・`wrong_order`・`not_supported`・`not_in_this_step`）と処理中の `busy` は、TASK-268・TASK-269 で `command_class`・`judge` に確定した。型と署名は変えないため、版 3 のままである
- 下書きの操作。再読み込み（SAVE-4）は版 4 で `Event::Reloaded`・`session::reload`・`Session::reload` として固定した（TASK-320 1/2）。`apply()` の `Op` には含めず、履歴の入力の列の型（`fandhe_workshop_contract::history::HistoryOp`、TASK-309）の `Reload` が持つ

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
| EVT-6 (3) 処理中の停止は世代を 1 つ上げる | `Control::Stop` が `IncrementIfProcessing`。処理中は `RunState.busy`。`Event::Stopped.new_generation` | 実装済み（`stop` モジュール、TASK-211 1/2）。`stop_restore` の試験 | 処理中でない停止は `Input::Cancel` 扱い（`Control::Stop` の rustdoc、EVT-10）。`session::step` が `accept` へ渡し、世代は変えない（実装済み、TASK-212。`cancel_input` の試験） |
| EVT-6 (4) 移動は成功時だけ世代を上げる | `Control::GoTo` が `IncrementOnSuccess`。`Event::Moved.new_generation`。失敗は `Event::Refused` の `op: ControlKind::GoTo` | 実装済み（`go_to` モジュール、TASK-213 1/2） | |
| EVT-11 別の Step への移動は世代を +1 し、処理中を解き、移動先に入る | `Control::GoTo(StepUid)`・`Event::Moved`・`Event::Entered`・`SessionState.entry` | 実装済み（`go_to` モジュール、TASK-213 1/2）。`move_step` の `move_while_busy_increments_generation_and_clears_busy` ほか。移動先を `Entered` にし（達成済みでも戻す）、状態の入口の写しは `None` に無効化する（移動の後の受付から処理中になって停止すると、移動先の受付の時点へ戻る。入口の写しは移動の前の受付からは導かない。`entry_derive`）。無い Step は `Refused(UnknownStep)`、世代のあふれ・現在の Step 無しは状態を変えない。移動前の世代の進行は移動の後に届いても `Ignored(Stale)` が 1 件だけで状態を変えない（TASK-213 2/2。`reserved_ticks_after_move_are_ignored`・`stale_tick_after_reenter_and_reinput_is_ignored`・`p2_pre_move_ticks_never_change_run` ほか） | 閉じた周回の Step への移動は `Refused(ClosedRound)`（MOD-15、TASK-249）。移動先の `on_enter` の効果は TASK-217（入口の写しは着地で無効化し、ログから導く。TASK-242 1/2） |
| EVT-6 (5) 保存は世代を変えない | `Control::Save` が `Keep`。`Event::CheckpointSaved` は `new_generation` を持たない | 実装済み（`checkpoint` モジュール、TASK-214 1/3） 2026-10-21: 保存が成功し、現在の Step が `env_action`（`action: save`）を宣言していれば、続けて受付（確認を省く宣言なら達成まで）にする（`env_input`、TASK-64 1/2）。世代の表・`Op`・イベントの型・`CONTRACT_VERSION` は変えない |
| EVT-6 (6) 復帰は成功時だけ世代を上げ、復帰先は写しで世代だけ新しい | `Control::Restore` が `IncrementOnSuccess`。`Checkpoint.run`（保存時の写し）と `Event::Restored.new_generation` | 実装済み（`checkpoint` モジュール、TASK-214 1/3）。`checkpoint_restore` の `restore_after_saving_at_three_ticks_returns_saved_run`・`p4_restore_returns_saved_run`・`p4_failed_restore_keeps_generation` ほか | 実体ごとの戻り方は TASK-241 1/2・2/2 で実装済み（`restore_owner_slice`・`restore_call_step_owner`・`round_slice_init` の `mod9_*`。5.261・5.262） |
| EVT-12 チェックポイントの保存と復帰 | `Control::Save`・`Control::Restore(CheckpointId)`・`Event::CheckpointSaved`・`Event::Restored`・`SessionState.checkpoints` | 実装済み（`checkpoint` モジュール、TASK-214 1/3）。保存は実行状態と状態の入口の写しの欄を控え（処理中でも可。欄は通常の操作では空で、処理中の保存へ復帰した後の受付の時点は、`Restored` から保存の時点へ起点を移してログから導く）、`log_len` は保存操作の直前のログの長さ。復帰は持ち主の根の切り身と位置（`current`・`busy`・`completed`）を写しへ戻して他の根の切り身は残し、世代は現在の +1 にし（世界状態は保存の後に持ち主が書いた経路だけを写しへ戻し、`global_flags` は戻さず、案内レベルは写しの値。持ち主は呼び出しの Step なら呼ばれた実体の根。写しのログ位置がログの長さを超えるときは `Warning` だけ）、`busy` と入口の写しも保存時点へ戻す。`Restored.log_len` は写しの値（EVT-14）。無い ID は `Refused(UnknownCheckpoint)`、世代・ID のあふれと現在の Step の無い写しは `Warning` だけで状態を変えない | 実体ごとの戻り方は TASK-241 1/2・2/2 で実装済み（`restore_owner_slice`・`restore_call_step_owner`・`round_slice_init` の `mod9_*`。5.261・5.262）。復帰前の進行の無視と処理中の再開は `restore_resume` が確かめる（2026-10-16、TASK-214 3/3。`pre_restore_tick_with_saved_index_is_ignored`・`restore_resumes_progress_in_new_generation`）。`Restored` による復習・変化の印の作り直し（EVT-14）は `apply()` を通して `restore_review` が確かめる（2026-10-16、TASK-214 2/3。`Event::Restored.log_len` までのイベントから作った `Projection::review` との一致） |
| EVT-6 の外: 周回の開始 | `Control::Begin` が `IncrementOnSuccess`。`Event::Begun.new_generation` | 読み取れる | 6 項目に数えない（試作の設計）。3 節の脚注の表で分けた |
| EVT-8 の 3 要素 | `TickAddr { generation, step, index }`（`addr.rs`）。現在の側は `RunState.generation`・`RunState.current`・`Busy.done` | 読み取れる | |
| EVT-8 (a) 一致したら進行が 1 つ進む | `Event::Progress { index, total }`・`Busy.done` | 実装済み（`tick` モジュール、TASK-210 2/3）。同じ操作の列から同じ状態とイベント列が再現される（EVT-16、`tick_progress` の `p16_replay_is_deterministic`） | |
| EVT-8 (b) 食い違いは状態を変えず「無視」を 1 件 | `Event::Ignored { at, reason }`・`IgnoreReason` | 実装済み（`tick` モジュール、TASK-210 3/3）。`tick_progress` の `mismatched_tick_records_one_ignored` ほか | |
| EVT-8 (b) の場合ごとの理由の割り当て | 古い世代・移動前の Step は `IgnoreReason::Stale` | 決定済み | 同じ番号の二重の届きは `Stale`（世代・Step・番号のいずれかが古い）、処理中でなければ `NoProcessing` とした（TASK-210 3/3）。専用の理由の追加は版上げの契約タスク |
| EVT-8 の判定順（宛先の照合を先に） | `TickAddr` と `IgnoreReason` の rustdoc | 実装済み（`judge_tick` が先に照合、TASK-210 3/3） | |
| EVT-9 処理中の停止は世代を +1 し、処理中を解き、入口の写しへ戻す（持ち主の根の切り身と、持ち主が写しの後に書いた経路だけ） | `Event::Stopped { at, new_generation }`・`SessionState.entry`（`EntryCopy`） | 実装済み（`stop` モジュール、TASK-211 1/2）。`stop_restore` の `stop_after_two_ticks_removes_draft` ほか。写しは、状態に現在の Step の写しがあればそれを、無ければログから導いた受付の時点の `run`（`entry_derive`。状態には書かない。フラグ・選んだ選択肢は戻さない）を使う。受付の時点を一意に決められない・世代があふれる場合は `Warning` だけで状態を変えない（`stop_without_stored_entry_derives_the_same_entry_from_log`・`stop_without_entry_or_accept_in_log_warns_only`） | 停止前の世代の進行は停止の後に届いても `Ignored(Stale)` だけで状態を変えず、停止の後は入力を受け付け直せる（`stale_tick_after_stop_is_ignored`・`stale_tick_after_reinput_is_ignored`・`input_after_stop_is_accepted_and_completes`・`p2_pre_stop_ticks_never_change_run`、TASK-211 2/2） | 実体ごとの戻り方は実装済み（`stop_owner_slice`、TASK-242 2/2。入口の写しは Step に入るたびに置き換わり 1 つだけ。TASK-242 1/2） |
| GUIDE-6 ヒントの上限は現在の案内レベルで出せる数まで。3 段に固定しない（D-55） | `guidance::visible_hints` の `len()`、`hint_open` の上限 | 実装済み（`hint_open`、TASK-295・TASK-296。試験 `hint_limit_rule`・`hint_limit_39`） | |
| GUIDE-17 現在の案内レベルで出せる n 段まで 1 段ずつ開ける | `Control::OpenHint`、`Event::HintOpened { opened, total }`、`SessionState.hints_opened`、`guidance::visible_hints` の `len()` | 実装済み（`hint_open`、TASK-295。試験 `hint_open`）。鍵は Step の実体 ID（根を含む完全な uid）で周回・実体ごとに分かれ、根ごとの合計は `SessionState::hints_opened_in` で読む（TASK-297 1/2。試験 `hint_instance_key`・pack の `round_hints`）。減らないこと（試験 `hint_count_monotone`・pack の `round_hint_counts`）と送信 0 件（`egress-count` の G3）は実装済み（TASK-297 2/2） | TASK-297 |
| GUIDE-17 n 段を超える操作と現在の Step が無い操作は `NoHint` で拒否 | `RefuseReason::NoHint`、`ControlKind::OpenHint` | 実装済み（`hint_open`、TASK-295。試験 `hint_open`） | |
| GUIDE-17 ヒントを開いても世代は変わらない | `OpenHint` が `Keep`、`HintOpened` が `new_generation` を持たない | 読み取れる | |
| JUDGE-3 入力 1 件の判定結果は 6 種のどれか 1 つで、`Judged` として出る | `Event::Judged { at, judgment }`・contract の `Judgment::kind()`（全域）・`judge::judge_input` | 読み取れる（型と純関数）。`Op::Input` は `apply()` に結線済み | 発行位置は `InputReceived` の直後（`Op::Input`）。`apply()` は判定を 1 回だけ行い、その結果に従って状態を変える（拒否・問い返しは状態を変えない）。判定できない宣言と、受付・確認の判定の後に遷移が続かない場合は `Judged` を置かず `Warning` を出す。取り消しの受付の規則（`on.cancel`）は `Entered` の Step で他の入力と同じ受付になり（TASK-212）、`Accepted` の Step では意味が仕様に無いため `Judged` を置かず `Warning` だけにする。進行の位置は TASK-210 |
| JUDGE-5 拒否された入力は Step を受付済みにせず、世界状態も変えない | 公開の型・署名は増えない。`session::apply` の内部の関門 `reject_guard`（公開しない） | 内部の規則で、読み取る API は無い | 拒否の分岐は `state.run` を書かずに返る。破る実装があれば、関門が `run`・`entry` を操作前へ戻し、イベントを `InputReceived`・`Judged` と固定文の `Warning` 1 件に絞る（黙って直さない）。試験は `reject_invariant`（1,000 通り・拒否の 7 分類）。契約の版は変えない |
| SAVE-4 処理中の再読み込みは処理中のまま続け、世代を +1 する | `Event::Reloaded { at, new_generation }`・`session::reload`・`Session::reload`・`HistoryOp::Reload` | 実装済み（`reload` モジュール、TASK-320 1/2）。`reload_busy` の `reload_while_busy_keeps_busy_and_bumps_generation`・`two_reloads_add_two_generations` ほか、`history_verify` の `property_reload_at_any_point_*`。入口の写しは作り直さない。再読み込みの前の宛先の進行は `Ignored(Stale)` が 1 件だけで状態を変えず、再読み込みの後の停止は入口へ戻る（TASK-320 2/2。`reload_busy` の `stale_tick_after_reload_is_ignored_once`・`stale_ticks_from_before_two_reloads_are_each_ignored`・`stop_after_reload_returns_to_step_entry`、`history_verify` の H-6） | 右のログの行（SAVE-6）は `Projection` が `Reloaded` 1 件につき 1 行にする（TASK-322）。文言は web の `log_lines` が持つ |
| SAVE-5 停止・移動・復帰の直後の再読み込みは直後の状態と等しく、操作の前の宛先は `Ignored(Stale)` | `Event::Stopped`・`Event::Moved`・`Event::Restored`、`session::reload`、`HistoryOp::Reload` | 試験で固定（TASK-321）。`reload_after_control` の `reload_right_after_stop_restores_the_stopped_state`・`reload_right_after_move_restores_the_moved_state`・`reload_right_after_restore_restores_the_restored_state`・`reload_right_after_confirm_burst_keeps_one_achievement`（連打 S-9）、web の `reload_after_control_store`。製品コードは変えていない | 自動ブラウザ（S-6・S-7・S-9・S-19）は web に履歴の記録・復元の結線が入る TASK-319 の後 |
| GUIDE-3 案内レベルは実行全体で 1 つ | `RunState.guidance_level`（`Slice` に置かない）、`Control::SetGuidance` | 読み取れる | 切り替えの処理は TASK-294 が実装する |
| GUIDE-1 水準に合う本文が選ばれ、その水準の版が無ければ濃い側に倒れる | `body_level::select_body`・`body_doc_at` | 実装済み（TASK-292）。`body_level_select` の 12 通りの表・水準の定義に合う目印の語・鍵 `A` と未知の鍵の無視で固定。pack の `body_level_runtime` が静的検査の `body_for` との一致を確かめる | 画面への結線は SHELL-7 のタスク |
| MOD-4 下書きの根は「最初の `/` まで」 | `Root`・`StepUid::root`（`addr.rs`） | 所見あり | 入れ子の実体でだけ、文言（最初の `/`）と実装（最後の `/` より前＝`Instance.id`）が食い違う。実装は語彙 v1 の 9 節（フラグは実体ごと）と合うため変えず、試験で固定した（`docs/testing.md` の 5.81）。仕様側の決定は未了（#1413）。決定が「最初の `/`」なら切り身・フラグ・チェックポイントの持ち主の鍵にも効くため、`CONTRACT_VERSION` を上げる別の変更になる。切り身（MOD-3・MOD-9・MOD-10）の根も同じ定義から来ており、同じ食い違いがある |
| SAFE-34 上限で止めた理由は専用のイベントで通知する | `Event::TransitionLimitReached { at, landings }` | 実装済み（`transit` モジュール、TASK-160 2/3）。`transition_limit` の `assert_limit_event` が、巡回 3 本と連鎖の境界で、止めたときにちょうど 1 件で `Warning` の直後・末尾にあり `landings` が上限と一致することを、止まらないときは 0 件であることを確かめる。`Projection` は 1 件につき 1 行にする | 見出し・案内・まとめ表示は web が持つ（`log_lines`・`limit_notice`）。止めた後は `session::apply` が操作の前の Step・状態へ戻す（SAFE-35、TASK-160 3/3。新しいイベント・型は足さず版は据え置き。`transition_limit` の `assert_returned` ほか） |

## 7. 契約適合試験とサンプル

表示・保存・入力判定・案内の担当が `apply()` へ結線するときの合否の基準である（TASK-205）。次のものから成る。

| 物 | 置き場 | 役割 |
|---|---|---|
| サンプル | `testdata/contract/event-sequence.json`（サンプルの版 7、対象は `CONTRACT_VERSION` 7） | 21 件の操作の前後の状態と期待のイベント列 |
| サンプルの説明 | `testdata/contract/README.md` | 欄の意味・版の規則 |
| 形の試験 | `crates/core/tests/contract_sample.rs` | サンプルが core の型で読めること、連なり、網羅 |
| 適合試験 | `crates/core/tests/contract_conformance.rs` | 下表の 3 層 |
| 読む部品 | `crates/core/tests/common/contract_sample.rs` | サンプルを core の型で読む |

適合試験は 3 層に分ける。`apply()` が骨格の間も意味のある層と、結線の合否を決める厳密な層を分け、厳密な層の判定を弱めて通すことはしない。

| 層 | 確かめること | 現在 | CI |
|---|---|---|---|
| サンプルと規則の表 | サンプルの世代の差が 3 節の表と EVT-6 の (1)〜(6) に合う | 通る | 載る |
| 実際の `apply()` の不変条件 | EVT-22（同じ入力列で状態とイベント列が一致）、3 つの入口の一致、世代が減らない・増分は 1 まで | 通る | 載る |
| 厳密な照合 | 連鎖して流し、各操作の状態とイベント列がサンプルと一致する（`sample_replay_matches_apply`） | 全件（e01〜e21）が通る（2026-10-16、TASK-214 1/3 で `#[ignore]` を外した。CI に載る）。単独の照合も残す: e06（`sample_stop_entry_matches_apply`）、e07（`sample_stale_tick_entry_matches_apply`）、e09〜e13（`sample_ticks_entry_matches_apply`）、e15（`sample_confirm_entry_matches_apply`）、e16（`sample_skip_confirm_entry_matches_apply`）、e17・e18（`sample_go_to_entry_matches_apply`）、e19・e20（`sample_restore_entry_matches_apply`。e14 の保存を添える） | 全件の連鎖と上の単独照合が載る |
| 比較器の自己確認 | 期待どおりの提供元は合格、1 か所壊した提供元は最初の食い違いの操作 ID で失敗する | 通る | 載る |

実行コマンド:

- 通常: `make test`（CI の `rust-test` が流す）
- 全件の連鎖の確認: `cargo test --locked -p fandhe-workshop-core --test contract_conformance`（`#[ignore]` は無い）

`sample_replay_matches_apply` の `#[ignore]` は、全件が通ることを確かめて外した（2026-10-16、TASK-214 1/3）。以後、契約の挙動を変える変更は、この試験を通す。
サンプルの版の規則（契約を変えたら版を上げる）は `testdata/contract/README.md` の「版の規則」に従う。

## 8. 変更履歴

| 版 | 日付 | 変更 | 理由 |
|---|---|---|---|
| 1 | 2026-10-15 | 初版。型の一覧・世代の表・宛先の形・机上の照合の記録 | TASK-204 3/3 |
| 2 | 2026-10-15 | `Control::OpenHint`・`Control::SetGuidance`、`Event::HintOpened`、`RefuseReason::NoHint`、`ControlKind::OpenHint`、`RunState.guidance_level`、`SessionState.hints_opened` を追加。世代の表に 2 行、照合の記録に GUIDE-17・GUIDE-3 を追加。`CONTRACT_VERSION` を 1 から 2 へ | TASK-290 |
| 3 | 2026-10-15 | `Event::Judged` を追加。`judge` モジュール（判定結果の統合）を一覧に追加。5 節に `Tick` の発行位置などの未決を、6 節に JUDGE-3 の照合を追加。`CONTRACT_VERSION` を 2 から 3 へ | TASK-267 |
| 4 | 2026-10-16 | `Event::Reloaded`・`session::reload`・`Session::reload` を追加（再読み込みは `Op` に含めない）。3 節に注記、5 節の項目を更新、6 節に SAVE-4 の照合を追加。`CONTRACT_VERSION` を 3 から 4 へ | TASK-320 1/2 |
| 5 | 2026-10-18 | `Slice.clarifying`（問い返しの待ち、EP-4）を追加。旧データは serde の既定で読め、`None` は書き出さない。5 節の項目を更新。`CONTRACT_VERSION` を 4 から 5 へ | TASK-42 2/2 |
| 6 | 2026-10-19 | `Event::TransitionLimitReached`（遷移の上限で止めたことの専用のイベント、SAFE-34・D-45）を追加。作成者向けの `Warning`（SAFE-23）は残し、その直後に 1 件置く。2 節・6 節を更新。`CONTRACT_VERSION` を 5 から 6 へ | TASK-160 2/3 |
| 7 | 2026-10-31 | `RefuseReason::ClosedRound`（閉じた周回の Step への `GoTo` の拒否、MOD-15・D-24）と、理由から引く `RefuseNextAction`・`RefuseReason::next_action` を追加。`Event::Refused` の形は変えない。`CONTRACT_VERSION` を 6 から 7 へ | TASK-249 |

- 2026-10-19: 入口の写しを状態（`SessionState.entry`）に書かず、処理中の停止と上限で止めた後の戻り（SAFE-35、TASK-160 3/3）でだけログから受付の時点を導くようにした（`entry_derive`）。通常の操作では `SessionState.entry` は従来どおり空のままで、履歴の写しの形は変わらない。型・表・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-16: `Restored` による復習・変化の印の作り直しと、ログの並びが書き出し・読み込みで変わらないことの試験（`restore_review`、TASK-214 2/3）を足した。型・表・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-16: チェックポイントの保存と復帰（`checkpoint` モジュール、TASK-214 1/3）を `apply()` に結線し、サンプルの連鎖の全件の照合の `#[ignore]` を外した。`Event::Restored.log_len` の doc を「復帰したチェックポイントの保存の時点のログの長さ」に明確化した（値は変えない）。型・表・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-16: 再読み込みの前に予約された進行の無視（TASK-320 2/2）の試験を足した。型・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-16: 移動前に予約された進行の無視（TASK-213 2/2）の試験を足した。型・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-16: 処理中の停止と入口の写しへの復帰（`stop` モジュール、TASK-211 1/2）を実装した。型・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-15: 7 節（契約適合試験とサンプル）を追加した。型と世代の表は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-205 2/2。版 2 への引き上げは上の表のとおり TASK-290）
- 2026-10-15: `Projection` の導出の中身（ログ行・変化の印・復習・復帰の復習）を実装した。公開の型・署名・直列化の形は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-215 2/2）
- 2026-10-15: e16 の単独照合を追加した。型・直列化の形は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-208）
- 2026-10-16: 食い違う進行を `Ignored` 1 件で記録するようにした（`tick` モジュール）。型・表・署名は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-210 3/3）
- 2026-10-16: 宛先が一致する進行 `Op::Tick` を `apply()` に結線した（`tick` モジュール）。型・表・署名は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-210 2/3）
- 2026-10-23: 学習モジュールの実体ごとの状態の読み出し（`instance`、`Session::instances`・`slice`・`step_status`・`steps_of`）を足した。型・直列化・世代の表は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-233 1/2）
- 2026-10-16: 世代の表を `session::apply` の関門で強制した。型・表・署名は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた（TASK-210 1/3）
- 2026-10-16: 停止前の進行の無視と停止後の入力（TASK-211 2/2）の試験を足し、サンプルの連鎖の照合を先頭 13 件（e01〜e13）に広げた。型・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-19: 1 回の操作で起きる内部の遷移を 1,000 回で止める歩行器（`transit` モジュール、TASK-160 1/3、SAFE-23）を `confirm`・`go_to` に結んだ。止めたときは `Warning` を 1 件足し、現在の Step・処理中・入口の写しを空にする。型・表・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-16: 別の Step への移動（`go_to` モジュール、TASK-213 1/2）を `apply()` に結線した。型・表・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-16: 復帰前の進行の無視と処理中の再開（TASK-214 3/3）の試験を足した。型・表・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-16: `Projection` が `Event::Reloaded` を右のログの 1 行にするようにした（SAVE-6・D-57。TASK-322）。型・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-23: 呼び出しの Step の状態・選択の持ち主を呼ばれた実体の根にした（`instance::owner_of` を足し、`status_of` に `ir` を渡す。フラグ・効果の根は `root_of` のまま。TASK-235 1/2）。状態の型・欄・直列化は変わらず、切り身の鍵が変わるのは `use` の Step の状態だけである。配信が未実装で外に出た保存データが無いこと、spec の session-design §1 の規則に合わせる修正であることから、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-23: Step に入るたびに状態の入口の写しを無効化し、写しを 1 つだけにした（`transit` の着地・呼び出し元への戻り。MOD-10、TASK-242 1/2）。型・表・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-23: 処理中の停止を、持ち主の根の切り身・位置と持ち主が写しの後に書いた世界状態の経路だけ戻す形にした（`stop`・`entry_derive::revert_owner`。MOD-10、TASK-242 2/2）。型・表・署名は変えないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-27: 現在の根の下書きを読む `Session::current_draft`・`RecordedSession::current_draft`・`SessionState::draft_root`・`current_draft` を足した（読み取り専用。`drafts` から導くだけで欄は足さない。MOD-4、TASK-236 1/2）。型・直列化・世代の表は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた
- 2026-10-29: ヒントの使用回数を読む `SessionState::hints_opened_at`・`hints_opened_in` と、`Session`・`RecordedSession` の同名の読み取り口を足した（読み取り専用。`hints_opened` から導くだけで欄は足さない。GUIDE-7、TASK-297 1/2）。型・直列化・世代の表は変わらないため、文書の版・`CONTRACT_VERSION` は据え置いた
