// `data-payload` の書き換え試験（TASK-148。SAFE-6）の入力を読む純粋な Node のモジュール。
//
// 値の正は spec の PoC-7 の追補 4 の写し（testdata/untrusted-input-safety/test-data-addendum-4/extra-cases.json）の `e5_data_payload` 1 か所に置き、
// ペイロードを tools 側へ書き写さない。呼び出し元は data-payload-tamper.mjs（書き換えの補助）と data-payload-tamper.test.mjs。
// 写しの sha256（test-data-addendum-4.sha256）・対象 10 件・XSS の ID 6 件・壊れた値 7 件が合わなければ例外にする（fail-closed。0 件のまま通る空振りを防ぐ）。
// 文字列は返すだけで、評価（eval・Function）はしない。
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadXssCases } from './xss-cases.mjs';

const SAFETY_DIR = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../testdata/untrusted-input-safety',
);
export const DEFAULT_FILE = path.join(SAFETY_DIR, 'test-data-addendum-4/extra-cases.json');
export const DEFAULT_SHA_FILE = path.join(SAFETY_DIR, 'test-data-addendum-4.sha256');
export const EXPECTED_TARGETS = 10;
export const EXPECTED_VALUES = 6;
export const EXPECTED_RAW = 7;

// sha256 の一覧（`<hash>  <相対パス>`）から file の期待値を引き、実際の内容と照合する。一致しなければ例外。
export function verifySha(file, shaFile) {
  const rel = 'test-data-addendum-4/extra-cases.json';
  const line = fs.readFileSync(shaFile, 'utf8').split('\n').find((l) => l.trim().endsWith(` ${rel}`));
  if (!line) throw new Error(`sha256 の一覧に ${rel} が無い`);
  const want = line.trim().split(/\s+/)[0];
  const got = crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  if (want !== got) throw new Error('extra-cases.json の sha256 が一覧と一致しない');
}

// `e5_data_payload` の解析。件数が違えば例外。`values` の XSS の ID は `xssById`（ID → 文字列）で引き、引けなければ例外。
export function parseDataPayloadCases(json, xssById) {
  const e5 = json?.e5_data_payload;
  if (!e5) throw new Error('e5_data_payload が無い');
  const strings = (a, n, name) => {
    if (!Array.isArray(a) || a.length !== n || !a.every((x) => typeof x === 'string')) {
      throw new Error(`${name}: ${n} 件の文字列でない`);
    }
    return a;
  };
  const targets = strings(e5.targets, EXPECTED_TARGETS, 'targets');
  const ids = strings(e5.values, EXPECTED_VALUES, 'values');
  const raw = strings(e5.raw_values, EXPECTED_RAW, 'raw_values');
  const values = ids.map((id) => {
    const payload = xssById.get(id);
    if (typeof payload !== 'string' || payload === '') throw new Error(`${id}: XSS のケースに無い`);
    return { label: id, value: payload };
  });
  const rawValues = raw.map((value, i) => ({ label: `raw[${i}]`, value }));
  return { targets, values: [...values, ...rawValues] };
}

// 既定の写しを読む。sha256・件数を照合してから返す。
export function loadDataPayloadCases({ file = DEFAULT_FILE, shaFile = DEFAULT_SHA_FILE, xssFile } = {}) {
  verifySha(file, shaFile);
  const xss = new Map(loadXssCases(xssFile).map((c) => [c.id, c.payload]));
  return parseDataPayloadCases(JSON.parse(fs.readFileSync(file, 'utf8')), xss);
}
