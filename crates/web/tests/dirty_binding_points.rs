//! 更新の dirty field が束縛点と keyed list に対応することの試験（TASK-348 1/3。FEND-3）。
//!
//! 上流の `Runtime::apply_update_for_dirty` は、mount 時に走査した束縛点の表にも DOM の `data-bind-list` にも無い
//! field が dirty に 1 件でもあると、`root` の全子ノードを作り直す（構造フォールバック）。全置換が起きると、
//! フォーカス・入力途中の値・要素の同一性が失われる。ここでは、33 回の更新（入力の受付・進行・保存・確認・
//! ビューの選択）の各回で、dirty の field が `view()` の束縛点（`data-bind-text`・`data-bind-attr`・
//! `data-bind-class`）か keyed list（`data-bind-list`）に対応し、目印の集合が mount 時から変わらないことを確かめる。
//! 上流の判定条件を `view()` の目印で模した native の確認で、実 DOM での要素の同一性・フォーカスの保持（B-7）と
//! `rerender` の負の対照（B-7n）は、それぞれ別のブラウザ試験の担当である。
//! 試験では `unwrap` を使ってよい。失敗の文面には field 名・更新の番号・action の種類名だけを出し、
//! 教材由来の値は出さない。

use std::collections::BTreeSet;
use std::path::PathBuf;

use fandhe_frontend_core::keyed::BIND_LIST_ATTR;
use fandhe_frontend_core::Node;
use fandhe_frontend_wasm_client::{collect_binding_specs, unresolved_binding_specs};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::artifact_view::ViewKind;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::{Component, DirtyTracked};
use serde_json::{json, Value};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn initial_state() -> SessionState {
    let sample = read_json(testdata().join("contract/event-sequence.json"));
    serde_json::from_value(sample["initial"].clone()).unwrap()
}

fn host_of_ir(ir: Ir) -> EngineHost {
    EngineHost::with_session(
        Session::from_state(ir, initial_state()),
        standard_registry(),
    )
    .unwrap()
}

/// `min-3step` に 3 種のビューを足した教材のホスト。既存の `views`（`v-files`）は、sample の `ui` op が
/// 参照するため消さずに残す。
fn multi_view_host() -> EngineHost {
    let mut v = read_json(testdata().join("engine-event-consistency/test-data/min-3step.json"));
    let views = v["views"].as_object_mut().unwrap();
    views.insert("v-diff".into(), json!({"kind": "diff"}));
    views.insert("v-card".into(), json!({"kind": "card_list"}));
    views.insert("v-tree".into(), json!({"kind": "file_tree"}));
    for node in v["nodes"].as_array_mut().unwrap() {
        if node["uid"] == "main/st-1" {
            node["step"]["views"] = json!(["v-diff", "v-card", "v-tree"]);
        }
    }
    host_of_ir(load_ir(&serde_json::to_vec(&v).unwrap()).unwrap())
}

/// sample と同じ JSON の形から `Op` を作る（`Op` の内部の型を試験に持ち込まない）。
fn op_of(v: Value) -> Op {
    serde_json::from_value(v).unwrap()
}

/// 描画された目印の集合。束縛点（text・attr・class）の field と keyed list の field に分ける。
#[derive(Debug, PartialEq, Eq, Clone)]
struct Markers {
    bound: BTreeSet<String>,
    keyed: BTreeSet<String>,
}

impl Markers {
    fn of(node: &Node) -> Self {
        let bound = collect_binding_specs(node)
            .into_iter()
            .map(|s| s.field)
            .collect();
        let mut keyed = BTreeSet::new();
        collect_keyed(node, &mut keyed);
        Markers { bound, keyed }
    }

    fn contains(&self, field: &str) -> bool {
        self.bound.contains(field) || self.keyed.contains(field)
    }

    fn all(&self) -> BTreeSet<String> {
        self.bound.union(&self.keyed).cloned().collect()
    }
}

fn collect_keyed(node: &Node, out: &mut BTreeSet<String>) {
    if let Node::Element {
        attrs, children, ..
    } = node
    {
        for (k, v) in attrs {
            if k == BIND_LIST_ATTR {
                out.insert(v.clone());
            }
        }
        for c in children {
            collect_keyed(c, out);
        }
    }
}

/// 更新の列を流し、各回の条件を確かめる器。
struct Run {
    host: EngineHost,
    mount: Markers,
    n: usize,
    with_dirty: usize,
    seen: BTreeSet<&'static str>,
}

impl Run {
    fn new(host: EngineHost) -> Self {
        let mount = Markers::of(&host.view());
        Run {
            host,
            mount,
            n: 0,
            with_dirty: 0,
            seen: BTreeSet::new(),
        }
    }

    /// 1 回の更新を行い、dirty の field が mount 時の目印に対応すること、更新後の目印の集合が mount 時と
    /// 同じであること、値を答えない束縛点が無いことを確かめる。
    fn step(&mut self, action: HostAction, label: &str) {
        self.host.update(action);
        self.n += 1;
        let dirty: Vec<&'static str> = self.host.dirty_fields().to_vec();
        if !dirty.is_empty() {
            self.with_dirty += 1;
        }
        for f in &dirty {
            assert!(
                self.mount.contains(f),
                "束縛点の無い dirty field {f}（更新 #{}・{label}）→ root の全置換になる",
                self.n
            );
            self.seen.insert(f);
        }
        let now = Markers::of(&self.host.view());
        assert_eq!(
            now, self.mount,
            "目印の集合が mount 時から変わった（更新 #{}・{label}）",
            self.n
        );
        let unresolved: Vec<String> = unresolved_binding_specs(&self.host.view(), &self.host)
            .into_iter()
            .map(|s| s.field)
            .collect();
        assert!(
            unresolved.is_empty(),
            "値を答えない束縛点 {unresolved:?}（更新 #{}・{label}）",
            self.n
        );
    }

    fn input(&mut self, name: &str, payload: &str) {
        let action = EngineHost::decode_action(name, payload).unwrap();
        self.step(action, name);
    }

    fn op(&mut self, op: Op, label: &str) {
        self.step(HostAction::Engine(op), label);
    }

    /// 次の進行の宛先があれば、その進行を 1 回送る。
    fn tick(&mut self) {
        let addr = self.host.next_tick().expect("処理中のはず");
        self.op(Op::Tick(addr), "tick");
    }

    fn select(&mut self, kind: ViewKind) {
        self.step(HostAction::SelectView(kind), "select-view");
    }
}

/// 33 回の更新の列。入力の受付・進行・保存・確認・ビューの選択を含む。
fn run_33() -> Run {
    let mut r = Run::new(multi_view_host());
    // 入力の受付（10 回）。
    r.input("draft", "/");
    r.input("draft", "/m");
    r.input("draft", &"a".repeat(20_001));
    r.input("draft", "ok");
    r.input("esc", "Escape");
    r.input("draft", "/make-note Bad_Name");
    r.input("submit", "");
    r.input("draft", "/make-note first-note");
    r.input("submit", "");
    r.input("complete", "");
    // 進行（2 回）と停止、受け付け直し（4 回）、残りの進行（5 回）。
    r.tick();
    r.tick();
    r.input("stop", "");
    r.input("draft", "/make-note first-note");
    r.input("submit", "");
    r.input("complete", "");
    for _ in 0..5 {
        r.tick();
    }
    // 保存（1 回）。
    r.op(Op::Control(Control::Save), "save");
    // ビューの選択（3 回）。
    r.select(ViewKind::CardList);
    r.select(ViewKind::FileTree);
    r.select(ViewKind::Diff);
    // 確認と選択（2 回）。
    r.op(
        op_of(json!({"input": {"ui": {"view": "v-files", "path": "notes/draft"}}})),
        "ui",
    );
    r.input("choose", "opt-memo");
    // 移動・復帰・範囲外・処理中でない停止（5 回）。
    r.op(op_of(json!({"control": {"go_to": "main/st-1"}})), "go_to");
    r.op(op_of(json!({"control": {"restore": 1}})), "restore");
    r.op(op_of(json!({"control": {"restore": 99}})), "restore-oob");
    r.op(
        op_of(json!({"control": {"go_to": "main/st-9"}})),
        "go_to-oob",
    );
    r.op(Op::Control(Control::Stop), "stop-idle");
    // 候補を開き直す（1 回）。
    r.input("draft", "/");
    r
}

/// 33 回の更新で、dirty になったことを確かめる 11 種の field。空振り（何も dirty にならない列）を検出する。
const EXPECTED_SEEN: &[&str] = &[
    "input_notice",
    "input_candidates",
    "learner_log",
    "engine_log_rows",
    "engine_log_consumed",
    "run_status_word",
    "run_status_state",
    "artifact_consumed",
    "review_items",
    "review_consumed",
    "offer_choices",
];

/// 33 回の列では dirty にできない field。理由は各行のコメントに書く。除外は最小にする。
const NEEDS_OTHER_FIXTURES: &[&str] = &[
    // 遷移 1,000 回の上限の教材が要り、その教材は試験の設定（`test-hooks`）にだけある。
    "log_notice",
    // 保存の失敗の表示は、保存の口（`set_history_saver`）を結んだホストが要る。`save_failure.rs` が dirty を確かめる。
    "save_notice",
    // 33 回の列の教材が宣言していない種類のビューの list。選ばれない種類は空・`hidden` で描かれるだけで、
    // dirty にするには種類ごとの宣言を持つ教材が要る。
    "view_app_demo",
    "view_check_list",
    "view_document",
    "view_task_tree",
];

#[test]
fn 更新_33_回の_dirty_field_はすべて束縛点か_keyed_list_に対応し_11_種以上を確かめる() {
    let r = run_33();
    println!(
        "updates={} with_dirty={} dirty_fields_seen={:?}",
        r.n, r.with_dirty, r.seen
    );
    assert_eq!(r.n, 33);
    for f in EXPECTED_SEEN {
        assert!(r.seen.contains(f), "一度も dirty にならなかった field {f}");
    }
    assert!(r.seen.len() >= 11, "dirty の種類 {}", r.seen.len());
}

#[test]
fn 描画される_field_はすべて一度は_dirty_になる_他の教材が要るものを除く() {
    let r = run_33();
    let missing: Vec<String> = r
        .mount
        .all()
        .into_iter()
        .filter(|f| !r.seen.contains(f.as_str()) && !NEEDS_OTHER_FIXTURES.contains(&f.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "一度も dirty にならない field {missing:?}"
    );
}

/// 承認を持つ教材のホスト（`approval_buttons.rs` の `host_with` と同じ要領）。
fn approval_host() -> EngineHost {
    let mut v = read_json(testdata().join("engine-event-consistency/test-data/min-3step.json"));
    let rule = |id: &str, on: Value, accept: bool| {
        if accept {
            json!({"id": id, "on": on, "outcome": {"kind": "accept"}})
        } else {
            json!({"id": id, "on": on, "outcome": {"kind": "reject", "rejection": "rj-decline"}})
        }
    };
    let rules = vec![
        rule(
            "r-t0",
            json!({"approval": "revise", "target": "t-alpha"}),
            true,
        ),
        rule("r-bare", json!({"approval": "revise"}), false),
        rule("r-early", json!({"approval": "approve"}), false),
    ];
    let step = v["nodes"][0]["step"].as_object_mut().unwrap();
    step.insert("input".into(), json!({"kind": "approval"}));
    step.insert("rules".into(), Value::Array(rules));
    step.insert("confirm".into(), json!({"kind": "approve"}));
    host_of_ir(load_ir(&serde_json::to_vec(&v).unwrap()).unwrap())
}

#[test]
fn 承認の確認でも_dirty_field_は束縛点か_keyed_list_に対応する() {
    let mut r = Run::new(approval_host());
    r.input("decide", "approve");
    r.input("decide", "revise");
    r.input("revise-target", "t-alpha");
    while r.host.next_tick().is_some() {
        r.tick();
        assert!(r.n < 40, "進行が終わらない");
    }
    r.input("decide", "reject");
    r.input("decide", "approve");
    println!("approval updates={} dirty_fields_seen={:?}", r.n, r.seen);
    assert!(r.with_dirty >= 3, "dirty を積んだ更新の数 {}", r.with_dirty);
    assert!(r.seen.contains("offer_choices"));
}

#[test]
fn 入力欄なしの教材では入力の_action_が_dirty_を積まず_他の更新も対応する() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/input_field_hidden/ir.json");
    let mut r = Run::new(host_of_ir(load_ir(&std::fs::read(p).unwrap()).unwrap()));
    assert!(
        !r.mount.contains("input_candidates"),
        "入力欄なしの教材に候補の list がある"
    );
    for (name, payload) in [
        ("draft", "/make-note abc"),
        ("submit", ""),
        ("complete", ""),
    ] {
        r.input(name, payload);
        assert!(
            r.host.dirty_fields().is_empty(),
            "入力欄なしで {name} が dirty を積んだ"
        );
    }
    r.op(Op::Control(Control::Save), "save");
    r.input("choose", "opt-memo");
    r.select(ViewKind::Diff);
    r.op(op_of(json!({"control": {"go_to": "main/st-1"}})), "go_to");
    r.input("stop", "");
    println!("hidden updates={} dirty_fields_seen={:?}", r.n, r.seen);
}
