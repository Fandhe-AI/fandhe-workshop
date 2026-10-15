//! コマンド入力の字句化（TASK-262 1/2、JUDGE-1）の試験。

mod common;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::judgment::{LexError, LexedArg, LexedCommand};
use fandhe_workshop_contract::profile::{classify_input, InputClass};
use fandhe_workshop_core::command_lex::{lex_command, CommandLexError};
use proptest::prelude::*;

fn pos(s: &str) -> LexedArg {
    LexedArg::Positional(s.to_string())
}

fn named(k: &str, v: Option<&str>) -> LexedArg {
    LexedArg::Named {
        key: k.to_string(),
        value: v.map(str::to_string),
    }
}

fn ok(prefix: &str, raw: &str) -> LexedCommand {
    lex_command(prefix, raw).unwrap_or_else(|e| panic!("{raw:?}: {e}"))
}

fn cmd(n: &str, a: Vec<LexedArg>) -> LexedCommand {
    LexedCommand {
        name: n.to_string(),
        args: a,
    }
}

#[test]
fn golden_is_reproduced() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../contract/tests/data/judgment_1/lexed.json"
    );
    let golden: Vec<LexedCommand> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let inputs = [
        "/ls",
        "/cat a.txt b.txt",
        "/build --out dist --verbose --out other x",
    ];
    assert_eq!(golden.len(), inputs.len());
    for (g, raw) in golden.iter().zip(inputs) {
        assert_eq!(&ok("/", raw), g, "{raw}");
    }
}

#[test]
fn normal_cases() {
    assert_eq!(ok("/", "  /ls  "), cmd("ls", vec![]));
    assert_eq!(
        ok("/", "/a\u{3000}b\tc\nd"),
        cmd("a", vec![pos("b"), pos("c"), pos("d")])
    );
    assert_eq!(ok("/", "/ビルド 出力"), cmd("ビルド", vec![pos("出力")]));
    assert_eq!(
        ok("/", "/a --x --y 1"),
        cmd("a", vec![named("x", None), named("y", Some("1"))])
    );
    assert_eq!(
        ok("/", "/a --n -5 -x"),
        cmd("a", vec![named("n", Some("-5")), pos("-x")])
    );
    assert_eq!(ok("/", "/a --k=v"), cmd("a", vec![named("k=v", None)]));
    assert_eq!(
        ok("/", "/a --out-dir d --out-dir e"),
        cmd(
            "a",
            vec![named("out-dir", Some("d")), named("out-dir", Some("e"))]
        )
    );
    assert_eq!(ok("/", "//foo"), cmd("/foo", vec![]));
    assert_eq!(ok("", "ls -l"), cmd("ls", vec![pos("-l")]));
    assert_eq!(ok("::", "::go x"), cmd("go", vec![pos("x")]));
}

#[test]
fn failure_cases() {
    use CommandLexError::{Malformed, NotCommand};
    assert_eq!(lex_command("/", "ls"), Err(NotCommand));
    assert_eq!(lex_command("/", ""), Err(NotCommand));
    assert_eq!(lex_command("/", "   "), Err(NotCommand));
    assert_eq!(lex_command("/", "/"), Err(Malformed(LexError::EmptyName)));
    assert_eq!(
        lex_command("/", "/ foo"),
        Err(Malformed(LexError::EmptyName))
    );
    assert_eq!(lex_command("", "   "), Err(Malformed(LexError::EmptyName)));
    assert_eq!(
        lex_command("/", "/a --"),
        Err(Malformed(LexError::EmptyKey))
    );
    assert_eq!(
        lex_command("/", "/a ---x"),
        Err(Malformed(LexError::KeyStartsWithDash))
    );
}

#[test]
fn not_command_iff_text() {
    for prefix in ["/", "", "::"] {
        for raw in ["", " ", "ls", "/ls", " /ls", "::x", ":: x", "/ x"] {
            let text = classify_input(prefix, raw) == InputClass::Text;
            let not_cmd = lex_command(prefix, raw) == Err(CommandLexError::NotCommand);
            assert_eq!(text, not_cmd, "{prefix:?} {raw:?}");
        }
    }
}

#[test]
fn error_display_does_not_echo_input() {
    for raw in ["secret", "/ secret", "/secret --", "/secret ---secret"] {
        if let Err(e) = lex_command("/", raw) {
            assert!(!e.to_string().contains("secret"), "{raw}");
        }
    }
}

fn input_strategy() -> impl Strategy<Value = (usize, String)> {
    let atom = prop::sample::select(vec![
        " ", "\u{3000}", "\t", "\n", "-", "--", "/", "=", "a", "b", "x", "あ", "::", "!",
    ]);
    (0usize..4, prop::collection::vec(atom, 0..14)).prop_map(|(p, v)| (p, v.concat()))
}

#[test]
fn lexing_properties() {
    let prefixes = ["/", "", "::", "!"];
    check(
        "command-lex",
        DEFAULT_CASES,
        input_strategy(),
        |(p, raw)| {
            let prefix = prefixes[p];
            let first = lex_command(prefix, &raw);
            prop_assert_eq!(&first, &lex_command(prefix, &raw));
            if let Ok(c) = first {
                prop_assert_eq!(c.validate(), Ok(()));
                let json = serde_json::to_string(&c).unwrap();
                prop_assert_eq!(&serde_json::from_str::<LexedCommand>(&json).unwrap(), &c);
                let tokens = raw.split_whitespace().count();
                let consumed: usize = 1 + c
                    .args
                    .iter()
                    .map(|a| match a {
                        LexedArg::Positional(_) => 1,
                        LexedArg::Named { value, .. } => 1 + usize::from(value.is_some()),
                    })
                    .sum::<usize>();
                prop_assert_eq!(tokens, consumed);
            }
            Ok(())
        },
    );
}
