//! 文書ビューの練習データの本文の表示（ART-18・TASK-489 1/2。D-50・D-51）の host の層の試験。
//!
//! 試験用の教材（`tests/data/doc_body_pack`）を実際に compile し、製品の起動と同じ入口（`Session::entered_at`）で
//! 開いて承認を送り、左エリアの文書ビューの行に本文が出ること、`body` を宣言しない写しでは出ないことを確かめる。
//! 期待値は IR の `fixture_docs` から試験の中で独自に引く（`body_doc` は呼ばない）。
//! ブラウザでの CSP あり・なしの確認は `tools/browser/doc-body.test.mjs` の担当である。
//! 試験では `unwrap` を使ってよい。失敗の文面には教材由来の値を出さない。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;

const BODY: &str = "data-testid=\"artifact-doc-body\"";

fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/doc_body_pack")
}

/// 一意な一時ディレクトリの教材の写し。Drop で消す。
struct Tmp(PathBuf);

impl Tmp {
    /// 写しを作り、文書ビューの宣言 `{kind: document, body: shown}` を `replacement` に置き換える。
    fn variant(replacement: &str) -> Tmp {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "doc-body-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&p);
        copy(&pack_dir(), &p);
        let y = p.join("workshop.yaml");
        let text = fs::read_to_string(&y).unwrap();
        let from = "{kind: document, body: shown}";
        assert!(text.contains(from));
        fs::write(&y, text.replace(from, replacement)).unwrap();
        Tmp(p)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &dst);
        } else {
            fs::copy(e.path(), dst).unwrap();
        }
    }
}

fn ir_of(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).unwrap_or_else(|d| panic!("compile: {} 件の診断", d.len()));
    load_ir(&bytes).expect("load_ir")
}

fn host(ir: Ir) -> EngineHost {
    let node = ir.starts[0].node.clone();
    EngineHost::with_session(Session::entered_at(ir, &node), standard_registry()).unwrap()
}

fn approve(h: &mut EngineHost) {
    h.update(EngineHost::decode_action("decide", "approve").unwrap());
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

/// 文書ビューの `notes/draft` の行の html（`<li` の続きから `</li>` の手前まで）。
fn draft_row(h: &EngineHost) -> Option<String> {
    html(h)
        .split("<li")
        .skip(1)
        .find(|s| s.contains("data-path=\"notes/draft\""))
        .map(|s| s.split("</li>").next().unwrap().to_owned())
}

/// 練習データの木の最初の文字。期待値を IR から独自に引く。
fn first_text(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Object(m) => {
            if let Some(s) = m.get("text").and_then(|t| t.as_str()) {
                return Some(s.to_owned());
            }
            m.values().find_map(first_text)
        }
        serde_json::Value::Array(a) => a.iter().find_map(first_text),
        _ => None,
    }
}

fn heading_of(ir: &Ir, id: &str) -> String {
    first_text(&serde_json::to_value(&ir.fixture_docs[id]).unwrap()).expect("見出しの文字がある")
}

#[test]
fn 承認の前は本文が無く_承認のたびに行の本文が練習データの内容になる() {
    let ir = ir_of(&pack_dir());
    let (a, b) = (heading_of(&ir, "doc-a"), heading_of(&ir, "doc-b"));
    assert_ne!(a, b);
    let mut h = host(ir);
    assert_eq!(html(&h).matches(BODY).count(), 0);

    approve(&mut h);
    let row = draft_row(&h).expect("経路の行がある");
    assert_eq!(row.matches(BODY).count(), 1);
    assert!(row.contains(&a), "doc-a の見出しが出る");
    assert!(!row.contains(&b));
    assert!(row.contains("<h1"), "見出しが構造で出る");
    let body = row.split(BODY).nth(1).unwrap();
    for forbidden in [" role=", " aria-", " style="] {
        assert!(!body.contains(forbidden), "本文の区画に {forbidden} が無い");
    }

    approve(&mut h);
    let row = draft_row(&h).expect("経路の行がある");
    assert_eq!(row.matches(BODY).count(), 1);
    assert!(row.contains(&b), "doc-b の見出しに替わる");
    assert!(!row.contains(&a), "doc-a の見出しは消える");
}

#[test]
fn body_を宣言しない写しでは本文が出ず_経路の行は出る() {
    for repl in ["{kind: document}", "{kind: document, body: hidden}"] {
        let t = Tmp::variant(repl);
        let mut h = host(ir_of(&t.0));
        approve(&mut h);
        approve(&mut h);
        assert!(draft_row(&h).is_some(), "経路の行は出る");
        assert_eq!(html(&h).matches(BODY).count(), 0, "本文は出ない");
    }
}
