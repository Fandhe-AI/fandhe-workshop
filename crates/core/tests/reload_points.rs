//! 33 地点での再読み込みの一致の試験（TASK-319 2/3、SAVE-3）。
//!
//! 3 Step 以上の教材の途中で再読み込みしたとき、同じ Step・同じ左の状態・同じ復習・同じログ・同じ入力欄の値で
//! 再開し（世代を除く）、そこから完走した最終の世界状態も、再読み込みなしの完走と一致することを確かめる。
//! 経路は本番と同じ保存→起動である（`RecordedSession::envelope_for_save` → `export_bytes` →
//! `boot_session`）。`Session::reload` を直接呼ぶだけの試験にはしない。web の結線は TASK-319 3/3 が持つ。
//!
//! # 出所とデータ
//!
//! 出所は PoC-6 の native 試験（`reload_at_every_move_of_round1_gives_same_state_and_same_final_world`）で、
//! v1.0.0 の教材の 1 周目（16 Step）を誘導つきの操作で進め、全地点で再読み込みした。ただし本物の教材は
//! `on_enter` の効果・モジュールの出口・遷移の `when: param`・章の完了の判定が core に未実装のため、
//! 今のエンジンでは完走できない。そのため 16 Step の形（入力方式・確認の種類・処理中の数）だけを写した、
//! 平らな 1 本の教材を `tests/data/reload_points_1/ir.json` に手書きで置く。未実装の語は使わない
//! （使っていないことは `データは未実装の語を使わず形が地点の数え方の前提どおり` が確かめる）。
//! 完走は `RunState.completed` がまだ立たないため、最後の Step（`final`）が達成になったことで判定する。
//!
//! # 地点の数え方
//!
//! 1 地点は「誘導つきの 1 手を打つ前の、完走前の状態」である。Step ごとに、入った直後が 1、
//! 処理中の進行の数だけ処理中の地点、確認が `none` 以外なら確認待ちが 1 つ。合計は 33
//! （入った直後 16・処理中 5・確認待ち 12）。PoC の assert は下限つきだが、本試験は数を固定する。
//! 各地点では保存の前に、地点ごとに違う下書きを記録する。未操作の履歴は復元されない（SAVE-28）ため、
//! 初期の地点も復元の対象にするためと、入力欄の値の一致を空同士の比較にしないためである。
//! 最後の Step の確認の操作は、その後に地点が無い（達成して完走する）ので地点に数えない。

mod common;

use common::tick::next_tick;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_boot::{boot_session, BootOutcome};
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_open::OpeningCaps;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::projection::Projection;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::{load_ir, Session};

const STEPS: usize = 16;
const START: &str = "s-1";
/// 完走の判定に使う最後の Step。
const LAST: &str = "main/st-16";
/// 1 回の完走に許す操作数の上限（無限ループで試験が止まらないための安全弁）。
const FINISH_LIMIT: usize = 200;

fn ir() -> Ir {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/reload_points_1/ir.json");
    let bytes = std::fs::read(path).expect("read ir");
    load_ir(&bytes).expect("load ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

/// 世代 1・先頭の Step に入った直後の状態。
fn initial() -> SessionState {
    let first = uid("main/st-1");
    let mut slice = Slice::default();
    slice.status.insert(first.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(first.clone()),
        ..RunState::default()
    };
    run.slices.insert(first.root(), slice);
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn opening(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

fn fresh(ir: &Ir) -> RecordedSession {
    RecordedSession::new(Session::from_state(ir.clone(), initial()), START)
}

fn status_of(state: &SessionState, step: &StepUid) -> StepStatus {
    state
        .run
        .slices
        .get(&step.root())
        .and_then(|s| s.status.get(step))
        .copied()
        .unwrap_or_default()
}

/// 完走か。`completed` が実装されるまでは、最後の Step の達成で判定する。
fn done(state: &SessionState) -> bool {
    state.run.completed || status_of(state, &uid(LAST)) == StepStatus::Achieved
}

/// 現在の状態に対する、誘導つきの次の 1 手。完走していれば `None`。
///
/// 宛先は状態から毎回作る（復元後は世代が +1 のため、保持した宛先は使えない）。
fn guided(state: &SessionState) -> Option<Op> {
    if done(state) {
        return None;
    }
    if state.run.busy.is_some() {
        return next_tick(state).map(Op::Tick);
    }
    let current = state.run.current.clone()?;
    let n: usize = current.0.strip_prefix("main/st-")?.parse().ok()?;
    let accepted = status_of(state, &current) == StepStatus::Accepted;
    let path = format!("notes/n{n}");
    let input = if accepted {
        confirm_input(n, &path)
    } else {
        accept_input(n, &path)
    };
    Some(Op::Input(input))
}

/// 入力の受付になる入力。
fn accept_input(n: usize, path: &str) -> Input {
    match n {
        2 => Input::Text(format!("メモ {n}")),
        4 | 5 | 9 => Input::Choice("opt-a".into()),
        3 | 8 => Input::Approval {
            decision: Decision::Approve,
            target: None,
        },
        6 => Input::Approval {
            decision: Decision::Revise,
            target: Some("expect".into()),
        },
        7 | 16 => Input::Ui {
            view: "v-files".into(),
            path: path.into(),
        },
        _ => Input::Command(format!("/make-note note-{n}")),
    }
}

/// 受付済みの Step を達成にする確認の入力。
fn confirm_input(n: usize, path: &str) -> Input {
    match n {
        2 | 7 | 15 => Input::Choice("opt-a".into()),
        6 => Input::Approval {
            decision: Decision::Approve,
            target: None,
        },
        _ => Input::Ui {
            view: "v-files".into(),
            path: path.into(),
        },
    }
}

/// 完走するまで誘導つきの手を打つ。上限を超えたら、止まった Step を添えて失敗させる。
fn finish(rs: &mut RecordedSession) {
    for _ in 0..FINISH_LIMIT {
        let Some(op) = guided(rs.session().state()) else {
            return;
        };
        rs.apply(op);
    }
    panic!(
        "did not finish within {FINISH_LIMIT} ops; stuck at {:?}",
        rs.session().state().run.current
    );
}

/// 再読み込みなしの完走で得た最終の世界状態。
fn baseline(ir: &Ir) -> fandhe_workshop_core::world::World {
    let mut rs = fresh(ir);
    finish(&mut rs);
    assert!(done(rs.session().state()), "baseline must complete");
    rs.session().state().run.world.clone()
}

/// 保存→起動で復元する。
fn reload_through_storage(ir: &Ir, rs: &RecordedSession, k: usize) -> RecordedSession {
    let env = rs.envelope_for_save().expect("envelope");
    let bytes = export_bytes(&env).expect("export");
    let out = boot_session(
        Some(&bytes),
        ir.clone(),
        &opening(ir),
        &OpeningCaps::current(),
        initial(),
        START,
    );
    let BootOutcome::Restored { session, notes } = out else {
        panic!("point {k}: must be restored");
    };
    assert!(notes.is_none(), "point {k}: no conversion notes");
    session
}

/// 再読み込みの前後で、世代を除いて一致することを確かめる。
fn assert_same_but_generation(k: usize, before: &SessionState, after: &SessionState) {
    assert_eq!(
        after.run.generation.0,
        before.run.generation.0 + 1,
        "point {k}: generation"
    );
    let mut run = after.run.clone();
    run.generation = before.run.generation;
    assert_eq!(run, before.run, "point {k}: run");
    assert_eq!(after.drafts, before.drafts, "point {k}: drafts");
    assert_eq!(
        after.checkpoints, before.checkpoints,
        "point {k}: checkpoints"
    );
    assert_eq!(after.next_checkpoint, before.next_checkpoint, "point {k}");
    assert_eq!(after.entry, before.entry, "point {k}: entry");
    assert_eq!(after.hints_opened, before.hints_opened, "point {k}: hints");
    assert_eq!(after.log.len(), before.log.len() + 1, "point {k}: log len");
    assert_eq!(
        after.log.get(..before.log.len()),
        Some(before.log.as_slice()),
        "point {k}: log prefix"
    );
    assert!(
        matches!(after.log.last(), Some(Event::Reloaded { .. })),
        "point {k}: log ends with Reloaded"
    );
    assert_eq!(
        Projection::from_events(&after.log).review(),
        Projection::from_events(&before.log).review(),
        "point {k}: review"
    );
}

/// 地点の種類ごとの数。
#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    points: usize,
    entered: usize,
    busy: usize,
    awaiting_confirm: usize,
}

#[test]
fn 三十三の地点で再読み込みの前後が世代を除いて一致し完走した世界状態が一致する() {
    let ir = ir();
    let base_world = baseline(&ir);
    let mut rs = fresh(&ir);
    let mut counts = Counts::default();
    loop {
        let state = rs.session().state().clone();
        if done(&state) {
            break;
        }
        let k = counts.points;
        counts.points += 1;
        if state.run.busy.is_some() {
            counts.busy += 1;
        } else if status_of(&state, &state.run.current.clone().expect("current"))
            == StepStatus::Accepted
        {
            counts.awaiting_confirm += 1;
        } else {
            counts.entered += 1;
        }

        // 地点ごとに違う空でない下書きを記録する（未操作の履歴にしないため）。
        rs.draft(format!("下書き {k}"));
        let before = rs.session().state().clone();
        let mut restored = reload_through_storage(&ir, &rs, k);
        assert_same_but_generation(k, &before, restored.session().state());

        // 復元した側を完走させ、再読み込みなしの完走と世界状態が一致する。
        finish(&mut restored);
        assert!(done(restored.session().state()), "point {k}: completes");
        assert_eq!(
            restored.session().state().run.world,
            base_world,
            "point {k}: final world"
        );

        // 元の側で誘導つきの 1 手を打って、次の地点へ進む。
        let op = guided(&state).expect("guided op");
        rs.apply(op);
    }
    println!("OBS {counts:?}");
    assert_eq!(
        counts,
        Counts {
            points: 33,
            entered: 16,
            busy: 5,
            awaiting_confirm: 12,
        }
    );
}

#[test]
fn データは未実装の語を使わず形が地点の数え方の前提どおり() {
    let ir = ir();
    assert_eq!(ir.nodes.len(), STEPS);
    let busy_total: usize = ir.nodes.iter().map(|n| n.step.processing.len()).sum();
    assert_eq!(busy_total, 5, "処理中の進行の合計");
    let finals: Vec<_> = ir
        .nodes
        .iter()
        .filter(|n| n.step.is_final)
        .map(|n| n.uid.as_str())
        .collect();
    assert_eq!(finals, [LAST], "final は最後の Step だけ");
    let awaiting = ir
        .nodes
        .iter()
        .filter(|n| {
            n.step
                .confirm
                .as_ref()
                .and_then(|c| c.get("kind"))
                .and_then(|k| k.as_str())
                != Some("none")
        })
        .count();
    assert_eq!(awaiting, 12, "確認が none でない Step の数");
    for n in &ir.nodes {
        let s = &n.step;
        assert!(s.on_enter.is_empty(), "{}: on_enter", n.uid);
        assert!(s.use_module.is_none(), "{}: use", n.uid);
        assert!(n.exit_to.is_none(), "{}: exit_to", n.uid);
        assert!(
            s.next.iter().all(|t| t.when.is_none()),
            "{}: 条件つきの next",
            n.uid
        );
        assert!(
            s.rules
                .iter()
                .all(|r| r.get("when").is_none_or(|w| w.is_null())),
            "{}: 規則の when",
            n.uid
        );
    }
}
