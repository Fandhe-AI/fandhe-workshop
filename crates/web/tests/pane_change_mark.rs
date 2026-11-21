//! 見えていない側に変化があったタブの印（SHELL-5・A11Y-21。#1038。#1013 の決定）の試験。
//!
//! 選ばれていないタブの側に変化があると、そのタブの名前に「（変化あり）」が付き（`shell_frame::TAB_CHANGED_SUFFIX`）、
//! クラス `sh-tab-changed` が付く。そのタブを選ぶと消える。左は世界状態の変化、右は応答の行で変化とみなす。
//! 選ばれている側の変化では印を付けない。完成した画面は `shell`・`a11y` の検査を通る。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::shell_frame::{TAB_CHANGED_CLASS, TAB_CHANGED_SUFFIX};
use fandhe_workshop_web::{a11y, shell};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn host() -> EngineHost {
    let ir = load_ir(
        &std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
            .unwrap(),
    )
    .unwrap();
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let initial: SessionState = serde_json::from_value(v["initial"].clone()).unwrap();
    let mut h =
        EngineHost::with_session(Session::from_state(ir, initial), standard_registry()).unwrap();
    h.boot(None);
    h
}

fn act(h: &mut EngineHost, name: &str, payload: &str) {
    h.update(EngineHost::decode_action(name, payload).unwrap());
}

/// タブ（`pane-tab`）の `id` と、その子の文字・クラス。
fn tabs(v: &Node) -> Vec<(String, String, String)> {
    fn walk(n: &Node, out: &mut Vec<(String, String, String)>) {
        if let Node::Element {
            attrs, children, ..
        } = n
        {
            let get = |k: &str| {
                attrs
                    .iter()
                    .find(|(a, _)| a == k)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            };
            if get("data-testid") == "pane-tab" {
                let label = format!("{children:?}");
                out.push((get("id"), label, get("class")));
            }
            for c in children {
                walk(c, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(v, &mut out);
    out
}

/// 印のあるタブの `id`。名前の文字とクラスが食い違えば失敗する。
fn marked(h: &EngineHost) -> Vec<String> {
    let v = h.view();
    shell::check_complete_v2(&v).unwrap();
    a11y::check_no_undefined(&v).unwrap();
    tabs(&v)
        .into_iter()
        .filter_map(|(id, label, class)| {
            let by_text = label.contains(TAB_CHANGED_SUFFIX);
            let by_class = class == TAB_CHANGED_CLASS;
            assert_eq!(by_text, by_class, "{id}: 文字とクラスが食い違う");
            by_text.then_some(id)
        })
        .collect()
}

#[test]
fn shell5_unseen_left_change_marks_the_artifacts_tab_until_it_is_selected() {
    let mut h = host();
    assert!(marked(&h).is_empty(), "開いた直後は印なし");
    // 操作のタブのまま受け付けると、左（成果物）に行が増える。右は見えているので印を付けない。
    act(&mut h, "draft", "/make-note first-note");
    act(&mut h, "submit", "");
    assert_eq!(marked(&h), vec!["pane-tab-artifacts".to_owned()]);
    act(&mut h, "select-pane", "artifacts");
    assert!(marked(&h).is_empty(), "選ぶと消える");
}

#[test]
fn shell5_unseen_right_change_marks_the_ops_tab() {
    let mut h = host();
    act(&mut h, "select-pane", "artifacts");
    // 成果物のタブのまま断られる入力を送ると、右（操作）に拒否の行が増える。左は変わらない。
    act(&mut h, "draft", "/zzz");
    act(&mut h, "submit", "");
    assert_eq!(marked(&h), vec!["pane-tab-ops".to_owned()]);
    act(&mut h, "select-pane", "ops");
    assert!(marked(&h).is_empty());
}

#[test]
fn shell5_marks_do_not_survive_a_history_swap() {
    // 印が付いた状態で別の履歴を開くと、前の履歴の印は残らない。
    let mut h = host();
    act(&mut h, "draft", "/make-note first-note");
    act(&mut h, "submit", "");
    assert_eq!(marked(&h), vec!["pane-tab-artifacts".to_owned()]);
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let ir = load_ir(
        &std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
            .unwrap(),
    )
    .unwrap();
    let env = fandhe_workshop_core::history_load::parse_history_envelope(
        fandhe_workshop_core::history_load::parse_history_json(&sample).unwrap(),
    )
    .unwrap();
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let initial: SessionState = serde_json::from_value(v["initial"].clone()).unwrap();
    let state = fandhe_workshop_core::history_replay::replay(&ir, initial, &env.ops).unwrap();
    let built =
        fandhe_workshop_core::history_envelope::build_envelope(&ir, &env.start, env.ops, state)
            .unwrap();
    let bytes = fandhe_workshop_core::history_export::export_bytes(&built).unwrap();
    assert_eq!(
        h.open_history(&bytes),
        fandhe_workshop_web::host::ImportOutcome::Open
    );
    assert!(marked(&h).is_empty(), "差し替えると印は消える");
}

#[test]
fn shell5_a_progress_line_alone_marks_the_ops_tab() {
    // 成果物のタブを見ている間に、効果の無い進行（右のログの進行の行だけが増える）が届くと、操作のタブに印が付く。
    let mut h = host();
    act(&mut h, "draft", "/make-note first-note");
    act(&mut h, "submit", "");
    act(&mut h, "select-pane", "artifacts");
    assert!(marked(&h).is_empty());
    let t = h.next_tick().expect("処理中");
    act(
        &mut h,
        "tick",
        &format!("{} {} {}", t.generation.0, t.index, t.step.0),
    );
    assert_eq!(marked(&h), vec!["pane-tab-ops".to_owned()]);
}

#[test]
fn shell5_progress_lines_hidden_by_reduced_motion_do_not_mark_the_ops_tab() {
    // 動きを抑える設定では進行の行を出さない（A11Y-11）。出ない行では操作のタブに印を付けない。
    let mut h = host();
    h.set_reduced_motion(true);
    act(&mut h, "draft", "/make-note first-note");
    act(&mut h, "submit", "");
    act(&mut h, "select-pane", "artifacts");
    let t = h.next_tick().expect("処理中");
    act(
        &mut h,
        "tick",
        &format!("{} {} {}", t.generation.0, t.index, t.step.0),
    );
    assert!(marked(&h).is_empty(), "{:?}", marked(&h));
}
