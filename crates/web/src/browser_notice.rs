//! Chrome 以外のブラウザと旧版の Chrome への「一度だけ」の案内の判定（SHELL-30・D-74。TASK-544）。
//!
//! 呼び出し元は起動の配線（`persist::mount_with_history`。wasm のみ）で、`navigator.userAgent` の文字列を [`needs_notice`] に渡し、
//! 結果を `host::EngineHost::set_browser_notice` へ渡す。描画は `shell_notice`、文面と操作は `shell`（契約の版 2）が持つ。
//! 締め出さずに開く（案 A。案 C は採らない）。案内は閉じられる非破壊の表示なので、判定できないときは出す側に倒す。
//!
//! # TASK-544 で決めたこと
//!
//! - 旧版の基準: Chrome の主版が [`MIN_CHROME_MAJOR`]（152）未満。試験で確かめた Chromium の主版（`docs/toolchain.md`。153）から
//!   1 を引いた値で、段階配布で最新の安定版の利用者に誤って出さないための余裕である。実時間の最新版は、外部への通信を
//!   CSP と SAFE が禁じるため知れない。版の固定を上げたら、`tests/browser_notice.rs` が落ちて見直しを促す（fail-closed）。
//! - 判定: 括弧の中を除いた製品トークンがすべて許可リストに入り、Chrome（`HeadlessChrome` を含む）の主版がすべて基準以上のときだけ出さない。
//!   Edge・Opera・Safari・Firefox・iOS の Chrome・Android の WebView などは、許可リスト外のトークンで出す側になる。
//! - 「一度だけ」: ページの中では起動時に 1 件で、再描画・操作で増えず、閉じると 0 件。ページをまたいでは、閉じた記録
//!   （[`BROWSER_NOTICE_KEY`] = [`DISMISSED`]）が同じオリジンにあれば出さない。閉じずに再読み込みすれば再び出る。
//!
//! UA は受講者が変えられる入力として扱い、判定にだけ使う。保持・出力・記録しない（D-48）。全域の純関数で、どんな入力でも panic しない。
//! User-Agent Client Hints は扱わない（`learner_os` と同じ）。

/// 「確かめた Chrome」の主版の下限。確かめた版（153）から 1 引いた値。
pub const MIN_CHROME_MAJOR: u32 = 152;

/// 閉じた記録の localStorage の鍵。履歴の鍵・起動番号の鍵と接頭辞が同じ meta の空間で、履歴の一覧・削除の対象にならない。
pub const BROWSER_NOTICE_KEY: &str = "fandhe-workshop/meta/browser-notice";

/// 閉じた記録の値（固定）。
pub const DISMISSED: &str = "dismissed";

/// 許可する製品トークン。これ以外が 1 つでもあれば「確かめた Chrome」とは見なさない。
const ALLOWED: [&str; 6] = [
    "Mozilla",
    "AppleWebKit",
    "Chrome",
    "HeadlessChrome",
    "Safari",
    "Mobile",
];

/// 判定の結果の理由。画面には出さない（試験の読みやすさのため）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// 確かめた Chrome。案内しない。
    Verified,
    /// Chrome の主版が基準未満。
    OldChrome,
    /// Chrome と判定できない（他のブラウザ・解釈できない文字列を含む）。
    NotChrome,
}

/// 括弧の中（コメント部）を除いた文字列。入れ子は深さで数え、閉じ忘れは末尾まで括弧の中として扱う。
fn strip_comments(ua: &str) -> String {
    let mut depth = 0usize;
    let mut out = String::with_capacity(ua.len());
    for c in ua.chars() {
        match c {
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// UA を判定する。
pub fn verdict(user_agent: &str) -> Verdict {
    let stripped = strip_comments(user_agent);
    let mut chrome_seen = false;
    let mut old = false;
    for token in stripped.split_whitespace() {
        let (name, version) = match token.split_once('/') {
            Some((n, v)) => (n, Some(v)),
            None => (token, None),
        };
        if !ALLOWED.contains(&name) {
            return Verdict::NotChrome;
        }
        if name == "Chrome" || name == "HeadlessChrome" {
            chrome_seen = true;
            let major = version
                .and_then(|v| v.split('.').next())
                .and_then(|m| m.parse::<u32>().ok());
            match major {
                Some(m) if m >= MIN_CHROME_MAJOR => {}
                Some(_) => old = true,
                None => return Verdict::NotChrome,
            }
        }
    }
    if !chrome_seen {
        Verdict::NotChrome
    } else if old {
        Verdict::OldChrome
    } else {
        Verdict::Verified
    }
}

/// 案内を出す対象か。確かめた Chrome 以外は真。
pub fn needs_notice(user_agent: &str) -> bool {
    verdict(user_agent) != Verdict::Verified
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHROME: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36";

    #[test]
    fn verified_chrome_is_not_noticed() {
        assert_eq!(verdict(CHROME), Verdict::Verified);
        assert!(!needs_notice(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/153.0.8010.12 Safari/537.36"
        ));
    }

    #[test]
    fn old_chrome_and_others_are_noticed() {
        assert_eq!(verdict(&CHROME.replace("153", "151")), Verdict::OldChrome);
        assert_eq!(
            verdict(&format!("{CHROME} Edg/153.0.0.0")),
            Verdict::NotChrome
        );
        assert_eq!(verdict(""), Verdict::NotChrome);
        assert_eq!(verdict("Chrome/x"), Verdict::NotChrome);
    }
}
