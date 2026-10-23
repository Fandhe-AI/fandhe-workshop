//! 静的検査の本文の選択（`guidance::body_for`）と実行時の木の選択（core の `body_level::select_body`）の
//! 一致の試験（TASK-292、GUIDE-1）。
//!
//! `E-GUIDANCE-LEAK` は `body_for` が選ぶパスの本文を調べる。実行時に別の版が出ると検査が意味を失うため、
//! コンパイルした IR の各 Node × 各水準で、両者が同じ版を選ぶことを確かめる。試験では `unwrap` を使ってよい。

mod common;

use common::Tmp;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::{Ir, Node};
use fandhe_workshop_core::body_level::select_body;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::guidance::body_for;

const WORKSHOP: &str = "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}\n";

fn pack(extra: &str) -> Tmp {
    let t = Tmp::new("body-level-runtime");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - {{id: a, title: A, body: a.md{extra}, next: [{{to: b}}], input: {{kind: choice, options: [{{id: ok, label: OK}}]}}, rules: [{{id: r-a, on: {{option: ok}}, outcome: {{kind: accept}}}}], confirm: {{kind: none, reason: r}}}}\n  - {{id: b, title: B, body: a.md, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    for f in ["a.md", "a.b.md", "a.c.md"] {
        t.put(&format!("chapters/c1/{f}"), format!("# {f}\n").as_bytes());
    }
    t
}

/// `body_for` が選んだパスに対応する木。`body` と同じなら `body_doc`、そうでなければ鍵を逆引きする。
fn expected_tree<'a>(node: &'a Node, path: &str) -> Option<&'a fandhe_workshop_contract::doc::Doc> {
    if node.step.body.as_deref() == Some(path) {
        return node.body_doc.as_ref();
    }
    let key = node
        .step
        .body_levels
        .iter()
        .find(|(_, p)| p.as_str() == path)
        .map(|(k, _)| k)?;
    node.body_levels_doc.get(key)
}

#[test]
fn runtime_selection_agrees_with_the_static_selection() {
    for extra in [
        "",
        ", body_levels: {B: a.b.md}",
        ", body_levels: {C: a.c.md}",
        ", body_levels: {B: a.b.md, C: a.c.md}",
    ] {
        let t = pack(extra);
        let ir: Ir = serde_json::from_slice(&compile_dir(&t.0).unwrap()).unwrap();
        assert!(!ir.nodes.is_empty());
        for node in &ir.nodes {
            for lvl in GuidanceLevel::ALL {
                let picked = select_body(node, lvl).map(|s| s.doc);
                match body_for(&node.step, lvl) {
                    Some(p) => {
                        let want = expected_tree(node, p);
                        assert!(want.is_some(), "{extra} {lvl}: tree missing");
                        assert_eq!(picked, want, "{extra} {} {lvl}", node.uid);
                    }
                    None => assert_eq!(picked, None, "{extra} {} {lvl}", node.uid),
                }
            }
        }
    }
}
