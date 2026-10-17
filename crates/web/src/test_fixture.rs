//! ブラウザ試験が mount するホストの開始状態（試験の設定 `test-hooks` だけ。TASK-347 2/2）。
//!
//! 呼び出し元は `host::EngineHost::for_mount` だけである。`lib.rs` が feature `test-hooks` の背後でだけ
//! 組み込むため、配信する設定の wasm には入らない（`tools/check-delivery.sh` が教材の文字列の不在を確かめる）。
//!
//! 開始（`Control::Begin`）はまだ core が実装していないため、契約の試料（`event-sequence.json`）が持つ
//! 開始直後の状態から始める。状態からの `Session` の再構築は、表示層で他に許されない操作（`tests/no_progress.rs`）で、
//! このファイルの 1 箇所だけを例外にしている。教材と状態は埋め込みの試験データで、外から入らない。
//! 確認が `answer` の教材への差し替え（`answer_session`）は、`load-fixture:answer` の合図で使う（TASK-445 2/2）。
//! 履歴ファイルの読み込みの試験（TASK-330 2/2）が使う開ける包み（`loadable_envelope`）もここで作る。

use fandhe_workshop_core::{load_ir, Session, SessionState};

const IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/engine-event-consistency/test-data/min-3step.json"
));
const SAMPLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/contract/event-sequence.json"
));

/// 試験用の教材と開始直後の状態から実行環境を作る。読めなければ `None`（呼び出し側は session なしに倒す）。
pub(crate) fn start_session() -> Option<Session> {
    session_of(IR)
}

/// 教材のバイト列と開始直後の状態から実行環境を作る。`Session::from_state` の唯一の呼び出し（`tests/no_progress.rs`）。
fn session_of(ir_bytes: &[u8]) -> Option<Session> {
    let ir = load_ir(ir_bytes).ok()?;
    Some(Session::from_state(ir, start_state()?))
}

/// 確認が `answer` の Step を持つ試験用の実行環境（TASK-445 2/2。`load-fixture:answer` が差し替える）。
///
/// 埋め込みの教材の `main/st-1` の確認を、選択肢 2 件（誤り・正解）の `answer` に置き換える。誤答の文面は教材が
/// もともと持つ `rj-decline` を指す。mount の教材（`start_session`）は、st-1 の `point_at` に依る既存の試験のため変えない。
/// 読めない・節点が無い・`load_ir` を通らないときは `None`（呼び出し側は何もしない）。
#[cfg(feature = "test-hooks")]
pub(crate) fn answer_session() -> Option<Session> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let node = value
        .get_mut("nodes")?
        .as_array_mut()?
        .iter_mut()
        .find(|n| n.get("uid").and_then(|u| u.as_str()) == Some("main/st-1"))?;
    node.get_mut("step")?.as_object_mut()?.insert(
        "confirm".to_owned(),
        serde_json::json!({
            "kind": "answer",
            "options": [
                { "id": "opt-wrong", "correct": false, "label": "誤った案" },
                { "id": "opt-right", "correct": true, "label": "正しい案" }
            ],
            "on_wrong": "rj-decline"
        }),
    );
    session_of(&serde_json::to_vec(&value).ok()?)
}

/// 入力方式が `approval` で、対象つきの「直す」の規則を持つ試験用の実行環境（TASK-447 1/2。`load-fixture:approval`）。
///
/// 埋め込みの教材の `main/st-1` を、対象 2 つ（`t-alpha`・`t-beta`）の「直す」で受付、対象なしの「直す」と
/// 早すぎる「はい」を既存の `rj-decline`（次の行動つき）で拒否する規則に置き換える。確認は `approve`。
/// 規則は細い形（`null` の欄なし）で書く。太い形の読みは core の試験が固定する。
#[cfg(feature = "test-hooks")]
pub(crate) fn approval_session() -> Option<Session> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let node = value
        .get_mut("nodes")?
        .as_array_mut()?
        .iter_mut()
        .find(|n| n.get("uid").and_then(|u| u.as_str()) == Some("main/st-1"))?;
    let step = node.get_mut("step")?.as_object_mut()?;
    step.insert(
        "input".to_owned(),
        serde_json::json!({ "kind": "approval" }),
    );
    step.insert(
        "confirm".to_owned(),
        serde_json::json!({ "kind": "approve" }),
    );
    let accept = |id: &str, target: &str| {
        serde_json::json!({
            "id": id,
            "on": { "approval": "revise", "target": target },
            "outcome": { "kind": "accept" }
        })
    };
    let reject = |id: &str, on: serde_json::Value| {
        serde_json::json!({
            "id": id,
            "on": on,
            "outcome": { "kind": "reject", "rejection": "rj-decline" }
        })
    };
    step.insert(
        "rules".to_owned(),
        serde_json::json!([
            accept("r-alpha", "t-alpha"),
            accept("r-beta", "t-beta"),
            reject("r-bare-revise", serde_json::json!({ "approval": "revise" })),
            reject("r-early", serde_json::json!({ "approval": "approve" })),
        ]),
    );
    session_of(&serde_json::to_vec(&value).ok()?)
}

/// 試験の開始直後の状態（契約の試料の `initial`）。
fn start_state() -> Option<SessionState> {
    let value: serde_json::Value = serde_json::from_slice(SAMPLE).ok()?;
    serde_json::from_value(value.get("initial")?.clone()).ok()
}

/// ツリーの試料の 1 行。`(id, depth, kind, before, after)`。値は汎用のダミーの文字列で、教材固有の語を使わない。
#[cfg(target_arch = "wasm32")]
type TreeSpec = (
    &'static str,
    u8,
    crate::artifact_view::RowKind,
    Option<&'static str>,
    Option<&'static str>,
);

/// ファイルツリーの試料。追加・削除の行を含み、後の状態で 6 行が出る。
#[cfg(target_arch = "wasm32")]
const FILE_TREE_SPECS: [TreeSpec; 6] = {
    use crate::artifact_view::RowKind::{Dir, File};
    [
        ("d-a", 0, Dir, Some("dir-a"), Some("dir-a")),
        ("f-a1", 1, File, Some("file-a1"), Some("file-a1")),
        ("d-a2", 1, Dir, Some("dir-a2"), Some("dir-a2")),
        ("f-a2x", 2, File, None, Some("file-a2x")),
        ("f-b", 0, File, Some("file-b"), Some("file-b")),
        ("f-old", 0, File, Some("file-old"), None),
    ]
};

/// タスクツリーの試料。後の状態で 5 行が出る。
#[cfg(target_arch = "wasm32")]
const TASK_TREE_SPECS: [TreeSpec; 5] = {
    use crate::artifact_view::RowKind::Task;
    [
        ("t-1", 0, Task, Some("task-1"), Some("task-1")),
        ("t-11", 1, Task, Some("task-11"), Some("task-11")),
        ("t-12", 1, Task, Some("task-12"), Some("task-12")),
        ("t-2", 0, Task, Some("task-2"), Some("task-2")),
        ("t-21", 1, Task, Some("task-21"), Some("task-21")),
    ]
};

/// ツリーの試料を、後の状態・指定の変種で描いた HTML 文字列にする（`debug_render_tree` の実体）。
///
/// `kind` はツリーの種類（`file_tree`・`task_tree`）、`variant` は `a`・`b`。どれかが範囲外、または描けなければ `None`。
/// 試料は埋め込みで、外からのデータは受け取らない。行の項目は `tree_attrs` の属性を `view()` の側で直接書く（F-2）。
#[cfg(target_arch = "wasm32")]
pub(crate) fn render_tree_sample(kind: &str, variant: &str) -> Option<String> {
    use crate::artifact_view::{
        tree_view_list, Cell, Phase, Row, RowId, Variant, ViewKind, ROW_ATTR,
    };
    use crate::tree_attrs::shown_tree_attrs;
    use fandhe_frontend_core::{el, render, text};

    let kind = ViewKind::parse(kind).filter(|k| k.is_tree())?;
    let variant = match variant {
        "a" => Variant::A,
        "b" => Variant::B,
        _ => return None,
    };
    let specs: &[TreeSpec] = match kind {
        ViewKind::FileTree => &FILE_TREE_SPECS,
        _ => &TASK_TREE_SPECS,
    };
    let cell = |s: &str| Cell {
        label: s.to_owned(),
        ..Cell::default()
    };
    let rows: Vec<Row<Cell>> = specs
        .iter()
        .map(|&(id, depth, k, before, after)| {
            Row::new(
                RowId::new(id).ok()?,
                depth,
                k,
                before.map(cell),
                after.map(cell),
            )
            .ok()
        })
        .collect::<Option<_>>()?;
    let attrs = shown_tree_attrs(&rows, Phase::After);
    let items = rows
        .iter()
        .zip(attrs)
        .filter_map(|(row, tree)| {
            let shown = row.shown(Phase::After)?;
            let mut a: Vec<(&str, &str)> = vec![(ROW_ATTR, row.id().as_str())];
            // 変種 (a) は対照で、木の属性を付けない。
            let owned = tree.filter(|_| variant == Variant::B);
            if let Some(t) = &owned {
                a.extend(t.pairs());
            }
            Some((
                format!("k{}", row.id().as_str()),
                el("li", a, vec![text(&shown.content.label)]),
            ))
        })
        .collect();
    let node = tree_view_list("ul", kind, variant, true, items).ok()?;
    Some(render(&node))
}

/// 契約の試料の「2 周分」の履歴。書き出しの試験（TASK-330 1/2）が使う。
#[cfg(target_arch = "wasm32")]
const TWO_ROUNDS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/contract/history/two-rounds.json"
));

/// 2 周分の履歴の包みを返す。`Session` を作らず、読み込みの入口（段 1〜5）だけで読む。読めなければ `None`。
#[cfg(target_arch = "wasm32")]
pub(crate) fn two_rounds_envelope(
) -> Option<fandhe_workshop_core::history_envelope::SessionEnvelope> {
    use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
    parse_history_envelope(parse_history_json(TWO_ROUNDS).ok()?).ok()
}

/// 開ける履歴の包みを返す（TASK-330 2/2）。
///
/// 契約の試料（2 周分）の `ops` を、試験の教材と開始直後の状態に適用し直し、得た状態を写しにして包み直す。
/// 試料そのままの写しは再読み込みの意味の実装前の値で、読み込みの段 8 で `replay_mismatch` になるため使えない。
/// 受講中に記録した `ops` ではなく、試料を適用し直したものである（ホストは受講中の `ops` を記録するが、この試料は受講中の記録ではない）。
/// 2 周分・停止・保存・再読み込みを含むため、復習・チェックポイント・世界状態は空でない。
#[cfg(target_arch = "wasm32")]
pub(crate) fn loadable_envelope() -> Option<fandhe_workshop_core::history_envelope::SessionEnvelope>
{
    use fandhe_workshop_core::history_envelope::build_envelope;
    use fandhe_workshop_core::history_replay::replay;

    let ir = load_ir(IR).ok()?;
    let env = two_rounds_envelope()?;
    let state = replay(&ir, start_state()?, &env.ops).ok()?;
    build_envelope(&ir, &env.start, env.ops, state).ok()
}
