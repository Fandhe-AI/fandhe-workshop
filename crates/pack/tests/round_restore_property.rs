//! 2 周の教材での「復元の不一致 0 件」の性質テスト（TASK-83 2/3。BUILD-7。記録は `docs/testing.md` の 5.341）。
//!
//! 2 周の教材 `module-reuse-rounds` の IR で、`round_gen` の操作列（固定・誘導つき。各 1,000 通り）を流し、
//! 列の途中（時点 t）と最後に履歴を書き出して、次の 2 経路で開き直した状態が元と一致することを確かめる。
//! (i) 書き出し → 読み込み（`load_session`。別のオリジン、SAVE-16）: 変換なしで開き、状態・開始点・操作の列が元と等しい。
//! (ii) 再読み込み（`boot_session`。web の復元と同じ関数）: 世代の +1 とログ末尾の `Reloaded` 1 件を除いて状態が等しい。
//! 最小教材だけだった core の `history_replay.rs`・`history_verify.rs` の復元の性質を、周回をまたぐ実体（4 実体）に広げる。
//!
//! 比べない範囲: 時点 t の履歴を再読み込みした続きに残りの操作を流す比較は行わない。残りの操作の進行は元の世代の宛先を持ち、
//! 再読み込み後は古い進行として無視されるため、元の列と同じ結果になる前提が成り立たない（5.341）。
//! 置き場が pack なのは、2 周の IR を作る `compile_dir` が pack にあり、core から pack へ依存できないため（`round_property.rs` と同じ）。
//! 失敗の文言には経路と時点の名前だけを出し、下書き・入力の本文は出さない。

mod common;
#[allow(
    dead_code,
    reason = "core の共通ヘルパのうち本ファイルが使わない部品があるため"
)]
#[path = "../../core/tests/common/property.rs"]
mod property;
#[path = "common/round_gen.rs"]
mod round_gen;
#[path = "common/rounds.rs"]
mod rounds;

use std::sync::{Mutex, OnceLock};

#[allow(unused_imports, reason = "round_gen が super::testdata を引くため")]
use common::testdata;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_core::addr::Generation;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_boot::{boot_session, BootOutcome};
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::load_session;
use fandhe_workshop_core::history_open::OpeningCaps;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::SessionState;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use round_gen::{fixed_cases, fixture, guided_cases, Case, Harness, Stats};

fn versions() -> &'static HistoryVersions {
    static V: OnceLock<HistoryVersions> = OnceLock::new();
    V.get_or_init(|| {
        let ir = &fixture().ir;
        record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
    })
}

/// 世代の +1 とログ末尾の `Reloaded` 1 件を除いて等しい。違いは経路の名前だけで返す。
fn only_reload_differs(before: &SessionState, after: &SessionState) -> bool {
    let (mut a, mut b) = (after.clone(), before.clone());
    a.run.generation = Generation(0);
    b.run.generation = Generation(0);
    a.log.clear();
    b.log.clear();
    let n = before.log.len();
    a == b
        && after.run.generation.0 == before.run.generation.0 + 1
        && after.log.get(..n) == Some(before.log.as_slice())
        && after.log.len() == n + 1
        && matches!(after.log.last(), Some(Event::Reloaded { .. }))
}

/// 現在の履歴を書き出し、2 経路で開き直して元と比べる。保存できない（操作が 0 件）ときは `Ok(false)`。
fn check_restore(rs: &RecordedSession, point: &str, stats: &mut Stats) -> Result<bool, String> {
    let Some(env) = rs.envelope_for_save() else {
        return Ok(false);
    };
    let bytes = export_bytes(&env).map_err(|_| format!("{point}: export failed"))?;
    let ir = fixture().ir.clone();
    let caps = OpeningCaps::current();

    let loaded = load_session(
        &bytes,
        ir.clone(),
        versions(),
        &caps,
        SessionState::default(),
    )
    .map_err(|_| format!("{point}: import refused"))?;
    if loaded.notes.is_some() {
        return Err(format!("{point}: import converted"));
    }
    if loaded.session.state() != rs.session().state() {
        return Err(format!("{point}: import state mismatch"));
    }
    if loaded.start != rs.start() || loaded.ops.as_slice() != rs.ops() {
        return Err(format!("{point}: import start/ops mismatch"));
    }

    let outcome = boot_session(
        Some(&bytes),
        ir.clone(),
        versions(),
        &caps,
        SessionState::default(),
        &RecordedSession::first_start(&ir),
    );
    let BootOutcome::Restored { session, notes } = outcome else {
        return Err(format!("{point}: reload not restored"));
    };
    if notes.is_some() {
        return Err(format!("{point}: reload converted"));
    }
    if !only_reload_differs(rs.session().state(), session.session().state()) {
        return Err(format!("{point}: reload state mismatch"));
    }
    if session.session().state().run.slices != rs.session().state().run.slices {
        return Err(format!("{point}: reload slices mismatch"));
    }

    stats.add("restores_checked", 1);
    let st = rs.session().state();
    if st.run.busy.is_some() {
        stats.add("restores_while_busy", 1);
    }
    if !st.drafts.is_empty() {
        stats.add("restores_with_draft", 1);
    }
    if !st.checkpoints.is_empty() {
        stats.add("restores_with_checkpoint", 1);
    }
    Ok(true)
}

/// 1 ケース。時点 t（操作の数で丸める）と最後に書き出して比べる。
fn run_restore_case(case: &Case, t_raw: usize) -> Result<Stats, String> {
    let mut h = Harness::new(case.setup);
    let t = t_raw % (case.specs.len() + 1);
    let mut stats = Stats::default();
    for (i, spec) in case.specs.iter().enumerate() {
        if i == t {
            check_restore(&h.rs, "mid", &mut stats)?;
        }
        h.step(i, spec)?;
    }
    check_restore(&h.rs, "end", &mut stats)?;
    stats.merge(&h.finish_stats());
    Ok(stats)
}

fn run_property<S>(name: &str, cases: u32, strategy: S) -> Stats
where
    S: Strategy<Value = (Case, usize)>,
{
    let total = Mutex::new(Stats::default());
    property::check(name, cases, strategy, |(c, t)| {
        let s = run_restore_case(&c, t).map_err(TestCaseError::fail)?;
        total.lock().expect("stats lock").merge(&s);
        Ok(())
    });
    let s = total.into_inner().expect("stats");
    println!("PROPERTY-TEST stats name={name} {:?}", s.0);
    s
}

#[test]
fn round_restore_holds_fixed_generation() {
    run_property("round-restore-fixed", 1_000, (fixed_cases(), 0usize..1_000));
}

#[test]
fn round_restore_holds_guided_generation() {
    run_property(
        "round-restore-guided",
        1_000,
        (guided_cases(), 0usize..1_000),
    );
}

/// 生成が 2 周目に届き、処理中・下書き・チェックポイントを持つ時点でも復元を比べていること（空振りでないことの確認）。
/// 網羅性の確認なので、種と件数を既定に固定して環境変数の上書きを受けない。
#[test]
fn round_restore_generation_covers_both_rounds() {
    let mut all = Stats::default();
    for (name, guided) in [
        ("restore-reach-fixed", false),
        ("restore-reach-guided", true),
    ] {
        let total = Mutex::new(Stats::default());
        let body = |(c, t): (Case, usize)| -> Result<(), TestCaseError> {
            let s = run_restore_case(&c, t).map_err(TestCaseError::fail)?;
            total.lock().expect("stats lock").merge(&s);
            Ok(())
        };
        let r = if guided {
            property::try_check_with(
                name,
                &property::DEFAULT_SEED,
                1_000,
                &(guided_cases(), 0usize..1_000),
                body,
            )
        } else {
            property::try_check_with(
                name,
                &property::DEFAULT_SEED,
                1_000,
                &(fixed_cases(), 0usize..1_000),
                body,
            )
        };
        r.unwrap_or_else(|m| panic!("{m}"));
        all.merge(&total.into_inner().expect("stats"));
    }
    println!("PROPERTY-TEST restore reach stats {:?}", all.0);
    for k in [
        "restores_checked",
        "cases_both_rounds",
        "restores_while_busy",
        "restores_with_draft",
        "restores_with_checkpoint",
    ] {
        assert!(all.get(k) > 0, "{k} is 0: not covered");
    }
}
