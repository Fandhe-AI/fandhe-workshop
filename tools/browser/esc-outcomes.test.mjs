// 変換中でない入力欄での Esc の画面の結果の試験（TASK-441 1/2。TERM-7。PoC-2 の K-09・K-11・K-14）。
//
// 試験の設定（FANDHE_WEB_TEST_DIR。教材は min-3step.json）で、右のログ・状態表示・候補の一覧が揃った画面を見る。
//   (1) 処理中なら、候補の一覧が開いていても Esc は停止になり、候補の一覧も閉じる（下書きは残る）
//   (2) 処理中でなく候補の一覧が開いていれば（待機中・停止の後）、Esc は一覧を閉じるだけで、停止もログの増加も起きない
// 範囲外: 取り消しの入力と、取り消しを受け付けない Step の拒否の行は TASK-441 2/2（#335）、Esc がちょうど 1 回届くことは
// TASK-353 の持ち分。このため各ケースは Esc を 1 回押すところで止め、閉じた後の Esc の結果は検査しない。
// 待ちは固定の sleep ではなく、上限つきのポーリングで書く。prefers-reduced-motion は no-preference。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-esc-outcomes/';
const STATUS = '[data-testid="run-status"]';
const DRAFT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const LIST = '[data-testid="candidate-list"]';
const BTNS = '[data-testid="candidate-btn"]';
const RAW = '[data-testid="raw-trigger"]';
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

const isHidden = (page) => page.locator(LIST).evaluate((el) => el.hasAttribute('hidden'));
const labels = (page) => page.locator(BTNS).allTextContents();

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

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.emulateMedia({ reducedMotion: 'no-preference' });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

// Esc の直前の前提: 処理中で、途中（段が残っている）で、候補の一覧が開いている。競合で完了していたらここで失敗する。
async function assertBusyWithCandidates(page) {
  assert.equal((await statusOf(page)).state, 'busy', 'Esc の前に処理中でない');
  assert.equal(await isHidden(page), false, 'Esc の前に候補が閉じている');
  assert.ok((await page.locator(PROGRESS).count()) < TOTAL_TICKS, 'Esc の前に完了してしまった');
}

async function assertStoppedAndClosed(page, before0, draft) {
  const st = await waitUntil('stopped', () => statusOf(page), (s) => s.state === 'stopped');
  assert.deepEqual(st, { count: 1, state: 'stopped', word: '停止' });
  const e = await engine(page);
  assert.equal(e.runStatus, 'stopped');
  assert.equal(e.generation, before0.generation + 1);
  assert.equal(await page.locator(STOPPED).count(), 1);
  assert.equal(await isHidden(page), true, '停止と同時に候補の一覧が閉じる');
  assert.equal(await page.locator(BTNS).count(), 0);
  assert.equal(await page.inputValue(DRAFT), draft, '下書きは残る');
  // 古い世代の進行が Ignored として届いても、「停止」のまま・候補は閉じたまま。
  await waitUntil('stale tick', async () => (await engine(page)).ignoredReasons.length, (n) => n >= 1);
  assert.equal((await statusOf(page)).state, 'stopped');
  assert.equal(await isHidden(page), true);
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  test(`処理中に候補が開いていても Esc は停止になり候補も閉じる（K-11・K-14。送信は raw。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const before0 = await engine(page);
      await page.fill(DRAFT, '/he');
      assert.equal(await isHidden(page), false);
      assert.deepEqual(await labels(page), ['/help']);
      // 下書きと候補を残したまま、処理中の入力だけを core へ送る。
      await sendRaw(page, { input: { command: BODY } });
      await waitUntil('busy', () => statusOf(page), (s) => s.state === 'busy');
      await assertBusyWithCandidates(page);
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      await assertStoppedAndClosed(page, before0, '/he');
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });

  test(`送信の後の処理中に /he を打って Esc を押すと停止になり候補も閉じる（K-11。実 UI。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const before0 = await engine(page);
      await page.fill(DRAFT, BODY);
      await page.click(SUBMIT);
      await waitUntil('busy', () => statusOf(page), (s) => s.state === 'busy');
      await page.fill(DRAFT, '/he');
      await assertBusyWithCandidates(page);
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      await assertStoppedAndClosed(page, before0, '/he');
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });

  test(`待機中に候補が開いているときの Esc は閉じるだけで、状態もログも変えない（K-09。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const before0 = await engine(page);
      await page.fill(DRAFT, '/he');
      assert.equal(await isHidden(page), false);
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      assert.equal(await isHidden(page), true);
      assert.deepEqual(await statusOf(page), { count: 1, state: 'idle', word: '待機中' });
      const e = await engine(page);
      assert.equal(e.logLen, before0.logLen);
      assert.equal(e.generation, before0.generation);
      assert.equal(e.runStatus, 'idle');
      assert.equal(await page.locator(STOPPED).count(), 0);
      assert.equal(await page.inputValue(DRAFT), '/he');
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });

  test(`停止の後に候補が開いているときの Esc は閉じるだけで、停止を重ねない（K-09。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const before0 = await engine(page);
      await page.fill(DRAFT, '/he');
      await sendRaw(page, { input: { command: BODY } });
      await waitUntil('busy', () => statusOf(page), (s) => s.state === 'busy');
      await assertBusyWithCandidates(page);
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      await assertStoppedAndClosed(page, before0, '/he');
      // 下書きを変えて候補を再び開き、もう一度 Esc。処理中ではないので閉じるだけ。
      await page.fill(DRAFT, '/hel');
      assert.equal(await isHidden(page), false);
      const e1 = await engine(page);
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      assert.equal(await isHidden(page), true);
      assert.deepEqual(await statusOf(page), { count: 1, state: 'stopped', word: '停止' });
      const e2 = await engine(page);
      assert.equal(e2.logLen, e1.logLen);
      assert.equal(e2.generation, e1.generation);
      assert.equal(await page.locator(STOPPED).count(), 1);
      assert.equal(await page.inputValue(DRAFT), '/hel');
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });
}
