//! 画面（web）と教材テスト（pack）が、同じ規則で入力を分けることの試験（TASK-52 1/2、R4）。

use fandhe_workshop_contract::profile::ProfileSetting;
use fandhe_workshop_contract::profile::{InputClass, InputClass::Command, InputClass::Text};
use fandhe_workshop_pack::profile::classify_play_input;
use fandhe_workshop_web::profile::CommandPrefix;

const TABLE: &[(&str, &str, InputClass)] = &[
    ("/", "/ls", Command),
    ("/", "ls", Text),
    ("/", "  /ls ", Command),
    ("/", "\u{3000}/ls", Command),
    ("$", "$hello", Command),
    ("$", "/hello", Text),
    ("::", "::x", Command),
    ("::", ":x", Text),
    ("!!!!", "!!!!a", Command),
    ("!!!!", "!!!a", Text),
    ("", "hello world", Command),
    ("", "$x", Command),
];

#[test]
fn screen_and_play_agree() {
    for (p, raw, want) in TABLE {
        let screen = CommandPrefix::new(p).unwrap().classify(raw);
        let setting = ProfileSetting {
            command_prefix: (*p).to_owned(),
            ..Default::default()
        };
        let play = classify_play_input(&setting, raw);
        assert_eq!(screen, *want, "screen {p:?} {raw:?}");
        assert_eq!(play, *want, "play {p:?} {raw:?}");
    }
}

#[test]
fn accepted_prefixes_agree() {
    for p in [
        "", "/", "$", "::", "abcd", "abcde", "-x", "a b", "<b>", "!!!!!",
    ] {
        let web_ok = CommandPrefix::new(p).is_ok();
        let contract_ok = fandhe_workshop_contract::profile::check_command_prefix(p).is_ok();
        assert_eq!(web_ok, contract_ok, "{p:?}");
    }
}
