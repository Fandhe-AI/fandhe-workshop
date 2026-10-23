//! 実行環境 [`Session`]、状態の型、純粋な遷移関数 [`apply`] の署名（TASK-204 2/3、契約の版は [`crate::CONTRACT_VERSION`]）。
//!
//! 表示・保存・入力判定・案内の担当が並行して前提にする公開 API を、型と署名として固定するのが役割である。
//! 呼び出し元は web（入力の受け口と表示）・保存（状態の書き出しと復元）・契約適合試験（TASK-205）。
//! 状態変化の提供元の境界（[`crate::provider::StateChangeProvider`]）の具体型は、[`Engine`] が
//! [`SessionState`]・[`Op`]・[`Event`] に束縛する。
//!
//! 本件が固定するのは型・署名・純粋な補助関数・規則の表までで、状態機械の各分岐は実装しない。
//! [`apply`] の本体は契約だけの骨格である。入力の受付（`Op::Input`、TASK-207 1/3）は `accept` モジュールが、受付済みへの確認の操作による達成（TASK-207 2/3）は `confirm` モジュールが埋めた。
//! 世代の規則の表（[`GenerationRule`]）は、[`apply`] が分岐の後に強制する（TASK-210 1/3、EVT-6）。
//! 宛先が一致する進行 `Op::Tick` は `tick` モジュールが進め、食い違う進行は `Ignored` 1 件で記録する（TASK-210 2/3・3/3、EVT-8・EVT-16）。
//! 処理中の停止は `stop` モジュールが入口の写しへ戻す（TASK-211 1/2、EVT-9）。
//! 別の Step への移動は `go_to` モジュールが行い、入口の写しを無効化する（TASK-213 1/2、EVT-11。写しは Step に入るたびに `transit` が無効化し、ログから導く。MOD-10・TASK-242 1/2）。
//! 処理中でない停止は取り消しの入力 `Input::Cancel` として `accept` へ委ねる（TASK-212、EVT-10）。
//! チェックポイントの保存と復帰は `checkpoint` モジュールが行う（TASK-214 1/3、EVT-12）。
//! 再読み込みは `apply` の外にあり、`reload` モジュールが世代を +1 する（[`reload()`]、TASK-320 1/2、SAVE-4）。
//! ヒントを 1 段開く操作は `hint_open` モジュールが行い、出せる数を上限に拒否する（TASK-295、GUIDE-17）。
//! 残りの分岐は状態を変えずイベントも返さず、担当タスク
//! （`Changed` と導出 TASK-215、途中開始 TASK-217、周回と呼び出しの Step の持ち主 TASK-235〜239・241）が埋める。
//! 実体の根ごとの状態の読み出しは [`crate::instance`]（TASK-233 1/2）が持ち、[`Session`] の読み取り用メソッドから引ける。
//!
//! 不変条件:
//! - [`apply`] は全域（panic しない）で決定的。I/O・時刻・乱数を使わない（EVT-22）
//! - [`apply`] が返すイベント列は、新しい状態の [`SessionState::log`] の末尾に足された分と一致する
//! - 操作 1 件の中で、経路ごとに `Changed` は高々 1 件で、変更前は操作前の値である（EVT-13、[`crate::changed`]）
//! - 世代は減らない（変わり方の表は [`crate::input::GenerationRule`]）
//! - 拒否された入力は状態を変えない。破る実装があれば、`apply` が戻して `Warning` を足す（`reject_guard`、JUDGE-5）
//!
//! 状態の値は教材・利用者由来の文字列を含みうる。HTML への埋め込みのエスケープは web 層の責務である。
//! 直列化は `BTreeMap`・`BTreeSet` で順を決定的にし、未知の欄は拒否する。欄の追加・変更は版上げになる。

use std::collections::{BTreeMap, BTreeSet};

use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Ir;
use serde::{Deserialize, Serialize};

use crate::accept;
use crate::addr::{Generation, Root, StepUid};
use crate::begin;
use crate::changed;
use crate::checkpoint;
use crate::event::{Cause, Event};
use crate::go_to;
use crate::hint_open;
use crate::input::{CheckpointId, Control, GenerationRule, Input, Op};
use crate::instance;
use crate::provider::StateChangeProvider;
use crate::reload;
use crate::step_status::StepStatus;
use crate::stop;
use crate::tick;
use crate::world::World;

/// 処理中（進行を順に受けている間）の状態。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Busy {
    /// 次に来るべき進行の番号（0 始まり）。
    pub done: u32,
    /// 進行の総数。
    pub total: u32,
}

/// 実体の根ごとの切り身。停止・復帰で持ち主の根の分だけを戻す（MOD-3・MOD-9）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slice {
    /// Step ごとの状態（未登録は `NotStarted`）。
    pub status: BTreeMap<StepUid, StepStatus>,
    /// Step ごとに選んだ選択肢の ID。
    pub choices: BTreeMap<StepUid, String>,
    /// この根のフラグ。
    pub flags: BTreeSet<String>,
    /// 問い返しの待ち（EP-4）。待っている問い返しの ID で、`clarifying` 条件が読む。
    ///
    /// 旧データを読めるよう、無ければ `None`、`None` なら書き出さない。立てる・消す処理は結線のタスク
    /// （TASK-207・TASK-267）の持ち分で、それまで本番の値は常に `None` となり、`clarifying` 条件は偽
    /// （取り消しの受付の規則は当たらない）に倒れる。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clarifying: Option<String>,
}

/// 実行の位置と世界の状態。停止・復帰が戻す単位である（下書きとログは含まない）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunState {
    /// 現在の世代。
    pub generation: Generation,
    /// 現在の Step。開始前は `None`。
    pub current: Option<StepUid>,
    /// 処理中なら `Some`。
    pub busy: Option<Busy>,
    /// 章を完了したか。
    pub completed: bool,
    /// 根ごとの切り身。
    pub slices: BTreeMap<Root, Slice>,
    /// 世界状態全体。
    pub world: World,
    /// 根をまたぐフラグ。
    pub global_flags: BTreeSet<String>,
    /// 案内レベル。実行全体で 1 つで、切り身（[`Slice`]）には置かない（GUIDE-3・GUIDE-13、D-55）。
    /// 既定は A で、実際の開始値は開始時に IR から決める（TASK-294）。切り替えは `Control::SetGuidance`。
    pub guidance_level: GuidanceLevel,
}

impl Default for RunState {
    fn default() -> Self {
        RunState {
            generation: Generation::INITIAL,
            current: None,
            busy: None,
            completed: false,
            slices: BTreeMap::new(),
            world: World::new(),
            global_flags: BTreeSet::new(),
            guidance_level: GuidanceLevel::A,
        }
    }
}

/// 入口の写し。停止の戻り先で、そのときのログの長さを添える。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryCopy {
    /// 入った時点の実行状態。
    pub run: RunState,
    /// そのときのログの長さ。
    pub log_len: u32,
}

/// 保存したチェックポイント（`Control::Save`）。全体の状態の写し（`run`）・入口の写し・保存の時点のログの長さ・持ち主の根の 4 つを持つ（MOD-10）。
/// 通し番号は [`SessionState::next_checkpoint`] の 1 つで、実体・周回をまたいで振る。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    /// 保存時点の実行状態。
    pub run: RunState,
    /// 保存時点の入口の写し。
    pub entry: Option<EntryCopy>,
    /// 保存時点のログの長さ。
    pub log_len: u32,
    /// 保存時に `owner_of` で決めた持ち主の根（呼び出しの Step なら呼ばれた実体の根。無ければ `None`）。
    pub owner: Option<Root>,
}

/// [`apply`] が受け渡す状態の全体。`StateChangeProvider` の `State` である。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionState {
    /// 実行の位置と世界の状態。
    pub run: RunState,
    /// これまでのイベントのログ。追記だけで、[`apply`] の戻りのイベント列が末尾に足される。
    pub log: Vec<Event>,
    /// 入口の写し。通常の操作では値を入れず（`None`）、写しを持つチェックポイントへの復帰でだけ戻る。
    /// 処理中の停止・上限で止めた後の戻りが使う写しは、この欄が空ならログから導く（`entry_derive`、SAFE-35）。
    /// 欄に値を入れると履歴の写し（`snapshot`）が変わり、同じ版の履歴の照合（VER-2・VER-3）に響く。
    pub entry: Option<EntryCopy>,
    /// 保存したチェックポイント。
    pub checkpoints: BTreeMap<CheckpointId, Checkpoint>,
    /// 次に振るチェックポイントの ID。
    pub next_checkpoint: CheckpointId,
    /// 根ごとの下書き。鍵は `StepUid::root`（入れ子の実体では `Instance.id`）。停止・復帰で戻さない（MOD-4・MOD-9）。下書きを変える操作は版 1 では固定しない。
    pub drafts: BTreeMap<Root, String>,
    /// Step の実体ごとに開いたヒントの段数。停止・復帰で戻さない（GUIDE-7）ため、`run` の外に置く。
    /// 1 段ずつ増やす処理は `hint_open`（TASK-295）。実体ごとの鍵と周回をまたぐ合計は TASK-297 が実装する。
    pub hints_opened: BTreeMap<StepUid, u32>,
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState {
            run: RunState::default(),
            log: Vec::new(),
            entry: None,
            checkpoints: BTreeMap::new(),
            next_checkpoint: CheckpointId(1),
            drafts: BTreeMap::new(),
            hints_opened: BTreeMap::new(),
        }
    }
}

/// 状態に操作を適用し、新しい状態とイベント列を返す純粋な関数（EXT-4）。
///
/// 全域で決定的であり、教材 `ir` は読むだけである。戻りのイベント列は新しい状態の `log` の末尾に
/// 足された分と一致する。入力の分岐は受付と確認による達成まで（TASK-207 1/3・2/3）、進行は一致の前進と食い違いの無視の記録まで（TASK-210 2/3・3/3）、処理中の停止は入口の写しへの復帰まで（TASK-211 1/2）、移動は移動先への入りと入口の写しの無効化まで（TASK-213 1/2）、保存と復帰は写しの控えと戻しまで（TASK-214 1/3。復帰前の進行の無視と処理中の再開の試験は 3/3）実装済みで、ほかの分岐は契約だけの骨格のまま、
/// 状態を変えずイベントも返さない。
pub fn apply(ir: &Ir, state: SessionState, op: Op) -> (SessionState, Vec<Event>) {
    let mut state = state;
    // 操作 1 件の `Changed` は、前後の世界状態の差分から作る（EVT-13、TASK-215）。各分岐が語レベルの
    // `Changed` を出しても、ここで経路ごとに 1 件・変更前は操作前の値に置き換わる。
    // 上限で止めた操作を操作の前の状態へ戻すため（SAFE-35）、世界状態だけでなく `run` 全体と状態の入口の写しの欄を
    // 控える。処理中の戻り先（受付の時点）は、止めたときにだけ操作の前のログから導く（`transit::return_after_limit`）。
    // 費用は切り身・フラグなど `world` 以外の分の複製が増えるだけで、操作 1 件につき 1 回に収まる。
    let run_before = state.run.clone();
    let entry_before = state.entry.clone();
    let current_before = state.run.current.clone();
    let rule = op.generation_rule();
    let gen_before = state.run.generation;
    let was_busy = state.run.busy.is_some();
    // 処理中でない停止は取り消しの入力として扱うので、`Changed` の原因も入力にする（EVT-10）。
    let cause = if matches!(op, Op::Control(Control::Stop)) && !was_busy {
        Cause::Input
    } else {
        changed::cause_of(&op)
    };
    let events = step(ir, &mut state, op);
    // 拒否は状態を変えない（JUDGE-5）。破っていれば戻して `Warning` で見える形にする。上限の戻しより前に行う。
    let events =
        crate::reject_guard::guard_rejection(events, &mut state, &run_before, &entry_before);
    // 上限で止めたら、操作の前の Step・状態へ戻す（SAFE-35、D-45）。世代の強制より前に行う。
    let mut cause = cause;
    let mut returned_generation = None;
    if events
        .iter()
        .any(|e| matches!(e, Event::TransitionLimitReached { .. }))
    {
        match crate::transit::return_after_limit(&mut state, &run_before, &entry_before) {
            crate::transit::LimitReturn::Halted => {}
            crate::transit::LimitReturn::Restored => {
                if was_busy {
                    returned_generation = Some(state.run.generation);
                }
            }
            crate::transit::LimitReturn::RewoundToEntry => {
                returned_generation = Some(state.run.generation);
                // 入口へ戻した世界状態の差分は取り消しで、学習者の変更ではない（`Projection` が併合しない）。
                cause = Cause::Stop;
            }
        }
    }
    // 世代の規則の表を分岐の外で強制する。`Changed` の宛先（下の `default_stamp`）より前に行い、
    // 状態とイベントの世代を食い違わせない。
    state.run.generation = settle_generation(rule, was_busy, gen_before, state.run.generation);
    // 処理中から戻したときの世代の +1 は、表の強制で消さない（復帰前の進行を無効にする。`return_after_limit`）。
    if let Some(generation) = returned_generation {
        state.run.generation = generation;
    }
    // 宛先の既定は操作後の世代と Step。各分岐のタスクが見直してよい初版の規則である。
    let default_at = changed::default_stamp(
        state.run.generation,
        state.run.current.as_ref(),
        current_before.as_ref(),
    );
    let events = changed::record_changes(
        events,
        &run_before.world,
        &state.run.world,
        &default_at,
        cause,
    );
    state.log.extend(events.iter().cloned());
    (state, events)
}

/// 再読み込み（SAVE-4）。処理中のまま続け、世代を +1 して `Reloaded` を 1 件ログへ足す純粋な関数。
///
/// `apply` の `Op` には含めない（世代の表を変えないため）。戻りのイベント列は新しい状態の `log` の末尾に
/// 足された分と一致する。現在の Step が無いとき、世代があふれるときは状態を変えない（後者は `Warning` のみ）。
pub fn reload(state: SessionState) -> (SessionState, Vec<Event>) {
    let mut state = state;
    let events = reload::on_reload(&mut state);
    state.log.extend(events.iter().cloned());
    (state, events)
}

/// 操作 1 件の後の世代を、[`GenerationRule`] の表に合わせて決める（EVT-6、TASK-210 1/3）。
///
/// 呼び出し元は [`apply`] のみ。分岐（`accept`・`confirm` や後続の停止・移動・復帰）が表に反して世代を
/// 動かしても、ここで戻して fail-closed にする。世代は減らず、増分は 1 までで、あふれは変えない側に倒す。
/// 全域で panic しない。表に variant を足すときは、ここも網羅の誤りとして気づける（同じ crate では `match` が網羅を要求する）。後続の分岐の世代の誤りがここで黙って直るため、各分岐の試験は世代を運ぶイベントの
/// `new_generation` と状態の世代の一致を別に確かめる。
fn settle_generation(
    rule: GenerationRule,
    was_busy: bool,
    before: Generation,
    after: Generation,
) -> Generation {
    // 増分が認められるのは「そのまま」か「ちょうど 1 つ先」だけ。
    let stepped = if after == before || before.next() == Some(after) {
        after
    } else {
        before
    };
    match rule {
        GenerationRule::Keep => before,
        GenerationRule::IncrementIfProcessing => {
            if was_busy {
                stepped
            } else {
                before
            }
        }
        GenerationRule::IncrementOnSuccess => stepped,
    }
}

/// 操作 1 件の遷移。返したイベント列は呼び出し側 [`apply`] がログへ足す。
fn step(ir: &Ir, state: &mut SessionState, op: Op) -> Vec<Event> {
    match op {
        // 入力の受付は `accept`（TASK-207 1/3）、確認の操作による達成は `confirm`（TASK-207 2/3）。世代と宛先の照合は TASK-210。
        Op::Input(input) => accept::on_input(ir, state, input),
        // 宛先が一致する進行の前進と食い違う進行の無視の記録は `tick`（TASK-210 2/3・3/3）。世代は変えない（1/3、`apply` の関門）。
        Op::Tick(addr) => tick::on_tick(ir, state, addr),
        // 処理中の停止と入口の写しへの復帰は `stop`（TASK-211 1/2）。世代は +1 になる（`apply` の関門が上限を守る）。
        // 処理中でない停止は取り消しの入力 `Input::Cancel` として扱う（TASK-212、EVT-10）。`stop` は教材を読まない
        // 不変条件を保つため、分岐はここで行う。世代は変わらない（`apply` の関門）。
        Op::Control(Control::Stop) if state.run.busy.is_none() => {
            accept::on_input(ir, state, Input::Cancel)
        }
        Op::Control(Control::Stop) => stop::on_stop(state),
        // 移動は `go_to`（TASK-213 1/2）。世代は +1 になり、入口の写しは無効化する（着地の無効化は `transit`。MOD-10）。
        Op::Control(Control::GoTo(to)) => go_to::on_go_to(ir, state, to),
        // 保存と復帰は `checkpoint`（TASK-214 1/3）。保存は世代を変えず、復帰は成功したときだけ +1 になる（`apply` の関門）。
        // 保存が成功し、現在の Step が保存を入力とする宣言（`env_action`）なら受付にする（TASK-64 1/2、PACK-34）。
        Op::Control(Control::Save) => {
            let mut events = checkpoint::on_save(ir, state);
            if events
                .iter()
                .any(|e| matches!(e, Event::CheckpointSaved { .. }))
            {
                events.extend(crate::env_input::on_saved(ir, state));
            }
            events
        }
        Op::Control(Control::Restore(id)) => checkpoint::on_restore(ir, state, id),
        // ヒントを 1 段開く操作と上限・Step 無しでの拒否は `hint_open`（TASK-295、GUIDE-17）。世代は変わらない（`Keep`）。
        Op::Control(Control::OpenHint) => hint_open::on_open_hint(ir, state),
        // 次の周回の開始は `begin`（TASK-237 1/2）。開始条件は状態に触れる前に評価し、世代は成功したときだけ +1 になる。
        Op::Control(Control::Begin(start)) => begin::on_begin(ir, state, &start),
        // TASK-294 案内レベルの切り替え。
        Op::Control(_) => Vec::new(),
    }
}

/// 教材（IR）を束ねて [`StateChangeProvider`] を満たす提供元。
///
/// 受け手は `&self` で、世界状態をすべて [`SessionState`] に置く。呼び出し元は [`Session`] と適合試験。
#[derive(Debug, Clone, Copy)]
pub struct Engine<'ir> {
    ir: &'ir Ir,
}

impl<'ir> Engine<'ir> {
    /// 教材を借りて作る。
    pub fn new(ir: &'ir Ir) -> Self {
        Engine { ir }
    }
}

impl StateChangeProvider for Engine<'_> {
    type State = SessionState;
    type Input = Op;
    type Event = Event;

    fn apply(&self, state: SessionState, input: Op) -> (SessionState, Vec<Event>) {
        apply(self.ir, state, input)
    }
}

/// 実行環境。教材と状態を持ち、操作を順に適用する。世代は状態の中にある。
///
/// 呼び出し元は web（1 つの教材の実行）。保存からの復元は [`Session::from_state`] で行う。
/// 世代の規則の表（[`GenerationRule`]）は [`apply`] の境界で強制される。
#[derive(Debug, Clone)]
pub struct Session {
    ir: Ir,
    state: SessionState,
}

impl Session {
    /// 初期状態（現在の Step なし）で作る。開始は `Control::Begin` が担う。
    pub fn new(ir: Ir) -> Self {
        Session {
            ir,
            state: SessionState::default(),
        }
    }

    /// 開始点の節点に入った直後の状態で作る（配信する設定の起動。TASK-112 2/2）。
    ///
    /// 世代は初期値・現在の Step は `node`・その Step は `Entered`・入口の写しは同じ状態（ログの長さ 0）で、世界状態は空、
    /// 案内の水準は既定。`Control::Begin` は実装済みだが、飛ばした Step の記録（TASK-217）・案内の水準の決定（TASK-294）と
    /// web の結線（TASK-254）が済むまでの暫定で、
    /// 呼び出し元の web は表示層が状態を組み立てない規則（`no_progress`）のため、この入口を通す。`node` が教材に有るかの照合は
    /// 呼び出し側が行う（`from_state` と同じ）。
    pub fn entered_at(ir: Ir, node: &str) -> Self {
        let uid = StepUid(node.to_owned());
        let mut slice = Slice::default();
        slice.status.insert(uid.clone(), StepStatus::Entered);
        let mut run = RunState {
            current: Some(uid.clone()),
            ..RunState::default()
        };
        run.slices
            .insert(crate::instance::owner_of(&ir, &uid), slice);
        let state = SessionState {
            entry: Some(EntryCopy {
                run: run.clone(),
                log_len: 0,
            }),
            run,
            ..SessionState::default()
        };
        Session { ir, state }
    }

    /// 既存の状態から作る。IR との照合はしない（保存復元の担当が行う）。
    pub fn from_state(ir: Ir, state: SessionState) -> Self {
        Session { ir, state }
    }

    /// 教材。
    pub fn ir(&self) -> &Ir {
        &self.ir
    }

    /// 現在の状態。
    pub fn state(&self) -> &SessionState {
        &self.state
    }

    /// 現在の世代。
    pub fn generation(&self) -> Generation {
        self.state.run.generation
    }

    /// これまでのイベントのログ。
    pub fn log(&self) -> &[Event] {
        &self.state.log
    }

    /// 操作を適用し、今回足されたイベントを返す。
    pub fn apply(&mut self, op: Op) -> &[Event] {
        let before = self.state.log.len();
        let state = std::mem::take(&mut self.state);
        let (state, _events) = apply(&self.ir, state, op);
        self.state = state;
        self.state.log.get(before..).unwrap_or_default()
    }

    /// 再読み込みし、今回足されたイベントを返す（[`reload()`]）。
    pub fn reload(&mut self) -> &[Event] {
        let before = self.state.log.len();
        let state = std::mem::take(&mut self.state);
        let (state, _events) = reload(state);
        self.state = state;
        self.state.log.get(before..).unwrap_or_default()
    }

    /// 教材の実体の根を IR の順に返す（章の実体 `main` を含む。MOD-1）。
    pub fn instances(&self) -> Vec<Root> {
        instance::instance_roots(&self.ir)
    }

    /// 実体の根の切り身。まだ何も書かれていない根は `None`。
    pub fn slice(&self, root: &Root) -> Option<&Slice> {
        instance::slice_of(&self.state.run, root)
    }

    /// Step の実体の状態。未登録は `NotStarted`。
    pub fn step_status(&self, uid: &StepUid) -> StepStatus {
        instance::status_of(&self.ir, &self.state.run, uid)
    }

    /// 実体の根に属する Step の実体を IR の順に返す。
    pub fn steps_of(&self, root: &Root) -> Vec<StepUid> {
        instance::steps_of(&self.ir, root)
    }

    /// 状態への可変の参照。下書きの写し（`drafts`）を `ops` の記録と同時に更新するためだけに、
    /// [`crate::recorded_session::RecordedSession`] が使う。進行（世代・Step・ログ）は [`Session::apply`] 以外で変えない。
    pub(crate) fn state_mut(&mut self) -> &mut SessionState {
        &mut self.state
    }

    /// 状態を取り出す（保存のため）。
    pub fn into_state(self) -> SessionState {
        self.state
    }
}
