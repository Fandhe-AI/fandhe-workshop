//! 候補の一覧の開閉（TASK-440 1/2。TERM-8・TERM-42）をホストを通して確かめる試験。
//! 教材は `min-3step.json`（`help`・`make-note`・`publish-note` と、`unsupported` の `sync-cloud`）。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::Component;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::terminal_area::check_area;

fn host() -> EngineHost {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    let ir = load_ir(&std::fs::read(path).unwrap()).unwrap();
    EngineHost::with_session(Session::new(ir), standard_registry()).unwrap()
}

fn act(h: &mut EngineHost, name: &str, payload: &str) {
    let a = EngineHost::decode_action(name, payload).expect("decoded");
    h.update(a);
}

/// 候補の一覧の `ul` を探し、（hidden か、項目の文字）を返す。
fn list(n: &Node) -> Option<(bool, Vec<String>)> {
    let Node::Element {
        tag,
        attrs,
        children,
    } = n
    else {
        return None;
    };
    if attrs
        .iter()
        .any(|(k, v)| *k == "data-testid" && v == "candidate-list")
    {
        assert_eq!(*tag, "ul");
        let hidden = attrs.iter().any(|(k, _)| *k == "hidden");
        let mut texts = Vec::new();
        collect_buttons(children, &mut texts);
        return Some((hidden, texts));
    }
    children.iter().find_map(list)
}

/// 候補の一覧の `ul` の部分木を取り出す。
fn list_node(n: &Node) -> Option<&Node> {
    let Node::Element {
        attrs, children, ..
    } = n
    else {
        return None;
    };
    if attrs
        .iter()
        .any(|(k, v)| *k == "data-testid" && v == "candidate-list")
    {
        return Some(n);
    }
    children.iter().find_map(list_node)
}

fn collect_buttons(nodes: &[Node], out: &mut Vec<String>) {
    for n in nodes {
        if let Node::Element {
            tag,
            attrs,
            children,
        } = n
        {
            if *tag == "button" {
                assert!(attrs.iter().all(|(k, _)| !k.starts_with("data-action")
                    && *k != "data-payload"
                    && *k != "aria-label"));
                if let Some(Node::Text(t)) = children.first() {
                    out.push(t.clone());
                }
            } else {
                collect_buttons(children, out);
            }
        }
    }
}

fn state(h: &EngineHost) -> (bool, Vec<String>) {
    list(&h.view()).expect("candidate-list")
}

#[test]
fn 接頭辞だけの下書きは_unsupported_を除く全件を名前順に開く() {
    let mut h = host();
    assert_eq!(state(&h), (true, vec![]));
    act(&mut h, "draft", "/");
    assert_eq!(
        state(&h),
        (
            false,
            vec!["/help".into(), "/make-note".into(), "/publish-note".into()]
        )
    );
}

#[test]
fn 開く場面と開かない場面() {
    let mut h = host();
    act(&mut h, "draft", "/he");
    assert_eq!(state(&h), (false, vec!["/help".into()]));
    for d in ["/help", "/zzz", "/he lp", "/s", "he", ""] {
        act(&mut h, "draft", d);
        assert_eq!(state(&h), (true, vec![]), "{d:?}");
    }
}

#[test]
fn esc_は一覧を閉じるだけでログも下書きも変えず_下書きが変われば再び開く() {
    let mut h = host();
    let log = h.model().session().unwrap().log().len();
    act(&mut h, "draft", "/he");
    act(&mut h, "esc", "Escape");
    assert_eq!(state(&h), (true, vec![]));
    assert_eq!(h.model().session().unwrap().log().len(), log);
    act(&mut h, "esc", "Escape");
    assert_eq!(h.model().session().unwrap().log().len(), log);
    act(&mut h, "draft", "/hel");
    assert_eq!(state(&h), (false, vec!["/help".into()]));
    act(&mut h, "esc", "Escape");
    assert_eq!(state(&h), (true, vec![]));
}

#[test]
fn 開閉どちらの状態でも契約に適合する() {
    let mut h = host();
    for step in 0..3 {
        match step {
            1 => act(&mut h, "draft", "/"),
            2 => act(&mut h, "esc", "Escape"),
            _ => {}
        }
        let view = h.view();
        let ul = list_node(&view).expect("candidate-list");
        assert_eq!(check_no_undefined(ul), Ok(()), "step {step}");
        assert_eq!(check_area(ul), Ok(()), "step {step}");
    }
}

#[test]
fn コマンド表の無いホストでは開かない() {
    let mut h = EngineHost::new();
    act(&mut h, "draft", "/he");
    assert_eq!(state(&h), (true, vec![]));
}
