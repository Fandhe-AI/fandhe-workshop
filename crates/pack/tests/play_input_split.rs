//! `test` の `cmd:`・`text:` を画面と同じ関数で分けることの結合試験（TASK-406 1/2、AUTHOR-7）。
//!
//! 同じ教材を `compile` した IR に、画面の送信口（`RecordedSession::submit`）と `fandhe-pack test` の実行系
//! （`play_run::run_pack`）へ同じ文字列を送り、判定が一致することを確かめる。接頭辞を変えた教材では、
//! 鍵（`cmd`・`text`）ではなく文字列の先頭で分かれる。

mod common;

use std::path::Path;
use std::process::Command;

use common::{copy_dir, Tmp, SECRET};

use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::Session;
use fandhe_workshop_pack::compile::compile;
use fandhe_workshop_pack::play_run::{run_pack, TestOutcome};
use fandhe_workshop_pack::source;

/// 最小の教材（コマンドの Step から始まる）に、接頭辞を足した一時パック。`prefix_yaml` は YAML の値。
fn pack(tag: &str, prefix_yaml: &str, tests: &str) -> Tmp {
    let t = Tmp::new(tag);
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b"),
        &t.0,
    );
    let w = std::fs::read_to_string(t.0.join("workshop.yaml")).expect("read");
    let w = w.replace(
        "  guidance: \"A\"\n",
        &format!("  guidance: \"A\"\n  profile: {{command_prefix: {prefix_yaml}}}\n"),
    );
    t.put("workshop.yaml", w.as_bytes());
    t.put("tests/playthrough.yaml", tests.as_bytes());
    t
}

/// 画面と同じ経路で 1 回送ったときの判定の語。
fn screen_word(t: &Tmp, start: &str, prefix: &str, raw: &str) -> String {
    let lp = source::load(&t.0).expect("load");
    let ir = compile(&lp).expect("compile");
    let node = ir
        .starts
        .iter()
        .find(|s| s.id == start)
        .expect("start")
        .node
        .clone();
    let session = Session::entered_at(ir, &node);
    let mut rs = RecordedSession::new(session, start);
    let events = rs.submit(prefix, raw.to_string()).to_vec();
    match events.iter().find_map(|e| match e {
        Event::Judged { judgment, .. } => Some(judgment.clone()),
        _ => None,
    }) {
        Some(Judgment::Accepted(note)) if note.is_some() => "accepted:free_text_unjudged".into(),
        Some(Judgment::Accepted(_)) => "accepted".into(),
        Some(Judgment::Rejected(r)) => format!("rejected:{}", r.category.as_str()),
        Some(Judgment::Clarified(_)) => "clarified".into(),
        _ if events.iter().any(|e| matches!(e, Event::Accepted { .. })) => "accepted".into(),
        _ => "none".into(),
    }
}

/// 教材テストは模範解答を 1 件以上要るので、接頭辞つきの 1 入力だけの模範解答を前に置く。
fn error_tests(key: &str, raw: &str, expect: &str) -> String {
    format!(
        "playthroughs:\n  - id: pt\n    end: \"main/st-1\"\n    segments:\n      - start: s-1\n        steps:\n          - {{cmd: \"$make-note first-note\", expect: \"accepted\"}}\nerror_inputs:\n  - id: e1\n    start: s-1\n    input: {{{key}: \"{raw}\", expect: \"{expect}\"}}\n"
    )
}

/// `test` の実行系が `key: raw` を送ったときに、画面の語と一致した（期待が通った）か。
fn test_matches_screen(
    tag: &str,
    prefix_yaml: &str,
    prefix: &str,
    key: &str,
    raw: &str,
) -> (String, bool) {
    let probe = pack(tag, prefix_yaml, &error_tests(key, raw, "accepted"));
    let word = screen_word(&probe, "s-1", prefix, raw);
    let t = pack(tag, prefix_yaml, &error_tests(key, raw, &word));
    let ok = match run_pack(&t.0) {
        TestOutcome::Ran(r) => r.error_inputs.iter().all(|c| c.failure.is_none()),
        other => panic!("expected Ran: {other:?}"),
    };
    (word, ok)
}

#[test]
fn author7_dollar_prefix_cmd_without_prefix_runs_as_text_like_screen() {
    let raw = format!("hello {SECRET}");
    let (word, ok) = test_matches_screen("a7-1", "\"$\"", "$", "cmd", &raw);
    assert!(ok, "screen={word}");
    let (as_text, _) = test_matches_screen("a7-1b", "\"$\"", "$", "text", &raw);
    assert_eq!(word, as_text, "cmd: は接頭辞が無ければ text: と同じ");
}

#[test]
fn author7_dollar_prefix_text_with_prefix_runs_as_command_like_screen() {
    let raw = "$nothing";
    let (word, ok) = test_matches_screen("a7-2", "\"$\"", "$", "text", raw);
    assert!(ok, "screen={word}");
    assert_eq!(word, "rejected:unknown_command");
    let (as_cmd, _) = test_matches_screen("a7-2b", "\"$\"", "$", "cmd", raw);
    assert_eq!(word, as_cmd);
}

#[test]
fn author7_dollar_prefix_cmd_with_prefix_is_command() {
    let t = pack(
        "a7-3",
        "\"$\"",
        &error_tests("cmd", "$make-note first-note", "accepted"),
    );
    assert_eq!(
        screen_word(&t, "s-1", "$", "$make-note first-note"),
        "accepted"
    );
    match run_pack(&t.0) {
        TestOutcome::Ran(r) => assert!(r.all_passed(), "{:?} {:?}", r.playthroughs, r.error_inputs),
        other => panic!("expected Ran: {other:?}"),
    }
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("test")
        .arg(&t.0)
        .output()
        .expect("run");
    assert_eq!(o.status.code(), Some(0));
}

#[test]
fn author7_empty_prefix_text_runs_as_command_like_screen() {
    let raw = "make-note first-note";
    let (word, ok) = test_matches_screen("a7-4", "\"\"", "", "text", raw);
    assert!(ok, "screen={word}");
    assert_eq!(word, "accepted");
}

#[test]
fn author7_default_prefix_cmd_without_slash_matches_screen() {
    let raw = format!("hello {SECRET}");
    let (word, ok) = test_matches_screen("a7-5", "\"/\"", "/", "cmd", &raw);
    assert!(ok, "screen={word}");
}

#[test]
fn author7_output_does_not_echo_input_values() {
    let raw = format!("$make-note {SECRET}");
    let t = pack("a7-6", "\"$\"", &error_tests("cmd", &raw, "accepted"));
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("test")
        .arg(&t.0)
        .output()
        .expect("run");
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(!all.contains(SECRET), "{all}");
}

/// 自然文の Step だけの教材。画面では接頭辞を含まない文字列が自然文として受け付けられる。
fn text_pack(tag: &str, tests: &str) -> Tmp {
    let t = Tmp::new(tag);
    t.put(
        "workshop.yaml",
        b"pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A, profile: {command_prefix: \"$\"}}\n",
    );
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - {id: a, title: A, body: a.md, input: {kind: text, min: 1, max: 100}, confirm: {kind: approve}, final: true}\n",
    );
    t.put("chapters/c1/a.md", b"# a\n");
    t.put("tests/playthrough.yaml", tests.as_bytes());
    t
}

#[test]
fn author7_dollar_prefix_cmd_without_prefix_is_free_text_on_text_step() {
    let tests = |expect: &str| {
        format!(
            "playthroughs:\n  - id: pt\n    end: \"c1/a\"\n    segments:\n      - start: s1\n        steps:\n          - {{text: hello, expect: \"{expect}\"}}\nerror_inputs:\n  - id: e1\n    start: s1\n    input: {{cmd: hello, expect: \"{expect}\"}}\n"
        )
    };
    let probe = text_pack("a7-7", &tests("accepted"));
    let word = screen_word(&probe, "s1", "$", "hello");
    assert_eq!(word, "accepted");
    let t = text_pack("a7-7b", &tests(&word));
    match run_pack(&t.0) {
        TestOutcome::Ran(r) => assert!(
            r.error_inputs.iter().all(|c| c.failure.is_none()),
            "{:?}",
            r.error_inputs
        ),
        other => panic!("expected Ran: {other:?}"),
    }
}
