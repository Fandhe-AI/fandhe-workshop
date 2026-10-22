//! 本文と練習データの木が IR に載ることの結合試験（TASK-60 の 1/2・TASK-61、04-behavior の PACK-26・SAFE-37・ART-18）。
//!
//! `compile_dir` が、Step の本文（`body`・`body_levels`）と練習データの内容を安全な木にして IR（版 2）へ載せ、
//! その IR を `load_ir` が読めることを、実パック（`testdata/`・`tests/data/`）と一時ディレクトリの
//! パックで確かめる。深い入れ子は、IR の JSON の入れ子の上限を超える「書き出せたが読めない IR」を
//! 作らないことも確かめる。一時ディレクトリは `CARGO_TARGET_TMPDIR` の下に作り、作業ツリーは書き換えない。
//! 試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use common::{testdata, Tmp, WORKSHOP};
use fandhe_workshop_contract::doc::{Block, Doc, TREE_VERSION};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::{check_ir_limits, load_ir};
use fandhe_workshop_pack::compile::compile_dir;

fn data(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(rel)
}

fn packs() -> Vec<(&'static str, PathBuf)> {
    vec![
        (
            "min-3step-pack",
            testdata("engine-event-consistency/test-data/min-3step-pack"),
        ),
        ("ch1-pack", testdata("input-field-ime/test-data/ch1-pack")),
        ("ime-pack", testdata("input-field-ime/test-data/ime-pack")),
        ("min-3step-pack-b", data("min-3step-pack-b")),
        ("fixture-pack", data("fixture-pack")),
    ]
}

fn compile(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).expect("compiles");
    load_ir(&bytes).expect("the engine loads the IR")
}

fn json_of(doc: &Doc) -> String {
    serde_json::to_string(doc).unwrap()
}

#[test]
fn every_step_body_and_level_has_a_tree() {
    for (name, dir) in packs() {
        let ir = compile(&dir);
        assert_eq!((ir.format_version, ir.pack.ir_format), (2, 2), "{name}");
        let mut with_body = 0;
        for n in &ir.nodes {
            assert_eq!(
                n.step.body.is_some(),
                n.body_doc.is_some(),
                "{name} {}",
                n.uid
            );
            if let Some(d) = &n.body_doc {
                assert_eq!(d.version, TREE_VERSION, "{name} {}", n.uid);
                with_body += 1;
            }
            let levels: BTreeSet<_> = n.step.body_levels.keys().collect();
            let level_docs: BTreeSet<_> = n.body_levels_doc.keys().collect();
            assert_eq!(levels, level_docs, "{name} {}", n.uid);
            for d in n.body_levels_doc.values() {
                assert_eq!(d.version, TREE_VERSION, "{name} {}", n.uid);
            }
        }
        assert!(with_body > 0, "{name}: no body tree");
    }
}

#[test]
fn fixture_docs_follow_the_extension() {
    let ir = compile(&data("fixture-pack"));
    assert_eq!(
        ir.fixture_docs.keys().collect::<Vec<_>>(),
        ir.fixtures.keys().collect::<Vec<_>>()
    );
    // `.md` は Markdown の木（見出しとリスト）。
    let md = &ir.fixture_docs["f-doc"];
    assert!(matches!(md.blocks[0], Block::Heading { level: 1, .. }));
    assert!(md.blocks.iter().any(|b| matches!(b, Block::List { .. })));
    // `.txt` は解釈せず、全文が 1 つのコードブロックになる（`#` が見出しにならない）。
    let raw = std::fs::read_to_string(data("fixture-pack/fixtures/run.txt")).unwrap();
    assert_eq!(
        ir.fixture_docs["f-log"].blocks,
        vec![Block::Code {
            lang: None,
            text: raw
        }]
    );
}

#[test]
fn body_tree_has_no_html_kind_and_no_null() {
    let ir = compile(&data("fixture-pack"));
    let a = ir.nodes[0].body_doc.as_ref().unwrap();
    let j = json_of(a);
    // 生の `<script>` は、文字列として段落に入り、木の種別にはならない。
    assert!(j.contains("<script>alert(1)</script>"));
    assert!(!j.contains("\"t\":\"script\"") && !j.contains("\"t\":\"html\""));
    assert!(!j.contains("null"));
    // 日本語の行の途中の改行は連結し、英数字の段落の改行は残す（TASK-61・PACK-27）。
    assert!(j.contains("次の行です。日本語の\"},{\"t\":\"text\",\"text\":\"行の途中"));
    assert!(!j.contains("日本語の\"},{\"t\":\"text\",\"text\":\"\\n\""));
    assert!(j.contains(
        "alpha line\"},{\"t\":\"text\",\"text\":\"\\n\"},{\"t\":\"text\",\"text\":\"beta line"
    ));
    // リンクの URL は検査せずに入れる。
    assert!(j.contains("\"href\":\"https://example.com/\""));
    // 本文のある Step の水準別の本文。
    assert!(ir.nodes[0].body_levels_doc.contains_key("B"));
}

/// 入れ子が深い本文でも、compile は成功し、出力は `check_ir_limits` と `load_ir` を通る。
#[test]
fn deeply_nested_body_compiles_to_a_loadable_ir() {
    let bodies = [
        format!("{}x\n", "> ".repeat(1000)),
        format!(
            "{}x\n",
            (0..1000)
                .map(|i| format!("{}- ", " ".repeat(i % 3)))
                .collect::<String>()
        ),
        format!("{}x{}\n", "*a ".repeat(1000), "*".repeat(1000)),
        format!("{}x\n", "1. ".repeat(500)),
        format!("{}x\n", "> - ".repeat(500)),
    ];
    for (i, body) in bodies.iter().enumerate() {
        let t = Tmp::new("deep");
        t.put("workshop.yaml", WORKSHOP.as_bytes());
        t.put(
            "chapters/c1/chapter.yaml",
            concat!(
                "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
                "  - {id: a, title: A, body: a.md, body_levels: {B: a.md}, next: [{to: b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n",
                "  - {id: b, title: B, final: true}\n",
            )
            .as_bytes(),
        );
        t.put("chapters/c1/a.md", body.as_bytes());
        let bytes = compile_dir(&t.0).unwrap_or_else(|_| panic!("case {i} must compile"));
        check_ir_limits(&bytes).unwrap_or_else(|_| panic!("case {i}: limits"));
        load_ir(&bytes).unwrap_or_else(|e| panic!("case {i}: {e}"));
    }
}

/// 表のセルや見出しに深い強調を入れた最悪の形でも、同じく読める。
#[test]
fn deeply_nested_table_cell_compiles_to_a_loadable_ir() {
    let cell = format!("{}x{}", "*a ".repeat(200), "*".repeat(200));
    let body = format!("| h |\n|---|\n| {cell} |\n\n> | h |\n> |---|\n> | {cell} |\n");
    let t = Tmp::new("deep-table");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        concat!(
            "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
            "  - {id: a, title: A, body: a.md, next: [{to: b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n",
            "  - {id: b, title: B, final: true}\n",
        )
        .as_bytes(),
    );
    t.put("chapters/c1/a.md", body.as_bytes());
    let bytes = compile_dir(&t.0).expect("compiles");
    load_ir(&bytes).expect("loads");
}
