//! 右のターミナル風エリアが守る語の契約（TASK-431、版 1。ログの行・状態表示・Step の状態の語）。
//!
//! 右のエリアの実装（TASK-434〜464）と通しの試験（TASK-87・TASK-92・TASK-374）は、ログの行の種類・
//! 状態表示・Step の状態の語を観察点に使う。担当ごとに決めると統合で食い違うため、先に本モジュールで固定する。
//! 呼び出し元は画面の試験で、`view()` が返す [`Node`] の木を [`check_area`]（右のエリアの部分木）・
//! [`check_view`]（画面全体から右のエリアを探す）で照合する。native の rlib で試験できるよう、wasm 専用にしない。
//!
//! 出所は spec の `screen-terminal-area.md` の TERM-23・TERM-31〜33、`engine-accessibility.md` の A11Y-6、
//! 試作 PoC-12 と試作の `right.rs` である。
//! 値・識別子・規則を変える場合は、先に spec を直し、[`TERMINAL_AREA_CONTRACT_VERSION`] を上げる。黙って変えない。
//!
//! 版 1 は 2 回に分けて書き上げる。1/2（本モジュールの現状）はログの行・状態表示・Step の状態の語、
//! 2/2 は入力の口の識別子と観察点の制約である。2/2 は版を上げず、識別子の閉じた一覧に足す。
//!
//! 版 1（1/2）が持つ規則:
//! 1. ログの行の種類と構造（[`LineKind`]・[`InputMethod`]。`log-list` の直接の子は `li` だけ）
//! 2. 状態表示の値と語（[`RunStatus`]。識別子 `run-status`）
//! 3. Step の状態の語（[`step_status_word`]。識別子 `step-status`）
//! 4. 右のエリアの `data-testid` の閉じた一覧（定義にない識別子は違反）
//!
//! 識別子の持ち主の境界:
//! - `log-list`・`pane-tabs`・`pane-tab`・`mark-glyph`・`mark-text`・ライブの欄・通知欄は `a11y`（TASK-366）が持つ。
//!   本契約は再定義せず、`a11y` の定数から閉じた一覧を組み立てる
//! - `line-` で始まる識別子は本契約の予約空間とする。`log-list` の行の中にだけ置ける
//! - 行の種類の名前 `received`・`progress` は `a11y::LIVE_EXCLUDED_LINE_KINDS` が値を固定している。
//!   承認の操作の記録は利用者自身の入力の写しなので、別の種類にせず `received` と `data-input="approval"` で
//!   表す。別の種類にすると `a11y` の版上げが要る
//!
//! 意図して固定しないもの:
//! - 行の文言と `line-decision`・`line-target` の文字（ボタンの語は 2/2、行の文言は TASK-434・TASK-437）
//! - どのイベントをどの行にするかの対応（TASK-434〜437）と、状態表示の導出（停止から待機中へ戻す時機。TASK-436）
//! - `aria-*`・ライブの欄（TASK-366）
//! - Step の状態のうち `Entered`・`NotStarted`・`Skipped` と章の完了の語（spec が 2 語しか確定していない。
//!   TASK-435 が決めた後に版を上げて足す）。`step-status` の文字は木では検査しない
//! - 入力の口の識別子と、未決の目印（2/2・TASK-464）
//!
//! 違反の文面は英語の固定文で、位置・規則名・識別子・属性名だけを含む。文字ノードの中身
//! （受講者の入力・教材の文面）は埋め込まない（A09）。

use crate::a11y::{
    LiveRegion, Region, StatusNotice, LOG_LIST_TESTID, MARK_GLYPH_TESTID, MARK_TEXT_TESTID,
    TABLIST_TESTID, TAB_TESTID, TESTID_ATTR,
};
use core::fmt;
use fandhe_frontend_core::Node;
use fandhe_workshop_contract::judgment::RejectCategory;
use fandhe_workshop_core::input::{Decision, Input};
use fandhe_workshop_core::step_status::StepStatus;

/// この契約の版。値・識別子・規則を変えたら上げる（2/2 の追加は上げない）。
pub const TERMINAL_AREA_CONTRACT_VERSION: u32 = 1;

/// 行の種類を載せる属性名。
pub const KIND_ATTR: &str = "data-kind";
/// 行の世代を載せる属性名（10 進の数字だけ）。
pub const GEN_ATTR: &str = "data-gen";
/// 入力の記録の入力の種類を載せる属性名。
pub const INPUT_ATTR: &str = "data-input";
/// 拒否の分類を載せる属性名。
pub const CATEGORY_ATTR: &str = "data-category";
/// 処理中の段階の番号（0 始まり）を載せる属性名。
pub const INDEX_ATTR: &str = "data-index";
/// 処理中の段階の総数を載せる属性名。
pub const TOTAL_ATTR: &str = "data-total";
/// 承認の操作の値を載せる属性名。
pub const DECISION_ATTR: &str = "data-decision";
/// 状態表示の値を載せる属性名。
pub const STATE_ATTR: &str = "data-state";

/// 状態表示の識別子。
pub const RUN_STATUS_TESTID: &str = "run-status";
/// Step の状態の識別子。
pub const STEP_STATUS_TESTID: &str = "step-status";
/// 本文の部品。
pub const LINE_BODY_TESTID: &str = "line-body";
/// 拒否の理由の部品。
pub const LINE_REASON_TESTID: &str = "line-reason";
/// 次の行動の部品。
pub const LINE_NEXT_ACTION_TESTID: &str = "line-next-action";
/// 問い返しの質問の部品。
pub const LINE_QUESTION_TESTID: &str = "line-question";
/// 処理中の段階の部品。
pub const LINE_STAGE_TESTID: &str = "line-stage";
/// 承認の操作の部品。
pub const LINE_DECISION_TESTID: &str = "line-decision";
/// 承認の対象の部品。
pub const LINE_TARGET_TESTID: &str = "line-target";

/// 本契約が持つ識別子の閉じた一覧（1/2 分）。
pub const TERMINAL_AREA_TESTIDS: [&str; 9] = [
    RUN_STATUS_TESTID,
    STEP_STATUS_TESTID,
    LINE_BODY_TESTID,
    LINE_REASON_TESTID,
    LINE_NEXT_ACTION_TESTID,
    LINE_QUESTION_TESTID,
    LINE_STAGE_TESTID,
    LINE_DECISION_TESTID,
    LINE_TARGET_TESTID,
];

/// 本契約の予約空間の接頭辞。
pub const LINE_PREFIX: &str = "line-";

/// ログの行の種類。承認の操作の記録は [`LineKind::Received`] の `data-input="approval"` で表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// 入力の記録（承認の操作の記録を含む）。
    Received,
    /// 応答。
    Response,
    /// 拒否の理由と次の行動。
    Rejected,
    /// 問い返し。
    Clarified,
    /// 処理中の段階。
    Progress,
    /// 停止。
    Stopped,
}

impl LineKind {
    /// すべての種類。
    pub const ALL: [LineKind; 6] = [
        LineKind::Received,
        LineKind::Response,
        LineKind::Rejected,
        LineKind::Clarified,
        LineKind::Progress,
        LineKind::Stopped,
    ];

    /// `data-kind` の値。
    pub const fn as_str(self) -> &'static str {
        match self {
            LineKind::Received => "received",
            LineKind::Response => "response",
            LineKind::Rejected => "rejected",
            LineKind::Clarified => "clarified",
            LineKind::Progress => "progress",
            LineKind::Stopped => "stopped",
        }
    }

    /// `data-kind` の値から種類を引く。未知は `None`。
    pub fn parse(v: &str) -> Option<LineKind> {
        LineKind::ALL.into_iter().find(|k| k.as_str() == v)
    }
}

/// 入力の記録の入力の種類（`data-input` の値）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMethod {
    /// コマンド。
    Command,
    /// 自由な文章。
    Text,
    /// 選択肢。
    Choice,
    /// UI 部品の操作。
    Ui,
    /// 承認の操作。
    Approval,
}

impl InputMethod {
    /// すべての種類。
    pub const ALL: [InputMethod; 5] = [
        InputMethod::Command,
        InputMethod::Text,
        InputMethod::Choice,
        InputMethod::Ui,
        InputMethod::Approval,
    ];

    /// `data-input` の値。
    pub const fn as_str(self) -> &'static str {
        match self {
            InputMethod::Command => "command",
            InputMethod::Text => "text",
            InputMethod::Choice => "choice",
            InputMethod::Ui => "ui",
            InputMethod::Approval => "approval",
        }
    }

    /// 値から引く。未知は `None`。
    pub fn parse(v: &str) -> Option<InputMethod> {
        InputMethod::ALL.into_iter().find(|m| m.as_str() == v)
    }

    /// core の入力から記録の種類を求める。取り消しは版 1 では記録の行の対象にしないため `None`。
    pub fn from_input(input: &Input) -> Option<InputMethod> {
        match input {
            Input::Command(_) => Some(InputMethod::Command),
            Input::Text(_) => Some(InputMethod::Text),
            Input::Choice(_) => Some(InputMethod::Choice),
            Input::Ui { .. } => Some(InputMethod::Ui),
            Input::Approval { .. } => Some(InputMethod::Approval),
            Input::Cancel => None,
        }
    }
}

/// 承認の操作の `data-decision` の値。
pub const fn decision_value(d: Decision) -> &'static str {
    match d {
        Decision::Approve => "approve",
        Decision::Reject => "reject",
        Decision::Revise => "revise",
    }
}

/// `data-decision` の値の一覧。
pub const DECISION_VALUES: [&str; 3] = ["approve", "reject", "revise"];

/// 状態表示の値（待機中・処理中・停止）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    /// 待機中。
    Idle,
    /// 処理中。
    Busy,
    /// 停止。
    Stopped,
}

impl RunStatus {
    /// すべての値。
    pub const ALL: [RunStatus; 3] = [RunStatus::Idle, RunStatus::Busy, RunStatus::Stopped];

    /// `data-state` の値。
    pub const fn data_state(self) -> &'static str {
        match self {
            RunStatus::Idle => "idle",
            RunStatus::Busy => "busy",
            RunStatus::Stopped => "stopped",
        }
    }

    /// 表示の語。
    pub const fn word(self) -> &'static str {
        match self {
            RunStatus::Idle => "待機中",
            RunStatus::Busy => "処理中",
            RunStatus::Stopped => "停止",
        }
    }

    /// `data-state` の値から引く。未知は `None`。
    pub fn parse(v: &str) -> Option<RunStatus> {
        RunStatus::ALL.into_iter().find(|s| s.data_state() == v)
    }
}

/// Step の状態の語。版 1 が固定するのは受付済みと達成の 2 語だけで、他は `None`。
pub const fn step_status_word(s: StepStatus) -> Option<&'static str> {
    match s {
        StepStatus::Accepted => Some("受付済み（確認待ち）"),
        StepStatus::Achieved => Some("達成"),
        StepStatus::NotStarted | StepStatus::Entered | StepStatus::Skipped => None,
    }
}

/// 右のエリアで許される `data-testid` の閉じた一覧（本契約 + `a11y` が持つもの）に含まれるか。
pub fn is_defined_testid(id: &str) -> bool {
    TERMINAL_AREA_TESTIDS.contains(&id)
        || LiveRegion::ALL.iter().any(|l| l.testid() == id)
        || StatusNotice::ALL.iter().any(|n| n.testid() == id)
        || [
            LOG_LIST_TESTID,
            TABLIST_TESTID,
            TAB_TESTID,
            MARK_GLYPH_TESTID,
            MARK_TEXT_TESTID,
        ]
        .contains(&id)
}

/// 適合の違反。位置は根からの子の添字の列（根は空）。文面に文字ノードの中身は含めない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// 生 HTML は中身を検査できないため認めない。
    RawHtml {
        /// 位置。
        path: Vec<usize>,
    },
    /// 要素の規則違反。
    Element {
        /// 位置。
        path: Vec<usize>,
        /// 違反した規則の名前。
        rule: &'static str,
        /// 識別子・tag・属性名など。
        detail: String,
    },
    /// 対象が見つからない（`check_view` の fail-closed）。
    Count {
        /// 数えた対象。
        what: String,
        /// 期待する個数（最小）。
        expected: usize,
        /// 見つかった個数。
        found: usize,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Violation::RawHtml { path } => write!(f, "raw html at {path:?} cannot be checked"),
            Violation::Element { path, rule, detail } => write!(f, "{rule} at {path:?}: {detail}"),
            Violation::Count {
                what,
                expected,
                found,
            } => write!(f, "expected at least {expected} of {what}, found {found}"),
        }
    }
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn path_of(links: &[(usize, usize)], mut id: usize) -> Vec<usize> {
    let mut path = Vec::new();
    while links[id].0 != usize::MAX {
        path.push(links[id].1);
        id = links[id].0;
    }
    path.reverse();
    path
}

fn is_digits(v: &str) -> bool {
    !v.is_empty() && v.len() <= 18 && v.bytes().all(|b| b.is_ascii_digit())
}

fn is_line_part(id: &str) -> bool {
    id.starts_with(LINE_PREFIX)
}

/// 1 行の集計。行の部品は、行の木をすべて歩いた後に必須・禁止を照合する。
struct Line {
    id: usize,
    kind: Option<LineKind>,
    input: Option<InputMethod>,
    parts: Vec<&'static str>,
}

/// 祖先の文脈。子へ持ち回す。
#[derive(Clone, Copy, Default)]
struct Ctx {
    /// 直接の親が `log-list`。
    parent_is_log_list: bool,
    /// 属する行（`lines` の添字）。
    line: Option<usize>,
    /// 行の部品の中。
    in_part: bool,
}

/// 1 行の部品の要件。`(必須, 任意)`。
fn part_spec(
    kind: LineKind,
    input: Option<InputMethod>,
) -> Option<(&'static [&'static str], &'static [&'static str])> {
    Some(match kind {
        LineKind::Received => match input? {
            InputMethod::Approval => (&[LINE_DECISION_TESTID], &[LINE_TARGET_TESTID]),
            _ => (&[LINE_BODY_TESTID], &[]),
        },
        LineKind::Response => (&[LINE_BODY_TESTID], &[LINE_NEXT_ACTION_TESTID]),
        LineKind::Rejected => (&[LINE_REASON_TESTID, LINE_NEXT_ACTION_TESTID], &[]),
        LineKind::Clarified => (&[LINE_QUESTION_TESTID, LINE_NEXT_ACTION_TESTID], &[]),
        LineKind::Progress => (&[LINE_STAGE_TESTID], &[]),
        LineKind::Stopped => (&[LINE_BODY_TESTID], &[]),
    })
}

/// 本契約の部品の識別子（`'static` の一覧から引く）。
fn static_part(id: &str) -> Option<&'static str> {
    TERMINAL_AREA_TESTIDS.iter().copied().find(|t| *t == id)
}

fn has_nonblank_text(children: &[Node]) -> bool {
    children
        .iter()
        .any(|c| matches!(c, Node::Text(t) if t.chars().any(|ch| !ch.is_whitespace())))
}

fn all_text(children: &[Node]) -> bool {
    children.iter().all(|c| matches!(c, Node::Text(_)))
}

fn direct_text(children: &[Node]) -> String {
    let mut s = String::new();
    for c in children {
        if let Node::Text(t) = c {
            s.push_str(t);
        }
    }
    s
}

struct Walk {
    out: Vec<Violation>,
    links: Vec<(usize, usize)>,
    lines: Vec<Line>,
}

impl Walk {
    fn bad(&mut self, id: usize, rule: &'static str, detail: impl Into<String>) {
        self.out.push(Violation::Element {
            path: path_of(&self.links, id),
            rule,
            detail: detail.into(),
        });
    }

    /// `log-list` の直接の子の行の属性を検査し、行を登録する。
    fn start_line(&mut self, id: usize, attrs: &[(String, String)]) -> usize {
        let kind = match attr(attrs, KIND_ATTR) {
            None => {
                self.bad(id, "line-kind-missing", KIND_ATTR);
                None
            }
            Some(v) => {
                let k = LineKind::parse(v);
                if k.is_none() {
                    self.bad(id, "line-kind-unknown", KIND_ATTR);
                }
                k
            }
        };
        if !attr(attrs, GEN_ATTR).is_some_and(is_digits) {
            self.bad(id, "line-gen", GEN_ATTR);
        }
        let mut input = None;
        match kind {
            Some(LineKind::Received) => match attr(attrs, INPUT_ATTR).map(InputMethod::parse) {
                Some(Some(m)) => input = Some(m),
                _ => self.bad(id, "line-input", INPUT_ATTR),
            },
            Some(LineKind::Rejected) => {
                let ok = attr(attrs, CATEGORY_ATTR)
                    .is_some_and(|v| RejectCategory::ALL.iter().any(|c| c.as_str() == v));
                if !ok {
                    self.bad(id, "line-category", CATEGORY_ATTR);
                }
            }
            Some(LineKind::Progress) => {
                let idx = attr(attrs, INDEX_ATTR).filter(|v| is_digits(v));
                let tot = attr(attrs, TOTAL_ATTR).filter(|v| is_digits(v));
                let ok = match (idx, tot) {
                    (Some(i), Some(t)) => matches!(
                        (i.parse::<u64>(), t.parse::<u64>()),
                        (Ok(i), Ok(t)) if i < t
                    ),
                    _ => false,
                };
                if !ok {
                    self.bad(id, "line-progress", "data-index and data-total");
                }
            }
            _ => {}
        }
        self.lines.push(Line {
            id,
            kind,
            input,
            parts: Vec::new(),
        });
        self.lines.len() - 1
    }

    fn finish_lines(&mut self) {
        let lines = core::mem::take(&mut self.lines);
        for line in &lines {
            let Some(kind) = line.kind else { continue };
            let Some((required, optional)) = part_spec(kind, line.input) else {
                continue;
            };
            for r in required {
                if !line.parts.contains(r) {
                    self.bad(line.id, "line-part-missing", *r);
                }
            }
            for (i, p) in line.parts.iter().enumerate() {
                if !required.contains(p) && !optional.contains(p) {
                    self.bad(line.id, "line-part-forbidden", *p);
                } else if line.parts[..i].contains(p) {
                    self.bad(line.id, "line-part-duplicate", *p);
                }
            }
        }
    }
}

fn walk(area: &Node) -> Vec<Violation> {
    let mut w = Walk {
        out: Vec::new(),
        links: vec![(usize::MAX, 0)],
        lines: Vec::new(),
    };
    // 再帰しない。深い木でもスタックあふれを起こさないため。位置は親への参照の表で持つ。
    let mut stack: Vec<(&Node, usize, Ctx)> = vec![(area, 0, Ctx::default())];
    while let Some((node, id, ctx)) = stack.pop() {
        let (tag, attrs, children) = match node {
            Node::Text(_) => continue,
            Node::RawHtml(_) => {
                w.out.push(Violation::RawHtml {
                    path: path_of(&w.links, id),
                });
                continue;
            }
            Node::Element {
                tag,
                attrs,
                children,
            } => (*tag, attrs, children),
        };
        let mut child_ctx = Ctx {
            parent_is_log_list: false,
            line: ctx.line,
            in_part: ctx.in_part,
        };
        let testid = attr(attrs, TESTID_ATTR);
        if let Some(t) = testid {
            if !is_defined_testid(t) {
                w.bad(id, "undefined-testid", t);
            }
        }
        if ctx.parent_is_log_list {
            if tag == "li" {
                child_ctx.line = Some(w.start_line(id, attrs));
            } else {
                w.bad(id, "log-list-child", tag);
                child_ctx.line = None;
            }
            child_ctx.in_part = false;
        }
        match testid {
            Some(LOG_LIST_TESTID) => {
                child_ctx.parent_is_log_list = true;
                child_ctx.line = None;
                child_ctx.in_part = false;
            }
            Some(RUN_STATUS_TESTID) => check_run_status(&mut w, id, attrs, children),
            Some(t) if is_line_part(t) => {
                check_part(&mut w, id, t, attrs, children, ctx, &mut child_ctx)
            }
            _ => {}
        }
        for (i, c) in children.iter().enumerate().rev() {
            w.links.push((id, i));
            stack.push((c, w.links.len() - 1, child_ctx));
        }
    }
    w.finish_lines();
    w.out
}

fn check_run_status(w: &mut Walk, id: usize, attrs: &[(String, String)], children: &[Node]) {
    match attr(attrs, STATE_ATTR).and_then(RunStatus::parse) {
        None => w.bad(id, "run-status-state", STATE_ATTR),
        Some(s) => {
            if direct_text(children).trim() != s.word() {
                w.bad(id, "run-status-word", STATE_ATTR);
            }
        }
    }
}

fn check_part(
    w: &mut Walk,
    id: usize,
    testid: &str,
    attrs: &[(String, String)],
    children: &[Node],
    ctx: Ctx,
    child_ctx: &mut Ctx,
) {
    let Some(line_idx) = ctx.line else {
        w.bad(id, "line-part-outside-line", testid);
        return;
    };
    if ctx.in_part {
        w.bad(id, "line-part-nested", testid);
        return;
    }
    child_ctx.in_part = true;
    let Some(part) = static_part(testid) else {
        // 予約空間の未定義の識別子は `undefined-testid` で報告済み。
        return;
    };
    let line = &mut w.lines[line_idx];
    line.parts.push(part);
    let (kind, input) = (line.kind, line.input);
    if part == LINE_NEXT_ACTION_TESTID && !has_nonblank_text(children) {
        w.bad(id, "next-action-empty", testid);
    }
    let learner_text = (kind == Some(LineKind::Received) && part == LINE_BODY_TESTID)
        || (input == Some(InputMethod::Approval) && part == LINE_TARGET_TESTID);
    if learner_text && !all_text(children) {
        w.bad(id, "learner-text-children", testid);
    }
    if part == LINE_DECISION_TESTID
        && !attr(attrs, DECISION_ATTR).is_some_and(|v| DECISION_VALUES.contains(&v))
    {
        w.bad(id, "line-decision-value", DECISION_ATTR);
    }
}

/// 渡された部分木を右のエリアとして照合する。違反はすべて集めて返す。
///
/// 規則: `RawHtml` の禁止、`data-testid` の閉じた一覧、`log-list` の子と行の属性・部品、部品の配置と重複、
/// `line-next-action` の非空、受講者の文字列の部品が文字ノードだけであること、`run-status` の値と語。
pub fn check_area(area: &Node) -> Result<(), Vec<Violation>> {
    let out = walk(area);
    if out.is_empty() {
        Ok(())
    } else {
        Err(out)
    }
}

/// 画面全体から右のエリア（`aria-label` が案内とログ・コマンドの入力の要素）を探して照合する。
/// 1 つも無ければ失敗する（対象が無いまま空振りで通るのを防ぐ）。エリアの外は管轄外。
pub fn check_view(view: &Node) -> Result<(), Vec<Violation>> {
    let labels = [
        Region::GuideAndLog.aria_label(),
        Region::CommandInput.aria_label(),
    ];
    let mut links: Vec<(usize, usize)> = vec![(usize::MAX, 0)];
    let mut stack: Vec<(&Node, usize)> = vec![(view, 0)];
    let mut roots: Vec<(&Node, usize)> = Vec::new();
    while let Some((node, id)) = stack.pop() {
        let Node::Element {
            attrs, children, ..
        } = node
        else {
            continue;
        };
        if attr(attrs, "aria-label").is_some_and(|v| labels.contains(&v)) {
            // 最も外側の根で打ち切る。内側は外側の照合が含む。
            roots.push((node, id));
            continue;
        }
        for (i, c) in children.iter().enumerate().rev() {
            links.push((id, i));
            stack.push((c, links.len() - 1));
        }
    }
    if roots.is_empty() {
        return Err(vec![Violation::Count {
            what: "terminal area region".to_string(),
            expected: 1,
            found: 0,
        }]);
    }
    let mut out = Vec::new();
    for (node, id) in roots {
        if let Err(vs) = check_area(node) {
            let base = path_of(&links, id);
            for v in vs {
                out.push(match v {
                    Violation::RawHtml { path } => Violation::RawHtml {
                        path: [base.clone(), path].concat(),
                    },
                    Violation::Element { path, rule, detail } => Violation::Element {
                        path: [base.clone(), path].concat(),
                        rule,
                        detail,
                    },
                    other => other,
                });
            }
        }
    }
    if out.is_empty() {
        Ok(())
    } else {
        Err(out)
    }
}
