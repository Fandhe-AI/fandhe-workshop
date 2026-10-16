//! 適用し直した結果と包みの写しの照合（`history_replay::verify_envelope`、TASK-316 2/3、SAVE-1）の試験。
//!
//! 読み込みは実在の口（`load_gate::load_json`）を通し、書き出しは `serde_json` で行う。固定の試験で
//! 一致・食い違い・型つきのエラーと「表示だけ」の元を確かめ、性質テスト（固定・誘導つきの生成を各 1,000 通り）で
//! 次の 2 つを確かめる。
//! - 書き出し→読み込みの一致: 記録の経路の包みを書き出して読み込み、照合が通り、写しと一致する
//! - 任意の時点の書き出し→読み込みの後の一致: 時点 t の包みを読み込んで得た状態に残りの `ops` を流した結果が、
//!   列を最後まで流した結果と一致する
//!
//! 後者は再読み込みを挟む形（H-5）である。時点 t で再読み込みを挟んだ列を続けて流した結果と、時点 t の包みを
//! 書き出し→読み込みで復元して再読み込みしてから残りを流した結果が一致し、再読み込みの後の世代が時点 t の
//! 世代 +1 であることも明示して確かめる（TASK-320 1/2）。性質テストの成功条件は一致が主である（`docs/testing.md` 第 2 節）。
//! 性質テストにはほかに、書き出しのバイト列の安定（H-7）と写しから作り直した実行の一致（H-2）、`ops` を 1 件消した包みが
//! 黙って誤った状態で開かないこと（H-8）を含める。変異 `SAVE1-1`〜`SAVE1-6` の検出先になる（`docs/testing.md` 5.59）。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::history_sample;
use common::op_gen::{guided_op_specs, op_specs, OpSpec, Which};
use common::property::{check, MAX_OPS};
use common::record_path::{draft_text, RecordPath};
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::limits::LoadEntry;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::history_envelope::{build_envelope, SessionEnvelope};
use fandhe_workshop_core::history_replay::{
    replay, verify_envelope, view_only_source, ReplayError, SnapshotField, VerifyError,
};
use fandhe_workshop_core::input::{CheckpointId, Control, Op};
use fandhe_workshop_core::load_gate::load_json;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::ops_record::{reload_recorded, OpsRecorder, SessionOp};
use fandhe_workshop_core::session::{Session, SessionState};
use proptest::collection::vec;
use proptest::prelude::*;

/// 教材（IR）と最初の状態を読む。ケースごとではなく、試験の先頭で 1 回だけ呼ぶ。
fn load_min_ir() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    (load_ir(&bytes).expect("load ir"), sample.initial)
}

/// 包みを書き出して、実在の読み込みの口で読み戻す。
fn roundtrip(env: &SessionEnvelope) -> Result<SessionEnvelope, String> {
    let bytes = serde_json::to_vec(env).map_err(|e| format!("export: {e}"))?;
    load_json::<SessionEnvelope>(LoadEntry::History, &bytes).map_err(|e| format!("import: {e:?}"))
}

/// 1/3 の台本と同じ列を記録の経路で流す。
fn scripted<'a>(ir: &'a Ir, initial: &SessionState) -> RecordPath<'a> {
    let mut path = RecordPath::new(ir, initial);
    path.draft("/make");
    path.draft("/make-note first-note");
    path.spec(&OpSpec::Cmd(0));
    path.spec(&OpSpec::Tick(Which::Current));
    path.spec(&OpSpec::Tick(Which::Current));
    path.spec(&OpSpec::Guided(0));
    path.spec(&OpSpec::Guided(0));
    path.draft("ノートを作りたい");
    path.spec(&OpSpec::Text);
    path.spec(&OpSpec::Stop);
    path.spec(&OpSpec::GoTo(1));
    path.spec(&OpSpec::Save);
    path.spec(&OpSpec::Restore(9));
    path.spec(&OpSpec::Restore(1));
    path.op(Op::Control(Control::OpenHint));
    path.draft("");
    path.draft("未送信の下書き");
    path
}

/// 台本の包みを書き出して読み戻したもの。
fn scripted_envelope(ir: &Ir, initial: &SessionState) -> SessionEnvelope {
    let RecordPath { state, rec, .. } = scripted(ir, initial);
    let env = build_envelope(ir, "s-1", rec.into_ops(), state).expect("build");
    roundtrip(&env).expect("roundtrip")
}

#[test]
fn empty_sample_verifies_to_its_snapshot() {
    let (ir, initial) = load_min_ir();
    let bytes = history_sample::read_bytes("empty");
    let env = load_json::<SessionEnvelope>(LoadEntry::History, &bytes).expect("load");
    assert_eq!(verify_envelope(&ir, initial.clone(), &env), Ok(initial));
}

#[test]
fn scripted_history_round_trips_and_verifies() {
    let (ir, initial) = load_min_ir();
    let env = scripted_envelope(&ir, &initial);
    assert!(env.ops.iter().any(|o| matches!(o, HistoryOp::Submit(_))));
    assert_eq!(
        verify_envelope(&ir, initial, &env),
        Ok(env.snapshot.clone())
    );
}

#[test]
fn tampered_snapshot_is_replay_mismatch() {
    let (ir, initial) = load_min_ir();
    let env = scripted_envelope(&ir, &initial);

    let mut next = env.clone();
    next.snapshot.next_checkpoint = CheckpointId(next.snapshot.next_checkpoint.0 + 1);
    let err = verify_envelope(&ir, initial.clone(), &next).expect_err("must mismatch");
    assert_eq!(
        err,
        VerifyError::Mismatch {
            field: SnapshotField::NextCheckpoint
        }
    );
    assert_eq!(err.load_reason(), Some(LoadReason::ReplayMismatch));

    let mut log = env;
    assert!(!log.snapshot.log.is_empty());
    log.snapshot.log.pop();
    assert_eq!(
        verify_envelope(&ir, initial, &log),
        Err(VerifyError::Mismatch {
            field: SnapshotField::Log
        })
    );
}

#[test]
fn tampered_ops_is_replay_mismatch() {
    let (ir, initial) = load_min_ir();
    let env = scripted_envelope(&ir, &initial);

    // 受理された送信を 1 件消すと、ログが変わる。
    let at = env
        .ops
        .iter()
        .position(|o| matches!(o, HistoryOp::Submit(_)))
        .expect("has a submit");
    let mut no_submit = env.clone();
    no_submit.ops.remove(at);
    let err = verify_envelope(&ir, initial.clone(), &no_submit).expect_err("must mismatch");
    assert!(matches!(err, VerifyError::Mismatch { .. }), "{err}");
    assert_eq!(err.load_reason(), Some(LoadReason::ReplayMismatch));

    // 末尾の未送信の下書きを消すと、下書きが変わる。
    let mut no_draft = env;
    let last = no_draft.ops.pop();
    assert!(matches!(last, Some(HistoryOp::Draft(_))));
    assert_eq!(
        verify_envelope(&ir, initial, &no_draft),
        Err(VerifyError::Mismatch {
            field: SnapshotField::Drafts
        })
    );
}

#[test]
fn replay_errors_are_typed() {
    let (ir, initial) = load_min_ir();
    let env = scripted_envelope(&ir, &initial);

    let mut blank = env.clone();
    blank.ops.push(HistoryOp::Submit("  ".to_owned()));
    let index = blank.ops.len() - 1;
    let err = verify_envelope(&ir, initial.clone(), &blank).expect_err("blank");
    assert_eq!(err, VerifyError::Replay(ReplayError::BlankSubmit { index }));
    assert_eq!(err.load_reason(), Some(LoadReason::ReplayMismatch));

    // 写しを更新せず `Reload` だけを足した包みは、世代とログが食い違う（書き換え・壊れ）。
    let mut reload = env;
    reload.ops.push(HistoryOp::Reload);
    let err = verify_envelope(&ir, initial.clone(), &reload).expect_err("reload");
    assert!(matches!(err, VerifyError::Mismatch { .. }), "{err}");
    assert_eq!(err.load_reason(), Some(LoadReason::ReplayMismatch));

    let mut bad = ir;
    bad.pack.profile = Some(serde_json::json!({ "command_prefix": "ab" }));
    let empty =
        load_json::<SessionEnvelope>(LoadEntry::History, &history_sample::read_bytes("empty"))
            .expect("load");
    let err = verify_envelope(&bad, initial, &empty).expect_err("profile");
    assert!(matches!(err, VerifyError::Replay(ReplayError::Profile(_))));
    assert_eq!(err.load_reason(), None);
}

#[test]
fn verified_state_is_the_replayed_state() {
    let (ir, initial) = load_min_ir();
    let env = scripted_envelope(&ir, &initial);
    let replayed = replay(&ir, initial.clone(), &env.ops).expect("replay");
    assert_eq!(verify_envelope(&ir, initial, &env), Ok(replayed));
}

#[test]
fn view_only_source_reads_the_snapshot_without_replay() {
    let (ir, initial) = load_min_ir();
    let mut env = scripted_envelope(&ir, &initial);
    env.snapshot.next_checkpoint = CheckpointId(env.snapshot.next_checkpoint.0 + 7);
    assert!(verify_envelope(&ir, initial, &env).is_err());
    let view = view_only_source(&env);
    assert!(std::ptr::eq(view.snapshot(), &env.snapshot));
}

#[test]
fn errors_do_not_carry_user_strings() {
    let (ir, initial) = load_min_ir();
    let secret = "秘密の下書きSECRET-0123";
    let mut env = scripted_envelope(&ir, &initial);
    env.ops.push(HistoryOp::Draft(secret.to_owned()));
    env.ops.push(HistoryOp::Submit(format!("{secret} ")));
    let err = verify_envelope(&ir, initial, &env).expect_err("must fail");
    let text = format!("{err} {err:?}");
    assert!(!text.contains("SECRET"), "{text}");
}

/// 戦略を 1,000 通り流す。ケースごとに、下書きの挟み方と再読み込みの時点 `t` を引く。
fn run_property<S, F>(name: &str, strategy: S, body: F)
where
    S: Strategy<Value = Vec<OpSpec>>,
    F: Fn(&Ir, &SessionState, &[OpSpec], &[Vec<u8>], usize) -> Result<(), String>,
{
    let (ir, initial) = load_min_ir();
    // 各枠の下書きは 0〜2 件。時点 t の状態に下書きが残る列・畳まれる列の両方を含める。
    // `t` は生の値で渡し、各本体が自分の取りうる範囲に丸める。
    let with_extra = (
        strategy,
        vec(vec(0u8..5, 0..=2), MAX_OPS + 1),
        0usize..1_000,
    );
    check(name, 1_000, with_extra, |(specs, drafts, t)| {
        match body(&ir, &initial, &specs, &drafts, t) {
            Ok(()) => Ok(()),
            Err(message) => {
                prop_assert!(false, "{message}");
                Ok(())
            }
        }
    });
}

/// 下書きを挟みながら列を流し、時点 `cut`（流した操作の数）の `ops` の接頭辞と状態を控える。
/// 戻りは（最後までの経路、時点の接頭辞と状態）。
fn run_recording<'a>(
    ir: &'a Ir,
    initial: &SessionState,
    specs: &[OpSpec],
    drafts: &[Vec<u8>],
    cut: usize,
) -> (RecordPath<'a>, (Vec<SessionOp>, SessionState)) {
    let mut path = RecordPath::new(ir, initial);
    let mut taken = None;
    for (i, spec) in specs.iter().enumerate() {
        if i == cut {
            taken = Some((path.rec.ops().to_vec(), path.state.clone()));
        }
        for d in drafts[i].iter().filter_map(|k| draft_text(*k)) {
            path.draft(d);
        }
        path.spec(spec);
    }
    if cut == specs.len() {
        taken = Some((path.rec.ops().to_vec(), path.state.clone()));
    }
    for d in drafts[specs.len()].iter().filter_map(|k| draft_text(*k)) {
        path.draft(d);
    }
    let taken = taken.unwrap_or_else(|| (Vec::new(), initial.clone()));
    (path, taken)
}

/// 書き出し→読み込み→照合が通り、写しと一致する。同じ包みの書き出しは同じバイト列になる。
fn export_import_verifies(
    ir: &Ir,
    initial: &SessionState,
    specs: &[OpSpec],
    drafts: &[Vec<u8>],
    _cut: usize,
) -> Result<(), String> {
    let (path, _) = run_recording(ir, initial, specs, drafts, 0);
    let RecordPath { state, rec, .. } = path;
    let env =
        build_envelope(ir, "s-1", rec.into_ops(), state).map_err(|e| format!("build: {e}"))?;
    let loaded = roundtrip(&env)?;
    // 同じ包みの書き出しは何度でも同じバイト列になる（試作の H-7）。
    let first = serde_json::to_vec(&env).map_err(|e| format!("export: {e}"))?;
    let second = serde_json::to_vec(&env).map_err(|e| format!("export: {e}"))?;
    if first != second {
        return Err("two exports of the same envelope differ".to_owned());
    }
    if Some(&first) != serde_json::to_vec(&loaded).ok().as_ref() {
        return Err("export is not byte-identical after the roundtrip".to_owned());
    }
    // 任意の時点の写しから作り直した実行は、その写しを返す（試作の H-2）。
    let rebuilt = Session::from_state(ir.clone(), loaded.snapshot.clone());
    if rebuilt.state() != &loaded.snapshot {
        return Err("a session rebuilt from the snapshot does not return it".to_owned());
    }
    match verify_envelope(ir, initial.clone(), &loaded) {
        Ok(s) if s == loaded.snapshot => Ok(()),
        Ok(_) => Err("verified state differs from the snapshot".to_owned()),
        Err(e) => Err(format!("verify failed: {e}")),
    }
}

/// 時点 t で再読み込みを挟む（H-5）。続けて流した結果と、時点 t の包みを書き出し→読み込みで復元して
/// 再読み込みしてから残りを流した結果が一致する。再読み込みの後の世代は時点 t の世代 +1 である。
fn reload_at_any_point(
    ir: &Ir,
    initial: &SessionState,
    specs: &[OpSpec],
    drafts: &[Vec<u8>],
    t: usize,
) -> Result<(), String> {
    let cut = t % (specs.len() + 1);
    // 経路 A: 時点 cut の接頭辞と状態を控えてから、再読み込みを挟んで最後まで流す。
    let mut path = RecordPath::new(ir, initial);
    let mut taken = None;
    for (i, spec) in specs.iter().enumerate() {
        if i == cut {
            taken = Some((path.rec.ops().to_vec(), path.state.clone()));
            path.reload();
        }
        for d in drafts[i].iter().filter_map(|k| draft_text(*k)) {
            path.draft(d);
        }
        path.spec(spec);
    }
    if cut == specs.len() {
        taken = Some((path.rec.ops().to_vec(), path.state.clone()));
        path.reload();
    }
    for d in drafts[specs.len()].iter().filter_map(|k| draft_text(*k)) {
        path.draft(d);
    }
    let (prefix, cut_state) = taken.unwrap_or_else(|| (Vec::new(), initial.clone()));
    let full = path.rec.ops().to_vec();
    let last = path.state;
    // 再読み込みを挟んだ列は、接頭辞の直後が `Reload` になる（下書きは再読み込みを挟んで畳まれない）。
    if full.get(..prefix.len()) != Some(prefix.as_slice())
        || full.get(prefix.len()) != Some(&HistoryOp::Reload)
    {
        return Err(format!("the prefix at t={cut} is not followed by a reload"));
    }
    // 経路 B: 時点 cut の包みを復元し、再読み込みしてから残りを流す。
    let env = build_envelope(ir, "s-1", prefix.clone(), cut_state.clone())
        .map_err(|e| format!("build: {e}"))?;
    let loaded = roundtrip(&env)?;
    let restored = verify_envelope(ir, initial.clone(), &loaded)
        .map_err(|e| format!("verify at t={cut}: {e}"))?;
    let mut rec = OpsRecorder::new();
    let (reloaded, _) = reload_recorded(restored, &mut rec);
    // 共有の関数が世代を上げ損ねても両経路が同じに壊れて一致するため、世代の増分は別に主張する。
    if cut_state.run.current.is_some()
        && reloaded.run.generation.0 != cut_state.run.generation.0 + 1
    {
        return Err(format!(
            "reload at t={cut} did not bump the generation by one"
        ));
    }
    match replay(ir, reloaded, &full[prefix.len() + 1..]) {
        Ok(s) if s == last => {}
        Ok(s) => {
            return Err(format!(
                "state after reload at t={cut} differs in {:?}",
                fandhe_workshop_core::history_replay::first_difference(&s, &last)
            ))
        }
        Err(e) => return Err(format!("replay the rest failed: {e}")),
    }
    // 再読み込みを含む全体の包みも、書き出して読み戻して照合が通り、写しと一致する。
    let full_env =
        build_envelope(ir, "s-1", full, last.clone()).map_err(|e| format!("build full: {e}"))?;
    let loaded = roundtrip(&full_env)?;
    match verify_envelope(ir, initial.clone(), &loaded) {
        Ok(s) if s == last => Ok(()),
        Ok(_) => Err("verified state with a reload differs from the last state".to_owned()),
        Err(e) => Err(format!("verify with a reload failed: {e}")),
    }
}

/// 書き出して読み戻した包みの `ops` を 1 件消すと、黙って誤った状態で開かない（試作の H-8）。
/// 開けた場合は写しと一致する（消した操作が状態に影響しない偶然だけ）。開けない場合は `replay_mismatch` になる。
fn tampered_ops_never_open_wrong_state(
    ir: &Ir,
    initial: &SessionState,
    specs: &[OpSpec],
    drafts: &[Vec<u8>],
    t: usize,
) -> Result<(), String> {
    let (path, _) = run_recording(ir, initial, specs, drafts, 0);
    let RecordPath { state, rec, .. } = path;
    let env =
        build_envelope(ir, "s-1", rec.into_ops(), state).map_err(|e| format!("build: {e}"))?;
    let mut loaded = roundtrip(&env)?;
    if loaded.ops.is_empty() {
        return Ok(());
    }
    let at = t % loaded.ops.len();
    loaded.ops.remove(at);
    match verify_envelope(ir, initial.clone(), &loaded) {
        Ok(s) if s == loaded.snapshot => Ok(()),
        Ok(_) => Err(format!("opened a wrong state after removing ops[{at}]")),
        Err(e) if e.load_reason() == Some(LoadReason::ReplayMismatch) => Ok(()),
        Err(e) => Err(format!("unexpected error after removing ops[{at}]: {e}")),
    }
}

#[test]
fn property_tampered_ops_never_open_wrong_state_fixed() {
    run_property(
        "history-verify-tampered",
        op_specs(),
        tampered_ops_never_open_wrong_state,
    );
}

#[test]
fn property_tampered_ops_never_open_wrong_state_guided() {
    run_property(
        "history-verify-tampered-guided",
        guided_op_specs(),
        tampered_ops_never_open_wrong_state,
    );
}

#[test]
fn property_export_import_verifies_fixed() {
    run_property("history-verify-export", op_specs(), export_import_verifies);
}

#[test]
fn property_export_import_verifies_guided() {
    run_property(
        "history-verify-export-guided",
        guided_op_specs(),
        export_import_verifies,
    );
}

#[test]
fn property_reload_at_any_point_fixed() {
    run_property("history-verify-reload", op_specs(), reload_at_any_point);
}

#[test]
fn property_reload_at_any_point_guided() {
    run_property(
        "history-verify-reload-guided",
        guided_op_specs(),
        reload_at_any_point,
    );
}
