// 停止・移動・古い進行の無視の縦断試験（TASK-222 2/3。B-3・B-4・B-5。EVT-6・EVT-8・EVT-9・EVT-11）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、実タイマーの進行（表示側の 300ms）を流しながら、状態の不変条件を確かめる。
//   B-3 処理中（進行 2 段）の停止で、世代が 2 になり、処理中が解け、入口の状態へ戻る。停止の後も再び入力できる
//   B-4 処理中（進行 2 段）の移動で、世代が 2 になり、移動前に予約されたタイマーは状態を変えない
//   B-5 停止・移動の後に届く古い進行（古い世代・同じ番号の二重・移動前の Step 宛て）は無視され、状態は変わらない
//
// 停止・移動・古い進行は、見えない `raw-trigger` に `Op` の JSON を書いて click する経路で送る（confirm-burst と同じ）。
// Esc はまだエンジンに結線されておらず、画面の移動ボタンも無い。tick-trigger 型の隠し要素は FEND-4 が禁じるため、
// 元の PoC の「tick-trigger を書き換えて click」は raw-trigger による `Op::Tick` の注入で置き換える。
// 進行の間隔は 300ms である。spec の TASK-222 は 60ms と書くが、D-47・EVT-18 が後の決定でビルド時の定数 300ms にした。
// 実タイマーの進行は、別々の `page.evaluate` の間に割り込む。そこで「進行が n 段になったことの確認 → click → 書き出し」を
// 1 回の evaluate の同期区間の中で行い、停止と注入の間にタイマーが割り込めないようにする。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-stop-move-ignore/';
const RAW = '[data-testid="raw-trigger"]';
const COMMAND = { input: { command: '/make-note first-note' } };
const CHOICE = { input: { choice: 'opt-memo' } };
const STOP = { control: 'stop' };
const GO_ST2 = { control: { go_to: 'main/st-2' } };
const TOTAL_TICKS = 5;
const ST1 = 'main/st-1';
const NOTE_LI = '[data-testid="part-artifact-view"] li[data-path="notes/draft"]';

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

test('B-3 停止で世代が 2 になり、入口の状態へ戻り、再び送れる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    const wait = (await interval(page)) * 2 + 100;
    const initial = await engine(page);
    await sendRaw(page, COMMAND);
    const { afters: [stopped] } = await actWhenTicks(page, 2, [{ op: STOP }]);
    assert.equal(stopped.generation, 2);
    assert.equal(stopped.current, ST1);
    assert.equal(stateOf(stopped).run.busy, null);
    assert.equal(kindCount(stopped, 'stopped'), 1);
    assert.deepEqual(ticksOf(stopped, 1), [0, 1]);
    assert.deepEqual(paths(stopped), [], 'notes/draft が残っている');
    assert.deepEqual(stateOf(stopped).run.world, stateOf(initial).run.world, '入口の状態へ戻っていない');
    assert.equal(await nextTick(page), null);
    const deadline = Date.now() + 5000;
    while ((await page.locator(NOTE_LI).count()) !== 0) {
      assert.ok(Date.now() < deadline, '左の notes/draft が消えない');
      await page.waitForTimeout(25);
    }

    // 後から届く古いタイマーは、無視 1 件になり、状態は変わらない。
    await page.waitForTimeout(wait);
    const later = await engine(page);
    assert.equal(ignoredCount(later) - ignoredCount(stopped), 1);
    assert.equal(later.ignoredReasons.at(-1).reason, 'stale');
    assert.deepEqual(stateOf(later), stateOf(stopped));
    assert.deepEqual(ticksOf(later, 1), [0, 1]);

    // 停止の後でも再び入力できる。
    await sendRaw(page, COMMAND);
    assert.deepEqual(await nextTick(page), { generation: 2, step: ST1, index: 0 });
    const done = await waitUntil(page, 'all ticks of generation 2', (e) => ticksOf(e, 2).length >= TOTAL_TICKS);
    assert.equal(done.log.filter((r) => r.kind === 'accepted' && r.generation === 2).length, 1);
    assert.deepEqual(ticksOf(done, 2), [0, 1, 2, 3, 4]);
    assert.deepEqual(paths(done), ['notes/draft']);
    assertConsistent(done);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('B-4 処理中の移動で世代が 2 になり、移動前のタイマーは状態を変えない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    const wait = (await interval(page)) * 2 + 100;
    await sendRaw(page, COMMAND);
    const { afters: [moved] } = await actWhenTicks(page, 2, [{ op: GO_ST2 }]);
    assert.equal(moved.generation, 2);
    assert.equal(moved.current, 'main/st-2');
    assert.equal(stateOf(moved).run.busy, null);
    assert.equal(kindCount(moved, 'moved'), 1);
    assert.equal(kindCount(moved, 'entered', 'main/st-2'), 1);
    assert.deepEqual(ticksOf(moved, 1), [0, 1]);
    assert.equal(await nextTick(page), null);

    await page.waitForTimeout(wait);
    const later = await engine(page);
    assert.deepEqual(stateOf(later), stateOf(moved));
    assert.deepEqual(ticksOf(later, 1), [0, 1]);
    assert.equal(ignoredCount(later) - ignoredCount(moved), 1);
    assert.equal(later.ignoredReasons.at(-1).reason, 'stale');
    assert.equal(later.generation, 2);
    assert.equal(later.current, 'main/st-2');

    // 移動先が動く。
    await sendRaw(page, CHOICE);
    const e = await engine(page);
    assert.equal(kindCount(e, 'accepted', 'main/st-2'), 1);
    assert.equal(kindCount(e, 'achieved', 'main/st-2'), 1);
    assert.equal(e.current, 'main/st-3');
    assertConsistent(e);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('B-5 停止の直後に届いた停止前の世代の進行は無視され、状態の JSON は一致する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    const wait = (await interval(page)) * 2 + 100;
    await sendRaw(page, COMMAND);
    const { addr, afters: [a, b] } = await actWhenTicks(page, 2, [{ op: STOP }, { useNextTick: true }]);
    assert.deepEqual(addr, { generation: 1, step: ST1, index: 2 });
    assert.equal(a.generation, 2);
    assert.deepEqual(stateOf(b), stateOf(a));
    assert.equal(b.logLen - a.logLen, 1);
    assert.equal(b.log.at(-1).kind, 'ignored');
    assert.equal(ignoredCount(b) - ignoredCount(a), 1);
    const last = b.ignoredReasons.at(-1);
    assert.deepEqual(
      { generation: last.generation, step: last.step, reason: last.reason },
      { generation: 2, step: ST1, reason: 'stale' },
    );
    assert.deepEqual(b.ticks, a.ticks);

    // 自然に届く予約済みのタイマーも無視され、状態は変わらない。
    await page.waitForTimeout(wait);
    const later = await engine(page);
    assert.equal(ignoredCount(later), 2);
    assert.deepEqual(stateOf(later), stateOf(a));
    assert.deepEqual(ticksOf(later, 1), [0, 1]);
    assertConsistent(later);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('B-5 同じ番号の二重の届きは 1 回だけ進み、二重の分は無視になる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    await sendRaw(page, COMMAND);
    const dup = { tick: { generation: 1, step: ST1, index: 0 } };
    const { before: a, afters: [b] } = await actWhenTicks(page, 1, [{ op: dup }]);
    assert.deepEqual(stateOf(b), stateOf(a));
    assert.equal(ignoredCount(b) - ignoredCount(a), 1);
    assert.equal(b.ignoredReasons.at(-1).reason, 'stale');
    assert.deepEqual(b.ticks, a.ticks);

    const done = await waitUntil(page, 'all ticks', (e) => e.ticks.length >= TOTAL_TICKS);
    assert.deepEqual(done.ticks.map(([g, i]) => [g, i]), [[1, 0], [1, 1], [1, 2], [1, 3], [1, 4]]);
    assert.equal(ignoredCount(done), 1);
    assert.deepEqual(paths(done), ['notes/draft']);
    assertConsistent(done);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('B-5 移動前の Step 宛ての進行は無視される', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    await sendRaw(page, GO_ST2);
    const a = await engine(page);
    assert.equal(a.generation, 2);
    assert.equal(a.current, 'main/st-2');
    await sendRaw(page, { tick: { generation: 2, step: ST1, index: 0 } });
    const b = await engine(page);
    assert.deepEqual(stateOf(b), stateOf(a));
    assert.equal(ignoredCount(b) - ignoredCount(a), 1);
    assert.equal(b.ignoredReasons.at(-1).reason, 'stale');
    assertConsistent(b);
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
