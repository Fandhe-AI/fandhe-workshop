//! 処理中の進行の連続適用（TASK-227 1/2、EVT-17）。
//!
//! 処理中の演出の Step は、宛先つきの `Op::Tick` を 1 件ずつ [`Session::apply`] に渡して進む。通常の速さでは
//! web のホストのタイマーが一定の間隔で届ける。早送りは、その進行の列を間隔を待たずに続けて適用する。
//! 呼び出し元は web のホスト（切り替えの結線は SHELL-24 のタスク）と試験である。
//! web の `tick_schedule::next_tick` と `next_tick` は同じ規則の重複で、解消は結線のタスクで行う。
//!
//! 規則（上から評価する）。
//! 1. 処理中でなければ何も適用せず [`FastForwardEnd::NotBusy`] で返る
//! 2. 適用した数が `limit` に達したら [`FastForwardEnd::LimitReached`] で止まる（呼び出し側が停止を挟める）
//! 3. 宛先を状態から組み立て、`Op::Tick` を 1 件適用する
//! 4. 適用の後に処理中が解けていれば [`FastForwardEnd::Finished`]。世代・Step が変わった、または
//!    番号が進んでいなければ [`FastForwardEnd::Stalled`] で打ち切る（fail-closed）
//!
//! 同一性の根拠。連続適用は新しい操作を作らず、通常の速さで届く進行と同じ `Op::Tick` の列を同じ順に `apply` へ
//! 渡す。変わるのは間隔だけなので、状態・ログ・`Progress` の数・`Changed`・`ops` は通常の速さと一致する
//! （EVT-17・EVT-27）。早送りの入・切は `ops` にもイベントにも残さない（D-54）。
//!
//! 停止。早送りは同期のループなので途中で停止を観察しない。呼び出し側が `limit` で区切り、区切りの間に `Control::Stop` を
//! 適用する。停止の後は処理中が解けて `NotBusy` になり、停止前の宛先の進行は `Ignored` で無視される（EVT-9。`wait` の
//! Step の停止後の動きは TASK-231）。
//!
//! 扱わないこと。受講者の入力・確認・承認を補わない（適用するのは進行だけなので、確認・選択・承認・復帰の場面は削られない）。早送りの切り替えの画面は SHELL-24〜。
//! 契約の版（`CONTRACT_VERSION`）の対象外のモジュールである。
//!
//! 不変条件。終わることは、番号が狭義単調増加であることと `limit` で保証する。教材由来の値と履歴由来の
//! `busy` は信頼しない。I/O・時刻・乱数は持たない。遷移の上限で止めて世代が +1 された場合も `Stalled` で
//! 止める（通常の速さでも古い宛先として無視される経路で、同一性は崩れない）。

use crate::addr::TickAddr;
use crate::event::Event;
use crate::input::Op;
use crate::session::{Session, SessionState};

/// 連続適用が止まった理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FastForwardEnd {
    /// 呼び出し時に処理中でない。何も適用せず、記録もしない。
    NotBusy,
    /// 処理中が解けた（最後の進行を適用した。`confirm: none` の達成・遷移を含む）。
    Finished,
    /// 上限の数だけ適用した。処理中のまま残る。
    LimitReached,
    /// 適用しても番号が進まない、または宛先の世代・Step が変わった。fail-closed で打ち切った。
    Stalled,
}

/// 1 回の連続適用の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastForward {
    /// 適用した進行の宛先（適用の順）。
    pub ticks: Vec<TickAddr>,
    /// 各進行の `apply` が返したイベントを順に連結したもの（ログの末尾に足された分）。
    pub events: Vec<Event>,
    /// 止まった理由。
    pub end: FastForwardEnd,
}

/// 処理中の Step の、次に来るべき進行の宛先。処理中でなければ `None`。
pub(crate) fn next_tick(state: &SessionState) -> Option<TickAddr> {
    let step = state.run.current.clone()?;
    let busy = state.run.busy.as_ref()?;
    Some(TickAddr {
        generation: state.run.generation,
        step,
        index: busy.done,
    })
}

/// 進行を 1 件適用できる対象。[`Session`] と記録つきの実行環境が実装し、ループを 1 か所に保つ。
pub(crate) trait TickTarget {
    fn state(&self) -> &SessionState;
    fn apply_tick(&mut self, op: Op) -> Vec<Event>;
}

impl TickTarget for Session {
    fn state(&self) -> &SessionState {
        Session::state(self)
    }

    fn apply_tick(&mut self, op: Op) -> Vec<Event> {
        self.apply(op).to_vec()
    }
}

/// 連続適用のループ本体。
pub(crate) fn drive<T: TickTarget>(target: &mut T, limit: u32) -> FastForward {
    let mut out = FastForward {
        ticks: Vec::new(),
        events: Vec::new(),
        end: FastForwardEnd::NotBusy,
    };
    let mut next = next_tick(target.state());
    if next.is_none() {
        return out;
    }
    let mut applied: u32 = 0;
    while let Some(addr) = next {
        if applied >= limit {
            out.end = FastForwardEnd::LimitReached;
            return out;
        }
        let events = target.apply_tick(Op::Tick(addr.clone()));
        applied = applied.saturating_add(1);
        out.events.extend(events);
        out.ticks.push(addr.clone());
        next = next_tick(target.state());
        if let Some(n) = &next {
            if n.generation != addr.generation || n.step != addr.step || n.index <= addr.index {
                out.end = FastForwardEnd::Stalled;
                return out;
            }
        }
    }
    out.end = FastForwardEnd::Finished;
    out
}

/// 現在の処理中の進行を、間隔を待たずに最大 `limit` 件まで 1 件ずつ適用する。
pub fn fast_forward(session: &mut Session, limit: u32) -> FastForward {
    drive(session, limit)
}
