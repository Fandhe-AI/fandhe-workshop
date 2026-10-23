//! 欠陥パック 24 種（PACK-29 の 12 種 × 形式 A・B）の検出の試験（TASK-56 1/3）。
//!
//! 正しい教材パック（`testdata/pack-defects/base/format-{a,b}`）に、欠陥を 1 か所だけ埋めたファイルを重ねて
//! 欠陥パックを組み立て、期待のコードが期待のファイル・項目で報告されることを確かめる。
//! 検出の判定は、コード・ファイル・項目の 3 つがすべて一致する診断が 1 件以上あること。コードだけ、
//! 場所だけの一致は検出に数えない。副次の診断（期待外のコード）は成否に影響させない。
//!
//! - D12 は `pack.ir_format` を 3 にして埋める。本 repo は版 1 と 2 の両方を読む（D-52）ため、2 では欠陥にならない
//! - D09・D10 は `fandhe-pack test` でしか見つからない。`test` は TASK-404（#648）、`E-PLAY-*` の扱いは
//!   TASK-398（#468）の持ち分で未実装のため、検出の試験は `#[ignore]` で置く。静的な 10 種 × 2 = 20 パックの
//!   検出を必須にし、24/24 は TASK-404・TASK-398 の完了後に `#[ignore]` を外して確かめる
//!
//! 語彙 v1.1 の欠陥 11 種（D13〜D23、形式 B だけ。TASK-56 2/3・#416）は、末尾の `pack29_v1_1_*` が確かめる。
//! 土台は `base/format-b-v1_1`、差し替えは `v1_1/b/<ID>`。必須の検出は `validate` の 11/11 である。D21・D22 は
//! TASK-49 2/2（#496）の `E-GUIDANCE-LEAK`・`E-HINT-HIDDEN`、D23 は TASK-49 1/2（#495）で検出する。
//!
//! 正しいパックの誤検出 0 件（TASK-56 3/3・#417）は、末尾の `pack29_correct_packs_*` が確かめる。
//! `validate` と、`test` の段の静的な検査（`E-PLAY-INPUT-CLASS`・誤入力の `expect` の読み取り）は必須で、
//! CLI `test` の誤検出 0 件は TASK-404 1/3（#649）の実行器で確かめる。
//! 試験の限界は `testdata/pack-defects/README.md` の「試験の限界」に記録する。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{assert_no_secret, copy_dir, testdata, Tmp};

use fandhe_workshop_pack::cli::EXIT_OK;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::play_expect::{check_error_input_expects, read_error_input_expects};
use fandhe_workshop_pack::profile::check_play_inputs;
use fandhe_workshop_pack::source::load;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Format {
    A,
    B,
}

impl Format {
    fn dir(self) -> &'static str {
        match self {
            Format::A => "a",
            Format::B => "b",
        }
    }

    fn base(self) -> &'static str {
        match self {
            Format::A => "format-a",
            Format::B => "format-b",
        }
    }

    fn ext(self) -> &'static str {
        match self {
            Format::A => "toml",
            Format::B => "yaml",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Validate,
    Test,
}

/// 期待の表の 1 行（形式を除く）。`file` は拡張子を除いたパックの根からの相対パス。
struct Defect {
    id: &'static str,
    code: DiagCode,
    file: &'static str,
    item: &'static str,
    phase: Phase,
}

const CHAPTER: &str = "chapters/ch-d/chapter";
const DEFECTS: [Defect; 12] = [
    Defect {
        id: "D01",
        code: DiagCode::RefStep,
        file: CHAPTER,
        item: "step:st-2-4",
        phase: Phase::Validate,
    },
    Defect {
        id: "D02",
        code: DiagCode::RefView,
        file: CHAPTER,
        item: "step:st-2-1",
        phase: Phase::Validate,
    },
    Defect {
        id: "D03",
        code: DiagCode::RefFixture,
        file: CHAPTER,
        item: "step:st-2-2",
        phase: Phase::Validate,
    },
    Defect {
        id: "D04",
        code: DiagCode::RefHint,
        file: CHAPTER,
        item: "step:st-2-3",
        phase: Phase::Validate,
    },
    Defect {
        id: "D05",
        code: DiagCode::Unreachable,
        file: CHAPTER,
        item: "step:st-2-orphan",
        phase: Phase::Validate,
    },
    Defect {
        id: "D06",
        code: DiagCode::DeadEnd,
        file: CHAPTER,
        item: "step:st-2-stub",
        phase: Phase::Validate,
    },
    Defect {
        id: "D07",
        code: DiagCode::ModuleCycle,
        file: "modules/mod-inner/module",
        item: "step:u-loop",
        phase: Phase::Validate,
    },
    Defect {
        id: "D08",
        code: DiagCode::EmptyNextAction,
        file: "workshop",
        item: "rejection:rj-missing-arg",
        phase: Phase::Validate,
    },
    Defect {
        id: "D09",
        code: DiagCode::PlayIncomplete,
        file: "tests/playthrough",
        item: "playthrough:pt-main",
        phase: Phase::Test,
    },
    Defect {
        id: "D10",
        code: DiagCode::PlayCategoryMismatch,
        file: "tests/playthrough",
        item: "error_input:e03",
        phase: Phase::Test,
    },
    Defect {
        id: "D11",
        code: DiagCode::NoConfirm,
        file: CHAPTER,
        item: "step:st-2-7",
        phase: Phase::Validate,
    },
    Defect {
        id: "D12",
        code: DiagCode::FormatVersion,
        file: "workshop",
        item: "pack:ir_format",
        phase: Phase::Validate,
    },
];

const FORMATS: [Format; 2] = [Format::A, Format::B];

fn defects_root() -> PathBuf {
    testdata("pack-defects")
}

fn rel_file(d: &Defect, f: Format) -> String {
    format!("{}.{}", d.file, f.ext())
}

fn base_dir(f: Format) -> PathBuf {
    defects_root().join("base").join(f.base())
}

fn overlay_dir(d: &Defect, f: Format) -> PathBuf {
    defects_root().join("v1").join(f.dir()).join(d.id)
}

/// 正しいパックを一時ディレクトリへ写し、その上に欠陥のファイルを重ねる。
fn assemble(d: &Defect, f: Format) -> Tmp {
    let t = Tmp::new(&format!("defect-{}-{}", d.id, f.dir()));
    copy_dir(&base_dir(f), &t.0);
    copy_dir(&overlay_dir(d, f), &t.0);
    t
}

fn files_under(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    for e in fs::read_dir(dir).expect("read_dir").flatten() {
        let name = format!("{prefix}{}", e.file_name().to_string_lossy());
        if e.path().is_dir() {
            files_under(&e.path(), &format!("{name}/"), out);
        } else {
            out.push(name);
        }
    }
}

fn matches_defect(diag: &Diagnostic, d: &Defect, f: Format) -> bool {
    diag.code() == d.code && diag.file() == rel_file(d, f) && diag.item() == d.item
}

#[test]
fn pack29_bases_validate_clean_and_compile_to_same_ir() {
    for f in FORMATS {
        let diags = validate_dir(&base_dir(f));
        assert!(diags.is_empty(), "{f:?}: {diags:?}");
    }
    let a = compile_dir(&base_dir(Format::A)).expect("compile format A");
    let b = compile_dir(&base_dir(Format::B)).expect("compile format B");
    assert_eq!(a, b, "the two formats must compile to the same IR");
}

#[test]
fn pack29_defect_dirs_match_table_and_only_replace_base_files() {
    for f in FORMATS {
        let mut dirs: Vec<String> = fs::read_dir(defects_root().join("v1").join(f.dir()))
            .expect("read v1")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        dirs.sort();
        let want: Vec<&str> = DEFECTS.iter().map(|d| d.id).collect();
        assert_eq!(dirs, want, "{f:?}: v1 directories must equal the table");

        for d in &DEFECTS {
            let overlay = overlay_dir(d, f);
            let mut files = Vec::new();
            files_under(&overlay, "", &mut files);
            assert_eq!(
                files,
                vec![rel_file(d, f)],
                "{} {f:?}: one replaced file",
                d.id
            );
            let new = fs::read(overlay.join(&files[0])).expect("read overlay");
            let old = fs::read(base_dir(f).join(&files[0])).expect("base has the same path");
            assert_ne!(new, old, "{} {f:?}: overlay must differ from base", d.id);
        }
    }
}

#[test]
fn pack29_static_defects_reported_at_expected_location() {
    let mut missed = Vec::new();
    let mut detected = 0;
    for f in FORMATS {
        for d in DEFECTS.iter().filter(|d| d.phase == Phase::Validate) {
            let t = assemble(d, f);
            let diags = validate_dir(&t.0);
            assert_no_secret(&diags);
            if diags.iter().any(|x| matches_defect(x, d, f)) {
                detected += 1;
            } else {
                let got: Vec<String> = diags
                    .iter()
                    .map(|x| format!("{} {} {}", x.code(), x.file(), x.item()))
                    .collect();
                missed.push(format!(
                    "{} {f:?}: want {} {} {}; got {got:?}",
                    d.id,
                    d.code,
                    rel_file(d, f),
                    d.item
                ));
            }
        }
    }
    assert!(
        missed.is_empty(),
        "undetected defects:\n{}",
        missed.join("\n")
    );
    assert_eq!(detected, 20);
    println!("PACK-29 static defects detected {detected}/20");
}

#[test]
fn pack29_play_defects_pass_static_checks() {
    for f in FORMATS {
        for d in DEFECTS.iter().filter(|d| d.phase == Phase::Test) {
            let diags = validate_dir(&assemble(d, f).0);
            assert!(diags.is_empty(), "{} {f:?}: {diags:?}", d.id);
        }
    }
}

#[test]
#[ignore = "E-PLAY-* の行を出すのは TASK-404 3/3 以降（#651）。コードは TASK-398 (#468) で固定済み"]
fn pack29_play_defects_reported_by_test() {
    for f in FORMATS {
        for d in DEFECTS.iter().filter(|d| d.phase == Phase::Test) {
            let t = assemble(d, f);
            let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
                .arg("test")
                .arg(&t.0)
                .output()
                .expect("run fandhe-pack");
            let all = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            let head = format!("{}: {}: {}: ", rel_file(d, f), d.item, d.code);
            assert!(
                all.lines().any(|l| l.starts_with(&head)),
                "{} {f:?}: no diagnostic starting with `{head}`; exit={:?}",
                d.id,
                o.status.code()
            );
        }
    }
}

// ---- 語彙 v1.1 の欠陥 11 種（D13〜D23、形式 B だけ。TASK-56 2/3、#416）----

/// v1.1 の期待の表の 1 行。`overlay` は重ねるファイル、`file` は期待のファイルで、どちらも拡張子つき。
/// D21（本文）と D22（`workshop.yaml`）は、重ねるファイルと診断の付くファイルが違う。
struct DefectV11 {
    id: &'static str,
    code: DiagCode,
    file: &'static str,
    item: &'static str,
    overlay: &'static str,
}

const CH1: &str = "chapters/ch-1/chapter.yaml";
const CH2: &str = "chapters/ch-2/chapter.yaml";
const CH13: &str = "chapters/ch-13/chapter.yaml";
const WORKSHOP_B: &str = "workshop.yaml";
const DEFECTS_V1_1: [DefectV11; 11] = [
    DefectV11 {
        id: "D13",
        code: DiagCode::RefRejection,
        file: CH2,
        item: "step:st-2-7",
        overlay: CH2,
    },
    DefectV11 {
        id: "D14",
        code: DiagCode::Structure,
        file: CH1,
        item: "step:st-1-4",
        overlay: CH1,
    },
    DefectV11 {
        id: "D15",
        code: DiagCode::SlotUnknown,
        file: WORKSHOP_B,
        item: "rejection:rj-default-unknown-command",
        overlay: WORKSHOP_B,
    },
    DefectV11 {
        id: "D16",
        code: DiagCode::SlotContext,
        file: WORKSHOP_B,
        item: "rejection:rj-default-wrong-order",
        overlay: WORKSHOP_B,
    },
    DefectV11 {
        id: "D17",
        code: DiagCode::SlotNoFallback,
        file: WORKSHOP_B,
        item: "rejection:rj-default-unknown-command",
        overlay: WORKSHOP_B,
    },
    DefectV11 {
        id: "D18",
        code: DiagCode::SlotNeedsDo,
        file: CH2,
        item: "step:st-2-3",
        overlay: CH2,
    },
    DefectV11 {
        id: "D19",
        code: DiagCode::SlotSyntax,
        file: WORKSHOP_B,
        item: "rejection:rj-default-busy",
        overlay: WORKSHOP_B,
    },
    DefectV11 {
        id: "D20",
        code: DiagCode::RefBody,
        file: CH13,
        item: "step:st-13-2",
        overlay: CH13,
    },
    DefectV11 {
        id: "D21",
        code: DiagCode::GuidanceLeak,
        file: CH13,
        item: "step:st-13-2",
        overlay: "chapters/ch-13/st-13-2.b.md",
    },
    DefectV11 {
        id: "D22",
        code: DiagCode::HintHidden,
        file: CH13,
        item: "step:st-13-2",
        overlay: WORKSHOP_B,
    },
    DefectV11 {
        id: "D23",
        code: DiagCode::Parse,
        file: CH13,
        item: "scope",
        overlay: CH13,
    },
];

fn base_dir_v1_1() -> PathBuf {
    defects_root().join("base").join("format-b-v1_1")
}

fn overlay_dir_v1_1(d: &DefectV11) -> PathBuf {
    defects_root().join("v1_1").join("b").join(d.id)
}

fn assemble_v1_1(d: &DefectV11) -> Tmp {
    let t = Tmp::new(&format!("defect-{}-b", d.id));
    copy_dir(&base_dir_v1_1(), &t.0);
    copy_dir(&overlay_dir_v1_1(d), &t.0);
    t
}

fn matches_v1_1(diag: &Diagnostic, d: &DefectV11) -> bool {
    diag.code() == d.code && diag.file() == d.file && diag.item() == d.item
}

fn describe(diags: &[Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .map(|x| format!("{} {} {}", x.code(), x.file(), x.item()))
        .collect()
}

/// `dir` の直下のエントリ名を集める（ファイルも dir も対象）。
fn entry_names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir)
        .expect("read_dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

#[test]
fn pack29_v1_1_base_validates_clean_and_compiles() {
    let diags = validate_dir(&base_dir_v1_1());
    assert!(diags.is_empty(), "{diags:?}");
    compile_dir(&base_dir_v1_1()).expect("compile v1.1 base");
}

#[test]
fn pack29_v1_1_defect_dirs_match_table_and_only_replace_base_files() {
    let v1_1 = defects_root().join("v1_1");
    assert_eq!(entry_names(&v1_1), vec!["b"], "v1_1 has format B only");
    let want: Vec<&str> = DEFECTS_V1_1.iter().map(|d| d.id).collect();
    assert_eq!(
        entry_names(&v1_1.join("b")),
        want,
        "v1_1/b directories must equal the table"
    );

    for d in &DEFECTS_V1_1 {
        let overlay = overlay_dir_v1_1(d);
        let mut files = Vec::new();
        files_under(&overlay, "", &mut files);
        assert_eq!(files, vec![d.overlay.to_string()], "{}: one file", d.id);
        let new = fs::read(overlay.join(d.overlay)).expect("read overlay");
        let old = fs::read(base_dir_v1_1().join(d.overlay)).expect("base has the path");
        assert_ne!(new, old, "{}: overlay must differ from base", d.id);
    }
}

#[test]
fn pack29_v1_1_static_defects_reported_at_expected_location() {
    let mut missed = Vec::new();
    let mut detected = 0;
    for d in DEFECTS_V1_1.iter() {
        let diags = validate_dir(&assemble_v1_1(d).0);
        assert_no_secret(&diags);
        if diags.iter().any(|x| matches_v1_1(x, d)) {
            detected += 1;
        } else {
            missed.push(format!(
                "{}: want {} {} {}; got {:?}",
                d.id,
                d.code,
                d.file,
                d.item,
                describe(&diags)
            ));
        }
    }
    assert!(
        missed.is_empty(),
        "undetected defects:\n{}",
        missed.join("\n")
    );
    assert_eq!(detected, 11);
    println!("PACK-29 v1.1 static defects detected {detected}/11");
}

// ---- 正しいパックの誤検出 0 件（TASK-56 3/3、#417）----

/// `validate` の誤検出 0 件を確かめる正しいパック（土台 3 つ）。
const CORRECT_FOR_VALIDATE: [&str; 3] = ["format-a", "format-b", "format-b-v1_1"];

/// `test` の誤検出 0 件を確かめる正しいパック。教材テストを持つ土台だけで、`format-b-v1_1` は `tests/` が
/// 無く `test` の段の検査が空振りになるため含めない。
const CORRECT_FOR_TEST: [&str; 2] = ["format-a", "format-b"];

fn correct_pack(name: &str) -> PathBuf {
    let p = defects_root().join("base").join(name);
    assert!(p.is_dir(), "correct pack {name} must exist");
    p
}

/// 一覧が空振りしない（両形式を含む）ことを確かめる。
fn assert_both_formats(names: &[&str]) {
    let has = |manifest: &str| {
        names
            .iter()
            .any(|n| correct_pack(n).join(manifest).is_file())
    };
    assert!(has("workshop.toml"), "format A pack is required");
    assert!(has("workshop.yaml"), "format B pack is required");
}

#[test]
fn pack29_correct_packs_have_no_false_positives_in_validate() {
    assert_both_formats(&CORRECT_FOR_VALIDATE);
    let mut found = Vec::new();
    for name in CORRECT_FOR_VALIDATE {
        let diags = validate_dir(&correct_pack(name));
        assert_no_secret(&diags);
        found.extend(describe(&diags).into_iter().map(|d| format!("{name}: {d}")));
    }
    assert!(found.is_empty(), "false positives:\n{}", found.join("\n"));
    println!(
        "PACK-29 false positives (validate) 0 over {} packs",
        CORRECT_FOR_VALIDATE.len()
    );
}

#[test]
fn pack29_correct_packs_have_no_false_positives_in_test_static_checks() {
    assert_both_formats(&CORRECT_FOR_TEST);
    let mut found = Vec::new();
    for name in CORRECT_FOR_TEST {
        let lp = load(&correct_pack(name)).unwrap_or_else(|d| panic!("{name}: load: {d:?}"));
        // 教材テストが読めずに空になる経路を、誤検出 0 件と取り違えないための確認。
        assert!(lp.tests_file.is_some(), "{name}: tests file must be loaded");
        let expects = read_error_input_expects(&lp)
            .unwrap_or_else(|d| panic!("{name}: error_inputs expect: {d:?}"));
        assert_eq!(expects.len(), 3, "{name}: error_inputs e01..e03");
        // 既定の接頭辞では `check_play_inputs` は何も検査しない。0 件は自明で、`pack.profile` を
        // 変える教材が来たときの備えとして呼ぶ。
        let mut diags = check_play_inputs(&lp);
        diags.extend(check_error_input_expects(&lp));
        assert_no_secret(&diags);
        found.extend(describe(&diags).into_iter().map(|d| format!("{name}: {d}")));
    }
    assert!(found.is_empty(), "false positives:\n{}", found.join("\n"));
    println!(
        "PACK-29 false positives (test static checks) 0 over {} packs",
        CORRECT_FOR_TEST.len()
    );
}

/// 実行器（TASK-404 1/3）が模範解答の完走と誤入力の分類を確かめ、診断を 1 件も出さない（終了コード 0）ことを要求する。
#[test]
fn pack29_correct_packs_have_no_false_positives_in_test() {
    for name in CORRECT_FOR_TEST {
        let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
            .arg("test")
            .arg(correct_pack(name))
            .output()
            .expect("run fandhe-pack");
        let all = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        let diag_lines: Vec<&str> = all
            .lines()
            .filter(|l| {
                l.splitn(3, ": ")
                    .nth(2)
                    .is_some_and(|r| r.starts_with("E-"))
            })
            .collect();
        assert!(diag_lines.is_empty(), "{name}: {diag_lines:?}");
        assert_eq!(
            o.status.code(),
            Some(i32::from(EXIT_OK)),
            "{name}: exit code"
        );
    }
}
