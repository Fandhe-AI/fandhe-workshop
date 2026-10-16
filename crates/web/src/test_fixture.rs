//! ブラウザ試験が mount するホストの開始状態（試験の設定 `test-hooks` だけ。TASK-347 2/2）。
//!
//! 呼び出し元は `host::EngineHost::for_mount` だけである。`lib.rs` が feature `test-hooks` の背後でだけ
//! 組み込むため、配信する設定の wasm には入らない（`tools/check-delivery.sh` が教材の文字列の不在を確かめる）。
//!
//! 開始（`Control::Begin`）はまだ core が実装していないため、契約の試料（`event-sequence.json`）が持つ
//! 開始直後の状態から始める。状態からの `Session` の再構築は、表示層で他に許されない操作（`tests/no_progress.rs`）で、
//! このファイルの 1 箇所だけを例外にしている。教材と状態は埋め込みの試験データで、外から入らない。

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
    let ir = load_ir(IR).ok()?;
    let value: serde_json::Value = serde_json::from_slice(SAMPLE).ok()?;
    let state: SessionState = serde_json::from_value(value.get("initial")?.clone()).ok()?;
    Some(Session::from_state(ir, state))
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
