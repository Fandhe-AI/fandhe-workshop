// 停止の操作（Esc・停止ボタン）と停止後のやり直しの試験（TASK-436 2/2。TERM-33）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、実 UI（欄への入力と送信ボタン）から処理中の Step（5 段）を始め、途中で
// Esc（欄にフォーカスして Escape）または停止ボタンの click で止める。確かめること:
//   (1) 状態表示が「停止」（data-state=stopped）になり、engine 側の導出（runStatus）と一致する
//   (2) 右のログに stopped の行が 1 件、世代が +1、run が入口の状態（世代を除く）へ戻り、成果物が送信前と一致する
//   (3) 古い世代の進行が Ignored として届いても「停止」が保たれる
//   (4) 同じ本文を再び送信でき、「処理中」→ 新しい世代の進行 → 「待機中」になる
//   (5) 配信設定には試験の口が無く、停止ボタンが 1 件あり、待機中に押しても何も起きない
// 文脈: prefers-reduced-motion は no-preference で流す。reduce の側は reduced-motion-st15.test.mjs（TASK-378 1/2）が確かめる。
// 待ちは固定の sleep ではなく、上限つきのポーリングで書く。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-stop-redo/';
const STATUS = '[data-testid="run-status"]';
const DRAFT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const STOP = '[data-testid="stop-btn"]';
const LOG = '[data-testid="part-log-list"]';
const PROGRESS = `${LOG} li[data-kind="progress"]`;
const STOPPED = `${LOG} li[data-kind="stopped"]`;
const BODY = '/make-note first-note';
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

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;

const statusOf = (page) => page.evaluate((sel) => {
  const els = document.querySelectorAll(sel);
  const el = els[0];
  return { count: els.length, state: el?.getAttribute('data-state') ?? null, word: el?.textContent ?? null };
}, STATUS);

async function waitUntil(what, read, pred, timeout = 8000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await read();
    if (await pred(last)) return last;
    await new Promise((r) => setTimeout(r, 20));
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

const withoutGeneration = (run) => {
  const { generation, ...rest } = run;
  return rest;
};

async function submit(page) {
  await page.fill(DRAFT, BODY);
  await page.click(SUBMIT);
}

for (const how of ['esc', 'button']) {
  test(`${how === 'esc' ? 'Esc' : '停止ボタン'}で停止し、入口の状態へ戻り、再び送信できる`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.emulateMedia({ reducedMotion: 'no-preference' });
      await page.goto(srv.url);
      await waitMounted(page);

      const before0 = await engine(page);
      await submit(page);
      await waitUntil('busy', () => statusOf(page), (s) => s.state === 'busy');
      // 段階の途中（1 件以上 TOTAL_TICKS 未満）まで進めてから止める。
      await waitUntil('mid progress', () => page.locator(PROGRESS).count(), (n) => n >= 1);
      assert.ok((await page.locator(PROGRESS).count()) < TOTAL_TICKS, '止める前に完了してしまった');

      if (how === 'esc') {
        await page.focus(DRAFT);
        await page.keyboard.press('Escape');
      } else {
        await page.click(STOP);
      }

      const st = await waitUntil('stopped', () => statusOf(page), (s) => s.state === 'stopped');
      assert.deepEqual(st, { count: 1, state: 'stopped', word: '停止' });
      const e = await engine(page);
      assert.equal(e.runStatus, 'stopped');
      assert.equal(await page.locator(STOPPED).count(), 1);
      assert.equal(e.generation, before0.generation + 1);
      assert.deepEqual(withoutGeneration(e.stateWithoutLog.run), withoutGeneration(before0.stateWithoutLog.run));
      assert.deepEqual(e.artifacts, before0.artifacts, '処理中の効果が取り消される');
      assert.equal(e.worldMatchesState, true);
      assert.equal(e.incrementalMatches, true);

      // 古い世代の進行が Ignored として届いても「停止」のまま。
      await waitUntil('stale tick', async () => (await engine(page)).ignoredReasons.length, (n) => n >= 1);
      assert.equal((await statusOf(page)).state, 'stopped');

      // 再送信。
      await submit(page);
      await waitUntil('busy again', () => statusOf(page), (s) => s.state === 'busy');
      const done = await waitUntil('idle', () => statusOf(page), (s) => s.state === 'idle');
      assert.deepEqual(done, { count: 1, state: 'idle', word: '待機中' });
      const e2 = await engine(page);
      assert.equal(e2.ticks.filter(([g]) => g === e.generation).length, TOTAL_TICKS);
      assert.equal(e2.runStatus, 'idle');

      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });
}

test('配信設定には試験の口が無く、停止ボタンは待機中に何も起こさない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    assert.equal(await page.locator('[data-testid="raw-trigger"]').count(), 0);
    assert.equal(await page.locator(STOP).count(), 1);
    await page.click(STOP);
    assert.deepEqual(await statusOf(page), { count: 1, state: 'idle', word: '待機中' });
    const safety = await collectSafety(page, rec);
    assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
    await context.close();
  });
});
