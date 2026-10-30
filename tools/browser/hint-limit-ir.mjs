// ヒントの上限の試験（TASK-530 2/2。hint-open の場面 H・egress-count の G4）が配信する設定の画面へ置く IR の変種。
//
// 手書きの `ch0-3` の IR を `variantOf` で配信向けにし、`main/st-0-1` に 3 段（文面にマークアップの形を含む）と、水準 C だけで出す
// 4 つ目のヒントを足す。既定の案内レベル A では 3 段が上限になり、4 つ目の文面は画面のどこにも出てはならない。
// 文面は秘密情報ではない固定の試験データで、出力には出さない。
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { variantOf } from './author-preview.mjs';

const IR_03 = fileURLToPath(new URL('../../crates/web/tests/data/ch0_3/ir.json', import.meta.url));

export const LIMIT_UID = 'main/st-0-1';
export const LIMIT_TEXTS = ['一段目', '<b>二段目</b>', '三段目'];
export const HIDDEN_TEXT = '伏せる四段目';

export function hintLimitIr() {
  const ir = variantOf(JSON.parse(fs.readFileSync(IR_03, 'utf8')));
  const defs = [['h-a', LIMIT_TEXTS[0]], ['h-b', LIMIT_TEXTS[1]], ['h-c', LIMIT_TEXTS[2]], ['h-d', HIDDEN_TEXT]];
  for (const [id, text] of defs) ir.hints[id] = { level: 1, text };
  ir.hints['h-d'].levels = ['C'];
  const node = ir.nodes.find((n) => n.uid === LIMIT_UID);
  node.step.hints = defs.map(([id]) => id);
  return ir;
}
