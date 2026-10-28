//! `module-reuse-rounds` の 2 周分の操作列を「計画」（`PlanStep` の列）として持つ共有部品（TASK-216。EVT-13、MOD-5）。
//!
//! 呼び出し元は `round_slice_init.rs`（切り身・世界状態のハッシュの試験）と `history_size_rounds.rs`（2 周分の履歴の大きさの測定）、
//! web の `log_rounds_count.rs`（2 周分の右のログの項目数の測定。TASK-454 1/2）。
//! 同じ入力列を 2 つの試験が共有し、測定の対象と状態の試験の対象が食い違わないようにする。
//! 計画は値の列なので、TASK-336 が誤入力や `Control::Save` を差し込んで条件 B／C を作れる。動かす側（`Session` か
//! `RecordedSession`）の差は [`Drive`] が吸収する。
//!
//! 未実装の部品（着地の `on_enter`、`when` つき `next`、`wait` の自動開始）を避けるため、1 周目は 3 か所、2 周目は 2 か所を
//! `GoTo` で飛ばす（`docs/testing.md` の 5.258・5.260）。3 周目（TASK-245）も 2 周目と同じ 2 か所を飛ばす。飛ばした分、`Changed` の件数は真の通しと違いうる。

#![allow(dead_code, reason = "試験ファイルごとに使う部品が異なるため")]

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Decision, Input, Op};
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::{Session, SessionState};

/// 計画の 1 手。
#[derive(Debug, Clone)]
pub enum PlanStep {
    /// エンジンへ送る操作。
    Op(Op),
    /// 処理中の次の段階を進める。宛先は実行時の世代・現在の Step・処理中の進み具合から組む。
    Tick,
    /// 現在の Step の uid が、この接尾辞で終わることの確認（途中の位置の確認を計画に埋め込む）。
    ExpectCurrentSuffix(&'static str),
}

/// 計画の中の `GoTo` の数（測定の基点 `goto_substitutions` の根拠）。
pub fn goto_count(plan: &[PlanStep]) -> usize {
    plan.iter()
        .filter(|s| matches!(s, PlanStep::Op(Op::Control(Control::GoTo(_)))))
        .count()
}

/// 計画を動かす側。`Session` と `RecordedSession` の差を吸収する。
pub trait Drive {
    fn apply(&mut self, op: Op) -> Vec<Event>;
    fn state(&self) -> &SessionState;
}

impl Drive for Session {
    fn apply(&mut self, op: Op) -> Vec<Event> {
        Session::apply(self, op).to_vec()
    }
    fn state(&self) -> &SessionState {
        Session::state(self)
    }
}

impl Drive for RecordedSession {
    fn apply(&mut self, op: Op) -> Vec<Event> {
        RecordedSession::apply(self, op).to_vec()
    }
    fn state(&self) -> &SessionState {
        self.session().state()
    }
}

/// 計画を順に流し、出たイベントを返す。確認に外れたら panic する（試験の部品のため）。
pub fn run_plan(d: &mut impl Drive, plan: &[PlanStep]) -> Vec<Event> {
    let mut events = Vec::new();
    for step in plan {
        match step {
            PlanStep::Op(op) => events.extend(d.apply(op.clone())),
            PlanStep::Tick => {
                let run = &d.state().run;
                let addr = TickAddr {
                    generation: run.generation,
                    step: run.current.clone().expect("current"),
                    index: run.busy.as_ref().expect("busy").done,
                };
                events.extend(d.apply(Op::Tick(addr)));
            }
            PlanStep::ExpectCurrentSuffix(suffix) => {
                let cur = d.state().run.current.clone().expect("current").0;
                assert!(cur.ends_with(suffix), "expected {suffix}, at: {cur}");
            }
        }
    }
    events
}

/// 開始点 `start_id` の節点と同じ `unit` の `main` の節点を呼び出し元とする実体の根（IR の順）と、開始の節点。
pub fn round_roots(ir: &Ir, start_id: &str) -> (Vec<Root>, String) {
    let node_id = ir
        .starts
        .iter()
        .find(|s| s.id == start_id)
        .expect("start")
        .node
        .clone();
    let unit = ir
        .nodes
        .iter()
        .find(|n| n.uid == node_id)
        .expect("start node")
        .unit
        .clone();
    let callers: Vec<&str> = ir
        .nodes
        .iter()
        .filter(|n| n.scope == "main" && n.unit == unit && n.enter.is_some())
        .map(|n| n.uid.as_str())
        .collect();
    let roots = ir
        .instances
        .iter()
        .filter(|i| i.caller.as_deref().is_some_and(|c| callers.contains(&c)))
        .map(|i| Root(i.id.clone()))
        .collect();
    (roots, node_id)
}

pub fn steps(ir: &Ir, roots: &[Root]) -> Vec<StepUid> {
    roots
        .iter()
        .flat_map(|r| fandhe_workshop_core::instance::steps_of(ir, r))
        .collect()
}

/// 周回の根の集まりから、末尾が `/<id>` の Step を引く。
pub fn step_in(ir: &Ir, roots: &[Root], id: &str) -> StepUid {
    let suffix = format!("/{id}");
    steps(ir, roots)
        .into_iter()
        .find(|u| u.0.ends_with(&suffix))
        .unwrap_or_else(|| panic!("step {id}"))
}

fn cmd(c: &str) -> PlanStep {
    PlanStep::Op(Op::Input(Input::Command(c.into())))
}

fn ui(v: &str, p: &str) -> PlanStep {
    PlanStep::Op(Op::Input(Input::Ui {
        view: v.into(),
        path: p.into(),
    }))
}

fn ch(c: &str) -> PlanStep {
    PlanStep::Op(Op::Input(Input::Choice(c.into())))
}

fn text(t: &str) -> PlanStep {
    PlanStep::Op(Op::Input(Input::Text(t.into())))
}

fn ap(d: Decision, t: Option<&str>) -> PlanStep {
    PlanStep::Op(Op::Input(Input::Approval {
        decision: d,
        target: t.map(Into::into),
    }))
}

fn go(ir: &Ir, roots: &[Root], id: &str) -> PlanStep {
    PlanStep::Op(Op::Control(Control::GoTo(step_in(ir, roots, id))))
}

fn begin(id: &str) -> PlanStep {
    PlanStep::Op(Op::Control(Control::Begin(id.into())))
}

const IDEA_TEXT: &str =
    "ログインした人だけが書き込める掲示板で、投稿が増えると目的の投稿を探しにくい";

/// 1 周目の計画（`Begin("s-r1")` から `pt-r1` の入力列まで）。
/// (a) 着地の `on_enter` が未実装で結果の文書が無いので `st-2-6` へ飛ぶ。(b) `when` つき `next` が評価されないので
/// `st-4-1` へ飛ぶ。(c) `wait` の Step で処理中が始まらないので `st-7-3` へ飛ぶ。
pub fn round1_plan(ir: &Ir) -> Vec<PlanStep> {
    let (r1, _) = round_roots(ir, "s-r1");
    vec![
        begin("s-r1"),
        cmd("/new-idea bbs-with-login"),
        ui("v-files", "ideas/bbs-with-login/README.md"),
        text(IDEA_TEXT),
        PlanStep::Tick,
        PlanStep::Tick,
        PlanStep::Tick,
        ch("ans-login-first"),
        ap(Decision::Approve, None),
        ui("v-doc", "ideas/bbs-with-login/README.md#status"),
        ch("cand-session"),
        go(ir, &r1, "st-2-6"),
        ap(Decision::Revise, Some("expect")),
        ap(Decision::Approve, None),
        ui("v-tasks", "TASK-1"),
        ch("ans-login-1"),
        ap(Decision::Approve, None),
        ui("v-doc", "ideas/bbs-with-login/README.md#status"),
        ch("diff-only"),
        ui("v-files", "impl/docs/behavior-bbs-with-login.md"),
        go(ir, &r1, "st-4-1"),
        cmd("/init-claude"),
        cmd("/setup-repo-guards"),
        cmd("/create-issue-tree behavior-bbs-with-login.md"),
        ui("v-issue-tree", "issues/child-1"),
        cmd("/implement-issue-tree 42 --external-checks []"),
        ch("ans-none-explicit"),
        go(ir, &r1, "st-7-3"),
        ui("v-pr-cards", "prs/pr-1-a"),
        cmd("/implement-issue-tree 42"),
        ch("ans-add-arg"),
        ui("v-pr-cards", "prs/pr-1-a#merge"),
        ui("v-issue-tree", "issues/child-1"),
    ]
}

/// 2 周目の開始（`Control::Begin`）だけの計画。
pub fn begin_round2_plan() -> Vec<PlanStep> {
    vec![begin("s-r2")]
}

/// 2 周目以降の周回の、開始の後の計画の本体（TASK-245）。2 周目と 3 周目は同じ構成（`setup: "false"`・`branch_demo: "false"`）で、
/// 違うのは開始点・題材・根・子・PR の引数だけなので、列を 1 つにして引数で変える（二重定義をしない）。
/// 飛ばすのは `st-2-6`・`st-7-4` の 2 か所。
fn later_round_plan(
    ir: &Ir,
    start_id: &str,
    idea: &str,
    root: &str,
    child: &str,
    pr: &str,
) -> Vec<PlanStep> {
    let (rs, _) = round_roots(ir, start_id);
    let readme = format!("ideas/{idea}/README.md");
    let status = format!("{readme}#status");
    let behavior = format!("behavior-{idea}.md");
    let child_path = format!("issues/child-{child}");
    let pr_path = format!("prs/{pr}-a");
    let pr_merge = format!("{pr_path}#merge");
    vec![
        cmd(&format!("/new-idea {idea}")),
        ui("v-files", &readme),
        text(IDEA_TEXT),
        PlanStep::Tick,
        PlanStep::Tick,
        PlanStep::Tick,
        ch("ans-login-first"),
        ap(Decision::Approve, None),
        ui("v-doc", &status),
        ch("cand-session"),
        go(ir, &rs, "st-2-6"),
        ap(Decision::Revise, Some("expect")),
        ap(Decision::Approve, None),
        ui("v-tasks", "TASK-1"),
        ch("ans-login-1"),
        ap(Decision::Approve, None),
        ui("v-doc", &status),
        PlanStep::ExpectCurrentSuffix("/st-3-2"),
        ch("diff-only"),
        ui("v-files", &format!("impl/docs/{behavior}")),
        cmd(&format!("/create-issue-tree {behavior}")),
        ui("v-issue-tree", &child_path),
        cmd(&format!(
            "/implement-issue-tree {root} --external-checks []"
        )),
        ch("ans-none-explicit"),
        go(ir, &rs, "st-7-4"),
        ui("v-pr-cards", &pr_path),
        ui("v-pr-cards", &pr_merge),
        ui("v-issue-tree", &child_path),
    ]
}

/// 2 周目の開始の後の計画。入力列は `pt-rounds` の `s-r2` の区間。飛ばすのは `st-2-6`・`st-7-4` の 2 か所。
pub fn round2_plan(ir: &Ir) -> Vec<PlanStep> {
    later_round_plan(ir, "s-r2", "bbs-search", "43", "9", "pr-2")
}

/// 3 周目の開始（`Control::Begin`）だけの計画（TASK-245。MOD-12）。
pub fn begin_round3_plan() -> Vec<PlanStep> {
    vec![begin("s-r3")]
}

/// 3 周目の開始の後の計画（TASK-245）。2 周目と同じ入力列で、引数だけが `ch-round3` のもの。飛ばすのは同じ 2 か所。
pub fn round3_plan(ir: &Ir) -> Vec<PlanStep> {
    later_round_plan(ir, "s-r3", "bbs-notify", "44", "12", "pr-3")
}

/// 2 周目の計画を、`prefix` で始まる最初のコマンドの直前で切った前半（TASK-238。MOD-8）。
/// 末尾の `ExpectCurrentSuffix` で `expect_suffix` の Step に居ることを確かめる。入力列を `round2_plan` と重複させないため、
/// 元の計画から切り出す。
pub fn round2_plan_until_command(
    ir: &Ir,
    prefix: &str,
    expect_suffix: &'static str,
) -> Vec<PlanStep> {
    let plan = round2_plan(ir);
    let at = plan
        .iter()
        .position(
            |s| matches!(s, PlanStep::Op(Op::Input(Input::Command(c))) if c.starts_with(prefix)),
        )
        .unwrap_or_else(|| panic!("{prefix} in round 2 plan"));
    let mut head = plan[..at].to_vec();
    head.push(PlanStep::ExpectCurrentSuffix(expect_suffix));
    head
}

/// 2 周目の計画を、`st-6-1` の `/create-issue-tree` の直前で切った前半（TASK-238 1/2。MOD-8）。
pub fn round2_plan_until_create_tree(ir: &Ir) -> Vec<PlanStep> {
    round2_plan_until_command(ir, "/create-issue-tree", "/st-6-1")
}

/// 条件 A の計画: 1 周目 → 保存 → 2 周目の開始 → 2 周目 → 保存（ID は `rounds-plan-a-v1`）。
/// 履歴の大きさの測定（pack）と右のログの項目数の測定（web）が同じ操作列を測るための共有の定義。
pub fn plan_a(ir: &Ir) -> Vec<PlanStep> {
    let save = || PlanStep::Op(Op::Control(Control::Save));
    let mut p = round1_plan(ir);
    p.push(save());
    p.extend(begin_round2_plan());
    p.extend(round2_plan(ir));
    p.push(save());
    p
}
