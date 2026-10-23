//! GUIDE-4（隠す語の検査）の受け入れ試験（TASK-300 1/2・#595）。
//!
//! PoC-3 の欠陥 D20〜D23 に相当する 4 種類が、期待のコード・ファイル・項目で検出されることを確かめる。
//! 4 種類は `testdata/pack-defects/v1_1/b/` をそのまま読み、表を `defect_packs.rs` と 1 か所に保つ（写さない）。
//! D22 は GUIDE-2 の `E-HINT-HIDDEN` で、GUIDE-4 の証拠ではない。4 種類の数にだけ入れる。
//!
//! G01〜G03（`testdata/pack-defects/guide4/b/`）は、GUIDE-4 が挙げる対象（本文・`do`）と経路（開始点の水準・
//! `body_levels`）を 1 つずつ単独で効かせる補助の欠陥で、4 種類の外として別に数える。
//! モジュールの経路は欠陥パックにせず、`guidance_levels.rs` のインライン試験が担当する。
//! 限界 3 項目（GUIDE-5）の観察は #596 の担当で、本ファイルは「検出する」側の確認だけを置く。

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{assert_no_secret, copy_dir, testdata, Tmp};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const CH13: &str = "chapters/ch-13/chapter.yaml";
const ITEM: &str = "step:st-13-2";
const HIDDEN_WORD: &str = "make-item";

/// 4 種類の表の 1 行。`overlay` は `v1_1/b/<ID>` の中の差し替えファイル。
struct Defect {
    id: &'static str,
    code: DiagCode,
    item: &'static str,
    overlay: &'static str,
}

const FOUR: [Defect; 4] = [
    Defect {
        id: "D20",
        code: DiagCode::RefBody,
        item: ITEM,
        overlay: CH13,
    },
    Defect {
        id: "D21",
        code: DiagCode::GuidanceLeak,
        item: ITEM,
        overlay: "chapters/ch-13/st-13-2.b.md",
    },
    Defect {
        id: "D22",
        code: DiagCode::HintHidden,
        item: ITEM,
        overlay: "workshop.yaml",
    },
    Defect {
        id: "D23",
        code: DiagCode::Parse,
        item: "scope",
        overlay: CH13,
    },
];

/// 補助の欠陥。どれも `chapters/ch-13/chapter.yaml` だけを重ね、期待は `E-GUIDANCE-LEAK`。
const AUX: [&str; 3] = ["G01", "G02", "G03"];

fn root() -> PathBuf {
    testdata("pack-defects")
}

fn base() -> PathBuf {
    root().join("base").join("format-b-v1_1")
}

fn four_overlay(d: &Defect) -> PathBuf {
    root().join("v1_1").join("b").join(d.id)
}

fn aux_overlay(id: &str) -> PathBuf {
    root().join("guide4").join("b").join(id)
}

fn assemble(overlay: &Path, tag: &str) -> Tmp {
    let t = Tmp::new(&format!("guide4-{tag}"));
    copy_dir(&base(), &t.0);
    copy_dir(overlay, &t.0);
    t
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

fn aux_detected(id: &str) {
    let t = assemble(&aux_overlay(id), id);
    let diags = validate_dir(&t.0);
    assert_no_secret(&diags);
    assert!(
        has(&diags, DiagCode::GuidanceLeak, CH13, ITEM),
        "{id}: want E-GUIDANCE-LEAK {CH13} {ITEM}; got {:?}",
        describe(&diags)
    );
}

fn all_overlays() -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = FOUR
        .iter()
        .map(|d| (d.id.to_string(), four_overlay(d)))
        .collect();
    v.extend(AUX.iter().map(|id| (id.to_string(), aux_overlay(id))));
    v
}

#[test]
fn guide4_base_is_clean() {
    let diags = validate_dir(&base());
    assert!(diags.is_empty(), "{:?}", describe(&diags));
}

#[test]
fn guide4_overlay_dirs_match_table() {
    assert_eq!(names(&root().join("guide4")), vec!["b"]);
    assert_eq!(names(&root().join("guide4").join("b")), AUX.to_vec());
    for id in AUX {
        let dir = aux_overlay(id);
        assert_eq!(names(&dir), vec!["chapters"], "{id}");
        let new = fs::read(dir.join(CH13)).expect("read overlay");
        let old = fs::read(base().join(CH13)).expect("base has the path");
        assert_ne!(new, old, "{id}: overlay must differ from base");
    }
}

#[test]
fn guide4_four_defects_d20_to_d23_detected() {
    let mut detected = 0;
    let mut missed = Vec::new();
    for d in &FOUR {
        assert!(
            four_overlay(d).join(d.overlay).is_file(),
            "{}: overlay file is missing",
            d.id
        );
        let t = assemble(&four_overlay(d), d.id);
        let diags = validate_dir(&t.0);
        assert_no_secret(&diags);
        if has(&diags, d.code, CH13, d.item) {
            detected += 1;
        } else {
            missed.push(format!("{}: got {:?}", d.id, describe(&diags)));
        }
    }
    assert!(missed.is_empty(), "undetected:\n{}", missed.join("\n"));
    assert_eq!(detected, 4);
    println!("GUIDE-4 defects detected {detected}/4");
}

#[test]
fn guide4_leak_in_do_detected() {
    aux_detected("G01");
}

#[test]
fn guide4_leak_via_start_guidance_only_detected() {
    aux_detected("G02");
}

#[test]
fn guide4_leak_via_body_levels_only_detected() {
    aux_detected("G03");
}

#[test]
fn guide4_leaks_are_validate_only() {
    let mut dirs: Vec<(String, PathBuf)> = vec![("D21".into(), four_overlay(&FOUR[1]))];
    dirs.extend(AUX.iter().map(|id| (id.to_string(), aux_overlay(id))));
    for (id, overlay) in dirs {
        let t = assemble(&overlay, &format!("co-{id}"));
        assert!(compile_dir(&t.0).is_ok(), "{id}: compile must succeed");
    }
}

#[test]
fn guide4_diagnostics_do_not_echo_hidden_words() {
    for (id, overlay) in all_overlays() {
        let diags = validate_dir(&assemble(&overlay, &format!("echo-{id}")).0);
        assert_no_secret(&diags);
        for x in &diags {
            assert!(
                !x.message().contains(HIDDEN_WORD),
                "{id}: message echoes the hidden word"
            );
        }
    }
}
