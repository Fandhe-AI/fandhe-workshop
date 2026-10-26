// prefers-reduced-motion: reduce で段階の表示を省く試験（TASK-372 1/2。A11Y-11・EVT-19）。
//
// reduce の文脈で処理中の Step（5 段）を流し、右のログの段階の行（li[data-kind=progress]）と progress 要素が最初から最後まで
// 0 件であること、進行そのものは起きている（engine.ticks が 5 件）こと、結果の行と状態の語は通常どおり出ることを確かめる。
// 設定は読み込み時に 1 回読むため、emulateMedia は goto より前に呼ぶ。対照として no-preference では段階の行が 5 件出る。
// no-preference の詳細とアニメーション・transition・逐次表示の 0 件の検査は TASK-372 2/2 の持ち分。
// 待ちは固定の sleep ではなく、上限つきのポーリングで書く。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-reduced-motion/';
const RAW = '[data-testid="raw-trigger"]';
const STATUS = '[data-testid="run-status"]';
const PROGRESS_LI = '[data-testid="part-log-list"] li[data-kind="progress"]';
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

const sample = (page) => page.evaluate(({ status, li }) => ({
  state: document.querySelector(status)?.getAttribute('data-state') ?? null,
  progressRows: document.querySelectorAll(li).length,
  progressEls: document.querySelectorAll('progress').length,
  received: document.querySelectorAll('[data-testid="part-log-list"] li[data-kind="received"]').length,
}), { status: STATUS, li: PROGRESS_LI });

async function waitUntil(what, read, pred, timeout = 10000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await read();
    if (await pred(last)) return last;
    await new Promise((r) => setTimeout(r, 20));
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

// ページ内で progress の行・要素が足されたことを数える。ポーリングの間に一瞬だけ現れて消える分も拾う。
const WATCH = () => {
  window.__rmAdded = 0;
  const isProgress = (n) => n.nodeType === 1
    && (n.matches('li[data-kind="progress"], progress') || n.querySelector('li[data-kind="progress"], progress'));
  new MutationObserver((ms) => {
    for (const m of ms) for (const n of m.addedNodes) if (isProgress(n)) window.__rmAdded += 1;
  }).observe(document.documentElement, { childList: true, subtree: true });
};

for (const csp of [true, false]) {
  const mode = csp ? 'CSP あり' : 'CSP なし';

  test(`reduce では処理中でも段階の行が 0 件で、結果と状態は通常どおり出る（${mode}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.emulateMedia({ reducedMotion: 'reduce' });
      await page.goto(srv.url);
      await waitMounted(page);
      await page.evaluate(WATCH);
      assert.equal((await engine(page)).reducedMotion, true);

      await sendRaw(page, INPUT);
      const samples = [];
      let sawBusy = false;
      await waitUntil('busy then idle', async () => {
        const s = await sample(page);
        samples.push(s);
        if (s.state === 'busy') sawBusy = true;
        return s;
      }, (s) => sawBusy && s.state === 'idle');
      assert.ok(sawBusy, '処理中を観察できなかった');
      assert.ok(samples.every((s) => s.progressRows === 0 && s.progressEls === 0), JSON.stringify(samples));
      assert.equal(await page.evaluate(() => window.__rmAdded), 0, '段階の行が一瞬でも足された');

      const eng = await engine(page);
      assert.equal(eng.ticks.filter(([g]) => g === 1).length, TOTAL_TICKS, '進行は起き、表示だけを省く');
      assert.equal(eng.runStatus, 'idle');
      assert.equal(eng.log.filter((r) => r.line === 'progress').length, 0);
      assert.ok(eng.log.some((r) => r.line === 'received'), '受付の行が出ている');
      assert.ok((await sample(page)).received >= 1, '結果の行が DOM に出ている');

      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    }));
  });

  test(`対照: no-preference では同じ操作で段階の行が 5 件出る（${mode}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context } = await openPage(browser, { origin: srv.origin });
      await page.emulateMedia({ reducedMotion: 'no-preference' });
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal((await engine(page)).reducedMotion, false);
      await sendRaw(page, INPUT);
      const done = await waitUntil('idle with rows', () => sample(page), (s) => s.progressRows >= TOTAL_TICKS && s.state === 'idle');
      assert.equal(done.progressRows, TOTAL_TICKS);
      await context.close();
    }));
  });
}

test('配信設定でも reduce で開いてマウントでき、例外・CSP 違反が 0 件である', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    const safety = await collectSafety(page, rec);
    assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
    await context.close();
  });
});
