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
//! 範囲外: 語彙 v1.1 の欠陥 11 種（#416）、正しいパックの誤検出 0 件（`test` を含む）と試験の限界の記録（#417）。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{assert_no_secret, copy_dir, testdata, Tmp};

use fandhe_workshop_pack::compile::compile_dir;
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
#[ignore = "fandhe-pack test is TASK-404 (#648); E-PLAY-* codes are fixed by TASK-398 (#468)"]
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
