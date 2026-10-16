// 進行（tick）と補完（complete）を Runtime の口で届ける結線の試験（TASK-349 1/2。FEND-4）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、見えない `raw-trigger` から入力を 1 件送ると処理中の Step になる。
// 以降の進行は、表示側の実タイマー（300ms）が `dispatch_action("tick", 宛先)` で届ける。手で進行を送らない。
// 確かめること:
//   (1) 進行が（世代, 番号）ごとにちょうど 1 回ずつ効き、処理が完了する。予約の遅延はすべて進行の間隔である
//   (2) 停止の後に届く予約済みの古いタイマーは無視され（記録 1 件）、状態は進まない
//   (3) 停止の後に入力し直すと、新しい世代の進行が 1 回ずつ効く
//   (4) 補完は配送の口を通って届き、ログも進行も変えない
//   (5) CSP 違反・ページ内エラー・コンソールのエラーが 0 件。配信設定には試験の口が無い
// 待ちは固定の sleep ではなく、上限つきのポーリングで書く（停止の後の「何も増えない」確認だけは、
// 予約済みのタイマーが届くのに足りる時間を置く）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero, summarize,
} from './harness.mjs';

const BASE = '/fandhe-tick-dispatch/';
const RAW = '[data-testid="raw-trigger"]';
const INPUT = { input: { command: '/make-note first-note' } };
const STOP = { control: 'stop' };
const TOTAL_TICKS = 5;

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

const state = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson()));
const engine = async (page) => (await state(page)).engine;
const nextTick = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.nextTick()));

// 条件が真になるまで待つ。上限を超えたら、最後の状態を添えて失敗にする。
async function waitUntil(page, what, pred, timeout = 5000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await engine(page);
    if (await pred(last)) return last;
    await page.waitForTimeout(25);
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

const ticksOf = (e, generation) => e.ticks.filter(([g]) => g === generation);
const ignoredOf = (e) => e.log.filter((r) => r.kind === 'ignored').length;
const expectTicks = (generation) => Array.from({ length: TOTAL_TICKS }, (_, i) => [generation, i]);

async function openTestPage(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  // setTimeout の遅延を記録する（進行の予約がすべて進行の間隔であることの確認）。
  await opened.page.addInitScript(() => {
    const orig = window.setTimeout.bind(window);
    window.__delays = [];
    window.setTimeout = (fn, delay, ...rest) => {
      window.__delays.push(Number(delay) || 0);
      return orig(fn, delay, ...rest);
    };
  });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

test('入力の後の進行が実タイマーで 1 回ずつ届き、処理が完了する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    const interval = await page.evaluate(() => window.__fandheTest.tickIntervalMs());
    assert.equal(interval, 300);
    assert.equal(await nextTick(page), null, '開始直後は処理中でない');
    await sendRaw(page, INPUT);
    assert.deepEqual(await nextTick(page), { generation: 1, step: 'main/st-1', index: 0 });
    const done = await waitUntil(page, 'all ticks', (e) => ticksOf(e, 1).length >= TOTAL_TICKS, 8000);
    assert.deepEqual(done.ticks, expectTicks(1), '各（世代, 番号）がちょうど 1 回');
    assert.equal(ignoredOf(done), 0, '無視の記録が出ていない');
    assert.equal(done.incrementalMatches, true);
    assert.equal(done.worldMatchesState, true);
    assert.equal(await nextTick(page), null, '完了の後は予約が無い');
    // 完了の後も進行が増えないこと（重複の予約が無い）。
    await page.waitForTimeout(interval * 2);
    assert.deepEqual((await engine(page)).ticks, expectTicks(1));
    const delays = await page.evaluate(() => window.__delays.filter((d) => d > 0));
    assert.deepEqual(delays, Array(TOTAL_TICKS).fill(interval), JSON.stringify(delays));
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
    await context.close();
  });
});

test('停止の後に届く古いタイマーは無視され、入力し直すと新しい世代の進行が届く', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    const interval = await page.evaluate(() => window.__fandheTest.tickIntervalMs());
    await sendRaw(page, INPUT);
    await waitUntil(page, 'first tick', (e) => ticksOf(e, 1).length >= 1);
    await sendRaw(page, STOP);
    const stopped = await engine(page);
    assert.equal(stopped.generation, 2);
    assert.equal(await nextTick(page), null, '停止で予約の印が消える');
    const stoppedTicks = ticksOf(stopped, 1);
    const ignoredBefore = ignoredOf(stopped);
    // 予約済みの古いタイマーが届くのに足りる時間を置く。
    await page.waitForTimeout(interval * 2 + 100);
    const after = await engine(page);
    assert.deepEqual(ticksOf(after, 1), stoppedTicks, '世代 1 の進行が増えていない');
    assert.equal(ticksOf(after, 2).length, 0);
    assert.equal(ignoredOf(after) - ignoredBefore, 1, '古い宛先は無視の記録 1 件');
    assert.equal(after.generation, 2);
    assert.equal(after.current, stopped.current);
    assert.equal(await nextTick(page), null);

    await sendRaw(page, INPUT);
    assert.deepEqual(await nextTick(page), { generation: 2, step: 'main/st-1', index: 0 });
    const done = await waitUntil(page, 'generation 2 ticks', (e) => ticksOf(e, 2).length >= TOTAL_TICKS, 8000);
    assert.deepEqual(ticksOf(done, 2), expectTicks(2));
    assert.deepEqual(ticksOf(done, 1), stoppedTicks);
    assert.equal(done.incrementalMatches, true);
    assert.equal(done.worldMatchesState, true);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
    await context.close();
  });
});

test('complete は配送の口を通って届き、ログも進行も変えない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    assert.equal((await state(page)).completeCount, 0);
    const before = await engine(page);
    await page.evaluate(() => window.__fandheTest.requestComplete());
    const deadline = Date.now() + 5000;
    while ((await state(page)).completeCount !== 1 && Date.now() < deadline) {
      await page.waitForTimeout(25);
    }
    assert.equal((await state(page)).completeCount, 1);
    const after = await engine(page);
    assert.equal(after.logLen, before.logLen);
    assert.deepEqual(after.ticks, before.ticks);
    const s = summarize(rec);
    assert.equal(s.csp + s.pageErrors + s.consoleErrors, 0, JSON.stringify(rec));
    await context.close();
  });
});

test('配信設定には試験の口も、進行・補完を届ける隠し要素も無い', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    for (const id of ['tick-trigger', 'esc-trigger', 'complete-trigger', 'raw-trigger']) {
      assert.equal(await page.locator(`[data-testid="${id}"]`).count(), 0, id);
    }
    // 進行の宛先（tick・complete・esc）を data-action に持つ要素が無く、data-payload を持つ要素は指せる行（ui）だけ。
    for (const action of ['tick', 'complete', 'esc']) {
      assert.equal(await page.locator(`[data-action="${action}"]`).count(), 0, `data-action=${action}`);
    }
    const payloadActions = await page.$$eval('[data-payload]', (els) => els.map((e) => e.getAttribute('data-action')));
    assert.ok(payloadActions.every((a) => a === 'ui'), `data-payload outside ui: ${payloadActions}`);
    await context.close();
  });
});
