//! GUIDE-5（隠す語の検査の範囲の限界 3 項目）の受け入れ試験（TASK-300 2/2・#596）。
//!
//! 決定 D-43 の 3 項目を、欠陥パック（`testdata/pack-defects/guide5/b/` の差し替え）で確かめる。
//! - (a) 実体の引数で渡す `do`: 引数の展開後の文字列まで検査し、`E-GUIDANCE-LEAK` で検出する（G04）。
//! - (b) skill 名を含むヒントの段 2・3: 水準 B でも検査しない。作成者の規約として受け入れ、検出しないことを観察する（G05）。
//! - (c) 開始点の `guidance` も `body_levels` も持たない章の本文: 検査の対象外。検出しないことを観察する（G06）。
//!   正の対照は `guide4/b/G02`（`body_levels` だけ削り、開始点の `guidance: B` が効くと同じ本文が検出される）。
//!
//! `guidance_leak_scope.rs` のインライン試験は検査の範囲そのものの確認で、本ファイルは同じ 3 項目を土台
//! `base/format-b-v1_1` に重ねた欠陥パックで確かめる受け入れ試験である（`guide4_defects.rs` と
//! `guidance_levels.rs` の分担と同じ）。CLI `validate` / `test` は未実装（TASK-404）のため、ライブラリの
//! `validate_dir` と `compile_dir` で確かめる。

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{assert_no_secret, copy_dir, testdata, without_expanded_do_leak, Tmp};

use fandhe_workshop_contract::guidance::{visible_hints, GuidanceLevel};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const CH13: &str = "chapters/ch-13/chapter.yaml";
const MODULE: &str = "modules/m-guide5/module.yaml";
const HIDDEN_WORD: &str = "make-item";

/// 差し替えの表。ID と、差し替えが置く最上位の名前（辞書順）。
const TABLE: [(&str, &[&str]); 3] = [
    ("G04", &["chapters", "modules"]),
    ("G05", &["chapters", "workshop.yaml"]),
    ("G06", &["chapters"]),
];

fn root() -> PathBuf {
    testdata("pack-defects")
}

fn base() -> PathBuf {
    root().join("base").join("format-b-v1_1")
}

fn overlay(id: &str) -> PathBuf {
    root().join("guide5").join("b").join(id)
}

fn assemble_from(overlay: &Path, tag: &str) -> Tmp {
    let t = Tmp::new(&format!("guide5-{tag}"));
    copy_dir(&base(), &t.0);
    copy_dir(overlay, &t.0);
    t
}

fn assemble(id: &str) -> Tmp {
    assemble_from(&overlay(id), id)
}

fn describe(diags: &[Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .map(|x| format!("{} {} {}", x.code(), x.file(), x.item()))
        .collect()
}

fn has(diags: &[Diagnostic], code: DiagCode, file: &str, item: &str) -> bool {
    diags
        .iter()
        .any(|x| x.code() == code && x.file() == file && x.item() == item)
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .expect("read_dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn guide5_base_is_clean() {
    let diags = validate_dir(&base());
    assert!(diags.is_empty(), "{:?}", describe(&diags));
}

#[test]
fn guide5_overlay_dirs_match_table() {
    assert_eq!(names(&root().join("guide5")), vec!["b"]);
    let ids: Vec<&str> = TABLE.iter().map(|(id, _)| *id).collect();
    assert_eq!(names(&root().join("guide5").join("b")), ids);
    for (id, top) in TABLE {
        assert_eq!(names(&overlay(id)), top.to_vec(), "{id}");
        let new = fs::read(overlay(id).join(CH13)).expect("read overlay");
        let old = fs::read(base().join(CH13)).expect("base has the path");
        assert_ne!(new, old, "{id}: overlay must differ from base");
    }
}

#[test]
fn guide5_a_argument_do_leak_detected() {
    let t = assemble("G04");
    let diags = validate_dir(&t.0);
    assert_no_secret(&diags);
    assert!(
        has(&diags, DiagCode::GuidanceLeak, MODULE, "step:g5-a"),
        "{:?}",
        describe(&diags)
    );
    // 展開後の段の診断がちょうど 1 件で、残りは無い（定義の段の字面では同じ Step の漏れは出ない）。
    let (rest, removed) = without_expanded_do_leak(diags);
    assert_eq!(removed, 1, "{:?}", describe(&rest));
    assert!(rest.is_empty(), "{:?}", describe(&rest));
}

#[test]
fn guide5_a_safe_argument_has_no_leak() {
    // 対照: 引数の値だけを安全な文字列にすると、同じ構成で診断が 0 件になる。
    let t = assemble("G04");
    let ch = fs::read_to_string(t.0.join(CH13)).expect("read chapter");
    assert!(ch.contains("/make-item で備品を作る"));
    t.put(
        CH13,
        ch.replace("/make-item で備品を作る", "備品を作る")
            .as_bytes(),
    );
    let diags = validate_dir(&t.0);
    assert!(diags.is_empty(), "{:?}", describe(&diags));
}

#[test]
fn guide5_b_hint_tiers_are_not_checked() {
    let t = assemble("G05");
    let diags = validate_dir(&t.0);
    assert_no_secret(&diags);
    assert!(diags.is_empty(), "{:?}", describe(&diags));
    // 検査されないのは、ヒントが見えないからではない。水準 B で段 2・3 が実際に見える。
    let ir: Ir = serde_json::from_slice(&compile_dir(&t.0).expect("compile")).expect("ir");
    let node = ir
        .nodes
        .iter()
        .find(|n| n.step.id == "st-13-2")
        .expect("st-13-2");
    let visible = visible_hints(&ir, &node.step, GuidanceLevel::B);
    assert!(visible.contains(&"h-13-2-2"), "{visible:?}");
    assert!(visible.contains(&"h-13-2-3"), "{visible:?}");
}

#[test]
fn guide5_b_same_text_in_do_is_detected() {
    // 対照: 同じ語を水準 B で検査される `do` に置くと検出される。(b) の 0 件は語の表の漏れではなく、
    // 検査の範囲の外にあることによる。
    let t = assemble("G05");
    let ch = fs::read_to_string(t.0.join(CH13)).expect("read chapter");
    let from = "do: 左の画面で変わった箇所を探す";
    assert!(ch.contains(from));
    t.put(
        CH13,
        ch.replace(from, "do: /make-item で作った備品を探す")
            .as_bytes(),
    );
    let diags = validate_dir(&t.0);
    assert!(
        has(&diags, DiagCode::GuidanceLeak, CH13, "step:st-13-2"),
        "{:?}",
        describe(&diags)
    );
}

#[test]
fn guide5_c_undeclared_chapter_body_is_not_checked() {
    let t = assemble("G06");
    // 本文が隠す語を含むことが前提（含まなければ 0 件は意味を持たない）。
    let body = fs::read_to_string(t.0.join("chapters/ch-13/st-13-2.md")).expect("body");
    assert!(body.contains(HIDDEN_WORD));
    let diags = validate_dir(&t.0);
    assert_no_secret(&diags);
    assert!(diags.is_empty(), "{:?}", describe(&diags));
}

#[test]
fn guide5_c_declared_start_is_detected() {
    // 正の対照: guide4 の G02 は G06 に開始点の `guidance: B` だけを足した形で、同じ本文が検出される。
    let t = assemble_from(&root().join("guide4").join("b").join("G02"), "c-g02");
    let diags = validate_dir(&t.0);
    assert!(
        has(&diags, DiagCode::GuidanceLeak, CH13, "step:st-13-2"),
        "{:?}",
        describe(&diags)
    );
}

#[test]
fn guide5_leaks_are_validate_only() {
    let t = assemble("G04");
    assert!(compile_dir(&t.0).is_ok(), "G04: compile must succeed");
}

#[test]
fn guide5_diagnostics_do_not_echo_hidden_words() {
    for (id, _) in TABLE {
        let diags = validate_dir(&assemble(id).0);
        assert_no_secret(&diags);
        for x in &diags {
            assert!(
                !x.message().contains(HIDDEN_WORD),
                "{id}: message echoes the hidden word"
            );
        }
    }
    println!("GUIDE-5 limits: (a) detected, (b)(c) not reported (observed)");
}
