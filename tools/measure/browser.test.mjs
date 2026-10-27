// tools/measure/browser.mjs の統計関数と引数の誤用の試験（node:test。scripts/test-measure.sh から呼ばれる）。
// ブラウザは起動しない。ブラウザを使う経路は all.sh の実行（scripts/test-measure.sh の (d)）で確かめる。
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { median, p95, toMicros, resolveLaunchOptions, checkCalibration, applyCpuThrottle, calibrationInfo } from './browser.mjs';
import { cases, judge, expectedCounts, VR_THRESHOLD_US } from './view-redraw.mjs';
import * as pr from './product-redraw.mjs';
import * as vr from './view-redraw.mjs';

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

test('cli: view-redraw without --dir exits 2 with empty stdout', () => {
  const r = run(['--kind', 'view-redraw']);
  assert.equal(r.status, 2);
  assert.equal(r.stdout, '');
});

// view-redraw の判定（純関数）。表の鍵は `${kind}:${variant}:${op}`。
function allWithin(us = 1000) {
  const t = {};
  for (const c of cases()) t[`${c.kind}:${c.variant}:${c.op}`] = us;
  return t;
}

test('view-redraw cases: 12 combinations x 2 ops, no rejected variant, switch source differs and allows the variant', () => {
  const list = cases();
  assert.equal(list.length, 24);
  assert.ok(!list.some((c) => (c.kind === 'document' || c.kind === 'diff') && c.variant === 'a'));
  for (const c of list) {
    assert.notEqual(c.from, c.kind);
    assert.ok(!((c.from === 'document' || c.from === 'diff') && c.variant === 'a'), `${c.kind}:${c.variant} from ${c.from}`);
  }
});

test('view-redraw judge: all within -> every kind within, judged flags set', () => {
  const j = judge(allWithin());
  assert.ok(Object.values(j).every((v) => v.within));
  assert.deepEqual(Object.keys(j).filter((k) => j[k].judged).sort(), ['diff', 'document', 'task_tree']);
});

test('view-redraw judge: diff has only variant b, so one slow op makes diff over', () => {
  const t = allWithin();
  t['diff:b:switch'] = VR_THRESHOLD_US + 1;
  const j = judge(t);
  assert.equal(j.diff.within, false);
  assert.equal(j.document.within, true);
});

test('view-redraw judge: the threshold itself is within; one variant within on both ops is enough', () => {
  const t = allWithin();
  t['diff:b:toggle'] = VR_THRESHOLD_US;
  assert.equal(judge(t).diff.within, true);
  t['task_tree:a:toggle'] = VR_THRESHOLD_US + 1;
  assert.equal(judge(t).task_tree.within, true);
  t['task_tree:b:switch'] = VR_THRESHOLD_US + 1;
  assert.equal(judge(t).task_tree.within, false);
});

test('view-redraw judge: a missing p95 throws', () => {
  const t = allWithin();
  delete t['document:b:switch'];
  assert.throws(() => judge(t));
});

test('view-redraw expectedCounts: hand-counted sizes of the scale data', () => {
  const c = expectedCounts();
  assert.deepEqual(c.document, { before: 80, after: 97 });
  assert.deepEqual(c.diff, { before: 479, after: 500 });
  assert.deepEqual(c.task_tree, { before: 46, after: 50 });
  assert.deepEqual(c.file_tree, { before: 191, after: 200 });
  assert.deepEqual(c.card_list, { before: 20, after: 20 });
  assert.deepEqual(c.check_list, { before: 20, after: 20 });
});

test('cli: log-slope without --dir exits 2 with empty stdout', () => {
  const r = run(['--kind', 'log-slope']);
  assert.equal(r.status, 2);
  assert.equal(r.stdout, '');
});

// product-redraw の組・名前・判定（純関数）。
test('product-redraw cases: 7 kinds, 3 judged, from is the previous kind and differs', () => {
  const list = pr.cases();
  assert.equal(list.length, 7);
  assert.deepEqual(list.filter((c) => c.judged).map((c) => `${c.kind}:${c.rows}`), ['task_tree:50', 'document:200', 'diff:500']);
  for (const c of list) assert.notEqual(c.from, c.kind);
});

test('product-redraw measurementName: kind, rows, switch and stat', () => {
  assert.equal(pr.measurementName({ kind: 'diff', rows: 500 }, 'p95'), 'product_redraw_diff_500_switch_p95');
});

test('product-redraw judge: switch only, partial, threshold boundary is within', () => {
  const p95 = Object.fromEntries(pr.cases().map((c) => [`${c.kind}:${c.rows}`, 1000]));
  p95['diff:500'] = pr.PR_THRESHOLD_US;
  let v = pr.judge(p95);
  assert.equal(v.diff.within, true);
  assert.ok(Object.values(v).every((j) => j.partial === true));
  p95['diff:500'] = pr.PR_THRESHOLD_US + 1;
  v = pr.judge(p95);
  assert.equal(v.diff.within, false);
  assert.equal(v.task_tree.within, true);
});

test('product-redraw judge: a missing p95 throws', () => {
  assert.throws(() => pr.judge({}), /missing p95/);
});

test('product-redraw sampleDiffRows: counts hunk and line rows of the sample', () => {
  const r = pr.sampleDiffRows();
  assert.equal(r.total, r.hunk + r.line);
  assert.ok(r.hunk >= 1 && r.line >= 1);
});

test('cli: product-redraw without --dir exits 2 with empty stdout', () => {
  const r = run(['--kind', 'product-redraw']);
  assert.equal(r.status, 2);
  assert.equal(r.stdout, '');
});

// CPU 4 倍の記録（ART-15。TASK-485 3/3）の純関数と誤用。ブラウザは起動しない。
test('checkCalibration: a ratio of exactly 2 passes, below 2 throws', () => {
  const ok = checkCalibration(10, 20, 2);
  assert.equal(ok.ratio, 2);
  assert.equal(ok.unthrottled_ms, 10);
  assert.equal(ok.throttled_ms, 20);
  assert.throws(() => checkCalibration(10, 19.99, 2), /not in effect/);
});

test('checkCalibration: zero, negative, NaN and Infinity throw', () => {
  for (const [a, b] of [[0, 10], [10, 0], [-1, 10], [10, -1], [NaN, 10], [10, NaN], [Infinity, 10], [10, Infinity]]) {
    assert.throws(() => checkCalibration(a, b, 2), /not in effect/, `${a},${b}`);
  }
});

test('calibrationInfo: min and max of the ratios, empty or non-finite throws', () => {
  const i = calibrationInfo([{ ratio: 3.5 }, { ratio: 4.2 }, { ratio: 3.9 }]);
  assert.equal(i.ratio_min, 3.5);
  assert.equal(i.ratio_max, 4.2);
  assert.equal(i.checks, 3);
  assert.throws(() => calibrationInfo([]));
  assert.throws(() => calibrationInfo([{ ratio: NaN }]));
});

test('applyCpuThrottle: only the rate 4 reaches CDP; other rates throw before any call', async () => {
  let calls = 0;
  const sent = [];
  const context = { newCDPSession: async () => { calls++; return { send: async (m, p) => { sent.push([m, p]); } }; } };
  for (const rate of [1, 2, 6, 0, -4, NaN, '4', undefined]) {
    await assert.rejects(() => applyCpuThrottle(context, {}, rate), /rate must be 4/);
  }
  assert.equal(calls, 0);
  await applyCpuThrottle(context, {}, 4);
  assert.deepEqual(sent, [['Emulation.setCPUThrottlingRate', { rate: 4 }]]);
});

test('view-redraw-cpu4x cases: 3 kinds, 8 cases, no rejected (a) variants, from differs', () => {
  const list = vr.cpu4xCases();
  assert.equal(list.length, 8);
  assert.deepEqual([...new Set(list.map((c) => c.kind))].sort(), ['diff', 'document', 'task_tree']);
  assert.ok(!list.some((c) => (c.kind === 'document' || c.kind === 'diff') && c.variant === 'a'));
  assert.ok(list.some((c) => c.kind === 'task_tree' && c.variant === 'a'));
  for (const c of list) assert.notEqual(c.from, c.kind);
});

test('view-redraw-cpu4x measurementName: cpu4x prefix, kind, variant, op and stat', () => {
  assert.equal(vr.cpu4xMeasurementName({ kind: 'diff', variant: 'b', op: 'switch' }, 'p95'), 'view_redraw_cpu4x_diff_b_switch_p95');
});

function allP95(v) {
  return Object.fromEntries(vr.cpu4xCases().map((c) => [`${c.kind}:${c.variant}:${c.op}`, v]));
}

test('view-redraw-cpu4x record: over_threshold only, no judgement fields, boundary is not over', () => {
  const t = allP95(1000);
  t['diff:b:switch'] = vr.VR_THRESHOLD_US;
  let r = vr.cpu4xRecord(t);
  assert.equal(r.diff.b.over_threshold, false);
  t['diff:b:switch'] = vr.VR_THRESHOLD_US + 1;
  r = vr.cpu4xRecord(t);
  assert.equal(r.diff.b.over_threshold, true);
  assert.equal(r.document.b.over_threshold, false);
  assert.deepEqual(Object.keys(r).sort(), ['diff', 'document', 'task_tree']);
  const json = JSON.stringify(r);
  assert.ok(!json.includes('within') && !json.includes('judged'));
  assert.ok('toggle_p95_us' in r.diff.b && 'switch_p95_us' in r.diff.b);
});

test('view-redraw-cpu4x record: a missing p95 throws', () => {
  const t = allP95(1000);
  delete t['task_tree:a:toggle'];
  assert.throws(() => vr.cpu4xRecord(t), /missing p95/);
});

test('product-redraw-cpu4x cases: 3 judged kinds with their sizes', () => {
  const list = pr.cpu4xCases();
  assert.deepEqual(list.map((c) => `${c.kind}:${c.rows}`), ['task_tree:50', 'document:200', 'diff:500']);
});

test('product-redraw-cpu4x measurementName and record', () => {
  assert.equal(pr.cpu4xMeasurementName({ kind: 'diff', rows: 500 }, 'p95'), 'product_redraw_cpu4x_diff_500_switch_p95');
  const p95 = Object.fromEntries(pr.cpu4xCases().map((c) => [`${c.kind}:${c.rows}`, 1000]));
  p95['diff:500'] = pr.PR_THRESHOLD_US;
  let r = pr.cpu4xRecord(p95);
  assert.equal(r.diff.over_threshold, false);
  p95['diff:500'] = pr.PR_THRESHOLD_US + 1;
  r = pr.cpu4xRecord(p95);
  assert.equal(r.diff.over_threshold, true);
  assert.equal(r.diff.rows, 500);
  const json = JSON.stringify(r);
  assert.ok(!json.includes('within') && !json.includes('judged') && !json.includes('partial'));
  assert.throws(() => pr.cpu4xRecord({}), /missing p95/);
});

test('cli: the cpu4x kinds without --dir exit 2 with empty stdout', () => {
  for (const kind of ['view-redraw-cpu4x', 'product-redraw-cpu4x']) {
    const r = run(['--kind', kind]);
    assert.equal(r.status, 2, kind);
    assert.equal(r.stdout, '');
  }
});
