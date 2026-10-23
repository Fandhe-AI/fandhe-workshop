//! 左の成果物の 1 リストの畳み（TASK-484 1/2。FEND-22・ART-14・D-46）。
//!
//! 経路を最後に変えた Step でグループにし、目安（1,000）を超えたときだけ古い Step の行を要約 1 行へ畳むこと、
//! 最新の Step の行は畳まないこと、同じログからは同じ描画になることを確かめる。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::list_policy::{plan_fold_older, FoldEntry};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn ir() -> Ir {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    load_ir(&bytes).unwrap()
}

/// `(Step, 経路の接頭辞, 件数)` の列をログに積んだホスト。
fn host(spec: &[(&str, &str, usize)]) -> EngineHost {
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let mut initial = sample["initial"].clone();
    let mut log = Vec::new();
    for (step, prefix, n) in spec {
        for i in 0..*n {
            log.push(serde_json::json!({"changed": {
                "at": {"generation": 1, "step": step},
                "path": format!("{prefix}/{i:05}"),
                "before": null,
                "after": {"fixture": null, "attrs": {"kind": "none", "state": "created"}},
                "cause": "input",
            }}));
        }
    }
    initial["log"] = serde_json::Value::Array(log);
    let state: SessionState = serde_json::from_value(initial).unwrap();
    EngineHost::with_session(Session::from_state(ir(), state), standard_registry()).unwrap()
}

/// 既定で選ばれるビューの list の本体の HTML。
fn list_body(host: &EngineHost) -> String {
    let html = render_component_html(host);
    let needle = format!("data-view=\"{}\"", host.model().selected_view().as_str());
    let at = html.find(&needle).unwrap();
    let end = at + html[at..].find('>').unwrap();
    let close = end + html[end..].find("</ul>").unwrap();
    html[end + 1..close].to_owned()
}

fn count(s: &str, pat: &str) -> usize {
    s.matches(pat).count()
}

#[test]
fn 目安ちょうどなら畳まない() {
    let body = list_body(&host(&[("main/st-1", "a", 500), ("main/st-2", "b", 500)]));
    assert_eq!(count(&body, "<li"), 1_000);
    assert_eq!(count(&body, "data-fold"), 0);
}

#[test]
fn 目安を超えると古い_step_だけが要約になり新しい_step_は全行残る() {
    let body = list_body(&host(&[("main/st-1", "a", 600), ("main/st-2", "b", 401)]));
    assert!(count(&body, "<li") <= 1_000, "{}", count(&body, "<li"));
    assert_eq!(count(&body, "data-fold=\"step\""), 1);
    assert_eq!(count(&body, "data-fold-rows=\"600\""), 1);
    assert_eq!(count(&body, "data-path=\"b/"), 401);
    assert_eq!(count(&body, "data-path=\"a/"), 0);
    assert_eq!(count(&body, "data-step=\"main/st-1\""), 1);
    assert!(!body.contains("data-list-limit") && !body.contains("data-list-error"));
}

#[test]
fn 要約行は指せず数えられない() {
    let body = list_body(&host(&[("main/st-1", "a", 600), ("main/st-2", "b", 401)]));
    let at = body.find("data-fold=").unwrap();
    let start = body[..at].rfind("<li").unwrap();
    let end = at + body[at..].find('>').unwrap();
    let tag = &body[start..=end];
    for attr in ["data-path", "data-event", "data-payload", "data-action"] {
        assert!(!tag.contains(attr), "{tag}");
    }
}

#[test]
fn 最新の_step_だけなら目安を超えても畳まない() {
    let body = list_body(&host(&[("main/st-1", "a", 1_500)]));
    assert_eq!(count(&body, "data-fold"), 0);
    assert_eq!(count(&body, "data-path=\"a/"), 1_500);
}

#[test]
fn 再描画は同じログから同一になる() {
    let spec = [("main/st-1", "a", 700), ("main/st-2", "b", 700)];
    let a = render_component_html(&host(&spec));
    let b = render_component_html(&host(&spec));
    assert_eq!(a, b);
}

#[test]
fn 古い_step_の経路を新しい_step_で変えるとその経路は新しい側に残る() {
    // st-1 の a/00000..a/00599 のうち先頭 10 件を st-2 で再び変える。
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let mut initial = sample["initial"].clone();
    let ev = |step: &str, p: String| {
        serde_json::json!({"changed": {
            "at": {"generation": 1, "step": step},
            "path": p,
            "before": null,
            "after": {"fixture": null, "attrs": {"kind": "none", "state": "created"}},
            "cause": "input",
        }})
    };
    let mut log: Vec<_> = (0..1_100)
        .map(|i| ev("main/st-1", format!("a/{i:05}")))
        .collect();
    log.extend((0..10).map(|i| ev("main/st-2", format!("a/{i:05}"))));
    initial["log"] = serde_json::Value::Array(log);
    let state: SessionState = serde_json::from_value(initial).unwrap();
    let host =
        EngineHost::with_session(Session::from_state(ir(), state), standard_registry()).unwrap();
    let body = list_body(&host);
    assert!(count(&body, "<li") <= 1_000);
    // 新しい Step で変えた 10 件は畳まれず残る。
    for i in 0..10 {
        assert!(body.contains(&format!("data-path=\"a/{i:05}\"")), "{i}");
    }
    assert_eq!(count(&body, "data-fold=\"step\""), 1);
}

#[test]
fn plan_fold_older_は_head_を返さず古い側だけを畳む() {
    for lens in [
        vec![1_500usize],
        vec![600, 401],
        vec![300, 300, 300, 300],
        vec![1, 1, 2_000],
        vec![5, 900, 5, 5, 900],
    ] {
        let plan = plan_fold_older(&lens, 1_000);
        assert!(plan.iter().all(|e| !matches!(e, FoldEntry::Head { .. })));
        let shown: usize = plan
            .iter()
            .map(|e| match e {
                FoldEntry::Rows { group, skip } => lens[*group] - skip,
                _ => 1,
            })
            .sum();
        let latest = *lens.last().unwrap();
        assert!(shown <= 1_000usize.max(latest + 1), "{lens:?} -> {shown}");
    }
}

/// 指せる行（宛先を持つ行）は古い Step でも畳まれない（畳むと古い経路を指せず Step を達成できない）。
#[test]
fn 指せる行は古い_step_でも畳まれず残る() {
    use fandhe_frontend_core::{el, text};
    use fandhe_workshop_core::event::Event;
    use fandhe_workshop_web::artifact_fold::{fold_rows, ArtifactRow};

    let ev = |step: &str, p: &str| -> Event {
        serde_json::from_value(serde_json::json!({"changed": {
            "at": {"generation": 1, "step": step},
            "path": p,
            "before": null,
            "after": {"fixture": null, "attrs": {"kind": "none", "state": "created"}},
            "cause": "input",
        }}))
        .unwrap()
    };
    let mut log = Vec::new();
    let mut rows: Vec<ArtifactRow> = Vec::new();
    for (step, prefix, n) in [("main/st-1", "a", 600usize), ("main/st-2", "b", 401)] {
        for i in 0..n {
            let path = format!("{prefix}/{i:05}");
            log.push(ev(step, &path));
            // 古い Step の先頭 1 行だけを指せる行にする。
            let pinned = prefix == "a" && i == 0;
            rows.push((
                path.clone(),
                format!("p{path}"),
                el("li", vec![], vec![text(path.as_str())]),
                pinned,
            ));
        }
    }
    let out = fold_rows(&log, rows);
    let keys: Vec<&str> = out.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.contains(&"pa/00000"), "指せる行が畳まれた");
    assert!(!keys.contains(&"pa/00001"));
    assert!(keys.contains(&"pb/00400"));
    assert!(out.len() <= 1_000, "{}", out.len());
}
