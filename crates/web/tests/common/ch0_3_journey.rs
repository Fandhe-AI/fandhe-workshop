//! CH-0〜CH-3 の形の 20 Step の通し（`Move`・`journey`・開始状態）を、試験の間で共有する部品（TASK-337）。
//!
//! 出所は `ch0_3_run_host.rs`。履歴の大きさの測定（`history_size_all_chapters.rs`）が同じ操作列を使えるよう、二重定義を避けて 1 か所に置く。
//! 試験では `unwrap` を使ってよい。

#![allow(dead_code, reason = "試験ファイルごとに使う部品が異なるため")]

use std::path::PathBuf;

use fandhe_workshop_contract::judgment::JudgmentKind;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use serde_json::Value;

pub const FIRST: &str = "main/st-0-1";
pub const LAST: &str = "main/st-3-2";

pub fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch0_3/ir.json")
}

/// 開始直後のホスト。開始の節点は教材の先頭（`main/st-0-1`）に合わせる。
pub fn page() -> EngineHost {
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

pub fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

pub fn state(h: &EngineHost) -> SessionState {
    h.model().session().unwrap().state().clone()
}

/// 画面から送れる手 1 つ。
#[derive(Debug, Clone, Copy)]
pub enum Move {
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
    pub fn apply(self, h: &mut EngineHost) {
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

pub use JudgmentKind::{Accepted as Acc, Clarified as Clr, Confirmed as Cfm, Rejected as Rej};
pub use StepStatus::{Accepted as Wait, Entered as In};

pub const V: &str = "v-cards";
pub const F: &str = "v-files";

/// 模範の操作列。`(手, 最初の判定の種類, 手の後の現在の Step, 手の後の状態)`。
///
/// 受付と同時に達成になる Step（確認が `none`）は、次の Step の入力待ちになる。
#[rustfmt::skip]
pub fn journey() -> Vec<(Move, JudgmentKind, &'static str, StepStatus)> {
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
pub fn settle(h: &mut EngineHost) {
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}
