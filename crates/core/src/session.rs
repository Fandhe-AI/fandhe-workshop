//! 実行環境 [`Session`]、状態の型、純粋な遷移関数 [`apply`] の署名（TASK-204 2/3、契約の版は [`crate::CONTRACT_VERSION`]）。
//!
//! 表示・保存・入力判定・案内の担当が並行して前提にする公開 API を、型と署名として固定するのが役割である。
//! 呼び出し元は web（入力の受け口と表示）・保存（状態の書き出しと復元）・契約適合試験（TASK-205）。
//! 状態変化の提供元の境界（[`crate::provider::StateChangeProvider`]）の具体型は、[`Engine`] が
//! [`SessionState`]・[`Op`]・[`Event`] に束縛する。
//!
//! 本件が固定するのは型・署名・純粋な補助関数・規則の表までで、状態機械の各分岐は実装しない。
//! [`apply`] の本体は契約だけの骨格で、状態を変えずイベントも返さない。各分岐は担当タスク
//! （受付・達成 TASK-207、世代と進行の照合 TASK-210、停止 TASK-211、移動 TASK-213、保存と復帰 TASK-214、
//! `Changed` と導出 TASK-215、途中開始 TASK-217、実体と周回 TASK-233〜239）が埋める。
//!
//! 不変条件:
//! - [`apply`] は全域（panic しない）で決定的。I/O・時刻・乱数を使わない（EVT-22）
//! - [`apply`] が返すイベント列は、新しい状態の [`SessionState::log`] の末尾に足された分と一致する
//! - 世代は減らない（変わり方の表は [`crate::input::GenerationRule`]）
//!
//! 状態の値は教材・利用者由来の文字列を含みうる。HTML への埋め込みのエスケープは web 層の責務である。
//! 直列化は `BTreeMap`・`BTreeSet` で順を決定的にし、未知の欄は拒否する。欄の追加・変更は版上げになる。

use std::collections::{BTreeMap, BTreeSet};

use fandhe_workshop_contract::ir::Ir;
use serde::{Deserialize, Serialize};

use crate::addr::{Generation, Root, StepUid};
use crate::event::Event;
use crate::input::{CheckpointId, Op};
use crate::provider::StateChangeProvider;
use crate::step_status::StepStatus;
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

/// 保存したチェックポイント（`Control::Save`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    /// 保存時点の実行状態。
    pub run: RunState,
    /// 保存時点の入口の写し。
    pub entry: Option<EntryCopy>,
    /// 保存時点のログの長さ。
    pub log_len: u32,
    /// 持ち主の根（無ければ `None`）。
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
    /// 入口の写し。
    pub entry: Option<EntryCopy>,
    /// 保存したチェックポイント。
    pub checkpoints: BTreeMap<CheckpointId, Checkpoint>,
    /// 次に振るチェックポイントの ID。
    pub next_checkpoint: CheckpointId,
    /// 根ごとの下書き。停止・復帰で戻さない（MOD-4・MOD-9）。下書きを変える操作は版 1 では固定しない。
    pub drafts: BTreeMap<Root, String>,
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
        }
    }
}

/// 状態に操作を適用し、新しい状態とイベント列を返す純粋な関数（EXT-4）。
///
/// 全域で決定的であり、教材 `ir` は読むだけである。戻りのイベント列は新しい状態の `log` の末尾に
/// 足された分と一致する。現時点は契約だけの骨格で、状態を変えずイベントも返さない。
pub fn apply(ir: &Ir, state: SessionState, op: Op) -> (SessionState, Vec<Event>) {
    // 骨格のため教材はまだ読まない。分岐を埋めるタスクが読む。
    let _ = ir;
    let mut state = state;
    let events = step(&mut state, op);
    state.log.extend(events.iter().cloned());
    (state, events)
}

/// 操作 1 件の遷移。返したイベント列は呼び出し側 [`apply`] がログへ足す。
fn step(state: &mut SessionState, op: Op) -> Vec<Event> {
    let _ = state;
    match op {
        // TASK-207: 入力の受付と達成。TASK-210: 世代と宛先の照合。
        Op::Input(_) => Vec::new(),
        // TASK-210: 宛先つきの進行。
        Op::Tick(_) => Vec::new(),
        // TASK-211 停止、TASK-213 移動、TASK-214 保存と復帰、TASK-217 途中開始、TASK-233〜239 周回。
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

    /// 状態を取り出す（保存のため）。
    pub fn into_state(self) -> SessionState {
        self.state
    }
}
