//! 受講者の OS の推定（決定 D-48・JUDGE-28。TASK-285 1/2）。
//!
//! 案内文・ヒントに出す修飾キーの表示（Cmd／Ctrl）を決める材料である。web の入口が、ブラウザの
//! User-Agent の文字列を [`estimate`] へ渡して [`LearnerOs`] を得る。操作そのものは両 OS で同じため、
//! この値は表示だけに使い、実行環境（`Session`）・履歴・操作の記録には入れない（履歴の形式を変えない）。
//!
//! 推定の規則は試作（`estimate_os`）と同じ部分一致で、`Windows` を先に、次に `Macintosh`・`Mac OS`
//! （iPhone・iPad の UA も含む）を見る。どちらでもなければ [`LearnerOs::Unknown`] とし、呼び出し側は
//! 修飾キーを出さない（Ctrl に倒さない）。User-Agent Client Hints（`userAgentData`）は扱わない。
//! User-Agent は受講者の側で変えられる信頼できない入力のため、全域の純関数にし、文字列を保持も出力もしない。

/// 推定した受講者の OS。表示する修飾キーの選択にだけ使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LearnerOs {
    /// macOS・iPadOS・iOS（修飾キーは Cmd）。
    Mac,
    /// Windows（修飾キーは Ctrl）。
    Windows,
    /// 推定できない（Linux・Android・空文字など）。修飾キーを出さない。
    #[default]
    Unknown,
}

/// User-Agent の文字列から OS を推定する。どんな入力でも panic せず、判別できなければ `Unknown` を返す。
///
/// `Windows` を先に判定するのは、複数の語を含む UA を Windows 側に倒して Mac と取り違えないため。
#[must_use]
pub fn estimate(user_agent: &str) -> LearnerOs {
    if user_agent.contains("Windows") {
        LearnerOs::Windows
    } else if user_agent.contains("Macintosh") || user_agent.contains("Mac OS") {
        LearnerOs::Mac
    } else {
        LearnerOs::Unknown
    }
}
