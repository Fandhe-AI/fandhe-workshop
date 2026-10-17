//! 入力方式ごとの入力の口の切り替え（TASK-450 1/3。TERM-23）の試験。
//!
//! 8 Step の教材（`tests/data/reload_points_ch2/ir.json`）の各 Step に入った直後を描き、方式に合う口だけが出て
//! 他の口が出ないことを数える。切り替わるのは Step ごとに変わる口（選択肢のボタン・承認のボタン）で、
//! 入力欄・送信は教材全体の設定（`input_field`）が決めるため、Step の方式では出し入れしない（入力欄なしの表示は `input_field_hidden.rs`。TASK-450 2/3）。
//! 入力待ちの選択肢の判定は core の入力の段（`judge_select`）で、ホストは照合しない。
//! 教材由来の文字列（選択肢の ID・表示名）は、文字ノードと属性値のエスケープに通ることも確かめる。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use serde_json::{json, Value};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn initial_state() -> SessionState {
    let p = testdata().join("contract/event-sequence.json");
    let s: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    serde_json::from_value(s["initial"].clone()).unwrap()
}

fn host_of(ir: Ir) -> EngineHost {
    EngineHost::with_session(
        Session::from_state(ir, initial_state()),
        standard_registry(),
    )
    .unwrap()
}

fn reload_points() -> EngineHost {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/reload_points_ch2/ir.json");
    host_of(load_ir(&std::fs::read(p).unwrap()).unwrap())
}

/// `min-3step` の `main/st-1` を `edit` で書き換えたホスト。
fn min3_with(edit: impl FnOnce(&mut serde_json::Map<String, Value>)) -> EngineHost {
    let p = testdata().join("engine-event-consistency/test-data/min-3step.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    edit(v["nodes"][0]["step"].as_object_mut().unwrap());
    host_of(load_ir(&serde_json::to_vec(&v).unwrap()).unwrap())
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

/// 入った直後の Step で、4 つの口の数を (選択肢, 承認, 入力欄, 送信) の順で返す。
fn ports(h: &EngineHost) -> (usize, usize, usize, usize) {
    (
        count(h, "choice-btn"),
        count(h, "approval-btn"),
        count(h, "draft-input"),
        count(h, "submit-input"),
    )
}

fn send(h: &mut EngineHost, text: &str) {
    h.update(act("draft", text));
    h.update(act("submit", ""));
    settle(h);
}

/// 受付済み（確認待ち）なら確認の操作を、そうでなければ入力を打って次の Step へ進める。
fn advance(h: &mut EngineHost) {
    let cur = current(h);
    let n: usize = cur.strip_prefix("main/st-").unwrap().parse().unwrap();
    let path = format!("notes/n{n}");
    let ui = || {
        Op::Input(Input::Ui {
            view: "v-files".into(),
            path: path.clone(),
        })
    };
    if status(h) == StepStatus::Accepted {
        match n {
            2 | 7 => h.update(act("choose", "opt-a")),
            6 => h.update(act("decide", "approve")),
            _ => h.update(HostAction::Engine(ui())),
        }
    } else {
        match n {
            2 => send(h, "メモ 2"),
            3 | 8 => h.update(act("decide", "approve")),
            4 | 5 => h.update(act("choose", "opt-a")),
            6 => h.update(act("revise-target", "expect")),
            7 => h.update(HostAction::Engine(ui())),
            _ => send(h, &format!("/make-note note-{n}")),
        }
    }
    settle(h);
}

fn go_to(h: &mut EngineHost, uid: &str) {
    for _ in 0..40 {
        if current(h) == uid && status(h) == StepStatus::Entered {
            return;
        }
        advance(h);
    }
    panic!("{uid} に入れない");
}

#[test]
fn 方式ごとに入った直後の口の数が表のとおりになる() {
    let mut h = reload_points();
    // (Step の番号, 選択肢のボタン, 承認のボタン)。入力欄と送信は教材全体の設定が決めるため全 Step で 1 つ。
    let table = [
        (1, 0, 0),
        (2, 0, 0),
        (3, 0, 3),
        (4, 2, 0),
        (5, 2, 0),
        (6, 0, 4),
        (7, 0, 0),
        (8, 0, 3),
    ];
    for (n, choices, approvals) in table {
        go_to(&mut h, &format!("main/st-{n}"));
        assert_eq!(
            ports(&h),
            (choices, approvals, 1, 1),
            "st-{n}: {}",
            html(&h)
        );
    }
}

#[test]
fn choice_の入力待ちは宣言の選択肢を表示名で出し_id_は属性値だけに入る() {
    let mut h = reload_points();
    go_to(&mut h, "main/st-4");
    let out = html(&h);
    assert!(out.contains("data-choice=\"opt-a\""), "{out}");
    assert!(out.contains("data-choice=\"opt-b\""), "{out}");
    assert!(out.contains(">案 A<") && out.contains(">案 B<"), "{out}");
    assert!(
        !out.contains(">opt-a</button>") && !out.contains(">opt-b</button>"),
        "{out}"
    );
    assert_eq!(out.matches("data-action=\"choose\"").count(), 2, "{out}");
}

#[test]
fn 入力待ちの選択肢は_core_の入力の段で判定され_宣言に無い_id_は進まない() {
    let mut h = reload_points();
    go_to(&mut h, "main/st-4");
    // 宣言に無い ID は拒否で、Step は入力待ちのまま。
    h.update(act("choose", "opt-forged"));
    settle(&mut h);
    assert_eq!(current(&h), "main/st-4");
    assert_eq!(status(&h), StepStatus::Entered);
    assert_eq!(count(&h, "choice-btn"), 2);
    // st-4 は確認が `none` で、宣言の選択肢で達成になり次へ進む。
    h.update(act("choose", "opt-a"));
    settle(&mut h);
    assert_eq!(current(&h), "main/st-5");
    // st-5 は確認が `point_at`。受付済みになり、確認待ちでは入力の選択肢は消える。
    h.update(act("choose", "opt-a"));
    settle(&mut h);
    assert_eq!(current(&h), "main/st-5");
    assert_eq!(status(&h), StepStatus::Accepted);
    assert_eq!(count(&h, "choice-btn"), 0, "{}", html(&h));
}

#[test]
fn 処理中は入力待ちの選択肢を出さない() {
    // st-1 を、受付のあとに進行が残る `choice` にする（元の st-1 は 5 段の進行を持つ）。
    let mut h = min3_with(|step| {
        step.insert(
            "input".into(),
            json!({ "kind": "choice", "options": [{ "id": "opt-a", "label": "A" }] }),
        );
        step.insert(
            "rules".into(),
            json!([{ "id": "r-a", "on": { "option": "opt-a" }, "outcome": { "kind": "accept" } }]),
        );
    });
    assert_eq!(count(&h, "choice-btn"), 1);
    h.update(act("choose", "opt-a"));
    assert!(h.next_tick().is_some(), "処理中");
    assert_eq!(count(&h, "choice-btn"), 0, "{}", html(&h));
}

#[test]
fn 壊れた宣言は選択肢を出さない() {
    for options in [
        json!([]),
        json!([{ "id": "a", "label": "A" }, { "id": "a", "label": "B" }]),
        json!([{ "id": "a", "label": "" }]),
    ] {
        let h = min3_with(|step| {
            step.insert(
                "input".into(),
                json!({ "kind": "choice", "options": options }),
            );
        });
        assert_eq!(count(&h, "choice-btn"), 0, "{}", html(&h));
    }
    // 方式が choice でない Step（command）は 0 件。
    let h = min3_with(|_| {});
    assert_eq!(count(&h, "choice-btn"), 0);
}

#[test]
fn 選択肢の_id_と表示名はエスケープされる() {
    let h = min3_with(|step| {
        step.insert(
            "input".into(),
            json!({ "kind": "choice", "options": [
                { "id": "x\"><img src=x>", "label": "<script>alert(1)</script>" }
            ]}),
        );
    });
    let out = html(&h);
    assert_eq!(count(&h, "choice-btn"), 1, "{out}");
    assert!(!out.contains("<script>alert"), "{out}");
    assert!(!out.contains("<img src=x>"), "{out}");
    assert!(out.contains("&lt;script&gt;"), "{out}");
}

#[test]
fn ui_action_の入力待ちは宣言の対象の行で受付になり_対象外の行は進まない() {
    // 判定は宣言との照合だけで決まる（core）。入力待ちの `ui_action` に口（選択肢・承認）が出ず、
    // 画面操作の入力（`Input::Ui`）は宣言の対象でだけ受付になる。
    let mut h = min3_with(|step| {
        step.insert(
            "input".into(),
            json!({ "kind": "ui_action", "targets": [{ "view": "v-files", "path": "notes/draft" }] }),
        );
        step.insert(
            "rules".into(),
            json!([{ "id": "r-point", "on": { "ui": { "view": "v-files", "path": "notes/draft" } },
                "outcome": { "kind": "accept" } }]),
        );
        step.insert(
            "confirm".into(),
            json!({ "kind": "none", "reason": "試験" }),
        );
    });
    assert_eq!((ports(&h).0, ports(&h).1), (0, 0));
    h.update(HostAction::Engine(Op::Input(Input::Ui {
        view: "v-files".into(),
        path: "notes/other".into(),
    })));
    settle(&mut h);
    assert_eq!(current(&h), "main/st-1", "対象外の行は進まない");
    h.update(HostAction::Engine(Op::Input(Input::Ui {
        view: "v-files".into(),
        path: "notes/draft".into(),
    })));
    settle(&mut h);
    assert_ne!(current(&h), "main/st-1", "宣言の対象の行で受付になる");
}

#[cfg(feature = "test-hooks")]
#[test]
fn load_fixture_ui_action_の_st_2_は_ui_action_で_行から受付になる() {
    /// 描いた HTML から `data-row="notes/draft"` の `data-payload` を取り出す。
    fn row_payload(h: &EngineHost) -> String {
        let out = html(h);
        let start = out.find("data-row=\"notes/draft\"").expect(&out);
        let tag = &out[out[..start].rfind('<').unwrap()..start + out[start..].find('>').unwrap()];
        tag.split("data-payload=\"")
            .nth(1)
            .and_then(|r| r.split('"').next())
            .unwrap()
            .to_owned()
    }
    let mut h = EngineHost::new();
    h.update(EngineHost::decode_action("raw", "load-fixture:ui-action").unwrap());
    send(&mut h, "/make-note first-note");
    // st-1 の確認は point_at。左の行を指して先へ進む。
    h.update(act("ui", &row_payload(&h)));
    settle(&mut h);
    assert_eq!(current(&h), "main/st-2");
    assert_eq!(status(&h), StepStatus::Entered);
    assert_eq!(
        count(&h, "choice-btn"),
        0,
        "ui_action の Step に選択肢は出ない"
    );
    h.update(act("ui", &row_payload(&h)));
    settle(&mut h);
    assert_eq!(current(&h), "main/st-3", "行を指して受付・達成になる");
}
