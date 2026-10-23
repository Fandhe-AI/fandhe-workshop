//! 宛先つきの進行 `Op::Tick` の分岐（TASK-210 2/3・3/3、EVT-8・EVT-16）。
//!
//! 宛先（世代・Step の実体 ID・進行の番号）が現在の「次の進行」と一致したときだけ、進行を 1 つ進める。
//! 食い違うときは状態を変えず、`Ignored` を 1 件だけ記録する。
//! 進行は時刻ではなく `Tick` の操作の列として進むので、同じ操作の列からは同じ状態と同じイベント列が再現される。
//! 呼び出し元は [`crate::session::apply`] だけで、公開 API には含めない（契約の対象外）。
//! 照合は [`crate::judge::judge_tick`] が 1 か所で行い、本モジュールは 3 要素の比較を再実装しない。
//! 効果の適用は [`crate::effect::apply_tick`]、確認を省く Step の達成は [`crate::confirm`] が持つ。
//!
//! # 規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 宛先が食い違うなら、状態を変えず `Ignored` を 1 件だけ返す（`at` は操作前の世代と現在の Step、
//!    理由は [`crate::judge::judge_tick`] のもの。同じ番号の二重の届きは `Stale`）。現在の Step が無い場合は
//!    `Stamp` を作れないため空のイベント列を返す（`accept::on_input` と揃える）
//! 2. 現在の Step が IR に無い、`busy.total` が `processing` の数と合わない、`busy.done` が範囲の外、
//!    進行の宣言が読めない、のいずれかなら、固定文の `Warning` だけを出して状態を変えない（fail-closed）
//! 3. `Progress`（番号は適用した進行の 0 始まり）→ 効果のイベントの順に出し、`busy.done` を 1 つ進める
//! 4. 最後の進行なら処理中を解く。Step の状態は `Accepted` のまま残す。ただし `confirm: none` の Step は
//!    続けて達成の処理（`Achieved` → 効果 → `Transitioned` → `Entered`）を行う（EVT-5）
//!
//! `Tick` は `Judged` を出さない（版 3 の契約サンプル e07 の形。無視も `Ignored` 1 件だけ）。
//!
//! # 扱わないこと
//!
//! - 二重の届き専用の無視の理由（版上げの契約タスク。現状は `Stale`）
//! - `wait` の Step の、入った時点からの処理中と処理の完了での受付（EVT-20。Step に入るときの処理を持つタスク）
//! - 停止・移動・復帰（TASK-211・213・214）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 世代は変えない（[`crate::session::apply`] の関門が強制する）。`Warning` の文面は固定文で、教材の値を埋め込まない。

use fandhe_workshop_contract::effect::ProcessingTick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::Judgment;

use crate::addr::{Stamp, TickAddr};
use crate::confirm;
use crate::effect::{apply_tick, NoConditions};
use crate::event::{Cause, Event};
use crate::judge::{judge_tick, JudgeDetail};
use crate::session::{RunState, SessionState};
use crate::step_status::StepStatus;

/// 進行の宣言・総数・現在の Step が読めないときの固定文。教材の値を含めない。
const INVALID_PROCESSING: &str = "processing declaration could not be read; tick ignored";

/// 確認を省く宣言が不正なときの固定文。
const INVALID_SKIP: &str = "confirm none declaration could not be read; step not achieved";

/// 食い違う進行の無視を `Ignored` 1 件で記録する（状態は変えない）。
///
/// `at` は操作前の状態の世代と現在の Step で、宛先の値は使わない（外から来た値を記録に写さない）。
/// 現在の Step が無い、または判定が `Ignored` の理由を持たない（想定外の組）ときは、状態を変えず空を返す。
fn ignored(run: &RunState, detail: &JudgeDetail) -> Vec<Event> {
    match (&run.current, detail) {
        (Some(step), JudgeDetail::Ignored(reason)) => vec![Event::Ignored {
            at: Stamp {
                generation: run.generation,
                step: step.clone(),
            },
            reason: *reason,
        }],
        _ => Vec::new(),
    }
}

/// `Op::Tick` の分岐。一致した進行を 1 つ進め、出たイベント列を返す。
pub(crate) fn on_tick(ir: &Ir, state: &mut SessionState, addr: TickAddr) -> Vec<Event> {
    // 照合の正は `judge_tick` の 1 か所。IR の参照・進行宣言の検査・効果の適用より先に行う（EVT-8）。
    let outcome = judge_tick(&state.run, &addr);
    if outcome.judgment != Judgment::Progressed {
        return ignored(&state.run, &outcome.detail);
    }
    let (Some(current), Some(busy)) = (state.run.current.clone(), state.run.busy.clone()) else {
        return Vec::new();
    };
    let at = Stamp {
        generation: state.run.generation,
        step: current.clone(),
    };
    let warn = |message: &str| {
        vec![Event::Warning {
            at: at.clone(),
            message: message.to_string(),
        }]
    };
    let Some(node) = ir.nodes.iter().find(|n| n.uid == current.0) else {
        return warn(INVALID_PROCESSING);
    };
    let step = &node.step;
    if usize::try_from(busy.total).ok() != Some(step.processing.len()) {
        return warn(INVALID_PROCESSING);
    }
    let Some(tick) = usize::try_from(busy.done)
        .ok()
        .and_then(|i| step.processing.get(i))
        .and_then(|v| ProcessingTick::from_value(v).ok())
    else {
        return warn(INVALID_PROCESSING);
    };
    let Some(next_done) = busy.done.checked_add(1) else {
        return warn(INVALID_PROCESSING);
    };
    // 達成の宣言は、状態を変える前に読む。読めないなら進行も進めない（一部だけ適用しない）。
    let finishing = next_done >= busy.total;
    let achieve_items = if finishing {
        match confirm::on_accept_mode(step) {
            confirm::OnAccept::AwaitConfirm => None,
            confirm::OnAccept::Achieve(items) => Some(items),
            confirm::OnAccept::Invalid => return warn(INVALID_SKIP),
        }
    } else {
        None
    };
    // 効果の根（フラグ）なので `root_of`。Step の状態は `status_of` で持ち主から読む。
    let root = crate::instance::root_of(&current);
    let mut events = apply_tick(
        &mut state.run,
        &root,
        &tick,
        addr.index,
        busy.total,
        &at,
        Cause::Input,
        &NoConditions,
    );
    if !finishing {
        if let Some(b) = state.run.busy.as_mut() {
            b.done = next_done;
        }
        return events;
    }
    state.run.busy = None;
    let accepted = crate::instance::status_of(ir, &state.run, &current) == StepStatus::Accepted;
    if let (Some(items), true) = (achieve_items, accepted) {
        events.extend(confirm::achieve_with(ir, state, node, &at, &items));
    }
    events
}
