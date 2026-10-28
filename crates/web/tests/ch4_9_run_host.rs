//! CH-4〜CH-9 の形の 17 Step・6 章を 1 つの実行環境で最後まで進める試験。host の層（TASK-674 1/2）。
//!
//! ブラウザの試験（`tools/browser/ch4-9-run.test.mjs`）と同じ教材（`tests/data/ch4_9/ir.json`）を、画面から送れる手
//! （下書きと送信・選択肢・承認のボタン・左の行・「次の章へ進む」）だけで通す。テスト専用の口（`raw`）は使わない。
//! 教材 repo の本物の CH-4〜CH-9 ではなく、その形を写した試験用の教材である（`tests/data/ch4_9/README.md`）。
//! ブラウザより前に IR の誤りを捕まえるのが役割で、各手の後に判定の種類・現在の Step・状態を確かめる。
//! 章の境目は `next_chapter::press_target` の経路（`next-chapter` の action）で越え、各章の最後で提示が 1 件、
//! 章の途中と CH-9 の最後（次の章が無い）で 0 件であることも固定する。処理中の送信が `busy` で拒否されることは
//! ブラウザでは時間の窓に頼るため、ここで進行を止めたまま確かめる。
//! 試験では `unwrap` を使ってよい。

#[path = "common/ch4_9_journey.rs"]
mod ch4_9_journey;

use ch4_9_journey::*;
use fandhe_workshop_contract::judgment::JudgmentKind;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::next_chapter::next_chapter;
use serde_json::Value;

fn status_of(s: &SessionState, uid: &str) -> StepStatus {
    let uid = StepUid(uid.to_owned());
    s.run
        .slices
        .get(&uid.root())
        .and_then(|sl| sl.status.get(&uid))
        .copied()
        .unwrap_or_default()
}

/// 「次の章へ進む」の提示の数（0 か 1）。
fn offers(h: &EngineHost) -> usize {
    let m = h.model();
    usize::from(next_chapter(m.session().unwrap().ir(), &state(h)).is_some())
}

/// 通しを流し、最後の状態を返す。各手の判定・現在の Step・状態と、提示の個数を確かめる。
fn run_journey(h: &mut EngineHost) -> (SessionState, usize) {
    let mut presses = 0;
    for (i, (mv, kind, current, status)) in journey().into_iter().enumerate() {
        let before = state(h).log.len();
        let offered_before = offers(h);
        mv.apply(h);
        let s = state(h);
        if let Move::Next(_) = mv {
            assert_eq!(offered_before, 1, "手 {i} {mv:?}: 押す前に提示が 1 件");
            presses += 1;
        } else {
            let first = s.log[before..]
                .iter()
                .find_map(|e| match e {
                    Event::Judged { judgment, .. } => Some(judgment.kind()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("手 {i} {mv:?}: 判定が無い"));
            assert_eq!(Some(first), kind, "手 {i} {mv:?}: 判定の種類");
        }
        settle(h);
        let s = state(h);
        assert_eq!(
            s.run.current.as_ref().unwrap().0,
            current,
            "手 {i} {mv:?}: 現在の Step"
        );
        assert_eq!(status_of(&s, current), status, "手 {i} {mv:?}: 状態");
        // 提示は、次の手が章の移動であるときだけ 1 件。章の途中・押した直後・CH-9 の最後は 0 件。
        let next_is_cross = journey()
            .get(i + 1)
            .is_some_and(|(m, ..)| matches!(m, Move::Next(_)));
        assert_eq!(
            offers(h),
            usize::from(next_is_cross),
            "手 {i} {mv:?}: 「次の章へ進む」の提示の数"
        );
    }
    (state(h), presses)
}

#[test]
fn 画面から送れる手だけで十七の_step_を最後まで進める() {
    let mut h = page();
    let (s, presses) = run_journey(&mut h);
    assert_eq!(s.run.current.as_ref().unwrap().0, LAST);
    let main = &s.run.slices[&StepUid(FIRST.to_owned()).root()];
    assert_eq!(main.status.len(), 17, "全 Step に入った");
    assert!(
        main.status.values().all(|v| *v == StepStatus::Achieved),
        "全 Step が達成: {:?}",
        main.status
    );
    assert_eq!(presses, 5, "章の境目は 5 回");
    assert_eq!(offers(&h), 0, "CH-9 の後は提示なし（開始点が無い）");
}

#[test]
fn 通しの途中で警告と無視と省略が0件() {
    let mut h = page();
    let (s, _) = run_journey(&mut h);
    let mut achieved = 0;
    for e in &s.log {
        match e {
            Event::Warning { .. } => panic!("警告: {e:?}"),
            Event::Ignored { .. } => panic!("無視: {e:?}"),
            Event::Achieved { explicit_skip, .. } => {
                assert!(!explicit_skip, "省略による達成");
                achieved += 1;
            }
            _ => {}
        }
    }
    assert_eq!(achieved, 17);
}

#[test]
fn 処理中の送信は_busy_で拒否され_終わると進める() {
    let mut h = page();
    for (mv, ..) in journey().into_iter().take(27) {
        mv.apply(&mut h);
        settle(&mut h);
    }
    assert_eq!(state(&h).run.current.as_ref().unwrap().0, "main/st-7-2");
    // 受付の直後は処理中。進行を打たずに、もう一度送る。
    Move::Send("/run-tasks").apply(&mut h);
    assert!(h.next_tick().is_some(), "処理中のはず");
    let before = state(&h).log.len();
    Move::Send("/run-tasks").apply(&mut h);
    let s = state(&h);
    let kinds: Vec<JudgmentKind> = s.log[before..]
        .iter()
        .filter_map(|e| match e {
            Event::Judged { judgment, .. } => Some(judgment.kind()),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, vec![Rej], "処理中の送信は拒否");
    settle(&mut h);
    assert_eq!(status_of(&state(&h), "main/st-7-2"), Wait);
}

#[test]
fn 教材は十七_step_六章で開始点6件で_final_が6件() {
    let ir: Value = serde_json::from_slice(&std::fs::read(data_path()).unwrap()).unwrap();
    let nodes = ir["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 17);
    let finals: Vec<&str> = nodes
        .iter()
        .filter(|n| n["step"]["final"] == true)
        .map(|n| n["uid"].as_str().unwrap())
        .collect();
    assert_eq!(
        finals,
        [
            "main/st-4-2",
            "main/st-5-2",
            "main/st-6-3",
            "main/st-7-4",
            "main/st-8-3",
            "main/st-9-1"
        ]
    );
    assert_eq!(ir["starts"].as_array().unwrap().len(), 6);
    let mut views: Vec<&str> = ir["views"]
        .as_object()
        .unwrap()
        .values()
        .map(|v| v["kind"].as_str().unwrap())
        .collect();
    views.sort_unstable();
    assert_eq!(
        views,
        [
            "app_demo",
            "card_list",
            "check_list",
            "diff",
            "file_tree",
            "task_tree"
        ]
    );
}
