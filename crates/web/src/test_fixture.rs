//! ブラウザ試験が mount するホストの開始状態（試験の設定 `test-hooks` だけ。TASK-347 2/2）。
//!
//! 呼び出し元は `host::EngineHost::for_mount` だけである。`lib.rs` が feature `test-hooks` の背後でだけ
//! 組み込むため、配信する設定の wasm には入らない（`tools/check-delivery.sh` が教材の文字列の不在を確かめる）。
//!
//! 開始（`Control::Begin`）はまだ core が実装していないため、契約の試料（`event-sequence.json`）が持つ
//! 開始直後の状態から始める。状態からの `Session` の再構築は、表示層で他に許されない操作（`tests/no_progress.rs`）で、
//! このファイルの 1 箇所だけを例外にしている。教材と状態は埋め込みの試験データで、外から入らない。
//! 確認が `answer` の教材への差し替え（`answer_session`）は、`load-fixture:answer` の合図で使う（TASK-445 2/2）。
//! 複数の種類のビューを宣言する教材（`views_session`）は `load-fixture:views` で使う（TASK-470 2/2）。
//! 入力方式が `ui_action` の Step を持つ教材（`ui_action_session`）は `load-fixture:ui-action` で使う（TASK-450 1/3）。
//! 履歴ファイルの読み込みの試験（TASK-330 2/2）が使う開ける包み（`loadable_envelope`）もここで作る。
//! 再読み込みの一致のブラウザ試験（TASK-319 3/3）は、起動の前に 8 Step の教材（`reload_points_session`）を選ぶ。
//! 入力欄なしの教材（`?fixture=no-input`。`pack.profile.input_field: hidden`）も起動の前に選ぶ（TASK-450 2/3）。
//! 章をまたいで 20 Step を 1 つのページで通す教材（`?fixture=ch0-3`。先頭は `starts[0]` の節点）も起動の前に選ぶ（TASK-450 3/3）。
//! 遷移の上限（SAFE-23）の試験は、巡回する IR 4 本（`cycle_session`）と巡回しない長い連鎖（`chain_session`）を `load-fixture:cycle-*`・`load-fixture:chain-*` で使う（TASK-160 1/3）。
//! 選択は URL のクエリの完全一致だけで決める（`mount_fixture_from_search`）。教材の差し替え（`load-fixture:`）は
//! 起動の後に働き、保存先の鍵（`pack.id`）が mount の教材のままになるため、復元の試験には使えない。

use fandhe_workshop_core::addr::StepUid;
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

/// 再読み込みの一致の試験用の 8 Step の教材（`crates/web/tests/data/reload_points_ch2/ir.json`。`pack.id` は mount の既定と別）。
const RELOAD_POINTS_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/reload_points_ch2/ir.json"
));

/// 入力欄なしの 3 Step の教材（`crates/web/tests/data/input_field_hidden/ir.json`。`pack.id` は mount の既定と別）。
const NO_INPUT_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/input_field_hidden/ir.json"
));

/// 章をまたいで 20 Step を通す教材（`crates/web/tests/data/ch0_3/ir.json`。`pack.id` は mount の既定と別。TASK-450 3/3）。
const CH03_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/ch0_3/ir.json"
));

/// mount の教材の選択。既定は `min-3step`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MountFixture {
    /// 3 Step の `min-3step`（既存のブラウザ試験の前提）。
    #[default]
    Default,
    /// 8 Step の再読み込みの地点の試験用の教材。
    ReloadPoints,
    /// 入力欄なしの 3 Step の教材（`pack.profile.input_field: hidden`。TASK-450 2/3）。
    NoInput,
    /// 章をまたいで 20 Step を通す教材（TASK-450 3/3）。
    Ch03,
}

/// URL のクエリ（`location.search`）から mount の教材を選ぶ。`?fixture=reload-points` の完全一致だけが
/// 8 Step の教材、`?fixture=no-input` の完全一致だけが入力欄なしの教材、`?fixture=ch0-3` の完全一致だけが 20 Step の通しの教材を選び、
/// 空・未知の名前・余分な文字・他のキーとの併用は、すべて既定に倒す。
pub(crate) fn mount_fixture_from_search(search: &str) -> MountFixture {
    match search {
        "?fixture=reload-points" => MountFixture::ReloadPoints,
        "?fixture=no-input" => MountFixture::NoInput,
        "?fixture=ch0-3" => MountFixture::Ch03,
        _ => MountFixture::Default,
    }
}

/// 選んだ教材と開始直後の状態から実行環境を作る。読めなければ `None`。
pub(crate) fn mount_session(fixture: MountFixture) -> Option<Session> {
    match fixture {
        MountFixture::Default => start_session(),
        MountFixture::ReloadPoints => session_of(RELOAD_POINTS_IR),
        MountFixture::NoInput => session_of(NO_INPUT_IR),
        MountFixture::Ch03 => session_of(CH03_IR),
    }
}

/// 教材のバイト列と開始直後の状態から実行環境を作る。`Session::from_state` の唯一の呼び出し（`tests/no_progress.rs`）。
///
/// 開始の節点は教材の `starts[0].node` で、契約の試料の開始状態の節点（`main/st-1`）をそれに合わせる。
/// 既存の教材は先頭が `main/st-1` で、結果は変わらない。節点が教材に無い・`starts` が読めないときは `None`。
fn session_of(ir_bytes: &[u8]) -> Option<Session> {
    session_at(ir_bytes, None)
}

/// [`session_of`] の開始の節点を、`starts` の ID で選べるようにしたもの。`None` は `starts[0]`。
///
/// ID は教材の `starts` から引き、Rust のコードに教材の節点の名前を書かない。ID が無いときは `None`。
fn session_at(ir_bytes: &[u8], start_id: Option<&str>) -> Option<Session> {
    let ir = load_ir(ir_bytes).ok()?;
    let value: serde_json::Value = serde_json::from_slice(ir_bytes).ok()?;
    let starts = value.get("starts")?.as_array()?;
    let start = match start_id {
        None => starts.first()?,
        Some(id) => starts
            .iter()
            .find(|s| s.get("id").and_then(serde_json::Value::as_str) == Some(id))?,
    };
    let node = start.get("node")?.as_str()?;
    ir.nodes.iter().any(|n| n.uid == node).then_some(())?;
    Some(Session::from_state(ir, start_state_at(node)?))
}

/// 任意の教材（IR の JSON のバイト列）から実行環境を作る（TASK-175 2/2。`load-ir:` が差し替える）。
///
/// 版の違う履歴 15 件の、開く側の教材をブラウザで入れ替えるために使う。読めない・開始の節点が無いときは `None`。
#[cfg(feature = "test-hooks")]
pub(crate) fn session_from_ir_bytes(ir_bytes: &[u8]) -> Option<Session> {
    session_of(ir_bytes)
}

/// 巡回する IR（`use` の `enter` の自己参照・2 者の巡回・`exit_to` の 2 者の巡回・1 入力あたりの遷移が少数の巡回）。
/// 作成者向けの検査を通さずに配られた IR の想定で、遷移の上限（SAFE-23）の試験が使う。spec の写しで、編集しない。
#[cfg(feature = "test-hooks")]
const CYCLE_ENTER_SELF_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-enter-self.json"
));
#[cfg(feature = "test-hooks")]
const CYCLE_ENTER_2_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-enter-2.json"
));
#[cfg(feature = "test-hooks")]
const CYCLE_EXIT_2_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-exit-2.json"
));
#[cfg(feature = "test-hooks")]
const CYCLE_D07_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-d07-flat.json"
));

/// 巡回する IR の試験用の実行環境（遷移の上限の試験。`load-fixture:cycle-*`）。
///
/// 開始の節点は、`enter` の巡回の 2 本は `starts[0]`（通常の Step。試験が `go_to` で巡回の入口へ移す）、
/// `exit_to` の巡回は `starts` の ID `s-exit-2`、1 入力あたりの遷移が少数の対照は ID `s-d07` の節点にする。
/// 読めなければ `None`（呼び出し側は何もしない）。
#[cfg(feature = "test-hooks")]
pub(crate) fn cycle_session(ir: CycleIr) -> Option<Session> {
    match ir {
        CycleIr::EnterSelf => session_at(CYCLE_ENTER_SELF_IR, None),
        CycleIr::Enter2 => session_at(CYCLE_ENTER_2_IR, None),
        CycleIr::Exit2 => session_at(CYCLE_EXIT_2_IR, Some("s-exit-2")),
        CycleIr::D07 => session_at(CYCLE_D07_IR, Some("s-d07")),
    }
}

/// 巡回する IR の種類（試験の設定だけ）。
#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CycleIr {
    /// `enter` が自分を指す。
    EnterSelf,
    /// `enter` が 2 者で巡回する。
    Enter2,
    /// `exit_to` が 2 者で巡回する。
    Exit2,
    /// 巡回していても 1 入力あたりの遷移が少数の対照。
    D07,
}

/// 巡回しない `use` の長い連鎖の試験用の実行環境（遷移の上限の境界。`load-fixture:chain-1000`・`chain-1001`）。
///
/// 埋め込みの教材の `main/st-2`（選択の入力、`confirm: none`）の達成で、`use` の Step `len` 個を `enter` でたどって
/// 最後の Step `main/st-3` に着く。着地は `len + 1` 回で、`len` が 999 なら 1,000 回ちょうど（通る）、1,000 なら
/// 1,001 回（止まる）。`use` の Step は `main/st-3` の複製で、`enter` だけを足す。
#[cfg(feature = "test-hooks")]
pub(crate) fn chain_session(len: usize) -> Option<Session> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let nodes = value.get_mut("nodes")?.as_array_mut()?;
    let template = nodes
        .iter()
        .find(|n| n.get("uid").and_then(|u| u.as_str()) == Some("main/st-3"))?
        .clone();
    let first = if len == 0 {
        "main/st-3".to_owned()
    } else {
        "main/u-0".to_owned()
    };
    let entry = nodes
        .iter_mut()
        .find(|n| n.get("uid").and_then(|u| u.as_str()) == Some("main/st-2"))?;
    entry
        .get_mut("step")?
        .as_object_mut()?
        .insert("next".to_owned(), serde_json::json!([{ "to": first }]));
    let mut added = Vec::with_capacity(len);
    for i in 0..len {
        let mut n = template.clone();
        let target = if i + 1 == len {
            "main/st-3".to_owned()
        } else {
            format!("main/u-{}", i + 1)
        };
        let obj = n.as_object_mut()?;
        obj.insert("uid".to_owned(), serde_json::json!(format!("main/u-{i}")));
        obj.insert("enter".to_owned(), serde_json::json!(target));
        obj.get_mut("step")?
            .as_object_mut()?
            .insert("id".to_owned(), serde_json::json!(format!("u-{i}")));
        added.push(n);
    }
    value.get_mut("nodes")?.as_array_mut()?.extend(added);
    session_of(&serde_json::to_vec(&value).ok()?)
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

/// 複数の種類のビューを Step ごとに宣言する試験用の実行環境（TASK-470 2/2。`load-fixture:views`）。
///
/// 埋め込みの教材の `views` に `card_list`・`check_list` を足し、各 Step の宣言を次にする。
/// `main/st-1` は file_tree・card_list・check_list（3 種の切り替え）、`main/st-2` は card_list・file_tree
/// （st-1 から移ると既定が変わる）、`main/st-3` は card_list（st-2 から移っても種類が同じ）。
/// `v-files` は契約の試料の操作列が指すため残す。読めない・節点が無いときは `None`。
#[cfg(feature = "test-hooks")]
pub(crate) fn views_session() -> Option<Session> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let views = value.get_mut("views")?.as_object_mut()?;
    views.insert(
        "v-cards".to_owned(),
        serde_json::json!({"kind": "card_list"}),
    );
    views.insert(
        "v-checks".to_owned(),
        serde_json::json!({"kind": "check_list"}),
    );
    let decl = [
        (
            "main/st-1",
            serde_json::json!(["v-files", "v-cards", "v-checks"]),
        ),
        ("main/st-2", serde_json::json!(["v-cards", "v-files"])),
        ("main/st-3", serde_json::json!(["v-cards"])),
    ];
    for node in value.get_mut("nodes")?.as_array_mut()? {
        let uid = node.get("uid").and_then(|u| u.as_str()).map(str::to_owned);
        if let Some((_, d)) = decl.iter().find(|(u, _)| Some(*u) == uid.as_deref()) {
            node.get_mut("step")?
                .as_object_mut()?
                .insert("views".to_owned(), d.clone());
        }
    }
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

/// 入力方式が `ui_action` の Step を持つ試験用の実行環境（TASK-450 1/3。`load-fixture:ui-action`）。
///
/// 埋め込みの教材の `main/st-2` を、対象 `v-files` の `notes/draft` を指すと受付になる `ui_action` に置き換える。
/// 確認は `none` のままで、受付がそのまま達成になる。`notes/draft` は st-1 の受付が作る行で、st-2 は `v-files` を
/// 宣言しているため、st-2 に入った時点で左の行は指せる行になる。規則は細い形で書く。
#[cfg(feature = "test-hooks")]
pub(crate) fn ui_action_session() -> Option<Session> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let node = value
        .get_mut("nodes")?
        .as_array_mut()?
        .iter_mut()
        .find(|n| n.get("uid").and_then(|u| u.as_str()) == Some("main/st-2"))?;
    let step = node.get_mut("step")?.as_object_mut()?;
    step.insert(
        "input".to_owned(),
        serde_json::json!({
            "kind": "ui_action",
            "targets": [{ "view": "v-files", "path": "notes/draft" }]
        }),
    );
    step.insert(
        "rules".to_owned(),
        serde_json::json!([{
            "id": "r-point",
            "on": { "ui": { "view": "v-files", "path": "notes/draft" } },
            "outcome": { "kind": "accept" }
        }]),
    );
    session_of(&serde_json::to_vec(&value).ok()?)
}

/// 契約の試料の開始状態が現在の Step とする節点。
const SAMPLE_START: &str = "main/st-1";

/// 契約の試料の開始状態の節点を `node` に置き換えた状態。試料の `current`・`status` の鍵・入口の写しの同じ欄が対象で、
/// 他の欄は変えない。`node` は教材の `starts[0].node`（埋め込みで外から入らない）で、JSON の文字列として組み立て直す。
fn start_state_at(node: &str) -> Option<SessionState> {
    let value: serde_json::Value = serde_json::from_slice(SAMPLE).ok()?;
    let initial = value.get("initial")?;
    let from = serde_json::to_string(SAMPLE_START).ok()?;
    let to = serde_json::to_string(node).ok()?;
    let text = serde_json::to_string(initial).ok()?.replace(&from, &to);
    let mut state: SessionState = serde_json::from_str(&text).ok()?;
    // 切り身の鍵は Step の実体の根（`main` か、入れ子の実体の ID）。節点が `main` の外にあるときは鍵も合わせる
    // （合わせないと、開始の Step の状態を引けず、入力が受け付けられない）。
    let uid = StepUid(node.to_owned());
    let root = uid.root();
    let main = StepUid(SAMPLE_START.to_owned()).root();
    if root != main {
        if let Some(slice) = state.run.slices.remove(&main) {
            state.run.slices.insert(root.clone(), slice);
        }
        if let Some(entry) = state.entry.as_mut() {
            if let Some(slice) = entry.run.slices.remove(&main) {
                entry.run.slices.insert(root, slice);
            }
        }
    }
    Some(state)
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
    let state = replay(&ir, start_state_at(SAMPLE_START)?, &env.ops).ok()?;
    build_envelope(&ir, &env.start, env.ops, state).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 完全一致のクエリだけが_8_step_の教材を選ぶ() {
        assert_eq!(
            mount_fixture_from_search("?fixture=reload-points"),
            MountFixture::ReloadPoints
        );
        for other in [
            "",
            "?",
            "?fixture=",
            "?fixture=unknown",
            "?fixture=reload-points&x=1",
            "?x=1&fixture=reload-points",
            "?fixture=reload-points2",
            "?fixture=Reload-Points",
            "fixture=reload-points",
            "?tick=5",
        ] {
            assert_eq!(
                mount_fixture_from_search(other),
                MountFixture::Default,
                "{other}"
            );
        }
    }

    #[test]
    fn 完全一致のクエリだけが入力欄なしの教材を選ぶ() {
        assert_eq!(
            mount_fixture_from_search("?fixture=no-input"),
            MountFixture::NoInput
        );
        for other in [
            "?fixture=no-input&x=1",
            "?x=1&fixture=no-input",
            "?fixture=no-input2",
            "?fixture=No-Input",
            "fixture=no-input",
        ] {
            assert_eq!(
                mount_fixture_from_search(other),
                MountFixture::Default,
                "{other}"
            );
        }
    }

    #[test]
    fn 入力欄なしの教材は鍵が別で入力欄なしを宣言する() {
        let a = mount_session(MountFixture::Default).unwrap();
        let b = mount_session(MountFixture::NoInput).unwrap();
        assert_ne!(a.ir().pack.id, b.ir().pack.id);
        assert_eq!(b.ir().nodes.len(), 3);
        let profile =
            fandhe_workshop_contract::profile::read_profile(b.ir().pack.profile.as_ref()).unwrap();
        assert_eq!(
            profile.input_field,
            fandhe_workshop_contract::profile::InputFieldMode::Hidden
        );
    }

    #[test]
    fn 完全一致のクエリだけが_ch0_3_の教材を選ぶ() {
        assert_eq!(
            mount_fixture_from_search("?fixture=ch0-3"),
            MountFixture::Ch03
        );
        for other in [
            "?fixture=ch0-3&x=1",
            "?x=1&fixture=ch0-3",
            "?fixture=ch0-3x",
            "?fixture=Ch0-3",
            "?fixture=ch0_3",
            "fixture=ch0-3",
        ] {
            assert_eq!(
                mount_fixture_from_search(other),
                MountFixture::Default,
                "{other}"
            );
        }
    }

    #[test]
    fn 通しの教材は鍵が別で_20_step_を持ち先頭の_step_から始まる() {
        let a = mount_session(MountFixture::Default).unwrap();
        let b = mount_session(MountFixture::Ch03).unwrap();
        assert_ne!(a.ir().pack.id, b.ir().pack.id);
        assert_eq!(b.ir().nodes.len(), 20);
        let s = b.state();
        let uid = s.run.current.as_ref().unwrap();
        assert_eq!(uid.0, "main/st-0-1");
        assert_eq!(s.run.slices[&uid.root()].status.len(), 1);
        assert_eq!(s.entry.as_ref().unwrap().run.current.as_ref(), Some(uid));
    }

    #[test]
    fn 既定の教材の開始状態は試料の節点のままである() {
        let a = mount_session(MountFixture::Default).unwrap();
        let uid = a.state().run.current.as_ref().unwrap().clone();
        assert_eq!(uid.0, SAMPLE_START);
    }

    #[test]
    fn 選んだ教材は鍵が別で_8_step_を持つ() {
        let a = mount_session(MountFixture::Default).unwrap();
        let b = mount_session(MountFixture::ReloadPoints).unwrap();
        assert_ne!(a.ir().pack.id, b.ir().pack.id);
        assert_eq!(b.ir().nodes.len(), 8);
    }
}
