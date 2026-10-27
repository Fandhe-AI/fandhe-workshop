//! 2 周の教材での根ごとの下書きと切り替えでの入れ替え（TASK-236 1/2。MOD-4。`proc3_iii_*`）の試験。
//!
//! `module-reuse-rounds` を `compile_dir` した IR で、1 周目を流した後に下書きを置き、2 周目を開始し、
//! `Control::GoTo` で周回を行き来しても、表に出る下書き（`current_draft`）がその根のものに入れ替わることを確かめる。
//! 1 周目の下書きは `st-3-2` に移して置く（計画の最後は `main` の Step で、途中の `st-6-1` を通る保証も計画に無いための代用）。
//! 送信で送った根だけが空になることは末尾の 2 件（TASK-236 2/2）が持つ。下書きの値は利用者の入力なので、比較だけを行う。

mod common;
#[path = "common/rounds.rs"]
mod rounds;

use common::testdata;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use rounds::{begin_round2_plan, round1_plan, round_roots, run_plan, step_in};

fn ir() -> Ir {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

fn goto(rs: &mut RecordedSession, uid: &StepUid) {
    rs.apply(Op::Control(Control::GoTo(uid.clone())));
    assert_eq!(
        rs.session().state().run.current.as_ref(),
        Some(uid),
        "moved"
    );
}

struct Setup {
    ir: Ir,
    rs: RecordedSession,
    r1_step: StepUid,
    r1_root: Root,
    r2_roots: Vec<Root>,
    r1_roots: Vec<Root>,
}

/// 1 周目を流して下書きを置き、2 周目を開始した直後の状態。
fn setup() -> Setup {
    let ir = ir();
    let mut rs = RecordedSession::new(Session::new(ir.clone()), RecordedSession::first_start(&ir));
    run_plan(&mut rs, &round1_plan(&ir));
    let (r1_roots, _) = round_roots(&ir, "s-r1");
    // 計画の最後は `main` の Step で、1 周目の実体の Step へは戻れる位置に移してから下書きを置く。
    let r1_step = step_in(&ir, &r1_roots, "st-3-2");
    goto(&mut rs, &r1_step);
    let r1_root = r1_step.root();
    rs.draft("下書き1".to_string());
    let before = rs.session().state().drafts.clone();
    run_plan(&mut rs, &begin_round2_plan());
    // 開始は下書きを書き換えない。
    assert_eq!(rs.session().state().drafts, before);
    let (r2_roots, _) = round_roots(&ir, "s-r2");
    Setup {
        ir,
        rs,
        r1_step,
        r1_root,
        r2_roots,
        r1_roots,
    }
}

#[test]
fn proc3_iii_round2_starts_without_a_draft() {
    let s = setup();
    assert_eq!(s.rs.current_draft(), None);
    let d = &s.rs.session().state().drafts;
    assert!(s.r2_roots.iter().all(|r| !d.contains_key(r)));
    assert_eq!(d.get(&s.r1_root).map(String::as_str), Some("下書き1"));
}

#[test]
fn proc3_iii_round2_draft_does_not_leak_into_round1() {
    let mut s = setup();
    let before = s.rs.session().state().drafts.clone();
    s.rs.draft("下書き2".to_string());
    assert_eq!(s.rs.current_draft(), Some("下書き2"));
    let d = &s.rs.session().state().drafts;
    assert_eq!(d.get(&s.r1_root).map(String::as_str), Some("下書き1"));
    assert_eq!(d.len(), before.len() + 1);
}

#[test]
fn proc3_iii_go_to_swaps_drafts_between_rounds() {
    let mut s = setup();
    let r2_step = step_in(&s.ir, &s.r2_roots, "st-2-1");
    goto(&mut s.rs, &r2_step);
    s.rs.draft("下書き2".to_string());
    for _ in 0..2 {
        goto(&mut s.rs, &s.r1_step.clone());
        assert_eq!(s.rs.current_draft(), Some("下書き1"));
        goto(&mut s.rs, &r2_step);
        assert_eq!(s.rs.current_draft(), Some("下書き2"));
    }
    // 同じ周回でも別の実体の Step は別の欄になる。
    let other = s
        .r1_roots
        .iter()
        .filter(|r| **r != s.r1_root)
        .find_map(|r| {
            fandhe_workshop_core::instance::steps_of(&s.ir, r)
                .into_iter()
                .next()
        })
        .expect("other instance step");
    // 先頭は呼び出しの Step で、移動すると呼ばれた実体の Step に着く。
    s.rs.apply(Op::Control(Control::GoTo(other)));
    let landed = s.rs.session().state().run.current.clone().expect("current");
    assert_ne!(landed.root(), s.r1_root);
    assert_eq!(s.rs.current_draft(), None);
}

#[test]
fn proc3_iii_replay_restores_current_draft() {
    let mut s = setup();
    let r2_step = step_in(&s.ir, &s.r2_roots, "st-2-1");
    goto(&mut s.rs, &r2_step);
    s.rs.draft("下書き2".to_string());
    goto(&mut s.rs, &s.r1_step.clone());
    let env = s.rs.envelope_for_save().expect("envelope");
    let replayed = replay(&s.ir, SessionState::default(), &env.ops).expect("replay");
    assert_eq!(replayed.drafts, s.rs.session().state().drafts);
    let restored = Session::from_state(s.ir.clone(), replayed);
    assert_eq!(restored.current_draft(), Some("下書き1"));
    assert_eq!(restored.current_draft(), s.rs.current_draft());
}

/// 送信の接頭辞（IR の `pack.profile` から。履歴の適用し直しと同じ読み方）。
fn prefix(ir: &Ir) -> String {
    fandhe_workshop_contract::profile::read_profile(ir.pack.profile.as_ref())
        .expect("profile")
        .command_prefix
}

// この教材の送信は実体をまたぐ遷移を起こさない。根をまたぐ送信は core の `submit_drafts` の合成 IR が受け持つ。

#[test]
fn proc3_iii_submit_clears_only_the_sent_round_root() {
    let mut s = setup();
    let r2_step = step_in(&s.ir, &s.r2_roots, "st-2-1");
    goto(&mut s.rs, &r2_step);
    s.rs.draft("下書き2".to_string());
    let before = s.rs.session().state().drafts.len();
    let p = prefix(&s.ir);
    s.rs.submit(&p, "/new-idea bbs-search".to_string());
    let d = &s.rs.session().state().drafts;
    assert!(!d.contains_key(&r2_step.root()), "sent root is cleared");
    assert_eq!(d.get(&s.r1_root).map(String::as_str), Some("下書き1"));
    assert_eq!(d.len(), before - 1);
}

#[test]
fn proc3_iii_round1_draft_survives_round2_submits_and_replay() {
    let mut s = setup();
    let r2_step = step_in(&s.ir, &s.r2_roots, "st-2-1");
    goto(&mut s.rs, &r2_step);
    s.rs.draft("下書き2".to_string());
    let p = prefix(&s.ir);
    s.rs.submit(&p, "/new-idea bbs-search".to_string());
    goto(&mut s.rs, &s.r1_step.clone());
    assert_eq!(s.rs.current_draft(), Some("下書き1"));
    let env = s.rs.envelope_for_save().expect("envelope");
    let replayed = replay(&s.ir, SessionState::default(), &env.ops).expect("replay");
    assert_eq!(replayed.drafts, s.rs.session().state().drafts);
}
