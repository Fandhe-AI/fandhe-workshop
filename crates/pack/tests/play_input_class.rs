//! `test` が教材テストの鍵（`cmd`・`text`）と画面での分けの食い違いを `E-PLAY-INPUT-CLASS` で報告することの
//! 結合試験（TASK-406 2/2、AUTHOR-7）。
//!
//! 入力は止めず画面と同じ判定で流した上で報告し、診断があれば終了コード 1 にする。接頭辞が `/`（既定）のときは
//! 検査しない。診断の行は入力の値を写さない。

mod common;

use std::path::Path;
use std::process::Command;

use common::{copy_dir, Tmp, SECRET};

use fandhe_workshop_pack::play_run::{run_pack, TestOutcome};
use fandhe_workshop_pack::{profile, source};

/// 最小の教材に、接頭辞（`prefix_yaml` が `None` なら省略）と教材テストを足した一時パック。
fn pack(tag: &str, prefix_yaml: Option<&str>, tests: &str) -> Tmp {
    let t = Tmp::new(tag);
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b"),
        &t.0,
    );
    if let Some(p) = prefix_yaml {
        let w = std::fs::read_to_string(t.0.join("workshop.yaml")).expect("read");
        let w = w.replace(
            "  guidance: \"A\"\n",
            &format!("  guidance: \"A\"\n  profile: {{command_prefix: {p}}}\n"),
        );
        t.put("workshop.yaml", w.as_bytes());
    }
    t.put("tests/playthrough.yaml", tests.as_bytes());
    t
}

/// 模範解答の 1 入力（`first`）と、誤入力 1 件の教材テスト。
fn tests(first: &str, key: &str, raw: &str, expect: &str) -> String {
    format!(
        "playthroughs:\n  - id: pt\n    end: \"main/st-1\"\n    segments:\n      - start: s-1\n        steps:\n          - {{cmd: \"{first}\", expect: \"accepted\"}}\nerror_inputs:\n  - id: e1\n    start: s-1\n    input: {{{key}: \"{raw}\", expect: \"{expect}\"}}\n"
    )
}

struct Run {
    code: Option<i32>,
    out: String,
    err: String,
}

fn cli(dir: &Path) -> Run {
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("test")
        .arg(dir)
        .output()
        .expect("run");
    Run {
        code: o.status.code(),
        out: String::from_utf8_lossy(&o.stdout).into_owned(),
        err: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

fn class_lines(r: &Run) -> Vec<&str> {
    r.out
        .lines()
        .filter(|l| l.contains("E-PLAY-INPUT-CLASS"))
        .collect()
}

const OK_FIRST: &str = "$make-note first-note";

#[test]
fn author7_input_class_dollar_cmd_without_prefix_is_reported() {
    let t = pack(
        "a7c-1",
        Some("\"$\""),
        &tests(OK_FIRST, "cmd", "hello", "accepted"),
    );
    let r = cli(&t.0);
    let lines = class_lines(&r);
    assert_eq!(lines.len(), 1, "{}", r.out);
    assert!(
        lines[0].starts_with("tests/playthrough.yaml: error_input:e1: E-PLAY-INPUT-CLASS: "),
        "{}",
        lines[0]
    );
    assert!(r.out.contains("PASS pt"), "{}", r.out);
    let at = r
        .out
        .lines()
        .position(|l| l.contains("E-PLAY-INPUT-CLASS"))
        .expect("line");
    assert!(at >= 3, "集計の後ろに出る: {}", r.out);
    assert_eq!(r.code, Some(1));
    assert!(r.err.is_empty(), "{}", r.err);
}

#[test]
fn author7_input_class_dollar_text_with_prefix_is_reported() {
    let t = pack(
        "a7c-2",
        Some("\"$\""),
        &tests(OK_FIRST, "text", "$nothing", "rejected:unknown_command"),
    );
    let r = cli(&t.0);
    assert_eq!(class_lines(&r).len(), 1, "{}", r.out);
    assert_eq!(r.code, Some(1));
}

#[test]
fn author7_input_class_empty_prefix_text_is_reported() {
    let t = pack(
        "a7c-3",
        Some("\"\""),
        &tests(
            "make-note first-note",
            "text",
            "make-note first-note",
            "accepted",
        ),
    );
    let r = cli(&t.0);
    // 接頭辞が空では `text:` がコマンドになり食い違う。模範解答の `cmd:` は合っている。
    let lines = class_lines(&r);
    assert_eq!(lines.len(), 1, "{}", r.out);
    assert!(lines[0].contains("error_input:e1"), "{}", lines[0]);
}

#[test]
fn author7_input_class_playthrough_position() {
    let tests = "playthroughs:\n  - id: pt\n    end: \"main/st-1\"\n    segments:\n      - start: s-1\n        steps:\n          - {cmd: \"$make-note first-note\", expect: \"accepted\"}\n          - {cmd: \"hello\", expect: \"accepted\"}\n";
    let t = pack("a7c-4", Some("\"$\""), tests);
    let r = cli(&t.0);
    let lines = class_lines(&r);
    assert_eq!(lines.len(), 1, "{}", r.out);
    assert!(lines[0].contains("playthrough:pt"), "{}", lines[0]);
    assert!(lines[0].contains("segment 1, input 2"), "{}", lines[0]);
}

#[test]
fn author7_input_class_default_prefix_is_not_checked() {
    for (tag, p) in [("a7c-5a", Some("\"/\"")), ("a7c-5b", None)] {
        let t = pack(
            tag,
            p,
            "playthroughs:\n  - id: pt\n    end: \"complete\"\n    segments:\n      - start: s-1\n        steps:\n          - {cmd: \"/make-note first-note\", expect: \"accepted\"}\n          - {choice: \"opt-memo\", expect: \"accepted\"}\n          - {approval: \"approve\", expect: \"accepted\"}\nerror_inputs:\n  - id: e1\n    start: s-1\n    input: {cmd: \"hello\", expect: \"accepted:free_text_unjudged\"}\n",
        );
        let r = cli(&t.0);
        assert!(class_lines(&r).is_empty(), "{}", r.out);
        let lp = source::load(&t.0).expect("load");
        assert!(profile::check_play_inputs(&lp).is_empty());
    }
}

#[test]
fn author7_input_class_matched_keys_exit_0() {
    let t = pack(
        "a7c-6",
        Some("\"$\""),
        &tests(OK_FIRST, "cmd", "$make-note first-note", "accepted"),
    );
    let r = cli(&t.0);
    assert!(class_lines(&r).is_empty(), "{}", r.out);
    assert_eq!(r.code, Some(0), "{}", r.out);
}

#[test]
fn author7_input_class_runs_inputs_anyway() {
    let t = pack(
        "a7c-7",
        Some("\"$\""),
        &tests(OK_FIRST, "text", "$nothing", "rejected:unknown_command"),
    );
    match run_pack(&t.0) {
        TestOutcome::Ran(rep) => {
            assert_eq!(rep.input_class.len(), 1);
            assert!(
                rep.error_inputs.iter().all(|c| c.failure.is_none()),
                "{:?}",
                rep.error_inputs
            );
            assert!(!rep.all_passed());
        }
        other => panic!("expected Ran: {other:?}"),
    }
}

#[test]
fn author7_input_class_diagnostics_before_failure_diagnostics() {
    let t = pack(
        "a7c-8",
        Some("\"$\""),
        &tests(OK_FIRST, "cmd", "hello", "rejected:unknown_command"),
    );
    let r = cli(&t.0);
    let a = r
        .out
        .lines()
        .position(|l| l.contains("E-PLAY-INPUT-CLASS"))
        .expect("class");
    let b = r
        .out
        .lines()
        .position(|l| l.contains("E-PLAY-CATEGORY-MISMATCH"))
        .expect("mismatch");
    assert!(a < b, "{}", r.out);
    assert_eq!(r.code, Some(1));
}

#[test]
fn author7_input_class_does_not_echo_values() {
    let raw = format!("hello {SECRET}");
    let t = pack(
        "a7c-9",
        Some("\"$\""),
        &tests(OK_FIRST, "cmd", &raw, "accepted"),
    );
    let r = cli(&t.0);
    assert_eq!(class_lines(&r).len(), 1, "{}", r.out);
    assert!(
        !r.out.contains(SECRET) && !r.err.contains(SECRET),
        "{}",
        r.out
    );
}

#[test]
fn author7_input_class_cli_matches_library() {
    let t = pack(
        "a7c-10",
        Some("\"$\""),
        &tests(OK_FIRST, "cmd", "hello", "accepted"),
    );
    let lp = source::load(&t.0).expect("load");
    let want: Vec<String> = profile::check_play_inputs(&lp)
        .iter()
        .map(|d| d.to_string())
        .collect();
    assert_eq!(want.len(), 1);
    let r = cli(&t.0);
    let got: Vec<String> = class_lines(&r).iter().map(|s| s.to_string()).collect();
    assert_eq!(got, want);
}
