//! 入力欄なしの教材（`pack.profile.input_field: hidden`）の表示の試験（TASK-450 2/3。PACK-20・TERM-23）。
//!
//! 入力欄・送信ボタン・補完の候補の一覧が DOM に 0 件で、停止と選択肢・画面操作・承認・確認の操作は出ることを、
//! 3 Step の教材（`tests/data/input_field_hidden/ir.json`）の各段で数える。キーの案内・OS の選択は未実装（TERM-38）の
//! ため、`select` 要素の 0 件で確かめる。口が無い教材では、下書き・送信・補完の action が状態を変えないことも固定する。
//! ブラウザでの実クリックによる完走は `tools/browser/input-field-hidden.test.mjs` の担当である。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::{Component, DirtyTracked};
use serde_json::Value;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn initial_state() -> SessionState {
    let p = testdata().join("contract/event-sequence.json");
    let s: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    serde_json::from_value(s["initial"].clone()).unwrap()
}

fn host_of_bytes(bytes: &[u8]) -> EngineHost {
    let ir = load_ir(bytes).unwrap();
    EngineHost::with_session(
        Session::from_state(ir, initial_state()),
        standard_registry(),
    )
    .unwrap()
}

/// 入力欄なしの教材のホスト。
fn no_input() -> EngineHost {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/input_field_hidden/ir.json");
    host_of_bytes(&std::fs::read(p).unwrap())
}

/// 既定の教材（`min-3step`。`input_field` の指定なし）のホスト。
fn default_pack() -> EngineHost {
    let p = testdata().join("engine-event-consistency/test-data/min-3step.json");
    host_of_bytes(&std::fs::read(p).unwrap())
}

fn html(h: &EngineHost) -> String {
    render_component_html(h)
}

fn count(h: &EngineHost, testid: &str) -> usize {
    html(h)
        .matches(&format!("data-testid=\"{testid}\""))
        .count()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn settle(h: &mut EngineHost) {
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}

fn status(h: &EngineHost) -> StepStatus {
    let s = h.model().session().unwrap().state();
    let uid = s.run.current.as_ref().unwrap();
    s.run.slices[&uid.root()].status[uid]
}

fn current(h: &EngineHost) -> String {
    h.model()
        .session()
        .unwrap()
        .state()
        .run
        .current
        .as_ref()
        .unwrap()
        .0
        .clone()
}

fn log_len(h: &EngineHost) -> usize {
    h.model().session().unwrap().log().len()
}

/// 入力の口として出てはならない 5 つのうち、DOM で数えられるものが 0 件であること。
///
/// 補完の候補（一覧とボタン）・入力欄・送信は testid と要素名、OS の選択は `select` 要素で数える。
/// キーの案内は部品が未実装（TERM-38）で、実装されたときもこの判定の下に置く。
fn assert_no_input_area(h: &EngineHost, at: &str) {
    let out = html(h);
    for testid in [
        "draft-input",
        "submit-input",
        "candidate-list",
        "candidate-btn",
    ] {
        assert_eq!(count(h, testid), 0, "{at}: {testid}\n{out}");
    }
    assert!(!out.contains("<textarea"), "{at}\n{out}");
    assert!(!out.contains("<select"), "{at}\n{out}");
    assert!(!out.contains("data-action-input"), "{at}\n{out}");
    // 停止と Esc の経路は、入力欄がなくても出る。
    assert_eq!(count(h, "stop-btn"), 1, "{at}\n{out}");
    assert!(out.contains("data-action-keydown"), "{at}\n{out}");
    assert_eq!(count(h, "hello-root"), 1, "{at}\n{out}");
}

/// 入力欄なしの 3 Step を、選択肢・画面操作・承認・確認だけで最後まで進める。各段で 0 件を確かめる。
fn walk(h: &mut EngineHost, mut each: impl FnMut(&EngineHost, &str)) {
    let ui = |path: &str| {
        HostAction::Engine(Op::Input(Input::Ui {
            view: "v-files".into(),
            path: path.into(),
        }))
    };
    each(h, "st-1 入力待ち");
    assert_eq!(current(h), "main/st-1");
    assert_eq!(count(h, "choice-btn"), 2);
    h.update(act("choose", "opt-a"));
    settle(h);
    assert_eq!(status(h), StepStatus::Accepted);
    each(h, "st-1 確認待ち");
    h.update(ui("notes/draft"));
    settle(h);

    assert_eq!(current(h), "main/st-2");
    each(h, "st-2 入力待ち");
    h.update(ui("notes/draft"));
    settle(h);
    assert_eq!(status(h), StepStatus::Accepted);
    each(h, "st-2 確認待ち");
    assert_eq!(count(h, "choice-btn"), 2);
    h.update(act("choose", "opt-right"));
    settle(h);

    assert_eq!(current(h), "main/st-3");
    each(h, "st-3 入力待ち");
    assert_eq!(count(h, "approval-btn"), 3);
    h.update(act("decide", "approve"));
    settle(h);
    assert_eq!(status(h), StepStatus::Accepted);
    each(h, "st-3 確認待ち");
    h.update(act("decide", "approve"));
    settle(h);
    each(h, "完了");
}

#[test]
fn 入力欄なしの教材では入力欄_送信_候補が0件で停止は出る() {
    let h = no_input();
    assert_no_input_area(&h, "開始直後");
    assert_eq!(count(&h, "input-notice"), 1);
}

#[test]
fn 入力欄なしでも選択肢_画面操作_承認_確認で最後まで進み各段で0件が保たれる() {
    let mut h = no_input();
    walk(&mut h, assert_no_input_area);
    assert_eq!(status(&h), StepStatus::Achieved);
}

#[test]
fn 入力欄なしでは下書き_送信_補完が状態を変えない() {
    let mut h = no_input();
    let before_html = html(&h);
    let before_log = log_len(&h);
    h.update(act("draft", "x"));
    assert!(h.dirty_fields().is_empty());
    h.update(act("submit", ""));
    assert!(h.dirty_fields().is_empty());
    h.update(HostAction::Complete);
    assert!(h.dirty_fields().is_empty());
    assert_eq!(h.draft(), "");
    assert_eq!(log_len(&h), before_log);
    assert_eq!(html(&h), before_html);
    assert_eq!(current(&h), "main/st-1");
}

#[test]
fn 既定の教材では入力の口が1件ずつ出る() {
    let h = default_pack();
    assert_eq!(count(&h, "draft-input"), 1);
    assert_eq!(count(&h, "submit-input"), 1);
    assert_eq!(count(&h, "candidate-list"), 1);
    assert_eq!(count(&h, "stop-btn"), 1);
}

#[test]
fn 入力欄なしの描画は未定義の識別子を持たない() {
    let mut h = no_input();
    walk(&mut h, |h, at| {
        let view = h.view();
        assert_eq!(
            fandhe_workshop_web::a11y::check_no_undefined(&view),
            Ok(()),
            "{at}"
        );
    });
}

#[test]
fn 読めない設定は既定の表示に倒れる() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/input_field_hidden/ir.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    v["pack"]["profile"] = serde_json::json!({ "input_field": "maybe" });
    // 読み込みが値を拒否するなら、読めない設定はホストまで届かない（既定に倒す分岐は到達しない）。
    let Ok(ir) = load_ir(&serde_json::to_vec(&v).unwrap()) else {
        return;
    };
    let h = EngineHost::with_session(
        Session::from_state(ir, initial_state()),
        standard_registry(),
    )
    .unwrap();
    assert_eq!(count(&h, "draft-input"), 1);
    assert_eq!(count(&h, "submit-input"), 1);
}
