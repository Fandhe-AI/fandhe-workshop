// 復帰と処理中でない Esc の縦断試験（TASK-222 3/3。B-6・B-6r・K-10。EVT-6・EVT-8・EVT-10・EVT-12）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、実タイマーの進行（表示側の 300ms）を流しながら、状態の不変条件を確かめる。
//   B-6  処理中（進行 3 段）で保存し、4 段で復帰すると、世代が 2 になり保存時の実行状態へ戻る。復帰前の宛先の進行は状態を変えない
//   B-6r 実タイマーのまま復帰しても、進行は（世代, 番号）ごとに 1 回で、受付済みで止まる
//   K-10 処理中でない停止（Esc）は取り消しの入力になり、世代は変わらない。入力の受け取りと拒否が各 1 件だけ増える
//
// 保存・復帰・停止・取り消しは、見えない `raw-trigger` に `Op` の JSON を書いて click する経路で送る（stop-move-ignore と同じ）。
// 実キーの Esc の取り消しは esc-outcomes が確かめる（TASK-441 2/2）。ここは tick-trigger 型の隠し要素を FEND-4 が禁じるため raw 経路で送る。
// 進行の番号は 0 始まりである。元の PoC の 1 始まり「1:1,1:2,1:3,1:4,2:4,2:5」は [[1,0],[1,1],[1,2],[1,3],[2,3],[2,4]] に当たる。
// 進行の間隔は 300ms である。spec の TASK-222 は 60ms と書くが、D-47・EVT-18 が後の決定でビルド時の定数 300ms にした。
// 実タイマーの進行は、別々の `page.evaluate` の間に割り込む。そこで「進行が n 段になったことの確認 → click → 書き出し」を
// 1 回の evaluate の同期区間の中で行う。保存と復帰は別の段（3 段・4 段）で行うため、evaluate を 2 回に分ける。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-restore-cancel/';
const RAW = '[data-testid="raw-trigger"]';
const COMMAND = { input: { command: '/make-note first-note' } };
const SAVE = { control: 'save' };
const STOP = { control: 'stop' };
const CANCEL = { input: 'cancel' };
const restoreOp = (id) => ({ control: { restore: id } });
const TOTAL_TICKS = 5;
const ST1 = 'main/st-1';

let browser;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const nextTick = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.nextTick()));
const interval = (page) => page.evaluate(() => window.__fandheTest.tickIntervalMs());

// 進行がちょうど n 段になった瞬間に、同じ同期区間で ops を順に click し、click ごとの後の書き出し（afters）を返す。
// before は click の前の書き出し、addr はその時点の次の進行の宛先。
// ops の要素は `{ op }`（その Op を送る）か `{ useNextTick: true }`（addr を `Op::Tick` として送る）。
// n 段を飛び越えた場合・上限時間を超えた場合は、ページの中でエラーにして失敗にする。
async function actWhenTicks(page, n, ops, timeout = 8000) {
  const res = await page.evaluate(([sel, count, list, limit]) => new Promise((resolve) => {
    const el = document.querySelector(sel);
    const snap = () => JSON.parse(window.__fandheTest.stateJson()).engine;
    const t0 = performance.now();
    const poll = () => {
      const e = snap();
      const len = e.ticks.length;
      if (len === count) {
        const addr = JSON.parse(window.__fandheTest.nextTick());
        const afters = [];
        for (const item of list) {
          const op = item.useNextTick ? { tick: addr } : item.op;
          el.setAttribute('data-payload', JSON.stringify(op));
          el.click();
          afters.push(snap());
        }
        resolve({ before: e, addr, afters });
        return;
      }
      if (len > count) {
        resolve({ error: `ticks skipped past ${count}: ${JSON.stringify(e)}` });
        return;
      }
      if (performance.now() - t0 > limit) {
        resolve({ error: `timeout waiting for ${count} ticks: ${JSON.stringify(e)}` });
        return;
      }
      setTimeout(poll, 2);
    };
    poll();
  }), [RAW, n, ops, timeout]);
  assert.ok(!res.error, res.error);
  return res;
}

// 条件が真になるまで待つ。上限を超えたら、最後の状態を添えて失敗にする。
async function waitUntil(page, what, pred, timeout = 8000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await engine(page);
    if (pred(last)) return last;
    await page.waitForTimeout(25);
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

const kindCount = (e, kind, step) => e.log.filter((r) => r.kind === kind && (step === undefined || r.step === step)).length;
const ticksOf = (e, g) => e.ticks.filter(([gen]) => gen === g).map(([, i]) => i);
const ignoredCount = (e) => e.ignoredReasons.length;
const stateOf = (e) => e.stateWithoutLog;
const paths = (e) => e.artifacts.map((a) => a.path);

async function openTestPage(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

async function assertSafe(page, rec) {
  const counts = await collectSafety(page, rec);
  assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
}

function assertConsistent(e) {
  assert.equal(e.incrementalMatches, true);
  assert.equal(e.worldMatchesState, true);
}

// 進行 3 段で保存し、4 段で restoreOps（'restore' は復帰の Op）を送る。
// 復帰の ID は保存の直前の next_checkpoint から読み、保存後の checkpoints に在ることを確かめる。
async function saveThenRestore(page, restoreOps) {
  const { before: b0, afters: [saved] } = await actWhenTicks(page, 3, [{ op: SAVE }]);
  const id = b0.stateWithoutLog.next_checkpoint;
  assert.ok(Object.hasOwn(saved.stateWithoutLog.checkpoints, String(id)), `checkpoint ${id} が保存されていない`);
  const ops = restoreOps.map((o) => (o === 'restore' ? { op: restoreOp(id) } : o));
  const res = await actWhenTicks(page, 4, ops);
  return { saved, ...res };
}

test('B-6 復帰で世代が 2 になり、保存時の状態へ戻り、復帰前の進行は状態を変えない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    const wait = (await interval(page)) * 2 + 100;
    await sendRaw(page, COMMAND);
    const { saved, addr, afters: [restored, injected] } = await saveThenRestore(page, ['restore', { useNextTick: true }]);
    assert.equal(kindCount(saved, 'checkpoint_saved'), 1);
    assert.equal(saved.generation, 1);
    assert.equal(stateOf(saved).run.busy.done, 3);
    assert.deepEqual(addr, { generation: 1, step: ST1, index: 4 });

    assert.equal(restored.generation, 2);
    assert.equal(restored.current, ST1);
    assert.equal(kindCount(restored, 'restored'), 1);
    assert.equal(stateOf(restored).run.busy.done, 3);
    // 世代だけが違い、実行状態と入口は保存時と一致する（checkpoints・next_checkpoint は戻らない仕様）。
    const noGen = (run) => ({ ...run, generation: undefined });
    assert.deepEqual(noGen(stateOf(restored).run), noGen(stateOf(saved).run));
    assert.deepEqual(stateOf(restored).entry, stateOf(saved).entry);
    assert.deepEqual(paths(restored), paths(saved)); // 差分の印（mark）は表示の状態で、復帰で変わりうる
    assert.deepEqual(restored.review, saved.review);

    // 復帰前の宛先の進行は、状態の JSON を変えず、無視 1 件だけを足す。
    assert.deepEqual(stateOf(injected), stateOf(restored));
    assert.equal(injected.logLen - restored.logLen, 1);
    assert.equal(injected.log.at(-1).kind, 'ignored');
    const last = injected.ignoredReasons.at(-1);
    assert.deepEqual(
      { generation: last.generation, step: last.step, reason: last.reason },
      { generation: 2, step: ST1, reason: 'stale' },
    );
    assert.deepEqual(injected.ticks, restored.ticks);
    assert.deepEqual(await nextTick(page), { generation: 2, step: ST1, index: 3 });

    // 自然に届く予約済みの古いタイマー（世代 1・番号 4）も無視され、世代 1 の進行は増えない。
    await page.waitForTimeout(wait);
    const later = await engine(page);
    assert.equal(ignoredCount(later) - ignoredCount(restored), 2, '注入 1 件と自然 1 件の無視になっていない');
    assert.deepEqual(ticksOf(later, 1), [0, 1, 2, 3]);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('B-6r 実タイマーのまま復帰しても、進行は（世代, 番号）ごとに 1 回で、受付済みで止まる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    const wait = (await interval(page)) * 2 + 100;
    await sendRaw(page, COMMAND);
    const { afters: [restored] } = await saveThenRestore(page, ['restore']);
    assert.equal(restored.generation, 2);
    await waitUntil(page, 'generation 2 ticks and idle', (e) => ticksOf(e, 2).length >= 2 && e.stateWithoutLog.run.busy === null);
    await page.waitForTimeout(wait);
    const done = await engine(page);
    assert.deepEqual(done.ticks.map(([g, i]) => [g, i]), [[1, 0], [1, 1], [1, 2], [1, 3], [2, 3], [2, 4]]);
    assert.deepEqual(ticksOf(done, 2), [3, 4]);
    assert.equal(ignoredCount(done), 1);
    assert.equal(done.ignoredReasons.at(-1).reason, 'stale');
    assert.equal(done.ignoredReasons.at(-1).generation, 2);
    assert.equal(stateOf(done).run.busy, null);
    assert.equal(stateOf(done).run.slices.main.status[ST1], 'accepted');
    assert.equal(kindCount(done, 'accepted'), 1);
    assert.equal(kindCount(done, 'achieved'), 0);
    assert.equal(done.generation, 2);
    assert.equal(await nextTick(page), null);
    assert.deepEqual(paths(done), ['notes/draft']);
    assertConsistent(done);
    await assertSafe(page, rec);
    await context.close();
  });
});

// 送った後の増分が、入力の受け取りと拒否（not_in_this_step）だけで、世代・状態が変わらないことを確かめる。
async function assertRejectedCancel(page, op) {
  const a = await engine(page);
  await sendRaw(page, op);
  const b = await engine(page);
  assert.equal(b.generation, 1);
  assert.equal(b.generation, a.generation);
  assert.equal(b.logLen - a.logLen, 2);
  assert.deepEqual(b.log.slice(a.log.length).map((r) => r.kind), ['input_received', 'judged']);
  assert.equal(b.judgedCategories.length - a.judgedCategories.length, 1);
  assert.equal(b.judgedCategories.at(-1).category, 'not_in_this_step');
  assert.equal(kindCount(b, 'stopped'), 0);
  assert.deepEqual(stateOf(b), stateOf(a));
  assert.deepEqual(b.ticks, []);
  assert.equal(await nextTick(page), null);
}

test('K-10 処理中でない停止は取り消しの入力になり、世代は変わらない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    await assertRejectedCancel(page, STOP);
    // 取り消しの入力そのもの（処理中でない Esc が作る Op）も同じ扱いになる。
    await assertRejectedCancel(page, CANCEL);
    // 世代が上がっていないので、通常の入力は世代 1 の宛先で進む。
    await sendRaw(page, COMMAND);
    const e = await engine(page);
    assert.equal(kindCount(e, 'accepted'), 1);
    assert.deepEqual(await nextTick(page), { generation: 1, step: ST1, index: 0 });
    const done = await waitUntil(page, 'all ticks', (x) => x.ticks.length >= TOTAL_TICKS);
    assertConsistent(done);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('配信設定には試験の口も、入力を届ける隠し要素も無い', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    assert.equal(await page.locator(RAW).count(), 0);
    await context.close();
  });
});
