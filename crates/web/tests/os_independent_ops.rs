//! 教材の 4 操作（Enter で送信・Shift+Enter で改行・Tab で補完・Esc で停止）が、OS の推定値
//! （macOS・Windows・推定なし・途中での選び直し）によらず同じ結果になることの試験（TASK-392 2/2。A11Y-25・D-48）。
//!
//! OS の値は表示（`key-hint`・`op-guide`・選ぶ欄の `selected`）だけに使い、キーの判定
//! （`contract::key_decide::decide`・`web::key_route::effect_for`）と Session・履歴は OS を知らない。
//! この前提をホストの水準で回帰として固定する。確かめないこと: Windows の実機・実キーイベントの OS 差・
//! ブラウザの既定の動作（`enter-submit`・`modifier-key-right-pane` の試験が持つ）。
//! 呼び出し元は CI の `make test` のみ。変異は `mutations/A11Y25-3`・`A11Y25-4`。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::key_decide::{decide, KeyInput};
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::learner_os::{estimate, LearnerOs};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::key_route::{effect_for, field_state, KeydownEffect};

const OS_FIELDS: [&str; 2] = ["key_hint", "op_guide"];

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn begun_host(os: LearnerOs) -> EngineHost {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    let session = Session::from_state(load_ir(&bytes).unwrap(), state);
    let mut host = EngineHost::with_session(session, standard_registry()).unwrap();
    host.set_learner_os(os);
    host
}

fn act(host: &mut EngineHost, name: &str, payload: &str) {
    host.update(EngineHost::decode_action(name, payload).expect(name));
}

fn make_note() -> HostAction {
    HostAction::Engine(Op::Input(Input::Command(
        "/make-note first-note".to_owned(),
    )))
}

fn tick0() -> HostAction {
    HostAction::Engine(Op::Tick(TickAddr {
        generation: Generation(1),
        step: StepUid::parse("main/st-1").unwrap(),
        index: 0,
    }))
}

fn key(name: &str, shift: bool) -> KeyInput {
    KeyInput {
        key: name.to_owned(),
        shift,
        ..KeyInput::default()
    }
}

fn effect(input: &KeyInput, has_text: bool, open: bool) -> KeydownEffect {
    effect_for(decide(input, &field_state(has_text, open)))
}

/// 操作の後の観察を 1 つの文字列にする。OS の表示の dirty（`key_hint`・`op_guide`）だけは除く。
fn snapshot(host: &EngineHost) -> String {
    let session = host.model().session().unwrap();
    let dirty: Vec<&str> = host
        .dirty_fields()
        .iter()
        .copied()
        .filter(|f| !OS_FIELDS.contains(f))
        .collect();
    format!(
        "log={:?}\ngen={:?} busy={:?}\ndraft={:?} open={}\nhistory={:?}\ndirty={:?}",
        session.log(),
        session.state().run.generation,
        session.state().run.busy,
        host.draft(),
        host.candidates_open(),
        host.history_to_save(),
        dirty
    )
}

fn stopped_count(host: &EngineHost) -> usize {
    host.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter(|e| matches!(e, Event::Stopped { .. }))
        .count()
}

/// 4 操作を順に行い、各操作の後の観察と、期待（送信でログが増える等）の確認結果を返す。
/// `switch` があれば、各操作の直前にその OS へ選び直す（受講者が選ぶ欄で切り替える）。
fn run_ops(os: LearnerOs, switch: Option<&str>) -> Vec<String> {
    let mut host = begun_host(os);
    let mut snaps = Vec::new();
    let pick = |host: &mut EngineHost| {
        if let Some(v) = switch {
            act(host, "os", v);
        }
    };

    // 1. Enter で送信
    let e = effect(&key("Enter", false), true, false);
    assert!(e.click_submit && e.prevent_default, "{e:?}");
    act(&mut host, "draft", "hello");
    pick(&mut host);
    let len0 = host.model().session().unwrap().log().len();
    act(&mut host, "submit", "");
    assert!(host.model().session().unwrap().log().len() > len0);
    snaps.push(format!("{e:?}\n{}", snapshot(&host)));

    // 2. Shift+Enter で改行
    let e = effect(&key("Enter", true), true, false);
    assert!(!e.click_submit && !e.prevent_default, "{e:?}");
    act(&mut host, "draft", "a\nb");
    pick(&mut host);
    let len1 = host.model().session().unwrap().log().len();
    assert_eq!(host.draft(), "a\nb");
    snaps.push(format!("{e:?}\n{}", snapshot(&host)));
    assert_eq!(host.model().session().unwrap().log().len(), len1);

    // 3. Tab で補完
    act(&mut host, "draft", "/he");
    assert!(host.candidates_open());
    pick(&mut host);
    let e = effect(&key("Tab", false), true, true);
    assert!(e.deliver_complete && e.prevent_default, "{e:?}");
    act(&mut host, "complete", "");
    assert_eq!(host.draft(), "/help");
    assert!(!host.candidates_open());
    snaps.push(format!("{e:?}\n{}", snapshot(&host)));

    // 4. Esc で停止（処理中）
    act(&mut host, "draft", "");
    host.update(make_note());
    host.update(tick0());
    pick(&mut host);
    let gen0 = host.model().session().unwrap().state().run.generation.0;
    let stopped0 = stopped_count(&host);
    act(&mut host, "esc", "Escape");
    let run = &host.model().session().unwrap().state().run;
    assert_eq!(run.generation.0, gen0 + 1);
    assert!(run.busy.is_none());
    assert_eq!(stopped_count(&host), stopped0 + 1);
    snaps.push(snapshot(&host));
    snaps
}

/// 列の一致を判定する。不一致なら最初の食い違いの位置を返す（検出器の対照でも使う）。
fn first_diff(a: &[String], b: &[String]) -> Option<usize> {
    if a.len() != b.len() {
        return Some(a.len().min(b.len()));
    }
    a.iter().zip(b).position(|(x, y)| x != y)
}

#[test]
fn 四つの操作の結果は_os_の推定値によらず同じ() {
    let base = run_ops(LearnerOs::Mac, None);
    let from_ua = estimate("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0");
    for (name, os) in [
        ("windows", LearnerOs::Windows),
        ("unknown", LearnerOs::Unknown),
        ("ua-windows", from_ua),
        (
            "ua-linux",
            estimate("Mozilla/5.0 (X11; Linux x86_64) Chrome/120.0"),
        ),
    ] {
        let got = run_ops(os, None);
        assert_eq!(first_diff(&base, &got), None, "{name}");
    }
}

#[test]
fn os_を途中で切り替えても操作の結果は変わらない() {
    let base = run_ops(LearnerOs::Mac, None);
    for (name, os, to) in [
        ("mac→windows", LearnerOs::Mac, "windows"),
        ("windows→mac", LearnerOs::Windows, "mac"),
        ("unknown→mac", LearnerOs::Unknown, "mac"),
    ] {
        let got = run_ops(os, Some(to));
        assert_eq!(first_diff(&base, &got), None, "{name}");
    }
}

#[test]
fn 切り替えで立つ_dirty_は案内の二つだけで下書きと候補は動かない() {
    let mut host = begun_host(LearnerOs::Mac);
    act(&mut host, "draft", "/he");
    assert!(host.candidates_open());
    act(&mut host, "os", "windows");
    assert_eq!(host.dirty_fields(), OS_FIELDS);
    assert_eq!(host.draft(), "/he");
    assert!(host.candidates_open());
}

/// 案内の文字（`key-hint`・`op-guide` の本文）と `selected` を除く。
fn normalize(html: &str) -> String {
    let mut out = html.replace(" selected=\"\"", "");
    for id in ["key-hint", "op-guide"] {
        let marker = format!("data-testid=\"{id}\"");
        let start = out.find(&marker).expect(id);
        let body = out[start..].find('>').unwrap() + start + 1;
        let end = out[body..].find("</p>").unwrap() + body;
        out.replace_range(body..end, "");
    }
    out
}

#[test]
fn os_によって描画が変わるのは案内の文字と選ぶ欄の選択だけ() {
    let html = |os| render_component_html(&begun_host(os));
    let mac = html(LearnerOs::Mac);
    let win = html(LearnerOs::Windows);
    let unk = html(LearnerOs::Unknown);
    assert_ne!(mac, win, "OS で案内が変わらない（対照が空）");
    assert_eq!(normalize(&mac), normalize(&win));
    assert_eq!(normalize(&mac), normalize(&unk));
    // 正規化が広すぎて差を消していないこと。キーの経路の属性の数は条件で同じで、0 でない。
    let attrs = |h: &str| h.matches("data-action").count() + h.matches("data-keydown").count();
    assert!(attrs(&mac) > 0);
    assert_eq!(attrs(&mac), attrs(&win));
    assert_eq!(attrs(&mac), attrs(&unk));
}

#[test]
fn 比較の検出器は一か所の違いを見つける() {
    let base = run_ops(LearnerOs::Mac, None);
    let mut other = base.clone();
    other[2].push('x');
    assert_eq!(first_diff(&base, &other), Some(2));
    assert_eq!(first_diff(&base, &base[..3]), Some(3));
    assert_eq!(first_diff(&base, &base), None);
}
