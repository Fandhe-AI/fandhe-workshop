//! 壊れた履歴 7 種（SAVE-20 の B-1〜B-7）が、どれも開かず、固定の理由コードで断られることの試験（TASK-332 1/2、#880）。
//!
//! 期待の表の本体は `testdata/broken-histories/expectations.json` で、本試験は結果から表を導かない。
//! 表の ID の集合・件数・コードは、本ファイルの定数と照合する（表だけを書き換えて合計をずらす操作を検出する）。
//! 履歴は現行のエンジンで試験の中で決定的に作り、実際の入口 `load_history`（段 1〜8）で開く。
//! 失敗の文には ID・真偽・固定のコードだけを出し、履歴の中身を出さない（SAFE-40）。
//! `version_histories.rs`（VER-2）は版の違い、`history_load.rs` は段ごとの境界を扱う。本ファイルは 7 種を 1 つの表で揃えて見る。
//! 画面・保存領域の不変は web の `broken_histories_host.rs` と自動ブラウザの `broken-histories.test.mjs` が担う。
//! ブラウザ試験の入力は `emit_broken_histories_for_browser`（`#[ignore]`）が書き出す（`scripts/test-browser.sh` だけが呼ぶ）。

mod common;

use std::collections::BTreeSet;

use common::contract_sample::{load, testdata_dir};
use common::mem_store::MemStore;
use common::op_gen::OpSpec;
use common::record_path::RecordPath;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::limits::MAX_HISTORY_BYTES;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, HistoryStore};
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_load::load_history;
use fandhe_workshop_core::history_open::{Opened, OpeningCaps};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::version_record::record_versions;
use serde::Deserialize;
use serde_json::{json, Value};

const TABLE: &str = "broken-histories/expectations.json";
const START: &str = "s-1";
const LIMIT: usize = MAX_HISTORY_BYTES as usize;

/// SAVE-20 の ID の集合（表の順。対照の B-0・B-4b を含む）。
const IDS: [&str; 9] = [
    "B-0", "B-1", "B-2", "B-3", "B-4", "B-4b", "B-5", "B-6", "B-7",
];
/// 開かない 7 種と理由コード（SAVE-20）。
const REFUSED: [(&str, &str); 7] = [
    ("B-1", "replay_mismatch"),
    ("B-2", "truncated"),
    ("B-3", "not_json"),
    ("B-4", "too_large"),
    ("B-5", "missing_version"),
    ("B-6", "other_pack"),
    ("B-7", "malformed"),
];
/// 開く対照。
const CONTROLS: [&str; 1] = ["B-0"];
/// 大きさの境界の対照。ちょうど上限の履歴は大きさでは断られず、未知の欄 `padding` のため後段の `malformed` で断られる。
const BOUNDARY: (&str, &str) = ("B-4b", "malformed");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    table_version: u32,
    rows: Vec<Row>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: String,
    kind: String,
    recipe: String,
    expect: Expect,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expect {
    result: String,
    code: Option<String>,
}

fn read_table() -> Table {
    let bytes = std::fs::read(testdata_dir().join(TABLE)).expect("table must be readable");
    serde_json::from_slice(&bytes).expect("table must match the schema")
}

/// 基準の教材（版・語彙・参照元を足した min-3step）。`other_id` があれば `pack.id` だけを替える。
fn pack_json(other_id: Option<&str>) -> Value {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(path).expect("read")).expect("parse");
    v["pack"]["version"] = json!("1.0.0");
    v["pack"]["vocab"] = json!("1.2");
    v["refs"] = json!([{ "name": "sample-ref", "commit": "0000000" }]);
    if let Some(id) = other_id {
        v["pack"]["id"] = json!(id);
    }
    v
}

fn ir_of(other_id: Option<&str>) -> Ir {
    load_ir(&serde_json::to_vec(&pack_json(other_id)).expect("to_vec")).expect("load_ir")
}

/// 基準の履歴（`st-1` の処理を終えた所まで）。包みは compact な JSON で書き出す。
fn base_history(ir: &Ir) -> Vec<u8> {
    let mut path = RecordPath::new(ir, &load().initial);
    path.draft("/make-note first-note");
    path.spec(&OpSpec::Cmd(0));
    for _ in 0..5 {
        path.spec(&OpSpec::Tick(common::op_gen::Which::Current));
    }
    let RecordPath { state, rec, .. } = path;
    let env = build_envelope(ir, START, rec.into_ops(), state).expect("build");
    serde_json::to_vec(&env).expect("serialize")
}

fn opening(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

/// 履歴を書き換えた版。`edit` は JSON の値を直接書き換える。
fn edited(edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut v: Value = serde_json::from_slice(&base_history(&ir_of(None))).expect("parse");
    edit(&mut v);
    serde_json::to_vec(&v).expect("to_vec")
}

/// 10 MiB に近い履歴。`padding` の欄で目標の大きさにそろえる。
fn padded(total: usize) -> Vec<u8> {
    let base = edited(|v| v["padding"] = json!(""));
    assert!(base.len() < total);
    let fill = "x".repeat(total - base.len());
    let out = edited(|v| v["padding"] = json!(fill));
    assert_eq!(out.len(), total);
    out
}

/// ID から履歴のバイト列と、開く側の教材を作る。
fn make(id: &str) -> (Vec<u8>, Ir) {
    let base = ir_of(None);
    let bytes = match id {
        "B-0" => base_history(&base),
        "B-1" => edited(|v| {
            let op = v["ops"]
                .as_array_mut()
                .expect("ops")
                .iter_mut()
                .find(|o| o.get("submit").is_some())
                .expect("a submit op");
            op["submit"] = json!("/make-note other-note");
        }),
        "B-2" => {
            let b = base_history(&base);
            b[..b.len() * 55 / 100].to_vec()
        }
        "B-3" => {
            let s = String::from_utf8(base_history(&base)).expect("utf8");
            let at = s.find("\"ops\":[").expect("ops") + "\"ops\":[".len();
            format!("{}}}{}", &s[..at], &s[at..]).into_bytes()
        }
        "B-4" => padded(LIMIT + 1),
        "B-4b" => padded(LIMIT),
        "B-5" => edited(|v| {
            v.as_object_mut().expect("object").remove("versions");
        }),
        "B-6" => base_history(&ir_of(Some("other-tutorial"))),
        "B-7" => edited(|v| v["snapshot"]["run"]["generation"] = json!("one")),
        other => panic!("unknown id {other}"),
    };
    (bytes, base)
}

fn open(id: &str) -> Opened {
    let (bytes, ir) = make(id);
    load_history(
        &bytes,
        &ir,
        &opening(&ir),
        &OpeningCaps::current(),
        load().initial,
    )
}

#[test]
fn the_table_matches_the_fixed_constants() {
    let t = read_table();
    assert_eq!(t.table_version, 1);
    let ids: Vec<&str> = t.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, IDS);
    let refused: Vec<(&str, &str)> = t
        .rows
        .iter()
        .filter(|r| r.expect.result == "refused")
        .map(|r| (r.id.as_str(), r.expect.code.as_deref().expect("code")))
        .collect();
    let mut want: Vec<(&str, &str)> = REFUSED.to_vec();
    want.insert(4, BOUNDARY);
    assert_eq!(refused, want);
    let open: Vec<&str> = t
        .rows
        .iter()
        .filter(|r| r.expect.result == "open")
        .map(|r| r.id.as_str())
        .collect();
    assert_eq!(open, CONTROLS);
    assert!(t
        .rows
        .iter()
        .all(|r| r.expect.result == "refused" || r.expect.result == "open"));
    assert!(t
        .rows
        .iter()
        .all(|r| !r.kind.is_empty() && !r.recipe.is_empty()));
    let kinds: BTreeSet<&str> = t.rows.iter().map(|r| r.kind.as_str()).collect();
    assert_eq!(
        kinds.len(),
        9,
        "7 kinds of breakage, the size boundary and the control"
    );
}

#[test]
fn all_seven_broken_histories_are_refused_with_the_fixed_code() {
    for (id, code) in REFUSED {
        match open(id) {
            Opened::Refused { code: got } => {
                assert_eq!(got.as_str(), code, "{id}");
                assert!(!got.as_str().is_empty(), "{id}");
            }
            other => panic!(
                "{id}: expected a refusal, got a result of another kind {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }
}

#[test]
fn the_size_boundary_is_not_refused_for_size() {
    let (id, code) = BOUNDARY;
    match open(id) {
        Opened::Refused { code: got } => {
            assert_ne!(got, LoadReason::TooLarge, "{id}");
            assert_eq!(got.as_str(), code, "{id}");
        }
        _ => panic!("{id}"),
    }
}

#[test]
fn the_controls_open() {
    for id in CONTROLS {
        let o = open(id);
        assert!(matches!(o, Opened::Open { .. }), "{id}");
    }
}

#[test]
fn the_code_names_no_input_content() {
    let codes: Vec<LoadReason> = REFUSED
        .iter()
        .map(|(id, _)| match open(id) {
            Opened::Refused { code } => code,
            _ => panic!("{id}"),
        })
        .collect();
    for code in codes {
        let s = code.as_str();
        assert!(
            s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
            "{s}"
        );
        assert!(!s.contains("other-note") && !s.contains("first-note"));
    }
}

#[test]
fn refusal_is_deterministic() {
    for (id, _) in REFUSED {
        let a = open(id);
        let b = open(id);
        let code = |o: &Opened| match o {
            Opened::Refused { code } => Some(*code),
            _ => None,
        };
        assert!(code(&a).is_some(), "{id}");
        assert_eq!(code(&a), code(&b), "{id}");
    }
}

#[test]
fn refusal_leaves_the_store_unchanged() {
    let store = MemStore::default();
    let key = HistoryKey::for_pack(DeliveryScope::root(), "min-3step", "start");
    let valid = String::from_utf8(make("B-0").0).expect("utf8");
    store.write(&key, &valid).expect("write");
    store.reset_counts();
    let before = store.snapshot();
    for (id, _) in REFUSED {
        assert!(matches!(open(id), Opened::Refused { .. }), "{id}");
    }
    assert_eq!(store.snapshot(), before);
    assert_eq!(store.writes(), 0);
    assert_eq!(store.removes(), 0);
}

#[test]
fn a_cut_inside_a_multibyte_char_is_also_truncated() {
    let mut v: Value = serde_json::from_slice(&base_history(&ir_of(None))).expect("parse");
    v["note"] = json!("ノート");
    let bytes = serde_json::to_vec(&v).expect("to_vec");
    let s = String::from_utf8(bytes.clone()).expect("utf8");
    let at = s.find("ノ").expect("char") + 1;
    let ir = ir_of(None);
    let o = load_history(
        &bytes[..at],
        &ir,
        &opening(&ir),
        &OpeningCaps::current(),
        load().initial,
    );
    assert!(
        matches!(
            o,
            Opened::Refused {
                code: LoadReason::Truncated
            }
        ),
        "cut inside a char"
    );
}

/// ブラウザ試験の入力を書き出す先（絶対パスのディレクトリ）。未設定・相対・ディレクトリでなければ panic する。
fn emit_dir() -> std::path::PathBuf {
    let raw = std::env::var_os("FANDHE_BROKEN_HISTORIES_OUT")
        .expect("FANDHE_BROKEN_HISTORIES_OUT must be set");
    let dir = std::path::PathBuf::from(raw);
    assert!(dir.is_absolute(), "the output directory must be absolute");
    assert!(dir.is_dir(), "the output directory must exist");
    dir
}

/// 9 件の履歴と基準の教材を、ブラウザ試験の入力として書き出す。
///
/// 呼び出し元は `scripts/test-browser.sh` だけで、`--ignored` を付けたときしか流れない。
/// ファイル名は固定の ID だけから作り、表の文字列をそのままパスにしない。
#[test]
#[ignore = "ブラウザ試験の入力を書き出す。scripts/test-browser.sh だけが --ignored で呼ぶ"]
fn emit_broken_histories_for_browser() {
    let out = emit_dir();
    let t = read_table();
    let ids: Vec<&str> = t.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, IDS, "the table ids must match SAVE-20");
    for id in IDS {
        assert!(id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        std::fs::write(out.join(format!("{id}.history.json")), make(id).0).expect("write history");
    }
    let ir = serde_json::to_vec(&pack_json(None)).expect("to_vec");
    std::fs::write(out.join("base.ir.json"), ir).expect("write ir");
}
