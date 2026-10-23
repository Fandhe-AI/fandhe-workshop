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

use std::path::PathBuf;

use fandhe_workshop_contract::judgment::JudgmentKind;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::next_chapter::next_chapter;
use fandhe_workshop_web::parts::Component;
use serde_json::Value;

const FIRST: &str = "main/st-4-1a";
const LAST: &str = "main/st-9-1";

fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch4_9/ir.json")
}

/// 開始直後のホスト。開始の節点は教材の先頭（`main/st-4-1a`）に合わせる。
fn page() -> EngineHost {
    let ir = load_ir(&std::fs::read(data_path()).unwrap()).unwrap();
    let sample: Value = serde_json::from_slice(
        &std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../testdata/contract/event-sequence.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let text = serde_json::to_string(&sample["initial"])
        .unwrap()
        .replace("\"main/st-1\"", &format!("\"{FIRST}\""));
    let state: SessionState = serde_json::from_str(&text).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn state(h: &EngineHost) -> SessionState {
    h.model().session().unwrap().state().clone()
}

/// 画面から送れる手 1 つ。
#[derive(Debug, Clone, Copy)]
enum Move {
    /// 下書きを入れて送信。
    Send(&'static str),
    /// 選択肢のボタン（入力の段・答えの段で共通）。
    Choose(&'static str),
    /// 承認のボタン。
    Decide(&'static str),
    /// 対象つきの「直す」。
    Revise(&'static str),
    /// 左の指せる行のクリック。
    Point(&'static str, &'static str),
    /// 「次の章へ進む」（payload は開始点の ID）。判定は出ない。
    Next(&'static str),
}

impl Move {
    fn apply(self, h: &mut EngineHost) {
        match self {
            Move::Send(t) => {
                h.update(act("draft", t));
                h.update(act("submit", ""));
            }
            Move::Choose(id) => h.update(act("choose", id)),
            Move::Decide(d) => h.update(act("decide", d)),
            Move::Revise(t) => h.update(act("revise-target", t)),
            Move::Point(view, path) => h.update(HostAction::Engine(Op::Input(
                fandhe_workshop_core::input::Input::Ui {
                    view: view.into(),
                    path: path.into(),
                },
            ))),
            Move::Next(start) => h.update(act("next-chapter", start)),
        }
    }
}

use JudgmentKind::{Accepted as Acc, Confirmed as Cfm, Rejected as Rej};
use StepStatus::{Accepted as Wait, Achieved as Done, Entered as In};

/// 模範の操作列。`(手, 最初の判定の種類（章の移動は None）, 手の後の現在の Step, 手の後の状態)`。
#[rustfmt::skip]
fn journey() -> Vec<(Move, Option<JudgmentKind>, &'static str, StepStatus)> {
    use Move::*;
    vec![
        // CH-4
        (Decide("revise"), Some(Rej), "main/st-4-1a", In),
        (Decide("approve"), Some(Rej), "main/st-4-1a", In),
        (Revise("t-scope"), Some(Acc), "main/st-4-1a", Wait),
        (Decide("approve"), Some(Cfm), "main/st-4-1b", In),
        (Decide("revise"), Some(Rej), "main/st-4-1b", In),
        (Decide("approve"), Some(Acc), "main/st-4-1b", Wait),
        (Point("v-files", "files/config"), Some(Cfm), "main/st-4-2", In),
        (Choose("opt-rerun"), Some(Acc), "main/st-4-2", Wait),
        (Choose("ans-empty"), Some(Rej), "main/st-4-2", Wait),
        (Choose("ans-exists"), Some(Cfm), "main/st-4-2", Done),
        (Next("s-5-1"), None, "main/st-5-1", In),
        // CH-5
        (Send("/setup-config BadName"), Some(Rej), "main/st-5-1", In),
        (Send("/setup-config main-area"), Some(Acc), "main/st-5-1", Wait),
        (Point("v-checks", "checks/config"), Some(Cfm), "main/st-5-2", In),
        (Choose("opt-stop-ng"), Some(Rej), "main/st-5-2", In),
        (Choose("opt-stop-ok"), Some(Acc), "main/st-5-2", Done),
        (Next("s-6-1"), None, "main/st-6-1", In),
        // CH-6
        (Send("1 行目の要件\n2 行目の要件"), Some(Acc), "main/st-6-2a", In),
        (Decide("revise"), Some(Rej), "main/st-6-2a", In),
        (Revise("t-split"), Some(Acc), "main/st-6-2a", Wait),
        (Decide("approve"), Some(Cfm), "main/st-6-2b", In),
        (Decide("approve"), Some(Acc), "main/st-6-3", In),
        (Choose("opt-read-status"), Some(Acc), "main/st-6-3", Done),
        (Next("s-7-1"), None, "main/st-7-1", In),
        // CH-7
        (Send("/add-guards"), Some(Rej), "main/st-7-1", In),
        (Send("/add-guards all-areas"), Some(Acc), "main/st-7-1", Wait),
        (Point("v-checks", "checks/guards"), Some(Cfm), "main/st-7-2", In),
        (Send("/run-tasks"), Some(Acc), "main/st-7-2", Wait),
        (Point("v-cards", "cards/job-1"), Some(Cfm), "main/st-7-3", In),
        (Choose("opt-view"), Some(Acc), "main/st-7-3", Wait),
        (Choose("ans-all"), Some(Rej), "main/st-7-3", Wait),
        (Choose("ans-stopped"), Some(Cfm), "main/st-7-4", In),
        (Point("v-cards", "cards/job-2"), Some(Rej), "main/st-7-4", In),
        (Point("v-cards", "cards/job-1"), Some(Acc), "main/st-7-4", Done),
        (Next("s-8-1"), None, "main/st-8-1", In),
        // CH-8
        (Point("v-demo", "demo/name-pick"), Some(Acc), "main/st-8-1", Wait),
        (Point("v-demo", "demo/post"), Some(Cfm), "main/st-8-2", In),
        (Choose("opt-sort-ng"), Some(Rej), "main/st-8-2", In),
        (Choose("opt-sort-ok"), Some(Acc), "main/st-8-3", In),
        (Decide("approve"), Some(Acc), "main/st-8-3", Wait),
        (Point("v-diff", "diff/fixed"), Some(Cfm), "main/st-8-3", Done),
        (Next("s-9-1"), None, LAST, In),
        // CH-9
        (Point("v-cards", "cards/checkpoint-1"), Some(Acc), LAST, Done),
    ]
}

/// 処理中が終わるまで進行を打つ。
fn settle(h: &mut EngineHost) {
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}

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
