//! ビューの `kind` の綴りの検査（`E-VIEW-KIND-UNKNOWN`）の受け入れ試験（TASK-796、D-97。ART-1・AUTHOR-2・EXT-8）。
//!
//! 綴りの誤りが `validate`・`compile` で 1 件の診断になり `compile` が IR を出さないこと、7 種が通ること、
//! 欠け・型の誤りが `E-PARSE` になること、診断が入力の値を写さないこと、既存の実教材に新しい診断が出ないことを
//! 一時ディレクトリの写しで確かめる。作業ツリーは書き換えない。試験では `unwrap` を使ってよい。

mod common;

use std::path::Path;
use std::process::Command;

use common::{copy_dir, Tmp, CHAPTER, WORKSHOP};
use fandhe_workshop_contract::view_kind::VIEW_KINDS;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

/// `views:` の本文（YAML）を足した最小パック。
fn with_views(views: &str) -> Tmp {
    let t = Tmp::new("view-kind");
    t.put(
        "workshop.yaml",
        format!("{WORKSHOP}views:\n{views}").as_bytes(),
    );
    t.put("chapters/c1/chapter.yaml", CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

fn unknown_of(d: &[Diagnostic]) -> Vec<&Diagnostic> {
    d.iter()
        .filter(|x| x.code() == DiagCode::ViewKindUnknown)
        .collect()
}

#[test]
fn misspelled_kind_is_one_diagnostic() {
    for bad in ["task-tree", "cardlist", "File_Tree", "'document '", "''"] {
        let t = with_views(&format!("  v-x: {{kind: {bad}}}\n"));
        let d = validate_dir(&t.0);
        assert_eq!(d.len(), 1, "{bad}: {d:?}");
        assert_eq!(d[0].code(), DiagCode::ViewKindUnknown);
        assert_eq!(d[0].item(), "view:v-x");
        let raw = bad.trim_matches('\'');
        if !raw.is_empty() {
            assert!(!d[0].message().contains(raw), "{}", d[0].message());
        }
    }
}

#[test]
fn misspelled_kind_fails_compile() {
    let t = with_views("  v-x: {kind: task-tree}\n");
    let Err(d) = compile_dir(&t.0) else {
        panic!("compile must fail");
    };
    assert_eq!(unknown_of(&d).len(), 1, "{d:?}");
}

#[test]
fn cli_validate_exits_1() {
    let t = with_views("  v-x: {kind: cardlist}\n");
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("validate")
        .arg(&t.0)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8(o.stdout).unwrap();
    let first = out.lines().next().unwrap();
    let f: Vec<&str> = first.splitn(4, ": ").collect();
    assert_eq!(f.len(), 4, "{first}");
    assert_eq!(f[1], "view:v-x");
    assert_eq!(f[2], "E-VIEW-KIND-UNKNOWN");
}

#[test]
fn all_seven_kinds_are_accepted() {
    let views: String = VIEW_KINDS
        .iter()
        .enumerate()
        .map(|(i, k)| format!("  v-{i}: {{kind: {k}}}\n"))
        .collect();
    let t = with_views(&views);
    assert_eq!(validate_dir(&t.0), vec![]);
    compile_dir(&t.0).expect("compiles");
}

#[test]
fn non_string_or_missing_kind_is_e_parse() {
    for v in ["{kind: 1}", "{kind: null}", "{body: shown}", "document"] {
        let t = with_views(&format!("  v-x: {v}\n"));
        let d = validate_dir(&t.0);
        assert_eq!(d.len(), 1, "{v}: {d:?}");
        assert_eq!(d[0].code(), DiagCode::Parse, "{v}");
        assert!(unknown_of(&d).is_empty());
    }
}

#[test]
fn unsafe_view_id_is_not_echoed() {
    let t = with_views("  'v x<b>': {kind: nope}\n");
    let d = validate_dir(&t.0);
    let u = unknown_of(&d);
    assert_eq!(u.len(), 1, "{d:?}");
    assert_eq!(u[0].item(), "view");
    assert!(!u[0].message().contains("v x"), "{}", u[0].message());
}

#[test]
fn two_bad_views_are_two_diagnostics_in_id_order() {
    let t = with_views("  v-b: {kind: x}\n  v-a: {kind: y}\n");
    let d = validate_dir(&t.0);
    let items: Vec<&str> = d.iter().map(Diagnostic::item).collect();
    assert_eq!(items, ["view:v-a", "view:v-b"]);
}

#[test]
fn existing_packs_have_no_new_diagnostic() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for rel in [
        "testdata/module-reuse-rounds/test-data/pack",
        "template-workshop/examples/review-request",
        "template-workshop/examples/task-breakdown",
        "template-workshop/examples/task-breakdown/probes/views",
    ] {
        let dir = root.join(rel);
        assert!(dir.is_dir(), "{rel}");
        let tmp = Tmp::new("view-kind-existing");
        copy_dir(&dir, &tmp.0);
        let d = validate_dir(&tmp.0);
        assert!(unknown_of(&d).is_empty(), "{rel}: {d:?}");
    }
}
