// XSS の試験用の入力（testdata/untrusted-input-safety/test-data/xss-cases.md）を読む純粋な Node のモジュール。
//
// データの正は testdata の写し 1 か所に置き、ペイロードを tools 側へ書き写さない（試験用入力を重複して作らない。TASK-147）。
// ブラウザ試験（log-lines-xss.test.mjs など）から呼ばれ、ケースの ID・入り口・ペイロードを返す。
// 件数・ブロックの数・`\u{HEX}` の範囲が合わなければ例外にする（fail-closed。0 件のまま通る空振りを防ぐ）。
// 文字列は返すだけで、評価（eval・Function）はしない。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const DEFAULT_FILE = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../testdata/untrusted-input-safety/test-data/xss-cases.md',
);
export const EXPECTED_TOTAL = 64;
export const EXPECTED_E5 = 49;

const FENCE = '~~~~~~';

// `\u{HEX}`（1〜6 桁）を 1 文字へ展開する。サロゲートの範囲と U+10FFFF 超は拒否する。
function expand(s, id) {
  return s.replace(/\\u\{([0-9A-Fa-f]{1,6})\}/g, (_, hex) => {
    const cp = parseInt(hex, 16);
    if (cp > 0x10ffff || (cp >= 0xd800 && cp <= 0xdfff)) {
      throw new Error(`${id}: invalid code point \\u{${hex}}`);
    }
    return String.fromCodePoint(cp);
  });
}

export function parseXssCases(markdown, { total = EXPECTED_TOTAL, e5 = EXPECTED_E5 } = {}) {
  const text = markdown.replace(/\r\n?/g, '\n');
  const parts = text.split(/^### (XSS-\d+)[ \t]*$/m);
  const cases = [];
  const seen = new Set();
  for (let i = 1; i < parts.length; i += 2) {
    const id = parts[i];
    const body = parts[i + 1];
    if (seen.has(id)) throw new Error(`${id}: duplicate id`);
    seen.add(id);
    const category = /^- 分類:[ \t]*(.+)$/m.exec(body)?.[1].trim();
    const entriesLine = /^- 入り口:[ \t]*(.+)$/m.exec(body)?.[1];
    const fire = /^- 発火:[ \t]*(.+)$/m.exec(body)?.[1].trim();
    if (!category || !entriesLine || !fire) throw new Error(`${id}: missing metadata`);
    const lines = body.split('\n');
    const fences = lines.map((l, n) => (l.startsWith(FENCE) ? n : -1)).filter((n) => n >= 0);
    if (fences.length !== 2) throw new Error(`${id}: expected exactly one payload block, got ${fences.length} fences`);
    const payload = expand(lines.slice(fences[0] + 1, fences[1]).join('\n'), id);
    if (payload === '') throw new Error(`${id}: empty payload`);
    cases.push({ id, category, entries: entriesLine.split(',').map((e) => e.trim()), fire, payload });
  }
  if (cases.length !== total) throw new Error(`expected ${total} cases, got ${cases.length}`);
  const e5Count = cases.filter((c) => c.entries.includes('E5')).length;
  if (e5Count !== e5) throw new Error(`expected ${e5} E5 cases, got ${e5Count}`);
  return cases;
}

export function loadXssCases(file = DEFAULT_FILE) {
  return parseXssCases(fs.readFileSync(file, 'utf8'));
}

// 入り口に E5（入力欄）を含むケースだけ。
export function e5Payloads(file = DEFAULT_FILE) {
  return loadXssCases(file).filter((c) => c.entries.includes('E5'));
}
