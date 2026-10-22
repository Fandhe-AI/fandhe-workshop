//! 履歴を開く側の版と突き合わせる判定（`history_open`、TASK-174、VER-3〜VER-7）の試験。
//!
//! 履歴は現行のエンジンで試験の中で作る（PoC の固定の履歴は試作の写しの形で、現行の `SessionState` では
//! 読めない）。教材の変種は min-3step の JSON を書き換えて `load_ir` で読み、`testdata/` は増やさない。
//! 版の欄だけを変える変種は、包みの `versions` を直接書き換える（PoC と同じ作り物）。
//! `content_major`・`format_older`・`engine_major`（表示だけになる 3 種。TASK-174 2/3）は、理由コードの値・向き・
//! 境界・判定の順序を `ver6_*`・`ver7_*` で固定する。`vocab_newer`・`format_newer`・`engine_newer`（開かない 3 種。
//! TASK-174 3/3）は、向き・境界・順序・理由の分類を `ver7_*`・`refused_*` で固定する。
//! VER-18（D-52・D-59）の線引きのうち採らなかった案は、変異 `VER18-1`・`VER18-2` でも固定する（TASK-177 1/2）。

mod common;

use std::collections::BTreeSet;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{OpSpec, Which};
use common::record_path::RecordPath;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::{
    Disposition, HistoryVersions, Version3, VersionAxis, VocabVersion,
};
use fandhe_workshop_contract::history_version::{ReasonCode, ReasonKind};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::load_reason::{HistoryLoadStage, LoadReason, HISTORY_LOAD_ORDER};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::history_envelope::{build_envelope, SessionEnvelope};
use fandhe_workshop_core::history_open::{open_history, Opened, OpeningCaps, VOCAB_MAX};
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
    OpeningCaps::current()
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
    check(&|h| h.versions.vocab = "1.7".to_owned(), &ir);
    check(&|h| h.versions.vocab = "x".to_owned(), &ir);
    // 開く側の教材の主版が違う。
    let major = variant(|v| v["pack"]["version"] = json!("1.0.0"));
    check(&|_| {}, &major);
}

#[test]
fn history_ir_format_newer_than_opening_pack_is_refused() {
    // 履歴が format 2、開く教材が format 1 の場合は、変換に進まず閉じたままにする。
    let mut ir = ir_of(&base_json());
    let mut h = envelope(&ir, within_st1(&ir));
    h.versions.ir_format = 2;
    ir.format_version = 1;
    assert_closed(
        &open(&ir, &h),
        Disposition::Refused,
        LoadReason::FormatNewer,
    );
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

/// 開く側のエンジンの版を模して開く（PoC V-v-3 と同じ作り物）。
fn open_as_engine(ir: &Ir, engine: &str, h: &SessionEnvelope) -> Opened {
    let hash = content_hash(ir).expect("hash");
    let opening = record_versions_with(ir, Version3::parse(engine).unwrap(), &hash).unwrap();
    open_history(ir, &opening, &caps(), initial(), h)
}

/// 履歴の版の欄だけを書き換えた包み（作り物）。
fn edited(h: &SessionEnvelope, edit: impl FnOnce(&mut SessionEnvelope)) -> SessionEnvelope {
    let mut h = h.clone();
    edit(&mut h);
    h
}

/// `st-2` を取り除いて `st-1` を `st-3` へつなぎ、教材の版を `version` にした変種。
fn remove_step(v: &mut Value, version: &str) {
    let nodes = v["nodes"].as_array_mut().expect("nodes");
    nodes.remove(1);
    nodes[0]["step"]["next"] = json!([{ "to": "main/st-3", "when": null }]);
    v["pack"]["version"] = json!(version);
}

/// IR の構造の版 2 を模した教材（`format_version` と `pack.ir_format` を両方 2 にする）。
fn format2(extra: impl FnOnce(&mut Value)) -> Ir {
    variant(|v| {
        v["format_version"] = json!(2);
        v["pack"]["ir_format"] = json!(2);
        extra(v);
    })
}

#[test]
fn ver6_content_major_with_removed_step_is_view_only_at_any_position() {
    let old = ir_of(&base_json());
    let early = envelope(&old, within_st1(&old));
    let late = envelope(&old, beyond_st1(&old));
    let major = variant(|v| remove_step(v, "1.0.0"));
    // 手前の履歴も、先の履歴も、主版が違えば表示だけ（位置によらない）。
    assert_closed(
        &open(&major, &early),
        Disposition::ViewOnly,
        LoadReason::ContentMajor,
    );
    assert_closed(
        &open(&major, &late),
        Disposition::ViewOnly,
        LoadReason::ContentMajor,
    );
    // 対照: 主版が同じなら、手前の履歴は変換して開く。手前の履歴が開かないのは主版の違いのため。
    let minor = variant(|v| remove_step(v, "0.0.2"));
    assert!(axes(&open(&minor, &early)).contains("Content"));
}

#[test]
fn ver6_content_major_boundaries() {
    let old = ir_of(&base_json());
    let h = envelope(&old, within_st1(&old));
    let at = |version: &str| variant(|v| v["pack"]["version"] = json!(version));
    assert_closed(
        &open(&at("1.0.0"), &h),
        Disposition::ViewOnly,
        LoadReason::ContentMajor,
    );
    assert!(axes(&open(&at("0.99.99"), &h)).contains("Content"));
    // 履歴の側の主版が新しい向きも表示だけ（向きを問わない）。
    let newer = edited(&h, |h| h.versions.content = "1.0.0".to_owned());
    assert_closed(
        &open(&old, &newer),
        Disposition::ViewOnly,
        LoadReason::ContentMajor,
    );
    // 主版が同じなら、副版・修正版がどちら向きに違っても段 6 を通る。
    let new2 = at("2.0.0");
    let same = edited(&h, |h| h.versions.content = "2.5.3".to_owned());
    assert!(axes(&open(&new2, &same)).contains("Content"));
    let lower = edited(&h, |h| h.versions.content = "1.9.9".to_owned());
    assert_closed(
        &open(&new2, &lower),
        Disposition::ViewOnly,
        LoadReason::ContentMajor,
    );
}

#[test]
fn ver7_format_older_is_view_only() {
    let old = ir_of(&base_json());
    let h = envelope(&old, within_st1(&old));
    // V-iv-4: 履歴の IR の版が 0。
    let zero = edited(&h, |h| h.versions.ir_format = 0);
    assert_closed(
        &open(&old, &zero),
        Disposition::ViewOnly,
        LoadReason::FormatOlder,
    );
    // 履歴 1・開く教材 2（変換の規則を持たない。D-52）。
    let new = format2(|_| {});
    assert_closed(
        &open(&new, &h),
        Disposition::ViewOnly,
        LoadReason::FormatOlder,
    );
    // 境界: 履歴 2・開く教材 2 は FormatOlder にならない。
    let h2 = envelope(&new, within_st1(&new));
    assert_eq!(open(&new, &h2).verdict().disposition(), Disposition::Open);
    let new2 = format2(|v| v["pack"]["version"] = json!("0.0.2"));
    assert_eq!(axes(&open(&new2, &h2)), set(&["Content", "ContentHash"]));
}

#[test]
fn ver7_engine_major_is_view_only() {
    let ir = ir_of(&base_json());
    let h = envelope(&ir, within_st1(&ir));
    let with_engine = |e: &str| edited(&h, |h| h.versions.engine = e.to_owned());
    // V-v-3: 履歴 0.0.0・開く側 1.0.0。
    assert_closed(
        &open_as_engine(&ir, "1.0.0", &h),
        Disposition::ViewOnly,
        LoadReason::EngineMajor,
    );
    // 境界: 主版が同じで開く側が新しい副版なら変換して開く。主版を跨ぐと表示だけ。
    assert_eq!(
        axes(&open_as_engine(&ir, "1.2.0", &with_engine("1.0.0"))),
        set(&["Engine"])
    );
    assert_closed(
        &open_as_engine(&ir, "2.0.0", &with_engine("1.9.9")),
        Disposition::ViewOnly,
        LoadReason::EngineMajor,
    );
    // 向き: 履歴の側が新しいときは engine_newer が先に当たり、開かない。
    assert_closed(
        &open_as_engine(&ir, "0.0.0", &with_engine("1.0.0")),
        Disposition::Refused,
        LoadReason::EngineNewer,
    );
}

#[test]
fn ver7_view_only_order_follows_the_contract() {
    let old = ir_of(&base_json());
    let h = envelope(&old, within_st1(&old));
    let major = variant(|v| v["pack"]["version"] = json!("1.0.0"));
    let older = edited(&h, |h| h.versions.ir_format = 0);
    let view = |o: &Opened, code| assert_closed(o, Disposition::ViewOnly, code);
    // format_older > engine_major > content_major。
    view(
        &open_as_engine(&major, "1.0.0", &older),
        LoadReason::FormatOlder,
    );
    view(
        &open_as_engine(&major, "1.0.0", &h),
        LoadReason::EngineMajor,
    );
    // 段 6 は段 7（開始点）・段 8（写しの照合）より先。
    let no_start = edited(&h, |h| h.start = "s-none".to_owned());
    view(&open(&major, &no_start), LoadReason::ContentMajor);
    let mut tampered = h.clone();
    tampered
        .snapshot
        .drafts
        .insert(StepUid("main/st-1".to_owned()).root(), "改ざん".to_owned());
    view(&open(&major, &tampered), LoadReason::ContentMajor);
    // 「新しい」側は 3 種より先（厳密な試験は 3/3）。
    let vocab = edited(&older, |h| h.versions.vocab = "1.7".to_owned());
    assert_closed(
        &open(&old, &vocab),
        Disposition::Refused,
        LoadReason::VocabNewer,
    );
}

#[test]
fn view_only_reasons_are_non_empty_and_classified() {
    let old = ir_of(&base_json());
    let h = envelope(&old, within_st1(&old));
    let major = variant(|v| v["pack"]["version"] = json!("1.0.0"));
    let older = edited(&h, |h| h.versions.ir_format = 0);
    let cases = [
        (
            open(&major, &h),
            ReasonKind::MajorDiffers(VersionAxis::Content),
        ),
        (
            open(&old, &older),
            ReasonKind::HistoryOlder(VersionAxis::IrFormat),
        ),
        (
            open_as_engine(&old, "1.0.0", &h),
            ReasonKind::MajorDiffers(VersionAxis::Engine),
        ),
    ];
    for (o, kind) in cases {
        assert_eq!(o.verdict().disposition(), Disposition::ViewOnly);
        let code = o.verdict().code().copied().expect("code");
        assert!(!code.as_str().is_empty());
        assert_eq!(code.kind(), Some(kind));
        assert!(o.state().is_none());
    }
}

fn vocab(major: u32, minor: u32) -> VocabVersion {
    VocabVersion { major, minor }
}

/// 語彙の上限を `max` に模して、`h` を開く。
fn open_with_vocab_max(ir: &Ir, max: VocabVersion, h: &SessionEnvelope) -> Opened {
    open_history(
        ir,
        &versions_of(ir),
        &OpeningCaps::with_vocab_max(max),
        initial(),
        h,
    )
}

fn refused(o: &Opened, code: LoadReason) {
    assert_closed(o, Disposition::Refused, code);
}

#[test]
fn ver7_format_newer_is_refused() {
    let old = ir_of(&base_json());
    let h1 = envelope(&old, within_st1(&old));
    let new = format2(|_| {});
    let h2 = envelope(&new, within_st1(&new));
    let with_format = |h: &SessionEnvelope, f: u32| edited(h, |h| h.versions.ir_format = f);
    let cap1 = OpeningCaps {
        ir_format_max: 1,
        vocab_max: VOCAB_MAX,
    };
    let open_cap = |ir: &Ir, caps: &OpeningCaps, h: &SessionEnvelope| {
        open_history(ir, &versions_of(ir), caps, initial(), h)
    };
    // (a) エンジンの上限だけを超える（開く教材は 2 だが、エンジンの上限は 1）。
    refused(
        &open_cap(&new, &cap1, &with_format(&h2, 2)),
        LoadReason::FormatNewer,
    );
    // 履歴 3・上限 2・開く教材 2。
    refused(&open(&new, &with_format(&h2, 3)), LoadReason::FormatNewer);
    // (b) 開く教材の IR の版だけを超える（履歴 2・開く教材 1・上限 2。V-iv-3）。
    let mut one = ir_of(&base_json());
    one.format_version = 1;
    refused(&open(&one, &with_format(&h1, 2)), LoadReason::FormatNewer);
    // (c) 境界: 履歴 = 開く教材 = 上限なら新しいとは見ない。
    assert_eq!(open(&new, &h2).verdict().disposition(), Disposition::Open);
    assert_eq!(
        open_cap(&old, &cap1, &h1).verdict().disposition(),
        Disposition::Open
    );
}

#[test]
fn ver7_vocab_newer_is_refused() {
    let ir = ir_of(&base_json());
    let h = envelope(&ir, within_st1(&ir));
    let hv = |v: &str| edited(&h, |h| h.versions.vocab = v.to_owned());
    // V-iv-2: 履歴 1.2・上限 1.0。
    refused(
        &open_with_vocab_max(&ir, vocab(1, 0), &hv("1.2")),
        LoadReason::VocabNewer,
    );
    // 副版・主版が新しい。
    refused(&open(&ir, &hv("1.7")), LoadReason::VocabNewer);
    refused(&open(&ir, &hv("2.0")), LoadReason::VocabNewer);
    // 等しい版は開く側に入り、段 6 を通って写しと一致すれば変換して開く。
    assert!(axes(&open(&ir, &hv("1.6"))).contains("Vocab"));
    // 数値の比較（文字列の辞書順ではない）。
    assert!(axes(&open_with_vocab_max(&ir, vocab(1, 10), &hv("1.9"))).contains("Vocab"));
    refused(
        &open_with_vocab_max(&ir, vocab(1, 9), &hv("1.10")),
        LoadReason::VocabNewer,
    );
}

#[test]
fn ver7_vocab_newer_on_the_opening_side_is_refused() {
    let ir = ir_of(&base_json());
    let h = envelope(&ir, within_st1(&ir));
    // 履歴は 1.0 のままで、開く側の語彙の版だけが上限を超える（作り物）。
    let mut opening = versions_of(&ir);
    opening.vocab = "1.7".to_owned();
    let o = open_history(&ir, &opening, &OpeningCaps::current(), initial(), &h);
    refused(&o, LoadReason::VocabNewer);
}

#[test]
fn ver18_vocab_line_is_the_engine_cap_not_the_pack_declaration() {
    let ir = ir_of(&base_json());
    let h = envelope(&ir, within_st1(&ir));
    // 履歴 1.2・開く教材の宣言 1.0・上限 1.3: 比べる相手は教材の宣言ではなくエンジンの上限。
    let o = open(&ir, &edited(&h, |h| h.versions.vocab = "1.2".to_owned()));
    assert!(axes(&o).contains("Vocab"));
    assert_eq!(o.state(), Some(&h.snapshot));
}

#[test]
fn ver7_engine_newer_is_refused() {
    let ir = ir_of(&base_json());
    let h = envelope(&ir, within_st1(&ir));
    let with_engine = |e: &str| edited(&h, |h| h.versions.engine = e.to_owned());
    // V-v-2: 履歴 0.2.0・開く側 0.1.0。
    refused(
        &open_as_engine(&ir, "0.1.0", &with_engine("0.2.0")),
        LoadReason::EngineNewer,
    );
    // 修正版だけ新しい。
    refused(
        &open_as_engine(&ir, "0.1.0", &with_engine("0.1.1")),
        LoadReason::EngineNewer,
    );
    // 主版が同じで副版が新しい。
    refused(
        &open_as_engine(&ir, "1.2.0", &with_engine("1.3.0")),
        LoadReason::EngineNewer,
    );
    // 等しい版は通る。
    assert_eq!(
        open_as_engine(&ir, "0.1.0", &with_engine("0.1.0"))
            .verdict()
            .disposition(),
        Disposition::Open
    );
}

#[test]
fn ver7_bad_version_precedes_newer() {
    let ir = ir_of(&base_json());
    let h = envelope(&ir, within_st1(&ir));
    for field in ["engine", "vocab", "content"] {
        let bad = edited(&h, |h| {
            h.versions.ir_format = 3;
            match field {
                "engine" => h.versions.engine = "x".to_owned(),
                "vocab" => h.versions.vocab = "x".to_owned(),
                _ => h.versions.content = "x".to_owned(),
            }
        });
        assert_closed(
            &open(&ir, &bad),
            Disposition::Refused,
            LoadReason::BadVersion,
        );
    }
    // 開く側の版が読めない場合。
    let mut opening = versions_of(&ir);
    opening.engine = "x".to_owned();
    let o = open_history(&ir, &opening, &OpeningCaps::current(), initial(), &h);
    refused(&o, LoadReason::BadVersion);
}

#[test]
fn ver7_stage6_order_follows_history_load_order() {
    // 期待する順序は契約から取る（試験に手で並べない）。`format_older` は `format_newer` と同じ軸で
    // 同時に満たせないため、表示だけの側の順序は `ver7_view_only_order_follows_the_contract` に任せる。
    let want: Vec<LoadReason> = HISTORY_LOAD_ORDER
        .iter()
        .copied()
        .filter(|c| c.stage() == HistoryLoadStage::VersionJudgment)
        .take(4)
        .collect();
    assert_eq!(
        want,
        [
            LoadReason::BadVersion,
            LoadReason::FormatNewer,
            LoadReason::VocabNewer,
            LoadReason::EngineNewer
        ]
    );
    let ir = ir_of(&base_json());
    let base = envelope(&ir, within_st1(&ir));
    // 4 つの条件を同時に満たす包みから、軸を 1 つずつ直す（作り物）。
    let mut h = edited(&base, |h| {
        h.versions.content = "x".to_owned();
        h.versions.ir_format = 3;
        h.versions.vocab = "1.7".to_owned();
        h.versions.engine = "9.9.9".to_owned();
    });
    let mut got = Vec::new();
    for fix in 0..4 {
        let o = open_as_engine(&ir, "1.0.0", &h);
        got.push(match o {
            Opened::Refused { code } => code,
            other => panic!("expected refused, got {:?}", other.verdict().disposition()),
        });
        match fix {
            0 => h.versions.content = base.versions.content.clone(),
            1 => h.versions.ir_format = base.versions.ir_format,
            2 => h.versions.vocab = base.versions.vocab.clone(),
            _ => h.versions.engine = "1.0.0".to_owned(),
        }
    }
    assert_eq!(got, want);
    // 3 種は段 7（開始点）・段 8（写しの照合）より先に当たる。
    let no_start = edited(&base, |h| {
        h.start = "s-none".to_owned();
        h.versions.vocab = "1.7".to_owned();
    });
    refused(&open(&ir, &no_start), LoadReason::VocabNewer);
    let mut tampered = edited(&base, |h| h.versions.engine = "9.9.9".to_owned());
    tampered
        .snapshot
        .drafts
        .insert(StepUid("main/st-1".to_owned()).root(), "改ざん".to_owned());
    refused(&open(&ir, &tampered), LoadReason::EngineNewer);
}

#[test]
fn refused_reasons_are_non_empty_and_classified() {
    let ir = ir_of(&base_json());
    let h = envelope(&ir, within_st1(&ir));
    let cases = [
        (
            open(&ir, &edited(&h, |h| h.versions.ir_format = 3)),
            ReasonKind::HistoryNewer(VersionAxis::IrFormat),
        ),
        (
            open(&ir, &edited(&h, |h| h.versions.vocab = "1.7".to_owned())),
            ReasonKind::HistoryNewer(VersionAxis::Vocab),
        ),
        (
            open_as_engine(
                &ir,
                "0.1.0",
                &edited(&h, |h| h.versions.engine = "0.2.0".to_owned()),
            ),
            ReasonKind::HistoryNewer(VersionAxis::Engine),
        ),
        (
            open(&ir, &edited(&h, |h| h.versions.vocab = "x".to_owned())),
            ReasonKind::UnreadableVersion,
        ),
    ];
    for (o, kind) in cases {
        assert_eq!(o.verdict().disposition(), Disposition::Refused);
        let code = o.verdict().code().copied().expect("code");
        assert!(!code.as_str().is_empty());
        assert_eq!(code.kind(), Some(kind));
        assert!(o.state().is_none());
    }
}
