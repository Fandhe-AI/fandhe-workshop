//! `ops` の 22 か所をそれぞれ書き換えて開くと、拒否か同一値のどちらかになり、誤った状態で開かないことの試験（TASK-332 2/2、#881）。
//!
//! 期待の表の本体は `testdata/broken-histories/ops-rewrites.json` で、本試験は結果から表を導かない。
//! 表の件数・分類の数・同一値の位置は本ファイルの定数と照合し、基準の履歴の構成（22 件・位置ごとの種類）も表と照合する。
//! 書き換えの規則は PoC-6 と同じ 3 つで、型の値のまま差し替えてから書き出す（JSON の文字列は手で書かない）。
//! 実際の入口 `load_history`（段 1〜8）で開く。失敗の文には ID・添字・真偽・固定のコードだけを出し、履歴の中身を出さない（SAFE-40）。
//! `broken_histories.rs`（1/2）は壊れ方 7 種を扱い、本ファイルは B-1 を `ops` の全位置に広げたものに当たる。
//! 画面・保存領域の不変は自動ブラウザの `ops-rewrites.test.mjs` が担う。入力は `emit_ops_rewrites_for_browser`（`#[ignore]`）が書き出す。

mod common;

use std::collections::BTreeSet;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{OpSpec, Which};
use common::record_path::RecordPath;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::history_envelope::{build_envelope, SessionEnvelope};
use fandhe_workshop_core::history_load::load_history;
use fandhe_workshop_core::history_open::{Opened, OpeningCaps};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::ops_record::SessionOp;
use fandhe_workshop_core::version_record::record_versions;
use serde::Deserialize;
use serde_json::{json, Value};

const TABLE: &str = "broken-histories/ops-rewrites.json";
const START: &str = "s-1";
/// 基準の履歴の `ops` の件数（PoC-6 の H-mid と同じ）。
const OPS: usize = 22;
/// 拒否される位置の数。
const REFUSED_COUNT: usize = 21;
/// 書き換えても元と同じ値になる位置（`Choice("no-such-option")` を同じ値に置き換える）。
const SAME_VALUE_AT: usize = 15;
const CODE: &str = "replay_mismatch";
/// 基準の履歴に必ず入るタグ。
const TAGS: [&str; 6] = ["submit", "draft", "reload", "input", "tick", "control"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    table_version: u32,
    base_ops: usize,
    rows: Vec<Row>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    index: usize,
    kind: String,
    rewrite: String,
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

/// 基準の教材（版・語彙・参照元を足した min-3step）。
fn pack_json() -> Value {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(path).expect("read")).expect("parse");
    v["pack"]["version"] = json!("1.0.0");
    v["pack"]["vocab"] = json!("1.2");
    v["refs"] = json!([{ "name": "sample-ref", "commit": "0000000" }]);
    v
}

fn ir_of() -> Ir {
    load_ir(&serde_json::to_vec(&pack_json()).expect("to_vec")).expect("load_ir")
}

fn opening(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

/// 位置ごとの種類。表の `kind` と同じ綴りで、基準の履歴の構成を表と照合するために使う。
fn kind_of(op: &SessionOp) -> &'static str {
    match op {
        HistoryOp::Submit(_) => "submit",
        HistoryOp::Draft(_) => "draft",
        HistoryOp::Reload => "reload",
        HistoryOp::Engine(Op::Tick(_)) => "tick",
        HistoryOp::Engine(Op::Input(Input::Choice(_))) => "input.choice",
        HistoryOp::Engine(Op::Input(Input::Ui { .. })) => "input.ui",
        HistoryOp::Engine(Op::Input(_)) => "input.other",
        HistoryOp::Engine(Op::Control(Control::Stop)) => "control.stop",
        HistoryOp::Engine(Op::Control(Control::Save)) => "control.save",
        HistoryOp::Engine(Op::Control(Control::OpenHint)) => "control.open_hint",
        HistoryOp::Engine(Op::Control(Control::SetGuidance(_))) => "control.set_guidance",
        HistoryOp::Engine(_) => "other",
    }
}

/// タグ（`.` の前の分類）。
fn tag_of(kind: &str) -> &str {
    kind.split('.').next().expect("kind")
}

/// 基準の履歴（`ops` が 22 件）。構成は表の `kind` の列に対応する。
fn base_envelope(ir: &Ir) -> SessionEnvelope {
    let mut p = RecordPath::new(ir, &load().initial);
    let cmd = || Op::Input(Input::Command("/make-note first-note".into()));
    p.op(cmd()); // 0 draft, 1 submit
    p.spec(&OpSpec::Tick(Which::Current)); // 2
    p.op(Op::Control(Control::Stop)); // 3
    p.op(cmd()); // 4 draft, 5 submit
    for _ in 0..5 {
        p.spec(&OpSpec::Tick(Which::Current)); // 6..=10
    }
    p.spec(&OpSpec::Save); // 11
    p.op(Op::Input(Input::Command("/publish-note".into()))); // 12 draft, 13 submit
    p.op(Op::Control(Control::OpenHint)); // 14
    p.op(Op::Input(Input::Choice("no-such-option".into()))); // 15
    p.reload(); // 16
    p.op(Op::Input(Input::Choice("opt-memo".into()))); // 17
    p.spec(&OpSpec::Save); // 18
    p.op(Op::Control(Control::SetGuidance(GuidanceLevel::B))); // 19
    p.op(Op::Input(Input::Ui {
        view: "v-files".into(),
        path: "notes/draft".into(),
    })); // 20
    p.draft("draft-mid"); // 21
    let RecordPath { state, rec, .. } = p;
    build_envelope(ir, START, rec.into_ops(), state).expect("build")
}

/// PoC-6 と同じ 3 つの書き換えの規則。
fn rewrite(op: &SessionOp) -> SessionOp {
    match op {
        HistoryOp::Engine(Op::Input(Input::Choice(_))) => {
            HistoryOp::Engine(Op::Input(Input::Choice("no-such-option".into())))
        }
        HistoryOp::Engine(Op::Control(Control::Save)) => {
            HistoryOp::Engine(Op::Control(Control::OpenHint))
        }
        _ => HistoryOp::Engine(Op::Input(Input::Command("/no-such-command".into()))),
    }
}

/// 規則名（表の `rewrite`）。
fn rule_of(op: &SessionOp) -> &'static str {
    match op {
        HistoryOp::Engine(Op::Input(Input::Choice(_))) => "choice_no_such_option",
        HistoryOp::Engine(Op::Control(Control::Save)) => "save_to_open_hint",
        _ => "command_no_such_command",
    }
}

fn open_bytes(bytes: &[u8], ir: &Ir) -> Opened {
    load_history(
        bytes,
        ir,
        &opening(ir),
        &OpeningCaps::current(),
        load().initial,
    )
}

fn base_bytes(ir: &Ir) -> Vec<u8> {
    serde_json::to_vec(&base_envelope(ir)).expect("serialize")
}

/// `at` 番目の `ops` を書き換えた履歴。`None` は書き換えなしの対照（W-00）。
fn rewritten(ir: &Ir, at: Option<usize>) -> Vec<u8> {
    let mut env = base_envelope(ir);
    if let Some(i) = at {
        let new = rewrite(&env.ops[i]);
        env.ops[i] = new;
    }
    serde_json::to_vec(&env).expect("serialize")
}

fn id_of(at: Option<usize>) -> String {
    format!("W-{:02}", at.map_or(0, |i| i + 1))
}

#[test]
fn the_table_matches_the_fixed_constants() {
    let t = read_table();
    assert_eq!(t.table_version, 1);
    assert_eq!(t.base_ops, OPS);
    assert_eq!(t.rows.len(), OPS);
    let idx: Vec<usize> = t.rows.iter().map(|r| r.index).collect();
    assert_eq!(idx, (0..OPS).collect::<Vec<_>>());
    let refused: Vec<&Row> = t
        .rows
        .iter()
        .filter(|r| r.expect.result == "refused")
        .collect();
    assert_eq!(refused.len(), REFUSED_COUNT);
    assert!(refused
        .iter()
        .all(|r| r.expect.code.as_deref() == Some(CODE)));
    let same: Vec<&Row> = t
        .rows
        .iter()
        .filter(|r| r.expect.result == "same_value")
        .collect();
    assert_eq!(
        same.iter().map(|r| r.index).collect::<Vec<_>>(),
        [SAME_VALUE_AT]
    );
    assert!(same.iter().all(|r| r.expect.code.is_none()));
    assert!(t
        .rows
        .iter()
        .all(|r| !r.kind.is_empty() && !r.rewrite.is_empty()));
}

#[test]
fn the_base_history_has_22_ops_whose_kinds_match_the_table() {
    let t = read_table();
    let ir = ir_of();
    let env = base_envelope(&ir);
    assert_eq!(env.ops.len(), OPS);
    for (i, row) in t.rows.iter().enumerate() {
        assert_eq!(kind_of(&env.ops[i]), row.kind, "index {i}");
        assert_eq!(rule_of(&env.ops[i]), row.rewrite, "index {i}");
    }
    let tags: BTreeSet<&str> = t.rows.iter().map(|r| tag_of(&r.kind)).collect();
    let want: BTreeSet<&str> = TAGS.iter().copied().collect();
    assert_eq!(tags, want);
}

#[test]
fn the_base_history_opens() {
    let ir = ir_of();
    let env = base_envelope(&ir);
    match open_bytes(&base_bytes(&ir), &ir) {
        Opened::Open { state } => assert!(state == env.snapshot, "W-00"),
        _ => panic!("W-00: the base history must open"),
    }
}

#[test]
fn every_rewrite_is_refused_or_same_value() {
    let t = read_table();
    let ir = ir_of();
    let base = base_envelope(&ir);
    let base_bytes = base_bytes(&ir);
    let (mut refused, mut same, mut wrong) = (0usize, 0usize, 0usize);
    for row in &t.rows {
        let i = row.index;
        let id = id_of(Some(i));
        let identical = rewrite(&base.ops[i]) == base.ops[i];
        let bytes = rewritten(&ir, Some(i));
        match (row.expect.result.as_str(), open_bytes(&bytes, &ir)) {
            ("refused", Opened::Refused { code }) if code.as_str() == CODE => {
                assert!(!identical, "{id}: a refused rewrite must change the op");
                assert_ne!(bytes, base_bytes, "{id}");
                refused += 1;
            }
            ("same_value", Opened::Open { state }) => {
                assert!(identical, "{id}: the rewrite must equal the original");
                assert!(
                    state == base.snapshot,
                    "{id}: state differs from the snapshot"
                );
                assert_eq!(bytes, base_bytes, "{id}");
                same += 1;
            }
            _ => wrong += 1,
        }
    }
    assert_eq!(refused, REFUSED_COUNT);
    assert_eq!(same, 1);
    assert_eq!(
        wrong, 0,
        "opened in a wrong state or refused for another reason"
    );
}

#[test]
fn exactly_one_position_maps_to_itself() {
    let ir = ir_of();
    let env = base_envelope(&ir);
    let fixed: Vec<usize> = (0..OPS)
        .filter(|&i| rewrite(&env.ops[i]) == env.ops[i])
        .collect();
    assert_eq!(fixed, [SAME_VALUE_AT]);
}

#[test]
fn refusal_is_deterministic() {
    let ir = ir_of();
    let run = || -> Vec<Option<LoadReason>> {
        (0..OPS)
            .map(|i| match open_bytes(&rewritten(&ir, Some(i)), &ir) {
                Opened::Refused { code } => Some(code),
                _ => None,
            })
            .collect()
    };
    assert_eq!(run(), run());
}

#[test]
fn the_code_names_no_input_content() {
    let ir = ir_of();
    for i in 0..OPS {
        if let Opened::Refused { code } = open_bytes(&rewritten(&ir, Some(i)), &ir) {
            let s = code.as_str();
            assert!(
                s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
                "{s}"
            );
            assert!(!s.contains("no-such") && !s.contains("draft-mid"));
        }
    }
}

/// ブラウザ試験の入力を書き出す先（絶対パスのディレクトリ）。未設定・相対・ディレクトリでなければ panic する。
fn emit_dir() -> std::path::PathBuf {
    let raw =
        std::env::var_os("FANDHE_OPS_REWRITES_OUT").expect("FANDHE_OPS_REWRITES_OUT must be set");
    let dir = std::path::PathBuf::from(raw);
    assert!(dir.is_absolute(), "the output directory must be absolute");
    assert!(dir.is_dir(), "the output directory must exist");
    dir
}

/// 23 件の履歴（W-00 と W-01〜W-22）と基準の教材を、ブラウザ試験の入力として書き出す。
///
/// 呼び出し元は `scripts/test-browser.sh` だけで、`--ignored` を付けたときしか流れない。
/// ファイル名は固定の ID だけから作り、表の文字列をそのままパスにしない。
#[test]
#[ignore = "ブラウザ試験の入力を書き出す。scripts/test-browser.sh だけが --ignored で呼ぶ"]
fn emit_ops_rewrites_for_browser() {
    let out = emit_dir();
    let ir = ir_of();
    let ats: Vec<Option<usize>> = std::iter::once(None).chain((0..OPS).map(Some)).collect();
    for at in ats {
        let id = id_of(at);
        assert!(id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        std::fs::write(out.join(format!("{id}.history.json")), rewritten(&ir, at))
            .expect("write history");
    }
    let ir_bytes = serde_json::to_vec(&pack_json()).expect("to_vec");
    std::fs::write(out.join("base.ir.json"), ir_bytes).expect("write ir");
}
