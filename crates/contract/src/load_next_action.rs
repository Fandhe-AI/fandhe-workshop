//! 読み込めない・開けない履歴に示す「次の行動」の対応づけ（TASK-334。SAVE-22・SHELL-36・D-57）。
//!
//! 履歴を開かないとき、受講者には理由ごとに次の行動を 1 つ示す（D-57）。どの理由にどの行動を出すかを、
//! この 1 か所の純粋な表（[`next_action`]）だけに定義する。呼び出し元は、通知の文を作る web の `shell_notice`（TASK-334）と、
//! 後で押せる操作として結線する入力・表示の責務（TASK-538）の 2 つである。2 つ以上の責務が参照するため contract に置く
//! （`docs/architecture.md` の判定規則 1）。
//!
//! 【案】割り当ては実装タスクで出した案であり、ユーザーの決定ではない（D-57 は「実装のタスクで案を出し、ユーザーが決める」）。
//! 決定が出たら、この関数の腕だけを直す。理由の列挙（[`LoadReason`]）は変えない。
//!
//! 【案】`StartFresh` は、どの理由にも割り当てていない。読み込みの経路では今の受講が続けられるため、今の進みを捨てる
//! 操作へ誤って誘わないようにした。起動時の壊れた履歴の経路（SHELL-17）で使う余地として、値だけを持つ。
//!
//! 定義しないもの:
//! - 受講者向けの文面（web が持つ。文面の確定は TASK-549・TASK-534）
//! - 押せる操作・属性・testid（TASK-538。shell の契約の範囲）
//! - 表示だけで開いた履歴への提示（VER-8）
//!
//! 閉じた列挙と固定の `&'static str` だけで、読み込んだ内容を含めない（SAFE-40）。

use crate::load_reason::LoadReason;

/// 次の行動の候補（D-57 の 3 つ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadNextAction {
    /// 別のファイルを選ぶ。
    ChooseAnotherFile,
    /// 書き出し元の版の教材で開く。
    OpenWithSourceVersion,
    /// 新しく始める。
    StartFresh,
}

impl LoadNextAction {
    /// すべての行動。
    pub const ALL: [LoadNextAction; 3] = [
        LoadNextAction::ChooseAnotherFile,
        LoadNextAction::OpenWithSourceVersion,
        LoadNextAction::StartFresh,
    ];

    /// 行動の識別子（ASCII の snake_case）。
    pub const fn as_str(self) -> &'static str {
        match self {
            LoadNextAction::ChooseAnotherFile => "choose_another_file",
            LoadNextAction::OpenWithSourceVersion => "open_with_source_version",
            LoadNextAction::StartFresh => "start_fresh",
        }
    }

    /// 正規の文字列だけを受ける。大文字・空白つき・未知の文字列は `None`。
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == s)
    }
}

/// 拒否の理由ごとの次の行動（案）。網羅 match で既定の腕を持たず、理由が足されたらコンパイルで止まる。
pub const fn next_action(reason: LoadReason) -> LoadNextAction {
    match reason {
        // ファイルそのものが壊れている・大きすぎる・別の教材のもの。版を合わせても開けない。
        LoadReason::TooLarge
        | LoadReason::TooDeep
        | LoadReason::TooManyOps
        | LoadReason::Truncated
        | LoadReason::NotJson
        | LoadReason::MissingVersion
        | LoadReason::Malformed
        | LoadReason::OtherPack
        | LoadReason::BadVersion
        | LoadReason::ReplayMismatch => LoadNextAction::ChooseAnotherFile,
        // 版の違いが原因。書き出したときの版の教材なら開ける。
        // 【推論】`start_missing` は教材の変更で起点が消えた場合として、こちらに入れる。
        LoadReason::HistoryFormatNewer
        | LoadReason::FormatNewer
        | LoadReason::VocabNewer
        | LoadReason::EngineNewer
        | LoadReason::FormatOlder
        | LoadReason::EngineMajor
        | LoadReason::ContentMajor
        | LoadReason::StartMissing
        | LoadReason::ReplayDiverged => LoadNextAction::OpenWithSourceVersion,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_reason::HISTORY_LOAD_ORDER;
    use LoadNextAction::{ChooseAnotherFile as C, OpenWithSourceVersion as O};

    /// 期待値は実装から引かず、独立の表で書く。
    const EXPECTED: [(&str, LoadNextAction); 19] = [
        ("too_large", C),
        ("too_deep", C),
        ("too_many_ops", C),
        ("truncated", C),
        ("not_json", C),
        ("missing_version", C),
        ("malformed", C),
        ("history_format_newer", O),
        ("other_pack", C),
        ("bad_version", C),
        ("format_newer", O),
        ("vocab_newer", O),
        ("engine_newer", O),
        ("format_older", O),
        ("engine_major", O),
        ("content_major", O),
        ("start_missing", O),
        ("replay_mismatch", C),
        ("replay_diverged", O),
    ];

    #[test]
    fn every_reason_matches_the_table() {
        assert_eq!(LoadReason::ALL.len(), EXPECTED.len());
        for (code, want) in EXPECTED {
            let r = LoadReason::parse(code).expect(code);
            assert_eq!(next_action(r), want, "{code}");
        }
    }

    #[test]
    fn load_order_is_covered() {
        for r in HISTORY_LOAD_ORDER {
            assert!(EXPECTED.iter().any(|(c, _)| *c == r.as_str()), "{r}");
        }
    }

    #[test]
    fn version_judgment_stage_follows_the_policy() {
        use crate::load_reason::HistoryLoadStage as S;
        for r in LoadReason::ALL {
            let want = match r.stage() {
                S::Size | S::Depth | S::Json | S::VersionField => C,
                S::VersionJudgment if r == LoadReason::BadVersion => C,
                S::VersionJudgment => O,
                _ => continue,
            };
            assert_eq!(next_action(r), want, "{r}");
        }
        for r in [
            LoadReason::ReplayDiverged,
            LoadReason::HistoryFormatNewer,
            LoadReason::StartMissing,
        ] {
            assert_eq!(next_action(r), O, "{r}");
        }
    }

    #[test]
    fn as_str_and_parse_round_trip() {
        for a in LoadNextAction::ALL {
            assert_eq!(LoadNextAction::parse(a.as_str()), Some(a));
        }
        assert_eq!(LoadNextAction::parse("Start_Fresh"), None);
        assert_eq!(LoadNextAction::parse(" start_fresh"), None);
        assert_eq!(LoadNextAction::parse("unknown"), None);
    }

    #[test]
    fn start_fresh_is_unassigned_in_this_proposal() {
        for r in LoadReason::ALL {
            assert_ne!(next_action(r), LoadNextAction::StartFresh, "{r}");
        }
    }
}
