//! 受講者の OS の推定の試験（TASK-285 1/2、JUDGE-28・D-48）。
//!
//! User-Agent の代表例から `Windows`・`Mac`・`Unknown` が決まること、どんな入力でも panic しないことを確かめる。
//! 呼び出し元は CI の `make test` のみである。

use fandhe_workshop_core::learner_os::{estimate, LearnerOs};

const WIN_CHROME: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const WIN_EDGE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
const MAC_CHROME: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const MAC_SAFARI: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";
const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";
const LINUX: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const ANDROID: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36";
const CROS: &str = "Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

#[test]
fn windows_の_ua_は_windows() {
    assert_eq!(estimate(WIN_CHROME), LearnerOs::Windows);
    assert_eq!(estimate(WIN_EDGE), LearnerOs::Windows);
}

#[test]
fn macos_と_ios_系の_ua_は_mac() {
    assert_eq!(estimate(MAC_CHROME), LearnerOs::Mac);
    assert_eq!(estimate(MAC_SAFARI), LearnerOs::Mac);
    assert_eq!(estimate(IPHONE), LearnerOs::Mac);
}

#[test]
fn 判別できない_ua_は_unknown() {
    for ua in [LINUX, ANDROID, CROS, "", "curl/8.0", "あいう"] {
        assert_eq!(estimate(ua), LearnerOs::Unknown, "{ua}");
    }
}

#[test]
fn 既定は_unknown() {
    assert_eq!(LearnerOs::default(), LearnerOs::Unknown);
}

#[test]
fn windows_を_mac_より先に判定する() {
    assert_eq!(estimate("Windows Macintosh"), LearnerOs::Windows);
}

#[test]
fn 長い文字列と制御文字と非_ascii_でも落ちない() {
    let long = "a".repeat(64 * 1024);
    assert_eq!(estimate(&long), LearnerOs::Unknown);
    let long_win = format!("{long}Windows{long}");
    assert_eq!(estimate(&long_win), LearnerOs::Windows);
    assert_eq!(
        estimate("\u{0}\u{7}\r\n\t\u{1F600}Windows"),
        LearnerOs::Windows
    );
    assert_eq!(estimate("\u{0}\u{FFFD}日本語"), LearnerOs::Unknown);
}
