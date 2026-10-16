//! 履歴の読み込みの段 1〜3（大きさ・入れ子の深さ・JSON の段。`truncated` の判定を含む）の試験（TASK-331 1/3、SAVE-19・SAVE-20）。
//!
//! 順序の正は contract の `HISTORY_LOAD_ORDER`。2 つの段に同時に違反する入力で、前の段の理由が返ることを固定する。
//! 100MB の実バッファは作らず、長さだけを見る判定は `history_size` の試験に任せる（本試験は 1 バイト超過と 2 倍で確かめる）。
//! 画面の状態（DOM・`localStorage`）は core では観測できない。ブラウザでの確認は後続で扱う（未作成）。

mod common;

use std::time::{Duration, Instant};

use common::history_sample::read_bytes;
use common::json_input::{nested, pad_to};
use common::mem_store::MemStore;
use fandhe_workshop_contract::limits::{LoadStage, MAX_HISTORY_BYTES, MAX_JSON_DEPTH};
use fandhe_workshop_contract::load_reason::{HistoryLoadStage, LoadReason, HISTORY_LOAD_ORDER};
use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, HistoryStore};
use fandhe_workshop_core::history_load::parse_history_json;

const LIMIT: usize = MAX_HISTORY_BYTES as usize;

fn reason(bytes: &[u8]) -> LoadReason {
    parse_history_json(bytes).unwrap_err()
}

fn order_pos(r: LoadReason) -> usize {
    HISTORY_LOAD_ORDER
        .iter()
        .position(|x| *x == r)
        .expect("reason must be in the order")
}

/// `{"a":"あ` を「あ」（3 バイト）の `n` バイト目で切る。
fn cut_multibyte(n: usize) -> Vec<u8> {
    let full = "{\"a\":\"あ\"}".as_bytes().to_vec();
    full[..6 + n].to_vec()
}

#[test]
fn exact_limit_valid_history_passes_size() {
    let body = read_bytes("two-rounds");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let padded = pad_to(&body, LIMIT);
    assert_eq!(padded.len(), LIMIT);
    assert_eq!(parse_history_json(&padded), Ok(v));
}

#[test]
fn over_by_one_is_too_large() {
    let padded = pad_to(&read_bytes("two-rounds"), LIMIT + 1);
    assert_eq!(reason(&padded), LoadReason::TooLarge);
}

#[test]
fn double_is_too_large() {
    let padded = vec![b' '; LIMIT * 2];
    assert_eq!(reason(&padded), LoadReason::TooLarge);
}

#[test]
fn depth_exact_passes_depth() {
    assert!(parse_history_json(&nested(MAX_JSON_DEPTH, true)).is_ok());
}

#[test]
fn depth_over_is_too_deep() {
    assert_eq!(
        reason(&nested(MAX_JSON_DEPTH + 1, true)),
        LoadReason::TooDeep
    );
}

#[test]
fn ten_thousand_levels_is_too_deep_within_a_second() {
    let b = nested(10_000, true);
    let t = Instant::now();
    assert_eq!(reason(&b), LoadReason::TooDeep);
    assert!(t.elapsed() < Duration::from_secs(1));
}

#[test]
fn cut_at_char_boundary_is_truncated() {
    let b = read_bytes("two-rounds");
    let cut = &b[..b.len() * 55 / 100];
    assert_eq!(reason(cut), LoadReason::Truncated);
}

#[test]
fn cut_inside_multibyte_char_is_truncated() {
    assert_eq!(reason(&cut_multibyte(1)), LoadReason::Truncated);
    assert_eq!(reason(&cut_multibyte(2)), LoadReason::Truncated);
}

#[test]
fn invalid_byte_in_middle_is_not_json() {
    assert_eq!(reason(b"{\"a\":\"\xff\"}"), LoadReason::NotJson);
}

#[test]
fn syntax_error_is_not_json() {
    assert_eq!(reason(b"{\"a\":1,}"), LoadReason::NotJson);
    assert_eq!(reason(b"{\"a\":1} x"), LoadReason::NotJson);
}

#[test]
fn empty_input_is_truncated() {
    assert_eq!(reason(b""), LoadReason::Truncated);
    assert_eq!(reason(b"  \n "), LoadReason::Truncated);
}

#[test]
fn valid_history_passes_three_stages() {
    for name in common::history_sample::SAMPLES {
        assert!(parse_history_json(&read_bytes(name)).is_ok(), "{name}");
    }
}

#[test]
fn size_precedes_depth() {
    let b = pad_to(&nested(MAX_JSON_DEPTH + 1, true), LIMIT + 1);
    assert_eq!(reason(&b), LoadReason::TooLarge);
}

#[test]
fn size_precedes_utf8() {
    let b = pad_to(&cut_multibyte(1), LIMIT + 1);
    assert_eq!(reason(&b), LoadReason::TooLarge);
}

#[test]
fn size_precedes_parse() {
    let b = pad_to(b"{not json", LIMIT + 1);
    assert_eq!(reason(&b), LoadReason::TooLarge);
}

#[test]
fn depth_precedes_utf8() {
    let mut b = nested(MAX_JSON_DEPTH + 1, true);
    b.push(0xff);
    assert_eq!(reason(&b), LoadReason::TooDeep);
}

#[test]
fn depth_precedes_eof() {
    assert_eq!(
        reason(&nested(MAX_JSON_DEPTH + 1, false)),
        LoadReason::TooDeep
    );
}

#[test]
fn utf8_precedes_parse() {
    let mut b = b"{not json \"".to_vec();
    b.extend_from_slice(&"あ".as_bytes()[..1]);
    assert_eq!(reason(&b), LoadReason::Truncated);
}

#[test]
fn reasons_match_contract_stages() {
    let cases: Vec<(Vec<u8>, HistoryLoadStage, LoadStage)> = vec![
        (
            pad_to(b"[]", LIMIT + 1),
            HistoryLoadStage::Size,
            LoadStage::Size,
        ),
        (
            nested(MAX_JSON_DEPTH + 1, true),
            HistoryLoadStage::Depth,
            LoadStage::Depth,
        ),
        (
            cut_multibyte(1),
            HistoryLoadStage::Json,
            LoadStage::JsonParse,
        ),
        (b"{,".to_vec(), HistoryLoadStage::Json, LoadStage::JsonParse),
    ];
    for (b, st, coarse) in cases {
        let r = reason(&b);
        assert_eq!(r.stage(), st, "{r}");
        assert_eq!(r.stage().coarse(), coarse, "{r}");
    }
}

#[test]
fn rejection_order_follows_history_load_order() {
    let mut deep_and_bad = nested(MAX_JSON_DEPTH + 1, true);
    deep_and_bad.push(0xff);
    // (入力, 返るべき理由, 同時に違反している後の段の理由)
    let cases: Vec<(Vec<u8>, LoadReason, LoadReason)> = vec![
        (
            pad_to(&nested(MAX_JSON_DEPTH + 1, true), LIMIT + 1),
            LoadReason::TooLarge,
            LoadReason::TooDeep,
        ),
        (deep_and_bad, LoadReason::TooDeep, LoadReason::NotJson),
        (
            pad_to(&cut_multibyte(1), LIMIT + 1),
            LoadReason::TooLarge,
            LoadReason::Truncated,
        ),
    ];
    for (b, got, later) in cases {
        assert_eq!(reason(&b), got);
        assert!(
            order_pos(got) < order_pos(later),
            "{got} must precede {later}"
        );
    }
}

#[test]
fn rejection_leaves_store_unchanged() {
    let store = MemStore::default();
    let k1 = HistoryKey::for_pack(DeliveryScope::root(), "pack-a", "start");
    let k2 = HistoryKey::for_pack(DeliveryScope::root(), "pack-b", "start");
    let valid = String::from_utf8(read_bytes("two-rounds")).unwrap();
    store.write(&k1, &valid).unwrap();
    store.write(&k2, &valid[..valid.len() * 55 / 100]).unwrap();
    store.reset_counts();
    let before = store.snapshot();
    let keys_before = store.keys().unwrap();

    // 保存先の経路（文字列）。
    let stored = store.read(&k2).unwrap().unwrap();
    assert_eq!(reason(stored.as_bytes()), LoadReason::Truncated);
    // ファイル由来の経路（バイト列）。
    let rejects: Vec<Vec<u8>> = vec![
        pad_to(b"[]", LIMIT + 1),
        nested(MAX_JSON_DEPTH + 1, true),
        cut_multibyte(2),
        b"{\"a\":1,}".to_vec(),
        Vec::new(),
    ];
    for b in &rejects {
        assert!(parse_history_json(b).is_err());
    }

    assert_eq!(store.snapshot(), before);
    assert_eq!(store.keys().unwrap(), keys_before);
    assert_eq!(store.writes(), 0);
    assert_eq!(store.removes(), 0);
}

#[test]
fn rejection_is_deterministic() {
    for b in [
        cut_multibyte(1),
        nested(MAX_JSON_DEPTH + 1, true),
        b"x".to_vec(),
    ] {
        assert_eq!(reason(&b), reason(&b));
    }
}

// ---- 段 4〜6（版の欄・型・版の判定。TASK-331 2/3）----

use common::contract_sample::{load as load_sample, testdata_dir};
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::{Disposition, HistoryVersions};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::history_load::{load_history, parse_history_envelope};
use fandhe_workshop_core::history_open::{Opened, OpeningCaps};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::version_record::record_versions;
use serde_json::{json, Value};

const MAX_OPS: usize = 20_000;

fn ir() -> Ir {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(path).expect("read min-3step")).expect("load_ir")
}

fn opening(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

fn two_rounds() -> Value {
    serde_json::from_slice(&read_bytes("two-rounds")).expect("parse")
}

fn bytes_of(edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut v = two_rounds();
    edit(&mut v);
    serde_json::to_vec(&v).expect("to_vec")
}

fn run_with(bytes: &[u8], opening_edit: impl FnOnce(&mut HistoryVersions)) -> Opened {
    let ir = ir();
    let mut op = opening(&ir);
    opening_edit(&mut op);
    load_history(
        bytes,
        &ir,
        &op,
        &OpeningCaps::current(),
        load_sample().initial,
    )
}

fn run(bytes: &[u8]) -> Opened {
    run_with(bytes, |_| {})
}

fn code(o: &Opened) -> (Disposition, LoadReason) {
    match o {
        Opened::ViewOnly { code } => (Disposition::ViewOnly, *code),
        Opened::Refused { code } => (Disposition::Refused, *code),
        other => panic!("expected a rejection, got {other:?}"),
    }
}

fn refused(bytes: &[u8]) -> LoadReason {
    let o = run(bytes);
    let (d, c) = code(&o);
    assert_eq!(d, Disposition::Refused, "{c}");
    c
}

fn ops_of(n: usize) -> Value {
    Value::Array((0..n).map(|_| json!({"draft": "x"})).collect())
}

fn read_bytes_addendum(name: &str) -> Vec<u8> {
    let p = testdata_dir()
        .join("save-restore-versions/test-data-addendum/histories")
        .join(format!("{name}.json"));
    std::fs::read(p).expect("read addendum")
}

fn reason_of(o: &Opened) -> Option<LoadReason> {
    match o {
        Opened::ViewOnly { code } | Opened::Refused { code } => Some(*code),
        _ => None,
    }
}

#[test]
fn stage4_no_versions_poc_file() {
    let b = read_bytes_addendum("b-5-no-versions");
    assert_eq!(refused(&b), LoadReason::MissingVersion);
}

#[test]
fn stage4_versions_not_a_table() {
    for bad in [None, Some(Value::Null), Some(json!([])), Some(json!("1"))] {
        let b = bytes_of(|v| match &bad {
            None => {
                v.as_object_mut().unwrap().remove("versions");
            }
            Some(x) => v["versions"] = x.clone(),
        });
        assert_eq!(refused(&b), LoadReason::MissingVersion, "{bad:?}");
    }
}

#[test]
fn stage4_format_missing_or_wrong() {
    for bad in [None, Some(json!("fandhe-workshop-histor")), Some(json!(1))] {
        let b = bytes_of(|v| match &bad {
            None => {
                v.as_object_mut().unwrap().remove("format");
            }
            Some(x) => v["format"] = x.clone(),
        });
        assert_eq!(refused(&b), LoadReason::MissingVersion, "{bad:?}");
    }
}

#[test]
fn stage4_history_format_missing() {
    let b = bytes_of(|v| {
        v.as_object_mut().unwrap().remove("history_format");
    });
    assert_eq!(refused(&b), LoadReason::MissingVersion);
}

#[test]
fn stage4_top_level_not_a_table() {
    assert_eq!(refused(b"[]"), LoadReason::MissingVersion);
    assert_eq!(refused(b"1"), LoadReason::MissingVersion);
}

#[test]
fn stage4_boundary_inner_defects_are_malformed() {
    let no_engine = bytes_of(|v| {
        v["versions"].as_object_mut().unwrap().remove("engine");
    });
    assert_eq!(refused(&no_engine), LoadReason::Malformed);
    let unknown = bytes_of(|v| v["versions"]["extra"] = json!(1));
    assert_eq!(refused(&unknown), LoadReason::Malformed);
    let hf = bytes_of(|v| v["history_format"] = json!("1"));
    assert_eq!(refused(&hf), LoadReason::Malformed);
}

#[test]
fn stage5_malformed() {
    assert_eq!(
        refused(&read_bytes_addendum("b-7-malformed")),
        LoadReason::Malformed
    );
    let cases: Vec<Vec<u8>> = vec![
        bytes_of(|v| v["snapshot"]["run"]["generation"] = json!("x")),
        bytes_of(|v| v["unknown_field"] = json!(1)),
        bytes_of(|v| v["ops"] = json!([{"zzz": 1}])),
        bytes_of(|v| {
            v.as_object_mut().unwrap().remove("snapshot");
        }),
    ];
    for b in cases {
        assert_eq!(refused(&b), LoadReason::Malformed);
    }
}

#[test]
fn stage5_ops_count_boundary() {
    let mk = |n: usize| {
        let mut v = two_rounds();
        v["ops"] = ops_of(n);
        v
    };
    assert!(parse_history_envelope(mk(MAX_OPS)).is_ok());
    assert_eq!(
        parse_history_envelope(mk(MAX_OPS + 1)).unwrap_err(),
        LoadReason::TooManyOps
    );
}

#[test]
fn stage5_too_many_ops_through_load_history() {
    let b = bytes_of(|v| v["ops"] = ops_of(MAX_OPS + 1));
    assert_eq!(refused(&b), LoadReason::TooManyOps);
    // 20,000 件ちょうどは段 5 の件数では断たない（段 7 以降の結果は本件の範囲外）。
    let ok = bytes_of(|v| v["ops"] = ops_of(MAX_OPS));
    assert_ne!(reason_of(&run(&ok)), Some(LoadReason::TooManyOps));
}

#[test]
fn stage5_newer_history_format_and_other_pack() {
    let b = bytes_of(|v| v["history_format"] = json!(2));
    assert_eq!(refused(&b), LoadReason::HistoryFormatNewer);
    let b = bytes_of(|v| v["pack_id"] = json!("other"));
    assert_eq!(refused(&b), LoadReason::OtherPack);
}

#[test]
fn stage6_refused_reasons_reach_the_entry() {
    let cases: Vec<(Vec<u8>, LoadReason)> = vec![
        (
            bytes_of(|v| v["versions"]["engine"] = json!("x")),
            LoadReason::BadVersion,
        ),
        (
            bytes_of(|v| v["versions"]["ir_format"] = json!(2)),
            LoadReason::FormatNewer,
        ),
        (
            bytes_of(|v| v["versions"]["vocab"] = json!("9.0")),
            LoadReason::VocabNewer,
        ),
        (
            bytes_of(|v| v["versions"]["engine"] = json!("1.0.0")),
            LoadReason::EngineNewer,
        ),
    ];
    for (b, want) in cases {
        assert_eq!(refused(&b), want);
    }
}

#[test]
fn stage6_view_only_reasons_reach_the_entry() {
    let o = run(&bytes_of(|v| v["versions"]["ir_format"] = json!(0)));
    assert_eq!(code(&o), (Disposition::ViewOnly, LoadReason::FormatOlder));
    let o = run_with(&read_bytes("two-rounds"), |op| {
        op.engine = "1.0.0".to_string();
    });
    assert_eq!(code(&o), (Disposition::ViewOnly, LoadReason::EngineMajor));
    let o = run_with(&read_bytes("two-rounds"), |op| {
        op.content = "1.0.0".to_string();
    });
    assert_eq!(code(&o), (Disposition::ViewOnly, LoadReason::ContentMajor));
}

#[test]
fn stages_4_to_6_follow_history_load_order() {
    let cut = {
        let mut b = bytes_of(|v| {
            v.as_object_mut().unwrap().remove("versions");
        });
        b.truncate(b.len() * 55 / 100);
        b
    };
    let cases: Vec<(Vec<u8>, LoadReason, LoadReason)> = vec![
        (cut, LoadReason::Truncated, LoadReason::MissingVersion),
        (
            bytes_of(|v| {
                v.as_object_mut().unwrap().remove("versions");
                v["pack_id"] = json!("other");
            }),
            LoadReason::MissingVersion,
            LoadReason::OtherPack,
        ),
        (
            bytes_of(|v| {
                v["format"] = json!("x");
                v["snapshot"]["run"]["generation"] = json!("x");
            }),
            LoadReason::MissingVersion,
            LoadReason::Malformed,
        ),
        (
            bytes_of(|v| {
                v["snapshot"]["run"]["generation"] = json!("x");
                v["ops"] = ops_of(MAX_OPS + 1);
            }),
            LoadReason::Malformed,
            LoadReason::TooManyOps,
        ),
        (
            bytes_of(|v| {
                v["ops"] = ops_of(MAX_OPS + 1);
                v["history_format"] = json!(2);
            }),
            LoadReason::TooManyOps,
            LoadReason::HistoryFormatNewer,
        ),
        (
            bytes_of(|v| {
                v["history_format"] = json!(2);
                v["pack_id"] = json!("other");
            }),
            LoadReason::HistoryFormatNewer,
            LoadReason::OtherPack,
        ),
        (
            bytes_of(|v| {
                v["pack_id"] = json!("other");
                v["versions"]["engine"] = json!("x");
            }),
            LoadReason::OtherPack,
            LoadReason::BadVersion,
        ),
        (
            bytes_of(|v| {
                v["pack_id"] = json!("other");
                v["versions"]["ir_format"] = json!(99);
            }),
            LoadReason::OtherPack,
            LoadReason::FormatNewer,
        ),
    ];
    for (b, got, later) in cases {
        assert_eq!(code(&run(&b)).1, got);
        assert!(
            order_pos(got) < order_pos(later),
            "{got} must precede {later}"
        );
    }
}

#[test]
fn stages_4_to_6_match_contract_stages() {
    let cases: Vec<(Vec<u8>, HistoryLoadStage, LoadStage)> = vec![
        (
            bytes_of(|v| {
                v.as_object_mut().unwrap().remove("versions");
            }),
            HistoryLoadStage::VersionField,
            LoadStage::TypeAndContent,
        ),
        (
            bytes_of(|v| v["unknown_field"] = json!(1)),
            HistoryLoadStage::Type,
            LoadStage::TypeAndContent,
        ),
        (
            bytes_of(|v| v["versions"]["engine"] = json!("x")),
            HistoryLoadStage::VersionJudgment,
            LoadStage::TypeAndContent,
        ),
    ];
    for (b, st, coarse) in cases {
        let o = run(&b);
        let c = code(&o).1;
        assert_eq!(c.stage(), st, "{c}");
        assert_eq!(c.stage().coarse(), coarse, "{c}");
        assert!(o.state().is_none());
        assert!(!c.to_string().is_empty());
    }
}

#[test]
fn stage4_to_6_rejection_leaves_store_unchanged() {
    let store = MemStore::default();
    let k1 = HistoryKey::for_pack(DeliveryScope::root(), "pack-a", "start");
    let k2 = HistoryKey::for_pack(DeliveryScope::root(), "pack-b", "start");
    let k3 = HistoryKey::for_pack(DeliveryScope::root(), "pack-c", "start");
    let valid = String::from_utf8(read_bytes("two-rounds")).unwrap();
    let no_versions = String::from_utf8(bytes_of(|v| {
        v.as_object_mut().unwrap().remove("versions");
    }))
    .unwrap();
    let newer = String::from_utf8(bytes_of(|v| v["versions"]["ir_format"] = json!(2))).unwrap();
    store.write(&k1, &valid).unwrap();
    store.write(&k2, &no_versions).unwrap();
    store.write(&k3, &newer).unwrap();
    store.reset_counts();
    let before = store.snapshot();
    let keys_before = store.keys().unwrap();

    for k in [&k2, &k3] {
        let stored = store.read(k).unwrap().unwrap();
        assert!(run(stored.as_bytes()).state().is_none());
    }
    let from_file: Vec<Vec<u8>> = vec![
        read_bytes_addendum("b-5-no-versions"),
        read_bytes_addendum("b-7-malformed"),
        bytes_of(|v| v["ops"] = ops_of(MAX_OPS + 1)),
    ];
    for b in &from_file {
        assert!(run(b).state().is_none());
    }

    assert_eq!(store.snapshot(), before);
    assert_eq!(store.keys().unwrap(), keys_before);
    assert_eq!(store.writes(), 0);
    assert_eq!(store.removes(), 0);
}

#[test]
fn reasons_carry_no_input_text() {
    let b = bytes_of(|v| v["format"] = json!("SECRET-MARKER"));
    let c = code(&run(&b)).1;
    assert!(!c.to_string().contains("SECRET-MARKER"));
    assert!(HISTORY_LOAD_ORDER.contains(&c));
}

// ---- 段 7・8（開始点・適用し直しと写しとの照合。TASK-331 3/3）----
//
// 判定の実体は `open_history` にあり、ここは `load_history`（バイト列の入口）から通して確かめる。
// 開く履歴は、`history_open` の試験と同じく記録の経路（`RecordPath`）で作る。固定の履歴のうち `two-rounds` は
// `reload` を含み、再読み込みの意味の実装（TASK-320）まで適用し直せない。

use common::op_gen::{OpSpec, Which};
use common::record_path::RecordPath;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::history_envelope::{build_envelope, SessionEnvelope};
use fandhe_workshop_core::input::{Input, Op};

/// 試験の履歴の開始点。教材（min-3step）の `starts` にある ID。
const START: &str = "s-1";

type OpeningEdit = Box<dyn Fn(&mut HistoryVersions)>;

fn no_edit() -> OpeningEdit {
    Box::new(|_| {})
}

fn load_with(bytes: &[u8], edit: &OpeningEdit) -> Opened {
    let ir = ir();
    let mut op = opening(&ir);
    edit(&mut op);
    load_history(
        bytes,
        &ir,
        &op,
        &OpeningCaps::current(),
        load_sample().initial,
    )
}

/// 開く履歴の包み。記録の経路で Step 1 の処理を終えるまで進める。
fn opening_envelope(ir: &Ir) -> SessionEnvelope {
    let mut path = RecordPath::new(ir, &load_sample().initial);
    path.draft("/make-note first-note");
    path.spec(&OpSpec::Cmd(0));
    for _ in 0..5 {
        path.spec(&OpSpec::Tick(Which::Current));
    }
    let RecordPath { state, rec, .. } = path;
    build_envelope(ir, START, rec.into_ops(), state).expect("build")
}

fn env_bytes(edit: impl FnOnce(&mut SessionEnvelope)) -> Vec<u8> {
    let mut h = opening_envelope(&ir());
    edit(&mut h);
    serde_json::to_vec(&h).expect("to_vec")
}

fn tamper_snapshot(h: &mut SessionEnvelope) {
    h.snapshot
        .drafts
        .insert(StepUid("main/st-1".to_owned()).root(), "改ざん".to_owned());
}

fn run8(bytes: &[u8]) -> Opened {
    load_with(bytes, &no_edit())
}

#[test]
fn stage7_8_valid_history_opens() {
    let o = run8(&env_bytes(|_| {}));
    assert!(matches!(o, Opened::Open { .. }), "{o:?}");
}

#[test]
fn stage7_start_missing() {
    let o = run8(&env_bytes(|h| h.start = "s-none".to_owned()));
    assert_eq!(code(&o), (Disposition::ViewOnly, LoadReason::StartMissing));
    assert!(o.state().is_none());
}

#[test]
fn stage8_tampered_snapshot_is_replay_mismatch() {
    let o = run8(&env_bytes(tamper_snapshot));
    assert_eq!(code(&o), (Disposition::Refused, LoadReason::ReplayMismatch));
    assert!(o.state().is_none());
}

#[test]
fn stage8_tampered_ops_is_replay_mismatch() {
    let o = run8(&env_bytes(|h| {
        h.ops.pop();
    }));
    assert_eq!(code(&o), (Disposition::Refused, LoadReason::ReplayMismatch));
}

#[test]
fn stage8_diverged_needs_a_version_difference() {
    let bad = env_bytes(tamper_snapshot);
    let minor: OpeningEdit = Box::new(|op| op.content = "0.0.2".to_owned());
    let refs: OpeningEdit = Box::new(|op| {
        op.refs.insert("x".to_owned(), "bbb".to_owned());
    });
    for edit in [&minor, &refs] {
        let o = load_with(&bad, edit);
        assert_eq!(
            code(&o),
            (Disposition::ViewOnly, LoadReason::ReplayDiverged)
        );
    }
    // 写しが一致していれば、版が違っても diverged にならず変換して開く。
    let o = load_with(&env_bytes(|_| {}), &refs);
    assert!(matches!(o, Opened::Converted { .. }), "{o:?}");
}

#[test]
fn reload_history_is_replay_mismatch_until_task320() {
    let b = read_bytes("two-rounds");
    assert!(std::str::from_utf8(&b).unwrap().contains("reload"));
    assert_eq!(refused(&b), LoadReason::ReplayMismatch);
}

#[test]
fn stages_6_to_8_follow_history_load_order() {
    let missing = || env_bytes(|h| h.start = "s-none".to_owned());
    let cases: Vec<(Vec<u8>, OpeningEdit, LoadReason, LoadReason)> = vec![
        (
            missing(),
            Box::new(|op| op.content = "9.0.0".to_owned()),
            LoadReason::ContentMajor,
            LoadReason::StartMissing,
        ),
        (
            missing(),
            Box::new(|op| op.engine = "1.0.0".to_owned()),
            LoadReason::EngineMajor,
            LoadReason::StartMissing,
        ),
        (
            env_bytes(|h| {
                h.start = "s-none".to_owned();
                tamper_snapshot(h);
            }),
            no_edit(),
            LoadReason::StartMissing,
            LoadReason::ReplayMismatch,
        ),
        (
            env_bytes(|h| {
                h.pack_id = "other".to_owned();
                tamper_snapshot(h);
            }),
            no_edit(),
            LoadReason::OtherPack,
            LoadReason::ReplayMismatch,
        ),
    ];
    for (b, edit, got, later) in cases {
        assert_eq!(code(&load_with(&b, &edit)).1, got);
        assert!(
            order_pos(got) < order_pos(later),
            "{got} must precede {later}"
        );
    }
    // 段 8 の 2 種は排他で、版が同じなら mismatch、違えば diverged（順も契約どおり）。
    assert!(order_pos(LoadReason::ReplayMismatch) < order_pos(LoadReason::ReplayDiverged));
}

#[test]
fn stage7_8_match_contract_stages() {
    let cases: Vec<(Vec<u8>, HistoryLoadStage)> = vec![
        (
            env_bytes(|h| h.start = "s-none".to_owned()),
            HistoryLoadStage::Replay,
        ),
        (env_bytes(tamper_snapshot), HistoryLoadStage::SnapshotMatch),
    ];
    for (b, st) in cases {
        let o = run8(&b);
        let c = code(&o).1;
        assert_eq!(c.stage(), st, "{c}");
        assert!(o.state().is_none());
    }
}

#[test]
fn exact_limit_valid_opening_history_opens_through_all_stages() {
    let b = pad_to(&env_bytes(|_| {}), LIMIT);
    assert_eq!(b.len(), LIMIT);
    assert!(matches!(run8(&b), Opened::Open { .. }));
}

#[test]
fn stage7_8_rejection_leaves_store_unchanged() {
    let store = MemStore::default();
    let texts = [
        env_bytes(|_| {}),
        env_bytes(|h| h.start = "s-none".to_owned()),
        env_bytes(tamper_snapshot),
    ];
    let keys: Vec<HistoryKey> = ["pack-a", "pack-b", "pack-c"]
        .iter()
        .map(|p| HistoryKey::for_pack(DeliveryScope::root(), p, "start"))
        .collect();
    for (k, t) in keys.iter().zip(&texts) {
        store.write(k, std::str::from_utf8(t).unwrap()).unwrap();
    }
    store.reset_counts();
    let before = store.snapshot();
    let keys_before = store.keys().unwrap();

    let want = [
        None,
        Some(LoadReason::StartMissing),
        Some(LoadReason::ReplayMismatch),
    ];
    // 保存先の経路（文字列）。開く結果でも書き込みは起きない。
    for (k, w) in keys.iter().zip(want) {
        let stored = store.read(k).unwrap().unwrap();
        assert_eq!(reason_of(&run8(stored.as_bytes())), w);
    }
    // ファイル由来の経路（バイト列）。
    for t in &texts {
        let _ = run8(t);
    }

    assert_eq!(store.snapshot(), before);
    assert_eq!(store.keys().unwrap(), keys_before);
    assert_eq!(store.writes(), 0);
    assert_eq!(store.removes(), 0);
}

#[test]
fn stage7_8_reasons_carry_no_input_text() {
    let marked = |h: &mut SessionEnvelope| {
        h.snapshot.drafts.insert(
            StepUid("main/st-1".to_owned()).root(),
            "SECRET-MARKER".to_owned(),
        );
    };
    let b = env_bytes(|h| {
        h.start = "SECRET-MARKER".to_owned();
        marked(h);
    });
    let c = code(&run8(&b)).1;
    assert_eq!(c, LoadReason::StartMissing);
    assert!(!c.to_string().contains("SECRET-MARKER"));
    let b = env_bytes(marked);
    let c = code(&run8(&b)).1;
    assert_eq!(c, LoadReason::ReplayMismatch);
    assert!(!c.to_string().contains("SECRET-MARKER"));
    // 同じ入力は同じ結果になる。
    assert_eq!(run8(&b), run8(&b));
}

/// 19 種の理由がすべて 1 つの入口（`load_history`）から返り、並びが `HISTORY_LOAD_ORDER` に沿う。
#[test]
fn all_nineteen_reasons_reach_the_single_entry() {
    use LoadReason as R;
    let refuse = Disposition::Refused;
    let view = Disposition::ViewOnly;
    let ver = |f: fn(&mut Value)| bytes_of(f);
    let cases: Vec<(Vec<u8>, OpeningEdit, R, Disposition)> = vec![
        (pad_to(b"[]", LIMIT + 1), no_edit(), R::TooLarge, refuse),
        (
            nested(MAX_JSON_DEPTH + 1, true),
            no_edit(),
            R::TooDeep,
            refuse,
        ),
        (cut_multibyte(1), no_edit(), R::Truncated, refuse),
        (b"{\"a\":1,}".to_vec(), no_edit(), R::NotJson, refuse),
        (
            ver(|v| {
                v.as_object_mut().unwrap().remove("versions");
            }),
            no_edit(),
            R::MissingVersion,
            refuse,
        ),
        (
            ver(|v| v["unknown_field"] = json!(1)),
            no_edit(),
            R::Malformed,
            refuse,
        ),
        (
            ver(|v| v["ops"] = ops_of(MAX_OPS + 1)),
            no_edit(),
            R::TooManyOps,
            refuse,
        ),
        (
            ver(|v| v["history_format"] = json!(2)),
            no_edit(),
            R::HistoryFormatNewer,
            refuse,
        ),
        (
            ver(|v| v["pack_id"] = json!("other")),
            no_edit(),
            R::OtherPack,
            refuse,
        ),
        (
            ver(|v| v["versions"]["engine"] = json!("x")),
            no_edit(),
            R::BadVersion,
            refuse,
        ),
        (
            ver(|v| v["versions"]["ir_format"] = json!(2)),
            no_edit(),
            R::FormatNewer,
            refuse,
        ),
        (
            ver(|v| v["versions"]["vocab"] = json!("9.0")),
            no_edit(),
            R::VocabNewer,
            refuse,
        ),
        (
            ver(|v| v["versions"]["engine"] = json!("1.0.0")),
            no_edit(),
            R::EngineNewer,
            refuse,
        ),
        (
            ver(|v| v["versions"]["ir_format"] = json!(0)),
            no_edit(),
            R::FormatOlder,
            view,
        ),
        (
            read_bytes("two-rounds"),
            Box::new(|op| op.engine = "1.0.0".to_owned()),
            R::EngineMajor,
            view,
        ),
        (
            read_bytes("two-rounds"),
            Box::new(|op| op.content = "1.0.0".to_owned()),
            R::ContentMajor,
            view,
        ),
        (
            env_bytes(|h| h.start = "s-none".to_owned()),
            no_edit(),
            R::StartMissing,
            view,
        ),
        (
            env_bytes(tamper_snapshot),
            no_edit(),
            R::ReplayMismatch,
            refuse,
        ),
        (
            env_bytes(tamper_snapshot),
            Box::new(|op| op.content = "0.0.2".to_owned()),
            R::ReplayDiverged,
            view,
        ),
    ];
    let mut seen = Vec::new();
    for (b, edit, want, disp) in &cases {
        let o = load_with(b, edit);
        assert_eq!(code(&o), (*disp, *want));
        assert!(o.state().is_none());
        assert!(!want.to_string().is_empty());
        seen.push(*want);
    }
    let positions: Vec<usize> = seen.iter().map(|r| order_pos(*r)).collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
    assert_eq!(seen, HISTORY_LOAD_ORDER.to_vec());
    let mut all = LoadReason::ALL.to_vec();
    all.sort_by_key(|r| order_pos(*r));
    assert_eq!(all, seen);
}

// ---- 適用し直しの所要時間（SAVE-19）----

/// 誤った入力の送信を記録して、`n` 件の `ops` を持つ履歴のバイト列にする。
fn wrong_input_history(n: usize, text_len: usize) -> Vec<u8> {
    let ir = ir();
    let mut path = RecordPath::new(&ir, &load_sample().initial);
    let text = format!("/{}", "z".repeat(text_len));
    while path.rec.len() < n {
        path.op(Op::Input(Input::Command(text.clone())));
    }
    let RecordPath { state, rec, .. } = path;
    let mut ops = rec.into_ops();
    ops.truncate(n);
    let h = build_envelope(&ir, START, ops, state).expect("build");
    serde_json::to_vec(&h).expect("to_vec")
}

/// 件数の上限ちょうど・10MB 近い履歴。作る費用が大きいため、2 本の試験で 1 度だけ作って共有する。
fn large_history() -> &'static [u8] {
    static LARGE: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    LARGE.get_or_init(|| wrong_input_history(MAX_OPS, 215))
}

/// 読み込みの全段（適用し直しを含む）の所要時間。`Open` になることも確かめる。
fn timed_open(b: &[u8]) -> Duration {
    let t = Instant::now();
    let o = run8(b);
    let d = t.elapsed();
    assert!(matches!(o, Opened::Open { .. }), "{:?}", reason_of(&o));
    d
}

/// 件数の上限（20,000 件）で 10MB 近い履歴が、全段を通って開く。
///
/// 660ms（spec SAVE-19 の PoC-6 の実測。29,138 件）は、現行の件数の上限を超えて段 5 で断たれるため再現できず、
/// 計測値は機械に依存するので表明しない。緩い上限（debug で 10 秒未満）だけを表明し、実測は `docs/testing.md` に記録する。
#[test]
fn max_ops_history_opens_within_a_loose_bound() {
    let b = large_history();
    assert!(b.len() <= LIMIT, "{}", b.len());
    assert!(b.len() > LIMIT * 9 / 10, "{}", b.len());
    let d = timed_open(b);
    eprintln!("replay: ops={MAX_OPS} bytes={} elapsed={d:?}", b.len());
    assert!(d < Duration::from_secs(10), "{d:?}");
}

/// 適用し直しの費用が件数に比例する（二乗にならない）。計時の揺れを避けるため、各件数で 3 回測った最小値を比べる。
#[test]
fn replay_cost_grows_linearly_with_op_count() {
    let min_of = |b: &[u8]| (0..3).map(|_| timed_open(b)).min().unwrap();
    let small = wrong_input_history(MAX_OPS / 4, 215);
    let large = large_history();
    let (ds, dl) = (min_of(&small), min_of(large));
    eprintln!(
        "replay: ops={} elapsed={ds:?} / ops={MAX_OPS} elapsed={dl:?}",
        MAX_OPS / 4
    );
    // 件数は 4 倍。線形なら約 4 倍で、二乗なら約 16 倍。余裕を見て 12 倍未満とする。
    assert!(
        dl.as_secs_f64() < ds.as_secs_f64().max(0.001) * 12.0,
        "{ds:?} -> {dl:?}"
    );
}
