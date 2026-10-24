//! 教材テストを共通エンジン（core の `Session`）で流す実行系と、合否の判定（TASK-404 1/3、AUTHOR-4）。
//!
//! 呼び出し元は `cli::run_test` と結合試験である。判定の論理は core の `apply` に任せ、ここは入力を `Op` に写し、
//! 追加されたイベント列から 1 操作の観測値を決めて、期待と比べるだけにする（判定を再実装しない）。
//!
//! 流す前の関門（[`run_pack`]）は、読み込み → `validate` → 教材テストの読み取り → `compile` の順で、
//! 1 つでも診断があれば試験を流さない（fail-closed。[`TestOutcome::NotRun`]）。
//!
//! 開始は画面（`web::start_select`）と同じく `ir.starts` の `node` から `Session::entered_at` で作る。
//! 進行の宛先も `web::tick_schedule::next_tick` と同じ式で組む。pack から web へは依存しないので式を写し、
//! 画面側が変わるときは本モジュールも同じ変更で追従する。
//!
//! `cmd:`・`text:` は、どちらも入力欄に打つ文字列として画面の送信と同じ `submit_input` で分ける（TASK-406 1/2、AUTHOR-7）。
//! 接頭辞は IR の `pack.profile` から 1 回だけ読み、読めなければ送らず `no_judgment` にする（画面も送らない）。
//! 鍵と分けの食い違い（`E-PLAY-INPUT-CLASS`）は [`run_loaded`] が `profile::check_play_inputs` で集め、入力は止めず画面と
//! 同じ判定で流した上で [`Report::input_class`] に持つ。1 件でもあれば [`Report::all_passed`] は偽になる（TASK-406 2/2）。
//! 画面の表示（ビューの行・本文・外観・入力欄が隠れること・見た目）と停止（Esc）は確かめない（`docs/cli.md` 12.1）。
//!
//! 扱わないこと
//! - R-22・R-21・入った実体の集計と最終行（TASK-405）。PASS・FAIL・集計の行は `cli::run_test` が `cli_contract` の関数で書く（TASK-404 2/3・3/3）。報告 [`Report`] が材料と試験の診断を持つ
//! - 空白だけの `cmd:`・`text:` を送らないこと（画面は送らないが、ここは分けた結果をそのまま適用する。既知の差）
//! - `text_checks`・`guidance_checks`・`asserts` の判定（[`Report::unsupported`] に積み、不合格に倒す）
//!
//! 全域かつ決定的である。I/O は [`run_pack`] の `source::load` だけで、時刻・乱数を持たない。

use std::path::Path;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_contract::profile::read_profile;
use fandhe_workshop_core::addr::{StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_replay::submit_input;
use fandhe_workshop_core::input::{Control, Decision, Input, Op};
use fandhe_workshop_core::Session;

use crate::cli_contract::PackSummary;
use crate::compile::compile;
use crate::diag::{DiagCode, Diagnostic};
use crate::play_expect::ResultExpect;
use crate::play_script::{
    self, ApprovalWord, End, ErrorInput, Expect, PlayOp, Playthrough, Script, ScriptStep,
    Unsupported,
};
use crate::source::{self, LoadedPack};
use crate::validate;

/// 1 操作で観測した結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    /// 受付（`free_text` は自然文の受付の注記つき）。
    Accepted {
        /// `accepted:free_text_unjudged` か。
        free_text: bool,
    },
    /// 拒否。
    Rejected(RejectCategory),
    /// 問い返し。
    Clarified,
    /// 確認の操作による達成。
    Achieved,
    /// 進行が 1 つ進んだ。
    Progress,
    /// 無視された。
    Ignored,
    /// 実行環境の操作が拒否された。
    Refused,
    /// 警告が出た（想定外の状態）。
    Warning,
    /// 遷移の上限で止まった（SAFE-23）。
    TransitionLimit,
    /// 判定が 1 件も無かった。
    NoJudgment,
    /// 開始点が無い。
    UnknownStart,
}

impl Observed {
    /// 教材テストの期待と同じ書き方の語（`rejected:busy` など）。後続の FAIL の行が使う。
    pub fn word(&self) -> String {
        match self {
            Observed::Accepted { free_text: false } => "accepted".to_string(),
            Observed::Accepted { free_text: true } => "accepted:free_text_unjudged".to_string(),
            Observed::Rejected(c) => format!("rejected:{}", c.as_str()),
            Observed::Clarified => "clarified".to_string(),
            Observed::Achieved => "achieved".to_string(),
            Observed::Progress => "progress".to_string(),
            Observed::Ignored => "ignored".to_string(),
            Observed::Refused => "refused".to_string(),
            Observed::Warning => "warning".to_string(),
            Observed::TransitionLimit => "transition_limit".to_string(),
            Observed::NoJudgment => "no_judgment".to_string(),
            Observed::UnknownStart => "unknown_start".to_string(),
        }
    }

    fn matches_expect(&self, e: &Expect) -> bool {
        match (e, self) {
            (Expect::Achieved, Observed::Achieved) | (Expect::Progress, Observed::Progress) => true,
            (Expect::Result(r), o) => o.matches_result(r),
            _ => false,
        }
    }

    fn matches_result(&self, r: &ResultExpect) -> bool {
        match (r, self) {
            (ResultExpect::Accepted { free_text }, Observed::Accepted { free_text: f }) => {
                free_text == f
            }
            (ResultExpect::Rejected(c), Observed::Rejected(o)) => c == o,
            (ResultExpect::Clarified, Observed::Clarified) => true,
            _ => false,
        }
    }
}

/// 不合格の位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// 入力が期待と合わなかった。
    Mismatch {
        /// 何番目の segment か（1 始まり。誤入力は 0）。
        segment: usize,
        /// 何番目の入力か（1 始まり。誤入力の本番の入力と開始できない場合は 0）。
        step: usize,
        /// 期待の語。
        expected: String,
        /// 観測した結果。
        observed: Observed,
    },
    /// 終わりの条件を満たさなかった。実体 ID は持たず、条件の種類だけを持つ。
    EndNotReached {
        /// `end: complete` だったか（偽なら Step の指定）。
        complete: bool,
    },
}

/// 1 件（模範解答または誤入力）の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseResult {
    /// ID。
    pub id: String,
    /// 流した入力の件数。
    pub inputs: usize,
    /// 不合格の理由。合格なら `None`。
    pub failure: Option<Failure>,
    /// 誤入力の本番の入力の観測値（模範解答は `None`）。
    pub observed: Option<Observed>,
}

impl CaseResult {
    /// 合格か。
    pub fn passed(&self) -> bool {
        self.failure.is_none()
    }
}

/// 全件の報告。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// 模範解答の結果。
    pub playthroughs: Vec<CaseResult>,
    /// 誤入力の結果。
    pub error_inputs: Vec<CaseResult>,
    /// 判定していない定義。あれば不合格に倒す。
    pub unsupported: Vec<Unsupported>,
    /// 教材テストのファイル（パック根からの相対パス）。試験の診断の `file` 欄に使う。
    pub tests_file: String,
    /// 入力の分けの診断（`E-PLAY-INPUT-CLASS`。接頭辞が `/` 以外のときだけ）。流した結果とは別に持ち、
    /// 1 件でもあれば不合格に倒す（食い違いを通ったと見せない）。
    pub input_class: Vec<Diagnostic>,
}

impl CaseResult {
    /// FAIL の行の材料（期待の語・実際の語・位置の注記）。合格なら `None`。
    ///
    /// 語は固定語彙だけで、教材データの値と実体 ID は含めない。
    pub fn fail_parts(&self) -> Option<(String, String, Option<String>)> {
        match self.failure.as_ref()? {
            Failure::Mismatch {
                segment,
                step,
                expected,
                observed,
            } => {
                let note = match (*segment, *step) {
                    (0, 0) => None,
                    (0, n) => Some(format!("事前の入力 {n}")),
                    (s, 0) => Some(format!("区間 {s} の開始")),
                    (s, n) => Some(format!("区間 {s} の入力 {n}")),
                };
                Some((expected.clone(), observed.word(), note))
            }
            Failure::EndNotReached { complete } => {
                let (e, o) = if *complete {
                    ("complete", "incomplete")
                } else {
                    ("end_step", "other_step")
                };
                Some((
                    e.to_string(),
                    o.to_string(),
                    Some("終わりの条件".to_string()),
                ))
            }
        }
    }
}

impl Report {
    /// 合格した模範解答の数。
    pub fn passed_playthroughs(&self) -> usize {
        self.playthroughs.iter().filter(|c| c.passed()).count()
    }

    /// 期待どおりだった誤入力の数。
    pub fn passed_error_inputs(&self) -> usize {
        self.error_inputs.iter().filter(|c| c.passed()).count()
    }

    /// 試験の診断を、入力の分け（流す前に静的に決まる）、模範解答、誤入力の記載の順で返す（TASK-404 3/3・TASK-406 2/2）。
    ///
    /// メッセージは英語の固定文と固定語彙だけで、教材データの値は写さない。
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let file = if self.tests_file.trim().is_empty() {
            "."
        } else {
            self.tests_file.as_str()
        };
        let one = |c: &CaseResult, kind: &str, code: DiagCode, what: &str| {
            let (expected, observed, _) = c.fail_parts()?;
            Diagnostic::new(
                file,
                format!("{kind}:{}", c.id),
                code,
                format!("{what}: expected {expected}, got {observed}"),
            )
            .ok()
        };
        let pts = self.playthroughs.iter().filter_map(|c| {
            one(
                c,
                "playthrough",
                DiagCode::PlayIncomplete,
                "playthrough did not complete as scripted",
            )
        });
        let eis = self.error_inputs.iter().filter_map(|c| {
            one(
                c,
                "error_input",
                DiagCode::PlayCategoryMismatch,
                "error input result differs from expected",
            )
        });
        self.input_class
            .iter()
            .cloned()
            .chain(pts)
            .chain(eis)
            .collect()
    }

    /// 全件が合格で、判定していない定義と入力の分けの診断が無く、1 件以上流したとき真。
    pub fn all_passed(&self) -> bool {
        !self.playthroughs.is_empty()
            && self.unsupported.is_empty()
            && self.input_class.is_empty()
            && self.playthroughs.iter().all(CaseResult::passed)
            && self.error_inputs.iter().all(CaseResult::passed)
    }

    /// 流した入力の総数。
    pub fn total_inputs(&self) -> usize {
        self.playthroughs
            .iter()
            .chain(&self.error_inputs)
            .map(|c| c.inputs)
            .sum()
    }
}

/// `run_pack` の結果。関門で止まったかどうかが型で見える。
#[derive(Debug)]
pub enum TestOutcome {
    /// 読み込めなかった。
    Unreadable(Vec<Diagnostic>),
    /// 関門の診断があり、試験を流していない。
    NotRun {
        /// 関門の診断。
        diags: Vec<Diagnostic>,
        /// 最終行の要約。
        summary: PackSummary,
    },
    /// 試験を流した。
    Ran(Report),
}

/// 関門を通してから教材テストを流す。ディレクトリを読むのはここだけ。
pub fn run_pack(dir: &Path) -> TestOutcome {
    match source::load(dir) {
        Err(d) => TestOutcome::Unreadable(d),
        Ok(pack) => run_loaded(&pack),
    }
}

/// 読み込み済みのパックに関門を当て、通れば流す。
pub fn run_loaded(pack: &LoadedPack) -> TestOutcome {
    let summary = PackSummary::of(pack);
    let mut diags = validate::validate(pack);
    let script = match play_script::read(pack) {
        Ok(s) => Some(s),
        Err(d) => {
            diags.extend(d);
            None
        }
    };
    let (Some(script), true) = (script, diags.is_empty()) else {
        return TestOutcome::NotRun { diags, summary };
    };
    match compile(pack) {
        Ok(ir) => {
            let mut report = run(&ir, &script);
            report.tests_file = pack.tests_file.clone().unwrap_or_default();
            report.input_class = crate::profile::check_play_inputs(pack);
            TestOutcome::Ran(report)
        }
        Err(d) => TestOutcome::NotRun { diags: d, summary },
    }
}

/// 教材テスト全件を流す。
pub fn run(ir: &Ir, script: &Script) -> Report {
    // 画面の適用し直し（`history_replay::replay`）と同じ読み方。読めなければ None で、入力は送らない。
    let prefix = read_profile(ir.pack.profile.as_ref())
        .ok()
        .map(|p| p.command_prefix);
    let prefix = prefix.as_deref();
    Report {
        playthroughs: script
            .playthroughs
            .iter()
            .map(|p| run_playthrough(ir, prefix, p))
            .collect(),
        error_inputs: script
            .error_inputs
            .iter()
            .map(|e| run_error_input(ir, prefix, e))
            .collect(),
        unsupported: script.unsupported.clone(),
        tests_file: String::new(),
        input_class: Vec::new(),
    }
}

fn open(ir: &Ir, start: &str) -> Option<Session> {
    let s = ir.starts.iter().find(|s| s.id == start)?;
    ir.nodes.iter().find(|n| n.uid == s.node)?;
    Some(Session::entered_at(ir.clone(), &s.node))
}

/// 入力欄に打つ文字列を画面と同じ関数で分けて `Op` にする。接頭辞が無い（読めない）ときは送らない。
fn typed(prefix: Option<&str>, raw: &str) -> Option<Op> {
    prefix.map(|p| Op::Input(submit_input(p, raw.to_string())))
}

fn to_op(session: &Session, prefix: Option<&str>, op: &PlayOp) -> Option<Op> {
    if let PlayOp::Command(s) | PlayOp::Text(s) = op {
        return typed(prefix, s);
    }
    Some(match op {
        PlayOp::Command(_) | PlayOp::Text(_) => return None,
        PlayOp::Choice(s) => Op::Input(Input::Choice(s.clone())),
        PlayOp::Ui { view, path } => Op::Input(Input::Ui {
            view: view.clone(),
            path: path.clone(),
        }),
        PlayOp::Approval { decision, target } => Op::Input(Input::Approval {
            decision: match decision {
                ApprovalWord::Approve => Decision::Approve,
                ApprovalWord::Reject => Decision::Reject,
                ApprovalWord::Revise => Decision::Revise,
            },
            target: target.clone(),
        }),
        PlayOp::Cancel => Op::Input(Input::Cancel),
        PlayOp::Tick => {
            let run = &session.state().run;
            // web の tick_schedule::next_tick と同じ式。処理中でなければ index 0 で送り、core が無視する。
            Op::Tick(TickAddr {
                generation: run.generation,
                step: run
                    .current
                    .clone()
                    .unwrap_or_else(|| StepUid(String::new())),
                index: run.busy.as_ref().map_or(0, |b| b.done),
            })
        }
    })
}

fn observe(events: &[Event]) -> Observed {
    if events
        .iter()
        .any(|e| matches!(e, Event::TransitionLimitReached { .. }))
    {
        return Observed::TransitionLimit;
    }
    if events.iter().any(|e| matches!(e, Event::Refused { .. })) {
        return Observed::Refused;
    }
    if let Some(j) = events.iter().find_map(|e| match e {
        Event::Judged { judgment, .. } => Some(judgment),
        _ => None,
    }) {
        return match j {
            Judgment::Accepted(note) => Observed::Accepted {
                free_text: note.is_some(),
            },
            Judgment::Rejected(r) => Observed::Rejected(r.category),
            Judgment::Clarified(_) => Observed::Clarified,
            Judgment::Confirmed => Observed::Achieved,
            Judgment::Progressed => Observed::Progress,
            Judgment::Ignored => Observed::Ignored,
        };
    }
    // 進行は Judged を出さない。最後の進行は受付、途中は進行として見る。
    if events.iter().any(|e| matches!(e, Event::Ignored { .. })) {
        return Observed::Ignored;
    }
    if events.iter().any(|e| matches!(e, Event::Accepted { .. })) {
        return Observed::Accepted { free_text: false };
    }
    if events.iter().any(|e| matches!(e, Event::Progress { .. })) {
        return Observed::Progress;
    }
    // 警告は判定・受付・進行と同時に出ることがある。それらが無いときだけ警告として見る。
    if events.iter().any(|e| matches!(e, Event::Warning { .. })) {
        return Observed::Warning;
    }
    Observed::NoJudgment
}

fn step_once(session: &mut Session, prefix: Option<&str>, op: &PlayOp) -> Observed {
    let Some(op) = to_op(session, prefix, op) else {
        return Observed::NoJudgment;
    };
    observe(session.apply(op))
}

/// 入力列を流し、最初に外れた位置を返す。`segment` は 1 始まり（0 は誤入力の `pre`）。
fn run_steps(
    session: &mut Session,
    prefix: Option<&str>,
    steps: &[ScriptStep],
    segment: usize,
    count: &mut usize,
) -> Option<Failure> {
    for (n, s) in steps.iter().enumerate() {
        *count += 1;
        let observed = step_once(session, prefix, &s.op);
        if !observed.matches_expect(&s.expect) {
            return Some(Failure::Mismatch {
                segment,
                step: n + 1,
                expected: match s.expect {
                    Expect::Achieved => "achieved".to_string(),
                    Expect::Progress => "progress".to_string(),
                    Expect::Result(r) => expect_word(&r),
                },
                observed,
            });
        }
    }
    None
}

fn expect_word(r: &ResultExpect) -> String {
    match r {
        ResultExpect::Accepted { free_text: false } => "accepted".to_string(),
        ResultExpect::Accepted { free_text: true } => "accepted:free_text_unjudged".to_string(),
        ResultExpect::Rejected(c) => format!("rejected:{}", c.as_str()),
        ResultExpect::Clarified => "clarified".to_string(),
    }
}

/// 終わりの条件を満たすか。`complete` は `run.completed` を見る。ただし core は章の完了（`completed` の更新）を
/// まだ結線していない（後続のタスク）ため、それまでの間は、全入力が期待どおりに通ったうえで最後の入力が
/// 確認の操作による達成（`achieved`）であることでも満たすとする。完了の結線後は後半を外す。
fn end_reached(session: &Session, p: &Playthrough) -> bool {
    let run = &session.state().run;
    match &p.end {
        End::Complete => {
            run.completed
                || p.segments
                    .last()
                    .and_then(|s| s.steps.last())
                    .is_some_and(|s| s.expect == Expect::Achieved)
        }
        End::AtStep(id) => run.current.as_ref().is_some_and(|c| &c.0 == id),
    }
}

fn run_playthrough(ir: &Ir, prefix: Option<&str>, p: &Playthrough) -> CaseResult {
    let mut inputs = 0;
    let mut session: Option<Session> = None;
    let result = |inputs, failure| CaseResult {
        id: p.id.clone(),
        inputs,
        failure,
        observed: None,
    };
    for (n, seg) in p.segments.iter().enumerate() {
        let begin_failed = Failure::Mismatch {
            segment: n + 1,
            step: 0,
            expected: "begin".to_string(),
            observed: Observed::UnknownStart,
        };
        let current = match session.as_mut() {
            Some(s) => {
                // 2 つ目以降の開始点は、状態を引き継いだまま重ねる。
                let events = s.apply(Op::Control(Control::Begin(seg.start.clone())));
                if events.iter().any(|e| matches!(e, Event::Refused { .. })) {
                    return result(inputs, Some(begin_failed));
                }
                s
            }
            None => match open(ir, &seg.start) {
                Some(s) => session.insert(s),
                None => return result(inputs, Some(begin_failed)),
            },
        };
        if let Some(f) = run_steps(current, prefix, &seg.steps, n + 1, &mut inputs) {
            return result(inputs, Some(f));
        }
    }
    let ok = session.as_ref().is_some_and(|s| end_reached(s, p));
    result(
        inputs,
        (!ok).then_some(Failure::EndNotReached {
            complete: matches!(p.end, End::Complete),
        }),
    )
}

fn run_error_input(ir: &Ir, prefix: Option<&str>, e: &ErrorInput) -> CaseResult {
    let fail = |inputs, failure, observed| CaseResult {
        id: e.id.clone(),
        inputs,
        failure: Some(failure),
        observed,
    };
    let Some(mut session) = open(ir, &e.start) else {
        return fail(
            0,
            Failure::Mismatch {
                segment: 0,
                step: 0,
                expected: expect_word(&e.expect),
                observed: Observed::UnknownStart,
            },
            Some(Observed::UnknownStart),
        );
    };
    let mut inputs = 0;
    if let Some(f) = run_steps(&mut session, prefix, &e.pre, 0, &mut inputs) {
        return fail(inputs, f, None);
    }
    inputs += 1;
    let observed = step_once(&mut session, prefix, &e.input);
    if observed.matches_result(&e.expect) {
        CaseResult {
            id: e.id.clone(),
            inputs,
            failure: None,
            observed: Some(observed),
        }
    } else {
        fail(
            inputs,
            Failure::Mismatch {
                segment: 0,
                step: 0,
                expected: expect_word(&e.expect),
                observed,
            },
            Some(observed),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_and_text_are_split_by_submit_input() {
        for p in ["/", "$", "::", ""] {
            for raw in [
                "/ls",
                "ls",
                "  $ls ",
                "\u{3000}$ls",
                "一行目\n二行目",
                "::x",
                ":x",
            ] {
                let want = Some(Op::Input(submit_input(p, raw.to_string())));
                assert_eq!(typed(Some(p), raw), want);
                assert_eq!(typed(None, raw), None);
            }
        }
    }

    #[test]
    fn observed_words_match_expect_notation() {
        assert_eq!(Observed::Accepted { free_text: false }.word(), "accepted");
        assert_eq!(
            Observed::Accepted { free_text: true }.word(),
            "accepted:free_text_unjudged"
        );
        assert_eq!(
            Observed::Rejected(RejectCategory::Busy).word(),
            "rejected:busy"
        );
        assert!(Observed::Achieved.matches_expect(&Expect::Achieved));
        assert!(!Observed::Progress.matches_expect(&Expect::Achieved));
        assert!(!Observed::Ignored.matches_expect(&Expect::Result(ResultExpect::Clarified)));
    }

    fn stamp() -> fandhe_workshop_core::addr::Stamp {
        fandhe_workshop_core::addr::Stamp {
            generation: fandhe_workshop_core::addr::Generation::INITIAL,
            step: StepUid(String::new()),
        }
    }

    fn warning() -> Event {
        Event::Warning {
            at: stamp(),
            message: "w".into(),
        }
    }

    #[test]
    fn warning_does_not_hide_accepted() {
        let events = [warning(), Event::Accepted { at: stamp() }];
        assert_eq!(observe(&events), Observed::Accepted { free_text: false });
    }

    #[test]
    fn warning_alone_is_observed_as_warning() {
        assert_eq!(observe(&[warning()]), Observed::Warning);
    }

    #[test]
    fn empty_report_is_not_a_pass() {
        assert!(!Report::default().all_passed());
        let mut r = Report::default();
        r.playthroughs.push(CaseResult {
            id: "p".into(),
            inputs: 1,
            failure: None,
            observed: None,
        });
        assert!(r.all_passed());
        r.unsupported.push(Unsupported::Asserts);
        assert!(!r.all_passed());
    }

    #[test]
    fn input_class_diagnostic_is_not_a_pass() {
        let mut r = Report::default();
        r.playthroughs.push(CaseResult {
            id: "p".into(),
            inputs: 1,
            failure: None,
            observed: None,
        });
        assert!(r.all_passed());
        r.input_class.push(
            Diagnostic::new("t.yaml", "playthrough:p", DiagCode::PlayInputClass, "m")
                .expect("diag"),
        );
        assert!(!r.all_passed());
        assert_eq!(r.diagnostics().len(), 1);
    }
}
