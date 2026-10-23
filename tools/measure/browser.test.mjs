// tools/measure/browser.mjs の統計関数と引数の誤用の試験（node:test。scripts/test-measure.sh から呼ばれる）。
// ブラウザは起動しない。ブラウザを使う経路は all.sh の実行（scripts/test-measure.sh の (d)）で確かめる。
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { median, p95, toMicros, resolveLaunchOptions } from './browser.mjs';

const SCRIPT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'browser.mjs');

test('median: odd count picks the middle value', () => {
  assert.equal(median([5, 1, 3]), 3);
});

test('median: even count averages the two middle values', () => {
  assert.equal(median([4, 1, 3, 2]), 2.5);
});

test('median: empty list throws', () => {
  assert.throws(() => median([]));
});

test('p95: nearest-rank on 20 values picks the 19th', () => {
  const v = Array.from({ length: 20 }, (_, i) => i + 1);
  assert.equal(p95(v), 19);
});

test('p95: nearest-rank on 200 values picks the 190th', () => {
  const v = Array.from({ length: 200 }, (_, i) => 200 - i);
  assert.equal(p95(v), 190);
});

test('p95: single value', () => {
  assert.equal(p95([7]), 7);
});

test('toMicros: rounds milliseconds to integer microseconds', () => {
  assert.equal(toMicros(0.0126), 13);
  assert.equal(toMicros(1.5), 1500);
});

test('toMicros: rejects values that round to 0 or are not finite', () => {
  assert.throws(() => toMicros(0.0004));
  assert.throws(() => toMicros(0));
  assert.throws(() => toMicros(-1));
  assert.throws(() => toMicros(Number.NaN));
});

test('resolveLaunchOptions: chromium by default, chrome by channel, anything else throws', () => {
  assert.deepEqual(resolveLaunchOptions({}), {});
  assert.deepEqual(resolveLaunchOptions({ FANDHE_BROWSER_CHANNEL: '' }), {});
  assert.deepEqual(resolveLaunchOptions({ FANDHE_BROWSER_CHANNEL: 'chrome' }), { channel: 'chrome' });
  assert.throws(() => resolveLaunchOptions({ FANDHE_BROWSER_CHANNEL: 'msedge' }));
});

function run(args) {
  return spawnSync(process.execPath, [SCRIPT, ...args], { encoding: 'utf8' });
}

test('cli: unknown option exits 2 with empty stdout', () => {
  const r = run(['--bogus']);
  assert.equal(r.status, 2);
  assert.equal(r.stdout, '');
});

test('cli: missing --dir exits 2', () => {
  const r = run(['--kind', 'redraw']);
  assert.equal(r.status, 2);
  assert.equal(r.stdout, '');
});

test('cli: unknown --kind exits 2', () => {
  const r = run(['--kind', 'other', '--dir', '.']);
  assert.equal(r.status, 2);
  assert.equal(r.stdout, '');
});

test('cli: log-redraw without --dir exits 2 with empty stdout', () => {
  const r = run(['--kind', 'log-redraw']);
  assert.equal(r.status, 2);
  assert.equal(r.stdout, '');
});

test('cli: input-paint without --dir exits 2 with empty stdout', () => {
  const r = run(['--kind', 'input-paint']);
  assert.equal(r.status, 2);
  assert.equal(r.stdout, '');
});
