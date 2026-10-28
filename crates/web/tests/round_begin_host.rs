//! 画面の「次の章へ進む」が 2 周目を新しいルートで始めることの試験（MOD-5・MOD-8・TASK-254 1/2）。
//!
//! 1 周目を終えた `module-reuse-rounds` の IR で、`next-chapter` の押下（`EngineHost::decode_action` → `update`）だけで
//! `Control::Begin` が送られ、2 周目の実体の根が新しく始まり、1 周目の切り身と世界状態が残ることを確かめる。
//! 2 周分の操作列は pack の試験の共有部品（`crates/pack/tests/common/rounds.rs`）を `#[path]` で借りる
//! （二重定義をして食い違うのを避けるため。部品は core と contract だけに依存する）。
//! ブラウザの通しは TASK-254 2/2（`tools/browser/round-handoff.test.mjs`）が受け持つ。本ファイルの `emit_round_handoff_for_browser`
//! （`#[ignore]`）がその入力を書き出す。世界状態の SHA-256 の一致と閉じた根の保持は、ここでも native で確かめる（MOD-5・MOD-8）。
//! 試験では `unwrap` を使ってよい。

#[path = "../../pack/tests/common/rounds.rs"]
mod rounds;
#[path = "../../pack/tests/common/sha256.rs"]
mod sha256;

use std::path::PathBuf;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::instance::owner_of;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::history_export::ExportGate;
use fandhe_workshop_web::host::{EngineHost, HostAction, ImportOutcome};
use fandhe_workshop_web::next_chapter::{next_chapter, ACTION_NEXT_CHAPTER};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::shell::NEXT_CHAPTER_TESTID;
use rounds::{round1_plan, round2_plan, round_roots, run_plan, steps, Drive, PlanStep};

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

/// 配信する設定の起動と同じ作り方のホスト（開始点の節点に入った直後。`persist` の起動と同じ）。
fn browser_host(ir: &Ir) -> HostDrive {
    let node = ir.starts.first().expect("starts").node.clone();
    let session = Session::entered_at(ir.clone(), &node);
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

/// 配信する設定の起動と同じ作り方のホストで 1 周目を終えたもの。
fn finished_browser_round1() -> (Ir, HostDrive) {
    let ir = ir();
    let mut h = browser_host(&ir);
    run_plan(&mut h, &round1_plan(&ir));
    (ir, h)
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

/// 世界状態の SHA-256（直列化した JSON の値。キーは `BTreeMap` で整列済み）。
fn world_hash(h: &HostDrive) -> String {
    let text = serde_json::to_string(&h.state().run.world).unwrap();
    sha256::hex(&sha256::sha256(text.as_bytes()))
}

/// 実体の引数から導いた成果物の経路（`issues/root-N`・`issues/child-M`・`prs/<pr>-a`）。経路を直書きしない。
fn artifact_paths(ir: &Ir, instance: &str) -> [String; 3] {
    let i = ir
        .instances
        .iter()
        .find(|i| i.id == instance)
        .unwrap_or_else(|| panic!("instance {instance}"));
    let arg = |k: &str| i.args.get(k).unwrap_or_else(|| panic!("arg {k}")).clone();
    [
        format!("issues/root-{}", arg("root")),
        format!("issues/child-{}", arg("child")),
        format!("prs/{}-a", arg("pr")),
    ]
}

fn state_attr(h: &HostDrive, path: &str) -> Option<String> {
    h.state()
        .run
        .world
        .get(path)
        .and_then(|e| e.attrs.get("state").cloned())
}

#[test]
fn 押下の前後で世界状態のハッシュが一致し_閉じた根と子が閉じたまま残る() {
    let (ir, mut h) = finished_round1();
    let [root, child, _pr] = artifact_paths(&ir, "issue-to-merge@round1");
    assert!(!h.state().run.world.is_empty(), "1 周目の終わりは空でない");
    let before = world_hash(&h);
    for p in [&root, &child] {
        assert_eq!(state_attr(&h, p).as_deref(), Some("closed"), "{p} 押下の前");
    }
    press(&mut h, "s-r2");
    assert_eq!(before, world_hash(&h), "世界状態の SHA-256 が一致する");
    for p in [&root, &child] {
        assert_eq!(state_attr(&h, p).as_deref(), Some("closed"), "{p} 押下の後");
    }
}

#[test]
fn 押下の後の書き出しは_ready_で_新しいホストで同じ状態に開く() {
    let (ir, mut h) = finished_browser_round1();
    press(&mut h, "s-r2");
    let bytes = match h.0.export_live() {
        ExportGate::Ready(b) => b,
        other => panic!("Ready のはず: {other:?}"),
    };
    // 開く側も配信する設定の起動と同じ初期状態（開始点の節点に入った直後）。
    let mut fresh = browser_host(&ir);
    let outcome = fresh.0.open_history(&bytes);
    assert!(!matches!(outcome, ImportOutcome::ViewOnly(_)));
    assert_eq!(fresh.state().run.current, h.state().run.current);
    assert_eq!(fresh.state().run.slices, h.state().run.slices);
    assert_eq!(fresh.state().run.world, h.state().run.world);
}

fn plan_json(plan: &[PlanStep]) -> serde_json::Value {
    plan.iter()
        .map(|s| match s {
            PlanStep::Op(op) => serde_json::json!({ "op": op }),
            PlanStep::Tick => serde_json::json!({ "tick": true }),
            PlanStep::ExpectCurrentSuffix(x) => serde_json::json!({ "expect_suffix": x }),
        })
        .collect()
}

/// ブラウザ試験（`tools/browser/round-handoff.test.mjs`）の入力を書き出す。IR・履歴・2 周目の計画は同じ `compile_dir` の
/// 出力から作る（`pack_id` と `content_hash` が必ず合う）。出力先は `FANDHE_ROUND_HANDOFF_OUT`（未設定は panic。fail-closed）。
/// ファイル名は固定で、教材の文字列からパスを作らない。押下はここに含めない（ブラウザが行う）。
#[test]
#[ignore = "ブラウザ試験の入力を書き出す。scripts/test-browser.sh だけが --ignored で呼ぶ"]
fn emit_round_handoff_for_browser() {
    let out = PathBuf::from(
        std::env::var("FANDHE_ROUND_HANDOFF_OUT").expect("FANDHE_ROUND_HANDOFF_OUT must be set"),
    );
    std::fs::create_dir_all(&out).unwrap();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/module-reuse-rounds/test-data/pack");
    let ir_bytes = compile_dir(&dir).expect("compile");
    let ir = load_ir(&ir_bytes).expect("load_ir");
    let ready = |h: &HostDrive| match h.0.export_live() {
        ExportGate::Ready(b) => b,
        other => panic!("Ready のはず: {other:?}"),
    };

    // 配信する設定の起動と同じ作り方（開始点の節点に入った直後）にする。開き直しは同じ初期状態から ops を適用し直すため、
    // 既定の空の状態から作った履歴はブラウザで `replay_mismatch` になる。1 周目の計画は先頭の `Begin` から流す（記録の先頭の op になる）。
    let browser_round1 = |upto: Option<usize>| {
        let mut h = browser_host(&ir);
        let plan: Vec<PlanStep> = round1_plan(&ir);
        let n = upto.unwrap_or(plan.len());
        run_plan(&mut h, &plan[..n]);
        h
    };
    let done = browser_round1(None);
    let [r1_root, r1_child, r1_pr] = artifact_paths(&ir, "issue-to-merge@round1");
    let r2_paths = artifact_paths(&ir, "issue-to-merge@round2");
    let end_bytes = ready(&done);
    let ops_len = serde_json::from_slice::<serde_json::Value>(&end_bytes).unwrap()["ops"]
        .as_array()
        .unwrap()
        .len();

    let half = browser_round1(Some(6));

    let (r1_roots, _) = round_roots(&ir, "s-r1");
    let (r2_roots, _) = round_roots(&ir, "s-r2");
    let expect = serde_json::json!({
        "world": done.state().run.world,
        "generation": done.state().run.generation.0,
        "r1_roots": r1_roots.iter().map(|r| r.0.clone()).collect::<Vec<_>>(),
        "r2_roots": r2_roots.iter().map(|r| r.0.clone()).collect::<Vec<_>>(),
        "closed_paths": [r1_root, r1_child],
        "r1_paths": [r1_root, r1_child, r1_pr],
        "r2_paths": r2_paths,
        "start": "s-r2",
        "ops_len": ops_len,
    });
    let write = |name: &str, bytes: &[u8]| std::fs::write(out.join(name), bytes).unwrap();
    write("ir.json", &ir_bytes);
    write("round1-end.history.json", &end_bytes);
    write("round1-half.history.json", &ready(&half));
    // 試験の設定の画面（`loadIr` で教材を入れ替えた初期状態は履歴の初期状態と違い、履歴を開けない）が 1 周目を送り直すための計画。
    write(
        "round1-plan.json",
        &serde_json::to_vec(&plan_json(&round1_plan(&ir))).unwrap(),
    );
    write(
        "round2-plan.json",
        &serde_json::to_vec(&plan_json(&round2_plan(&ir))).unwrap(),
    );
    write("expect.json", &serde_json::to_vec(&expect).unwrap());
}
