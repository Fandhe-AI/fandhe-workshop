//! 読み上げ用のライブの欄（`live-response`・`live-left`）の描画と、`live-response` の文の導出（A11Y-6。TASK-370 1/2）。
//!
//! 呼び出し元は `host::EngineHost` だけである。`view()` が [`live_nodes`] を root の直下に置き、`step_engine` が
//! 1 回の応答で伸びたログの行から [`response_summary`] を求めて [`LIVE_RESPONSE_FIELD`] を dirty に積む。
//! 「どの行が応答か」の判定は契約 `a11y::counts_for_live_response` に任せ、ここで種類を数え直さない。
//! 属性は `a11y::LIVE_REGION_ATTRS` を `view()` の直接の属性として付ける（束縛で aria を書かない。F-2）。
//! 欄は文字ノード 1 つだけを持ち、生の HTML の経路は使わない（SAFE-11）。
//! `live-left` は 2/2（#972）が文を導く。ここでは属性つきの要素を置き、文は空のままにする。

use fandhe_frontend_core::{bind_text, Node};
use fandhe_workshop_contract::judgment::NextAction;

use crate::a11y::{counts_for_live_response, LiveRegion, LIVE_REGION_ATTRS};
use crate::log_lines::{
    checkpoint_limit_heading, limit_body, LogLine, CHECKPOINT_LIMIT_BODY, LIMIT_HEADING,
    RELOADED_TEXT, STOPPED_TEXT,
};
use crate::parts::{FieldKind, PartField};

/// `live-response` の束縛点。値は「応答 N: 要約」で、応答ごとに N が 1 増える。
pub const LIVE_RESPONSE_FIELD: PartField = PartField {
    name: "live_response",
    kind: FieldKind::Bound,
};

/// `live-left` の束縛点。値は常に空で、要約は #972（A11Y-8）が足す。
pub const LIVE_LEFT_FIELD: PartField = PartField {
    name: "live_left",
    kind: FieldKind::Bound,
};

/// 欄の文の接頭辞。文言は A11Y-15 が未決のため仮置きである（変えるときはこの定数だけを直す）。
/// 通し番号を付けるのは、同じ拒否が続いても値が変わり、束縛点が確実に 1 回書くようにするため。
pub const RESPONSE_PREFIX: &str = "応答 ";

/// 視覚的にだけ隠す CSS のクラス名（`static/app.css`）。`hidden` は AX ツリーから消えるため使わない。
pub const VISUALLY_HIDDEN_CLASS: &str = "a11y-live";

/// 複数の行の要約を区切る語。
const LINE_SEPARATOR: &str = "／";

/// 欄の文。`seq` は 1 始まりの応答の通し番号。
pub fn response_text(seq: u64, summary: &str) -> String {
    format!("{RESPONSE_PREFIX}{seq}: {summary}")
}

fn join_parts(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|p| !p.trim().is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 1 行の要約。応答に数えない行（`received`・`progress`）は `None`。
pub fn line_summary(line: &LogLine) -> Option<String> {
    if !counts_for_live_response(line.kind().as_str()) {
        return None;
    }
    Some(match line {
        LogLine::Response { body, next_action } => {
            join_parts(&[body, next_action.as_ref().map_or("", NextAction::as_str)])
        }
        LogLine::Rejected {
            reason,
            next_action,
            ..
        } => join_parts(&[reason, next_action.as_str()]),
        LogLine::Clarified {
            question,
            next_action,
        } => join_parts(&[question, next_action.as_str()]),
        LogLine::Stopped => STOPPED_TEXT.to_owned(),
        LogLine::Reloaded => RELOADED_TEXT.to_owned(),
        LogLine::Limit { landings } => join_parts(&[LIMIT_HEADING, &limit_body(*landings)]),
        LogLine::CheckpointLimit => {
            join_parts(&[&checkpoint_limit_heading(), CHECKPOINT_LIMIT_BODY])
        }
        // 受領・承認・進捗は上で除外済み。到達しない分岐も数えない側に倒す。
        LogLine::Received { .. } | LogLine::Approval { .. } | LogLine::Progress { .. } => {
            return None
        }
    })
}

/// 1 回の応答で伸びたログの行から、欄の要約を求める。数える行が 0 件なら `None`（欄を触らない）。
pub fn response_summary(lines: &[LogLine]) -> Option<String> {
    let parts: Vec<String> = lines.iter().filter_map(line_summary).collect();
    (!parts.is_empty()).then(|| parts.join(LINE_SEPARATOR))
}

/// 2 つのライブの欄（`live-response`・`live-left`）。順に応答・残りの欄で、root の直下に置く。
pub fn live_nodes(response_text: &str) -> [Node; 2] {
    let node = |region: LiveRegion, field: &PartField, value: &str| {
        let mut attrs: Vec<(&str, &str)> = vec![
            ("data-testid", region.testid()),
            ("class", VISUALLY_HIDDEN_CLASS),
        ];
        attrs.extend(LIVE_REGION_ATTRS);
        bind_text("p", attrs, field.name, value)
    };
    [
        node(LiveRegion::Response, &LIVE_RESPONSE_FIELD, response_text),
        node(LiveRegion::Left, &LIVE_LEFT_FIELD, ""),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use fandhe_workshop_contract::judgment::RejectCategory;

    fn next(s: &str) -> NextAction {
        NextAction::new(s).unwrap()
    }

    #[test]
    fn progress_is_not_a_response() {
        let p = LogLine::Progress {
            stage: "s".into(),
            index: 0,
            total: 2,
        };
        assert_eq!(line_summary(&p), None);
        assert_eq!(response_summary(&[p]), None);
    }

    #[test]
    fn rejected_and_clarified_carry_text() {
        let j = LogLine::Rejected {
            category: RejectCategory::UnknownCommand,
            reason: "理由".into(),
            next_action: next("次"),
        };
        assert_eq!(line_summary(&j).as_deref(), Some("理由 次"));
        let c = LogLine::Clarified {
            question: "問い".into(),
            next_action: next("答える"),
        };
        assert_eq!(line_summary(&c).as_deref(), Some("問い 答える"));
        assert_eq!(
            line_summary(&LogLine::Stopped).as_deref(),
            Some(STOPPED_TEXT)
        );
    }

    #[test]
    fn several_lines_join_into_one_summary() {
        let lines = [
            LogLine::Stopped,
            LogLine::Reloaded,
            LogLine::Progress {
                stage: "s".into(),
                index: 0,
                total: 1,
            },
        ];
        assert_eq!(
            response_summary(&lines).as_deref(),
            Some("停止しました／再読み込みしました")
        );
    }

    #[test]
    fn response_text_has_sequence() {
        assert_eq!(response_text(3, "a"), "応答 3: a");
    }
}
