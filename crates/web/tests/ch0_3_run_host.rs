//! CH-0〜CH-3 の形の 20 Step を 1 つの実行環境で最後まで進める試験。host の層（TASK-450 3/3、TERM-23）。
//!
//! ブラウザの試験（`tools/browser/ch0-3-run.test.mjs`）と同じ教材（`tests/data/ch0_3/ir.json`）を、画面から送れる
//! 手（下書きと送信・補完・選択肢・承認のボタン・左の行・Esc）だけで通す。テスト専用の口（`raw`）は使わない。
//! 教材 repo の CH-0〜CH-3 ではなく、その形を写した試験用の教材である（`tests/data/ch0_3/README.md`）。
//! 各手の後に、判定の種類・現在の Step・状態を確かめる。確認の入力を飛ばした達成が無いこと、警告が 0 件であること、
//! 5 つの入力方式と 4 種の確認が通しの中に 1 回以上あることも固定する。
//! 試験では `unwrap` を使ってよい。

#[path = "common/ch0_3_journey.rs"]
mod ch0_3_journey;

use std::collections::BTreeSet;

use ch0_3_journey::*;
use fandhe_workshop_contract::judgment::JudgmentKind;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::parts::Component;
use serde_json::Value;

/// 1 回の通しに許す手の数の上限（無限ループで試験が止まらないための安全弁）。
const MOVE_LIMIT: usize = 200;

fn status_of(s: &SessionState, uid: &str) -> StepStatus {
    let uid = StepUid(uid.to_owned());
    s.run
        .slices
        .get(&uid.root())
        .and_then(|sl| sl.status.get(&uid))
        .copied()
        .unwrap_or_default()
}

/// 画面の上部の位置の文字（無ければ空）。
fn place_text(h: &EngineHost) -> String {
    fn go(n: &fandhe_frontend_core::Node, out: &mut String) {
        if let fandhe_frontend_core::Node::Element {
            attrs, children, ..
        } = n
        {
            if attrs
                .iter()
                .any(|(k, v)| k == "data-testid" && v == "shell-position")
            {
                if let Some(fandhe_frontend_core::Node::Text(t)) = children.first() {
                    out.push_str(t);
                }
            }
            for c in children {
                go(c, out);
            }
        }
    }
    let mut s = String::new();
    go(&h.view(), &mut s);
    s
}

/// 画面の Step の題（`step-title`）の文字。
fn title_text(h: &EngineHost) -> String {
    fn go(n: &fandhe_frontend_core::Node, out: &mut String) {
        if let fandhe_frontend_core::Node::Element {
            attrs, children, ..
        } = n
        {
            if attrs
                .iter()
                .any(|(k, v)| k == "data-testid" && v == "step-title")
            {
                if let Some(fandhe_frontend_core::Node::Text(t)) = children.first() {
                    out.push_str(t);
                }
            }
            for c in children {
                go(c, out);
            }
        }
    }
    let mut s = String::new();
    go(&h.view(), &mut s);
    s
}

/// 通しを流し、最後の状態を返す。各手の判定・現在の Step・状態を確かめる。
fn run_journey(h: &mut EngineHost) -> SessionState {
    let moves = journey();
    assert!(moves.len() < MOVE_LIMIT);
    for (i, (mv, kind, current, status)) in moves.into_iter().enumerate() {
        let before = state(h).log.len();
        let place_before = place_text(h);
        let title_before = title_text(h);
        mv.apply(h);
        // Step の題が変わった手の直後は、その field が dirty に積まれる（TASK-525。FEND-3）。
        if title_text(h) != title_before {
            assert!(
                fandhe_workshop_web::parts::DirtyTracked::dirty_fields(&*h)
                    .contains(&fandhe_workshop_web::step_body::STEP_BODY_FIELD.name),
                "手 {i} {mv:?}: 題が変わったのに step_body の dirty が無い"
            );
        }
        // 上部の位置が変わった手の直後は、その field が dirty に積まれる（TASK-524。FEND-3）。
        if place_text(h) != place_before {
            assert!(
                fandhe_workshop_web::parts::DirtyTracked::dirty_fields(&*h)
                    .contains(&fandhe_workshop_web::shell_frame::TOP_PLACE_FIELD.name),
                "手 {i} {mv:?}: 位置が変わったのに dirty が無い"
            );
        }
        let s = state(h);
        let first = s.log[before..]
            .iter()
            .find_map(|e| match e {
                Event::Judged { judgment, .. } => Some(judgment.kind()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("手 {i} {mv:?}: 判定が無い"));
        assert_eq!(first, kind, "手 {i} {mv:?}: 判定の種類");
        settle(h);
        let s = state(h);
        assert_eq!(
            s.run.current.as_ref().unwrap().0,
            current,
            "手 {i} {mv:?}: 現在の Step"
        );
        assert_eq!(status_of(&s, current), status, "手 {i} {mv:?}: 状態");
        // 「模擬」を含む版 2 の枠が、どの手の後にも残る（TASK-523。SHELL-6）。
        let v = h.view();
        assert_eq!(
            fandhe_workshop_web::shell::check_complete_v2(&v),
            Ok(()),
            "手 {i} {mv:?}: 枠の適合（模擬を含む）"
        );
    }
    state(h)
}

#[test]
fn 画面から送れる手だけで二十の_step_を最後まで進める() {
    let mut h = page();
    let s = run_journey(&mut h);
    assert_eq!(s.run.current.as_ref().unwrap().0, LAST);
    let main = &s.run.slices[&StepUid(FIRST.to_owned()).root()];
    assert_eq!(main.status.len(), 20, "全 Step に入った");
    assert!(
        main.status.values().all(|v| *v == StepStatus::Achieved),
        "全 Step が達成: {:?}",
        main.status
    );
}

#[test]
fn 通しの途中で警告と無視が0件で確認を飛ばした達成が無い() {
    let mut h = page();
    let s = run_journey(&mut h);
    let ir: Value = serde_json::from_slice(&std::fs::read(data_path()).unwrap()).unwrap();
    let confirm_none: BTreeSet<String> = ir["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["step"]["confirm"]["kind"] == "none")
        .map(|n| n["uid"].as_str().unwrap().to_owned())
        .collect();
    let mut warnings = 0;
    let mut ignored = 0;
    let mut confirmed_steps: BTreeSet<String> = BTreeSet::new();
    let mut achieved = 0;
    let mut last_judged_confirmed = false;
    let mut explicit_skip = 0;
    for e in &s.log {
        match e {
            Event::Warning { .. } => warnings += 1,
            Event::Ignored { .. } => ignored += 1,
            Event::Judged { judgment, .. } => {
                last_judged_confirmed = judgment.kind() == JudgmentKind::Confirmed;
            }
            Event::Achieved {
                at,
                explicit_skip: skip,
            } => {
                achieved += 1;
                if *skip {
                    explicit_skip += 1;
                }
                // 確認のある Step の達成は、直前の判定が「確認」である。確認が `none` の Step は受付と同時に達成になる。
                let uid = at.step.0.clone();
                if !confirm_none.contains(&uid) {
                    assert!(last_judged_confirmed, "{uid}: 確認なしの達成");
                    confirmed_steps.insert(uid);
                }
            }
            _ => {}
        }
    }
    assert_eq!(warnings, 0, "警告");
    assert_eq!(ignored, 0, "無視");
    assert_eq!(explicit_skip, 0, "省略による達成");
    assert_eq!(achieved, 20);
    assert_eq!(confirmed_steps.len() + confirm_none.len(), 20);
}

#[test]
fn 教材は五つの入力方式と四種の確認を持ち_通しの手がそれぞれを使う() {
    let ir: Value = serde_json::from_slice(&std::fs::read(data_path()).unwrap()).unwrap();
    let nodes = ir["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 20);
    let kinds = |key: &str| -> BTreeSet<String> {
        nodes
            .iter()
            .map(|n| n["step"][key]["kind"].as_str().unwrap().to_owned())
            .collect()
    };
    let want = |xs: &[&str]| xs.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<_>>();
    assert_eq!(
        kinds("input"),
        want(&["command", "text", "choice", "ui_action", "approval"])
    );
    assert_eq!(
        kinds("confirm"),
        want(&["point_at", "answer", "approve", "none"])
    );
    // 通しの手の側でも、各方式の手が 1 回以上ある（教材だけが形を持ち、手が使わない回帰を防ぐ）。
    let moves: Vec<Move> = journey().into_iter().map(|(m, ..)| m).collect();
    let has = |f: fn(&Move) -> bool| moves.iter().any(f);
    assert!(has(|m| matches!(m, Move::Send(t) if t.starts_with('/'))));
    assert!(has(|m| matches!(m, Move::Send(t) if !t.starts_with('/'))));
    assert!(has(|m| matches!(m, Move::Choose(_))));
    assert!(has(|m| matches!(m, Move::Decide(_))));
    assert!(has(|m| matches!(m, Move::Point(F, _))));
}
