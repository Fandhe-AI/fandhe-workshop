//! ブラウザ試験用の口。feature `test-hooks` のときだけ組み込まれる。
//!
//! 配信する設定には入れない。`static/test-hooks.js` がここの関数を JS へ橋渡しし、
//! TASK-28 以降のブラウザ試験が状態を読む。`debug_rerender`・`debug_request_complete`・`debug_storage_open`・
//! `debug_storage_write`・`debug_storage_state` を除き、状態は変更しない。
//! `debug_request_complete` は、配信と同じ配送の口（`set_timeout` → `dispatch_action`）で補完を届けるだけである。
//! `debug_storage_open` と `debug_storage_write` は、試験用の保存先（`STORAGE`）を開く・書くだけで、画面の状態には触れない。
//! `debug_view_fixture_mount` と `debug_view_fixture_dispatch` は、`#app` の外の別の root に代役の部品（`view_fixture`）を
//! マウントして action（`fixture_select`・`fixture_phase`・項目の外の部品を出し入れする `fixture_control`・差分・文書の (a) だけを受ける対照の口 `fixture_contrast`）を届けるだけで、本体の画面の状態には触れない。`debug_view_fixture_state` は読み取りのみ。
//! `debug_storage_state` は、`kind_for` を通して試験用の保存先の切り替えの状態（このセッションの保存先）だけを変えうる。
//! `debug_save_stage`・`debug_save_install_pagehide`・`debug_save_flush`（TASK-317 2/2）は、試験用の保留（`PENDING`）へ積む・
//! `pagehide` のリスナーを付ける・保留を試験用の保存先へ書き出すだけである。保留から保存先へ値が届く経路はこの 2 つ（`pagehide`・
//! `debug_save_flush`）だけで、この試験用の保留に遅延保存のタイマーは触れない（値が残ったのが `pagehide` の書き込みによることを試験で示すため。本番の保存先への遅延保存は `persist::save_deferred`、TASK-318）。
//! `debug_autosave_state`・`debug_history_save_len`（TASK-318）は読み取りのみである。
//! `debug_large_history_text`・`debug_secret_gate_len`（TASK-166・SAFE-31）は読み取りのみである。前者は 10MB に近い合成の履歴の文字列を返し、
//! 後者は保存の直前の関門（直列化・秘密情報の検査）を本番と同じ関数で 1 回走らせて、大きさと検出した形の数だけを返す。
//! `debug_last_import_state_json`（TASK-175 2/2）は開いた直後の状態を読むだけである。
//! `debug_export_prepare_loadable`・`debug_loadable_history_text`（TASK-330 2/2）は、開ける履歴の包み（試料の `ops` を適用し直したもの）を書き出す・文字列で返すだけで、
//! 状態は変えない。取り込みはファイルの選択（`static/test-hooks.js`）から `raw-trigger` 経由でホストの `open_history` に届く。

use wasm_bindgen::prelude::*;

/// 起動の前に、URL のクエリ（`location.search`）から mount の教材を選ぶ（TASK-319 3/3）。
///
/// `persist::mount_with_history` が呼ぶ。クエリの読み取りを配信する経路のファイルに置かないため、ここに置く
/// （`tests/tick_interval.rs` が配信する経路に URL の読み取りが無いことを固定する）。選択の規則は
/// `test_fixture::mount_fixture_from_search` の完全一致だけで、読めないときは既定の教材に倒す。状態は変えない。
pub(crate) fn mount_fixture_from_url() -> crate::test_fixture::MountFixture {
    let search = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    crate::test_fixture::mount_fixture_from_search(&search)
}

/// 直近に開いた履歴の、開いた直後（進行の前）の状態の JSON を返す（読み取りのみ。TASK-175 2/2）。
///
/// 開いていなければ `"null"`。写しの一致の確認用で、進行のタイマーが開いた後に状態を進めても揺れない。
/// 失敗の文に出さないこと（履歴の中身のため）。
#[wasm_bindgen]
pub fn debug_last_import_state_json() -> String {
    crate::glue::last_import_state().unwrap_or_else(|| "null".to_string())
}

/// 現在の画面の状態を JSON 文字列で返す（読み取りのみ）。
#[wasm_bindgen]
pub fn debug_state_json() -> String {
    let probe = crate::glue::keydown_probe_count()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "null".to_string());
    let esc = crate::glue::esc_count()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "null".to_string());
    // 本文は JSON に入れない（エスケープを避ける）。照合は `debug_draft` で行う。
    let (cps, over) = crate::glue::draft_snapshot()
        .map(|(n, o, _)| (n.to_string(), o.to_string()))
        .unwrap_or_else(|| ("null".to_string(), "null".to_string()));
    // 3 つの表示の照合用の状態（TASK-347 2/2）。未マウントなら null。中身の組み立ては `EngineHost` が持つ。
    let engine = crate::glue::engine_snapshot().unwrap_or_else(|| "null".to_string());
    let complete = crate::glue::complete_count()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"screen\":\"hello\",\"mounted\":{},\"keydownProbe\":{},\"escCount\":{},\"completeCount\":{},\"draftCodePoints\":{},\"inputOverLimit\":{},\"engine\":{}}}",
        crate::glue::is_mounted(),
        probe,
        esc,
        complete,
        cps,
        over,
        engine
    )
}

/// 状態の下書き本文を返す（読み取りのみ）。未マウントなら空文字列。
#[wasm_bindgen]
pub fn debug_draft() -> String {
    crate::glue::draft_snapshot()
        .map(|(_, _, d)| d)
        .unwrap_or_default()
}

/// 画面を丸ごと再構築する（再描画の時間の測定用）。実行したら `true`、未マウントなどで何もしなかったら `false`。
///
/// 配信する設定には入らない（`debug_` 接頭辞は `scripts/test-build-web.sh` が不在を検査する）。
#[wasm_bindgen]
pub fn debug_rerender() -> bool {
    crate::glue::rerender()
}

/// 入れ子の深さの検査（core の `check_json_depth`）を呼び、固定形の JSON で結果を返す（読み取りのみ）。
///
/// 10,000 段でもスタックを使い切らず理由が返ることを、ブラウザ試験（TASK-157）が確かめるための口。
/// 入力の内容は返り値に含めない。
#[wasm_bindgen]
pub fn debug_check_json_depth(text: &str) -> String {
    match fandhe_workshop_core::json_depth::check_json_depth(text.as_bytes()) {
        Ok(()) => "{\"ok\":true}".to_string(),
        Err(reason) => format!("{{\"ok\":false,\"reason\":\"{}\"}}", reason.as_str()),
    }
}

/// IR・履歴の読み込みの関門（core の `parse_json_value`）の 3 段目までを呼び、固定形の JSON で結果を返す（読み取りのみ）。
///
/// `entry` は `"ir"` か `"history"`。それ以外は `unknown_entry` を返す。型・内容の段は `serde_json::Value` では
/// 常に通るため、ここでは確かめない（native の `load_order` 試験が持つ）。入力の内容は返り値に含めない。
#[wasm_bindgen]
pub fn debug_check_json_load(entry: &str, text: &str) -> String {
    use fandhe_workshop_contract::limits::{LoadEntry, LoadStage};
    let entry = match entry {
        "ir" => LoadEntry::Ir,
        "history" => LoadEntry::History,
        _ => return "{\"ok\":false,\"error\":\"unknown_entry\"}".to_string(),
    };
    match fandhe_workshop_core::load_gate::parse_json_value(entry, text.as_bytes()) {
        Ok(_) => "{\"ok\":true}".to_string(),
        Err(err) => {
            let stage = match err.stage() {
                LoadStage::Size => "size",
                LoadStage::Depth => "depth",
                LoadStage::JsonParse => "json_parse",
                LoadStage::TypeAndContent => "type_and_content",
            };
            match err {
                fandhe_workshop_core::load_gate::LoadError::Limit(reason) => format!(
                    "{{\"ok\":false,\"stage\":\"{stage}\",\"reason\":\"{}\"}}",
                    reason.as_str()
                ),
                _ => format!("{{\"ok\":false,\"stage\":\"{stage}\"}}"),
            }
        }
    }
}

/// 本文の木の読み込み（core の `load_json::<Doc>`）を呼び、固定形の JSON で結果を返す（読み取りのみ）。
///
/// 手作りの木が関門のどの段で止まるかを、ブラウザ試験（TASK-149）が確かめるための口。
/// 入力の内容は返り値に含めない。
#[wasm_bindgen]
pub fn debug_check_doc(text: &str) -> String {
    use fandhe_workshop_contract::doc::Doc;
    use fandhe_workshop_contract::limits::{LoadEntry, LoadStage};
    match fandhe_workshop_core::load_gate::load_json::<Doc>(LoadEntry::Ir, text.as_bytes()) {
        Ok(_) => "{\"ok\":true}".to_string(),
        Err(err) => {
            let stage = match err.stage() {
                LoadStage::Size => "size",
                LoadStage::Depth => "depth",
                LoadStage::JsonParse => "json_parse",
                LoadStage::TypeAndContent => "type_and_content",
            };
            format!("{{\"ok\":false,\"stage\":\"{stage}\"}}")
        }
    }
}

/// 本文の木を関門に通し、通れば描画した HTML 文字列を返す（読み取りのみ）。拒否なら空文字列。
///
/// 本体のマウントと同じ「`render` の文字列 → innerHTML」の経路を、試験が `#app` の外の容器で再現するための口。
#[wasm_bindgen]
pub fn debug_render_doc(text: &str) -> String {
    use fandhe_workshop_contract::doc::Doc;
    use fandhe_workshop_contract::limits::LoadEntry;
    match fandhe_workshop_core::load_gate::load_json::<Doc>(LoadEntry::Ir, text.as_bytes()) {
        Ok(doc) => fandhe_frontend_core::render(&crate::doc_view::render_doc_node(&doc)),
        Err(_) => String::new(),
    }
}

/// `debug_render_quote_nest` が受け付ける段数の上限。`Doc` の生成と破棄は段数分だけ再帰するため、試験の設定でも
/// 無制限の段数を Rust 側で組み立てない。
const MAX_QUOTE_NEST_FOR_HOOK: u32 = 200;

/// 引用を `n` 段入れ子にした木を Rust で組み立てて描画し、HTML 文字列を返す（読み取りのみ）。
///
/// 関門（JSON の深さ 64）は 50・60 段の木を描画まで通さないため、描画の深さの上限（SAFE-12）を
/// ブラウザで確かめるには関門を通さずに木を作る口が要る。`n` が上限を超えたときは空文字列を返す。
#[wasm_bindgen]
pub fn debug_render_quote_nest(n: u32) -> String {
    use fandhe_workshop_contract::doc::{Block, Doc, TREE_VERSION};
    if n > MAX_QUOTE_NEST_FOR_HOOK {
        return String::new();
    }
    let mut blocks: Vec<Block> = Vec::new();
    for _ in 0..n {
        blocks = vec![Block::Quote { children: blocks }];
    }
    let doc = Doc {
        version: TREE_VERSION,
        blocks,
    };
    fandhe_frontend_core::render(&crate::doc_view::render_doc_node(&doc))
}

/// 進行の間隔（ミリ秒）の定数を返す（読み取りのみ）。
///
/// 配信する設定が `?tick=5` を無視して同じ値のままであることを、ブラウザ試験（TASK-229）が確かめるための口。
/// 値を書き換える口は作らない。
#[wasm_bindgen]
pub fn debug_tick_interval_ms() -> u32 {
    crate::tick_interval::TICK_INTERVAL_MS
}

thread_local! {
    /// 保存先の試験用の保持（`debug_storage_open` が準備完了の後に入れる）。
    static STORAGE: std::cell::RefCell<Option<crate::storage_idb::OpenedStorage>> =
        const { std::cell::RefCell::new(None) };
}

thread_local! {
    /// 離れる直前に書き出す未保存の値の保留（`debug_save_stage` が積む）。
    static PENDING: crate::save_pending::PendingSave = crate::save_pending::PendingSave::new();
}

/// 試験用の保留を、準備完了済みの試験用の保存先へ書き出す。保存先が未準備・借用中（再入）なら何もせず `None`。
fn flush_pending() -> Option<crate::save_pending::FlushOutcome> {
    PENDING.with(|p| {
        STORAGE.with(|s| {
            let guard = s.try_borrow().ok()?;
            let st = guard.as_ref()?;
            Some(p.flush(st))
        })
    })
}

fn storage_key(pack_id: &str, start_id: &str) -> fandhe_workshop_contract::storage::HistoryKey {
    use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey};
    let path = web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_default();
    HistoryKey::for_pack(DeliveryScope::from_pathname(&path), pack_id, start_id)
}

fn error_name(e: fandhe_workshop_contract::storage::StorageError) -> &'static str {
    use fandhe_workshop_contract::storage::StorageError;
    match e {
        StorageError::QuotaExceeded => "quota_exceeded",
        StorageError::Unavailable => "unavailable",
        StorageError::Failed => "failed",
    }
}

/// 保存先を非同期に開く（TASK-317）。準備完了は `debug_storage_state` の `opened` で見る。開き直すと前の保持を捨てる。
/// 鍵は必ず `HistoryKey` の構築子から作り、任意の文字列を鍵にしない。
#[wasm_bindgen]
pub fn debug_storage_open() {
    STORAGE.with(|s| *s.borrow_mut() = None);
    crate::storage_idb::open_history_storage(|opened| {
        STORAGE.with(|s| *s.borrow_mut() = Some(opened));
    });
}

/// 教材 ID と最初の開始点の ID から作った鍵の、保存領域に書く文字列を返す（読み取りのみ）。
#[wasm_bindgen]
pub fn debug_storage_key(pack_id: &str, start_id: &str) -> String {
    storage_key(pack_id, start_id).render()
}

/// 保存先へ書く。固定形の JSON（`ok` と、失敗なら `error`）を返す。値は返り値に含めない。
#[wasm_bindgen]
pub fn debug_storage_write(pack_id: &str, start_id: &str, value: &str) -> String {
    use fandhe_workshop_contract::storage::HistoryStore;
    let key = storage_key(pack_id, start_id);
    STORAGE.with(|s| match s.borrow().as_ref() {
        None => "{\"ok\":false,\"error\":\"not_opened\"}".to_string(),
        Some(st) => match st.write(&key, value) {
            Ok(()) => "{\"ok\":true}".to_string(),
            Err(e) => format!("{{\"ok\":false,\"error\":\"{}\"}}", error_name(e)),
        },
    })
}

/// 保存先から読む。無い・開いていないときは `undefined`。
#[wasm_bindgen]
pub fn debug_storage_read(pack_id: &str, start_id: &str) -> Option<String> {
    use fandhe_workshop_contract::storage::HistoryStore;
    let key = storage_key(pack_id, start_id);
    STORAGE.with(|s| s.borrow().as_ref()?.read(&key).ok().flatten())
}

/// 保存先の状態を固定形の JSON で返す（値・鍵は含めない）。
/// `kind` は今のセッションの保存先（`local` か `idb`）、`lastAsyncError` は種類の名前か `null`。
/// `kindFor` は鍵ごとの起動時の選択で、IndexedDB の側にあればこのセッションの保存先をそちらに固定する。
#[wasm_bindgen]
pub fn debug_storage_state(pack_id: &str, start_id: &str) -> String {
    use fandhe_workshop_contract::storage::{HistoryStore, StorageKind};
    let key = storage_key(pack_id, start_id);
    let name = |k: StorageKind| match k {
        StorageKind::LocalStorage => "local",
        StorageKind::IndexedDb => "idb",
    };
    STORAGE.with(|s| match s.borrow().as_ref() {
        None => "{\"opened\":false}".to_string(),
        Some(st) => {
            let err = st
                .last_async_error()
                .map(|e| format!("\"{}\"", error_name(e)))
                .unwrap_or_else(|| "null".to_string());
            let kind_for = name(st.kind_for(&key));
            format!(
                "{{\"opened\":true,\"idbAvailable\":{},\"kind\":\"{}\",\"kindFor\":\"{}\",\"lastAsyncError\":{}}}",
                st.idb_available(),
                name(st.kind()),
                kind_for,
                err
            )
        }
    })
}

/// 変更の後の遅延保存の状態を固定形の JSON で返す（読み取りのみ。TASK-318・SAVE-7）。
///
/// `debounce_ms`・`max_wait_ms` は実装の値（期待にしない）、`scheduled` は待っている変更の有無、`saves` は遅延保存の書き出しを試みた回数、`secretHolds` は秘密情報らしい文字列を見つけて保存を止めた回数、
/// `kind` は本番の保存先の種類（開いていなければ `null`）。値・鍵は含めない。
#[wasm_bindgen]
pub fn debug_autosave_state() -> String {
    let kind = crate::persist::storage_kind_name()
        .map(|k| format!("\"{k}\""))
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"debounce_ms\":{},\"max_wait_ms\":{},\"scheduled\":{},\"saves\":{},\"secretHolds\":{},\"kind\":{}}}",
        crate::save_schedule::SAVE_DEBOUNCE_MS,
        crate::save_schedule::SAVE_MAX_WAIT_MS,
        crate::glue::save_scheduled(),
        crate::persist::autosave_count(),
        crate::persist::secret_hold_count(),
        kind
    )
}

/// いま保存する値の長さ（バイト）。保存しない（未操作・止められている）ときは 0。中身は返さない（読み取りのみ。TASK-318）。
#[wasm_bindgen]
pub fn debug_history_save_len() -> usize {
    crate::glue::history_to_save().map(|v| v.len()).unwrap_or(0)
}

/// 10MB に近い合成の履歴の JSON 文字列を返す（読み取りのみ。TASK-166・SAFE-31）。
///
/// `kind` は `plain`・`dense`・`marker` の完全一致だけ。未知の名前・作れない場合は空文字列を返し、黙って別の種類に退避しない。
/// 受講者のデータではなく、試験の出力・失敗の文には出さない。ページの外へ出さず、同じ evaluate の中で `importText` に渡す。
#[wasm_bindgen]
pub fn debug_large_history_text(kind: &str) -> String {
    crate::test_fixture::LargeKind::from_name(kind)
        .and_then(crate::test_fixture::large_loadable_envelope)
        .and_then(|env| fandhe_workshop_core::history_export::export_bytes(&env).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default()
}

/// 保存の直前の関門を本番と同じ関数で 1 回走らせ、固定形の JSON `{"bytes":n,"shapes":k}` を返す（読み取りのみ。TASK-166・SAFE-31）。
///
/// 直列化（`history_to_save`）と、値の読み直しと走査（`secret_gate`）を通す。保存先へは書かず、警告・dirty も変えない。
/// 保存しない（未操作・止められている・上限超え）ときは `{"bytes":0,"shapes":0}`。値・形の名前は返さない。時刻は JS 側で取る。
#[wasm_bindgen]
pub fn debug_secret_gate_len() -> String {
    let (bytes, shapes) = match crate::glue::history_to_save() {
        Some(v) => (v.len(), crate::glue::secret_gate(&v).len()),
        None => (0, 0),
    };
    format!("{{\"bytes\":{bytes},\"shapes\":{shapes}}}")
}

/// ツリーの試料（ファイルツリー・タスクツリー）を、後の状態で描いた HTML 文字列を返す（読み取りのみ）。
///
/// `kind` は `file_tree`・`task_tree`、`variant` は `a`・`b`。範囲外・描けない場合は空文字列を返し、黙って別の形に
/// 退避しない。ブラウザ試験（TASK-472）が、ツリーの属性と全項目が見えることを `#app` の外の容器で確かめるための口で、
/// 試料は埋め込みの固定データだけである。
#[wasm_bindgen]
pub fn debug_render_tree(kind: &str, variant: &str) -> String {
    crate::test_fixture::render_tree_sample(kind, variant).unwrap_or_default()
}

/// 予約済みの次の進行の宛先を JSON 文字列で返す（読み取りのみ）。無ければ `null`。
///
/// 形は `{"generation":..,"step":"..","index":..}`。Step の実体 ID の文字列は `serde_json` でエスケープする。
#[wasm_bindgen]
pub fn debug_next_tick() -> String {
    match crate::glue::scheduled_tick() {
        Some(a) => serde_json::json!({
            "generation": a.generation.0,
            "step": a.step.0,
            "index": a.index,
        })
        .to_string(),
        None => "null".to_string(),
    }
}

/// 補完の action を、配信と同じ配送の口で届ける（タイマーの予約を通す）。
///
/// 画面の Tab は一覧が開いていて文字があるときだけ補完に届く。この口は、その条件によらず配送の口そのものを確かめるためのものである。
#[wasm_bindgen]
pub fn debug_request_complete() {
    crate::glue::request_complete();
}

thread_local! {
    /// 代役の部品の Runtime。本体の `RUNTIME`（`glue`）とは別に持ち、状態を共有しない。
    static VIEW_FIXTURE: std::cell::RefCell<Option<fandhe_frontend_wasm_full::Runtime<crate::view_fixture::ViewFixture>>> =
        const { std::cell::RefCell::new(None) };
}

/// 左のビューの共通の検査器（TASK-471 2/3）が使う代役の部品を、`root_id` の要素にマウントする。
///
/// 本体の `#app` ではなく、試験が作った別の root を渡す。返り値は固定形の JSON で、成功は `{"ok":true}`、
/// 失敗は `{"ok":false,"error":"already_mounted"|"busy"|"data"|"mount"}`。2 回目のマウントは拒否する。
#[wasm_bindgen]
pub fn debug_view_fixture_mount(root_id: &str) -> String {
    use crate::view_fixture::ViewFixture;
    VIEW_FIXTURE.with(|cell| {
        let Ok(mut slot) = cell.try_borrow_mut() else {
            return "{\"ok\":false,\"error\":\"busy\"}".to_string();
        };
        if slot.is_some() {
            return "{\"ok\":false,\"error\":\"already_mounted\"}".to_string();
        }
        let Ok(fixture) = ViewFixture::load() else {
            return "{\"ok\":false,\"error\":\"data\"}".to_string();
        };
        match fandhe_frontend_wasm_full::Runtime::mount(root_id, fixture) {
            Ok(rt) => {
                // 左の行の Enter・Space の結線（TASK-358 1/2）。失敗したら Runtime を保持しない。
                if crate::glue::wire_row_keys(rt.root()).is_err() {
                    return "{\"ok\":false,\"error\":\"mount\"}".to_string();
                }
                *slot = Some(rt);
                "{\"ok\":true}".to_string()
            }
            Err(_) => "{\"ok\":false,\"error\":\"mount\"}".to_string(),
        }
    })
}

/// 代役の部品へ action を届ける（`Runtime::dispatch_action`）。結果は固定形の JSON で返す。
///
/// `outcome` は `applied`（受理）・`rejected`（未知の名前・不正な payload・許されない変種。状態も画面も変わらない）・
/// `dropped`（再入）・`not_mounted`。
#[wasm_bindgen]
pub fn debug_view_fixture_dispatch(name: &str, payload: &str) -> String {
    use crate::action::{dispatch_effect, DispatchEffect};
    VIEW_FIXTURE.with(|cell| {
        let outcome = match cell.try_borrow().ok().as_deref().and_then(Option::as_ref) {
            None => "not_mounted",
            Some(rt) => match dispatch_effect(rt.dispatch_action(name, payload)) {
                DispatchEffect::Applied => "applied",
                DispatchEffect::Rejected => "rejected",
                DispatchEffect::Dropped => "dropped",
            },
        };
        format!("{{\"outcome\":\"{outcome}\"}}")
    })
}

/// 代役の部品の状態（選ばれたビュー・変種・前後）を固定形の JSON で返す（読み取りのみ）。未マウントは `null`。
#[wasm_bindgen]
pub fn debug_view_fixture_state() -> String {
    use crate::artifact_view::Phase;
    VIEW_FIXTURE.with(|cell| {
        let Ok(slot) = cell.try_borrow() else {
            return "null".to_string();
        };
        let Some(rt) = slot.as_ref() else {
            return "null".to_string();
        };
        // `component()` は借用を取る。update は同期で完了するため、JS から呼ばれる時点では借用は解けている。
        let f = rt.component();
        serde_json::json!({
            "selected": f.selected().as_str(),
            "variant": f.variant().as_str(),
            "phase": match f.phase() { Phase::Before => "before", Phase::After => "after" },
            "pointed": f.last_point().map(|t| serde_json::json!({"view": t.view(), "row": t.row().as_str()})),
            "point_count": f.point_count(),
        })
        .to_string()
    })
}

/// 値を保留へ積むだけで、保存先へは書かない（TASK-317 2/2）。固定形の JSON（保留の件数）を返し、値・鍵は返さない。
/// 鍵は必ず `HistoryKey` の構築子から作り、任意の文字列を鍵にしない。
#[wasm_bindgen]
pub fn debug_save_stage(pack_id: &str, start_id: &str, value: &str) -> String {
    let key = storage_key(pack_id, start_id);
    PENDING.with(|p| {
        p.stage(key, value.to_string());
        format!("{{\"ok\":true,\"pending\":{}}}", p.pending_len())
    })
}

/// `pagehide` で保留を書き出すリスナーを付ける（登録は 1 回だけ）。固定形の JSON（`ok`）を返す。
/// 保存先が未準備のときはリスナーの中で何も書かない（保留は残る）。
#[wasm_bindgen]
pub fn debug_save_install_pagehide() -> String {
    match crate::leave_flush::install_pagehide_flush(|| {
        flush_pending();
    }) {
        Ok(()) => "{\"ok\":true}".to_string(),
        Err(_) => "{\"ok\":false}".to_string(),
    }
}

/// このタブが古いタブ（他のタブの起動番号に追い越された）かの真偽（TASK-324 1/2。SAVE-12）。読み取りのみで、鍵・値は返さない。
#[wasm_bindgen]
pub fn debug_save_stale() -> bool {
    crate::persist::stale_tab()
}

/// 保留を試験から明示的に書き出す。固定形の JSON（件数と失敗の種類）を返し、値・鍵は返さない。
#[wasm_bindgen]
pub fn debug_save_flush() -> String {
    match flush_pending() {
        None => "{\"ok\":false,\"error\":\"not_opened\"}".to_string(),
        Some(o) => {
            let err = o
                .first_error
                .map(|e| format!("\"{}\"", error_name(e)))
                .unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"ok\":true,\"written\":{},\"failed\":{},\"error\":{}}}",
                o.written, o.failed, err
            )
        }
    }
}

thread_local! {
    /// 書き出しの試験用の `blob:` URL の保持（`debug_export_prepare` が入れ、`debug_export_revoke` が破棄する）。
    static EXPORT_URL: std::cell::RefCell<Option<crate::history_download::ExportUrl>> =
        const { std::cell::RefCell::new(None) };
}

/// 契約の試料（2 周分）の包みを書き出し、`blob:` URL を作って保持する（TASK-330 1/2）。固定形の JSON を返す。
///
/// 返すのは `url`・`name`・`bytes` だけで、履歴の中身は返さない。前の URL が残っていれば先に破棄する。
/// 失敗は `{"ok":false}`（理由の文は読まない）。ダウンロードの click は `static/test-hooks.js` が行う。
#[wasm_bindgen]
pub fn debug_export_prepare() -> String {
    export_prepare(crate::test_fixture::two_rounds_envelope())
}

/// 開ける包み（`loadable_envelope`）を書き出し、`blob:` URL を作って保持する（TASK-330 2/2）。戻りは `debug_export_prepare` と同じ形。
///
/// 別のオリジンで読み込む試験が、ダウンロードしたファイルを `setInputFiles` で選ぶために使う。
#[wasm_bindgen]
pub fn debug_export_prepare_loadable() -> String {
    export_prepare(crate::test_fixture::loadable_envelope())
}

/// 開ける包みの JSON 文字列を返す（読み取りのみ）。ファイルを経ずに同じ取り込みの経路へ入れ、基準の状態を作るための口。
///
/// 試料を適用し直したもので、受講者のデータではない。試験の出力・失敗の文には出さない。作れなければ空文字列。
#[wasm_bindgen]
pub fn debug_loadable_history_text() -> String {
    crate::test_fixture::loadable_envelope()
        .and_then(|env| fandhe_workshop_core::history_export::export_bytes(&env).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default()
}

/// 包みを書き出して `blob:` URL を作り、保持して、固定形の JSON を返す共通の実体。
fn export_prepare(env: Option<fandhe_workshop_core::history_envelope::SessionEnvelope>) -> String {
    let Some(env) = env else {
        return "{\"ok\":false}".to_string();
    };
    let Ok(bytes) = fandhe_workshop_core::history_export::export_bytes(&env) else {
        return "{\"ok\":false}".to_string();
    };
    EXPORT_URL.with(|u| u.borrow_mut().take());
    let Ok(url) = crate::history_download::object_url(&bytes) else {
        return "{\"ok\":false}".to_string();
    };
    let json = serde_json::json!({
        "ok": true,
        "url": url.as_str(),
        "name": crate::history_export::file_name(&env.pack_id),
        "bytes": bytes.len(),
    })
    .to_string();
    EXPORT_URL.with(|u| *u.borrow_mut() = Some(url));
    json
}

/// 保持している書き出しの URL を破棄する。破棄したら `true`、保持が無ければ `false`。
#[wasm_bindgen]
pub fn debug_export_revoke() -> bool {
    EXPORT_URL.with(|u| u.borrow_mut().take()).is_some()
}
