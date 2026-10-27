//! 3 周目の追加が既存を変えないことの受け入れ試験（TASK-245。MOD-12。記録は `docs/testing.md` の 5.335）。
//!
//! `module-reuse-rounds` の `step5/ch-round3.chapter.yaml` を `chapters/ch-round3/chapter.yaml` として足すだけで 3 周目の実体を
//! 書け、既存のものは何も変わらないことを、次の 5 点で確かめる。
//! (1) 既存のパックのファイルの SHA-256 が 1 件も変わらず、増えるのは追加の 1 件だけ。
//! (2) `validate` の診断が足す前と同じ（3 周目が増やす診断は 0 件）。
//! (3) IR の実体が 5 から 7 になり、既存の Step・実体・開始点が変わらない。
//! (4) 1 周目から 3 周目までが完走し、1・2 周目の切り身は 3 周目の前後で変わらない。
//! (5) 2 周目を終える前の 3 周目の開始は拒否されて状態を変えず、終えた後は開始できる。
//!
//! 診断の読み替え: spec の写しは `label` の必須化（PACK-37）と展開後の `do` の漏れの検査（PACK-52）より前の形で、生の診断が
//! 前後とも 14 件ある。この 2 種を除くと 0 件になるため、「診断 0 件」は「除いた残りが 0 件で、前後の一覧が等しい」と読み替える。
//! 入力列は `common/rounds.rs` の計画を使い、未実装の部品のため 2 か所を `GoTo` で飛ばす（5.258・5.260）。
//! core 側ではなく pack 側に置くのは、形式 B の教材のコンパイルが pack にしかなく、core から pack へは依存できないため。
//! 呼び出し元は CI の `make test` だけである。

mod common;
#[path = "common/rounds.rs"]
mod rounds;
#[path = "common/sha256.rs"]
mod sha256;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use common::{
    copy_dir, files_of, testdata, without_answer_label_required, without_expanded_do_leak, Tmp,
};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::RejectCategory;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::event::{ControlKind, Event, RefuseReason};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::instance::root_of;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::rejection_text::{resolve_rejection, RejectionSource};
use fandhe_workshop_core::start_gate::{check_start, StartRefusal};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use rounds::{
    begin_round2_plan, begin_round3_plan, goto_count, round1_plan, round2_plan,
    round2_plan_until_create_tree, round3_plan, round_roots, run_plan, step_in, steps,
};

const PACK: &str = "module-reuse-rounds/test-data/pack";
const STEP5: &str = "module-reuse-rounds/test-data/step5/ch-round3.chapter.yaml";
const ADDED: &str = "chapters/ch-round3/chapter.yaml";
const TEXT_ID: &str = "rj-start-needs-previous-round";

/// 3 周目の章を足した写し。`testdata` は読むだけで書き換えない。
fn round3_pack() -> Tmp {
    let t = Tmp::new("round3");
    copy_dir(&testdata(PACK), &t.0);
    t.put(ADDED, &fs::read(testdata(STEP5)).expect("read step5"));
    t
}

/// 相対パス（`/` 区切り）から SHA-256 の 16 進への対応。
fn manifest(dir: &Path) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    files_of(dir, dir, &mut files);
    files
        .into_iter()
        .map(|rel| {
            let bytes = fs::read(dir.join(&rel)).expect("read");
            let key = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            (key, sha256::hex(&sha256::sha256(&bytes)))
        })
        .collect()
}

fn ir_of(dir: &Path) -> Ir {
    load_ir(&compile_dir(dir).expect("compile")).expect("load_ir")
}

/// `log` を除いた状態の SHA-256。拒否の 1 件が `log` に足されるので比べる対象から外す。
fn state_hash(state: &SessionState) -> String {
    let mut copy = state.clone();
    copy.log.clear();
    let json = serde_json::to_vec(&copy).expect("serialize");
    sha256::hex(&sha256::sha256(&json))
}

/// 根の切り身の正準 JSON の SHA-256（16 進）。
fn slice_hash(s: &Session, root: &Root) -> String {
    let json = serde_json::to_string(s.slice(root).expect("slice")).expect("slice json");
    sha256::hex(&sha256::sha256(json.as_bytes()))
}

fn slice_hashes(s: &Session, roots: &[Root]) -> Vec<String> {
    roots.iter().map(|r| slice_hash(s, r)).collect()
}

fn status_of(s: &Session, ir: &Ir, roots: &[Root]) -> Vec<(StepUid, StepStatus)> {
    steps(ir, roots)
        .into_iter()
        .map(|u| {
            let st = s.step_status(&u);
            (u, st)
        })
        .collect()
}

fn flags_of(s: &Session, ir: &Ir, roots: &[Root], id: &str) -> Vec<String> {
    let root = root_of(&step_in(ir, roots, id));
    s.slice(&root)
        .map(|sl| sl.flags.iter().cloned().collect())
        .unwrap_or_default()
}

/// 1 周目を終えた状態（2 周目の開始条件を満たす）。
fn after_round1(ir: &Ir) -> Session {
    let mut s = Session::new(ir.clone());
    run_plan(&mut s, &round1_plan(ir));
    s
}

/// 2 周目まで終えた状態と、1 周目から 2 周目までのイベント。
fn after_round2(ir: &Ir) -> (Session, Vec<Event>) {
    let mut s = after_round1(ir);
    let mut ev = run_plan(&mut s, &begin_round2_plan());
    ev.extend(run_plan(&mut s, &round2_plan(ir)));
    (s, ev)
}

fn refused_once(events: &[Event]) -> bool {
    matches!(
        events,
        [Event::Refused {
            op: ControlKind::Begin,
            reason: RefuseReason::StartUnmet,
            rejection: Some(id),
            ..
        }] if id == TEXT_ID
    )
}

#[test]
fn mod12_existing_files_keep_sha256_after_adding_round3() {
    let src = testdata(PACK);
    let before = manifest(&src);
    assert_eq!(before.len(), 44, "元のパックのファイル数（空振りの防止）");
    let t = round3_pack();
    let after = manifest(&t.0);
    assert_eq!(after.len(), 45);
    for (k, v) in &before {
        assert_eq!(after.get(k), Some(v), "{k} changed or vanished");
    }
    let added: Vec<&String> = after.keys().filter(|k| !before.contains_key(*k)).collect();
    assert_eq!(added, [ADDED]);
    let step5 = fs::read(testdata(STEP5)).expect("read step5");
    assert_eq!(
        after[ADDED],
        sha256::hex(&sha256::sha256(&step5)),
        "追加の 1 件は step5 のファイルそのまま"
    );
    // compile・validate・実行が testdata を書き換えないこと。
    let _ = ir_of(&t.0);
    let _ = validate_dir(&t.0);
    assert_eq!(manifest(&src), before);
}

#[test]
fn mod12_validate_adds_no_diagnostics() {
    let t = round3_pack();
    let d_before = validate_dir(&testdata(PACK));
    let d_after = validate_dir(&t.0);
    assert_eq!(d_before, d_after, "3 周目の追加で診断の一覧が変わった");
    let (rest_b, label_b) = without_answer_label_required(d_before);
    let (rest_b, leak_b) = without_expanded_do_leak(rest_b);
    let (rest_a, label_a) = without_answer_label_required(d_after);
    let (rest_a, leak_a) = without_expanded_do_leak(rest_a);
    assert!(rest_b.is_empty(), "{rest_b:?}");
    assert!(rest_a.is_empty(), "{rest_a:?}");
    assert_eq!((label_b, leak_b), (label_a, leak_a));
    // 除外が空振りしていない（2026-10-27 の実測は 4 件と 10 件）。
    assert!(label_a > 0 && leak_a > 0);
}

#[test]
fn mod12_instances_grow_from_five_to_seven_without_touching_existing() {
    let before = ir_of(&testdata(PACK));
    let t = round3_pack();
    let after = ir_of(&t.0);
    assert_eq!(before.instances.len(), 5);
    assert_eq!(after.instances.len(), 7);
    assert_eq!(after.instances[..5], before.instances[..]);
    let added: Vec<(&str, Option<&str>)> = after.instances[5..]
        .iter()
        .map(|i| (i.id.as_str(), i.caller.as_deref()))
        .collect();
    assert_eq!(
        added,
        [
            ("idea-cycle@round3", Some("main/r3-idea")),
            ("issue-to-merge@round3", Some("main/r3-impl")),
        ]
    );
    assert_eq!(before.nodes.len(), 36);
    assert_eq!(after.nodes.len(), 54);
    for n in &before.nodes {
        let m = after.nodes.iter().find(|m| m.uid == n.uid).expect("node");
        assert_eq!(m, n, "{}", n.uid);
    }
    assert_eq!(before.starts.len(), 19);
    assert_eq!(after.starts.len(), 28);
    for s in &before.starts {
        let m = after.starts.iter().find(|m| m.id == s.id).expect("start");
        assert_eq!(m, s, "{}", s.id);
    }
    let s3 = after.starts.iter().find(|s| s.id == "s-r3").expect("s-r3");
    assert!(s3.requires.is_some() && s3.unmet.is_some());
    assert_eq!(s3.node, "main/r3-idea");
}

#[test]
fn mod12_three_rounds_run_to_the_end() {
    let t = round3_pack();
    let ir = ir_of(&t.0);
    let (mut s, _) = after_round2(&ir);
    let begin = run_plan(&mut s, &begin_round3_plan());
    let begun = begin
        .iter()
        .filter(|e| matches!(e, Event::Begun { .. }))
        .count();
    assert_eq!(begun, 1, "{begin:?}");
    assert!(!begin.iter().any(|e| matches!(e, Event::Refused { .. })));
    let ev = run_plan(&mut s, &round3_plan(&ir));
    let (r3, _) = round_roots(&ir, "s-r3");
    assert_eq!(
        s.state().run.current.as_ref().map(|u| u.0.as_str()),
        Some("main/r3-impl")
    );
    for c in ["main/r3-idea", "main/r3-impl"] {
        assert_eq!(s.step_status(&StepUid(c.into())), StepStatus::Achieved);
    }
    for id in ["st-2-8", "st-7-4"] {
        assert_eq!(
            s.step_status(&step_in(&ir, &r3, id)),
            StepStatus::Achieved,
            "{id}"
        );
    }
    // 2 周目と同じ形（`stopped` は無い）。
    assert_eq!(flags_of(&s, &ir, &r3, "st-7-1"), vec!["precheck_ok"]);
    assert_eq!(flags_of(&s, &ir, &r3, "st-2-4"), vec!["revised"]);
    let scopes: Vec<&str> = r3.iter().map(|r| r.0.as_str()).collect();
    let mut seen = 0;
    for e in &ev {
        if let Event::Flag { scope, .. } = e {
            seen += 1;
            assert!(scopes.contains(&scope.as_str()), "flag event in {scope}");
        }
    }
    assert!(seen > 0, "round 3 raised no flag events");
}

#[test]
fn mod12_round1_and_round2_slices_are_unchanged_by_round3() {
    let t = round3_pack();
    let ir = ir_of(&t.0);
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let (mut s, _) = after_round2(&ir);
    let h1 = slice_hashes(&s, &r1);
    let h2 = slice_hashes(&s, &r2);
    let st1 = status_of(&s, &ir, &r1);
    let st2 = status_of(&s, &ir, &r2);
    let world = s.state().run.world.clone();
    assert!(!world.is_empty());
    run_plan(&mut s, &begin_round3_plan());
    run_plan(&mut s, &round3_plan(&ir));
    assert_eq!(slice_hashes(&s, &r1), h1);
    assert_eq!(slice_hashes(&s, &r2), h2);
    assert_eq!(status_of(&s, &ir, &r1), st1);
    assert_eq!(status_of(&s, &ir, &r2), st2);
    for (k, v) in &world {
        assert_eq!(s.state().run.world.get(k), Some(v), "{k} changed");
    }
}

#[test]
fn mod12_round1_and_round2_run_the_same_on_the_extended_ir() {
    let base = ir_of(&testdata(PACK));
    let t = round3_pack();
    let ext = ir_of(&t.0);
    let (s_base, ev_base) = after_round2(&base);
    let (s_ext, ev_ext) = after_round2(&ext);
    assert_eq!(ev_base, ev_ext);
    assert_eq!(
        serde_json::to_string(s_base.state()).expect("json"),
        serde_json::to_string(s_ext.state()).expect("json")
    );
}

#[test]
fn mod12_round3_start_is_refused_until_round2_ends() {
    let t = round3_pack();
    let ir = ir_of(&t.0);
    let begin = || Op::Control(Control::Begin("s-r3".into()));

    // (i) 新しい Session。
    let mut s = Session::new(ir.clone());
    let before = s.state().clone();
    let ev = s.apply(begin()).to_vec();
    assert!(refused_once(&ev), "{ev:?}");
    assert!(s.state().run.current.is_none());
    assert!(!ev.iter().any(|e| matches!(
        e,
        Event::Begun { .. } | Event::Entered { .. } | Event::Skipped { .. }
    )));
    assert_eq!(state_hash(s.state()), state_hash(&before));

    // (ii) 1 周目だけを終えた状態、(iii) 2 周目の途中。
    let mut mid2 = after_round1(&ir);
    run_plan(&mut mid2, &begin_round2_plan());
    run_plan(&mut mid2, &round2_plan_until_create_tree(&ir));
    for mut s in [after_round1(&ir), mid2] {
        let before = s.state().clone();
        let ev = s.apply(begin()).to_vec();
        assert!(refused_once(&ev), "{ev:?}");
        let after = s.state();
        assert_eq!(state_hash(after), state_hash(&before));
        assert_eq!(after.run, before.run);
        assert_eq!(after.current_draft(), before.current_draft());
        assert_eq!(after.log.len(), before.log.len() + 1);
    }

    // 拒否の文面は順序違いで、次の行動が空でない。
    let fresh = Session::new(ir.clone());
    match check_start(&ir, &fresh.state().run, "s-r3") {
        Err(StartRefusal::Unmet { start, text: Ok(r) }) => {
            assert_eq!(start, "s-r3");
            assert_eq!(r.text, TEXT_ID);
        }
        other => panic!("{other:?}"),
    }
    let start = ir.starts.iter().find(|s| s.id == "s-r3").expect("s-r3");
    let node = ir
        .nodes
        .iter()
        .find(|n| n.uid == start.node)
        .expect("start node");
    let r = resolve_rejection(&ir, &node.step, RejectionSource::Rule(TEXT_ID)).expect("resolve");
    assert_eq!(r.category, RejectCategory::WrongOrder);
    assert!(!r.next_action.as_str().trim().is_empty());

    // (iv) 対照: 2 周目を終えた後は開始できる。
    let (mut s, _) = after_round2(&ir);
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let (h1, h2) = (slice_hashes(&s, &r1), slice_hashes(&s, &r2));
    let generation = s.state().run.generation;
    let ev = s.apply(begin()).to_vec();
    assert!(
        ev.iter().any(|e| matches!(e, Event::Begun { .. })),
        "{ev:?}"
    );
    assert!(!ev.iter().any(|e| matches!(e, Event::Refused { .. })));
    assert_eq!(Some(s.state().run.generation), generation.next());
    assert_eq!(slice_hashes(&s, &r1), h1);
    assert_eq!(slice_hashes(&s, &r2), h2);
}

#[test]
fn mod12_round2_plan_matches_the_shared_body() {
    let t = round3_pack();
    let ir = ir_of(&t.0);
    let p2 = round2_plan(&ir);
    let p3 = round3_plan(&ir);
    assert_eq!(goto_count(&p2), 2);
    assert_eq!(goto_count(&p3), 2);
    assert_eq!(p2.len(), p3.len(), "同じ構成の計画");
    // 題材と引数だけが違う（3 周目の計画が 2 周目の写しでないことの確認）。
    assert_ne!(format!("{p2:?}"), format!("{p3:?}"));
}
