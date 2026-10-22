//! 語彙と contract の判定結果・拒否分類の型の結線の試験（PACK-11、TASK-45 1/2）。
//!
//! 語彙の表（`vocab::TERMS`）の `result`・`rejection` の語が contract の `JudgmentKind`・`Category` と一致すること、
//! 教材の `rejections.<id>.category`・`defaults` のキー・`on_reject` のキーが語彙の 8 語で読まれることを確かめる。
//! 教材テストの誤入力の期待（`rejected:wrong_order` など）の読み取りと分類の試験は TASK-45 2/2 の担当である。
//! 一時ディレクトリは `CARGO_TARGET_TMPDIR` の下に作る。

mod common;

use std::collections::BTreeSet;

use common::{assert_no_secret, ch1, testdata, Tmp, SECRET, WORKSHOP};

use fandhe_workshop_contract::judgment::{Category, JudgmentKind, RejectCategory};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::vocab::{rejection_category, result_kind, TERMS};
use fandhe_workshop_pack::{DiagCode, Diagnostic};

fn terms(kind: &str) -> Vec<&'static str> {
    TERMS
        .iter()
        .filter(|t| t.kind == kind)
        .map(|t| t.term)
        .collect()
}

#[test]
fn pack11_result_terms_map_to_contract_kinds() {
    let mapped: BTreeSet<&str> = terms("result")
        .into_iter()
        .map(|t| result_kind(t).expect(t).as_str())
        .collect();
    assert_eq!(
        mapped,
        BTreeSet::from(["accepted", "rejected", "clarified"])
    );
    for k in [
        JudgmentKind::Confirmed,
        JudgmentKind::Progressed,
        JudgmentKind::Ignored,
    ] {
        assert_eq!(result_kind(k.as_str()), None);
    }
}

#[test]
fn pack11_rejection_terms_match_contract_categories() {
    let vocab: BTreeSet<&str> = terms("rejection").into_iter().collect();
    let contract: BTreeSet<&str> = Category::ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(vocab, contract);
    let rejections = vocab
        .iter()
        .filter(|t| rejection_category(t).expect(t).is_rejection())
        .count();
    assert_eq!(rejections, RejectCategory::ALL.len());
    assert!(!rejection_category("free_text_unjudged")
        .expect("term")
        .is_rejection());
    for bad in ["Busy", " busy", "unknown", ""] {
        assert_eq!(rejection_category(bad), None, "{bad:?}");
    }
}

#[test]
fn pack11_existing_packs_pass_category_checks() {
    compile_dir(&ch1()).expect("ch1-pack");
    compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("module-reuse-rounds");
}

const CHAPTER: &str = concat!(
    "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
    "  - {id: a, title: A, next: [{to: b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n",
    "  - {id: b, title: B, final: true}\n",
);

fn diags(workshop_extra: &str, chapter: &str) -> Vec<Diagnostic> {
    let t = Tmp::new("judgment");
    t.put(
        "workshop.yaml",
        format!("{WORKSHOP}{workshop_extra}").as_bytes(),
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    compile_dir(&t.0).err().unwrap_or_default()
}

fn only(d: &[Diagnostic], file: &str, item: &str) {
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), DiagCode::Parse, "{d:?}");
    assert_eq!(d[0].file(), file, "{d:?}");
    assert_eq!(d[0].item(), item, "{d:?}");
    assert_no_secret(d);
}

#[test]
fn pack11_bad_rejection_category_is_e_parse() {
    for body in [
        "{category: Unknown_Command, message: m, next_action: n}",
        "{message: m, next_action: n}",
        "{category: 3, message: m, next_action: n}",
        "{category: '', message: m, next_action: n}",
        "x",
    ] {
        let d = diags(&format!("rejections:\n  r1: {body}\n"), CHAPTER);
        only(&d, "workshop.yaml", "rejection:r1");
    }
}

#[test]
fn pack11_bad_defaults_key_is_e_parse() {
    let d = diags("defaults:\n  Busy: r1\n", CHAPTER);
    only(&d, "workshop.yaml", "defaults:Busy");
}

#[test]
fn pack11_bad_on_reject_key_is_e_parse() {
    let chapter = CHAPTER.replace(
        "{id: a, title: A,",
        "{id: a, title: A, on_reject: {nope: r1},",
    );
    let d = diags("", &chapter);
    only(&d, "chapters/c1/chapter.yaml", "step:a");
}

#[test]
fn pack11_bad_on_reject_key_in_module_is_e_parse() {
    let t = Tmp::new("judgment-mod");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = concat!(
        "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
        "  - {id: a, title: A, use: m, next: [{to: b}]}\n",
        "  - {id: b, title: B, final: true}\n",
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put(
        "modules/m/module.yaml",
        b"id: m\ntitle: M\nsteps:\n  - {id: x, title: X, on_reject: {nope: r1}, final: true}\n",
    );
    let d = compile_dir(&t.0).err().unwrap_or_default();
    assert!(
        d.iter().any(|x| x.code() == DiagCode::Parse
            && x.file() == "modules/m/module.yaml"
            && x.item() == "step:x"),
        "{d:?}"
    );
    assert_no_secret(&d);
}

#[test]
fn pack11_free_text_unjudged_is_accepted_everywhere() {
    let chapter = CHAPTER.replace(
        "{id: a, title: A,",
        "{id: a, title: A, on_reject: {free_text_unjudged: r1},",
    );
    let extra = "rejections:\n  r1: {category: free_text_unjudged, message: m, next_action: n}\ndefaults:\n  free_text_unjudged: r1\n";
    let d = diags(extra, &chapter);
    assert!(d.iter().all(|x| x.code() != DiagCode::Parse), "{d:?}");
}

#[test]
fn pack11_unsafe_category_is_not_echoed() {
    let d = diags(
        &format!("rejections:\n  r1: {{category: '{SECRET} x', message: m, next_action: n}}\n"),
        CHAPTER,
    );
    only(&d, "workshop.yaml", "rejection:r1");
    assert!(!d[0].to_string().contains("SECRET"), "{d:?}");
}
