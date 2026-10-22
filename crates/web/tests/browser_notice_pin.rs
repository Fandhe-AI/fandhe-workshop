//! 基準の主版（TASK-544。SHELL-30）が `docs/toolchain.md` の Chromium の主版と 1 つ違いであることの試験。
//! 変異注入の作業コピーには docs が無いため、判定の表の試験（`browser_notice`）とは別の対象にしてある。

use fandhe_workshop_web::browser_notice::MIN_CHROME_MAJOR;

/// 版の固定（`docs/toolchain.md`）を上げたら基準の見直しを促す（fail-closed）。
#[test]
fn threshold_follows_pinned_chromium() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/toolchain.md");
    let doc = std::fs::read_to_string(path).expect("docs/toolchain.md を読めない");
    let major = doc
        .split("Chromium ")
        .skip(1)
        .find_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if digits.is_empty() || !rest[digits.len()..].starts_with('.') {
                return None;
            }
            digits.parse::<u32>().ok()
        })
        .expect("docs/toolchain.md に `Chromium <主版>.` が無い");
    assert_eq!(major, MIN_CHROME_MAJOR + 1);
}
