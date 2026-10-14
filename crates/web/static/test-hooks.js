// 試験の設定でだけ配る橋渡し。wasm の debug_* を JS から呼べる名前で出す。
import { debug_state_json, debug_rerender, debug_check_json_depth, debug_check_json_load } from "./wasm/fandhe_workshop_web.js";

export function stateJson() {
  return debug_state_json();
}

// 再描画の測定用。true なら再構築を実行した。false は未マウントなどで何もしていない。
export function rerender() {
  return debug_rerender();
}

// 入れ子の深さの検査（TASK-157）。固定形の JSON 文字列（ok / reason）を返す。
export function checkJsonDepth(text) {
  return debug_check_json_depth(text);
}

// IR・履歴の読み込みの関門（TASK-157）。entry は "ir" か "history"。固定形の JSON 文字列（ok / stage / reason）を返す。
export function checkJsonLoad(entry, text) {
  return debug_check_json_load(entry, text);
}
