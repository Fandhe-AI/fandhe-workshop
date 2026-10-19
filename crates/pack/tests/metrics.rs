//! `fandhe-pack metrics <pack-dir>` の結合試験（TASK-402、AUTHOR-1）。
//!
//! 期待値は、PoC-3 の試作の数え方（`docs/cli.md` 版 3 の 11 節）を実装とは別に（python で）書いて得た定数である。
//! 掲示板教材そのものは repo に無く（D-11。`testdata/README.md`）、spec は private で試験から読まないため、
//! 自動試験では最小の教材パック 2 形式と、条件の語彙を使う架空のパック（`testdata/condition-vocab-pack`）で代える。
//! 掲示板教材との照合はローカルで行い、`docs/testing.md` 5.148 に記録した。作業ツリーは書き換えない。

mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::{testdata, Tmp};

const HEADER: &str = "file,lines,effective_lines,bytes,max_depth";

fn run(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("metrics")
        .arg(dir)
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// ヘッダ・行を改行で結び、末尾に改行を 1 つ付ける。
fn table(rows: &[&str]) -> String {
    let mut s = String::from(HEADER);
    for r in rows {
        s.push('\n');
        s.push_str(r);
    }
    s.push('\n');
    s
}

#[test]
fn me1_min_pack_format_a() {
    let o = run(&testdata(
        "engine-event-consistency/test-data/min-3step-pack",
    ));
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stderr.is_empty());
    assert_eq!(
        stdout(&o),
        table(&[
            "chapters/ch-min/chapter.toml,93,74,2800,8",
            "tests/playthrough.toml,55,46,1544,4",
            "workshop.toml,98,77,3390,3",
            "TOTAL,246,197,7734,8",
        ])
    );
}

#[test]
fn me2_min_pack_format_b() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b");
    let o = run(&dir);
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stderr.is_empty());
    assert_eq!(
        stdout(&o),
        table(&[
            "chapters/ch-min/chapter.yaml,94,93,2661,10",
            "tests/playthrough.yaml,14,13,521,8",
            "workshop.yaml,82,80,3400,3",
            "TOTAL,190,186,6582,10",
        ])
    );
}

#[test]
fn me3_condition_vocab_pack_with_modules() {
    let o = run(&testdata("condition-vocab-pack"));
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stderr.is_empty());
    assert_eq!(
        stdout(&o),
        table(&[
            "chapters/ch-1/chapter.yaml,32,29,1045,8",
            "chapters/ch-13/chapter.yaml,95,88,2815,10",
            "chapters/ch-2/chapter.yaml,202,191,6315,10",
            "chapters/ch-7/chapter.yaml,40,36,1059,10",
            "modules/impl-run/module.yaml,71,65,2114,9",
            "modules/merge-gate/module.yaml,31,28,972,9",
            "workshop.yaml,89,80,3495,3",
            "TOTAL,560,517,17815,10",
        ])
    );
}

#[test]
fn me4_output_is_deterministic_and_has_five_fields() {
    let dir = testdata("condition-vocab-pack");
    let (a, b) = (stdout(&run(&dir)), stdout(&run(&dir)));
    assert_eq!(a, b);
    for l in a.lines() {
        assert_eq!(l.split(',').count(), 5, "{l}");
    }
}

#[test]
fn me5_missing_dir_and_missing_workshop_are_diagnostics() {
    let o = run(Path::new("/nonexistent/fandhe-metrics-dir"));
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stderr.is_empty());
    let out = stdout(&o);
    assert!(out.contains("E-PARSE"), "{out}");
    assert!(!out.contains("file,lines"), "{out}");

    let t = Tmp::new("me5");
    t.put("chapters/c/chapter.yaml", b"id: c\n");
    let o = run(&t.0);
    assert_eq!(o.status.code(), Some(1));
    assert!(stdout(&o).contains("E-PARSE"));

    t.put("workshop.yaml", b"pack: {}\n");
    t.put("workshop.toml", b"[pack]\n");
    let o = run(&t.0);
    assert_eq!(o.status.code(), Some(1));
    assert!(stdout(&o).contains("both workshop.toml and workshop.yaml"));
}

#[test]
fn me6_non_utf8_file_is_diagnostic_and_prints_no_table() {
    let t = Tmp::new("me6");
    t.put("workshop.yaml", b"pack: {}\n");
    t.put("chapters/c/chapter.yaml", b"id: \xff\xfe\n");
    let o = run(&t.0);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stderr.is_empty());
    let out = stdout(&o);
    assert!(out.contains("chapters/c/chapter.yaml"), "{out}");
    assert!(out.contains("E-PARSE"), "{out}");
    assert!(!out.contains("file,lines"), "{out}");
    assert!(!out.contains("TOTAL"), "{out}");
}

#[test]
fn me7_too_deep_directory_is_diagnostic() {
    let t = Tmp::new("me7");
    t.put("workshop.yaml", b"pack: {}\n");
    let mut deep = t.0.join("deep");
    for _ in 1..33 {
        deep = deep.join("d");
    }
    std::fs::create_dir_all(&deep).unwrap();
    let o = run(&t.0);
    assert_eq!(o.status.code(), Some(1));
    let out = stdout(&o);
    assert!(out.contains("E-LIMIT-DEPTH"), "{out}");
    assert!(!out.contains("TOTAL"), "{out}");
}

#[cfg(unix)]
#[test]
fn me8_symlink_is_diagnostic_and_outside_is_not_read() {
    let outside = Tmp::new("me8-out");
    outside.put("secret-outside.yaml", b"SECRET-OUTSIDE: 1\n");
    let t = Tmp::new("me8");
    t.put("workshop.yaml", b"pack: {}\n");
    std::os::unix::fs::symlink(outside.0.join("secret-outside.yaml"), t.0.join("link.yaml"))
        .unwrap();
    let o = run(&t.0);
    assert_eq!(o.status.code(), Some(1));
    let out = stdout(&o);
    assert!(out.contains("E-LIMIT-SYMLINK"), "{out}");
    assert!(!out.contains("SECRET-OUTSIDE") && !out.contains("secret-outside"));
    assert!(!out.contains("TOTAL"), "{out}");
}

#[cfg(unix)]
#[test]
fn me9_odd_file_names_keep_one_row_of_five_fields() {
    let t = Tmp::new("me9");
    t.put("workshop.yaml", b"pack: {}\n");
    t.put("a,b.yaml", b"k: 1\n");
    t.put("x\ny.yaml", b"k: 1\n");
    let o = run(&t.0);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(
        stdout(&o),
        table(&[
            "a\\u{002C}b.yaml,1,1,5,1",
            "workshop.yaml,1,1,9,2",
            "x\\u{000A}y.yaml,1,1,5,1",
            "TOTAL,3,3,19,2",
        ])
    );
}

#[test]
fn me10_only_toml_and_yaml_files_are_counted() {
    let t = Tmp::new("me10");
    t.put("workshop.yaml", b"pack: {}\n");
    t.put("notes.md", b"# a\n\nb\n");
    t.put("data.json", b"{}\n");
    let o = run(&t.0);
    assert_eq!(
        stdout(&o),
        table(&["workshop.yaml,1,1,9,2", "TOTAL,1,1,9,2"])
    );
}
