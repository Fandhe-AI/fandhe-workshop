// Step の状態表示（step-status）の試験（TASK-435 1/2。TERM-31）。3 種の確認ごとの試験は TASK-435 2/2 の持ち分。
//
// 試験の設定（TEST_HOOKS=1）の画面で、見えない `raw-trigger` から Op を送る（run-status.test.mjs と同じ経路）。
// 確かめること:
//   (1) 開始直後は空で、engine 側の導出（stateJson の stepStatus）は null
//   (2) 入力の送信の後に「受付済み（確認待ち）」になり、5 段の進行の後も、確認の前は「達成」が DOM の変化の記録に 0 件
//   (3) 確認を省く Step（confirm: none）では、受付と同時に「達成」が出る（受付済みの語は出ない）
//   (4) 配信設定には試験の口が無く、step-status は空である。外部リクエスト・エラー・CSP 違反が 0 件
// 待ちは固定の sleep ではなく、上限つきのポーリングで書く。1 回ごとに新しい画面を開く。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-step-status/';
const RAW = '[data-testid="raw-trigger"]';
const STATUS = '[data-testid="step-status"]';
const MAKE_NOTE = { input: { command: '/make-note first-note' } };
const GO_ST2 = { control: { go_to: 'main/st-2' } };
const CHOOSE = { input: { choice: 'opt-memo' } };
const TOTAL_TICKS = 5;
const ACCEPTED = '受付済み（確認待ち）';
const ACHIEVED = '達成';

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
const kindCount = (e, kind, step) => e.log.filter((r) => r.kind === kind && (step === undefined || r.step === step)).length;

const statusOf = (page) => page.evaluate((sel) => {
  const els = document.querySelectorAll(sel);
  return { count: els.length, word: els[0]?.textContent ?? null };
}, STATUS);

// step-status の文字の変化をすべて記録する（一瞬だけ出る「達成」を取りこぼさないため）。
const observe = (page) => page.evaluate((sel) => {
  const el = document.querySelector(sel);
  window.__stepStatusSeen = [el.textContent];
  new MutationObserver(() => window.__stepStatusSeen.push(document.querySelector(sel).textContent))
    .observe(el, { characterData: true, childList: true, subtree: true });
}, STATUS);
const seen = (page) => page.evaluate(() => window.__stepStatusSeen);
const clearSeen = (page) => page.evaluate(() => { window.__stepStatusSeen = []; });

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

async function openTestPage(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.emulateMedia({ reducedMotion: 'no-preference' });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

test('送信の後に受付済みになり、確認の前は達成が一度も出ない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    assert.deepEqual(await statusOf(page), { count: 1, word: '' });
    assert.equal((await engine(page)).stepStatus, null);
    await observe(page);

    await sendRaw(page, MAKE_NOTE);
    await waitUntil('accepted', () => statusOf(page), (s) => s.word === ACCEPTED);
    await waitUntil('all ticks', () => engine(page), (e) => e.ticks.length >= TOTAL_TICKS);
    // 最後の進行の後に、実タイマーの間隔の数倍だけ待っても受付済みのまま。
    await new Promise((r) => setTimeout(r, 400));
    assert.equal((await statusOf(page)).word, ACCEPTED);
    const e = await engine(page);
    assert.equal(e.stepStatus, 'accepted');
    assert.equal(kindCount(e, 'achieved'), 0, '確認の前に達成が出ている');
    assert.ok(!(await seen(page)).includes(ACHIEVED), JSON.stringify(await seen(page)));

    const safety = await collectSafety(page, rec);
    assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
    await context.close();
  });
});

test('確認を省く Step は受付と同時に達成が出る', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    await sendRaw(page, GO_ST2);
    await waitUntil('st-2', () => engine(page), (e) => e.current === 'main/st-2');
    assert.deepEqual(await statusOf(page), { count: 1, word: '' });
    await observe(page);
    await clearSeen(page);

    await sendRaw(page, CHOOSE);
    await waitUntil('achieved', () => statusOf(page), (s) => s.word === ACHIEVED);
    const e = await engine(page);
    assert.equal(e.current, 'main/st-3');
    assert.equal(e.stepStatus, 'achieved');
    assert.equal(kindCount(e, 'accepted', 'main/st-2'), 1);
    assert.equal(kindCount(e, 'achieved', 'main/st-2'), 1);
    assert.ok(!(await seen(page)).includes(ACCEPTED), JSON.stringify(await seen(page)));

    const safety = await collectSafety(page, rec);
    assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
    await context.close();
  });
});

test('配信設定には試験の口が無く、step-status は空である', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    assert.equal(await page.locator(RAW).count(), 0);
    assert.deepEqual(await statusOf(page), { count: 1, word: '' });
    await context.close();
  });
});
