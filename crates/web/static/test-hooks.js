// 試験の設定でだけ配る橋渡し。wasm の debug_* を JS から呼べる名前で出す。
import { debug_state_json, debug_rerender, debug_draft, debug_check_json_depth, debug_check_json_load, debug_check_doc, debug_render_doc, debug_render_quote_nest } from "./wasm/fandhe_workshop_web.js";

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
