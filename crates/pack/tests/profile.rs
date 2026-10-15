//! `pack.profile` の診断（`E-PROFILE-PREFIX`・`E-PROFILE-COMMAND-NAME`・`E-PARSE`）の結合試験（TASK-52 1/2）。

mod common;

use common::{Tmp, CHAPTER};
use fandhe_workshop_contract::profile::{InputClass, ProfileSetting};
use fandhe_workshop_pack::compile::compile;
use fandhe_workshop_pack::profile::classify_play_input;
use fandhe_workshop_pack::source;

/// `profile` は YAML のフロー形式の文字列で渡す（接頭辞は必ず引用符で囲む）。
fn pack_with(profile: &str, commands: &str) -> Tmp {
    let t = Tmp::new("profile");
    let mut w = format!(
        "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, profile: {profile}}}\n"
    );
    w.push_str(commands);
    t.put("workshop.yaml", w.as_bytes());
    t.put("chapters/c1/chapter.yaml", CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

fn diags(t: &Tmp) -> Vec<String> {
    let lp = source::load(&t.0).expect("load");
    match compile(&lp) {
        Ok(_) => vec![],
        Err(d) => d.iter().map(|x| x.to_string()).collect(),
    }
}

#[test]
fn bad_prefix_is_reported_once() {
    for p in ["abcde", "a b", "ab", "-x", "1"] {
        let t = pack_with(&format!("{{command_prefix: \"{p}\"}}"), "");
        let d = diags(&t);
        assert_eq!(d.len(), 1, "{p}: {d:?}");
        assert!(
            d[0].starts_with("workshop.yaml: pack:profile.command_prefix: E-PROFILE-PREFIX"),
            "{}",
            d[0]
        );
        assert!(!d[0].contains(p), "diagnostic echoes the value: {}", d[0]);
    }
}

#[test]
fn type_errors_are_parse_errors() {
    for p in [
        "{command_prefix: 5}",
        "{unknown: 1}",
        "{input_field: maybe}",
        "[1]",
    ] {
        let t = pack_with(p, "");
        let d = diags(&t);
        assert_eq!(d.len(), 1, "{p}: {d:?}");
        assert!(
            d[0].starts_with("workshop.yaml: pack:profile: E-PARSE"),
            "{}",
            d[0]
        );
    }
}

#[test]
fn empty_prefix_checks_command_names() {
    let cmds = "commands:\n  ls: builtin\n  \"ls a\": builtin\n  \"\": builtin\n";
    let t = pack_with("{command_prefix: \"\"}", cmds);
    let d = diags(&t);
    assert_eq!(d.len(), 2, "{d:?}");
    assert!(
        d.iter().all(|x| x.contains("E-PROFILE-COMMAND-NAME")),
        "{d:?}"
    );
    assert!(d.iter().any(|x| x.contains("command:#1")), "{d:?}");
    assert!(d.iter().any(|x| x.contains("command:#3")), "{d:?}");
    assert!(d.iter().all(|x| !x.contains("ls a")), "{d:?}");
    let t = pack_with("{command_prefix: \"/\"}", cmds);
    assert!(diags(&t).is_empty());
}

#[test]
fn valid_prefixes_compile_and_stay_in_ir() {
    for p in ["$", ""] {
        let t = pack_with(&format!("{{command_prefix: \"{p}\"}}"), "");
        let lp = source::load(&t.0).expect("load");
        let ir = compile(&lp).expect("compile");
        let v = ir.pack.profile.expect("profile kept");
        assert_eq!(v["command_prefix"], p);
    }
}

#[test]
fn play_input_uses_shared_split() {
    let s = ProfileSetting {
        command_prefix: "$".into(),
        ..Default::default()
    };
    assert_eq!(classify_play_input(&s, "$ls"), InputClass::Command);
    assert_eq!(classify_play_input(&s, "ls"), InputClass::Text);
}
