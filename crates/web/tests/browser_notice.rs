//! Chrome 以外のブラウザへの案内の判定（TASK-544。SHELL-30・D-74）の試験。
//!
//! 実在の形の UA の表で出す・出さないを固定し。基準の主版の照合は `browser_notice_pin`。
//! 失敗の文には案件名だけを出す。

use fandhe_workshop_web::browser_notice::needs_notice;

const WIN: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36";
const MAC: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36";
const ANDROID: &str = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Mobile Safari/537.36";
const HEADLESS: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/153.0.8010.12 Safari/537.36";
const SAFARI_MAC: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";
const SAFARI_IOS: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1";
const CRIOS: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/153.0.0.0 Mobile/15E148 Safari/604.1";
const FXIOS: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/125.0 Mobile/15E148 Safari/605.1.15";
const FIREFOX: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0";

#[test]
fn verified_chrome_is_not_noticed() {
    let cases = [
        ("win", WIN.to_owned()),
        ("mac", MAC.to_owned()),
        ("android", ANDROID.to_owned()),
        ("headless", HEADLESS.to_owned()),
        ("boundary", WIN.replace("153", "152")),
    ];
    for (name, ua) in cases {
        assert!(!needs_notice(&ua), "{name}");
    }
}

#[test]
fn other_browsers_and_old_chrome_are_noticed() {
    let cases = [
        ("old151", WIN.replace("153", "151")),
        ("old120", WIN.replace("153", "120")),
        ("edge", format!("{WIN} Edg/153.0.0.0")),
        ("opera", format!("{WIN} OPR/110.0.0.0")),
        (
            "samsung",
            ANDROID.replace("Mobile Safari", "SamsungBrowser/25.0 Mobile Safari"),
        ),
        ("safari-mac", SAFARI_MAC.to_owned()),
        ("safari-ios", SAFARI_IOS.to_owned()),
        ("crios", CRIOS.to_owned()),
        ("fxios", FXIOS.to_owned()),
        ("firefox", FIREFOX.to_owned()),
        (
            "webview",
            ANDROID.replace("(KHTML, like Gecko)", "(KHTML, like Gecko) Version/4.0"),
        ),
        ("empty", String::new()),
        ("unrelated", "curl/8.0".to_owned()),
        ("non-numeric", WIN.replace("153.0.0.0", "x")),
        ("unclosed", "Mozilla/5.0 (X11; Chrome/160".to_owned()),
    ];
    for (name, ua) in cases {
        assert!(needs_notice(&ua), "{name}");
    }
}

#[test]
fn never_panics_on_odd_input() {
    let big = "(".repeat(65_536) + &")".repeat(70_000) + "Chrome/\u{0}\u{3042}";
    for ua in [
        big.as_str(),
        "\u{0}\u{7f}",
        "あ/い う/え",
        ")))(((",
        "/",
        "Chrome/",
        "Chrome/99999999999",
    ] {
        let _ = needs_notice(ua);
    }
}
