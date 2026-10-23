//! `fandhe-pack test` の枠と終了コード（TASK-404 1/3、AUTHOR-4）の結合試験。
//!
//! 関門に失敗した教材パックでは試験が流れないこと（ライブラリの `run_pack` が `NotRun` を返す）、流れた試験の
//! 合否が終了コード 0 / 1 になること、標準エラー出力が空であることを確かめる。模範解答の PASS の行（TASK-404 2/3）も見る。FAIL・集計の行は後続のタスクで足す。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{copy_dir, testdata, Tmp};

use fandhe_workshop_pack::play_run::{run_pack, Failure, Observed, Report, TestOutcome};
use fandhe_workshop_pack::play_script::Unsupported;

fn min_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b")
}

fn defects(rel: &str) -> PathBuf {
    testdata("pack-defects").join(rel)
}

/// 土台に欠陥の重ねを足した一時パック。
fn overlay(base: &str, over: &str, tag: &str) -> Tmp {
    let t = Tmp::new(tag);
    copy_dir(&defects(base), &t.0);
    copy_dir(&defects(over), &t.0);
    t
}

struct Run {
    code: Option<i32>,
    out: String,
    err: String,
}

fn cli(cmd: &str, dir: &Path) -> Run {
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg(cmd)
        .arg(dir)
        .output()
        .expect("run fandhe-pack");
    Run {
        code: o.status.code(),
        out: String::from_utf8_lossy(&o.stdout).into_owned(),
        err: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

fn assert_exit_0_or_1(r: &Run) {
    assert!(matches!(r.code, Some(0 | 1)), "exit {:?}", r.code);
    assert!(r.err.is_empty(), "stderr: {}", r.err);
}

fn ran(dir: &Path) -> Report {
    match run_pack(dir) {
        TestOutcome::Ran(r) => r,
        other => panic!("expected Ran: {other:?}"),
    }
}

fn copy_min(tag: &str) -> Tmp {
    let t = Tmp::new(tag);
    copy_dir(&min_pack(), &t.0);
    t
}

/// 最小の教材に、処理中の進行と確認の操作を含む実際に通る模範解答を与えた一時パック。
fn min_with_ticks() -> Tmp {
    let t = copy_min("t404-min-ticks");
    t.put(
        "tests/playthrough.yaml",
        [
            "playthroughs:",
            "  - id: pt-min",
            "    end: main/st-2",
            "    segments:",
            "      - start: s-1",
            "        steps:",
            "          - {cmd: \"/make-note first-note\", expect: accepted}",
            "          - {tick: true, expect: progress}",
            "          - {tick: true, expect: progress}",
            "          - {tick: true, expect: progress}",
            "          - {tick: true, expect: progress}",
            "          - {tick: true, expect: progress}",
            "          - {ui: \"v-files notes/draft\", expect: achieved}",
            "error_inputs:",
            "  - id: e-unknown",
            "    start: s-1",
            "    input: {cmd: \"/nothing\", expect: \"rejected:unknown_command\"}",
        ]
        .join("\n")
        .as_bytes(),
    );
    t
}

#[test]
fn author4_min_pack_with_ticks_exits_0() {
    let t = min_with_ticks();
    let rep = ran(&t.0);
    assert!(rep.all_passed(), "{rep:?}");
    assert_eq!(cli("test", &t.0).code, Some(0));
}

#[test]
fn author4_stale_fixture_is_a_failure_not_a_pass() {
    // 処理中の Step に進行なしで次を送る教材テストは、エンジンが拒否する（busy）ので不合格になる。
    let rep = ran(&min_pack());
    assert!(!rep.all_passed());
    assert_eq!(cli("test", &min_pack()).code, Some(1));
}

#[test]
fn author4_correct_packs_exit_0() {
    for dir in [defects("base/format-a"), defects("base/format-b")] {
        let r = cli("test", &dir);
        assert_eq!(r.code, Some(0), "{dir:?}: {}", r.out);
        assert!(r.err.is_empty());
        assert!(!r.out.contains("E-") && !r.out.contains("NG:"), "{}", r.out);
        assert!(ran(&dir).all_passed());
    }
}

#[test]
fn author4_failed_checks_do_not_run_tests() {
    for f in ["a", "b"] {
        let t = overlay(
            &format!("base/format-{f}"),
            &format!("v1/{f}/D01"),
            &format!("t404-d01-{f}"),
        );
        let test = cli("test", &t.0);
        let validate = cli("validate", &t.0);
        assert_eq!(test.code, Some(1));
        assert_exit_0_or_1(&test);
        assert_eq!(test.out, validate.out, "same output as validate");
        assert!(matches!(run_pack(&t.0), TestOutcome::NotRun { .. }));
    }
}

#[test]
fn author4_failing_tests_exit_1() {
    for f in ["a", "b"] {
        for (id, case, error_input) in [("D09", "pt-main", false), ("D10", "e03", true)] {
            let t = overlay(
                &format!("base/format-{f}"),
                &format!("v1/{f}/{id}"),
                &format!("t404-{id}-{f}"),
            );
            let r = cli("test", &t.0);
            assert_eq!(r.code, Some(1), "{id} {f}");
            assert_exit_0_or_1(&r);
            let report = ran(&t.0);
            let failed: Vec<_> = report
                .playthroughs
                .iter()
                .chain(&report.error_inputs)
                .filter(|c| !c.passed())
                .collect();
            assert_eq!(failed.len(), 1, "{id} {f}: {failed:?}");
            assert_eq!(failed[0].id, case);
            if error_input {
                assert!(matches!(
                    failed[0].failure,
                    Some(Failure::Mismatch {
                        observed: Observed::Rejected(_),
                        ..
                    })
                ));
            }
        }
    }
}

#[test]
fn author4_unreadable_pack_exits_1() {
    let r = cli("test", Path::new("/nonexistent/fandhe-test-404"));
    assert_eq!(r.code, Some(1));
    assert_exit_0_or_1(&r);
    assert_eq!(
        r.out.lines().last(),
        Some("NG: 診断 1 件（読み込めません）")
    );
    assert!(!r.out.contains("nonexistent"));
}

#[test]
fn author4_malformed_script_is_parse_error() {
    let t = copy_min("t404-malformed");
    t.put(
        "tests/playthrough.yaml",
        b"playthroughs:\n  - id: pt\n    end: complete\n    segments:\n      - start: s-1\n        steps:\n          - {cmd: SECRET-X, text: SECRET-Y, expect: accepted}\n",
    );
    let r = cli("test", &t.0);
    assert_eq!(r.code, Some(1));
    assert_exit_0_or_1(&r);
    assert!(r.out.contains("playthrough:pt: E-PARSE"), "{}", r.out);
    assert!(!r.out.contains("SECRET"));
    assert!(matches!(run_pack(&t.0), TestOutcome::NotRun { .. }));
}

#[test]
fn author4_missing_or_empty_tests_is_structure_error() {
    let t = copy_min("t404-notests");
    fs::remove_file(t.0.join("tests/playthrough.yaml")).expect("remove");
    let r = cli("test", &t.0);
    assert_eq!(r.code, Some(1));
    assert_exit_0_or_1(&r);
    assert!(r.out.contains("E-STRUCTURE"), "{}", r.out);

    let t = copy_min("t404-empty");
    t.put("tests/playthrough.yaml", b"playthroughs: []\n");
    let r = cli("test", &t.0);
    assert_eq!(r.code, Some(1));
    assert!(r.out.contains("E-STRUCTURE"), "{}", r.out);
    assert!(matches!(run_pack(&t.0), TestOutcome::NotRun { .. }));
}

#[test]
fn author4_unsupported_sections_fail_closed() {
    let base = fs::read_to_string(min_pack().join("tests/playthrough.yaml")).expect("read");
    let t = copy_min("t404-textchecks");
    t.put(
        "tests/playthrough.yaml",
        format!("{base}text_checks:\n  - {{id: tc, anything: 1}}\n").as_bytes(),
    );
    let r = cli("test", &t.0);
    assert_exit_0_or_1(&r);
    assert_eq!(r.code, Some(1));
    assert_eq!(ran(&t.0).unsupported, vec![Unsupported::TextChecks]);

    let t = copy_min("t404-asserts");
    t.put(
        "tests/playthrough.yaml",
        base.replace(
            "    end: \"complete\"\n",
            "    end: \"complete\"\n    asserts: []\n",
        )
        .as_bytes(),
    );
    let r = cli("test", &t.0);
    assert_exit_0_or_1(&r);
    assert_eq!(r.code, Some(1));
    assert_eq!(ran(&t.0).unsupported, vec![Unsupported::Asserts]);
}

/// `min_with_ticks` の教材テストの先頭の模範解答（`pt-min`）の YAML を、ID を差し替えて 2 本並べた一時パック。
fn two_playthroughs(first: &str, second: &str, tag: &str) -> Tmp {
    let t = copy_min(tag);
    let one = |id: &str| {
        [
            format!("  - id: {id}"),
            "    end: main/st-2".to_string(),
            "    segments:".to_string(),
            "      - start: s-1".to_string(),
            "        steps:".to_string(),
            "          - {cmd: \"/make-note first-note\", expect: accepted}".to_string(),
            "          - {tick: true, expect: progress}".to_string(),
            "          - {tick: true, expect: progress}".to_string(),
            "          - {tick: true, expect: progress}".to_string(),
            "          - {tick: true, expect: progress}".to_string(),
            "          - {tick: true, expect: progress}".to_string(),
            "          - {ui: \"v-files notes/draft\", expect: achieved}".to_string(),
        ]
        .join("\n")
    };
    let text = format!("playthroughs:\n{}\n{}\n", one(first), one(second));
    t.put("tests/playthrough.yaml", text.as_bytes());
    t
}

#[test]
fn author4_pass_line_per_playthrough() {
    for dir in [defects("base/format-a"), defects("base/format-b")] {
        let r = cli("test", &dir);
        assert_eq!(r.code, Some(0), "{dir:?}: {}", r.out);
        assert_eq!(r.out, "PASS pt-main（入力 2 件）\n");
        assert!(r.err.is_empty());
    }
}

#[test]
fn author4_pass_line_counts_ticks() {
    let t = min_with_ticks();
    let r = cli("test", &t.0);
    assert_eq!(r.code, Some(0), "{}", r.out);
    // cmd 1・tick 5・ui 1。誤入力 e-unknown の行は本版では出ない。
    assert_eq!(r.out, "PASS pt-min（入力 7 件）\n");
}

#[test]
fn author4_failed_playthrough_has_no_pass_line() {
    for f in ["a", "b"] {
        let t = overlay(
            &format!("base/format-{f}"),
            &format!("v1/{f}/D09"),
            &format!("t404-nopass-{f}"),
        );
        let r = cli("test", &t.0);
        assert_eq!(r.code, Some(1));
        assert!(
            !r.out.lines().any(|l| l.starts_with("PASS pt-main")),
            "{}",
            r.out
        );
    }
    let r = cli("test", &min_pack());
    assert_eq!(r.code, Some(1));
    assert!(!r.out.contains("PASS"), "{}", r.out);
}

#[test]
fn author4_pass_line_order_follows_script() {
    let t = two_playthroughs("pt-zeta", "pt-alpha", "t404-order");
    let r = cli("test", &t.0);
    assert_eq!(r.code, Some(0), "{}", r.out);
    assert_eq!(
        r.out,
        "PASS pt-zeta（入力 7 件）\nPASS pt-alpha（入力 7 件）\n"
    );
}

#[test]
fn author4_pass_line_does_not_echo_unsafe_id() {
    let t = two_playthroughs("\"SECRET-X OK: y\"", "pt-ok", "t404-unsafe-id");
    let r = cli("test", &t.0);
    assert_eq!(r.code, Some(0), "{}", r.out);
    assert_eq!(r.out, "PASS #1（入力 7 件）\nPASS pt-ok（入力 7 件）\n");
    assert!(!r.out.contains("SECRET"));
}
