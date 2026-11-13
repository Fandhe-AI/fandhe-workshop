//! 読み上げ用のライブの欄（`live-response`・`live-left`）の描画と、`live-response` の文の導出（A11Y-6。TASK-370 1/2）。
//!
//! 呼び出し元は `host::EngineHost` だけである。`view()` が [`live_nodes`] を root の直下に置き、`step_engine` が
//! 1 回の応答で伸びたログの行から [`response_summary`] を求めて [`LIVE_RESPONSE_FIELD`] を dirty に積む。
//! 「どの行が応答か」の判定は契約 `a11y::counts_for_live_response` に任せ、ここで種類を数え直さない。
//! 属性は `a11y::LIVE_REGION_ATTRS` を `view()` の直接の属性として付ける（束縛で aria を書かない。F-2）。
//! 欄は文字ノード 1 つだけを持ち、生の HTML の経路は使わない（SAFE-11）。
//! `live-left` の文は [`left_summary`] が導く（A11Y-8。TASK-370 2/2）。ある Step の間に左の成果物が 1 件以上
//! 変わったとき「「<題>」: 左の画面が N 件変わりました」を知らせる。件数の文で、変化の中身は含めない。
//! 変化の有無は契約 `a11y::live_left_should_update` に任せる。

use fandhe_frontend_core::{bind_text, Node};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::NextAction;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::projection::ReviewItem;

use crate::a11y::{
    counts_for_live_response, live_left_should_update, LiveRegion, LIVE_REGION_ATTRS,
};
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

/// `live-left` の束縛点。値は左の変化の要約で、変化が無い間は空（A11Y-8）。
pub const LIVE_LEFT_FIELD: PartField = PartField {
    name: "live_left",
    kind: FieldKind::Bound,
};

/// 欄の文の接頭辞。文言は A11Y-15 が未決のため仮置きである（変えるときはこの定数だけを直す）。
/// 通し番号を付けるのは、同じ拒否が続いても値が変わり、束縛点が確実に 1 回書くようにするため。
pub const RESPONSE_PREFIX: &str = "応答 ";

/// `live-left` の文の中間と末尾。文言は A11Y-15 が未決のため仮置きである（変えるときはこの 2 つだけを直す）。
const LEFT_MIDDLE: &str = "」: 左の画面が ";
const LEFT_SUFFIX: &str = " 件変わりました";

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

/// `live-left` の文。`title` は教材由来の未信頼の文字列で、文字ノードとしてだけ出す（SAFE-11）。
/// 件数が 0 なら空（契約 `live_left_should_update`）。
pub fn left_text(title: &str, count: usize) -> String {
    if live_left_should_update(count) {
        format!("「{title}{LEFT_MIDDLE}{count}{LEFT_SUFFIX}")
    } else {
        String::new()
    }
}

/// 1 回の更新で伸びたイベント列 `events` から、`live-left` の文を求める（A11Y-8）。
///
/// 対象の Step は、原因が入力の変化のうち最後のものの Step とする。確認を省く Step では変化と次の Step への
/// 移動が同じ更新に入るため、変えた側の題で知らせる。入力の変化が無く基準点（入った・開始・移動・停止・復帰）が
/// あれば空にする（古い件数を言わない）。どちらでもなければ今の Step の件数に追随する。
/// 節点・項目が見つからないときは空に倒す。
pub fn left_summary(
    events: &[Event],
    review: &[ReviewItem],
    ir: &Ir,
    current: Option<&StepUid>,
) -> String {
    let changed_by_input = events.iter().rev().find_map(|e| match e {
        Event::Changed {
            at,
            cause: Cause::Input,
            ..
        } => Some(&at.step),
        _ => None,
    });
    let target = match changed_by_input {
        Some(step) => step,
        None => {
            let reset = events.iter().any(|e| {
                matches!(
                    e,
                    Event::Entered { .. }
                        | Event::Begun { .. }
                        | Event::Moved { .. }
                        | Event::Stopped { .. }
                        | Event::Restored { .. }
                )
            });
            match (reset, current) {
                (false, Some(step)) => step,
                _ => return String::new(),
            }
        }
    };
    let Some(node) = ir.nodes.iter().find(|n| n.uid == target.0) else {
        return String::new();
    };
    let count = review
        .iter()
        .find(|item| &item.step == target)
        .map_or(0, |item| item.changes.len());
    left_text(&node.step.title, count)
}

/// 2 つのライブの欄（`live-response`・`live-left`）。順に応答・左の変化で、root の直下に置く。
pub fn live_nodes(response_text: &str, left_text: &str) -> [Node; 2] {
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
        node(LiveRegion::Left, &LIVE_LEFT_FIELD, left_text),
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
    fn left_text_names_title_and_count() {
        assert_eq!(left_text("題", 2), "「題」: 左の画面が 2 件変わりました");
        assert_eq!(left_text("題", 0), "");
    }

    #[test]
    fn left_text_keeps_markup_as_characters() {
        let t = left_text("<b>x</b>", 1);
        assert!(t.contains("<b>x</b>"));
    }

    #[test]
    fn response_text_has_sequence() {
        assert_eq!(response_text(3, "a"), "応答 3: a");
    }
}
