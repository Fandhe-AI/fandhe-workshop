//! 周回をまたぐ操作列の生成と、混ざり 0 件（M-1）の検査の部品（TASK-243 1/2。MOD-11）。
//!
//! 呼び出し元は `round_property.rs`。定義の出所は `testdata/module-reuse-rounds/test-data/property-test.md`
//! （開始の型 T0〜T3・操作の重み・誘導の表・T(op)・数え方）である。core の `common/op_gen.rs` と同じく、生成の段階では
//! 状態を見ない抽象の [`OpSpec`] だけを作り、[`Harness`] が操作の直前の状態から具体の [`ROp`] に解決する。
//! 2 周の IR は pack の `compile_dir` でしか作れず core から pack へは依存できないため、置き場は pack の試験である。
//!
//! 検査は性質ごとの関数に分け、[`Harness::step`] が毎操作の後に流す。1/2 は M-1（[`check_m1`]）、2/2（TASK-243。#770）は
//! 1 周目の不変（M-2。[`check_m2`]。[`Mode::Round2Closed`] のときだけ）・移動の出発側の不変（[`check_move_source`]）・
//! 古い進行の無視（S-1。[`Harness::check_stale_tick`]）・同じモジュールの完了状態の非混在（[`check_completion_separation`]）を足した。
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
use fandhe_workshop_core::instance::{instance_roots, owner_of, root_of, status_of, steps_of};
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
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

/// M-2 の固定の生成（開始は T1 だけ。`Guided` の重みは固定の生成と同じ 6/30）。
pub fn m2_fixed_cases() -> impl Strategy<Value = Case> {
    op_sequence(spec_strategy(6)).prop_map(|specs| Case {
        setup: Setup::T1,
        specs,
    })
}

/// M-2 の誘導つきの生成（開始は T1 だけ。`Guided` の重みは 60。2 周目の完了まで届く列を増やす）。
pub fn m2_guided_cases() -> impl Strategy<Value = Case> {
    op_sequence(spec_strategy(60)).prop_map(|specs| Case {
        setup: Setup::T1,
        specs,
    })
}

/// 試験で共有する IR・周回の根・開始の型ごとの開始状態（1 回だけ作る）。
pub struct Fixture {
    pub ir: Ir,
    pub r1: BTreeSet<Root>,
    pub r2: BTreeSet<Root>,
    starts: [RecordedSession; 4],
    inputs: Vec<Input>,
    /// 持ち主の周回が 2 の節点（M-2 の `GoTo` の候補）。
    r2_steps: Vec<StepUid>,
    /// M-2 の `Begin` の候補の開始 ID（2 周目に閉じ、1 周目の世界状態の経路へ書かないもの）。
    m2_starts: Vec<String>,
    /// 同じモジュールの同じ局所 ID を持つ、ほかの実体の Step（完了状態の非混在の検査用）。
    siblings: BTreeMap<StepUid, Vec<StepUid>>,
    /// 1 周目の章の節点の `unit`（M-2 の閉じていることの表明用）。
    r1_unit: String,
    /// T1 の開始状態の 1 周目の切り身と 1 周目の世界状態の 13 経路（M-2 の基点）。
    m2_base_views: Vec<(Root, View)>,
    m2_base_world: Vec<(String, Option<Element>)>,
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
        let (r1v, r1_start_node) = round_roots(&ir, "s-r1");
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
        let r1set: BTreeSet<Root> = r1v.iter().cloned().collect();
        let r2set: BTreeSet<Root> = r2v.iter().cloned().collect();
        let round_of = |u: &str| {
            let o = owner_of(&ir, &StepUid(u.to_string()));
            if r1set.contains(&o) {
                1
            } else if r2set.contains(&o) {
                2
            } else {
                0
            }
        };
        let put_paths = |effects: &[serde_json::Value]| -> Vec<String> {
            effects
                .iter()
                .filter_map(|v| v.get("put").and_then(|p| p.get("path")))
                .filter_map(|p| p.as_str().map(str::to_owned))
                .collect()
        };
        let fresh_start = ir
            .starts
            .iter()
            .find(|s| s.id == "s-r2-fresh")
            .expect("s-r2-fresh");
        let mut r1_world_paths = put_paths(&fresh_start.effects);
        r1_world_paths.sort();
        assert_eq!(r1_world_paths.len(), 13, "{r1_world_paths:?}");
        let r1_unit = ir
            .nodes
            .iter()
            .find(|n| n.uid == r1_start_node)
            .map(|n| n.unit.clone())
            .expect("round 1 unit");
        let r1_callers: BTreeSet<&str> = ir
            .instances
            .iter()
            .filter(|i| r1set.contains(&Root(i.id.clone())))
            .filter_map(|i| i.caller.as_deref())
            .collect();
        let m2_starts: Vec<String> = ir
            .starts
            .iter()
            .filter(|s| {
                round_of(&s.node) != 1
                    && !r1_callers.contains(s.node.as_str())
                    && put_paths(&s.effects)
                        .iter()
                        .all(|p| !r1_world_paths.contains(p))
            })
            .map(|s| s.id.clone())
            .collect();
        assert!(m2_starts.contains(&"s-r2".to_string()), "{m2_starts:?}");
        assert!(!m2_starts.contains(&"s-r2-fresh".to_string()));
        let r2_steps: Vec<StepUid> = ir
            .nodes
            .iter()
            .filter(|n| round_of(&n.uid) == 2)
            .map(|n| StepUid(n.uid.clone()))
            .collect();
        let mut siblings: BTreeMap<StepUid, Vec<StepUid>> = BTreeMap::new();
        for a in &ir.nodes {
            for b in &ir.nodes {
                let (oa, ob) = (
                    owner_of(&ir, &StepUid(a.uid.clone())),
                    owner_of(&ir, &StepUid(b.uid.clone())),
                );
                let module = |o: &Root| {
                    ir.instances
                        .iter()
                        .find(|i| i.id == o.0)
                        .and_then(|i| i.module.clone())
                };
                let local = |u: &str| u.rsplit('/').next().unwrap_or("").to_string();
                if a.uid != b.uid
                    && oa != ob
                    && module(&oa).is_some()
                    && module(&oa) == module(&ob)
                    && local(&a.uid) == local(&b.uid)
                {
                    siblings
                        .entry(StepUid(a.uid.clone()))
                        .or_default()
                        .push(StepUid(b.uid.clone()));
                }
            }
        }
        let base = t1.session().state();
        let m2_base_views = r1v
            .iter()
            .map(|r| (r.clone(), view(&ir, base, r)))
            .collect();
        let m2_base_world = r1_world_paths
            .iter()
            .map(|p| (p.clone(), base.run.world.get(p).cloned()))
            .collect();
        Fixture {
            r1: r1set,
            r2: r2set,
            starts: [t0, t1, t2, t3],
            inputs: input_table(),
            r2_steps,
            m2_starts,
            siblings,
            r1_unit,
            m2_base_views,
            m2_base_world,
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

/// M-2: 毎操作の後に、1 周目の 2 実体の切り身と 1 周目の世界状態の 13 経路が、T1 の開始状態（基点）と一致する。
/// 操作の前ではなく基点と比べるので、毎操作の後で変わらないことを強く表す。違反は全件を返す（空なら合格）。
pub fn check_m2(f: &Fixture, after: &SessionState) -> Vec<String> {
    let mut v = Vec::new();
    for (root, base) in &f.m2_base_views {
        if let Some(part) = diff_part(base, &view(&f.ir, after, root)) {
            v.push(format!("root {root:?} part {part}"));
        }
    }
    for (path, base) in &f.m2_base_world {
        if after.run.world.get(path) != base.as_ref() {
            v.push(format!("world path {path}"));
        }
    }
    v
}

/// 移動の出発側の不変: `Moved` を伴う操作で、着地側の根に入らない出発側の根の切り身が操作の前後で変わらない。
/// 同じ根の中の移動は着地が出発側の根に `Entered` を置き、切り身が正当に変わるので対象外である（`docs/testing.md` 5.334）。
/// 検査したら `Some(周回をまたいだか)`、対象外なら `None`。違反は（根, 違った部分）。
pub fn check_move_source(
    f: &Fixture,
    before: &SessionState,
    after: &SessionState,
    events: &[Event],
) -> Result<Option<bool>, (Root, &'static str)> {
    let Some(cur) = &before.run.current else {
        return Ok(None);
    };
    if !events.iter().any(|e| matches!(e, Event::Moved { .. })) {
        return Ok(None);
    }
    let ir = &f.ir;
    let mut land = BTreeSet::new();
    if let Some(c) = &after.run.current {
        add_uid(&mut land, ir, c);
    }
    for e in events {
        match e {
            Event::Moved { to, .. } => add_uid(&mut land, ir, to),
            Event::Entered { at } => add_uid(&mut land, ir, &at.step),
            Event::ModuleEntered { instance, .. } => {
                land.insert(Root(instance.clone()));
            }
            _ => {}
        }
    }
    let mut src = BTreeSet::new();
    add_uid(&mut src, ir, cur);
    for r in src.iter().filter(|r| !land.contains(*r)) {
        if let Some(part) = diff_part(&view(ir, before, r), &view(ir, after, r)) {
            return Err((r.clone(), part));
        }
    }
    let (a, b) = (
        f.round_of_step(cur),
        after.run.current.as_ref().map_or(0, |u| f.round_of_step(u)),
    );
    Ok(Some(a != 0 && b != 0 && a != b))
}

/// 完了状態の非混在: 操作が `Achieved`・`Skipped`・`Accepted`・`Entered` を起こした Step について、同じモジュールの
/// ほかの実体の同じ局所 ID の Step の状態が、操作の前後で変わらない。その操作の宛先に入る相手と、`Restored` の持ち主の
/// 相手は除く。返すのは（検査した件数, 相手が達成済みだった件数）。違反は理由。
pub fn check_completion_separation(
    f: &Fixture,
    before: &SessionState,
    after: &SessionState,
    events: &[Event],
) -> Result<(u64, u64), String> {
    let ir = &f.ir;
    let mut targets: BTreeSet<&StepUid> = BTreeSet::new();
    let mut restored: BTreeSet<Root> = BTreeSet::new();
    for e in events {
        if let Some(at) = stamp_of(e) {
            targets.insert(&at.step);
        }
        match e {
            Event::Transitioned { to, .. } | Event::Moved { to, .. } => {
                targets.insert(to);
            }
            Event::Restored { owner, .. } => {
                restored.insert(owner_of(ir, owner));
            }
            _ => {}
        }
    }
    let status = |st: &SessionState, u: &StepUid| {
        st.run
            .slices
            .get(&owner_of(ir, u))
            .and_then(|s| s.status.get(u))
            .cloned()
    };
    let (mut checked, mut achieved) = (0, 0);
    for e in events {
        let (Event::Achieved { at, .. }
        | Event::Skipped { at }
        | Event::Accepted { at }
        | Event::Entered { at }) = e
        else {
            continue;
        };
        for sib in f.siblings.get(&at.step).into_iter().flatten() {
            if targets.contains(sib) || restored.contains(&owner_of(ir, sib)) {
                continue;
            }
            checked += 1;
            let was = status(before, sib);
            if was == Some(StepStatus::Achieved) {
                achieved += 1;
            }
            if was != status(after, sib) {
                return Err(format!(
                    "sibling of {:?} changed: {sib:?}",
                    at.step.0.rsplit('/').next().unwrap_or("")
                ));
            }
        }
    }
    Ok((checked, achieved))
}

/// 持ち主の根を実体の表から独立に引く（`owner_of`・`owner_in` は呼ばない）。
/// 製品の持ち主の決め方を壊した変異（X6）で、正解の側が一緒に変わらないようにするため（`docs/testing.md` 5.336）。
fn indep_owner(ir: &Ir, uid: &StepUid) -> Root {
    ir.instances
        .iter()
        .find(|i| i.caller.as_deref() == Some(uid.0.as_str()))
        .map(|i| Root(i.id.clone()))
        .unwrap_or_else(|| root_of(uid))
}

/// S-5・S-6 の検査で数えた件数。
#[derive(Debug, Default, Clone, Copy)]
pub struct RevertCounts {
    pub checked: u64,
    /// 持ち主以外が書いた世界状態の経路が、写しと復帰の前で違っていた件数。
    pub foreign_world_diff: u64,
    /// 持ち主以外の根の切り身が、写しと復帰の前で違っていた件数。
    pub foreign_slice_diff: u64,
    /// 持ち主が呼び出しの Step の実体である Step の状態が、写しと復帰の前で違っていた件数。
    pub call_step_diff: u64,
}

/// 戻しの共通の期待: 持ち主 `owner` の分だけが写し `copy` の値へ戻り、ほかは `before` のままである。
/// `since` は写しの後に追記されたログ。違反は（部分の名前）。下書き・入力の本文は出さない。
fn check_owner_revert(
    ir: &Ir,
    before: &SessionState,
    after: &SessionState,
    copy: &fandhe_workshop_core::session::RunState,
    since: &[Event],
    owner: &Root,
) -> Result<RevertCounts, String> {
    let mut c = RevertCounts {
        checked: 1,
        ..Default::default()
    };
    let (b, a) = (&before.run, &after.run);
    let mut roots: BTreeSet<&Root> = BTreeSet::new();
    roots.extend(b.slices.keys());
    roots.extend(a.slices.keys());
    roots.extend(copy.slices.keys());
    roots.insert(owner);
    for r in roots {
        let want = if r == owner { copy } else { b };
        let want = want.slices.get(r).cloned().unwrap_or_default();
        let got = a.slices.get(r).cloned().unwrap_or_default();
        if want != got {
            return Err(format!("slice of root {r:?}"));
        }
        if r != owner && copy.slices.get(r).cloned().unwrap_or_default() != want {
            c.foreign_slice_diff = 1;
        }
    }
    if a.current != copy.current
        || a.busy != copy.busy
        || a.completed != copy.completed
        || a.guidance_level != copy.guidance_level
    {
        return Err("position".to_string());
    }
    if a.global_flags != b.global_flags {
        return Err("global flags".to_string());
    }
    let touched: BTreeSet<&str> = since
        .iter()
        .filter_map(|e| match e {
            Event::Changed { at, path, .. } if &indep_owner(ir, &at.step) == owner => {
                Some(path.as_str())
            }
            _ => None,
        })
        .collect();
    let mut paths: BTreeSet<&str> = BTreeSet::new();
    paths.extend(b.world.keys().map(String::as_str));
    paths.extend(a.world.keys().map(String::as_str));
    paths.extend(copy.world.keys().map(String::as_str));
    for p in paths {
        let want = if touched.contains(p) { copy } else { b };
        if a.world.get(p) != want.world.get(p) {
            return Err(format!("world path {p}"));
        }
        if !touched.contains(p) && copy.world.get(p) != b.world.get(p) {
            c.foreign_world_diff = 1;
        }
    }
    // 呼び出しの Step は持ち主の根が置き場なので、実体を通した読みでも写しの値へ戻る。
    for i in &ir.instances {
        let (Some(caller), true) = (&i.caller, &Root(i.id.clone()) == owner) else {
            continue;
        };
        let uid = StepUid(caller.clone());
        if status_of(ir, a, &uid) != status_of(ir, copy, &uid) {
            return Err(format!("call step {caller}"));
        }
        if status_of(ir, b, &uid) != status_of(ir, copy, &uid) {
            c.call_step_diff = 1;
        }
    }
    Ok(c)
}

/// S-5: `Restored` を伴う操作で、保存した写しの持ち主の分だけが写しへ戻り、ほかは復帰の前のままである。
/// 持ち主は写しの現在の Step から独立に引く。
pub fn check_s5(
    ir: &Ir,
    before: &SessionState,
    after: &SessionState,
    events: &[Event],
) -> Result<RevertCounts, String> {
    let mut total = RevertCounts::default();
    for e in events {
        let Event::Restored { id, .. } = e else {
            continue;
        };
        let Some(cp) = before.checkpoints.get(id) else {
            continue;
        };
        let Some(cur) = &cp.run.current else {
            continue;
        };
        let owner = indep_owner(ir, cur);
        let since = usize::try_from(cp.log_len)
            .ok()
            .and_then(|n| before.log.get(n..))
            .unwrap_or(&[]);
        let c = check_owner_revert(ir, before, after, &cp.run, since, &owner)?;
        total.checked += c.checked;
        total.foreign_world_diff += c.foreign_world_diff;
        total.foreign_slice_diff += c.foreign_slice_diff;
        total.call_step_diff += c.call_step_diff;
    }
    Ok(total)
}

/// 停止の入口の境界（受付の位置）を、停止の前のログから独立に求める（製品の `entry_derive` は呼ばない）。
/// 状態に現在の Step の写しがあればその `log_len` を使う。無ければログを末尾からたどり、`Restored` は保存の時点へ
/// 跳び、現在の Step の `Accepted` の位置を返す。製品の境界は受付の操作の先頭（`InputReceived`）だが、
/// `Accepted` との間は `Judged` だけなので、`Changed` の集め方は同じになる。求まらなければ `None`。
fn entry_boundary(before: &SessionState, cur: &StepUid) -> Option<usize> {
    if let Some(e) = before
        .entry
        .as_ref()
        .filter(|e| e.run.current.as_ref() == Some(cur))
    {
        return usize::try_from(e.log_len).ok();
    }
    let mut at = before.log.len();
    while at > 0 {
        at -= 1;
        match before.log.get(at)? {
            Event::Restored { log_len, .. } => {
                at = usize::try_from(*log_len).ok().filter(|p| *p < at)?;
            }
            Event::Accepted { at: s } if &s.step == cur => return Some(at),
            _ => {}
        }
    }
    None
}

/// S-6: `Stopped` を伴う操作で、現在の Step の持ち主の分だけが入口の写しへ戻る。
/// 入口の写しは製品が非公開で導くので、持ち主以外の根・経路が停止の前のままであることだけを確かめる。
pub fn check_s6(
    ir: &Ir,
    before: &SessionState,
    after: &SessionState,
    events: &[Event],
) -> Result<u64, String> {
    if !events.iter().any(|e| matches!(e, Event::Stopped { .. })) {
        return Ok(0);
    }
    let Some(cur) = &before.run.current else {
        return Ok(0);
    };
    let owner = indep_owner(ir, cur);
    let (b, a) = (&before.run, &after.run);
    let mut roots: BTreeSet<&Root> = BTreeSet::new();
    roots.extend(b.slices.keys());
    roots.extend(a.slices.keys());
    for r in roots.into_iter().filter(|r| *r != &owner) {
        if b.slices.get(r) != a.slices.get(r) {
            return Err(format!("slice of root {r:?}"));
        }
    }
    if a.global_flags != b.global_flags {
        return Err("global flags".to_string());
    }
    // 持ち主が入口の後に書いていない経路は、停止の前のままである。入口の後に書いた経路は入口の値へ戻りうるので対象外。
    // 入口より前の書き込みで除くと、入口の後に別の根が更新した経路の巻き戻しを見逃すため、境界の後に限る。
    let boundary = entry_boundary(before, cur).ok_or_else(|| "entry boundary".to_string())?;
    let written: BTreeSet<&str> = before
        .log
        .get(boundary..)
        .ok_or_else(|| "entry boundary".to_string())?
        .iter()
        .filter_map(|e| match e {
            Event::Changed { at, path, .. } if indep_owner(ir, &at.step) == owner => {
                Some(path.as_str())
            }
            _ => None,
        })
        .collect();
    let mut paths: BTreeSet<&str> = BTreeSet::new();
    paths.extend(b.world.keys().map(String::as_str));
    paths.extend(a.world.keys().map(String::as_str));
    for p in paths.into_iter().filter(|p| !written.contains(p)) {
        if b.world.get(p) != a.world.get(p) {
            return Err(format!("world path {p}"));
        }
    }
    Ok(1)
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

/// ハーネスの動き。`Round2Closed` は M-2 用で、操作を「2 周目に閉じたもの」に絞る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    All,
    Round2Closed,
}

/// 操作を解決して流し、各操作の後に M-1 を検査するハーネス。
pub struct Harness {
    f: &'static Fixture,
    setup: Setup,
    mode: Mode,
    pub rs: RecordedSession,
    seen: Vec<TickAddr>,
    last_tick: Option<TickAddr>,
    pub ops: Vec<ROp>,
    pub stats: Stats,
    saw: [bool; 3],
    m2_reached: bool,
    m2_completed: bool,
}

impl Harness {
    pub fn new(setup: Setup) -> Self {
        Self::with_mode(setup, Mode::All)
    }

    /// M-2 用（T1 固定。操作を 2 周目に閉じる）。
    pub fn new_m2() -> Self {
        Self::with_mode(Setup::T1, Mode::Round2Closed)
    }

    fn with_mode(setup: Setup, mode: Mode) -> Self {
        let f = fixture();
        let mut h = Harness {
            f,
            setup,
            mode,
            m2_reached: false,
            m2_completed: false,
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
        let f = self.f;
        if let Some(cur) = self.rs.session().state().run.current.clone() {
            let r = f.round_of_step(&cur);
            self.saw[r as usize] = true;
            if r == 2 && owner_of(&f.ir, &cur).0.starts_with("issue-to-merge") {
                self.m2_reached = true;
            }
            // `ChapterCompleted` は未実装（5.258）なので、2 周目の完了は最後のモジュール `issue-to-merge@round2` を
            // 終えて呼び出し元の Step（`main/r2-impl`）へ戻った地点で見る。
            if f.ir.instances.iter().any(|i| {
                i.module.as_deref() == Some("issue-to-merge")
                    && f.r2.contains(&Root(i.id.clone()))
                    && i.caller.as_deref() == Some(cur.0.as_str())
            }) {
                self.m2_completed = true;
            }
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
            OpSpec::Guided => match self.guided() {
                ROp::Engine(Op::Control(Control::Begin(id)))
                    if self.mode == Mode::Round2Closed && !self.f.m2_starts.contains(&id) =>
                {
                    ROp::Engine(Op::Input(Input::Cancel))
                }
                op => op,
            },
            OpSpec::Input(i) => {
                ROp::Engine(Op::Input(self.f.inputs[i % self.f.inputs.len()].clone()))
            }
            OpSpec::Tick(k, n) => ROp::Engine(Op::Tick(self.tick_addr(k, n))),
            OpSpec::Stop => ROp::Engine(Op::Control(Control::Stop)),
            OpSpec::GoTo(i) if self.mode == Mode::Round2Closed => {
                let c = &self.f.r2_steps;
                ROp::Engine(Op::Control(Control::GoTo(c[i % c.len()].clone())))
            }
            OpSpec::GoTo(i) => {
                let nodes = &self.f.ir.nodes;
                ROp::Engine(Op::Control(Control::GoTo(StepUid(
                    nodes[i % nodes.len()].uid.clone(),
                ))))
            }
            OpSpec::Save => ROp::Engine(Op::Control(Control::Save)),
            OpSpec::Restore(i) => {
                let mut ids: Vec<CheckpointId> = st
                    .checkpoints
                    .iter()
                    .filter(|(_, c)| {
                        self.mode == Mode::All
                            || c.owner
                                .as_ref()
                                .is_some_and(|o| self.f.round_of_root(o) == 2)
                    })
                    .map(|(id, _)| *id)
                    .collect();
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
            OpSpec::Begin(i) if self.mode == Mode::Round2Closed => {
                let c = &self.f.m2_starts;
                let id = c
                    .get(i % (c.len() + 1))
                    .cloned()
                    .unwrap_or_else(|| "no-such-start".to_string());
                ROp::Engine(Op::Control(Control::Begin(id)))
            }
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
        let label = |what: &str| format!("{what} at op {index} ({spec:?} -> {})", kind_name(&op));
        check_m1(&self.f.ir, &before, &after, &targets).map_err(|(root, part)| {
            format!(
                "{}: root {root:?} part {part}; targets {targets:?}",
                label("M-1 violated")
            )
        })?;
        match check_move_source(self.f, &before, &after, &events) {
            Ok(Some(cross)) => {
                self.stats.add("moves_source_checked", 1);
                if cross {
                    self.stats.add("moves_cross_round", 1);
                }
            }
            Ok(None) => {}
            Err((root, part)) => {
                return Err(format!(
                    "{}: root {root:?} part {part}",
                    label("move source changed")
                ));
            }
        }
        let (checked, achieved) = check_completion_separation(self.f, &before, &after, &events)
            .map_err(|m| format!("{}: {m}", label("completion mixed")))?;
        self.stats.add("completion_sibling_checks", checked);
        self.stats
            .add("completion_sibling_checks_sibling_achieved", achieved);
        self.check_stale_tick(spec, &op, &before, &after, &events)
            .map_err(|m| format!("{}: {m}", label("S-1 violated")))?;
        let c = check_s5(&self.f.ir, &before, &after, &events)
            .map_err(|m| format!("{}: {m}", label("S-5 violated")))?;
        self.stats.add("s5_restores_checked", c.checked);
        self.stats
            .add("s5_restores_with_foreign_world_diff", c.foreign_world_diff);
        self.stats
            .add("s5_restores_with_foreign_slice_diff", c.foreign_slice_diff);
        self.stats.add("s5_call_step_restores", c.call_step_diff);
        let n = check_s6(&self.f.ir, &before, &after, &events)
            .map_err(|m| format!("{}: {m}", label("S-6 violated")))?;
        self.stats.add("s6_stops_checked", n);
        if self.mode == Mode::Round2Closed {
            if let Some(cur) = &after.run.current {
                let in_r1 = self.f.round_of_step(cur) == 1
                    || self
                        .f
                        .ir
                        .nodes
                        .iter()
                        .any(|n| n.uid == cur.0 && n.unit == self.f.r1_unit);
                if in_r1 {
                    return Err(format!(
                        "{}: M-2 sequence left round 2",
                        label("generation error")
                    ));
                }
            }
            let v = check_m2(self.f, &after);
            if let Some(first) = v.first() {
                return Err(format!(
                    "{}: {first} ({} total)",
                    label("M-2 violated"),
                    v.len()
                ));
            }
        }
        Ok(())
    }

    /// S-1: 宛先が現在の次の進行と違う進行は、状態を 1 ビットも変えず `Ignored` を 1 件だけ記録する。
    /// 古いかどうかは観察（`Progress` の有無）と宛先の照合の 2 通りで突き合わせ、食い違いを失敗にする。
    fn check_stale_tick(
        &mut self,
        spec: &OpSpec,
        op: &ROp,
        before: &SessionState,
        after: &SessionState,
        events: &[Event],
    ) -> Result<(), String> {
        let ROp::Engine(Op::Tick(addr)) = op else {
            return Ok(());
        };
        let f = self.f;
        let matches_next = before.run.current.as_ref() == Some(&addr.step)
            && before.run.generation == addr.generation
            && before
                .run
                .busy
                .as_ref()
                .is_some_and(|b| b.done == addr.index);
        let progressed = events.iter().any(|e| matches!(e, Event::Progress { .. }));
        if matches_next {
            if !progressed {
                return Err("matching tick did not progress".to_string());
            }
            self.stats.add("ticks_progressed", 1);
            return Ok(());
        }
        if progressed {
            return Err("stale tick progressed".to_string());
        }
        self.stats.add("stale_ticks", 1);
        let Some(cur) = &before.run.current else {
            self.stats.add("stale_ticks_no_current", 1);
            if after != before || !events.is_empty() {
                return Err("tick without current step changed state".to_string());
            }
            return Ok(());
        };
        match spec {
            OpSpec::Tick(0, _) => self.stats.add("stale_ticks_current", 1),
            OpSpec::Tick(1, _) => self.stats.add("stale_ticks_dup", 1),
            OpSpec::Tick(2, _) => self.stats.add("stale_ticks_past", 1),
            OpSpec::Tick(_, _) => self.stats.add("stale_ticks_other_step", 1),
            _ => {}
        }
        if before.run.busy.is_none() {
            self.stats.add("stale_ticks_not_busy", 1);
        }
        if owner_of(&f.ir, &addr.step) != owner_of(&f.ir, cur) {
            self.stats.add("stale_ticks_other_root", 1);
        }
        if f.round_of_step(&addr.step) != f.round_of_step(cur) {
            self.stats.add("stale_ticks_cross_round", 1);
        }
        let mut trimmed = after.clone();
        trimmed.log.truncate(before.log.len());
        if &trimmed != before {
            return Err("state changed".to_string());
        }
        let [Event::Ignored { at, reason }] = events else {
            return Err(format!("expected one Ignored, got {} events", events.len()));
        };
        let expected = Stamp {
            generation: before.run.generation,
            step: cur.clone(),
        };
        let logged = &after.log[before.log.len()..];
        if at != &expected || logged != events || logged.len() != 1 {
            return Err(format!("Ignored position or log mismatch ({reason:?})"));
        }
        Ok(())
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
        if self.mode == Mode::Round2Closed {
            s.add("m2_cases", 1);
            s.add("m2_ops", self.ops.len() as u64);
            if self.m2_reached {
                s.add("m2_reached_issue_to_merge_r2", 1);
            }
            if self.m2_completed {
                s.add("m2_round2_completed", 1);
            }
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
    run_case_in(case, Mode::All)
}

/// M-2 用に 1 ケースを流す（[`Mode::Round2Closed`]。開始は T1）。
pub fn run_m2_case(case: &Case) -> Result<Stats, String> {
    run_case_in(case, Mode::Round2Closed)
}

fn run_case_in(case: &Case, mode: Mode) -> Result<Stats, String> {
    let mut h = Harness::with_mode(case.setup, mode);
    for (i, spec) in case.specs.iter().enumerate() {
        h.step(i, spec)?;
    }
    if !h.replay_matches() {
        return Err("replay mismatch (S-8)".to_string());
    }
    Ok(h.finish_stats())
}
