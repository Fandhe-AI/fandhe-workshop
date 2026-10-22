//! ホストの履歴ファイルの読み込み（`EngineHost::open_history`、TASK-330 2/2。SAVE-16・SAVE-19）の試験。
//!
//! 開けるときは `Session` が適用し直した状態に差し替わり、導出した値（projection・world）が一括の導出と一致する。
//! 開けないときは、状態・導出した値・dirty のどれも変わらない。契約の試料（`two-rounds`）は再読み込みの意味の実装前の
//! 写しで開けないため、試料の `ops` を適用し直した結果を写しにした「開ける包み」を、この試験の中で作る。
//! `tests/` は `no_progress.rs` の対象外である。試験では `unwrap` を使ってよい。

#[path = "common/secret_values.rs"]
mod secret_values;

use std::path::PathBuf;

use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::addr::{StepUid, TickAddr};
use fandhe_workshop_core::changed::replay_changes;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::world::World;
use fandhe_workshop_core::{load_ir, Projection, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction, ImportOutcome};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn ir() -> Ir {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    load_ir(&bytes).unwrap()
}

fn initial() -> SessionState {
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    serde_json::from_value(v["initial"].clone()).unwrap()
}

fn host() -> EngineHost {
    EngineHost::with_session(Session::from_state(ir(), initial()), standard_registry()).unwrap()
}

/// 試料の `ops` を適用し直した結果を写しにした、開ける包みの JSON と、その写し。
fn loadable() -> (Vec<u8>, SessionState) {
    let ir = ir();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state.clone()).unwrap();
    (export_bytes(&built).unwrap(), state)
}

fn state_of(host: &EngineHost) -> SessionState {
    host.model().session().unwrap().state().clone()
}

/// ホストが持つ導出した値が、ログ全体から一括で導いた値と一致するか。
fn derived_matches(host: &EngineHost) -> bool {
    let log = host.model().session().unwrap().log();
    let mut world = World::new();
    replay_changes(&mut world, log);
    host.model().projection() == &Projection::from_events(log)
        && host.model().world() == &world
        && host.model().world_consumed() == log.len()
}

/// 試料の入力の語を `word` に置き換えた、開ける包み。`word` は JSON の文字列にそのまま入れられる文字だけで作る。
fn loadable_with(word: &str) -> Vec<u8> {
    let ir = ir();
    let text = std::fs::read_to_string(testdata().join("contract/history/two-rounds.json"))
        .unwrap()
        .replace("first-note", word);
    let env = parse_history_envelope(parse_history_json(text.as_bytes()).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    export_bytes(&built).unwrap()
}

/// 履歴の読み込みは中身の秘密情報を検査しない（SAFE-30・決定 D-45 で拡大を見送り。TASK-165 2/2）。
///
/// 読み込みの前後で警告は出ず、保存の口も呼ばれない。読み込んだ後の最初の保存で、保存の直前の関門が履歴の
/// 全体を検査して保存を止め、形の ID だけを警告に入れる。見送りの現状を固定し、規則を変えるときに気づけるようにする。
/// 値は部品から組み立て、assert のメッセージには試験データの ID だけを入れる。
#[test]
fn opening_a_history_does_not_warn_and_the_first_save_after_it_does() {
    use std::cell::Cell;
    use std::rc::Rc;

    let (id, expect, value) = secret_values::added_shapes().remove(3);
    let bytes = loadable_with(&value);
    let mut h = host();
    let calls = Rc::new(Cell::new(0));
    let c2 = Rc::clone(&calls);
    h.set_history_saver(Box::new(move |_| {
        c2.set(c2.get() + 1);
        Ok(())
    }));
    assert_eq!(
        h.open_history(&bytes),
        ImportOutcome::Open,
        "{id}: not opened"
    );
    assert!(
        h.model().secret_warning().is_empty(),
        "{id}: warned on open"
    );
    assert_eq!(calls.get(), 0, "{id}: saver called on open");

    let act = |h: &mut EngineHost, name: &str, payload: &str| {
        h.update(EngineHost::decode_action(name, payload).unwrap());
    };
    act(&mut h, "draft", "/make-note second-note");
    act(&mut h, "submit", "");
    let ids: Vec<&str> = h.model().secret_warning().iter().map(|s| s.id()).collect();
    assert!(
        ids == [expect.as_str()],
        "{id}: warning after the first save differs"
    );
    assert_eq!(calls.get(), 0, "{id}: saver called while held");
}

#[test]
fn open_replaces_the_session_with_the_snapshot_and_rederives_views() {
    let (bytes, snapshot) = loadable();
    let mut h = host();
    assert_ne!(state_of(&h), snapshot, "starts from a different state");
    assert_eq!(h.open_history(&bytes), ImportOutcome::Open);
    assert_eq!(state_of(&h), snapshot);
    assert!(derived_matches(&h));
    assert!(!h.model().projection().review().is_empty());
}

#[test]
fn open_marks_every_field_dirty_and_notifies_the_next_tick() {
    let (bytes, _) = loadable();
    let mut h = host();
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<Option<TickAddr>>::new()));
    let sink = seen.clone();
    h.set_tick_request(Box::new(move |next| sink.borrow_mut().push(next)));
    let notified = seen.borrow().len();
    assert_eq!(h.open_history(&bytes), ImportOutcome::Open);
    let mut expected: Vec<&str> = h.registry().fields().map(|f| f.name).collect();
    // `shell_notices` は registry の外にあり、`open` が前の通知を消すために明示して積む（FEND-15）。
    expected.push("shell_notices");
    let mut dirty: Vec<&str> = h.dirty_fields().to_vec();
    expected.sort_unstable();
    dirty.sort_unstable();
    assert_eq!(dirty, expected);
    assert_eq!(
        seen.borrow().len(),
        notified + 1,
        "the next tick is notified once"
    );
}

#[test]
fn open_then_continue_extends_the_log() {
    let (bytes, _) = loadable();
    let mut h = host();
    assert_eq!(h.open_history(&bytes), ImportOutcome::Open);
    let before = h.model().session().unwrap().log().len();
    h.update(HostAction::Engine(Op::Control(Control::GoTo(StepUid(
        "main/st-2".to_owned(),
    )))));
    assert!(h.model().session().unwrap().log().len() > before);
    assert!(derived_matches(&h));
}

fn assert_untouched(h: &mut EngineHost, bytes: &[u8], reason: LoadReason) {
    let state = state_of(h);
    let projection = h.model().projection().clone();
    let world = h.model().world().clone();
    let outcome = h.open_history(bytes);
    assert_eq!(outcome.reason(), Some(reason));
    assert!(!outcome.is_loaded());
    assert_eq!(state_of(h), state, "state is unchanged");
    assert_eq!(h.model().projection(), &projection);
    assert_eq!(h.model().world(), &world);
    // 開かない理由は通知欄にだけ出る（TASK-176 2/2）。状態・導出した値の field は積まない。
    assert_eq!(
        h.dirty_fields(),
        vec!["shell_notices"],
        "only the notice is dirty"
    );
}

#[test]
fn tampered_snapshot_is_refused_and_leaves_everything_unchanged() {
    let (bytes, _) = loadable();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["snapshot"]["next_checkpoint"] = serde_json::json!(9999);
    let mut h = host();
    assert_untouched(
        &mut h,
        &serde_json::to_vec(&v).unwrap(),
        LoadReason::ReplayMismatch,
    );
}

#[test]
fn not_json_is_refused_and_leaves_everything_unchanged() {
    let mut h = host();
    assert_untouched(&mut h, b"not json", LoadReason::NotJson);
}

#[test]
fn other_pack_is_refused_and_leaves_everything_unchanged() {
    let (bytes, _) = loadable();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["pack_id"] = serde_json::json!("another-pack");
    let mut h = host();
    assert_untouched(
        &mut h,
        &serde_json::to_vec(&v).unwrap(),
        LoadReason::OtherPack,
    );
}

#[test]
fn host_without_a_session_is_unavailable() {
    let (bytes, _) = loadable();
    let mut h = EngineHost::new();
    assert_eq!(h.open_history(&bytes), ImportOutcome::Unavailable);
    assert!(h.model().session().is_none());
}

#[test]
fn opening_versions_match_the_loaded_pack() {
    // 開く側の版は、いま載せている教材の IR から求める。書き出し側（build_envelope）と同じ求め方であることの確認。
    let ir = ir();
    let hash = content_hash(&ir).unwrap();
    let opening: HistoryVersions = record_versions(&ir, &hash).unwrap();
    let (bytes, _) = loadable();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["versions"]["content_hash"], opening.content_hash.as_str());
}

#[cfg(feature = "test-hooks")]
mod raw_action {
    use super::*;
    use fandhe_workshop_web::host::RAW_OPEN_HISTORY;

    fn payload(body: &[u8]) -> String {
        format!(
            "{RAW_OPEN_HISTORY}{}",
            String::from_utf8(body.to_vec()).unwrap()
        )
    }

    #[test]
    fn prefix_routes_to_open_history_and_the_result_is_recorded() {
        let (bytes, snapshot) = loadable();
        let mut h = host();
        let action = EngineHost::decode_action("raw", &payload(&bytes)).unwrap();
        assert!(matches!(action, HostAction::OpenHistory(_)));
        h.update(action);
        assert_eq!(state_of(&h), snapshot);
        assert_eq!(h.last_import(), Some(ImportOutcome::Open));
    }

    #[test]
    fn only_the_exact_prefix_routes_to_open_history() {
        for p in [
            "open-history",
            "Open-History:{}",
            " open-history:{}",
            "xopen-history:{}",
        ] {
            let a = EngineHost::decode_action("raw", p);
            assert!(!matches!(a, Some(HostAction::OpenHistory(_))), "{p}");
        }
    }

    #[test]
    fn a_refusal_is_recorded_with_a_fixed_code_only() {
        let mut h = host();
        let action = EngineHost::decode_action("raw", &payload(b"not json")).unwrap();
        h.update(action);
        assert_eq!(
            h.last_import(),
            Some(ImportOutcome::Refused(LoadReason::NotJson))
        );
    }
}
