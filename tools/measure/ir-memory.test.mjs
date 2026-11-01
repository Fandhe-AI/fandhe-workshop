// tools/measure/ir-memory.mjs の純関数の回帰テスト（playwright もブラウザも使わない。scripts/test-measure-ir-memory.sh から呼ばれる）。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  parsePsRss, rendererPids, buildCaseIr, summarizeCases, exitCodeFor, checkExpectation, HUGE_CHARS, MAX_IR_BYTES,
} from './ir-memory.mjs';

const BASE = fs.readFileSync(new URL('../../testdata/engine-event-consistency/test-data/min-3step.json', import.meta.url), 'utf8');

test('parsePsRss: Linux 形・macOS 形・消えた pid・要求外の pid', () => {
  assert.deepEqual(parsePsRss('  101 2048\n  102 4096\n', [101, 102]), { total: 6144, seen: 2 });
  assert.deepEqual(parsePsRss('  101   2048\n', [101, 102]), { total: 2048, seen: 1 });
  assert.deepEqual(parsePsRss('', [101]), { total: 0, seen: 0 });
  assert.deepEqual(parsePsRss('999 5\n101 7\n', [101]), { total: 7, seen: 1 });
});

test('parsePsRss: 読めない行は例外', () => {
  assert.throws(() => parsePsRss('PID RSS\n101 7\n', [101]));
  assert.throws(() => parsePsRss('101 abc\n', [101]));
});

test('rendererPids: renderer の数値の pid だけ', () => {
  const info = { processInfo: [{ type: 'browser', id: 1 }, { type: 'renderer', id: 22 }, { type: 'renderer', id: 'x' }, { type: 'gpu', id: 3 }] };
  assert.deepEqual(rendererPids(info), [22]);
  assert.deepEqual(rendererPids(null), []);
});

test('buildCaseIr: 欄の位置・長さ・バイト数。base は無変更', () => {
  const b = buildCaseIr(BASE, 'base');
  assert.equal(b.text, BASE);
  const t = buildCaseIr(BASE, 'pack_title');
  assert.equal(JSON.parse(t.text).pack.title.length, HUGE_CHARS);
  assert.equal(t.field, 'pack.title');
  assert.ok(t.bytes <= MAX_IR_BYTES);
  const h = buildCaseIr(BASE, 'hint_text');
  assert.equal(JSON.parse(h.text).hints['h-1-1'].text.length, HUGE_CHARS);
  assert.equal(JSON.parse(h.text).pack.title, JSON.parse(BASE).pack.title);
  assert.throws(() => buildCaseIr(BASE, 'nope'));
  assert.throws(() => buildCaseIr('{"pack":{},"hints":{}}', 'hint_text'));
});

test('summarizeCases: 最大と中央値の丸め・0 は例外', () => {
  const m = summarizeCases([{ name: 'base', peaks: [100, 103, 102] }, { name: 'pack_title', peaks: [10, 11] }]);
  assert.deepEqual(m.map((x) => [x.name, x.value]), [
    ['ir_memory_base_renderer_rss_max', 103], ['ir_memory_base_renderer_rss_median', 102],
    ['ir_memory_pack_title_renderer_rss_max', 11], ['ir_memory_pack_title_renderer_rss_median', 11],
  ]);
  assert.throws(() => summarizeCases([{ name: 'base', peaks: [0] }]));
  assert.throws(() => summarizeCases([{ name: 'base', peaks: [] }]));
});

test('exitCodeFor', () => {
  assert.equal(exitCodeFor(0), 0);
  assert.equal(exitCodeFor(1), 3);
});

test('checkExpectation: 期待と違う観察は理由を返す', () => {
  assert.equal(checkExpectation('base', { notices: 0, titles: 1 }), null);
  assert.ok(checkExpectation('base', { notices: 0, titles: 0 }));
  assert.equal(checkExpectation('pack_title', { notices: 1, reason: 'string_too_long' }), null);
  assert.ok(checkExpectation('pack_title', { notices: 1, reason: 'too_large' }));
  assert.ok(checkExpectation('hint_text', { notices: 0, reason: null }));
  assert.equal(checkExpectation('hint_text', { crashed: true }), null);
});
