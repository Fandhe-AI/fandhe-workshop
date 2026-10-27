//! 画面の「次の章へ進む」が 2 周目を新しいルートで始めることの試験（MOD-5・MOD-8・TASK-254 1/2）。
//!
//! 1 周目を終えた `module-reuse-rounds` の IR で、`next-chapter` の押下（`EngineHost::decode_action` → `update`）だけで
//! `Control::Begin` が送られ、2 周目の実体の根が新しく始まり、1 周目の切り身と世界状態が残ることを確かめる。
//! 2 周分の操作列は pack の試験の共有部品（`crates/pack/tests/common/rounds.rs`）を `#[path]` で借りる
//! （二重定義をして食い違うのを避けるため。部品は core と contract だけに依存する）。
//! 世界状態のハッシュの一致の正式な確認とブラウザの通しは TASK-254 2/2 が受け持つ。ここでは等値だけを見る。
//! 試験では `unwrap` を使ってよい。

#[path = "../../pack/tests/common/rounds.rs"]
mod rounds;

use std::path::PathBuf;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::instance::owner_of;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction, ImportOutcome};
use fandhe_workshop_web::next_chapter::{next_chapter, ACTION_NEXT_CHAPTER};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::shell::NEXT_CHAPTER_TESTID;
use rounds::{round1_plan, round_roots, run_plan, steps, Drive, PlanStep};

struct HostDrive(EngineHost);

impl Drive for HostDrive {
    fn apply(&mut self, op: Op) -> Vec<Event> {
        let before = self.0.model().session().unwrap().log().len();
        self.0.update(HostAction::Engine(op));
        self.0.model().session().unwrap().log()[before..].to_vec()
    }
    fn state(&self) -> &SessionState {
        self.0.model().session().unwrap().state()
    }
}

fn ir() -> Ir {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/module-reuse-rounds/test-data/pack");
    load_ir(&compile_dir(&dir).expect("compile")).expect("load_ir")
}

fn fresh_host(ir: &Ir) -> HostDrive {
    let session = Session::from_state(ir.clone(), SessionState::default());
    HostDrive(EngineHost::with_session(session, standard_registry()).unwrap())
}

fn button_count(h: &EngineHost) -> usize {
    format!("{:?}", h.view())
        .matches(NEXT_CHAPTER_TESTID)
        .count()
}

fn press(h: &mut HostDrive, payload: &str) {
    h.0.update(EngineHost::decode_action(ACTION_NEXT_CHAPTER, payload).unwrap());
}

fn finished_round1() -> (Ir, HostDrive) {
    let ir = ir();
    let mut h = fresh_host(&ir);
    let plan = round1_plan(&ir);
    run_plan(&mut h, &plan);
    (ir, h)
}

#[test]
fn 一周目の終わりに次の周回の入口がちょうど_1_個出る() {
    let (ir, h) = finished_round1();
    let offer = next_chapter(&ir, h.state()).expect("1 周目の終わりに提示が出る");
    assert_eq!(
        offer.start_id, "s-r2",
        "effects つきの s-r2-fresh でなく続けて始める入口"
    );
    assert_eq!(button_count(&h.0), 1);
}

#[test]
fn 押下で_begin_が送られ_2_周目が新しい根で始まり_1_周目が残る() {
    let (ir, mut h) = finished_round1();
    let before = h.state().clone();
    let gen = before.run.generation.0;
    let log_len = h.0.model().session().unwrap().log().len();
    press(&mut h, "s-r2");
    let events = h.0.model().session().unwrap().log()[log_len..].to_vec();
    assert!(
        matches!(events.first(), Some(Event::Begun { start, new_generation, .. })
            if start == "s-r2" && new_generation.0 == gen + 1),
        "先頭は Begun: {events:?}"
    );
    assert!(
        !events.iter().any(|e| matches!(e, Event::Moved { .. })),
        "GoTo ではない"
    );
    let (r2, _node) = round_roots(&ir, "s-r2");
    let after = h.state();
    let cur = after.run.current.clone().unwrap();
    assert!(r2.contains(&cur.root()), "2 周目の実体の根の Step に入る");
    assert_eq!(
        after.run.slices[&owner_of(&ir, &cur)].status[&cur],
        StepStatus::Entered
    );
    // 1 周目の切り身と世界状態は変わらない。
    let (r1, _) = round_roots(&ir, "s-r1");
    for u in steps(&ir, &r1) {
        let owner = owner_of(&ir, &u);
        assert_eq!(
            before.run.slices.get(&owner).map(|s| s.status.get(&u)),
            after.run.slices.get(&owner).map(|s| s.status.get(&u)),
            "1 周目の切り身が保たれる"
        );
    }
    assert_eq!(before.run.world, after.run.world, "世界状態が保たれる");
    assert_eq!(button_count(&h.0), 0, "押下の後は提示が消える");
}

#[test]
fn 保存した履歴に_begin_が残り_新しいホストで同じ位置に復元できる() {
    let (ir, mut h) = finished_round1();
    press(&mut h, "s-r2");
    let text = h.0.history_to_save().expect("保存する履歴がある");
    assert_eq!(
        text.matches("\"begin\":\"s-r2\"").count(),
        1,
        "begin が 1 件"
    );
    let mut fresh = fresh_host(&ir);
    let outcome = fresh.0.open_history(text.as_bytes());
    assert!(!matches!(outcome, ImportOutcome::ViewOnly(_)));
    assert_eq!(fresh.state().run.current, h.state().run.current);
    assert_eq!(fresh.state().run.slices, h.state().run.slices);
}

#[test]
fn 一周目の途中と提示と違う開始点の押下は何も変えない() {
    let ir = ir();
    let mut h = fresh_host(&ir);
    let half: Vec<PlanStep> = round1_plan(&ir).into_iter().take(6).collect();
    run_plan(&mut h, &half);
    let before = h.state().clone();
    press(&mut h, "s-r2");
    assert_eq!(before, *h.state(), "途中では押せない");
    let (_ir, mut done) = finished_round1();
    let before = done.state().clone();
    press(&mut done, "s-r2-fresh");
    assert_eq!(before, *done.state(), "effects つきの開始点は押せない");
}
