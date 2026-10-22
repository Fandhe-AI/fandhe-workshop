//! 入力欄の有無と関連する診断（`E-PROFILE-TEXT`・`E-PROFILE-NO-INPUT`・`E-PLAY-INPUT-CLASS`）の結合試験（TASK-52 2/2）。

mod common;

use common::Tmp;
use fandhe_workshop_pack::compile::compile;
use fandhe_workshop_pack::source;

fn chapter_pack(profile: &str, steps: &str, tests: Option<&str>) -> Tmp {
    let t = Tmp::new("profile-input");
    t.put(
        "workshop.yaml",
        format!(
            "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, profile: {profile}}}\n"
        )
        .as_bytes(),
    );
    let ch = format!("id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n{steps}");
    t.put("chapters/c1/chapter.yaml", ch.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    if let Some(x) = tests {
        t.put("tests/playthrough.yaml", x.as_bytes());
    }
    t
}

const TEXT_STEP: &str =
    "  - {id: a, title: A, body: a.md, input: {kind: text, min: 1, max: 100}, confirm: {kind: approve}, final: true}\n";

fn diags(t: &Tmp) -> Vec<String> {
    let lp = source::load(&t.0).expect("load");
    match compile(&lp) {
        Ok(_) => vec![],
        Err(d) => d.iter().map(|x| x.to_string()).collect(),
    }
}

fn with_code(t: &Tmp, code: &str) -> Vec<String> {
    diags(t).into_iter().filter(|d| d.contains(code)).collect()
}

#[test]
fn text_step_with_empty_prefix() {
    let t = chapter_pack("{command_prefix: \"\"}", TEXT_STEP, None);
    let d = with_code(&t, "E-PROFILE-TEXT");
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(
        d[0].starts_with("chapters/c1/chapter.yaml: step:a: E-PROFILE-TEXT"),
        "{}",
        d[0]
    );
}

#[test]
fn text_rule_with_empty_prefix_but_not_false() {
    let rule = |v: &str| {
        format!(
            "  - {{id: a, title: A, body: a.md, final: true, input: {{kind: command}}, confirm: {{kind: approve}}, rules: [{{id: r, on: {{text: {v}}}, outcome: {{kind: accept}}}}]}}\n"
        )
    };
    let t = chapter_pack("{command_prefix: \"\"}", &rule("true"), None);
    assert_eq!(with_code(&t, "E-PROFILE-TEXT").len(), 1);
    let t = chapter_pack("{command_prefix: \"\"}", &rule("false"), None);
    assert!(with_code(&t, "E-PROFILE-TEXT").is_empty());
}

#[test]
fn text_allowed_with_other_prefix() {
    for p in ["{command_prefix: \"$\"}", "{}"] {
        let t = chapter_pack(p, TEXT_STEP, None);
        assert!(with_code(&t, "E-PROFILE").is_empty(), "{p}");
    }
}

#[test]
fn hidden_rejects_command_and_text() {
    let steps = concat!(
        "  - {id: a, title: A, body: a.md, input: {kind: command}, confirm: {kind: approve}, next: [{to: b}]}\n",
        "  - {id: b, title: B, input: {kind: text, min: 1, max: 9}, confirm: {kind: approve}, next: [{to: c}]}\n",
        "  - {id: c, title: C, input: {kind: approval}, confirm: {kind: approve}, rules: [{id: r, on: {command: x}, outcome: {kind: accept}}], next: [{to: d}]}\n",
        "  - {id: d, title: D, rules: [{id: r, on: {text: true}, outcome: {kind: accept}}], final: true}\n",
    );
    let t = chapter_pack("{input_field: hidden}", steps, None);
    let d = with_code(&t, "E-PROFILE-NO-INPUT");
    assert_eq!(d.len(), 4, "{d:?}");
    for id in ["a", "b", "c", "d"] {
        let want = format!("chapters/c1/chapter.yaml: step:{id}: E-PROFILE-NO-INPUT");
        assert!(d.iter().any(|x| x.starts_with(&want)), "{id}: {d:?}");
    }
}

#[test]
fn hidden_allows_non_typing_inputs() {
    let steps = concat!(
        "  - {id: a, title: A, body: a.md, input: {kind: approval}, confirm: {kind: approve}, next: [{to: b}]}\n",
        "  - {id: b, title: B, input: {kind: wait}, confirm: {kind: approve}, processing: [{wait: 1}], final: true}\n",
    );
    let t = chapter_pack("{input_field: hidden}", steps, None);
    assert!(with_code(&t, "E-PROFILE").is_empty(), "{:?}", diags(&t));
}

#[test]
fn empty_prefix_and_hidden_text_reports_both() {
    let t = chapter_pack(
        "{command_prefix: \"\", input_field: hidden}",
        TEXT_STEP,
        None,
    );
    assert_eq!(with_code(&t, "E-PROFILE-TEXT").len(), 1);
    assert_eq!(with_code(&t, "E-PROFILE-NO-INPUT").len(), 1);
}

#[test]
fn broken_profile_does_not_cascade() {
    for p in [
        "{command_prefix: \"abcde\", input_field: hidden}",
        "{input_field: maybe}",
    ] {
        let t = chapter_pack(p, TEXT_STEP, None);
        let d = diags(&t);
        assert_eq!(d.len(), 1, "{p}: {d:?}");
        assert!(!d[0].contains("E-PROFILE-TEXT") && !d[0].contains("E-PROFILE-NO-INPUT"));
    }
}

fn play(profile: &str, tests: &str) -> Vec<String> {
    let t = chapter_pack(profile, TEXT_STEP, Some(tests));
    let lp = source::load(&t.0).expect("load");
    fandhe_workshop_pack::profile::check_play_inputs(&lp)
        .iter()
        .map(|d| d.to_string())
        .collect()
}

const TESTS: &str = concat!(
    "playthroughs:\n  - {id: pt, segments: [{steps: [{cmd: ls}, {cmd: '$ls'}]}]}\n",
    "error_inputs:\n  - {id: ei, pre: [{text: hello}], input: {text: '$x'}}\n",
    "text_checks:\n  - {id: tc, input: {text: plain}}\n",
);

#[test]
fn play_class_empty_prefix() {
    let d = play("{command_prefix: \"\"}", TESTS);
    // 接頭辞なしでは text: はすべてコマンドになる（3 件）。cmd: は常にコマンドで該当しない。
    assert_eq!(d.len(), 3, "{d:?}");
    assert!(d.iter().all(|x| x.contains("E-PLAY-INPUT-CLASS")), "{d:?}");
    assert!(d.iter().all(|x| x.starts_with("tests/playthrough.yaml: ")));
    let j = d.join("\n");
    assert!(
        j.contains("error_input:ei") && j.contains("text_check:tc"),
        "{d:?}"
    );
    assert!(!j.contains("playthrough:pt"), "{d:?}");
}

#[test]
fn play_class_dollar() {
    let d = play("{command_prefix: \"$\"}", TESTS);
    let j = d.join("\n");
    assert!(j.contains("playthrough:pt"), "{d:?}");
    assert!(j.contains("error_input:ei"), "{d:?}");
    assert!(!j.contains("text_check:tc"), "{d:?}");
    assert_eq!(d.len(), 2, "{d:?}");
}

#[test]
fn play_class_default_prefix_skips() {
    assert!(play("{}", TESTS).is_empty());
    let hello = "playthroughs:\n  - {segments: [{steps: [{cmd: hello}]}]}\n";
    assert!(play("{command_prefix: \"/\"}", hello).is_empty());
}

#[test]
fn play_class_unsafe_id_uses_position_and_ignores_malformed() {
    let d = play(
        "{command_prefix: \"$\"}",
        "playthroughs:\n  - {id: 'a b', segments: [{steps: [{cmd: ls}]}]}\n  - {segments: x}\n  - {segments: [5, {steps: 3}, {steps: [{cmd: 4}]}]}\n",
    );
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].contains("playthrough:#1"), "{}", d[0]);
    assert!(!d[0].contains("a b"));
}

#[test]
fn diagnostics_do_not_echo_values() {
    let d = play(
        "{command_prefix: \"$\"}",
        "playthroughs:\n  - {id: pt, segments: [{steps: [{cmd: SECRETCMD}]}]}\n",
    );
    assert_eq!(d.len(), 1);
    assert!(!d[0].contains("SECRETCMD"), "{}", d[0]);
}
