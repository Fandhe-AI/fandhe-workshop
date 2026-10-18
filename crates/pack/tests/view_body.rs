//! 文書ビューの本文の語（`views.<ID>.body`、EP-13、語彙 v1.4）の受け入れ試験（TASK-66 1/2、PACK-38・D-50・D-51）。
//!
//! 教材パックが「このビューは練習データの本文を出す」と宣言でき、その宣言が IR に載り、本文の木がそのビューの
//! 行の要素から引けること、書き換えた本文が既存の `put`・`replace` の `fixture` で表せること、省略と誤りの
//! 扱い、語彙の版の検査を、実パック（`tests/data/document-body-pack`）と一時ディレクトリの写しで確かめる。
//! 画面への表示（ART-18）は本試験の範囲外である。試験では `unwrap` を使ってよい。

mod common;

use std::path::{Path, PathBuf};

use common::{copy_dir, Tmp};
use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::view_body::body_doc;
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::effect::{apply_effects, NoConditions};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::RunState;
use fandhe_workshop_pack::compile::{compile, compile_dir};
use fandhe_workshop_pack::source::load;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;
use serde_json::Value;

fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/document-body-pack")
}

fn ir_of(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).expect("compiles");
    load_ir(&bytes).expect("the engine loads the IR")
}

/// 写しを作り、`workshop.yaml` の文字列を置き換える。
fn variant(tag: &str, from: &str, to: &str) -> Tmp {
    let tmp = Tmp::new(tag);
    copy_dir(&pack_dir(), &tmp.0);
    let path = tmp.0.join("workshop.yaml");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "置換元がある: {from}");
    std::fs::write(&path, text.replace(from, to)).unwrap();
    tmp
}

/// 本文の見出しの文字（`# ...` の中身）が木のどこかにあるか。
fn tree_has(ir: &Ir, id: &str, needle: &str) -> bool {
    serde_json::to_string(&ir.fixture_docs[id])
        .unwrap()
        .contains(needle)
}

#[test]
fn declared_body_reaches_ir_and_tree_is_reachable_from_the_view() {
    let dir = pack_dir();
    assert_eq!(validate_dir(&dir), vec![], "validate は診断 0 件");
    let ir = ir_of(&dir);
    assert_eq!(ir.pack.ir_format, 2);
    assert_eq!(ir.views["v-doc"]["body"], "shown");
    assert!(ir.fixture_docs.contains_key("doc-a") && ir.fixture_docs.contains_key("doc-b"));
    assert_eq!(
        body_doc(&ir, "v-doc", Some("doc-a")),
        Some(&ir.fixture_docs["doc-a"])
    );
    assert!(tree_has(&ir, "doc-a", "下書きのメモ"));
    assert!(tree_has(&ir, "doc-b", "清書したメモ"));
}

#[test]
fn replaced_body_follows_the_element_fixture() {
    let ir = ir_of(&pack_dir());
    let node = ir.nodes.iter().find(|n| n.uid == "main/a").unwrap();
    let items = |list: &[Value]| -> Vec<EffectItem> {
        list.iter()
            .map(|v| EffectItem::from_value(v).unwrap())
            .collect()
    };
    let uid = StepUid("main/a".to_string());
    let mut run = RunState::default();
    let at = Stamp {
        generation: Generation::INITIAL,
        step: uid,
    };
    let root = Root("main".to_string());
    let apply = |run: &mut RunState, list: &[Value]| {
        let ev = apply_effects(run, &root, &items(list), &at, Cause::Input, &NoConditions);
        assert!(!ev.iter().any(|e| matches!(e, Event::Warning { .. })));
    };

    apply(&mut run, &node.step.on_enter);
    let before = body_doc(&ir, "v-doc", run.world["notes/draft"].fixture.as_deref()).unwrap();
    apply(&mut run, &node.step.on_achieved);
    let after = body_doc(&ir, "v-doc", run.world["notes/draft"].fixture.as_deref()).unwrap();

    assert_eq!(before, &ir.fixture_docs["doc-a"]);
    assert_eq!(after, &ir.fixture_docs["doc-b"]);
    assert_ne!(before, after);
}

#[test]
fn omitting_body_keeps_todays_behavior() {
    let tmp = variant(
        "body-omitted",
        "{kind: document, body: shown}",
        "{kind: document}",
    );
    let ir = ir_of(&tmp.0);
    assert!(ir.views["v-doc"].get("body").is_none());
    assert_eq!(body_doc(&ir, "v-doc", Some("doc-a")), None);
    // 練習データの木自体は載る（版 2 の既存の動作）。
    assert!(ir.fixture_docs.contains_key("doc-a"));
}

#[test]
fn hidden_is_accepted_and_shows_nothing() {
    let tmp = variant("body-hidden", "body: shown", "body: hidden");
    assert_eq!(validate_dir(&tmp.0), vec![]);
    let ir = ir_of(&tmp.0);
    assert_eq!(body_doc(&ir, "v-doc", Some("doc-a")), None);
}

#[test]
fn bad_values_are_e_parse_without_echoing_input() {
    let cases = [
        ("Shown", "must be one of shown, hidden"),
        ("visible", "must be one of shown, hidden"),
        ("1", "must be a string"),
        ("~", "must be a string"),
    ];
    for (i, (value, want)) in cases.iter().enumerate() {
        let tmp = variant(
            &format!("body-bad-{i}"),
            "body: shown",
            &format!("body: {value}"),
        );
        let diags = validate_dir(&tmp.0);
        assert_eq!(diags.len(), 1, "{value}: {diags:?}");
        let d = &diags[0];
        assert_eq!(d.code(), DiagCode::Parse, "{value}");
        assert_eq!(d.item(), "view:v-doc");
        assert!(d.message().contains(want), "{value}: {}", d.message());
        if value.chars().all(|c| c.is_ascii_alphabetic()) {
            assert!(!d.message().contains(value), "入力の値を写さない: {value}");
        }
        // compile は IR を出さない（fail-closed）。
        let pack = load(&tmp.0).expect("loads");
        let errs = compile(&pack).expect_err("no IR for an unreadable body");
        assert!(errs.iter().any(|e| e.code() == DiagCode::Parse), "{value}");
    }
}

#[test]
fn vocab_declaration_older_than_1_4_is_a_mismatch() {
    let tmp = variant("body-vocab-old", "vocab: '1.4'", "vocab: '1.3'");
    let diags = validate_dir(&tmp.0);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].code(), DiagCode::VocabMismatch);
    assert!(
        diags[0].message().contains("EP-13"),
        "{}",
        diags[0].message()
    );
    // compile は版の検査をしない。
    assert!(compile_dir(&tmp.0).is_ok());
}

#[test]
fn vocab_declaration_1_4_is_accepted() {
    assert_eq!(validate_dir(&pack_dir()), vec![]);
}
