// 試験の設定でだけ配る橋渡し。wasm の debug_* を JS から呼べる名前で出す。
import { debug_state_json, debug_rerender, debug_draft, debug_check_json_depth, debug_check_json_load, debug_check_doc, debug_render_doc, debug_render_quote_nest, debug_tick_interval_ms, debug_render_tree, debug_next_tick, debug_request_complete, debug_storage_open, debug_storage_key, debug_storage_write, debug_storage_read, debug_storage_state, debug_view_fixture_mount, debug_view_fixture_dispatch, debug_view_fixture_state, debug_save_stage, debug_save_install_pagehide, debug_save_flush, debug_export_prepare, debug_export_prepare_loadable, debug_loadable_history_text, debug_export_revoke, debug_last_import_state_json } from "./wasm/fandhe_workshop_web.js";

export function stateJson() {
  return debug_state_json();
}

// 再描画の測定用。true なら再構築を実行した。false は未マウントなどで何もしていない。
export function rerender() {
  return debug_rerender();
}

// 状態側の下書き本文。欄の value との一致を確かめるために使う。
export function draft() {
  return debug_draft();
}

// 入れ子の深さの検査（TASK-157）。固定形の JSON 文字列（ok / reason）を返す。
export function checkJsonDepth(text) {
  return debug_check_json_depth(text);
}

// IR・履歴の読み込みの関門（TASK-157）。entry は "ir" か "history"。固定形の JSON 文字列（ok / stage / reason）を返す。
export function checkJsonLoad(entry, text) {
  return debug_check_json_load(entry, text);
}

// 本文の木の読み込みの関門（TASK-149）。固定形の JSON 文字列（ok / stage）を返す。
export function checkDoc(text) {
  return debug_check_doc(text);
}

// 本文の木を関門に通して描画した HTML 文字列を返す。拒否なら空文字列。
export function renderDoc(text) {
  return debug_render_doc(text);
}

// 引用を n 段入れ子にした木を Rust で組み立てて描画した HTML 文字列を返す（関門を通さない。n は 200 まで）。
export function renderQuoteNest(n) {
  return debug_render_quote_nest(n);
}

// 進行の間隔（ミリ秒。TASK-229）。URL のクエリで変わらないことの確認に使う。読み取りのみ。
export function tickIntervalMs() {
  return debug_tick_interval_ms();
}

// 履歴の保存先（TASK-317）。open は非同期で、準備完了は storageState の opened で見る。値は状態の JSON に含めない。
export function storageOpen() {
  return debug_storage_open();
}

export function storageKey(packId, startId) {
  return debug_storage_key(packId, startId);
}

export function storageWrite(packId, startId, value) {
  return debug_storage_write(packId, startId, value);
}

export function storageRead(packId, startId) {
  return debug_storage_read(packId, startId);
}

export function storageState(packId, startId) {
  return debug_storage_state(packId, startId);
}

// 離れる直前の書き出し（TASK-317 2/2）。saveStage は保留へ積むだけで書かない。書き出しの経路は pagehide（saveInstallPagehide で付ける）と saveFlush だけ。
export function saveStage(packId, startId, value) {
  return debug_save_stage(packId, startId, value);
}

export function saveInstallPagehide() {
  return debug_save_install_pagehide();
}

export function saveFlush() {
  return debug_save_flush();
}

// 履歴の書き出し（TASK-330 1/2）。exportPrepare は契約の試料（2 周分）の blob: URL を作り、JSON（ok / url / name / bytes）を返す。中身は返さない。
export function exportPrepare() {
  return debug_export_prepare();
}

// 保持している blob: URL を破棄する。破棄したら true。
export function exportRevoke() {
  return debug_export_revoke();
}

// ダウンロードの click は配信の本体に置かない（no_hidden_triggers が禁止）。試験の設定のこのファイルだけが <a download> を作って click し、すぐ外す。
export function exportClick(url, name) {
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
}

// ツリーの試料を後の状態で描いた HTML 文字列を返す（TASK-472）。kind は "file_tree" か "task_tree"、variant は "a" か "b"。範囲外は空文字列。
export function renderTree(kind, variant) {
  return debug_render_tree(kind, variant);
}

// 予約済みの次の進行の宛先（TASK-349）。JSON 文字列（generation / step / index）か "null"。読み取りのみ。
export function nextTick() {
  return debug_next_tick();
}

// 補完の action を配信と同じ配送の口（set_timeout → dispatch_action）で届ける（TASK-349）。
export function requestComplete() {
  debug_request_complete();
}

// 左のビューの共通の検査器（TASK-471 2/3）が使う代役の部品。#app の外の root の id を渡してマウントする。
// 返り値は固定形の JSON 文字列（mount: ok / error、dispatch: outcome、state: selected / variant / phase か "null"）。
export function viewFixtureMount(rootId) {
  return debug_view_fixture_mount(rootId);
}

export function viewFixtureDispatch(name, payload) {
  return debug_view_fixture_dispatch(name, payload);
}

export function viewFixtureState() {
  return debug_view_fixture_state();
}

// 開ける履歴の包み（TASK-330 2/2）。契約の試料の ops を適用し直したもので、受講中の記録ではない。
// exportPrepareLoadable は exportPrepare と同じ形（ok / url / name / bytes）の JSON を返す。
export function exportPrepareLoadable() {
  return debug_export_prepare_loadable();
}

// 開ける包みの JSON 文字列。importText に渡して基準の状態を作る。中身は試験の出力に出さない。
export function loadableHistoryText() {
  return debug_loadable_history_text();
}

// 履歴ファイルを選ぶ要素（TASK-330 2/2）。ファイルの選択と読み取りは DOM の値を読むため、Rust ではなくこのファイルが行う（check-dom-reads）。
// 選ばれたファイルの本文を、固定の接頭辞つきで見えない raw-trigger に載せて click する。結果は stateJson().engine.lastImport で読む。
const OPEN_HISTORY_PREFIX = 'open-history:';

function sendRaw(payload) {
  const trigger = document.querySelector('[data-testid="raw-trigger"]');
  if (!trigger) return false;
  trigger.setAttribute('data-payload', payload);
  trigger.click();
  trigger.setAttribute('data-payload', '');
  return true;
}

export function importInstall() {
  if (document.querySelector('[data-testid="history-file"]')) return;
  const input = document.createElement('input');
  input.type = 'file';
  input.hidden = true;
  input.setAttribute('data-testid', 'history-file');
  input.addEventListener('change', async () => {
    const file = input.files && input.files[0];
    if (!file) return;
    sendRaw(OPEN_HISTORY_PREFIX + (await file.text()));
  });
  document.body.appendChild(input);
}

// ファイルを経ずに同じ raw の経路で取り込む。基準の状態を作るために使う。
export function importText(text) {
  return sendRaw(OPEN_HISTORY_PREFIX + text);
}

// 任意の教材（IR の JSON）への差し替え（TASK-175 2/2）。版の違う履歴の、開く側の教材を入れ替える。読めなければ何も変わらない。
export function loadIr(text) {
  return sendRaw('load-ir:' + text);
}

// 開く側の版の上書き（TASK-175 2/2）。JSON 文字列 {"engine": "x.y.z" | null, "vocab_max": "x.y" | null}。両方 null で解除。
export function openingOverride(json) {
  return sendRaw('opening-override:' + json);
}

// 直近に開いた履歴の、開いた直後（進行の前）の状態の JSON 文字列か "null"。写しの一致の確認に使う。
export function lastImportStateJson() {
  return debug_last_import_state_json();
}
