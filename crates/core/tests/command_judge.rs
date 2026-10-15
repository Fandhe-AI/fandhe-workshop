//! コマンド入力の宣言との照合（TASK-262 2/2、JUDGE-1）の試験。
//!
//! PoC-3 の模範解答・誤入力のうち command に関わる組を、試作 bbs-tutorial の format-b の宣言の写しで確かめる
//! （出所: PoC-3 `pack-declarative-format/test-data/playthrough-model.md`・`error-inputs.md`）。

mod common;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::input_spec::{ArgShape, ArgSpec, CommandSpec};
use fandhe_workshop_contract::judgment::{LexedArg, LexedCommand, RejectCategory};
use fandhe_workshop_core::command_judge::{
    is_unknown_command, judge_command, CommandVerdict, MatchedCommand, ResolvedArg,
};
use fandhe_workshop_core::command_lex::{lex_command, CommandLexError};
use proptest::prelude::*;

fn arg(name: &str, positional: bool, required: bool, shape: ArgShape) -> ArgSpec {
    ArgSpec {
        name: name.to_string(),
        positional,
        required,
        shape,
    }
}

fn spec(name: &str, args: Vec<ArgSpec>) -> CommandSpec {
    CommandSpec {
        name: name.to_string(),
        args,
    }
}

fn judge(s: &CommandSpec, raw: &str) -> CommandVerdict {
    let lexed = lex_command("/", raw).unwrap_or_else(|e| panic!("{raw:?}: {e}"));
    judge_command(s, &lexed)
}

fn matched(pairs: &[(&str, Option<&str>)]) -> CommandVerdict {
    CommandVerdict::Matched(MatchedCommand {
        args: pairs
            .iter()
            .map(|(n, v)| ResolvedArg {
                name: n.to_string(),
                value: v.map(str::to_string),
            })
            .collect(),
    })
}

fn new_idea_ch1() -> CommandSpec {
    spec(
        "new-idea",
        vec![arg("name", true, false, ArgShape::KebabCase)],
    )
}
fn new_idea_ch2() -> CommandSpec {
    spec(
        "new-idea",
        vec![arg("name", true, true, ArgShape::KebabCase)],
    )
}
fn iit_ch7() -> CommandSpec {
    spec(
        "implement-issue-tree",
        vec![
            arg("parent", true, true, ArgShape::Digits),
            arg("external_checks", false, true, ArgShape::Any),
        ],
    )
}
fn iit_run() -> CommandSpec {
    spec(
        "implement-issue-tree",
        vec![
            arg("parent", true, true, ArgShape::Digits),
            arg("external_checks", false, false, ArgShape::Any),
        ],
    )
}
fn cit_ch13() -> CommandSpec {
    spec(
        "create-issue-tree",
        vec![
            arg("input", true, true, ArgShape::Any),
            arg("root", false, false, ArgShape::Digits),
        ],
    )
}

const KNOWN: [&str; 10] = [
    "help",
    "new-idea",
    "advance-phase",
    "create-pr",
    "create-commit",
    "init-claude",
    "setup-repo-guards",
    "create-issue-tree",
    "implement-issue-tree",
    "update-claude",
];

#[test]
fn poc3_pairs() {
    // 模範解答
    assert_eq!(judge(&spec("help", vec![]), "/help"), matched(&[]));
    assert_eq!(
        judge(&new_idea_ch1(), "/new-idea"),
        matched(&[("name", None)])
    );
    assert_eq!(
        judge(&new_idea_ch2(), "/new-idea bbs-with-login"),
        matched(&[("name", Some("bbs-with-login"))])
    );
    assert_eq!(
        judge(&iit_ch7(), "/implement-issue-tree 42 --external-checks []"),
        matched(&[("parent", Some("42")), ("external_checks", Some("[]"))])
    );
    assert_eq!(
        judge(&iit_run(), "/implement-issue-tree 42"),
        matched(&[("parent", Some("42")), ("external_checks", None)])
    );
    assert_eq!(
        judge(&cit_ch13(), "/create-issue-tree search-diff.md --root 42"),
        matched(&[("input", Some("search-diff.md")), ("root", Some("42"))])
    );
    assert_eq!(
        judge(&cit_ch13(), "/create-issue-tree search-diff.md"),
        matched(&[("input", Some("search-diff.md")), ("root", None)])
    );
    assert_eq!(
        judge(&spec("init-claude", vec![]), "/init-claude"),
        matched(&[])
    );
    let guards = spec(
        "setup-repo-guards",
        vec![
            arg("repo", true, true, ArgShape::Any),
            arg("visibility", true, true, ArgShape::Any),
        ],
    );
    assert_eq!(
        judge(&guards, "/setup-repo-guards owner/repo public"),
        matched(&[("repo", Some("owner/repo")), ("visibility", Some("public"))])
    );

    // 誤入力: 名前違い（既知・未知）
    assert_eq!(
        judge(&new_idea_ch1(), "/hepl"),
        CommandVerdict::NameMismatch
    );
    assert!(is_unknown_command(KNOWN, "hepl"));
    for (raw, name) in [
        ("/advance-phase", "advance-phase"),
        ("/create-pr", "create-pr"),
        ("/update-claude", "update-claude"),
    ] {
        assert_eq!(judge(&new_idea_ch2(), raw), CommandVerdict::NameMismatch);
        assert!(!is_unknown_command(KNOWN, name));
    }
    // e03・e04・e13
    assert_eq!(
        judge(&iit_ch7(), "/implement-issue-tree 42"),
        CommandVerdict::MissingArg
    );
    assert_eq!(
        judge(&new_idea_ch2(), "/new-idea BBS_With_Login"),
        CommandVerdict::BadArgShape
    );
    assert_eq!(
        judge(&iit_ch7(), "/implement-issue-tree abc --external-checks []"),
        CommandVerdict::BadArgShape
    );
    // e20
    assert_eq!(
        lex_command("/", "こんにちは"),
        Err(CommandLexError::NotCommand)
    );
}

#[test]
fn named_key_normalization() {
    let s = iit_ch7();
    for flag in ["--external-checks", "--external_checks"] {
        let raw = format!("/implement-issue-tree 1 {flag} x");
        assert_eq!(
            judge(&s, &raw),
            matched(&[("parent", Some("1")), ("external_checks", Some("x"))]),
            "{raw}"
        );
    }
    let hyphen = spec(
        "c",
        vec![arg("external-checks", false, true, ArgShape::Any)],
    );
    assert_eq!(
        judge(&hyphen, "/c --external_checks x"),
        matched(&[("external-checks", Some("x"))])
    );
    // 同名は後勝ち
    assert_eq!(
        judge(&hyphen, "/c --external-checks a --external_checks b"),
        matched(&[("external-checks", Some("b"))])
    );
}

#[test]
fn positional_resolution() {
    let s = spec(
        "c",
        vec![
            arg("a", true, true, ArgShape::Any),
            arg("opt", false, false, ArgShape::Any),
            arg("b", true, true, ArgShape::Digits),
        ],
    );
    assert_eq!(
        judge(&s, "/c x --opt y 7"),
        matched(&[("a", Some("x")), ("opt", Some("y")), ("b", Some("7"))])
    );
    assert_eq!(judge(&s, "/c x"), CommandVerdict::MissingArg);
    assert_eq!(judge(&s, "/c x z"), CommandVerdict::BadArgShape);
}

#[test]
fn named_without_value() {
    let required = spec("c", vec![arg("x", false, true, ArgShape::Any)]);
    assert_eq!(judge(&required, "/c --x"), CommandVerdict::BadArgShape);
    let optional = spec("c", vec![arg("x", false, false, ArgShape::Digits)]);
    assert_eq!(judge(&optional, "/c --x"), matched(&[("x", None)]));
}

#[test]
fn shape_boundaries() {
    let kebab = spec("c", vec![arg("v", true, true, ArgShape::KebabCase)]);
    for ok in ["a-b", "a1", "1", "a-b-c"] {
        assert!(
            matches!(
                judge(&kebab, &format!("/c {ok}")),
                CommandVerdict::Matched(_)
            ),
            "{ok}"
        );
    }
    for bad in ["-a", "a-", "a--b", "A", "a_b", "ａ", "あ"] {
        assert_eq!(
            judge(&kebab, &format!("/c {bad}")),
            CommandVerdict::BadArgShape,
            "{bad}"
        );
    }
    let digits = spec("c", vec![arg("v", true, true, ArgShape::Digits)]);
    assert!(matches!(
        judge(&digits, "/c 42"),
        CommandVerdict::Matched(_)
    ));
    for bad in ["4a", "４２", "-1", "4.2"] {
        assert_eq!(
            judge(&digits, &format!("/c {bad}")),
            CommandVerdict::BadArgShape,
            "{bad}"
        );
    }
    let any = spec("c", vec![arg("v", true, true, ArgShape::Any)]);
    assert!(matches!(
        judge(&any, "/c Zあ_!"),
        CommandVerdict::Matched(_)
    ));
}

#[test]
fn extra_args_are_ignored() {
    assert_eq!(
        judge(&new_idea_ch2(), "/new-idea a b c --zzz 1"),
        matched(&[("name", Some("a"))])
    );
}

#[test]
fn order_of_violations() {
    let s = spec(
        "c",
        vec![
            arg("a", true, true, ArgShape::Digits),
            arg("b", true, true, ArgShape::Any),
        ],
    );
    // 先の引数が形違い、後の引数が欠け -> 先の違反
    assert_eq!(judge(&s, "/c x"), CommandVerdict::BadArgShape);
    // 先が欠ければ後の形は見ない
    let s2 = spec(
        "c",
        vec![
            arg("a", false, true, ArgShape::Any),
            arg("b", true, true, ArgShape::Digits),
        ],
    );
    assert_eq!(judge(&s2, "/c x"), CommandVerdict::MissingArg);
}

#[test]
fn unchecked_spec_is_total() {
    let empty = spec("", vec![arg("", false, false, ArgShape::Any)]);
    let lexed = LexedCommand {
        name: String::new(),
        args: vec![],
    };
    assert!(matches!(
        judge_command(&empty, &lexed),
        CommandVerdict::Matched(_)
    ));
    // 重複と、正規化の後の衝突は、宣言の順に同じ値へ当たる
    let dup = spec(
        "c",
        vec![
            arg("foo-bar", false, true, ArgShape::Digits),
            arg("foo_bar", false, true, ArgShape::KebabCase),
        ],
    );
    // 同じ値に両方の宣言が当たる。先の宣言（digits）が先に検査される
    assert_eq!(judge(&dup, "/c --foo_bar ab"), CommandVerdict::BadArgShape);
    assert_eq!(
        judge(&dup, "/c --foo-bar 12"),
        matched(&[("foo-bar", Some("12")), ("foo_bar", Some("12"))])
    );
}

#[test]
fn category_mapping() {
    assert_eq!(
        CommandVerdict::MissingArg.category(),
        Some(RejectCategory::MissingArg)
    );
    assert_eq!(
        CommandVerdict::BadArgShape.category(),
        Some(RejectCategory::BadArgShape)
    );
    assert_eq!(CommandVerdict::NameMismatch.category(), None);
    assert_eq!(matched(&[]).category(), None);
}

fn shape_strategy() -> impl Strategy<Value = ArgShape> {
    prop_oneof![
        Just(ArgShape::KebabCase),
        Just(ArgShape::Digits),
        Just(ArgShape::Any)
    ]
}

fn spec_strategy() -> impl Strategy<Value = CommandSpec> {
    let names = prop::sample::select(vec!["a", "b", "a-b", "a_b", "c"]);
    let one = (names, any::<bool>(), any::<bool>(), shape_strategy())
        .prop_map(|(n, p, r, s)| arg(n, p, r, s));
    (
        prop::sample::select(vec!["cmd", "other"]),
        prop::collection::vec(one, 0..5),
    )
        .prop_map(|(n, a)| spec(n, a))
}

fn lexed_strategy() -> impl Strategy<Value = LexedCommand> {
    let value = prop::sample::select(vec!["42", "ab-c", "A", "x_y", "ａ", "--"]);
    let key = prop::sample::select(vec!["a", "b", "a-b", "a_b", "c", "zz"]);
    let one = prop_oneof![
        value
            .clone()
            .prop_map(|v| LexedArg::Positional(v.to_string())),
        (key, prop::option::of(value)).prop_map(|(k, v)| LexedArg::Named {
            key: k.to_string(),
            value: v.map(str::to_string),
        }),
    ];
    (
        prop::sample::select(vec!["cmd", "other"]),
        prop::collection::vec(one, 0..6),
    )
        .prop_map(|(n, a)| LexedCommand {
            name: n.to_string(),
            args: a,
        })
}

#[test]
fn judging_properties() {
    check(
        "command-judge",
        DEFAULT_CASES,
        (spec_strategy(), lexed_strategy()),
        |(s, l)| {
            let first = judge_command(&s, &l);
            prop_assert_eq!(&first, &judge_command(&s, &l));
            if s.name != l.name {
                prop_assert_eq!(first, CommandVerdict::NameMismatch);
                return Ok(());
            }
            prop_assert_ne!(&first, &CommandVerdict::NameMismatch);
            if let CommandVerdict::Matched(m) = first {
                prop_assert_eq!(m.args.len(), s.args.len());
                for (r, a) in m.args.iter().zip(&s.args) {
                    prop_assert_eq!(&r.name, &a.name);
                    if a.required {
                        prop_assert!(r.value.is_some());
                    }
                    if let Some(v) = &r.value {
                        let ok = match a.shape {
                            ArgShape::Any => !v.is_empty(),
                            ArgShape::Digits => {
                                !v.is_empty() && v.chars().all(|c| c.is_ascii_digit())
                            }
                            ArgShape::KebabCase => {
                                !v.is_empty()
                                    && !v.starts_with('-')
                                    && !v.ends_with('-')
                                    && !v.contains("--")
                                    && v.chars().all(|c| {
                                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
                                    })
                            }
                        };
                        prop_assert!(ok, "{v:?} vs {:?}", a.shape);
                    }
                }
            }
            Ok(())
        },
    );
}
