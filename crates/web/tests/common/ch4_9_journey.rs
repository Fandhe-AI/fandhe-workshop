//! CH-4〜CH-9 の形の 17 Step・6 章 の通し（`Move`・`journey`・開始状態）を、試験の間で共有する部品（TASK-337）。
//!
//! 出所は `ch4_9_run_host.rs`。履歴の大きさの測定（`history_size_all_chapters.rs`）が同じ操作列を使えるよう、二重定義を避けて 1 か所に置く。
//! 試験では `unwrap` を使ってよい。

#![allow(dead_code, reason = "試験ファイルごとに使う部品が異なるため")]

use std::path::PathBuf;

use fandhe_workshop_contract::judgment::JudgmentKind;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use serde_json::Value;

pub const FIRST: &str = "main/st-4-1a";
pub const LAST: &str = "main/st-9-1";

pub fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch4_9/ir.json")
}

/// 開始直後のホスト。開始の節点は教材の先頭（`main/st-4-1a`）に合わせる。
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
    pub fn apply(self, h: &mut EngineHost) {
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

pub use JudgmentKind::{Accepted as Acc, Confirmed as Cfm, Rejected as Rej};
pub use StepStatus::{Accepted as Wait, Achieved as Done, Entered as In};

/// 模範の操作列。`(手, 最初の判定の種類（章の移動は None）, 手の後の現在の Step, 手の後の状態)`。
#[rustfmt::skip]
pub fn journey() -> Vec<(Move, Option<JudgmentKind>, &'static str, StepStatus)> {
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
pub fn settle(h: &mut EngineHost) {
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}
