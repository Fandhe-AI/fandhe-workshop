//! 操作列の各操作の後に、性質 P-1a〜P-1d・P-2・P-3・P-4・P-6 を検査する検査器（TASK-219 2/3）。
//!
//! 呼び出し元は `op_sequence_properties.rs`。`op_gen::run_with` の検査フックに [`PropChecker::check`] を渡して使う。
//! #208（誘導つきの生成）も同じ検査器を再利用するため、`common` に置く。
//!
//! # 定義の出所
//!
//! 性質の定義は `testdata/engine-event-consistency/test-data/property-test.md`（PoC-1 で固定）の
//! 「確かめる性質」の表である。P-5（流し直しの一致）は `op_sequence_replay.rs` が持つため、ここでは扱わない。
//! 変異注入（TASK-220）も範囲外である。
//!
//! # 判定の規則
//!
//! 前提として、ログは追記だけで、今回のイベント列がその末尾に一致することを確かめる（`log`）。
//! 前提が崩れたときは、以降の検査の入力が信用できないため、そこで打ち切る。
//!
//! - P-1a: 増分で更新した [`Projection`] が、ログ全体から作り直したものと一致する
//! - P-1b: `Changed` を初期の世界状態へ順に当てた結果が、操作後の世界状態と一致する
//! - P-1c: 変化の印の経路と種類が、基準点の後の学習者の入力の `Changed` と一致し、基準点の世界状態と今の世界状態の
//!   差がすべて印に含まれる。基準点の世界状態は、基準点までの `Changed` に加え、基準点の後に並ぶ原因が入力でない
//!   `Changed`（停止・移動・復帰の巻き戻し）も当てたものである。巻き戻しは `record_changes` が操作のイベント列の
//!   末尾に足すため、基準点のイベントの後に並ぶが、基準点の側に属する
//! - P-1d: 復習の項目は、ログの入力の `Changed` と `Achieved` から導ける。`Restored` の操作では、復帰後の復習が
//!   `Restored.log_len` までのイベントから作り直した復習と、持ち主の根について一致する
//! - P-2: 宛先が現在と食い違う進行は、状態を変えず、`Ignored` を 1 件だけログに足す。宛先が一致する進行は
//!   `Ignored` を出さず `Progress` をちょうど 1 件出す。古い進行かどうかは、`OpSpec` ではなく解決した
//!   `Op::Tick` と操作前の状態から決める（`Dup` が現在の宛先に解決される場合があるため）
//! - P-3: `Achieved` は、同じ操作で同じ `at` の `Judged { Confirmed }` の後か、確認を省くと明示した Step
//!   （`confirm.kind` が `none`）で、同じ操作の同じ `at` の `Accepted` の後のものだけである。`confirm` が無い Step は確認を要する側に倒す（R-22）。
//!   復帰・停止の後は同じ世代に `Accepted` が前に無いため、`confirm_before_achieve.rs` の I3 は使わない。
//!   状態が達成の Step には、ログに `Achieved` がある
//! - P-4: ログの宛先の世代は単調に増える。世代は `Stopped`・`Moved`・`Restored`・`Begun` が出たときだけ、
//!   ちょうど 1 増える。判定は `OpSpec` ではなくイベントで行う（処理中でない停止は取り消しになる・移動の後の停止は
//!   世代を変えない・移動や復帰は拒否されうる）
//! - P-6: 同じ（世代, Step）で `Entered`・`Accepted`・`Achieved` はそれぞれ高々 1 回
//!
//! 失敗は、操作の番号・性質・解決した `Op` を含む文字列で返す。1 操作で複数の性質が違反した場合は、
//! すべてを改行で並べる（感度の試験が、壊した性質だけを名指しで確かめられるようにするため）。

use std::collections::BTreeMap;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_core::addr::{Root, Stamp, StepUid};
use fandhe_workshop_core::changed::{replay_changes, world_diff};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::projection::{Mark, MarkKind, Projection, ReviewItem};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::{Element, World};

use super::op_gen::Applied;
use super::tick::next_tick;

/// 統計。出力するだけで、assert には使わない（`docs/testing.md` 第 2 節）。
#[derive(Debug, Default, Clone)]
pub struct PropStats {
    /// 件数の表。鍵は事象の名前。
    pub counts: BTreeMap<&'static str, u64>,
    /// ログの最大の長さ。
    pub max_log: usize,
}

impl PropStats {
    fn bump(&mut self, key: &'static str) {
        *self.counts.entry(key).or_default() += 1;
    }

    /// 件数を読む。無ければ 0。
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// 別の統計を足し込む。
    pub fn merge(&mut self, other: &PropStats) {
        for (k, v) in &other.counts {
            *self.counts.entry(k).or_default() += v;
        }
        self.max_log = self.max_log.max(other.max_log);
    }
}

/// 1 本の操作列に付き、各操作の後の性質を検査する。状態を持つのは P-1a（増分の導出）と P-1b（累積の世界状態）だけ。
pub struct PropChecker<'a> {
    ir: &'a Ir,
    initial_world: World,
    world: World,
    projection: Projection,
    /// 統計。
    pub stats: PropStats,
}

fn is_base(e: &Event) -> bool {
    matches!(
        e,
        Event::Entered { .. }
            | Event::Stopped { .. }
            | Event::Moved { .. }
            | Event::Restored { .. }
            | Event::Begun { .. }
    )
}

fn mark_kind(before: &Option<Element>, after: &Option<Element>) -> MarkKind {
    match (before, after) {
        (None, Some(_)) => MarkKind::Put,
        (_, None) => MarkKind::Removed,
        (Some(_), Some(_)) => MarkKind::AttrChanged,
    }
}

/// すべてのイベントの `at`。`Event` は `#[non_exhaustive]` のため `_` が要るが、未知の variant は失敗にして
/// variant の追加時に検査の見直しを強いる。
fn stamp_of(e: &Event) -> Result<&Stamp, String> {
    match e {
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
        | Event::HintOpened { at, .. }
        | Event::Refused { at, .. }
        | Event::Ignored { at, .. }
        | Event::Warning { at, .. } => Ok(at),
        _ => Err(format!("unknown event variant (update op_props): {e:?}")),
    }
}

fn apply_changes(world: &mut World, events: &[Event], keep: impl Fn(&Cause) -> bool) {
    for e in events {
        if let Event::Changed { cause, .. } = e {
            if keep(cause) {
                replay_changes(world, std::slice::from_ref(e));
            }
        }
    }
}

fn review_of_root(items: &[ReviewItem], root: &Root) -> Vec<ReviewItem> {
    items
        .iter()
        .filter(|r| &r.step.root() == root)
        .cloned()
        .collect()
}

impl<'a> PropChecker<'a> {
    /// 初期状態から作る。
    pub fn new(ir: &'a Ir, initial: &SessionState) -> Self {
        PropChecker {
            ir,
            initial_world: initial.run.world.clone(),
            world: initial.run.world.clone(),
            projection: Projection::from_events(&initial.log),
            stats: PropStats::default(),
        }
    }

    /// Step の確認の種類。`confirm` が無ければ `None`、IR に無い Step も `None`（確認を要する側）。
    fn confirm_kind(&self, step: &StepUid) -> Option<String> {
        let node = self.ir.nodes.iter().find(|n| n.uid == step.0)?;
        let decl = node.step.confirm.as_ref()?;
        decl.get("kind")?.as_str().map(str::to_string)
    }

    /// 操作 `i` の結果に、すべての性質を検査する。
    pub fn check(&mut self, i: usize, a: &Applied) -> Result<(), String> {
        let head = format!("op #{i} {:?}", a.op);
        // 前提: ログは追記だけで、今回のイベント列が末尾に一致する。
        let n = a.before.log.len();
        if a.after.log.len() < n
            || a.after.log[..n] != a.before.log[..]
            || a.after.log[n..] != a.events[..]
        {
            return Err(format!("{head}: log: not append-only or events != suffix"));
        }
        self.stats.max_log = self.stats.max_log.max(a.after.log.len());

        let results = [
            ("P-1a", self.check_p1a(a)),
            ("P-1b", self.check_p1b(a)),
            ("P-1c", self.check_p1c(a)),
            ("P-1d", self.check_p1d(a)),
            ("P-2", self.check_p2(a)),
            ("P-3", self.check_p3(a)),
            ("P-4", self.check_p4(a)),
            ("P-6", check_p6(a)),
        ];
        let errors: Vec<String> = results
            .into_iter()
            .filter_map(|(prop, r)| r.err().map(|m| format!("{head}: {prop}: {m}")))
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }

    /// P-1a: 増分の導出が、ログ全体からの導出と一致する。
    pub fn check_p1a(&mut self, a: &Applied) -> Result<(), String> {
        self.projection.extend(&a.after.log);
        if self.projection != Projection::from_events(&a.after.log) {
            return Err("incremental projection != projection from the whole log".into());
        }
        Ok(())
    }

    /// P-1b: `Changed` を順に当てた世界状態が、操作後の世界状態と一致する。
    pub fn check_p1b(&mut self, a: &Applied) -> Result<(), String> {
        replay_changes(&mut self.world, &a.events);
        if self.world != a.after.run.world {
            let diff = world_diff(&self.world, &a.after.run.world);
            return Err(format!(
                "replayed world != world after the op; differing paths: {:?}",
                diff.iter().map(|d| d.0.as_str()).collect::<Vec<_>>()
            ));
        }
        Ok(())
    }

    /// P-1c: 変化の印が、基準点の後の入力の `Changed` と一致し、世界状態の差をすべて含む。
    pub fn check_p1c(&mut self, a: &Applied) -> Result<(), String> {
        let log = &a.after.log;
        let start = log.iter().rposition(is_base).map_or(0, |b| b + 1);
        // 基準点の世界状態: 基準点までの全 `Changed` と、基準点の後の入力でない `Changed`。
        let mut base = self.initial_world.clone();
        apply_changes(&mut base, &log[..start], |_| true);
        apply_changes(&mut base, &log[start..], |c| *c != Cause::Input);
        // 印の元: 基準点の後の入力の `Changed` を経路ごとに（最初の変更前, 最後の変更後）へ畳む。
        let mut folded: BTreeMap<&str, (&Option<Element>, &Option<Element>)> = BTreeMap::new();
        for e in &log[start..] {
            if let Event::Changed {
                path,
                before,
                after,
                cause: Cause::Input,
                ..
            } = e
            {
                folded
                    .entry(path.as_str())
                    .and_modify(|v| v.1 = after)
                    .or_insert((before, after));
            }
        }
        let expected: Vec<Mark> = folded
            .iter()
            .map(|(p, (b, af))| Mark {
                path: (*p).to_string(),
                kind: mark_kind(b, af),
            })
            .collect();
        if self.projection.marks() != expected.as_slice() {
            return Err(format!(
                "marks {:?} != expected {:?}",
                self.projection.marks(),
                expected
            ));
        }
        for (path, _, _) in world_diff(&base, &a.after.run.world) {
            if !folded.contains_key(path.as_str()) {
                return Err(format!(
                    "path {path:?} differs from the base world but has no mark"
                ));
            }
        }
        Ok(())
    }

    /// P-1d: 復習の項目は `Changed`・`Achieved` から導け、復帰の後の復習は保存の時点から作り直したものと一致する。
    pub fn check_p1d(&mut self, a: &Applied) -> Result<(), String> {
        let log = &a.after.log;
        for item in self.projection.review() {
            for c in &item.changes {
                let found = log.iter().any(|e| {
                    matches!(e, Event::Changed { at, path, cause: Cause::Input, .. }
                        if at.step == item.step && *path == c.path)
                });
                if !found {
                    return Err(format!(
                        "review change {:?} of {:?} has no Changed in the log",
                        c.path, item.step
                    ));
                }
            }
            if item.achieved
                && !log
                    .iter()
                    .any(|e| matches!(e, Event::Achieved { at, .. } if at.step == item.step))
            {
                return Err(format!(
                    "review item {:?} is achieved without Achieved",
                    item.step
                ));
            }
        }
        for e in &a.events {
            if let Event::Restored { log_len, owner, .. } = e {
                let len = usize::try_from(*log_len).unwrap_or(usize::MAX);
                if len > log.len() {
                    return Err(format!("Restored.log_len {len} is beyond the log"));
                }
                let root = owner.root();
                let rebuilt = Projection::from_events(&log[..len]);
                let got = review_of_root(self.projection.review(), &root);
                let want = review_of_root(rebuilt.review(), &root);
                if got != want {
                    return Err(format!(
                        "review after restore {got:?} != rebuilt from the first {len} events {want:?}"
                    ));
                }
                self.stats.bump("restore_review_checked");
            }
        }
        Ok(())
    }

    /// P-2: 古い進行は状態を変えず `Ignored` を 1 件だけ足す。一致する進行は `Progress` をちょうど 1 件出す。
    pub fn check_p2(&mut self, a: &Applied) -> Result<(), String> {
        let Op::Tick(addr) = &a.op else {
            return Ok(());
        };
        if next_tick(&a.before).as_ref() == Some(addr) {
            self.stats.bump("tick_current");
            let ignored = a
                .events
                .iter()
                .filter(|e| matches!(e, Event::Ignored { .. }))
                .count();
            let progress = a
                .events
                .iter()
                .filter(|e| matches!(e, Event::Progress { .. }))
                .count();
            if ignored != 0 || progress != 1 {
                return Err(format!(
                    "current tick: ignored={ignored} progress={progress}, events {:?}",
                    a.events
                ));
            }
            return Ok(());
        }
        self.stats.bump("tick_stale");
        let mut cut = a.after.clone();
        cut.log.truncate(a.before.log.len());
        if cut != a.before {
            return Err("a stale tick changed the state".into());
        }
        if a.before.run.current.is_some() {
            if !matches!(a.events.as_slice(), [Event::Ignored { .. }]) {
                return Err(format!(
                    "a stale tick must add exactly one Ignored, got {:?}",
                    a.events
                ));
            }
        } else {
            self.stats.bump("tick_without_current");
            if !a.events.is_empty() {
                return Err(format!(
                    "a tick without a current step must add nothing, got {:?}",
                    a.events
                ));
            }
        }
        if addr.generation.0 < a.before.run.generation.0 {
            self.stats.bump("stale_after_generation_change");
        }
        Ok(())
    }

    /// P-3: 確認の前の達成が無く、達成の状態にはログの `Achieved` がある。
    pub fn check_p3(&mut self, a: &Applied) -> Result<(), String> {
        for (k, e) in a.events.iter().enumerate() {
            let Event::Achieved { at, .. } = e else {
                continue;
            };
            let confirmed = a.events[..k].iter().any(|p| {
                matches!(p, Event::Judged { at: pa, judgment: Judgment::Confirmed } if pa == at)
            });
            let kind = self.confirm_kind(&at.step);
            if confirmed {
                self.stats.bump("achieved_by_confirm");
            } else if kind.as_deref() == Some("none") {
                // 確認を省く Step は、受付と同時に達成する。同じ操作で同じ `at` の `Accepted` が先行する。
                let accepted = a.events[..k]
                    .iter()
                    .any(|p| matches!(p, Event::Accepted { at: pa } if pa == at));
                if !accepted {
                    return Err(format!(
                        "achievement without acceptance at {at:?} (confirm kind none)"
                    ));
                }
                self.stats.bump("achieved_by_none");
            } else {
                return Err(format!(
                    "achievement before confirmation at {at:?} (confirm kind {kind:?})"
                ));
            }
        }
        for slice in a.after.run.slices.values() {
            for (step, status) in &slice.status {
                if *status == StepStatus::Achieved
                    && !a
                        .after
                        .log
                        .iter()
                        .any(|e| matches!(e, Event::Achieved { at, .. } if at.step == *step))
                {
                    return Err(format!("step {step:?} is Achieved without an event"));
                }
            }
        }
        Ok(())
    }

    /// P-4: 世代は単調に増え、停止・移動・復帰（・開始）の成功のときだけちょうど 1 増える。
    pub fn check_p4(&mut self, a: &Applied) -> Result<(), String> {
        let mut prev = 0u64;
        for e in &a.after.log {
            let g = stamp_of(e)?.generation.0;
            if g < prev {
                return Err(format!("generation decreased in the log: {prev} -> {g}"));
            }
            prev = g;
        }
        let mut bumps: Vec<(&'static str, u64)> = Vec::new();
        for e in &a.events {
            match e {
                Event::Stopped { new_generation, .. } => bumps.push(("gen_stop", new_generation.0)),
                Event::Moved { new_generation, .. } => bumps.push(("gen_move", new_generation.0)),
                Event::Restored { new_generation, .. } => {
                    bumps.push(("gen_restore", new_generation.0))
                }
                Event::Begun { new_generation, .. } => bumps.push(("gen_begin", new_generation.0)),
                _ => {}
            }
        }
        let before = a.before.run.generation.0;
        let after = a.after.run.generation.0;
        match bumps.as_slice() {
            [] if after == before => {}
            [(key, new)] if after == before + 1 && *new == after => self.stats.bump(key),
            _ => {
                return Err(format!(
                    "generation {before} -> {after} does not match the events {bumps:?}"
                ))
            }
        }
        if matches!(
            a.op,
            Op::Input(_) | Op::Tick(_) | Op::Control(Control::Save)
        ) && after != before
        {
            return Err("an input, tick or save changed the generation".into());
        }
        Ok(())
    }
}

/// P-6: 同じ（世代, Step）で `Entered`・`Accepted`・`Achieved` は高々 1 回。
pub fn check_p6(a: &Applied) -> Result<(), String> {
    let mut seen: BTreeMap<(&'static str, u64, &str), u32> = BTreeMap::new();
    for e in &a.after.log {
        let (kind, at) = match e {
            Event::Entered { at } => ("Entered", at),
            Event::Accepted { at } => ("Accepted", at),
            Event::Achieved { at, .. } => ("Achieved", at),
            _ => continue,
        };
        let n = seen
            .entry((kind, at.generation.0, at.step.0.as_str()))
            .or_default();
        *n += 1;
        if *n > 1 {
            return Err(format!(
                "{kind} twice at generation {} step {}",
                at.generation.0, at.step.0
            ));
        }
    }
    Ok(())
}
