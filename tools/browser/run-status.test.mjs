// 状態表示（run-status）と処理中の段階の進行の試験（TASK-436 1/2。TERM-33）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、見えない `raw-trigger` から入力を 1 件送ると処理中の Step（5 段）になる
// （tick-dispatch.test.mjs と同じ経路。送信ボタンの結線は TASK-438 の持ち分）。以降の進行は実タイマーが届ける。
// 確かめること:
//   (1) 開始直後は「待機中」（data-state=idle）
//   (2) 入力の後に「処理中」（busy）になり、処理中に progress の行が増え、data-index は 0,1,2,… の順で data-total は一定
//   (3) 完了の後に「待機中」へ戻り、DOM の data-state と engine 側の導出（stateJson の runStatus）が一致する
//   (4) CSP 違反・ページ内エラー・コンソールのエラーが 0 件。配信設定には試験の口が無く、状態表示は「待機中」
// 文脈: prefers-reduced-motion は no-preference で流す。reduce の側（段階の省略）は TASK-372 の持ち分で、本件は前提にしない。
// 待ちは固定の sleep ではなく、上限つきのポーリングで書く。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-run-status/';
const RAW = '[data-testid="raw-trigger"]';
const STATUS = '[data-testid="run-status"]';
const PROGRESS = '[data-testid="part-log-list"] li[data-kind="progress"]';
const INPUT = { input: { command: '/make-note first-note' } };
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

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;

const statusOf = (page) => page.evaluate((sel) => {
  const els = document.querySelectorAll(sel);
  const el = els[0];
  return { count: els.length, state: el?.getAttribute('data-state') ?? null, word: el?.textContent ?? null };
}, STATUS);

const progressRows = (page) => page.$$eval(PROGRESS, (lis) => lis.map((li) => ({
  index: li.getAttribute('data-index'), total: li.getAttribute('data-total'),
})));

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

test('入力の後に処理中になり、段階が進み、完了で待機中へ戻る', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await page.goto(srv.url);
    await waitMounted(page);

    assert.deepEqual(await statusOf(page), { count: 1, state: 'idle', word: '待機中' });
    assert.equal((await progressRows(page)).length, 0);

    await sendRaw(page, INPUT);
    await waitUntil('busy', () => statusOf(page), (s) => s.state === 'busy');
    assert.equal((await statusOf(page)).word, '処理中');

    // 処理中に段階の行が増える。観察した件数は単調に増え、途中の段階（0 < 件数 < 5）を処理中のまま少なくとも 1 回見る。
    const counts = [];
    let sawMidBusy = false;
    await waitUntil('all progress rows', async () => {
      const [rows, st] = await Promise.all([progressRows(page), statusOf(page)]);
      counts.push(rows.length);
      if (rows.length > 0 && rows.length < TOTAL_TICKS && st.state === 'busy') sawMidBusy = true;
      return rows;
    }, (rows) => rows.length >= TOTAL_TICKS);
    for (let i = 1; i < counts.length; i += 1) assert.ok(counts[i] >= counts[i - 1], JSON.stringify(counts));
    assert.ok(sawMidBusy, `処理中のまま段階が進む様子を観察できなかった: ${JSON.stringify(counts)}`);

    const rows = await progressRows(page);
    assert.deepEqual(rows.map((r) => r.index), Array.from({ length: TOTAL_TICKS }, (_, i) => String(i)));
    assert.ok(rows.every((r) => r.total === String(TOTAL_TICKS)), JSON.stringify(rows));
    const eng = await engine(page);
    assert.equal(eng.ticks.filter(([g]) => g === 1).length, rows.length, '進行の記録と行の数が一致');

    const done = await waitUntil('idle', () => statusOf(page), (s) => s.state === 'idle');
    assert.deepEqual(done, { count: 1, state: 'idle', word: '待機中' });
    assert.equal((await engine(page)).runStatus, 'idle');
    assert.equal((await progressRows(page)).length, TOTAL_TICKS, '戻った後も段階の行は残る');

    const safety = await collectSafety(page, rec);
    assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
    await context.close();
  });
});

test('配信設定には試験の口が無く、状態表示は待機中である', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    assert.equal(await page.locator(RAW).count(), 0);
    assert.deepEqual(await statusOf(page), { count: 1, state: 'idle', word: '待機中' });
    await context.close();
  });
});
