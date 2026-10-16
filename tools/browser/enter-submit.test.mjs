// Enter による入力欄の送信の試験（TASK-275 1/2。JUDGE-12・FEND-14）。
//
// 修飾なしの Enter は、送信ボタンの click と同じ入力・同じ記録になる。変換中・修飾キー付き・Shift 付きでは送信しない。
// 配信設定の画面で、Playwright の信頼入力（page.keyboard.press）を使って確かめる。
// 本格的な IME の模擬（CDP・試験文 30 文）は TASK-276、複数行の送信と Shift+Enter の改行は TASK-275 2/2 の担当で、
// ここでは変換中の Enter が送信にならないことの最小の確認だけを持つ。
//
// 入力: FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。入力は試験の中で組み立てる。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-enter-submit/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const ITEMS = '[data-testid="log-list"] > li';
// 改行は含めない（複数行は TASK-275 2/2）。
const CASES = ['hello', '日本語の文', '# 見出し [x](https://example.invalid/a) <b>t</b>'];

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(
    dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(),
    'FANDHE_WEB_DIR must point to a web directory built without TEST_HOOKS',
  );
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const lines = (page) => page.$$eval(ITEMS, (lis) => lis.map((li) => li.textContent));

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

async function sendAll(page, how) {
  for (let i = 0; i < CASES.length; i += 1) {
    await page.fill(INPUT, CASES[i]);
    if (how === 'click') await page.click(SUBMIT);
    else await page.keyboard.press('Enter');
    await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [ITEMS, i + 1]);
  }
}

for (const csp of [true, false]) {
  test(`Enter の送信は送信ボタンの click と同じ記録になる（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const a = await open(srv);
      await sendAll(a.page, 'click');
      const b = await open(srv);
      await b.page.focus(INPUT);
      await sendAll(b.page, 'enter');
      const viaClick = await lines(a.page);
      const viaEnter = await lines(b.page);
      assert.deepEqual(viaClick, CASES);
      assert.deepEqual(viaEnter, viaClick);
      // preventDefault が効き、欄に改行が入らない。
      assert.equal(await b.page.inputValue(INPUT), CASES[CASES.length - 1]);
      for (const { rec } of [a, b]) {
        const s = summarize(rec);
        const detail = JSON.stringify(rec);
        assert.equal(s.csp, 0, detail);
        assert.equal(s.pageErrors, 0, detail);
        assert.equal(s.consoleErrors, 0, detail);
      }
      await a.context.close();
      await b.context.close();
    }));
  });
}

test('下書きが空のとき、Enter でも click でも一覧は増えない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.focus(INPUT);
    await page.keyboard.press('Enter');
    await page.click(SUBMIT);
    assert.deepEqual(await lines(page), []);
    await context.close();
  });
});

test('Shift・Control・Alt・Meta 付きの Enter では送信しない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.fill(INPUT, 'x');
    await page.focus(INPUT);
    for (const mod of ['Shift', 'Control', 'Alt', 'Meta']) {
      await page.keyboard.press(`${mod}+Enter`);
    }
    assert.deepEqual(await lines(page), []);
    await context.close();
  });
});

test('押しっぱなしの自動リピート（repeat）の Enter では送信しない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.fill(INPUT, 'x');
    // dispatchEvent は preventDefault されると false を返す。リピートでも改行は抑止される。
    const notPrevented = await page.evaluate((sel) => {
      const el = document.querySelector(sel);
      const make = () => new KeyboardEvent('keydown', { key: 'Enter', repeat: true, bubbles: true, cancelable: true });
      el.dispatchEvent(make());
      el.dispatchEvent(make());
      return el.dispatchEvent(make());
    }, INPUT);
    assert.equal(notPrevented, false);
    assert.deepEqual(await lines(page), []);
    // 最初の押下（repeat なし）は 1 回だけ送信される。
    await page.focus(INPUT);
    await page.keyboard.press('Enter');
    await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, ITEMS);
    assert.deepEqual(await lines(page), ['x']);
    await context.close();
  });
});

test('変換中の Enter（isComposing・Process）では送信しない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.fill(INPUT, 'x');
    await page.evaluate((sel) => {
      const el = document.querySelector(sel);
      el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', isComposing: true, bubbles: true, cancelable: true }));
      el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Process', bubbles: true, cancelable: true }));
    }, INPUT);
    assert.deepEqual(await lines(page), []);
    await context.close();
  });
});
