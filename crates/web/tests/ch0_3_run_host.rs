//! CH-0〜CH-3 の形の 20 Step を 1 つの実行環境で最後まで進める試験。host の層（TASK-450 3/3、TERM-23）。
//!
//! ブラウザの試験（`tools/browser/ch0-3-run.test.mjs`）と同じ教材（`tests/data/ch0_3/ir.json`）を、画面から送れる
//! 手（下書きと送信・補完・選択肢・承認のボタン・左の行・Esc）だけで通す。テスト専用の口（`raw`）は使わない。
//! 教材 repo の CH-0〜CH-3 ではなく、その形を写した試験用の教材である（`tests/data/ch0_3/README.md`）。
//! 各手の後に、判定の種類・現在の Step・状態を確かめる。確認の入力を飛ばした達成が無いこと、警告が 0 件であること、
//! 5 つの入力方式と 4 種の確認が通しの中に 1 回以上あることも固定する。
//! 試験では `unwrap` を使ってよい。

use std::collections::BTreeSet;
use std::path::PathBuf;

use fandhe_workshop_contract::judgment::JudgmentKind;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use serde_json::Value;

/// 1 回の通しに許す手の数の上限（無限ループで試験が止まらないための安全弁）。
const MOVE_LIMIT: usize = 200;
const FIRST: &str = "main/st-0-1";
const LAST: &str = "main/st-3-2";

fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch0_3/ir.json")
}

/// 開始直後のホスト。開始の節点は教材の先頭（`main/st-0-1`）に合わせる。
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
    /// 下書きを入れて送信（入力欄で Enter）。
    Send(&'static str),
    /// 下書きを入れて Tab で補完し、送信する。
    CompleteAndSend(&'static str),
    /// 選択肢のボタン（入力の段・答えの段で共通）。
    Choose(&'static str),
    /// 承認のボタン（`approve`・`reject`・`revise`）。
    Decide(&'static str),
    /// 対象つきの「直す」。
    Revise(&'static str),
    /// 左の指せる行のクリック。
    Point(&'static str, &'static str),
    /// 処理中でない Esc。
    Esc,
}

impl Move {
    fn apply(self, h: &mut EngineHost) {
        match self {
            Move::Send(t) => {
                h.update(act("draft", t));
                h.update(act("submit", ""));
            }
            Move::CompleteAndSend(t) => {
                h.update(act("draft", t));
                h.update(HostAction::Complete);
                h.update(act("submit", ""));
            }
            Move::Choose(id) => h.update(act("choose", id)),
            Move::Decide(d) => h.update(act("decide", d)),
            Move::Revise(t) => h.update(act("revise-target", t)),
            Move::Point(view, path) => h.update(HostAction::Engine(Op::Input(Input::Ui {
                view: view.into(),
                path: path.into(),
            }))),
            Move::Esc => h.update(HostAction::Engine(Op::Control(Control::Stop))),
        }
    }
}

use JudgmentKind::{Accepted as Acc, Clarified as Clr, Confirmed as Cfm, Rejected as Rej};
use StepStatus::{Accepted as Wait, Entered as In};

const V: &str = "v-cards";
const F: &str = "v-files";

/// 模範の操作列。`(手, 最初の判定の種類, 手の後の現在の Step, 手の後の状態)`。
///
/// 受付と同時に達成になる Step（確認が `none`）は、次の Step の入力待ちになる。
#[rustfmt::skip]
fn journey() -> Vec<(Move, JudgmentKind, &'static str, StepStatus)> {
    use Move::*;
    vec![
        // CH-0
        (Choose("opt-left"), Rej, "main/st-0-1", In),
        (Choose("opt-top"), Rej, "main/st-0-1", In),
        (Choose("opt-right"), Acc, "main/st-0-1", Wait),
        (Point(V, "cards/notice"), Cfm, "main/st-0-2a", In),
        (Choose("opt-read-a"), Acc, "main/st-0-2a", Wait),
        (Choose("ans-a-ng"), Rej, "main/st-0-2a", Wait),
        (Choose("ans-a-ok"), Cfm, "main/st-0-2b", In),
        (Choose("opt-read-b"), Acc, "main/st-0-2b", Wait),
        (Choose("ans-b-ok"), Cfm, "main/st-0-2c", In),
        (Choose("opt-read-c"), Acc, "main/st-0-2c", Wait),
        (Choose("ans-c-ok"), Cfm, "main/st-1-1", In),
        // CH-1
        (Send("/hepl"), Rej, "main/st-1-1", In),
        (Send("/help"), Acc, "main/st-1-1", Wait),
        (Point(V, "cards/help-1"), Cfm, "main/st-1-2", In),
        (CompleteAndSend("/he"), Acc, "main/st-1-3", In),
        (Send("1 行目のメモ\n2 行目のメモ"), Acc, "main/st-1-4", In),
        (Send("/new-idea"), Clr, "main/st-1-4", In),
        (Send("/advance-phase"), Rej, "main/st-1-4", In),
        (Esc, Acc, "main/st-1-4", Wait),
        (Choose("asked-back"), Cfm, "main/st-1-5", In),
        (Send("/make-card sample"), Acc, "main/st-1-5", Wait),
        (Point(V, "cards/sample"), Cfm, "main/st-1-6", In),
        (Choose("opt-right"), Acc, "main/st-1-6", Wait),
        (Choose("ans-restored"), Cfm, "main/st-2-1", In),
        // CH-2
        (Send("/new-idea ideas"), Acc, "main/st-2-1", Wait),
        (Point(F, "docs/readme"), Cfm, "main/st-2-2", In),
        (Choose("opt-view"), Acc, "main/st-2-2", Wait),
        (Choose("ans-readme"), Cfm, "main/st-2-3", In),
        (Decide("approve"), Acc, "main/st-2-3", Wait),
        (Point(F, "docs/plan"), Cfm, "main/st-2-4", In),
        (Send("状況を書いた自然文"), Acc, "main/st-2-5", In),
        (Send("/run-check"), Acc, "main/st-2-5", Wait),
        (Point(F, "checks/result"), Cfm, "main/st-2-6", In),
        (Decide("revise"), Rej, "main/st-2-6", In),
        (Decide("approve"), Rej, "main/st-2-6", In),
        (Revise("t-expect"), Acc, "main/st-2-6", Wait),
        (Decide("approve"), Cfm, "main/st-2-7", In),
        (Point(F, "tasks/t-2"), Rej, "main/st-2-7", In),
        (Point(F, "tasks/t-1"), Acc, "main/st-2-7", Wait),
        (Choose("ans-t2"), Rej, "main/st-2-7", Wait),
        (Choose("ans-t1"), Cfm, "main/st-2-8", In),
        (Choose("opt-go"), Acc, "main/st-3-1", In),
        // CH-3
        (Choose("opt-place-ng"), Rej, "main/st-3-1", In),
        (Choose("opt-place-ok"), Acc, "main/st-3-1", Wait),
        (Point(F, "work/place"), Cfm, "main/st-3-2", In),
        (Decide("approve"), Acc, "main/st-3-2", Wait),
        (Decide("approve"), Cfm, LAST, StepStatus::Achieved),
    ]
}

/// 処理中が終わるまで進行を打つ（ブラウザと同じく、処理中の地点を作らない）。
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

/// 通しを流し、最後の状態を返す。各手の判定・現在の Step・状態を確かめる。
fn run_journey(h: &mut EngineHost) -> SessionState {
    let moves = journey();
    assert!(moves.len() < MOVE_LIMIT);
    for (i, (mv, kind, current, status)) in moves.into_iter().enumerate() {
        let before = state(h).log.len();
        let place_before = place_text(h);
        mv.apply(h);
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
