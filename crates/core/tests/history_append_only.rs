//! 履歴が追記だけで増える性質（TASK-188 1/2、EXT-5、D-53）の試験。
//!
//! D-53（2026-10-12）で、今の `ops` を「追記だけで増える列」として保証し、初回は同期を実装しないと決まった
//! （D-5）。書き出しの形は `ops` と写しの両方を持つ（D-52・SAVE-17）ため、対象は写しの列の欄も含む。
//! 同期の試験は作らない。履歴の形式の文書は TASK-188 2/2 が、本ファイルの定義を引いて書く。
//!
//! # 「追記だけで増える」の定義
//!
//! 同じ実行の中で、前の包み prev と後の包み next について、次の 1〜4 をすべて満たすこと。
//!
//! 1. 頭の欄が変わらない: `format`・`history_format`・`pack_id`・`start`・`versions`（4 軸すべて）が等しい
//! 2. `ops` は確定した部分を書き換えない。n を prev の `ops` の件数とする
//!    - next の件数は n 以上である
//!    - 先頭から n - 1 件目までは等しい
//!    - n 件目は等しいか、両方が `Draft` である。末尾の未確定の `Draft` だけは置き換わってよい。
//!      これは今の記録規則（連続する下書きは最後の 1 件に畳む）そのもので、記録規則は変えない
//! 3. `snapshot.log` は、prev のものが next のものの接頭辞になっている
//! 4. `snapshot.checkpoints` は、prev の各項目が next に同じ値で残り、`next_checkpoint` が減らない
//!
//! # 対象にしない欄
//!
//! `snapshot` の `run`・`entry`・`drafts`・`hints_opened` は、列ではなく現在の状態で、操作のたびに上書きされる
//! （復帰は `run`・`entry` を戻し、下書きは入力のたびに変わる）。増え方の性質を持たないため確かめない。
//! 世代が単調に増えることは既存の性質（P 系）の範囲である。
//!
//! # 確かめること
//!
//! - 記録 1 段（下書き・操作・再読み込み）ごとの包みを直前の包みと比べ、ケースごとに引いた時点 t の包みと
//!   最後の包みも比べる。書き出し→読み込みを通した包みでも同じ性質が成り立つ
//! - 時点 t の包みを `boot_session` で復元し、本番の保存の入口（`envelope_for_save`）と続きの記録でも成り立つ
//!   （起動で `Reload` と `Reloaded` が足されても接頭辞が保たれる）
//! - 検査器は、1 か所ずつ壊した組で違反を出す（自己確認）。違反の文言に下書き・送信の本文を含めない
//!
//! 性質テストの成功条件は性質の成立だけにする。統計は出力するだけで assert しない（`docs/testing.md` 第 2 節）。

mod common;

use std::cell::RefCell;
use std::collections::BTreeMap;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{
    guided_op_specs, guided_op_specs_with_reload, op_specs, op_specs_with_reload, OpSpec, Which,
};
use common::property::{check, MAX_OPS};
use common::record_path::{draft_text, RecordPath};
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::limits::LoadEntry;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_boot::{boot_session, BootOutcome};
use fandhe_workshop_core::history_envelope::{build_envelope_with, SessionEnvelope};
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_open::OpeningCaps;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_gate::load_json;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::ops_record::SessionOp;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::version_record::record_versions;
use proptest::collection::vec;
use proptest::prelude::*;

/// 教材（IR）と最初の状態を読む。ケースごとではなく、試験の先頭で 1 回だけ呼ぶ。
fn load_min_ir() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    (load_ir(&bytes).expect("load ir"), sample.initial)
}

/// 版の欄。包みごとに `content_hash` を求める費用を避けるため、試験の先頭で 1 回だけ求める。
fn versions_of(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

/// 記録の経路の現在の `ops` と写しから包みを作る。
fn envelope(ir: &Ir, versions: &HistoryVersions, path: &RecordPath<'_>) -> SessionEnvelope {
    build_envelope_with(
        ir,
        versions.clone(),
        "s-1",
        path.rec.ops().to_vec(),
        path.state.clone(),
    )
    .expect("build")
}

/// 操作の種類の名前。違反の文言に使う（本文は出さない）。
fn kind(op: &SessionOp) -> &'static str {
    match op {
        HistoryOp::Draft(_) => "draft",
        HistoryOp::Submit(_) => "submit",
        HistoryOp::Reload => "reload",
        HistoryOp::Engine(_) => "engine",
    }
}

/// 追記だけで増えることの検査器（性質 1〜4）。違反の文言は性質の番号・位置・種類だけを含む（本文を含めない）。
fn check_append_only(prev: &SessionEnvelope, next: &SessionEnvelope) -> Result<(), String> {
    // 1. 頭の欄
    if prev.format != next.format
        || prev.history_format != next.history_format
        || prev.pack_id != next.pack_id
        || prev.start != next.start
    {
        return Err("P1: a head field changed".to_owned());
    }
    if prev.versions != next.versions {
        return Err("P1: versions changed".to_owned());
    }
    // 2. ops
    let n = prev.ops.len();
    if next.ops.len() < n {
        return Err(format!("P2: ops shrank from {n} to {}", next.ops.len()));
    }
    for i in 0..n {
        let (a, b) = (&prev.ops[i], &next.ops[i]);
        let open_draft_replaced =
            i + 1 == n && matches!((a, b), (HistoryOp::Draft(_), HistoryOp::Draft(_)));
        if a != b && !open_draft_replaced {
            return Err(format!(
                "P2: ops[{i}] was rewritten ({} -> {})",
                kind(a),
                kind(b)
            ));
        }
    }
    // 3. log の接頭辞
    let (pl, nl) = (&prev.snapshot.log, &next.snapshot.log);
    if nl.len() < pl.len() {
        return Err(format!("P3: log shrank from {} to {}", pl.len(), nl.len()));
    }
    if let Some(i) = (0..pl.len()).find(|&i| pl[i] != nl[i]) {
        return Err(format!("P3: log[{i}] was rewritten"));
    }
    // 4. checkpoints
    for (id, cp) in &prev.snapshot.checkpoints {
        match next.snapshot.checkpoints.get(id) {
            None => return Err(format!("P4: checkpoint {} was removed", id.0)),
            Some(c) if c != cp => return Err(format!("P4: checkpoint {} was rewritten", id.0)),
            Some(_) => {}
        }
    }
    if next.snapshot.next_checkpoint < prev.snapshot.next_checkpoint {
        return Err("P4: next_checkpoint decreased".to_owned());
    }
    Ok(())
}

/// 書き出し→読み込みを通した包み（実在の口）。
fn through_export(env: &SessionEnvelope) -> Result<SessionEnvelope, String> {
    let bytes = export_bytes(env).map_err(|e| format!("export: {e}"))?;
    load_json::<SessionEnvelope>(LoadEntry::History, &bytes).map_err(|e| format!("import: {e:?}"))
}

/// 記録 1 段ごとの包みを集めながら、直前の包みと比べる。
struct Trail<'a> {
    ir: &'a Ir,
    versions: &'a HistoryVersions,
    envs: Vec<SessionEnvelope>,
}

impl<'a> Trail<'a> {
    fn new(ir: &'a Ir, versions: &'a HistoryVersions, path: &RecordPath<'_>) -> Self {
        Trail {
            ir,
            versions,
            envs: vec![envelope(ir, versions, path)],
        }
    }

    /// 現在の経路の包みを足し、直前の包みと比べる。
    fn step(&mut self, path: &RecordPath<'_>, label: &str) -> Result<(), String> {
        let next = envelope(self.ir, self.versions, path);
        if let Some(prev) = self.envs.last() {
            check_append_only(prev, &next).map_err(|e| format!("{label}: {e}"))?;
        }
        self.envs.push(next);
        Ok(())
    }

    fn last(&self) -> &SessionEnvelope {
        self.envs.last().expect("never empty")
    }
}

/// 下書きを挟みながら 1 つの枠（下書き 0〜2 件と操作 1 件）を流す。
fn run_slot(
    path: &mut RecordPath<'_>,
    trail: &mut Trail<'_>,
    drafts: &[u8],
    spec: Option<&OpSpec>,
    i: usize,
) -> Result<(), String> {
    for d in drafts.iter().filter_map(|k| draft_text(*k)) {
        path.draft(d);
        trail.step(path, &format!("draft at #{i}"))?;
    }
    if let Some(spec) = spec {
        path.spec(spec);
        trail.step(path, &format!("op at #{i}"))?;
    }
    Ok(())
}

/// 時点 t と最後の 2 点を、書き出しの形を通しても確かめる。
fn check_export_form(first: &SessionEnvelope, last: &SessionEnvelope) -> Result<(), String> {
    let (a, b) = (through_export(first)?, through_export(last)?);
    if &a != first || &b != last {
        return Err("export round trip changed the envelope".to_owned());
    }
    check_append_only(&a, &b).map_err(|e| format!("after export: {e}"))
}

/// 性質 A: 記録 1 段ごと・離れた 2 点・書き出しの形で成り立つ。
fn grows_by_append_only(
    ir: &Ir,
    initial: &SessionState,
    versions: &HistoryVersions,
    specs: &[OpSpec],
    drafts: &[Vec<u8>],
    t: usize,
) -> Result<(), String> {
    let mut path = RecordPath::new(ir, initial);
    let mut trail = Trail::new(ir, versions, &path);
    for (i, spec) in specs.iter().enumerate() {
        run_slot(&mut path, &mut trail, &drafts[i], Some(spec), i)?;
    }
    run_slot(
        &mut path,
        &mut trail,
        &drafts[specs.len()],
        None,
        specs.len(),
    )?;
    let at = t % trail.envs.len();
    check_append_only(&trail.envs[at], trail.last()).map_err(|e| format!("t={at} to last: {e}"))?;
    check_export_form(&trail.envs[at], trail.last())
}

/// 性質 B: 時点 t の包みを起動で復元し、本番の保存の入口と続きの記録でも成り立つ。
fn grows_across_boot(
    ir: &Ir,
    initial: &SessionState,
    versions: &HistoryVersions,
    specs: &[OpSpec],
    drafts: &[Vec<u8>],
    t: usize,
) -> Result<(), String> {
    let cut = t % (specs.len() + 1);
    let mut path = RecordPath::new(ir, initial);
    let mut head = Trail::new(ir, versions, &path);
    for (i, spec) in specs.iter().take(cut).enumerate() {
        run_slot(&mut path, &mut head, &drafts[i], Some(spec), i)?;
    }
    run_slot(&mut path, &mut head, &drafts[cut], None, cut)?;
    let saved = head.last().clone();
    let bytes = export_bytes(&saved).map_err(|e| format!("export: {e}"))?;
    let outcome = boot_session(
        Some(&bytes),
        ir.clone(),
        versions,
        &OpeningCaps::current(),
        initial.clone(),
        "s-1",
    );
    let session = match outcome {
        BootOutcome::Restored { session, .. } => session,
        // 未操作の包みは新しく始まるだけで、比べる対象が無い（SAVE-28）。
        BootOutcome::Fresh(_) if saved.ops.is_empty() => return Ok(()),
        BootOutcome::Fresh(_) => return Err(format!("boot at t={cut} started fresh")),
        BootOutcome::NotOpened { .. } => return Err(format!("boot at t={cut} was not opened")),
    };
    // 起動直後の本番の保存の入口。`Reload` と `Reloaded` が足されても接頭辞は保たれる。
    let booted = session
        .envelope_for_save()
        .ok_or_else(|| format!("no envelope to save after boot at t={cut}"))?;
    check_append_only(&saved, &booted).map_err(|e| format!("boot at t={cut}: {e}"))?;
    // 続きを流す。直前の包みと、保存した時点の包みの両方と比べる。
    let mut path = RecordPath::resume(
        ir,
        session.session().state().clone(),
        session.ops().to_vec(),
    );
    let mut tail = Trail {
        ir,
        versions,
        envs: vec![booted],
    };
    for (i, spec) in specs.iter().enumerate().skip(cut) {
        let from = tail.envs.len();
        let drafts_here: &[u8] = if i == cut { &[] } else { &drafts[i] };
        run_slot(&mut path, &mut tail, drafts_here, Some(spec), i)?;
        for env in &tail.envs[from..] {
            check_append_only(&saved, env).map_err(|e| format!("after boot, op #{i}: {e}"))?;
        }
    }
    let from = tail.envs.len();
    run_slot(
        &mut path,
        &mut tail,
        &drafts[specs.len()],
        None,
        specs.len(),
    )?;
    for env in &tail.envs[from..] {
        check_append_only(&saved, env).map_err(|e| format!("after boot, last drafts: {e}"))?;
    }
    Ok(())
}

/// 1 ケースの検査（性質 A・B）。引数は IR・最初の状態・版の欄・操作の列・各枠の下書き・時点 t。
type CaseBody =
    fn(&Ir, &SessionState, &HistoryVersions, &[OpSpec], &[Vec<u8>], usize) -> Result<(), String>;

type Stats = RefCell<BTreeMap<&'static str, u64>>;

fn count_kinds(stats: &Stats, env: &SessionEnvelope) {
    let mut s = stats.borrow_mut();
    for op in &env.ops {
        *s.entry(kind(op)).or_default() += 1;
    }
}

/// 戦略を 1,000 通り流す。各枠の下書きは 0〜2 件（`history_replay` と同じ挟み方）で、時点 t も 1 つ引く。
fn run_property<S>(name: &str, strategy: S, body: CaseBody)
where
    S: Strategy<Value = Vec<OpSpec>>,
{
    let (ir, initial) = load_min_ir();
    let versions = versions_of(&ir);
    let stats = Stats::default();
    let input = (
        strategy,
        vec(vec(0u8..5, 0..=2), MAX_OPS + 1),
        0usize..1_000,
    );
    check(name, 1_000, input, |(specs, drafts, t)| {
        if let Err(message) = body(&ir, &initial, &versions, &specs, &drafts, t) {
            prop_assert!(false, "{message}");
        }
        // 統計のため、最後まで流した包みの種類別の件数を数える（assert しない）。
        let mut path = RecordPath::new(&ir, &initial);
        for (spec, d) in specs.iter().zip(&drafts) {
            for text in d.iter().filter_map(|k| draft_text(*k)) {
                path.draft(text);
            }
            path.spec(spec);
        }
        count_kinds(&stats, &envelope(&ir, &versions, &path));
        Ok(())
    });
    println!("PROPERTY-TEST stats={:?}", stats.borrow());
}

#[test]
fn property_append_only_fixed() {
    run_property("append-only", op_specs(), grows_by_append_only);
}

#[test]
fn property_append_only_guided() {
    run_property(
        "append-only-guided",
        guided_op_specs(),
        grows_by_append_only,
    );
}

#[test]
fn property_append_only_with_reload_fixed() {
    run_property(
        "append-only-reload",
        op_specs_with_reload(),
        grows_by_append_only,
    );
}

#[test]
fn property_append_only_with_reload_guided() {
    run_property(
        "append-only-reload-guided",
        guided_op_specs_with_reload(),
        grows_by_append_only,
    );
}

#[test]
fn property_append_only_across_boot_fixed() {
    run_property(
        "append-only-boot",
        op_specs_with_reload(),
        grows_across_boot,
    );
}

#[test]
fn property_append_only_across_boot_guided() {
    run_property(
        "append-only-boot-guided",
        guided_op_specs_with_reload(),
        grows_across_boot,
    );
}

/// 固定の列を流して、増え方の種類ごとの件数を数える。
enum Step {
    Draft(&'static str),
    Spec(OpSpec),
}

/// 手で並べた列。各種類の増え方を 1 回以上起こす。
fn script() -> Vec<Step> {
    use Step::{Draft, Spec};
    vec![
        Draft("/make"),
        Draft("/make-note first-note"),
        Spec(OpSpec::Cmd(0)),
        Draft("ノートを作りたい"),
        Spec(OpSpec::Save),
        Spec(OpSpec::Tick(Which::Current)),
        Spec(OpSpec::Tick(Which::Current)),
        Spec(OpSpec::Guided(0)),
        Spec(OpSpec::Guided(0)),
        Spec(OpSpec::Text),
        Spec(OpSpec::Save),
        Spec(OpSpec::Stop),
        Spec(OpSpec::GoTo(1)),
        Spec(OpSpec::Restore(1)),
        Spec(OpSpec::Reload),
        Spec(OpSpec::Restore(2)),
        Draft("未送信の下書き"),
        Draft("置き換えた下書き"),
    ]
}

/// 固定の列の各段の包み。
fn scripted_trail(
    ir: &Ir,
    initial: &SessionState,
    versions: &HistoryVersions,
) -> Vec<SessionEnvelope> {
    let mut path = RecordPath::new(ir, initial);
    let mut trail = Trail::new(ir, versions, &path);
    for (i, step) in script().iter().enumerate() {
        match step {
            Step::Draft(t) => path.draft(t),
            Step::Spec(s) => path.spec(s),
        }
        trail
            .step(&path, &format!("step #{i}"))
            .expect("append only");
    }
    trail.envs
}

#[test]
fn scripted_sequence_covers_each_kind_of_growth() {
    let (ir, initial) = load_min_ir();
    let versions = versions_of(&ir);
    let envs = scripted_trail(&ir, &initial, &versions);
    let mut counts = BTreeMap::<&'static str, u32>::new();
    for pair in envs.windows(2) {
        let (prev, next) = (&pair[0], &pair[1]);
        let added = &next.snapshot.log[prev.snapshot.log.len()..];
        let n = prev.ops.len();
        let key = match (prev.ops.last(), next.ops.last()) {
            (Some(HistoryOp::Draft(_)), Some(HistoryOp::Draft(_))) if next.ops.len() == n => {
                "draft-replaced"
            }
            (Some(HistoryOp::Submit(_)), Some(HistoryOp::Draft(_))) if next.ops.len() == n + 1 => {
                "draft-after-submit"
            }
            (_, Some(HistoryOp::Submit(_))) => "submit",
            (_, Some(HistoryOp::Reload)) => "reload",
            (_, Some(HistoryOp::Engine(Op::Control(Control::Save)))) => "save",
            (_, Some(HistoryOp::Engine(Op::Control(Control::Restore(_)))))
                if added.iter().any(|e| {
                    matches!(e, Event::Restored { log_len, .. }
                        if (*log_len as usize) < prev.snapshot.log.len())
                }) =>
            {
                "restore-after-log-grew"
            }
            (_, Some(HistoryOp::Engine(_))) => "engine",
            _ => "other",
        };
        *counts.entry(key).or_default() += 1;
    }
    println!("scripted growth kinds={counts:?}");
    for key in [
        "draft-replaced",
        "draft-after-submit",
        "submit",
        "engine",
        "reload",
        "save",
        "restore-after-log-grew",
    ] {
        assert!(
            counts.get(key).copied().unwrap_or(0) >= 1,
            "no {key}: {counts:?}"
        );
    }
    assert!(envs.last().expect("last").snapshot.checkpoints.len() >= 2);
}

/// 番兵。違反の文言に本文が出ないことの確認に使う。
const SENTINEL: &str = "SENTINEL-下書き本文-7f3a";

#[test]
fn checker_detects_each_violation() {
    let (ir, initial) = load_min_ir();
    let versions = versions_of(&ir);
    let envs = scripted_trail(&ir, &initial, &versions);
    let base = envs.last().expect("last").clone();
    assert!(base.ops.len() >= 4 && !base.snapshot.log.is_empty());
    assert!(!base.snapshot.checkpoints.is_empty());
    assert_eq!(check_append_only(&base, &base), Ok(()));

    // 陰性の対照: 末尾の下書きだけが別の下書きに変わる組・件が増える組は違反にならない。
    let mut replaced = base.clone();
    *replaced.ops.last_mut().expect("last") = HistoryOp::Draft(SENTINEL.to_owned());
    assert!(matches!(base.ops.last(), Some(HistoryOp::Draft(_))));
    assert_eq!(check_append_only(&base, &replaced), Ok(()));
    let mut grown = base.clone();
    grown.ops.push(HistoryOp::Reload);
    assert_eq!(check_append_only(&base, &grown), Ok(()));

    let first_submit = base
        .ops
        .iter()
        .position(|o| matches!(o, HistoryOp::Submit(_)))
        .expect("has a submit");
    let broken: Vec<(&str, SessionEnvelope)> = vec![
        ("ops[0] rewritten", {
            let mut e = base.clone();
            e.ops[0] = HistoryOp::Submit(SENTINEL.to_owned());
            e
        }),
        ("settled submit turned into a draft", {
            let mut e = base.clone();
            e.ops[first_submit] = HistoryOp::Draft(SENTINEL.to_owned());
            e
        }),
        ("open draft turned into a submit", {
            let mut e = base.clone();
            *e.ops.last_mut().expect("last") = HistoryOp::Submit(SENTINEL.to_owned());
            e
        }),
        ("ops shortened", {
            let mut e = base.clone();
            e.ops.pop();
            e
        }),
        ("log[0] dropped", {
            let mut e = base.clone();
            e.snapshot.log.remove(0);
            e
        }),
        ("checkpoint log_len changed", {
            let mut e = base.clone();
            e.snapshot
                .checkpoints
                .values_mut()
                .next()
                .expect("cp")
                .log_len += 1;
            e
        }),
        ("checkpoint removed", {
            let mut e = base.clone();
            let id = *e.snapshot.checkpoints.keys().next().expect("cp");
            e.snapshot.checkpoints.remove(&id);
            e
        }),
        ("next_checkpoint decreased", {
            let mut e = base.clone();
            e.snapshot.next_checkpoint.0 -= 1;
            e
        }),
        ("versions.engine changed", {
            let mut e = base.clone();
            e.versions.engine = SENTINEL.to_owned();
            e
        }),
        ("versions.ir_format changed", {
            let mut e = base.clone();
            e.versions.ir_format += 1;
            e
        }),
        ("versions.vocab changed", {
            let mut e = base.clone();
            e.versions.vocab = SENTINEL.to_owned();
            e
        }),
        ("versions.content changed", {
            let mut e = base.clone();
            e.versions.content = SENTINEL.to_owned();
            e
        }),
        ("versions.content_hash changed", {
            let mut e = base.clone();
            e.versions.content_hash = SENTINEL.to_owned();
            e
        }),
        ("start changed", {
            let mut e = base.clone();
            e.start = SENTINEL.to_owned();
            e
        }),
        ("pack_id changed", {
            let mut e = base.clone();
            e.pack_id = SENTINEL.to_owned();
            e
        }),
    ];
    for (what, next) in &broken {
        let message = check_append_only(&base, next).expect_err(what);
        assert!(
            !message.contains(SENTINEL),
            "{what}: the message leaks a body"
        );
        assert!(message.starts_with('P'), "{what}: {message}");
    }
    // 離れた 2 点でも同じ検査器が違反を出す（最初の包みと壊した最後の包み）。
    assert!(check_append_only(&envs[envs.len() / 2], &broken[4].1).is_err());
}
