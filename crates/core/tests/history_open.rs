//! 履歴を開く側の版と突き合わせる判定（`history_open`、TASK-174 1/3、VER-3〜VER-5）の試験。
//!
//! 履歴は現行のエンジンで試験の中で作る（PoC の固定の履歴は試作の写しの形で、現行の `SessionState` では
//! 読めない）。教材の変種は min-3step の JSON を書き換えて `load_ir` で読み、`testdata/` は増やさない。
//! 版の欄だけを変える変種は、包みの `versions` を直接書き換える（PoC と同じ作り物）。
//! `content_major`・`format_older`・`engine_major` の厳密な確認は TASK-174 2/3、`vocab_newer`・`format_newer`・
//! `engine_newer` の厳密な確認は 3/3 の範囲で、ここでは「開かないこと・状態を作らないこと」だけを見張る。

mod common;

use std::collections::BTreeSet;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{OpSpec, Which};
use common::record_path::RecordPath;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::{
    Disposition, HistoryVersions, Version3, VersionAxis, VocabVersion,
};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::history_envelope::{build_envelope, SessionEnvelope};
use fandhe_workshop_core::history_open::{open_history, Opened, OpeningCaps};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::version_record::{record_versions, record_versions_with};
use serde_json::{json, Value};

const START: &str = "s-1";

fn base_json() -> Value {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    serde_json::from_slice(&std::fs::read(path).expect("read min-3step")).expect("parse")
}

fn ir_of(v: &Value) -> Ir {
    load_ir(&serde_json::to_vec(v).expect("to_vec")).expect("load_ir")
}

fn initial() -> SessionState {
    load().initial
}

fn caps() -> OpeningCaps {
    OpeningCaps::with_vocab_max(VocabVersion { major: 1, minor: 3 })
}

fn versions_of(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

/// 教材の変種。`edit` で JSON を書き換えてから読む。
fn variant(edit: impl FnOnce(&mut Value)) -> Ir {
    let mut v = base_json();
    edit(&mut v);
    ir_of(&v)
}

fn current(state: &SessionState) -> String {
    state
        .run
        .current
        .as_ref()
        .map(|u| u.0.clone())
        .expect("current")
}

/// 開始点から、Step 1 の中で処理を終えるまで（確認の前）進めた記録の経路。
fn within_st1(ir: &Ir) -> RecordPath<'_> {
    let mut path = RecordPath::new(ir, &initial());
    path.draft("/make-note first-note");
    path.spec(&OpSpec::Cmd(0));
    for _ in 0..5 {
        path.spec(&OpSpec::Tick(Which::Current));
    }
    assert_eq!(current(&path.state), "main/st-1");
    path
}

/// Step 1 を達成して次へ進むまで進めた記録の経路。
fn beyond_st1(ir: &Ir) -> RecordPath<'_> {
    let mut path = within_st1(ir);
    path.spec(&OpSpec::Ui(0));
    assert_ne!(current(&path.state), "main/st-1", "the step must advance");
    path
}

fn envelope(ir: &Ir, path: RecordPath<'_>) -> SessionEnvelope {
    let RecordPath { state, rec, .. } = path;
    build_envelope(ir, START, rec.into_ops(), state).expect("build")
}

fn open(ir: &Ir, h: &SessionEnvelope) -> Opened {
    open_history(ir, &versions_of(ir), &caps(), initial(), h)
}

/// 変換して開いた結果の、版の違いの軸の名前の集合。
fn axes(o: &Opened) -> BTreeSet<String> {
    let Opened::Converted { notes, .. } = o else {
        panic!("expected converted, got {:?}", o.verdict().disposition());
    };
    assert!(!notes.diffs().is_empty());
    notes
        .diffs()
        .iter()
        .map(|d| format!("{:?}", d.axis))
        .collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| (*s).to_owned()).collect()
}

fn assert_closed(o: &Opened, disposition: Disposition, code: LoadReason) {
    assert_eq!(o.verdict().disposition(), disposition);
    assert!(o.state().is_none());
    let got = match o {
        Opened::ViewOnly { code } | Opened::Refused { code } => *code,
        _ => panic!("expected a closed result"),
    };
    assert_eq!(got, code);
    assert!(!got.as_str().is_empty());
}

/// `st-1` と `st-2` の間に、`st-2` を写した Step を足す（副版で Step を足した教材）。
fn add_step(v: &mut Value) {
    let nodes = v["nodes"].as_array_mut().expect("nodes");
    let mut added = nodes[1].clone();
    added["uid"] = json!("main/st-1b");
    added["step"]["id"] = json!("st-1b");
    added["step"]["next"] = json!([{ "to": "main/st-2", "when": null }]);
    nodes[0]["step"]["next"] = json!([{ "to": "main/st-1b", "when": null }]);
    nodes.insert(1, added);
    v["pack"]["version"] = json!("0.2.0");
}

#[test]
fn ver3_same_versions_open_without_conversion() {
    let ir = ir_of(&base_json());
    let h = envelope(&ir, beyond_st1(&ir));
    let o = open(&ir, &h);
    assert_eq!(o.verdict().disposition(), Disposition::Open);
    assert_eq!(o.state(), Some(&h.snapshot));
}

#[test]
fn ver4_content_minor_with_changed_text_is_converted() {
    let old = ir_of(&base_json());
    let h = envelope(&old, beyond_st1(&old));
    let new = variant(|v| {
        v["pack"]["version"] = json!("0.1.0");
        v["hints"]["h-1-1"]["text"] = json!("ノートを作る場面です。");
    });
    let o = open(&new, &h);
    assert_eq!(axes(&o), set(&["Content", "ContentHash"]));
    assert_eq!(o.state(), Some(&h.snapshot));
}

#[test]
fn ver4_content_patch_only_is_converted() {
    let old = ir_of(&base_json());
    let h = envelope(&old, beyond_st1(&old));
    let new = variant(|v| v["pack"]["version"] = json!("0.0.2"));
    let o = open(&new, &h);
    assert!(axes(&o).contains("Content"));
    assert_eq!(o.state(), Some(&h.snapshot));
}

#[test]
fn ver4_vocab_declaration_is_converted() {
    let old = ir_of(&base_json());
    let h = envelope(&old, beyond_st1(&old));
    let new = variant(|v| v["pack"]["vocab"] = json!("1.2"));
    let o = open(&new, &h);
    assert!(axes(&o).contains("Vocab"));
    assert_eq!(o.state(), Some(&h.snapshot));
}

#[test]
fn ver4_engine_minor_is_converted() {
    let ir = ir_of(&base_json());
    let h = envelope(&ir, beyond_st1(&ir));
    let hash = content_hash(&ir).expect("hash");
    let opening = record_versions_with(&ir, Version3::parse("0.1.0").unwrap(), &hash).unwrap();
    let o = open_history(&ir, &opening, &caps(), initial(), &h);
    assert_eq!(axes(&o), set(&["Engine"]));
    assert_eq!(o.state(), Some(&h.snapshot));
}

#[test]
fn ver4_ref_commit_is_converted() {
    let ir = ir_of(&base_json());
    let mut h = envelope(&ir, beyond_st1(&ir));
    h.versions.refs.insert("x".to_owned(), "aaa".to_owned());
    let mut opening = versions_of(&ir);
    opening.refs.insert("x".to_owned(), "bbb".to_owned());
    let o = open_history(&ir, &opening, &caps(), initial(), &h);
    assert_eq!(axes(&o), set(&["Refs"]));
    let Opened::Converted { notes, .. } = &o else {
        unreachable!()
    };
    assert_eq!(notes.diffs()[0].ref_name.as_deref(), Some("x"));
}

#[test]
fn ver4_converted_state_is_saved_with_the_opening_versions() {
    let old = ir_of(&base_json());
    let h = envelope(&old, beyond_st1(&old));
    let new = variant(|v| v["pack"]["version"] = json!("0.0.2"));
    let o = open(&new, &h);
    let state = o.state().expect("converted").clone();
    let saved = build_envelope(&new, START, Vec::new(), state).expect("build");
    assert_eq!(saved.versions, versions_of(&new));
}

#[test]
fn ver5_step_added_by_minor_keeps_early_history_and_diverges_after() {
    let old = ir_of(&base_json());
    let early = envelope(&old, within_st1(&old));
    let late = envelope(&old, beyond_st1(&old));
    let new = variant(add_step);

    let o = open(&new, &late);
    assert_closed(&o, Disposition::ViewOnly, LoadReason::ReplayDiverged);

    let o = open(&new, &early);
    assert!(axes(&o).contains("Content"));
    // 続きは足した Step を通る。
    let state = o.state().expect("converted").clone();
    let mut path = RecordPath::new(&new, &state);
    path.spec(&OpSpec::Ui(0));
    assert_eq!(current(&path.state), "main/st-1b");
}

#[test]
fn replay_mismatch_is_separated_from_diverged() {
    let ir = ir_of(&base_json());
    let mut h = envelope(&ir, beyond_st1(&ir));
    h.snapshot
        .drafts
        .insert(StepUid("main/st-1".to_owned()).root(), "改ざん".to_owned());
    assert_closed(
        &open(&ir, &h),
        Disposition::Refused,
        LoadReason::ReplayMismatch,
    );

    // 教材の副版の違いがあり、写しも一致しないときは版の違いで説明できるので表示だけ。
    let new = variant(|v| v["pack"]["version"] = json!("0.0.2"));
    assert_closed(
        &open(&new, &h),
        Disposition::ViewOnly,
        LoadReason::ReplayDiverged,
    );

    // 参照元の commit だけが違い、写しも一致しないときも、版の違いで説明できるので表示だけ。
    let mut opening = versions_of(&ir);
    opening.refs.insert("x".to_owned(), "bbb".to_owned());
    let o = open_history(&ir, &opening, &caps(), initial(), &h);
    assert_closed(&o, Disposition::ViewOnly, LoadReason::ReplayDiverged);
}

#[test]
fn fail_closed_for_versions_that_must_not_open() {
    let ir = ir_of(&base_json());
    let base = envelope(&ir, within_st1(&ir));
    let check = |edit: &dyn Fn(&mut SessionEnvelope), ir: &Ir| {
        let mut h = base.clone();
        edit(&mut h);
        let o = open(ir, &h);
        assert!(matches!(
            o.verdict().disposition(),
            Disposition::ViewOnly | Disposition::Refused
        ));
        assert!(o.state().is_none());
    };
    check(&|h| h.versions.content = "1.0.0".to_owned(), &ir);
    check(&|h| h.versions.engine = "1.0.0".to_owned(), &ir);
    check(&|h| h.versions.engine = "0.0.1".to_owned(), &ir);
    check(&|h| h.versions.ir_format = 3, &ir);
    check(&|h| h.versions.vocab = "1.4".to_owned(), &ir);
    check(&|h| h.versions.vocab = "x".to_owned(), &ir);
    // 開く側の教材の主版が違う。
    let major = variant(|v| v["pack"]["version"] = json!("1.0.0"));
    check(&|_| {}, &major);
}

#[test]
fn envelope_level_refusals_and_missing_start() {
    let ir = ir_of(&base_json());
    let base = envelope(&ir, within_st1(&ir));

    let mut h = base.clone();
    h.pack_id = "other".to_owned();
    assert_closed(&open(&ir, &h), Disposition::Refused, LoadReason::OtherPack);

    let mut h = base.clone();
    h.history_format += 1;
    assert_closed(
        &open(&ir, &h),
        Disposition::Refused,
        LoadReason::HistoryFormatNewer,
    );

    let mut h = base;
    h.start = "s-none".to_owned();
    assert_closed(
        &open(&ir, &h),
        Disposition::ViewOnly,
        LoadReason::StartMissing,
    );
}

#[test]
fn axis_names_match_the_debug_names_used_here() {
    // `axes` が Debug 名に依存するため、軸の名前が変わったら気付けるようにする。
    let names: BTreeSet<String> = VersionAxis::ALL.iter().map(|a| format!("{a:?}")).collect();
    assert_eq!(
        names,
        set(&[
            "Engine",
            "IrFormat",
            "Vocab",
            "Content",
            "ContentHash",
            "Refs"
        ])
    );
}
