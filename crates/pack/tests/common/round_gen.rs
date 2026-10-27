//! 周回をまたぐ操作列の生成と、混ざり 0 件（M-1）の検査の部品（TASK-243 1/2。MOD-11）。
//!
//! 呼び出し元は `round_property.rs`。定義の出所は `testdata/module-reuse-rounds/test-data/property-test.md`
//! （開始の型 T0〜T3・操作の重み・誘導の表・T(op)・数え方）である。core の `common/op_gen.rs` と同じく、生成の段階では
//! 状態を見ない抽象の [`OpSpec`] だけを作り、[`Harness`] が操作の直前の状態から具体の [`ROp`] に解決する。
//! 2 周の IR は pack の `compile_dir` でしか作れず core から pack へは依存できないため、置き場は pack の試験である。
//!
//! 検査は性質ごとの関数（[`check_m1`]）に分け、2/2（M-2・移動の出発側・S-1）が同じハーネスへ足せる形にする。
//! 下書き・送信の本文は利用者の入力に当たるため、[`OpSpec`] と [`ROp`] の `Debug` は値を出さず、表の添字と種類だけを出す。
//!
//! 未実装の部品（着地の `on_enter`、`when` つき `next`、`wait` の処理中の開始。`docs/testing.md` 5.258・5.260）で入力だけでは
//! 先へ進めない箇所は、誘導が `rounds.rs` の計画と同じ `GoTo` で飛ばす（統計 `guided_goto_substitutions`）。

#![allow(
    dead_code,
    reason = "2/2 が使う部品を含み、試験ファイルごとに使う部品が異なるため"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::OnceLock;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, Stamp, StepUid, TickAddr};
use fandhe_workshop_core::event::{Event, RefuseReason};
use fandhe_workshop_core::input::{CheckpointId, Control, Decision, Input, Op};
use fandhe_workshop_core::instance::{instance_roots, owner_of, root_of, steps_of};
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use proptest::prelude::*;

use super::property::op_sequence;
use super::rounds::round_roots;
use super::testdata;

/// コマンド接頭辞（`submit_drafts.rs` と同じ）。
const PREFIX: &str = "/";

/// 開始の型（`property-test.md`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setup {
    /// 1 周目の最初から。
    T0,
    /// 1 周目を流した後に 2 周目の開始。
    T1,
    /// 1 周目をせず 2 周目から（`s-r2-fresh`）。
    T2,
    /// 実体 `issue-to-merge` の 2 周目の途中開始（`#s-6-1`）。
    T3,
}

impl Setup {
    fn stat_key(self) -> &'static str {
        match self {
            Setup::T0 => "setup_t0",
            Setup::T1 => "setup_t1",
            Setup::T2 => "setup_t2",
            Setup::T3 => "setup_t3",
        }
    }
    fn index(self) -> usize {
        self as usize
    }
}

/// 抽象の操作。添字は表の長さで割った余りで使う。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OpSpec {
    /// 現在の状態での正しい次の操作。
    Guided,
    /// 学習者の入力（外れを含む）の表の添字。
    Input(usize),
    /// 進行。種類（0 現在・1 直前と同じ・2 過去の宛先・3 他の Step）と添字。
    Tick(u8, usize),
    Stop,
    GoTo(usize),
    Save,
    Restore(usize),
    Hint,
    SetDraft(usize),
    Submit,
    Begin(usize),
}

impl fmt::Debug for OpSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpSpec::Guided => write!(f, "Guided"),
            OpSpec::Input(i) => write!(f, "Input({i})"),
            OpSpec::Tick(k, i) => write!(f, "Tick({k},{i})"),
            OpSpec::Stop => write!(f, "Stop"),
            OpSpec::GoTo(i) => write!(f, "GoTo({i})"),
            OpSpec::Save => write!(f, "Save"),
            OpSpec::Restore(i) => write!(f, "Restore({i})"),
            OpSpec::Hint => write!(f, "Hint"),
            OpSpec::SetDraft(i) => write!(f, "SetDraft({i})"),
            OpSpec::Submit => write!(f, "Submit"),
            OpSpec::Begin(i) => write!(f, "Begin({i})"),
        }
    }
}

/// 解決した具体の操作。流し直し（S-8）が同じ列を使う。
#[derive(Clone, PartialEq, Eq)]
pub enum ROp {
    Engine(Op),
    Draft(String),
    Submit(String),
}

impl fmt::Debug for ROp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ROp::Engine(op) => write!(f, "{op:?}"),
            ROp::Draft(_) => write!(f, "Draft(..)"),
            ROp::Submit(_) => write!(f, "Submit(..)"),
        }
    }
}

/// 1 ケース（開始の型と操作の列）。
#[derive(Debug, Clone)]
pub struct Case {
    pub setup: Setup,
    pub specs: Vec<OpSpec>,
}

/// 操作の重み（固定の生成）。誘導つきの生成では `Guided` の重みだけを変える。
const OTHER_WEIGHTS: [u32; 10] = [4, 4, 2, 3, 2, 3, 2, 2, 1, 1];

fn pick_spec(r: u32, n: usize, guided_weight: u32) -> OpSpec {
    let mut acc = guided_weight;
    if r < acc {
        return OpSpec::Guided;
    }
    for (i, w) in OTHER_WEIGHTS.iter().enumerate() {
        acc += w;
        if r < acc {
            return match i {
                0 => OpSpec::Input(n),
                1 => OpSpec::Tick((n % 4) as u8, n / 4),
                2 => OpSpec::Stop,
                3 => OpSpec::GoTo(n),
                4 => OpSpec::Save,
                5 => OpSpec::Restore(n),
                6 => OpSpec::Hint,
                7 => OpSpec::SetDraft(n),
                8 => OpSpec::Submit,
                _ => OpSpec::Begin(n),
            };
        }
    }
    OpSpec::Guided
}

fn spec_strategy(guided_weight: u32) -> impl Strategy<Value = OpSpec> {
    let total = guided_weight + OTHER_WEIGHTS.iter().sum::<u32>();
    (0..total, 0usize..4096).prop_map(move |(r, n)| pick_spec(r, n, guided_weight))
}

fn setup_strategy(weights: [u32; 4]) -> impl Strategy<Value = Setup> {
    let total: u32 = weights.iter().sum();
    (0..total).prop_map(move |mut r| {
        for (i, w) in weights.iter().enumerate() {
            if r < *w {
                return [Setup::T0, Setup::T1, Setup::T2, Setup::T3][i];
            }
            r -= w;
        }
        Setup::T0
    })
}

/// 固定の生成（型の重み 2:3:1:1。`Guided` は 6/30）。
pub fn fixed_cases() -> impl Strategy<Value = Case> {
    (setup_strategy([2, 3, 1, 1]), op_sequence(spec_strategy(6)))
        .prop_map(|(setup, specs)| Case { setup, specs })
}

/// 誘導つきの生成（型の重み 1:6:1:1。操作の半分以上が `Guided`）。
pub fn guided_cases() -> impl Strategy<Value = Case> {
    (setup_strategy([1, 6, 1, 1]), op_sequence(spec_strategy(30)))
        .prop_map(|(setup, specs)| Case { setup, specs })
}

/// 試験で共有する IR・周回の根・開始の型ごとの開始状態（1 回だけ作る）。
pub struct Fixture {
    pub ir: Ir,
    pub r1: BTreeSet<Root>,
    pub r2: BTreeSet<Root>,
    starts: [RecordedSession; 4],
    inputs: Vec<Input>,
}

const IDEA_TEXT: &str =
    "ログインした人だけが書き込める掲示板で、投稿が増えると目的の投稿を探しにくい";
const DRAFTS: [&str; 5] = [
    "下書き A",
    "/new-idea bbs-search",
    "x",
    "ログイン",
    "/init-claude",
];

fn begin(rs: &mut RecordedSession, id: &str) {
    rs.apply(Op::Control(Control::Begin(id.into())));
}

fn input_table() -> Vec<Input> {
    let cmd = |c: &str| Input::Command(c.into());
    let ui = |v: &str, p: &str| Input::Ui {
        view: v.into(),
        path: p.into(),
    };
    let ch = |c: &str| Input::Choice(c.into());
    let ap = |d, t: Option<&str>| Input::Approval {
        decision: d,
        target: t.map(Into::into),
    };
    vec![
        cmd("/new-idea bbs-with-login"),
        cmd("/new-idea bbs-search"),
        cmd("/new-idea"),
        cmd("/new-idea Bad_Name"),
        cmd("/init-claude"),
        cmd("/setup-repo-guards"),
        cmd("/create-issue-tree behavior-bbs-with-login.md"),
        cmd("/create-issue-tree behavior-bbs-search.md"),
        cmd("/implement-issue-tree 42"),
        cmd("/implement-issue-tree 43 --external-checks []"),
        cmd("/nothing"),
        cmd("/help"),
        ch("ans-login-first"),
        ch("cand-session"),
        ch("go"),
        ch("diff-only"),
        ch("ans-login-1"),
        ch("ans-none-explicit"),
        ch("ans-add-arg"),
        ch("opt-none"),
        ui("v-files", "ideas/bbs-with-login/README.md"),
        ui("v-files", "ideas/bbs-search/README.md"),
        ui("v-doc", "ideas/bbs-with-login/README.md#status"),
        ui("v-doc", "ideas/bbs-search/README.md#status"),
        ui("v-tasks", "TASK-1"),
        ui("v-issue-tree", "issues/child-1"),
        ui("v-issue-tree", "issues/child-9"),
        ui("v-pr-cards", "prs/pr-1-a"),
        ui("v-pr-cards", "prs/pr-2-a#merge"),
        ui("v-files", "impl/docs/behavior-bbs-search.md"),
        ap(Decision::Approve, None),
        ap(Decision::Reject, None),
        ap(Decision::Revise, Some("expect")),
        ap(Decision::Revise, None),
        Input::Text(IDEA_TEXT.into()),
        Input::Text("短い".into()),
        Input::Cancel,
    ]
}

/// 共有の試験環境。
pub fn fixture() -> &'static Fixture {
    static F: OnceLock<Fixture> = OnceLock::new();
    F.get_or_init(|| {
        let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
        let ir = load_ir(&bytes).expect("load_ir");
        let (r1v, _) = round_roots(&ir, "s-r1");
        let (r2v, _) = round_roots(&ir, "s-r2");
        let fresh =
            || RecordedSession::new(Session::new(ir.clone()), RecordedSession::first_start(&ir));
        let mut t0 = fresh();
        begin(&mut t0, "s-r1");
        let mut t1 = fresh();
        super::rounds::run_plan(&mut t1, &super::rounds::round1_plan(&ir));
        super::rounds::run_plan(&mut t1, &super::rounds::begin_round2_plan());
        let mut t2 = fresh();
        begin(&mut t2, "s-r2-fresh");
        let t3_id = ir
            .starts
            .iter()
            .find(|s| {
                s.id.strip_suffix("#s-6-1")
                    .is_some_and(|inst| r2v.contains(&Root(inst.to_string())))
            })
            .expect("round 2 module start")
            .id
            .clone();
        let mut t3 = fresh();
        begin(&mut t3, &t3_id);
        for t in [&t0, &t1, &t2, &t3] {
            assert!(t.session().state().run.current.is_some(), "setup began");
        }
        Fixture {
            r1: r1v.into_iter().collect(),
            r2: r2v.into_iter().collect(),
            starts: [t0, t1, t2, t3],
            inputs: input_table(),
            ir,
        }
    })
}

impl Fixture {
    /// 0 は章・その他、1 は 1 周目、2 は 2 周目。
    pub fn round_of_root(&self, r: &Root) -> u8 {
        if self.r1.contains(r) {
            1
        } else if self.r2.contains(r) {
            2
        } else {
            0
        }
    }
    fn round_of_step(&self, uid: &StepUid) -> u8 {
        self.round_of_root(&owner_of(&self.ir, uid))
    }
}

/// 数えた件数（出力するだけで、性質の成功条件にしない）。
#[derive(Debug, Default, Clone)]
pub struct Stats(pub BTreeMap<&'static str, u64>);

impl Stats {
    pub fn add(&mut self, k: &'static str, n: u64) {
        *self.0.entry(k).or_insert(0) += n;
    }
    pub fn merge(&mut self, o: &Stats) {
        for (k, v) in &o.0 {
            if *k == "max_ops" {
                let e = self.0.entry(k).or_insert(0);
                *e = (*e).max(*v);
            } else {
                self.add(k, *v);
            }
        }
    }
    pub fn get(&self, k: &str) -> u64 {
        self.0.get(k).copied().unwrap_or(0)
    }
}

/// 根の切り身の見方（比較用）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct View {
    status: BTreeMap<StepUid, StepStatus>,
    choices: BTreeMap<StepUid, String>,
    flags: BTreeSet<String>,
    clarifying: Option<String>,
    hints: BTreeMap<StepUid, u32>,
    draft: Option<String>,
}

fn view(ir: &Ir, st: &SessionState, root: &Root) -> View {
    let s = st.run.slices.get(root).cloned().unwrap_or_default();
    View {
        status: s.status,
        choices: s.choices,
        flags: s.flags,
        clarifying: s.clarifying,
        hints: st
            .hints_opened
            .iter()
            .filter(|(u, _)| &owner_of(ir, u) == root)
            .map(|(u, n)| (u.clone(), *n))
            .collect(),
        draft: st.drafts.get(root).cloned(),
    }
}

/// 違った部分の名前（無ければ `None`）。
fn diff_part(a: &View, b: &View) -> Option<&'static str> {
    if a.status != b.status {
        Some("status")
    } else if a.choices != b.choices {
        Some("choices")
    } else if a.flags != b.flags {
        Some("flags")
    } else if a.clarifying != b.clarifying {
        Some("clarifying")
    } else if a.hints != b.hints {
        Some("hints")
    } else if a.draft != b.draft {
        Some("draft")
    } else {
        None
    }
}

fn stamp_of(e: &Event) -> Option<&Stamp> {
    Some(match e {
        Event::Entered { at }
        | Event::ModuleEntered { at, .. }
        | Event::ModuleExited { at, .. }
        | Event::InputReceived { at, .. }
        | Event::Progress { at, .. }
        | Event::Judged { at, .. }
        | Event::Accepted { at }
        | Event::Achieved { at, .. }
        | Event::Skipped { at }
        | Event::Transitioned { at, .. }
        | Event::ChapterCompleted { at }
        | Event::Flag { at, .. }
        | Event::Changed { at, .. }
        | Event::Stopped { at, .. }
        | Event::Moved { at, .. }
        | Event::CheckpointSaved { at, .. }
        | Event::Restored { at, .. }
        | Event::Begun { at, .. }
        | Event::Reloaded { at, .. }
        | Event::HintOpened { at, .. }
        | Event::Refused { at, .. }
        | Event::Ignored { at, .. }
        | Event::TransitionLimitReached { at, .. }
        | Event::Warning { at, .. } => at,
        // 非網羅の列挙のため。新しい種類は宛先が分からず T(op) に入らない（偽の違反が出れば気づける）。
        _ => return None,
    })
}

fn add_uid(set: &mut BTreeSet<Root>, ir: &Ir, uid: &StepUid) {
    // 下書き・フラグの鍵は `root_of`、状態・選択の持ち主は `owner_of`。偽の違反を避けるため両方を入れる。
    set.insert(owner_of(ir, uid));
    set.insert(root_of(uid));
}

/// 操作の宛先の根の集合 T(op)（`property-test.md`）。
pub fn target_roots(
    ir: &Ir,
    before: &SessionState,
    after: &SessionState,
    op: &ROp,
    events: &[Event],
) -> BTreeSet<Root> {
    let mut t = BTreeSet::new();
    for e in events {
        if let Some(at) = stamp_of(e) {
            add_uid(&mut t, ir, &at.step);
        }
        match e {
            Event::Transitioned { to, .. } | Event::Moved { to, .. } => add_uid(&mut t, ir, to),
            Event::Restored { owner, .. } => add_uid(&mut t, ir, owner),
            Event::ModuleEntered { instance, .. } | Event::ModuleExited { instance, .. } => {
                t.insert(Root(instance.clone()));
            }
            Event::Flag { scope, .. } => {
                t.insert(root_of(&StepUid(scope.clone())));
                t.insert(Root(scope.clone()));
            }
            _ => {}
        }
    }
    if let ROp::Engine(Op::Control(c)) = op {
        match c {
            Control::GoTo(uid) => add_uid(&mut t, ir, uid),
            Control::Restore(id) => {
                if let Some(owner) = before.checkpoints.get(id).and_then(|c| c.owner.clone()) {
                    t.insert(owner);
                }
            }
            Control::Begin(id) => {
                if let Some(s) = ir.starts.iter().find(|s| &s.id == id) {
                    add_uid(&mut t, ir, &StepUid(s.node.clone()));
                }
            }
            _ => {}
        }
    }
    for st in [before, after] {
        if let Some(cur) = &st.run.current {
            add_uid(&mut t, ir, cur);
        }
    }
    t
}

/// M-1: T(op) に入らないすべての根で、操作の前後の切り身が一致する。違反は（根, 違った部分）。
pub fn check_m1(
    ir: &Ir,
    before: &SessionState,
    after: &SessionState,
    targets: &BTreeSet<Root>,
) -> Result<(), (Root, &'static str)> {
    let mut roots: BTreeSet<Root> = instance_roots(ir).into_iter().collect();
    for st in [before, after] {
        roots.extend(st.run.slices.keys().cloned());
        roots.extend(st.drafts.keys().cloned());
        roots.extend(st.hints_opened.keys().map(|u| owner_of(ir, u)));
    }
    for r in roots.iter().filter(|r| !targets.contains(*r)) {
        if let Some(part) = diff_part(&view(ir, before, r), &view(ir, after, r)) {
            return Err((r.clone(), part));
        }
    }
    Ok(())
}

fn kind_name(op: &ROp) -> String {
    match op {
        ROp::Engine(Op::Input(_)) => "input".into(),
        ROp::Engine(Op::Tick(_)) => "tick".into(),
        ROp::Engine(Op::Control(c)) => format!("{c:?}"),
        ROp::Engine(_) => "engine".into(),
        ROp::Draft(_) => "draft".into(),
        ROp::Submit(_) => "submit".into(),
    }
}

/// 操作を解決して流し、各操作の後に M-1 を検査するハーネス。
pub struct Harness {
    f: &'static Fixture,
    setup: Setup,
    pub rs: RecordedSession,
    seen: Vec<TickAddr>,
    last_tick: Option<TickAddr>,
    pub ops: Vec<ROp>,
    pub stats: Stats,
    saw: [bool; 3],
}

impl Harness {
    pub fn new(setup: Setup) -> Self {
        let f = fixture();
        let mut h = Harness {
            f,
            setup,
            rs: f.starts[setup.index()].clone(),
            seen: Vec::new(),
            last_tick: None,
            ops: Vec::new(),
            stats: Stats::default(),
            saw: [false; 3],
        };
        h.note_position();
        h
    }

    pub fn state(&self) -> &SessionState {
        self.rs.session().state()
    }

    fn note_position(&mut self) {
        if let Some(cur) = &self.state().run.current {
            let r = self.f.round_of_step(cur);
            self.saw[r as usize] = true;
        }
    }

    fn current_tick(&self) -> TickAddr {
        let st = self.state();
        let step = st.run.current.clone().unwrap_or_else(|| {
            StepUid(
                self.f
                    .ir
                    .nodes
                    .first()
                    .map(|n| n.uid.clone())
                    .unwrap_or_default(),
            )
        });
        TickAddr {
            generation: st.run.generation,
            step,
            index: st.run.busy.as_ref().map_or(0, |b| b.done),
        }
    }

    fn tick_addr(&self, kind: u8, n: usize) -> TickAddr {
        let cur = self.current_tick();
        match kind {
            0 => cur,
            1 => self.last_tick.clone().unwrap_or(cur),
            2 if !self.seen.is_empty() => self.seen[n % self.seen.len()].clone(),
            3 => {
                let nodes = &self.f.ir.nodes;
                TickAddr {
                    step: StepUid(nodes[n % nodes.len()].uid.clone()),
                    ..cur
                }
            }
            _ => TickAddr {
                generation: fandhe_workshop_core::addr::Generation(
                    cur.generation.0.saturating_sub(1),
                ),
                ..cur
            },
        }
    }

    fn args_of(&self, root: &Root, key: &str, default: &str) -> String {
        self.f
            .ir
            .instances
            .iter()
            .find(|i| i.id == root.0)
            .and_then(|i| i.args.get(key).cloned())
            .unwrap_or_else(|| default.to_string())
    }

    /// 同じ根で、候補のうち IR に在る最初の Step への `GoTo`（未実装の部品の代用）。
    fn goto_first(&mut self, root: &Root, ids: &[&str]) -> Option<ROp> {
        let steps = steps_of(&self.f.ir, root);
        for id in ids {
            let suffix = format!("/{id}");
            if let Some(u) = steps.iter().find(|u| u.0.ends_with(&suffix)) {
                self.stats.add("guided_goto_substitutions", 1);
                return Some(ROp::Engine(Op::Control(Control::GoTo(u.clone()))));
            }
        }
        None
    }

    /// 誘導の表（`property-test.md`）。表に無い状態は取り消しに倒す。
    fn guided(&mut self) -> ROp {
        let cancel = ROp::Engine(Op::Input(Input::Cancel));
        let st = self.state().clone();
        if st.run.busy.is_some() {
            return ROp::Engine(Op::Tick(self.current_tick()));
        }
        let Some(cur) = st.run.current.clone() else {
            return ROp::Engine(Op::Control(Control::Begin("s-r1".into())));
        };
        let root = root_of(&cur);
        let id = cur.0.rsplit('/').next().unwrap_or("").to_string();
        let status = self.rs.session().step_status(&cur);
        let owner = owner_of(&self.f.ir, &cur);
        if matches!(status, StepStatus::Achieved | StepStatus::Skipped)
            && (st.run.completed || self.f.round_of_root(&owner) == 0)
        {
            return ROp::Engine(Op::Control(Control::Begin("s-r2".into())));
        }
        let after = status == StepStatus::Accepted;
        let idea = self.args_of(&root, "idea", "bbs-search");
        let child = self.args_of(&root, "child", "1");
        let pr = self.args_of(&root, "pr", "pr-1");
        let rootarg = self.args_of(&root, "root", "42");
        let cmd = |c: String| Some(Input::Command(c));
        let ui = |v: &str, p: String| {
            Some(Input::Ui {
                view: v.into(),
                path: p,
            })
        };
        let ch = |c: &str| Some(Input::Choice(c.into()));
        let ap = |d, t: Option<&str>| {
            Some(Input::Approval {
                decision: d,
                target: t.map(Into::into),
            })
        };
        let input = match (id.as_str(), after) {
            ("st-2-1", false) => cmd(format!("/new-idea {idea}")),
            ("st-2-1", true) => ui("v-files", format!("ideas/{idea}/README.md")),
            ("st-2-2", false) => Some(Input::Text(IDEA_TEXT.into())),
            ("st-2-2", true) => ch("ans-login-first"),
            ("st-2-3", false) => ap(Decision::Approve, None),
            ("st-2-3", true) => ui("v-doc", format!("ideas/{idea}/README.md#status")),
            ("st-2-4", false) => ch("cand-session"),
            ("st-2-4", true) | ("st-2-5", _) => {
                return self.goto_first(&root, &["st-2-6"]).unwrap_or(cancel);
            }
            ("st-2-6", false) => ap(Decision::Revise, Some("expect")),
            ("st-2-6", true) => ap(Decision::Approve, None),
            ("st-2-7", false) => ui("v-tasks", "TASK-1".into()),
            ("st-2-7", true) => ch("ans-login-1"),
            ("st-2-8", false) => ap(Decision::Approve, None),
            ("st-2-8", true) => ui("v-doc", format!("ideas/{idea}/README.md#status")),
            ("st-3-2", false) => ch("diff-only"),
            // 1 周目だけ、`when` つき `next` が評価されないので `round1_plan` と同じ `st-4-1` へ飛ぶ。
            ("st-3-2", true) if self.f.round_of_root(&owner) == 1 => {
                return self.goto_first(&root, &["st-4-1"]).unwrap_or(cancel);
            }
            ("st-3-2", true) => ui("v-files", format!("impl/docs/behavior-{idea}.md")),
            ("st-4-1", false) => cmd("/init-claude".into()),
            ("st-5-1", false) => cmd("/setup-repo-guards".into()),
            ("st-6-1", false) => cmd(format!("/create-issue-tree behavior-{idea}.md")),
            ("st-6-1", true) => ui("v-issue-tree", format!("issues/child-{child}")),
            ("st-7-1", false) => cmd(format!(
                "/implement-issue-tree {rootarg} --external-checks []"
            )),
            ("st-7-1", true) => ch("ans-none-explicit"),
            ("st-7-2", _) => {
                // `branch_demo` が真なら `st-7-3`（1 周目）、偽なら `st-7-4`（2 周目）。計画と同じ飛び先。
                let dest = if self.args_of(&root, "branch_demo", "true") == "true" {
                    "st-7-3"
                } else {
                    "st-7-4"
                };
                return self.goto_first(&root, &[dest]).unwrap_or(cancel);
            }
            ("st-7-3", false) => cmd(format!("/implement-issue-tree {rootarg}")),
            ("st-7-3", true) => ch("ans-add-arg"),
            ("st-7-4", false) => ui("v-pr-cards", format!("prs/{pr}-a#merge")),
            ("st-7-4", true) => ui("v-issue-tree", format!("issues/child-{child}")),
            _ => None,
        };
        match input {
            Some(i) => ROp::Engine(Op::Input(i)),
            None => {
                self.stats.add("guided_fallback", 1);
                cancel
            }
        }
    }

    fn resolve(&mut self, spec: &OpSpec) -> ROp {
        let st = self.state().clone();
        match *spec {
            OpSpec::Guided => self.guided(),
            OpSpec::Input(i) => {
                ROp::Engine(Op::Input(self.f.inputs[i % self.f.inputs.len()].clone()))
            }
            OpSpec::Tick(k, n) => ROp::Engine(Op::Tick(self.tick_addr(k, n))),
            OpSpec::Stop => ROp::Engine(Op::Control(Control::Stop)),
            OpSpec::GoTo(i) => {
                let nodes = &self.f.ir.nodes;
                ROp::Engine(Op::Control(Control::GoTo(StepUid(
                    nodes[i % nodes.len()].uid.clone(),
                ))))
            }
            OpSpec::Save => ROp::Engine(Op::Control(Control::Save)),
            OpSpec::Restore(i) => {
                let mut ids: Vec<CheckpointId> = st.checkpoints.keys().copied().collect();
                ids.push(CheckpointId(u32::MAX));
                ROp::Engine(Op::Control(Control::Restore(ids[i % ids.len()])))
            }
            OpSpec::Hint => ROp::Engine(Op::Control(Control::OpenHint)),
            OpSpec::SetDraft(i) => ROp::Draft(DRAFTS[i % DRAFTS.len()].to_string()),
            OpSpec::Submit => ROp::Submit(
                self.rs
                    .current_draft()
                    .map_or_else(|| DRAFTS[1].to_string(), str::to_string),
            ),
            OpSpec::Begin(i) => {
                let starts = &self.f.ir.starts;
                let n = starts.len() + 1;
                let id = starts
                    .get(i % n)
                    .map_or_else(|| "no-such-start".to_string(), |s| s.id.clone());
                ROp::Engine(Op::Control(Control::Begin(id)))
            }
        }
    }

    /// 1 操作を解決して流し、M-1 を検査する。違反は理由（値を含まない）を返す。
    pub fn step(&mut self, index: usize, spec: &OpSpec) -> Result<(), String> {
        let op = self.resolve(spec);
        if let ROp::Engine(Op::Tick(a)) = &op {
            if !self.seen.contains(a) {
                self.seen.push(a.clone());
            }
            self.last_tick = Some(a.clone());
        }
        let before = self.state().clone();
        let events: Vec<Event> = match &op {
            ROp::Engine(o) => self.rs.apply(o.clone()).to_vec(),
            ROp::Draft(t) => {
                self.rs.draft(t.clone());
                Vec::new()
            }
            ROp::Submit(t) => self.rs.submit(PREFIX, t.clone()).to_vec(),
        };
        let after = self.state().clone();
        self.ops.push(op.clone());
        let targets = target_roots(&self.f.ir, &before, &after, &op, &events);
        self.count(&before, &after, &op, &events, &targets);
        self.note_position();
        check_m1(&self.f.ir, &before, &after, &targets).map_err(|(root, part)| {
            format!(
                "M-1 violated at op {index} ({spec:?} -> {}): root {root:?} part {part}; targets {targets:?}",
                kind_name(&op)
            )
        })
    }

    fn count(
        &mut self,
        before: &SessionState,
        after: &SessionState,
        op: &ROp,
        events: &[Event],
        targets: &BTreeSet<Root>,
    ) {
        let f = self.f;
        self.stats.add("ops", 1);
        if targets.iter().any(|r| f.r1.contains(r)) && targets.iter().any(|r| f.r2.contains(r)) {
            self.stats.add("ops_touching_both", 1);
        }
        let cur_round = before
            .run
            .current
            .as_ref()
            .map_or(0, |u| f.round_of_step(u));
        for e in events {
            match e {
                Event::Moved { to, .. } => {
                    let to_round = f.round_of_step(to);
                    if cur_round != 0 && to_round != 0 && cur_round != to_round {
                        self.stats.add("gotos_cross_round", 1);
                    }
                }
                Event::Restored { id, .. } => {
                    self.stats.add("restores_ok", 1);
                    let owner_round = before
                        .checkpoints
                        .get(id)
                        .and_then(|c| c.owner.as_ref())
                        .map_or(0, |o| f.round_of_root(o));
                    if cur_round != 0 && owner_round != 0 && cur_round != owner_round {
                        self.stats.add("restores_cross_round", 1);
                    }
                }
                Event::Stopped { .. } => self.stats.add("stops_effective", 1),
                Event::HintOpened { at, .. } => match f.round_of_step(&at.step) {
                    1 => self.stats.add("hints_r1", 1),
                    2 => self.stats.add("hints_r2", 1),
                    _ => {}
                },
                Event::Refused { reason, .. } => match reason {
                    RefuseReason::NoHint => self.stats.add("hints_refused", 1),
                    RefuseReason::StartUnmet => self.stats.add("begin_refused_unmet", 1),
                    RefuseReason::UnknownStart => self.stats.add("begin_refused_unknown", 1),
                    _ => {}
                },
                Event::Begun { .. } => self.stats.add("begin_ok", 1),
                _ => {}
            }
        }
        let has_draft = |rs: &BTreeSet<Root>| {
            after
                .drafts
                .iter()
                .any(|(r, t)| rs.contains(r) && !t.is_empty())
        };
        if has_draft(&f.r1) {
            self.stats.add("ops_with_r1_draft", 1);
        }
        if has_draft(&f.r2) {
            self.stats.add("ops_with_r2_draft", 1);
        }
        if matches!(op, ROp::Submit(_)) {
            self.stats.add("submits", 1);
        }
    }

    /// ケースの終わりの集計。
    pub fn finish_stats(&mut self) -> Stats {
        let mut s = self.stats.clone();
        s.add("cases", 1);
        s.add(self.setup.stat_key(), 1);
        s.add("max_ops", self.ops.len() as u64);
        if self.saw[1] && self.saw[2] {
            s.add("cases_both_rounds", 1);
        }
        s
    }

    /// S-8 相当: 同じ開始の型から保持した操作の列を流し直し、状態が一致する。
    pub fn replay_matches(&self) -> bool {
        let mut rs = self.f.starts[self.setup.index()].clone();
        for op in &self.ops {
            match op {
                ROp::Engine(o) => {
                    rs.apply(o.clone());
                }
                ROp::Draft(t) => rs.draft(t.clone()),
                ROp::Submit(t) => {
                    rs.submit(PREFIX, t.clone());
                }
            }
        }
        rs.session().state() == self.state()
    }
}

/// 1 ケースを流して M-1 と流し直しの一致を検査し、統計を返す。
pub fn run_case(case: &Case) -> Result<Stats, String> {
    let mut h = Harness::new(case.setup);
    for (i, spec) in case.specs.iter().enumerate() {
        h.step(i, spec)?;
    }
    if !h.replay_matches() {
        return Err("replay mismatch (S-8)".to_string());
    }
    Ok(h.finish_stats())
}
