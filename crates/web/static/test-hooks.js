// 試験の設定でだけ配る橋渡し。wasm の debug_* を JS から呼べる名前で出す。
import { debug_state_json } from "./wasm/fandhe_workshop_web.js";

export function stateJson() {
  return debug_state_json();
}
