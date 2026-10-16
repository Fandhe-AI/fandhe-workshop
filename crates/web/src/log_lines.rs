//! 右のログの行のモデルと描画（TASK-434 1/2）。行の構造は `terminal_area`（TASK-431、版 1）に準拠する。
//!
//! 呼び出し元は `engine_parts` の `LogPart`（TASK-343 の `DisplayPart`）で、イベント 1 件を
//! [`line_from_event`] で行に写し、[`line_node`] で `li` に描く。一覧にする [`log_list_node`] は
//! `list_policy::keyed_view` を通す薄い包みで、適合試験と後続のタスクが使う。
//! 本文・理由・段階の名前・対象などの文字列は受講者・教材に由来する信頼できない値で、
//! `learner_text::learner_part` の文字ノードだけで描く。属性値には enum の固定の語と 10 進の数字だけを入れる
//! （SAFE-11。HTML 風の文字列が要素にならないことは `tests/log_lines_contract.rs` が木の上で確かめる）。
//!
//! 結線の範囲: `InputReceived`（`received`）・`Progress`（`progress`）・`Stopped`（`stopped`）と、
//! 拒否の `Judged`（`rejected`。TASK-437 1/2）。拒否の理由は、IR の文面の ID の解決と実行時の値の差し込みを
//! 済ませた文面（`host::HostModel::judged_text`）を呼び出し側が渡す。ここでは文面を作らず、渡されなければ
//! 行にしない（fail-closed）。
//! 拡張点: 問い返しの行は TASK-437 2/2（#328）、応答の行は TASK-435・TASK-438 が `line_from_event` に足す。
//! 型 [`LogLine`] は 6 種すべてを表せる。
//!
//! 親の識別子 `log-list` は引き継がない。`hello.rs` の送信済みの一覧が使っており、ブラウザ試験が数えているため、
//! `LogPart` の親は仮の `part-log-list` のままにする。引き継ぎ（hello の一覧の退役と `part-log*` の仮の識別子の
//! 撤去）は TASK-438（送信の結線）または TASK-517（配置）で行う。適合は、`log-list` の親に載せ替えた木で
//! `terminal_area::check_area` が確かめる。
//!
//! 世代は 10 進の数字で出す。`terminal_area` は 18 桁までを受け付ける。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::judgment::{Judgment, NextAction, RejectCategory};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input};

use crate::learner_text::learner_part;
use crate::list_policy::{keyed_view, ListError};
use crate::parts::PartField;
use crate::terminal_area::{
    decision_value, decision_word, InputMethod, LineKind, CATEGORY_ATTR, DECISION_ATTR, GEN_ATTR,
    INDEX_ATTR, INPUT_ATTR, KIND_ATTR, LINE_BODY_TESTID, LINE_DECISION_TESTID,
    LINE_NEXT_ACTION_TESTID, LINE_QUESTION_TESTID, LINE_REASON_TESTID, LINE_STAGE_TESTID,
    LINE_TARGET_TESTID, TOTAL_ATTR,
};

/// 停止の行の固定の文言。行の文言は TASK-434 の持ち分で、教材の値は含めない。
pub const STOPPED_TEXT: &str = "停止しました";

/// ログの 1 行。種類ごとに、TASK-431 が定める部品だけを持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogLine {
    /// 承認以外の入力の記録。
    Received {
        /// 入力の種類（`Approval` 以外）。
        method: InputMethod,
        /// 入力の本文。
        body: String,
    },
    /// 承認の操作の記録（`received` と `data-input="approval"`）。
    Approval {
        /// 承認の 3 値。
        decision: Decision,
        /// 対象。
        target: Option<String>,
    },
    /// 応答。
    Response {
        /// 本文。
        body: String,
        /// 次の行動。
        next_action: Option<NextAction>,
    },
    /// 拒否の理由と次の行動。
    Rejected {
        /// 拒否の分類。
        category: RejectCategory,
        /// 理由。
        reason: String,
        /// 次の行動（空を作れない型）。
        next_action: NextAction,
    },
    /// 問い返し。
    Clarified {
        /// 問い。
        question: String,
        /// 次の行動（空を作れない型）。
        next_action: NextAction,
    },
    /// 処理中の段階。
    Progress {
        /// 進行の名前。
        stage: String,
        /// 何番目か（0 始まり）。
        index: u32,
        /// 総数。`index < total` でなければ行にしない。
        total: u32,
    },
    /// 停止。
    Stopped,
}

impl LogLine {
    /// 行の種類。承認の記録は `Received`。
    pub fn kind(&self) -> LineKind {
        match self {
            LogLine::Received { .. } | LogLine::Approval { .. } => LineKind::Received,
            LogLine::Response { .. } => LineKind::Response,
            LogLine::Rejected { .. } => LineKind::Rejected,
            LogLine::Clarified { .. } => LineKind::Clarified,
            LogLine::Progress { .. } => LineKind::Progress,
            LogLine::Stopped => LineKind::Stopped,
        }
    }
}

/// イベントを行に写す。行にしないイベント（と、契約に違反する進行）は `None`。
///
/// `text` は、そのイベントの拒否の理由として core が充填した文面（`HostModel::judged_text`）。
/// 拒否の `Judged` は `text` が空白だけでないときだけ行にする（理由の無い行を出さない）。他のイベントは読まない。
///
/// `Event` は `non_exhaustive` のため、足された種類は `None` に倒す（行を黙って捏造しない）。
pub fn line_from_event(event: &Event, text: Option<&str>) -> Option<LogLine> {
    match event {
        Event::Judged {
            judgment: Judgment::Rejected(r),
            ..
        } => {
            let reason = text.filter(|t| !t.trim().is_empty())?;
            Some(LogLine::Rejected {
                category: r.category,
                reason: reason.to_owned(),
                next_action: r.next_action.clone(),
            })
        }
        Event::InputReceived { input, .. } => {
            let method = InputMethod::from_input(input)?;
            Some(match input {
                Input::Approval { decision, target } => LogLine::Approval {
                    decision: *decision,
                    target: target.clone(),
                },
                Input::Command(s) | Input::Text(s) | Input::Choice(s) => LogLine::Received {
                    method,
                    body: s.clone(),
                },
                Input::Ui { view, path } => LogLine::Received {
                    method,
                    body: format!("{view} {path}"),
                },
                _ => return None,
            })
        }
        Event::Progress {
            name, index, total, ..
        } if index < total => Some(LogLine::Progress {
            stage: name.clone(),
            index: *index,
            total: *total,
        }),
        Event::Stopped { .. } => Some(LogLine::Stopped),
        _ => None,
    }
}

/// 行 1 件を `li` に描く。契約に違反する行（`index >= total` の進行）は `None`。
pub fn line_node(generation: u64, line: &LogLine) -> Option<Node> {
    let gen = generation.to_string();
    let kind = line.kind().as_str();
    let mut attrs: Vec<(&str, &str)> = vec![(KIND_ATTR, kind), (GEN_ATTR, gen.as_str())];
    let index;
    let total;
    let mut children: Vec<Node> = Vec::new();
    match line {
        LogLine::Received { method, body } => {
            attrs.push((INPUT_ATTR, method.as_str()));
            children.push(learner_part(LINE_BODY_TESTID, body));
        }
        LogLine::Approval { decision, target } => {
            attrs.push((INPUT_ATTR, InputMethod::Approval.as_str()));
            children.push(el(
                "span",
                vec![
                    ("data-testid", LINE_DECISION_TESTID),
                    (DECISION_ATTR, decision_value(*decision)),
                ],
                vec![text(decision_word(*decision))],
            ));
            if let Some(t) = target {
                children.push(learner_part(LINE_TARGET_TESTID, t));
            }
        }
        LogLine::Response { body, next_action } => {
            children.push(learner_part(LINE_BODY_TESTID, body));
            if let Some(n) = next_action {
                children.push(learner_part(LINE_NEXT_ACTION_TESTID, n.as_str()));
            }
        }
        LogLine::Rejected {
            category,
            reason,
            next_action,
        } => {
            attrs.push((CATEGORY_ATTR, category.as_str()));
            children.push(learner_part(LINE_REASON_TESTID, reason));
            children.push(learner_part(LINE_NEXT_ACTION_TESTID, next_action.as_str()));
        }
        LogLine::Clarified {
            question,
            next_action,
        } => {
            children.push(learner_part(LINE_QUESTION_TESTID, question));
            children.push(learner_part(LINE_NEXT_ACTION_TESTID, next_action.as_str()));
        }
        LogLine::Progress {
            stage,
            index: i,
            total: t,
        } => {
            if i >= t {
                return None;
            }
            index = i.to_string();
            total = t.to_string();
            attrs.push((INDEX_ATTR, index.as_str()));
            attrs.push((TOTAL_ATTR, total.as_str()));
            children.push(learner_part(LINE_STAGE_TESTID, stage));
        }
        LogLine::Stopped => children.push(learner_part(LINE_BODY_TESTID, STOPPED_TEXT)),
    }
    Some(el("li", attrs, children))
}

/// 行の `li` を keyed list の親に並べる。必ず `list_policy::keyed_view` を通す。
///
/// # Errors
///
/// 項目数の上限・キーの不正などは [`ListError`]。呼び出し側は握りつぶさず、固定の通知に倒す。
pub fn log_list_node(
    attrs: Vec<(&str, &str)>,
    field: &PartField,
    items: Vec<(String, Node)>,
) -> Result<Node, ListError> {
    keyed_view("ul", attrs, field, items)
}
