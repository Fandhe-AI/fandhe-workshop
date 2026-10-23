//! 承認のボタンと対象つきの「直す」（TASK-447 1/2。TERM-27）の試験。
//!
//! 確かめるのは、(1) 規則に書かれた対象の数だけ対象つきのボタンが出ること、(2) 対象なしの「直す」は core の判定で
//! 拒否になり次の行動が出てボタンが残ること、(3) 受付の後は確認の段の 3 つに切り替わること、(4) 出さない条件
//! （fail-closed）、(5) 教材由来の対象が文字ノード・属性値のエスケープに通ること、の 5 点である。
//! ホストは判定も「いま出ているボタンか」の照合もしない。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::el;
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::offers::{approval_button, approval_offers};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::terminal_area::check_area;
use serde_json::{json, Value};

const NEXT: &str = "公開してよいかをもう一度決めます。";

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn rule(id: &str, on: Value, kind: &str) -> Value {
    if kind == "accept" {
        json!({ "id": id, "on": on, "outcome": { "kind": "accept" } })
    } else {
        json!({ "id": id, "on": on, "outcome": { "kind": "reject", "rejection": "rj-decline" } })
    }
}

fn thin_rules(targets: &[&str]) -> Vec<Value> {
    let mut v: Vec<Value> = targets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            rule(
                &format!("r-t{i}"),
                json!({ "approval": "revise", "target": t }),
                "accept",
            )
        })
        .collect();
    v.push(rule("r-bare", json!({ "approval": "revise" }), "reject"));
    v.push(rule("r-early", json!({ "approval": "approve" }), "reject"));
    v
}

/// `main/st-1` の入力・確認・規則を差し替えたホスト。
fn host_with(confirm: Option<Value>, rules: Vec<Value>, input: Value) -> EngineHost {
    let p = testdata().join("engine-event-consistency/test-data/min-3step.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let step = v["nodes"][0]["step"].as_object_mut().unwrap();
    step.insert("input".into(), input);
    step.insert("rules".into(), Value::Array(rules));
    match confirm {
        Some(c) => step.insert("confirm".into(), c),
        None => step.remove("confirm"),
    };
    let ir: Ir = load_ir(&serde_json::to_vec(&v).unwrap()).unwrap();
    let p = testdata().join("contract/event-sequence.json");
    let s: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let state: SessionState = serde_json::from_value(s["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn approval_host(targets: &[&str]) -> EngineHost {
    host_with(
        Some(json!({ "kind": "approve" })),
        thin_rules(targets),
        json!({ "kind": "approval" }),
    )
}

fn click_decide(h: &mut EngineHost, v: &str) {
    h.update(EngineHost::decode_action("decide", v).unwrap());
}

fn click_target(h: &mut EngineHost, t: &str) {
    h.update(EngineHost::decode_action("revise-target", t).unwrap());
}

fn drain(h: &mut EngineHost) {
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}

fn html(h: &EngineHost) -> String {
    render_component_html(h)
}

fn buttons(h: &EngineHost) -> usize {
    html(h).matches("data-testid=\"approval-btn\"").count()
}

fn target_buttons(h: &EngineHost) -> usize {
    html(h).matches("data-revise-target=").count()
}

fn rejected_rows(h: &EngineHost) -> usize {
    html(h).matches("data-kind=\"rejected\"").count()
}

fn offers_len(h: &EngineHost) -> usize {
    approval_offers(h.model().session().unwrap()).len()
}

#[test]
fn decide_は三つの値だけを_approval_の_op_にする() {
    for (v, d) in [
        ("approve", Decision::Approve),
        ("reject", Decision::Reject),
        ("revise", Decision::Revise),
    ] {
        assert!(matches!(
            EngineHost::decode_action("decide", v),
            Some(HostAction::Engine(Op::Input(Input::Approval { decision, target: None }))) if decision == d
        ));
    }
    for bad in ["", "Approve", "approve ", "approval", "x"] {
        assert!(
            EngineHost::decode_action("decide", bad).is_none(),
            "{bad:?}"
        );
    }
}

#[test]
fn revise_target_は対象をそのまま_op_にし_空は何もしない() {
    assert!(matches!(
        EngineHost::decode_action("revise-target", "a:b c"),
        Some(HostAction::Engine(Op::Input(Input::Approval { decision: Decision::Revise, target: Some(t) }))) if t == "a:b c"
    ));
    assert!(EngineHost::decode_action("revise-target", "").is_none());
}

#[test]
fn 入力の段は三つと対象の数だけのボタンが出る() {
    let h = approval_host(&["t-alpha", "t-beta"]);
    assert_eq!(buttons(&h), 5, "{}", html(&h));
    assert_eq!(target_buttons(&h), 2);
    let out = html(&h);
    assert!(out.contains("直す（対象: t-alpha）"), "{out}");
    assert!(out.contains("data-revise-target=\"t-beta\""), "{out}");
    for w in ["はい", "いいえ"] {
        assert!(out.contains(&format!(">{w}<")), "{out}");
    }
}

#[test]
fn ボタンは右のエリアの契約に通る() {
    let area = el(
        "div",
        vec![],
        vec![
            approval_button(Decision::Approve, None),
            approval_button(Decision::Reject, None),
            approval_button(Decision::Revise, None),
            approval_button(Decision::Revise, Some(("t-alpha", "表示"))),
        ],
    );
    check_area(&area).unwrap_or_else(|e| panic!("{e:?}"));
}

#[test]
fn 対象の重複は一つにまとまる() {
    let h = approval_host(&["same", "same"]);
    assert_eq!(target_buttons(&h), 1);
    assert_eq!(buttons(&h), 4);
}

#[test]
fn 太い形の規則でも対象つきのボタンが出る() {
    let wide = |approval: &str, target: Value, kind: &str| {
        rule(
            "r",
            json!({ "command": null, "missing": [], "args": {}, "option": null,
                    "approval": approval, "target": target, "ui": null, "text": null, "cancel": null }),
            kind,
        )
    };
    let h = host_with(
        Some(json!({ "kind": "approve" })),
        vec![
            wide("revise", json!("t-alpha"), "accept"),
            wide("revise", Value::Null, "reject"),
            wide("approve", Value::Null, "reject"),
        ],
        json!({ "kind": "approval" }),
    );
    assert_eq!(target_buttons(&h), 1, "{}", html(&h));
    assert_eq!(buttons(&h), 4);
}

#[test]
fn 対象なしの直すは拒否になり_次の行動が出てボタンが残る() {
    let mut h = approval_host(&["t-alpha", "t-beta"]);
    click_decide(&mut h, "revise");
    assert_eq!(rejected_rows(&h), 1, "{}", html(&h));
    assert!(html(&h).contains(NEXT), "{}", html(&h));
    assert_eq!(buttons(&h), 5, "押し直せる");
    click_decide(&mut h, "revise");
    assert_eq!(rejected_rows(&h), 2);
}

#[test]
fn 早すぎる承認は拒否になり_宣言に無い対象は_core_が拒否に倒す() {
    let mut h = approval_host(&["t-alpha"]);
    click_decide(&mut h, "approve");
    assert_eq!(rejected_rows(&h), 1);
    click_target(&mut h, "t-forged");
    assert_eq!(rejected_rows(&h), 2, "{}", html(&h));
    assert_eq!(buttons(&h), 4);
}

#[test]
fn 対象つきの直すは受付になり_確認の段の三つに切り替わる() {
    let mut h = approval_host(&["t-alpha", "t-beta"]);
    click_target(&mut h, "t-alpha");
    assert_eq!(rejected_rows(&h), 0, "{}", html(&h));
    drain(&mut h);
    assert_eq!(buttons(&h), 3, "{}", html(&h));
    assert_eq!(target_buttons(&h), 0);
}

#[test]
fn confirm_が無い_step_の確認待ちも三つ出る() {
    let mut h = host_with(
        None,
        thin_rules(&["t-alpha"]),
        json!({ "kind": "approval" }),
    );
    click_target(&mut h, "t-alpha");
    drain(&mut h);
    assert_eq!(buttons(&h), 3, "{}", html(&h));
}

#[test]
fn 承認でない_step_や確認が_point_at_のときは出ない() {
    let h = host_with(
        Some(json!({ "kind": "approve" })),
        thin_rules(&["t-alpha"]),
        json!({ "kind": "text" }),
    );
    assert_eq!(offers_len(&h), 0);
    let mut h = host_with(
        Some(json!({ "kind": "point_at", "view": "v", "path": "p" })),
        thin_rules(&["t-alpha"]),
        json!({ "kind": "approval" }),
    );
    click_target(&mut h, "t-alpha");
    drain(&mut h);
    assert_eq!(buttons(&h), 0, "{}", html(&h));
}

#[test]
fn 宣言が読めないときは出ない() {
    let h = host_with(
        Some(json!({ "kind": "approve" })),
        vec![rule(
            "r",
            json!({ "approval": "revise", "target": 1 }),
            "accept",
        )],
        json!({ "kind": "approval" }),
    );
    assert_eq!(buttons(&h), 0);
}

#[test]
fn 教材由来の対象は逃がされ_空白だけの対象はボタンにならない() {
    let evil = "<img src=x onerror=alert(1)>";
    let h = approval_host(&[evil, "   ", "ok"]);
    let out = html(&h);
    assert!(!out.contains("<img"), "{out}");
    assert!(out.contains("&lt;img"), "{out}");
    assert_eq!(target_buttons(&h), 2, "{out}");
}

/// `main/st-1` に「直す」の候補（宣言順）と、規則の対象 `t-alpha`・`t-beta` を持たせたホスト。
fn candidates_host(candidates: Value) -> EngineHost {
    host_with(
        Some(json!({ "kind": "approve" })),
        thin_rules(&["t-alpha", "t-beta"]),
        json!({ "kind": "approval", "revise": { "candidates": candidates } }),
    )
}

fn three_candidates() -> Value {
    json!([
        { "id": "t-alpha", "label": "冒頭の見出し" },
        { "id": "t-gamma", "label": "中ほどの図表" },
        { "id": "t-beta" }
    ])
}

#[test]
fn 候補の数だけ対象つきのボタンが出て_文字は表示名で_id_は属性だけに出る() {
    let h = candidates_host(three_candidates());
    let out = html(&h);
    assert_eq!(target_buttons(&h), 3, "規則の対象数（2）より多い: {out}");
    assert_eq!(buttons(&h), 6, "{out}");
    let a = out.find("直す（対象: 冒頭の見出し）").expect(&out);
    let g = out.find("直す（対象: 中ほどの図表）").expect(&out);
    let b = out
        .find("直す（対象: t-beta）")
        .expect("label なしは ID が文字");
    assert!(a < g && g < b, "宣言順: {out}");
    for id in ["t-alpha", "t-gamma"] {
        assert!(
            out.contains(&format!("data-revise-target=\"{id}\"")),
            "{out}"
        );
        assert!(
            !out.contains(&format!("対象: {id}")),
            "label ありは ID を文字にしない: {out}"
        );
    }
    assert!(out.contains("data-payload=\"t-gamma\""), "{out}");
}

#[test]
fn 誤った候補の押下は拒否になりボタンが残り_正しい対象は受付になる() {
    let mut h = candidates_host(three_candidates());
    click_target(&mut h, "t-gamma");
    assert_eq!(rejected_rows(&h), 1, "{}", html(&h));
    assert_eq!(buttons(&h), 6, "押し直せる");
    assert_eq!(offers_len(&h), 6);
    click_target(&mut h, "t-alpha");
    drain(&mut h);
    assert_eq!(rejected_rows(&h), 1, "{}", html(&h));
    assert_eq!(buttons(&h), 3, "{}", html(&h));
}

#[test]
fn 候補の表示名は文字ノードにだけ出て逃がされる() {
    let h = candidates_host(json!([
        { "id": "t-alpha", "label": "<img src=x onerror=alert(1)>" },
        { "id": "t-beta", "label": "ok" }
    ]));
    let out = html(&h);
    assert!(!out.contains("<img"), "{out}");
    assert!(out.contains("&lt;img"), "{out}");
    assert_eq!(target_buttons(&h), 2);
}

#[test]
fn 宣言が検査に通らない候補では一つも出ない() {
    let h = candidates_host(json!([]));
    assert_eq!(buttons(&h), 0, "{}", html(&h));
    let h = candidates_host(json!([{ "id": "t-alpha", "label": "   " }]));
    assert_eq!(buttons(&h), 0, "{}", html(&h));
}

#[cfg(feature = "test-hooks")]
mod load_fixture {
    use super::*;
    use fandhe_workshop_web::host::{FixtureName, RAW_LOAD_FIXTURE};

    #[test]
    fn 差し替えると入力待ちで三つと対象つきの二つが出る() {
        let mut h = EngineHost::new();
        let action =
            EngineHost::decode_action("raw", &format!("{RAW_LOAD_FIXTURE}approval")).unwrap();
        assert!(matches!(
            action,
            HostAction::LoadFixture(FixtureName::Approval)
        ));
        h.update(action);
        assert_eq!(buttons(&h), 5, "{}", html(&h));
        assert_eq!(target_buttons(&h), 2);
        click_decide(&mut h, "revise");
        assert_eq!(rejected_rows(&h), 1);
        click_target(&mut h, "t-alpha");
        drain(&mut h);
        assert_eq!(buttons(&h), 3, "{}", html(&h));
    }

    #[test]
    fn 候補つきの差し替えは候補の数だけ表示名で出る() {
        let mut h = EngineHost::new();
        let action =
            EngineHost::decode_action("raw", &format!("{RAW_LOAD_FIXTURE}approval-candidates"))
                .unwrap();
        assert!(matches!(
            action,
            HostAction::LoadFixture(FixtureName::ApprovalCandidates)
        ));
        h.update(action);
        assert_eq!(target_buttons(&h), 3, "{}", html(&h));
        assert!(html(&h).contains("直す（対象: 中ほどの図表）"));
        click_target(&mut h, "t-gamma");
        assert_eq!(rejected_rows(&h), 1);
        click_target(&mut h, "t-beta");
        drain(&mut h);
        assert_eq!(buttons(&h), 3, "{}", html(&h));
    }
}
