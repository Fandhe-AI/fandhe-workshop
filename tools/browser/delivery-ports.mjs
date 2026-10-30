// 配信物の部品の試験（delivery-ports.test.mjs。TASK-547 3/3）が使う補助関数。試験ファイルは import されると試験を登録してしまうため、
// 純粋な読み取りと解析だけをここに分ける。呼び出し元は delivery-ports.test.mjs だけ。
//
// 役割: (1) 画面の DOM から「試験用の口」らしい部品を数える探針、(2) 配信された WASM を同一オリジンの GET で取得して書き出しとバイト列を調べる関数、
// (3) tools/check-delivery.sh の出力を項目名 → 合否の表に読む関数と、ブラウザ側の観察との対応表。
// 返す値は名前・件数・合否だけで、DOM の文字は返さない。

import { scanWasm } from '../wasm-scan.mjs';

export const HIDDEN_NAMES = ['tick-trigger', 'esc-trigger', 'complete-trigger', 'raw-trigger'];
export const NEXT_CHAPTER = '[data-testid="shell-next-chapter"]';

/** 画面の部品の探針（読み取りだけ）。`hidden` は隠し要素 4 種の名前ごとの件数、`moves` は「移動:」で始まる操作の数。 */
export function probeParts(page) {
  return page.evaluate(([names, next]) => {
    const attrs = ['data-testid', 'id', 'class', 'data-action', 'name'];
    const hidden = Object.fromEntries(names.map((n) => [n, 0]));
    for (const el of document.querySelectorAll('*')) {
      for (const n of names) {
        if (attrs.some((a) => (el.getAttribute(a) ?? '').split(/\s+/).includes(n))) hidden[n] += 1;
      }
    }
    const squash = (s) => (s ?? '').replace(/\s+/g, '');
    const moves = [...document.querySelectorAll('button,a,[role="button"]')].filter((e) => squash(e.textContent).startsWith('移動:')).length;
    return {
      hidden,
      moves,
      nextChapter: document.querySelectorAll(next).length,
      hooks: typeof window.__fandheTest,
      meta: document.querySelectorAll('meta[name="fandhe-test-hooks"]').length,
      // TERM-41 は検討中のため記録だけ（合否に使わない）。
      bodyRef: [...document.querySelectorAll('li,p,div,span')].filter((e) => e.children.length === 0 && squash(e.textContent).includes('本文の参照:')).length,
    };
  }, [HIDDEN_NAMES, NEXT_CHAPTER]);
}

/** 部品の探針の結果が「口なし」か。配信する設定の全時点の期待。 */
export function isPortFree(p) {
  return Object.values(p.hidden).every((n) => n === 0) && p.moves === 0 && p.nextChapter === 0 && p.hooks === 'undefined' && p.meta === 0;
}

const WASM_PATH = 'wasm/fandhe_workshop_web_bg.wasm';
const BYTE_NAMES = ['debug_', 'raw-trigger', 'tick-trigger', 'esc-trigger', 'complete-trigger'];

/**
 * ページ内で WASM を同一オリジンから取得し、書き出しの名前の列を得て、バイト列の中の名前の有無を Node 側で数える。
 * 数え方は tools/wasm-scan.mjs（`name` セクションを除く）で、check-delivery.sh と同じ。
 */
export async function inspectWasm(page) {
  const got = await page.evaluate(async (rel) => {
    const res = await fetch(new URL(rel, document.baseURI).href);
    if (!res.ok) throw new Error(`wasm fetch ${res.status}`);
    const buf = await res.arrayBuffer();
    const mod = await WebAssembly.compile(buf);
    const exports = WebAssembly.Module.exports(mod).map((e) => e.name);
    let bin = '';
    const u8 = new Uint8Array(buf);
    for (let i = 0; i < u8.length; i += 0x8000) bin += String.fromCharCode(...u8.subarray(i, i + 0x8000));
    return { exports, b64: btoa(bin), size: buf.byteLength };
  }, WASM_PATH);
  const counts = scanWasm(Buffer.from(got.b64, 'base64'), BYTE_NAMES);
  return { exports: got.exports, bytes: Object.fromEntries(BYTE_NAMES.map((n) => [n, counts[n] > 0])), size: got.size };
}

/** 配信元の 1 パスの HTTP の状態（Node 側の GET。ページの通信の計数には入らない）。 */
export async function statusOf(srv, rel) {
  const res = await fetch(new URL(rel, srv.url).href);
  await res.arrayBuffer();
  return res.status;
}

/** check-delivery.sh と突き合わせる項目名の一覧。出力にこの名前が 1 つでも欠けたら読み取りを失敗にする（fail-closed）。 */
export const REQUIRED_ITEMS = [
  'mount_app is exported',
  'no debug_* export in wasm',
  'no fandhe-test-hooks meta in index.html',
  'no test-hooks.js',
  'no .d.ts',
  'no raw-trigger bytes in wasm',
  'no tick-trigger bytes in wasm',
  'no esc-trigger bytes in wasm',
  'no complete-trigger bytes in wasm',
  'no debug_ bytes in wasm',
];

// FAIL の行は、項目名が「見つかった」の形に変わるものがある。ok の形の名前へ正規化するための対応。
const FAIL_FORMS = [
  ['debug_* export found in wasm', 'no debug_* export in wasm'],
  ['test-hooks.js found', 'no test-hooks.js'],
  ['.d.ts found', 'no .d.ts'],
];

/** `ok: <項目>` / `FAIL: <項目>` の行を Map(項目名 → 合否) に読む。必須の項目名が欠けたら例外（出力の形が変わっても空振りで合格しない）。 */
export function parseCheckDelivery(stdout) {
  const out = new Map();
  for (const line of stdout.split('\n')) {
    const m = /^(ok|FAIL): (.*)$/.exec(line);
    if (!m) continue;
    const hit = m[1] === 'FAIL' ? FAIL_FORMS.find(([f]) => m[2].startsWith(f)) : undefined;
    out.set(hit ? hit[1] : m[2], m[1] === 'ok');
  }
  const missing = REQUIRED_ITEMS.filter((n) => !out.has(n));
  if (missing.length > 0) throw new Error(`check-delivery output lacks required items: ${missing.join(' | ')}`);
  return out;
}

/**
 * ブラウザ側の観察（exports・バイト列・HTTP の状態・DOM）から、check-delivery.sh の項目ごとの期待の合否を作る。
 * `debug_` のバイトの項目は wasm-opt の有無で値が変わるが、両側の判定が一致することを試験が確かめる。
 */
export function expectedTable(obs) {
  return new Map([
    ['mount_app is exported', obs.exports.includes('mount_app')],
    ['no debug_* export in wasm', !obs.exports.some((n) => n.startsWith('debug_'))],
    ['no fandhe-test-hooks meta in index.html', obs.meta === 0],
    ['no test-hooks.js', obs.hooksJsStatus === 404],
    ['no .d.ts', obs.dtsStatuses.every((s) => s === 404)],
    ['no raw-trigger bytes in wasm', !obs.bytes['raw-trigger']],
    ['no tick-trigger bytes in wasm', !obs.bytes['tick-trigger']],
    ['no esc-trigger bytes in wasm', !obs.bytes['esc-trigger']],
    ['no complete-trigger bytes in wasm', !obs.bytes['complete-trigger']],
    ['no debug_ bytes in wasm', !obs.bytes['debug_']],
  ]);
}
