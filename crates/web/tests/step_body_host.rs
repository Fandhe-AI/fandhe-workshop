//! Step の題・`do`・本文（TASK-525 1/2。SHELL-7）の host の層の試験。
//!
//! 期待値は IR から試験の中で独自に引き（`step_body` を呼ばない）、画面の文字・要素と照らす。
//! 本文の木を持つ教材は、`tests/data/ch0_3/ir.json` の JSON に `body_doc` などを差し込んで作る
//! （元の IR は変えない）。ブラウザでの CSP あり・なしの確認は `tools/browser/step-body.test.mjs` の担当である。
//! 試験では `unwrap` を使ってよい。失敗の文面には教材由来の値を出さない。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::DirtyTracked;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::shell;
use fandhe_workshop_web::step_body::STEP_BODY_FIELD;
use serde_json::{json, Value};

const UID: &str = "main/st-1-3";

fn ir_json() -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch0_3/ir.json");
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

/// 本文の木は IR の形式 2 の欄なので、差し込んだ変種は形式 2 として読む。
fn load(v: &Value) -> Ir {
    let mut v = v.clone();
    v["format_version"] = json!(2);
    v["pack"]["ir_format"] = json!(2);
    let v = &v;
    load_ir(&serde_json::to_vec(v).unwrap()).unwrap()
}

fn state_at(uid: &str, level: GuidanceLevel) -> SessionState {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/contract/event-sequence.json");
    let s: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let text = serde_json::to_string(&s["initial"])
        .unwrap()
        .replace("\"main/st-1\"", &format!("\"{uid}\""));
    let mut st: SessionState = serde_json::from_str(&text).unwrap();
    st.run.guidance_level = level;
    st
}

fn host(ir: Ir, uid: &str, level: GuidanceLevel) -> EngineHost {
    EngineHost::with_session(
        Session::from_state(ir, state_at(uid, level)),
        standard_registry(),
    )
    .unwrap()
}

fn attr<'a>(n: &'a Node, name: &str) -> Option<&'a str> {
    match n {
        Node::Element { attrs, .. } => attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str()),
        _ => None,
    }
}

fn kids(n: &Node) -> &[Node] {
    match n {
        Node::Element { children, .. } => children,
        _ => &[],
    }
}

fn find<'a>(n: &'a Node, testid: &str, out: &mut Vec<&'a Node>) {
    if attr(n, "data-testid") == Some(testid) {
        out.push(n);
    }
    for c in kids(n) {
        find(c, testid, out);
    }
}

fn only<'a>(n: &'a Node, testid: &str) -> Option<&'a Node> {
    let mut v = Vec::new();
    find(n, testid, &mut v);
    assert!(v.len() <= 1, "{testid} は多くて 1 個");
    v.pop()
}

fn text_of(n: &Node) -> String {
    match n {
        Node::Text(t) => t.clone(),
        _ => kids(n).iter().map(text_of).collect(),
    }
}

fn count_tag(n: &Node, tag: &str) -> usize {
    let own = matches!(n, Node::Element { tag: t, .. } if *t == tag) as usize;
    own + kids(n).iter().map(|c| count_tag(c, tag)).sum::<usize>()
}

fn raw_html(n: &Node) -> usize {
    let own = matches!(n, Node::RawHtml { .. }) as usize;
    own + kids(n).iter().map(raw_html).sum::<usize>()
}

fn assert_frame_ok(v: &Node) {
    assert_eq!(shell::check_no_undefined(v), Ok(()));
    assert_eq!(a11y::check_no_undefined(v), Ok(()));
}

fn doc(t: &str) -> Value {
    json!({"version":1,"blocks":[{"t":"paragraph","children":[{"t":"text","text":t}]}]})
}

fn set_node(v: &mut Value, uid: &str, f: impl FnOnce(&mut Value)) {
    let n = v["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["uid"] == uid)
        .unwrap();
    f(n);
}

#[test]
fn 全_step_で題と_do_が_ir_のとおり出る() {
    let v = ir_json();
    let ir = load(&v);
    for n in v["nodes"].as_array().unwrap() {
        let uid = n["uid"].as_str().unwrap();
        if n["scope"] != "main" {
            continue;
        }
        let view = host(ir.clone(), uid, GuidanceLevel::A).view();
        assert_frame_ok(&view);
        let title = only(&view, "step-title").expect("題が出る");
        assert_eq!(text_of(title), n["step"]["title"].as_str().unwrap());
        let d = only(&view, "step-do").expect("do が出る");
        let want = n["step"]["do"].as_str().unwrap();
        assert_eq!(text_of(d), format!("いまやること{want}"));
        // この IR は本文の木を持たないので、本文の項目は出ない。
        assert!(only(&view, "step-body").is_none());
    }
}

#[test]
fn 章の最初の_step_でも題と_do_が出る() {
    let ir = load(&ir_json());
    assert!(!ir.starts.is_empty());
    for s in &ir.starts {
        let view = host(ir.clone(), &s.node, GuidanceLevel::A).view();
        assert!(only(&view, "step-title").is_some(), "開始点 {}", s.id);
        assert!(only(&view, "step-do").is_some(), "開始点 {}", s.id);
    }
}

#[test]
fn 本文は見出し_一覧_表の構造を保ち外部の画像は出ない() {
    let mut v = ir_json();
    let body = json!({"version":1,"blocks":[
        {"t":"heading","level":1,"children":[{"t":"text","text":"大"}]},
        {"t":"heading","level":2,"children":[{"t":"text","text":"小"}]},
        {"t":"list","ordered":false,"items":[
            [{"t":"paragraph","children":[{"t":"text","text":"a"}]}],
            [{"t":"paragraph","children":[{"t":"text","text":"b"}]}]]},
        {"t":"list","ordered":true,"items":[
            [{"t":"paragraph","children":[{"t":"text","text":"c"}]}]]},
        {"t":"table","head":[[{"t":"text","text":"x"}],[{"t":"text","text":"y"}],[{"t":"text","text":"z"}]],
            "rows":[[[{"t":"text","text":"1"}],[{"t":"text","text":"2"}],[{"t":"text","text":"3"}]]]},
        {"t":"code","lang":"sh","text":"ls"},
        {"t":"quote","children":[{"t":"paragraph","children":[{"t":"text","text":"q"}]}]},
        {"t":"paragraph","children":[
            {"t":"link","href":"https://example.com/","children":[{"t":"text","text":"L"}]},
            {"t":"image","src":"img/a.png","alt":"内"},
            {"t":"image","src":"https://evil.example/x.png","alt":"外"}]}
    ]});
    set_node(&mut v, UID, |n| n["body_doc"] = body);
    let view = host(load(&v), UID, GuidanceLevel::A).view();
    assert_frame_ok(&view);
    let b = only(&view, "step-body").expect("本文が出る");
    assert_eq!(attr(b, "data-body-level"), Some("A"));
    assert_eq!(count_tag(b, "h1"), 1);
    assert_eq!(count_tag(b, "h2"), 1);
    assert_eq!(count_tag(b, "ul"), 1);
    assert_eq!(count_tag(b, "ol"), 1);
    assert_eq!(count_tag(b, "li"), 3);
    assert_eq!(count_tag(b, "table"), 1);
    assert_eq!(count_tag(b, "th"), 3);
    assert_eq!(count_tag(b, "td"), 3);
    assert_eq!(count_tag(b, "a"), 1);
    // 同梱の画像だけが img になり、外部の画像は代替の文字になる。
    assert_eq!(count_tag(b, "img"), 1);
    assert!(text_of(b).contains('外'));
    assert_eq!(raw_html(&view), 0);
}

#[test]
fn 題と_do_の中の記号は文字のまま出る() {
    let mut v = ir_json();
    set_node(&mut v, UID, |n| {
        n["step"]["title"] = json!("<img src=x onerror=1>");
        n["step"]["do"] = json!("<script>1</script>");
    });
    let view = host(load(&v), UID, GuidanceLevel::A).view();
    assert_frame_ok(&view);
    assert_eq!(
        text_of(only(&view, "step-title").unwrap()),
        "<img src=x onerror=1>"
    );
    assert_eq!(count_tag(&view, "script"), 0);
    assert_eq!(raw_html(&view), 0);
}

fn levelled() -> Ir {
    let mut v = ir_json();
    set_node(&mut v, UID, |n| {
        n["body_doc"] = doc("本文A");
        n["body_levels_doc"] = json!({"B": doc("本文B"), "C": doc("本文C"), "A": doc("無視")});
        n["step"]["do"] = json!("doA");
        n["step"]["do_levels"] = json!({"B": "doB", "C": "doC"});
    });
    load(&v)
}

fn shown(h: &EngineHost) -> (String, Option<String>, String) {
    let view = h.view();
    let b = only(&view, "step-body").expect("本文");
    (
        text_of(b),
        attr(b, "data-body-level").map(str::to_owned),
        text_of(only(&view, "step-do").expect("do")),
    )
}

#[test]
fn 案内レベルに応じた本文と_do_が出て欠けた水準は濃い側へ倒れる() {
    let (t, l, d) = shown(&host(levelled(), UID, GuidanceLevel::C));
    assert_eq!(
        (t.as_str(), l.as_deref(), d.as_str()),
        ("本文C", Some("C"), "いまやることdoC")
    );
    let (t, l, d) = shown(&host(levelled(), UID, GuidanceLevel::B));
    assert_eq!(
        (t.as_str(), l.as_deref(), d.as_str()),
        ("本文B", Some("B"), "いまやることdoB")
    );
    let (t, l, d) = shown(&host(levelled(), UID, GuidanceLevel::A));
    assert_eq!(
        (t.as_str(), l.as_deref(), d.as_str()),
        ("本文A", Some("A"), "いまやることdoA")
    );

    // B が無いと B は A に倒れ、C だけが無いと C は B に倒れる。
    let mut v = ir_json();
    set_node(&mut v, UID, |n| {
        n["body_doc"] = doc("本文A");
        n["step"]["do"] = json!("doA");
        n["step"]["do_levels"] = json!({"B": "doB"});
    });
    let (t, l, d) = shown(&host(load(&v), UID, GuidanceLevel::B));
    assert_eq!(
        (t.as_str(), l.as_deref(), d.as_str()),
        ("本文A", Some("A"), "いまやることdoB")
    );
    let (t, l, d) = shown(&host(load(&v), UID, GuidanceLevel::C));
    assert_eq!(
        (t.as_str(), l.as_deref(), d.as_str()),
        ("本文A", Some("A"), "いまやることdoB")
    );
    set_node(&mut v, UID, |n| {
        n["body_levels_doc"] = json!({"B": doc("本文B")});
    });
    let (t, l, _) = shown(&host(load(&v), UID, GuidanceLevel::C));
    assert_eq!((t.as_str(), l.as_deref()), ("本文B", Some("B")));
}

#[test]
fn 表示が変わらない更新では_dirty_にならない() {
    let mut h = host(levelled(), UID, GuidanceLevel::A);
    let _ = h.dirty_fields();
    h.update(HostAction::SelectPane(a11y::PaneTab::ALL[0]));
    assert!(!h.dirty_fields().contains(&STEP_BODY_FIELD.name));
}

#[test]
fn 履歴を開くと_dirty_になり復元した_step_の題が出る() {
    use fandhe_workshop_core::history_envelope::build_envelope;
    use fandhe_workshop_core::history_export::export_bytes;
    use fandhe_workshop_web::host::ImportOutcome;

    let ir = load(&ir_json());
    let env = build_envelope(
        &ir,
        &ir.starts[0].id,
        vec![],
        state_at("main/st-1-4", GuidanceLevel::A),
    )
    .unwrap();
    let bytes = export_bytes(&env).unwrap();
    let mut h = host(ir.clone(), "main/st-1-4", GuidanceLevel::A);
    let _ = h.dirty_fields();
    let outcome = h.open_history(&bytes);
    assert!(
        !matches!(outcome, ImportOutcome::Refused(_)),
        "前提: 履歴が開く"
    );
    assert!(h.dirty_fields().contains(&STEP_BODY_FIELD.name));
    let want = ir
        .nodes
        .iter()
        .find(|n| n.uid == "main/st-1-4")
        .unwrap()
        .step
        .title
        .clone();
    assert_eq!(text_of(only(&h.view(), "step-title").unwrap()), want);
}

#[test]
fn 教材が無いホストは題を出さず枠の適合を保つ() {
    let v = EngineHost::new().view();
    assert_frame_ok(&v);
    assert!(only(&v, "step-title").is_none());
}
