// tools/measure/browser.mjs の統計関数と引数の誤用の試験（node:test。scripts/test-measure.sh から呼ばれる）。
// ブラウザは起動しない。ブラウザを使う経路は all.sh の実行（scripts/test-measure.sh の (d)）で確かめる。
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { median, p95, toMicros, resolveLaunchOptions, planRoundsStage } from './browser.mjs';
import { cases, judge, expectedCounts, VR_THRESHOLD_US } from './view-redraw.mjs';

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

test('cli: log-rounds without --dir exits 2 with empty stdout', () => {
  const r = run(['--kind', 'log-rounds']);
  assert.equal(r.status, 2);
  assert.equal(r.stdout, '');
});

test('planRoundsStage: 6 groups at 500 rows split 244 inputs, the remainder going to the older groups', () => {
  const p = planRoundsStage({ groups: 6, rows: 500 });
  assert.equal(p.fillTotal, 244);
  assert.deepEqual(p.perGroup, [41, 41, 41, 41, 40, 40]);
  assert.deepEqual(p.window, [500, 578]);
  assert.equal(p.folded, false);
});

test('planRoundsStage: every stage keeps li on one side of 1,000 over the window and sums to the rows', () => {
  for (const groups of [6, 48]) {
    for (const rows of [500, 1200, 2000]) {
      const p = planRoundsStage({ groups, rows });
      assert.equal(2 * p.perGroup.reduce((a, b) => a + b, 0) + 2 * groups, rows);
      assert.equal(p.folded, rows >= 1200);
    }
  }
});

test('planRoundsStage: a stage crossing the fold target or with too few rows is refused', () => {
  assert.throws(() => planRoundsStage({ groups: 6, rows: 960 }), /crosses/);
  assert.throws(() => planRoundsStage({ groups: 48, rows: 100 }), /too few/);
  assert.throws(() => planRoundsStage({ groups: 6, rows: 501 }), /multiple/);
});
