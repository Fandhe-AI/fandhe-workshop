//! 版の違う履歴 15 件を、実施の前に固定した期待の表と突き合わせる試験（VER-2、TASK-175 1/2、#448）。
//!
//! 期待の表の本体は `testdata/version-histories/expectations.json` で、本試験は結果から表を導かない。
//! 履歴は現行のエンジンで試験の中で決定的に作り、`load_history`（段 1〜8 の実際の入口）で開く。
//! 表だけを書き換えて合計をずらす操作は、本ファイルの VER-2 の定数（件数・ID）との照合で検出する。
//! 食い違いは「期待より慎重な側」と「それ以外」に分けて数え、どちらも 0 件を求める（PoC-6 の 4 節の 3）。
//! 写しの一致と `Session` を作らないことの確認は、自動ブラウザの試験（`tools/browser/version-histories.test.mjs`、#449）が担う。
//! `open_history` は写しが一致したときだけ開くため、本試験の `state()` の検査は構造上ほぼ自明である。
//! ブラウザ試験が開く 15 件の履歴と教材は、同じ作り方を `emit_histories_for_browser`（`#[ignore]`）が書き出す
//! （`scripts/test-browser.sh` だけが `--ignored` で呼ぶ。バイト列を native の試験と同じにするため）。

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{OpSpec, Which, CMDS};
use common::record_path::RecordPath;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::{Version3, VersionAxis, VocabVersion};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::history_envelope::{build_envelope, SessionEnvelope};
use fandhe_workshop_core::history_load::load_history;
use fandhe_workshop_core::history_open::{Opened, OpeningCaps};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::version_record::{record_versions, record_versions_with};
use serde::Deserialize;
use serde_json::{json, Value};

const TABLE: &str = "version-histories/expectations.json";
const START: &str = "s-1";

/// VER-2 の ID の集合（表の順）。
const IDS: [&str; 15] = [
    "V-0", "V-i-1", "V-i-2", "V-ii-1", "V-ii-2", "V-iii-1", "V-iii-2", "V-iv-1", "V-iv-2",
    "V-iv-3", "V-iv-4", "V-v-1", "V-v-2", "V-v-3", "V-ref",
];
/// VER-2 の種ごとの件数。
const CATEGORY_COUNTS: [(Category, usize); 7] = [
    (Category::Same, 1),
    (Category::TextOnly, 2),
    (Category::StepAdded, 2),
    (Category::StepRemoved, 2),
    (Category::VocabFormat, 4),
    (Category::Engine, 3),
    (Category::Refs, 1),
];
/// VER-2 の結果ごとの件数（変換なし 1・変換して開く 6・表示だけ 5・開かない 3）。
const RESULT_COUNTS: [(Outcome, usize); 4] = [
    (Outcome::Open, 1),
    (Outcome::Converted, 6),
    (Outcome::ViewOnly, 5),
    (Outcome::Refused, 3),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Category {
    Same,
    TextOnly,
    StepAdded,
    StepRemoved,
    VocabFormat,
    Engine,
    Refs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Open,
    Converted,
    ViewOnly,
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Route {
    Early,
    Mid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum Pack {
    #[serde(rename = "base")]
    Base,
    #[serde(rename = "vocab-1.0")]
    Vocab10,
    #[serde(rename = "text-1.1.0")]
    Text110,
    #[serde(rename = "step-added-1.2.0")]
    StepAdded120,
    #[serde(rename = "step-removed-2.0.0")]
    StepRemoved200,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Axis {
    Engine,
    IrFormat,
    Vocab,
    Content,
    ContentHash,
    Refs,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    table_version: u32,
    rows: Vec<Row>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: String,
    category: Category,
    fabricated: bool,
    history: HistorySpec,
    opening: OpeningSpec,
    expect: Expect,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistorySpec {
    path: Route,
    pack: Pack,
    edit: Edit,
}

/// 版の欄だけの書き換え。持てるのはこの 3 欄だけで、`snapshot`・`ops` は書き換えない。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    engine: Option<String>,
    ir_format: Option<u32>,
    refs: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpeningSpec {
    pack: Pack,
    engine: Option<String>,
    vocab_max: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expect {
    result: Outcome,
    code: Option<String>,
    axes: Vec<Axis>,
}

/// 表を読む。固定の相対パスだけを辿り、未知の綴りは panic で落とす（既定値に倒さない）。
fn read_table() -> Table {
    let rel = Path::new(TABLE);
    assert!(
        rel.components().all(|c| matches!(c, Component::Normal(_))),
        "the table path must be a plain relative path"
    );
    let bytes =
        std::fs::read(testdata_dir().join(rel)).expect("expectations table must be readable");
    serde_json::from_slice(&bytes).expect("expectations table must match the schema")
}

fn base_json() -> Value {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    serde_json::from_slice(&std::fs::read(path).expect("read min-3step")).expect("parse")
}

/// 基準の教材 `base`。版・語彙・参照元を足す（名前と commit は架空のダミー）。
fn base_edit(v: &mut Value) {
    v["pack"]["version"] = json!("1.0.0");
    v["pack"]["vocab"] = json!("1.2");
    v["refs"] = json!([{ "name": "sample-ref", "commit": "0000000" }]);
}

impl Pack {
    /// 表と同じ綴りの名前（`Deserialize` の `rename` と一致させる）。ブラウザ試験の入力の置き場の名前にもなる。
    fn name(self) -> &'static str {
        match self {
            Pack::Base => "base",
            Pack::Vocab10 => "vocab-1.0",
            Pack::Text110 => "text-1.1.0",
            Pack::StepAdded120 => "step-added-1.2.0",
            Pack::StepRemoved200 => "step-removed-2.0.0",
        }
    }

    const ALL: [Pack; 5] = [
        Pack::Base,
        Pack::Vocab10,
        Pack::Text110,
        Pack::StepAdded120,
        Pack::StepRemoved200,
    ];
}

/// 教材の変種を作る。
fn pack_ir(pack: Pack) -> Ir {
    load_ir(&serde_json::to_vec(&pack_json(pack)).expect("to_vec")).expect("load_ir")
}

/// 教材の変種の IR の JSON（`load_ir` に渡す前の値）。ブラウザ試験の入力の書き出しも同じ値を使う。
fn pack_json(pack: Pack) -> Value {
    let mut v = base_json();
    base_edit(&mut v);
    match pack {
        Pack::Base => {}
        Pack::Vocab10 => v["pack"]["vocab"] = json!("1.0"),
        Pack::Text110 => {
            v["pack"]["version"] = json!("1.1.0");
            v["hints"]["h-1-1"]["text"] = json!("ノートを作る場面です。");
        }
        Pack::StepAdded120 => {
            let nodes = v["nodes"].as_array_mut().expect("nodes");
            let mut added = nodes[1].clone();
            added["uid"] = json!("main/st-1b");
            added["step"]["id"] = json!("st-1b");
            added["step"]["next"] = json!([{ "to": "main/st-2", "when": null }]);
            nodes[0]["step"]["next"] = json!([{ "to": "main/st-1b", "when": null }]);
            nodes.insert(1, added);
            v["pack"]["version"] = json!("1.2.0");
        }
        Pack::StepRemoved200 => {
            let nodes = v["nodes"].as_array_mut().expect("nodes");
            nodes.remove(1);
            nodes[0]["step"]["next"] = json!([{ "to": "main/st-3", "when": null }]);
            v["pack"]["version"] = json!("2.0.0");
        }
    }
    v
}

fn initial() -> SessionState {
    load().initial
}

fn current(state: &SessionState) -> String {
    state
        .run
        .current
        .as_ref()
        .map(|u| u.0.clone())
        .expect("current")
}

/// `early`: `st-1` の中で処理を終え、チェックポイントを 1 つ保存したところまで。
fn early(ir: &Ir) -> RecordPath<'_> {
    let mut path = RecordPath::new(ir, &initial());
    path.draft("/make-note first-note");
    path.spec(&OpSpec::Cmd(0));
    for _ in 0..5 {
        path.spec(&OpSpec::Tick(Which::Current));
    }
    assert_eq!(current(&path.state), "main/st-1");
    path.spec(&OpSpec::Save);
    path
}

/// `mid`: `early` に誤入力を足し、`st-1` を達成して次へ進め、未送信の下書きを残したところまで。
fn mid(ir: &Ir) -> RecordPath<'_> {
    let mut path = early(ir);
    let wrong = CMDS
        .iter()
        .position(|c| *c == "/nothing")
        .expect("/nothing");
    path.spec(&OpSpec::Cmd(wrong));
    path.spec(&OpSpec::Ui(0));
    assert_ne!(current(&path.state), "main/st-1", "the step must advance");
    path.draft("draft-mid");
    path
}

fn build(ir: &Ir, path: RecordPath<'_>) -> SessionEnvelope {
    let RecordPath { state, rec, .. } = path;
    build_envelope(ir, START, rec.into_ops(), state).expect("build")
}

fn make_history(spec: &HistorySpec) -> SessionEnvelope {
    let ir = pack_ir(spec.pack);
    let path = match spec.path {
        Route::Early => early(&ir),
        Route::Mid => mid(&ir),
    };
    let mut h = build(&ir, path);
    if let Some(e) = &spec.edit.engine {
        h.versions.engine.clone_from(e);
    }
    if let Some(f) = spec.edit.ir_format {
        h.versions.ir_format = f;
    }
    if let Some(refs) = &spec.edit.refs {
        for (k, v) in refs {
            h.versions.refs.insert(k.clone(), v.clone());
        }
    }
    h
}

/// 開く。履歴は直列化して `load_history` に渡す（段 1〜8 の実際の経路）。
fn open_row(row: &Row) -> (Opened, SessionEnvelope) {
    let h = make_history(&row.history);
    let ir = pack_ir(row.opening.pack);
    let hash = content_hash(&ir).expect("hash");
    let opening = match &row.opening.engine {
        None => record_versions(&ir, &hash),
        Some(e) => record_versions_with(&ir, Version3::parse(e).expect("engine version"), &hash),
    }
    .expect("opening versions");
    let caps = match &row.opening.vocab_max {
        None => OpeningCaps::current(),
        Some(v) => OpeningCaps::with_vocab_max(VocabVersion::parse(v).expect("vocab max")),
    };
    let bytes = serde_json::to_vec(&h).expect("serialize");
    (load_history(&bytes, &ir, &opening, &caps, initial()), h)
}

/// 軸の写し。`VersionAxis` の全種を網羅し、Debug 名に頼らない。
fn axis_of(a: VersionAxis) -> Axis {
    match a {
        VersionAxis::Engine => Axis::Engine,
        VersionAxis::IrFormat => Axis::IrFormat,
        VersionAxis::Vocab => Axis::Vocab,
        VersionAxis::Content => Axis::Content,
        VersionAxis::ContentHash => Axis::ContentHash,
        VersionAxis::Refs => Axis::Refs,
    }
}

fn result_of(o: &Opened) -> Outcome {
    match o {
        Opened::Open { .. } => Outcome::Open,
        Opened::Converted { .. } => Outcome::Converted,
        Opened::ViewOnly { .. } => Outcome::ViewOnly,
        Opened::Refused { .. } => Outcome::Refused,
    }
}

fn code_of(o: &Opened) -> Option<LoadReason> {
    match o {
        Opened::ViewOnly { code } | Opened::Refused { code } => Some(*code),
        _ => None,
    }
}

fn axes_of(o: &Opened) -> BTreeSet<Axis> {
    match o {
        Opened::Converted { notes, .. } => notes.diffs().iter().map(|d| axis_of(d.axis)).collect(),
        _ => BTreeSet::new(),
    }
}

/// 慎重さの順（開く 2 種は同順、表示だけ、開かない）。大きいほど慎重。
fn caution(r: Outcome) -> u8 {
    match r {
        Outcome::Open | Outcome::Converted => 0,
        Outcome::ViewOnly => 1,
        Outcome::Refused => 2,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Mismatch {
    /// 期待より慎重な側（開く期待が表示だけ・開かないになった、表示だけの期待が開かないになった）。
    Cautious,
    /// それ以外（危険な側への食い違い、理由・軸の取り違え、開く 2 種の取り違え）。
    Other,
}

/// 期待と結果を比べる。失敗の文には行 ID・結果の種類・コード・軸名だけを出し、履歴の中身は出さない。
fn compare(row: &Row, o: &Opened) -> Option<(Mismatch, String)> {
    let got = result_of(o);
    let got_code = code_of(o).map(LoadReason::as_str);
    let got_axes = axes_of(o);
    let want_axes: BTreeSet<Axis> = row.expect.axes.iter().copied().collect();
    if got == row.expect.result && got_code == row.expect.code.as_deref() && got_axes == want_axes {
        return None;
    }
    let kind = if caution(got) > caution(row.expect.result) {
        Mismatch::Cautious
    } else {
        Mismatch::Other
    };
    let msg = format!(
        "{}: expected {:?}/{:?}/{:?}, got {:?}/{:?}/{:?}",
        row.id, row.expect.result, row.expect.code, want_axes, got, got_code, got_axes
    );
    Some((kind, msg))
}

#[test]
fn table_shape_matches_ver2() {
    let t = read_table();
    assert_eq!(t.table_version, 1);
    let ids: Vec<&str> = t.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, IDS, "the 15 ids must match VER-2");
    for (c, n) in CATEGORY_COUNTS {
        assert_eq!(
            t.rows.iter().filter(|r| r.category == c).count(),
            n,
            "{c:?}"
        );
    }
    for (r, n) in RESULT_COUNTS {
        assert_eq!(
            t.rows.iter().filter(|x| x.expect.result == r).count(),
            n,
            "{r:?}"
        );
    }
    for r in &t.rows {
        let e = &r.expect;
        assert_eq!(
            e.result == Outcome::Converted,
            !e.axes.is_empty(),
            "{}: axes",
            r.id
        );
        let closed = matches!(e.result, Outcome::ViewOnly | Outcome::Refused);
        match &e.code {
            Some(c) => assert!(closed && !c.is_empty(), "{}: code", r.id),
            None => assert!(!closed, "{}: a closed result needs a code", r.id),
        }
        let edited = r.history.edit.engine.is_some()
            || r.history.edit.ir_format.is_some()
            || r.history.edit.refs.is_some();
        assert_eq!(
            r.fabricated, edited,
            "{}: fabricated marks edited versions",
            r.id
        );
    }
}

#[test]
fn fifteen_histories_match_the_fixed_expectations() {
    let t = read_table();
    let (mut cautious, mut other) = (Vec::new(), Vec::new());
    for row in &t.rows {
        let (o, h) = open_row(row);
        match compare(row, &o) {
            Some((Mismatch::Cautious, m)) => cautious.push(m),
            Some((Mismatch::Other, m)) => other.push(m),
            None => {}
        }
        // 開く 2 種だけが状態を持ち、表示だけ・開かないは状態を作らない。
        match result_of(&o) {
            Outcome::Open | Outcome::Converted => {
                assert_eq!(o.state(), Some(&h.snapshot), "{}", row.id)
            }
            Outcome::ViewOnly | Outcome::Refused => {
                assert!(o.state().is_none(), "{}: closed result has a state", row.id);
                assert!(
                    !code_of(&o).expect("code").as_str().is_empty(),
                    "{}",
                    row.id
                );
            }
        }
    }
    assert!(
        cautious.is_empty(),
        "mismatches on the cautious side: {cautious:?}"
    );
    assert!(other.is_empty(), "other mismatches: {other:?}");
}

#[test]
fn comparator_reports_a_changed_expectation() {
    let t = read_table();
    let mut row = t.rows[0].clone();
    assert_eq!(row.id, "V-0");
    let (o, _) = open_row(&row);
    assert!(compare(&row, &o).is_none());
    // 期待を 1 つ書き換えると、比較器は食い違いを 1 件返す（常に成功する比較器ではない）。
    row.expect.result = Outcome::Converted;
    row.expect.axes = vec![Axis::Content];
    let (kind, _) = compare(&row, &o).expect("a changed expectation must mismatch");
    assert_eq!(kind, Mismatch::Other);
    // 開く期待が表示だけになった場合は慎重な側に分類される。
    let mut row = t.rows[0].clone();
    row.expect.result = Outcome::ViewOnly;
    row.expect.code = Some("content_major".to_owned());
    let (kind, _) = compare(&row, &o).expect("mismatch");
    assert_eq!(
        kind,
        Mismatch::Other,
        "got open, expected closed is the other side"
    );
}

/// ブラウザ試験の入力を書き出す先（絶対パスのディレクトリ）。未設定・相対・ディレクトリでなければ panic する。
fn emit_dir() -> std::path::PathBuf {
    let raw = std::env::var_os("FANDHE_VERSION_HISTORIES_OUT")
        .expect("FANDHE_VERSION_HISTORIES_OUT must be set");
    let dir = std::path::PathBuf::from(raw);
    assert!(dir.is_absolute(), "the output directory must be absolute");
    assert!(dir.is_dir(), "the output directory must exist");
    dir
}

/// 15 件の履歴と開く側の教材を、ブラウザ試験の入力として書き出す（VER-2、#449）。
///
/// 呼び出し元は `scripts/test-browser.sh` だけで、`--ignored` を付けたときしか流れない（通常の `make test` は書き出さない）。
/// ファイル名は固定の ID と `Pack` の名前だけから作り、表の文字列をそのままパスにしない。
#[test]
#[ignore = "ブラウザ試験の入力を書き出す。scripts/test-browser.sh だけが --ignored で呼ぶ"]
fn emit_histories_for_browser() {
    let out = emit_dir();
    let t = read_table();
    let ids: Vec<&str> = t.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, IDS, "the 15 ids must match VER-2");
    std::fs::create_dir_all(out.join("packs")).expect("create packs dir");
    for (row, id) in t.rows.iter().zip(IDS) {
        assert!(
            id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "{id}"
        );
        let bytes = serde_json::to_vec(&make_history(&row.history)).expect("serialize");
        std::fs::write(out.join(format!("{id}.history.json")), bytes).expect("write history");
    }
    for pack in Pack::ALL {
        let bytes = serde_json::to_vec(&pack_json(pack)).expect("serialize");
        std::fs::write(
            out.join("packs").join(format!("{}.ir.json", pack.name())),
            bytes,
        )
        .expect("write pack");
    }
}
