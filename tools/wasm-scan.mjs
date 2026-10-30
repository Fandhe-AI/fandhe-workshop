#!/usr/bin/env node
// WASM をセクション単位で読み、名前が正確に `name` のカスタムセクション（関数名などのデバッグ情報）を除いた
// バイト列で文字列の件数を数える（TASK-82。tools/check-delivery.sh と tools/browser/delivery-ports.mjs が使う）。
//
// 除く理由: wasm-opt を適用しないビルド（CI の SKIP_WASM_OPT=1）では、`name` セクションに core 由来の関数名
// （debug_struct 等）が残り、バイト列全体の grep が配信する設定でも誤検出するため（2026-10-30 実測。他のセクションは 0 件）。
// 除くのは正確な名前 `name` だけで、`probe`・`namex` など他のカスタムセクションは数える（名前のずらしで抜けられない）。
//
// 使い方: node tools/wasm-scan.mjs <wasm> <pattern>...  →  `pattern<TAB>count` の行を出す。
// 終了コード: 0 = 成功 / 2 = 誤用、または WASM を解析できない（LEB128 超過・長さが範囲外・magic 違い。合格にしない）。
// 回帰テストは scripts/test-check-delivery.sh。パスと文字列は argv だけで受け、評価しない。
import fs from 'node:fs';
import { pathToFileURL } from 'node:url';

function leb(bytes, pos) {
  let result = 0;
  let shift = 0;
  for (let i = 0; i < 5; i += 1) {
    if (pos + i >= bytes.length) throw new Error('truncated LEB128');
    const b = bytes[pos + i];
    result += (b & 0x7f) * 2 ** shift;
    shift += 7;
    if ((b & 0x80) === 0) return [result, pos + i + 1];
  }
  throw new Error('LEB128 longer than 5 bytes');
}

/** `name` セクションを除いたバイト列の断片（Buffer の配列）を返す。壊れた入力は例外。 */
export function sectionsWithoutName(bytes) {
  const buf = Buffer.from(bytes);
  if (buf.length < 8 || buf.readUInt32BE(0) !== 0x0061736d || buf.readUInt32LE(4) !== 1) throw new Error('not a wasm module (magic/version)');
  const kept = [buf.subarray(0, 8)];
  let pos = 8;
  while (pos < buf.length) {
    const start = pos;
    const id = buf[pos];
    const [size, bodyStart] = leb(buf, pos + 1);
    const end = bodyStart + size;
    if (end > buf.length) throw new Error('section length out of range');
    let isName = false;
    if (id === 0) {
      const [nlen, nstart] = leb(buf, bodyStart);
      if (nstart + nlen > end) throw new Error('custom section name out of range');
      isName = buf.subarray(nstart, nstart + nlen).toString('latin1') === 'name';
    }
    if (!isName) kept.push(buf.subarray(start, end));
    pos = end;
  }
  return kept;
}

/** 文字列ごとの件数（`name` セクションを除く）。 */
export function scanWasm(bytes, patterns) {
  const hay = Buffer.concat(sectionsWithoutName(bytes));
  return Object.fromEntries(patterns.map((p) => {
    let n = 0;
    const needle = Buffer.from(p, 'latin1');
    for (let i = hay.indexOf(needle); i !== -1; i = hay.indexOf(needle, i + 1)) n += 1;
    return [p, n];
  }));
}

/** 全体（`name` を含む）での件数。 */
export function scanWasmAll(bytes, patterns) {
  const hay = Buffer.from(bytes);
  return Object.fromEntries(patterns.map((p) => [p, hay.includes(Buffer.from(p, 'latin1')) ? 1 : 0]));
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const [file, ...patterns] = process.argv.slice(2);
  if (!file || patterns.length === 0) {
    console.error('usage: wasm-scan.mjs <wasm> <pattern>...');
    process.exit(2);
  }
  try {
    const counts = scanWasm(fs.readFileSync(file), patterns);
    for (const p of patterns) console.log(`${p}\t${counts[p]}`);
  } catch (e) {
    console.error(`error: cannot scan wasm: ${e.message}`);
    process.exit(2);
  }
}
