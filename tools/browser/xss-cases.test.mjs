// xss-cases.mjs（XSS の試験用入力のパーサ）の単体試験。ブラウザは使わない。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadXssCases, e1Payloads, e5Payloads, parseXssCases } from './xss-cases.mjs';

const block = (id, entries, payload) => `### ${id}\n- 分類: 1\n- 入り口: ${entries}\n- 発火: hit\n\n~~~~~~text\n${payload}\n~~~~~~\n`;
const opts = { total: 1, e5: 1 };

test('64 件と、入り口に E5 を含む 49 件を読む', () => {
  assert.equal(loadXssCases().length, 64);
  assert.equal(e5Payloads().length, 49);
});

test('E5 の一覧に E5 を含まないケースは入らない', () => {
  const ids = e5Payloads().map((c) => c.id);
  assert.ok(!ids.includes('XSS-60'));
  assert.ok(ids.includes('XSS-47'));
});

test('\\u{HEX} は 1 文字に展開される', () => {
  const all = loadXssCases();
  const get = (id) => all.find((c) => c.id === id).payload;
  assert.equal(get('XSS-47').charCodeAt(1), 0);
  assert.ok(get('XSS-49').startsWith('‮'));
  assert.ok(get('XSS-49').endsWith('紛らわしい制御文字だけ'));
  assert.ok(!get('XSS-47').includes('\\u{'));
});

test('複数行のペイロードは LF でつなぐ', () => {
  const all = loadXssCases();
  assert.equal(all.find((c) => c.id === 'XSS-47').payload.split('\n').length, 2);
  assert.ok(all.find((c) => c.id === 'XSS-05').payload.includes('\n'));
});

test('壊れた入力は例外にする', () => {
  assert.doesNotThrow(() => parseXssCases(block('XSS-01', 'E5', 'a\\u{41}'), opts));
  assert.equal(parseXssCases(block('XSS-01', 'E5', 'a\\u{41}'), opts)[0].payload, 'aA');
  assert.throws(() => parseXssCases(block('XSS-01', 'E5', 'x\\u{D800}'), opts), /invalid code point/);
  assert.throws(() => parseXssCases(block('XSS-01', 'E5', 'x\\u{110000}'), opts), /invalid code point/);
  assert.throws(() => parseXssCases(block('XSS-01', 'E5', ''), opts), /empty payload/);
  assert.throws(() => parseXssCases('### XSS-01\n- 分類: 1\n- 入り口: E5\n- 発火: hit\n', opts), /payload block/);
  assert.throws(() => parseXssCases(block('XSS-01', 'E1', 'a'), opts), /E5 cases/);
  assert.throws(() => parseXssCases(block('XSS-01', 'E5', 'a') + block('XSS-01', 'E5', 'b'), { total: 2, e5: 2 }), /duplicate/);
  assert.throws(() => parseXssCases(block('XSS-01', 'E5', 'a'), { total: 2, e5: 1 }), /expected 2 cases/);
});

test('入り口に E1 を含む 49 件を読み、E1 を含まないケースは入らない', () => {
  const all = loadXssCases();
  const cases = e1Payloads();
  assert.equal(cases.length, 49);
  assert.ok(cases.every((c) => c.entries.includes('E1')));
  const ids = cases.map((c) => c.id);
  assert.ok(ids.includes('XSS-01'));
  assert.ok(ids.includes('XSS-47'));
  const excluded = all.filter((c) => !c.entries.includes('E1'));
  assert.equal(excluded.length, 15);
  for (const c of excluded) assert.ok(!ids.includes(c.id), c.id);
});
