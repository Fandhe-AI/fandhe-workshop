//! モジュールの巡回 3 種の欠陥パックの検出の試験（TASK-234・#755。仕様は MOD-2）。
//!
//! 巡回は次の 3 種で、`validate_dir`・`compile_dir`・CLI（`fandhe-pack validate` / `compile`）のどれでも
//! `E-MODULE-CYCLE` が期待のファイル・項目で出ることを確かめる。`compile` は止まらずに診断を返し、
//! 正しい土台では出ない。
//!
//! - C01: 2 つのモジュールが互いを呼ぶ。`v1/<x>/D07`（`defect_packs.rs` の D07 と同じ実体。写さない）
//! - C02: モジュールが自分を呼ぶ。`cycles/<x>/C02`
//! - C03: 3 つのモジュールを経由して呼び合う。`cycles/<x>/C03`（新しいモジュール `mod-relay` を足す）
//!
//! 判定は `defect_packs.rs`（コード・ファイル・項目の一致）より強い。深さの上限（`MAX_MODULE_DEPTH`）も同じ
//! コード `E-MODULE-CYCLE` を出すため、巡回の検出が外れても上の一致だけでは気づけない。そのため、
//! ちょうど 1 件であること・メッセージが経路を含むこと・`nesting exceeds` を含まないことも確かめる。
//! 1 経路ずつのインラインの試験は `modules.rs`・`validate_cycle_confirm.rs` が持つ。ここは実ファイルの欠陥パックと
//! 3 つの入口の一致を受け持つ。D07 を直したときは本試験の C01 も連動して直す。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use common::{assert_no_secret, copy_dir, testdata, Tmp};

use fandhe_workshop_pack::cli::EXIT_DIAGNOSTICS;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

#[derive(Clone, Copy, Debug)]
enum Format {
    A,
    B,
}

const FORMATS: [Format; 2] = [Format::A, Format::B];

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

/// 期待の表の 1 行。`overlay` は `testdata/pack-defects` からの相対（`<x>` は形式の `a`・`b`）。
struct Cycle {
    id: &'static str,
    overlay: &'static str,
    file: &'static str,
    item: &'static str,
    route: &'static str,
}

const CYCLES: [Cycle; 3] = [
    Cycle {
        id: "C01",
        overlay: "v1/<x>/D07",
        file: "modules/mod-inner/module",
        item: "step:u-loop",
        route: "mod-outer → mod-inner → mod-outer",
    },
    Cycle {
        id: "C02",
        overlay: "cycles/<x>/C02",
        file: "modules/mod-inner/module",
        item: "step:u-self",
        route: "mod-inner → mod-inner",
    },
    Cycle {
        id: "C03",
        overlay: "cycles/<x>/C03",
        file: "modules/mod-relay/module",
        item: "step:r-back",
        route: "mod-outer → mod-inner → mod-relay → mod-outer",
    },
];

fn overlay_dir(c: &Cycle, f: Format) -> PathBuf {
    testdata(&format!(
        "pack-defects/{}",
        c.overlay.replace("<x>", f.dir())
    ))
}

/// 土台を一時ディレクトリへ写し、欠陥の差し替えを重ねる。`testdata` は書き換えない。
fn assemble(c: &Cycle, f: Format) -> Tmp {
    let t = Tmp::new(&format!("mod2-{}-{}", c.id, f.dir()));
    copy_dir(&testdata(&format!("pack-defects/base/{}", f.base())), &t.0);
    copy_dir(&overlay_dir(c, f), &t.0);
    t
}

fn cycle_diags(d: &[Diagnostic]) -> Vec<&Diagnostic> {
    d.iter()
        .filter(|x| x.code() == DiagCode::ModuleCycle)
        .collect()
}

/// 巡回の診断がちょうど 1 件で、期待の場所・経路であることを確かめる。
fn assert_cycle(c: &Cycle, f: Format, diags: &[Diagnostic], via: &str) {
    let cs = cycle_diags(diags);
    let tag = format!("{} {f:?} ({via})", c.id);
    assert_eq!(cs.len(), 1, "{tag}: {diags:?}");
    let d = cs[0];
    assert_eq!(d.file(), format!("{}.{}", c.file, f.ext()), "{tag}");
    assert_eq!(d.item(), c.item, "{tag}");
    assert!(
        d.message()
            .contains(&format!("module call cycle: {}", c.route)),
        "{tag}: {}",
        d.message()
    );
    assert!(!d.message().contains("nesting exceeds"), "{tag}");
    assert_no_secret(diags);
}

/// 巡回で `compile` が止まらないことの証拠。期限内に返らなければ失敗にする。
fn within_timeout<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(60))
        .expect("did not return within 60s (compile must not hang on a cycle)")
}

#[test]
fn mod2_correct_bases_have_no_module_cycle() {
    for f in FORMATS {
        let base = testdata(&format!("pack-defects/base/{}", f.base()));
        let diags = validate_dir(&base);
        assert!(diags.is_empty(), "{f:?}: {diags:?}");
        assert!(compile_dir(&base).is_ok(), "{f:?}: compile failed");
    }
}

#[test]
fn mod2_cycle_dirs_match_table() {
    for f in FORMATS {
        let root = testdata(&format!("pack-defects/cycles/{}", f.dir()));
        let mut names: Vec<String> = fs::read_dir(&root)
            .expect("read cycles")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["C02", "C03"], "{f:?}");
        let base = testdata(&format!("pack-defects/base/{}", f.base()));
        for c in CYCLES.iter().skip(1) {
            let dir = overlay_dir(c, f);
            let mut files = Vec::new();
            common::files_of(&dir, &dir, &mut files);
            files.sort();
            let expect: Vec<PathBuf> = if c.id == "C02" {
                vec![format!("modules/mod-inner/module.{}", f.ext()).into()]
            } else {
                vec![
                    format!("modules/mod-inner/module.{}", f.ext()).into(),
                    format!("modules/mod-relay/module.{}", f.ext()).into(),
                ]
            };
            assert_eq!(files, expect, "{} {f:?}", c.id);
            for rel in &files {
                let b = base.join(rel);
                if b.exists() {
                    assert_ne!(
                        fs::read(&b).expect("read base"),
                        fs::read(dir.join(rel)).expect("read overlay"),
                        "{} {f:?}: {rel:?} equals the base",
                        c.id
                    );
                }
            }
        }
        assert!(overlay_dir(&CYCLES[0], f).is_dir(), "D07 missing for {f:?}");
    }
}

#[test]
fn mod2_three_cycles_detected_by_validate() {
    for f in FORMATS {
        for c in &CYCLES {
            let t = assemble(c, f);
            assert_cycle(c, f, &validate_dir(&t.0), "validate");
        }
    }
}

#[test]
fn mod2_three_cycles_detected_by_compile() {
    for f in FORMATS {
        for c in &CYCLES {
            let t = assemble(c, f);
            let dir = t.0.clone();
            let res = within_timeout(move || compile_dir(&dir));
            let diags = match res {
                Ok(_) => panic!("{} {f:?}: compile succeeded on a cycle", c.id),
                Err(d) => d,
            };
            assert_cycle(c, f, &diags, "compile");
            let v = validate_dir(&t.0);
            let (cv, cc) = (cycle_diags(&v), cycle_diags(&diags));
            assert_eq!(cv[0].file(), cc[0].file(), "{} {f:?}", c.id);
            assert_eq!(cv[0].item(), cc[0].item(), "{} {f:?}", c.id);
            assert_eq!(cv[0].message(), cc[0].message(), "{} {f:?}", c.id);
        }
    }
}

fn run_cli(sub: &str, dir: &Path) -> (Option<i32>, String, String) {
    let dir = dir.to_path_buf();
    let sub = sub.to_string();
    within_timeout(move || {
        let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
            .arg(&sub)
            .arg(&dir)
            .output()
            .expect("run fandhe-pack");
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    })
}

#[test]
fn mod2_cli_validate_and_compile_report_the_cycle_line() {
    for f in FORMATS {
        for c in &CYCLES {
            let t = assemble(c, f);
            let head = format!(
                "{}.{}: {}: E-MODULE-CYCLE: module call cycle: {}",
                c.file,
                f.ext(),
                c.item,
                c.route
            );
            let tmp = t.0.to_string_lossy().into_owned();
            for sub in ["validate", "compile"] {
                let (code, out, err) = run_cli(sub, &t.0);
                let tag = format!("{} {f:?} {sub}", c.id);
                assert_eq!(code, Some(i32::from(EXIT_DIAGNOSTICS)), "{tag}: {out}");
                assert!(out.lines().any(|l| l.starts_with(&head)), "{tag}: {out}");
                assert!(err.is_empty(), "{tag}: stderr: {err}");
                assert!(!out.contains(&tmp), "{tag}: absolute path leaked");
                if sub == "compile" {
                    assert!(!out.lines().any(|l| l.starts_with('{')), "{tag}: IR");
                }
            }
        }
    }
}
