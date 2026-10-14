// 試験の設定でだけ配る橋渡し。wasm の debug_* を JS から呼べる名前で出す。
import { debug_state_json, debug_rerender, debug_draft } from "./wasm/fandhe_workshop_web.js";

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
