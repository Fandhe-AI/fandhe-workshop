// Chrome 以外のブラウザへの「一度だけ」の案内のブラウザ試験（TASK-544。SHELL-30・D-74）。
//
// `static/app.js` が渡す `navigator.userAgent` を Rust 側が判定し、確かめた Chrome（主版 152 以上。`HeadlessChrome` を含む）以外のときだけ
// 枠の通知欄に案内を 1 件出す。UA は Playwright の `newContext({ userAgent })`（`openPage` の `userAgent`）で与え、配信物には差し替えの経路を持たない。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい context を開く）:
//   - Edge・macOS の Safari・Firefox・旧版の Chrome（120）では、締め出さずに開き（入力欄が使える）案内が 1 件出る
//   - Windows・macOS の Chrome 153 と UA を指定しない既定の HeadlessChrome では案内が 0 件
//   - 閉じると 0 件になり、root は同じ要素のまま。再読み込みの後も 0 件（同じオリジンの閉じた記録）
//   - 閉じずに再読み込みすれば再び 1 件。別の context（記録なし）では 1 件
//   - 配信する設定（IR 未同梱の枠）でも出て閉じられる
//   - 外部への要求・CSP 違反・ページのエラー・ダイアログ・ポップアップが 0 件
//
// 確かめないこと: 実機の Edge・Safari・Firefox での確認（UA の差し替えによる代理）。
// 失敗の文には案件名・件数・真偽だけを出し、UA の全文・保存の値を出さない。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-browser-notice/';
const ITEM = '[data-testid="shell-notice-item"][data-notice-kind="browser-notice"]';
const DISMISS = `${ITEM} [data-testid="shell-dismiss"]`;
const TEXT = `${ITEM} [data-testid="shell-notice-text"]`;
const INPUT = '[data-testid="draft-input"]';
const ROOT = '[data-testid="workshop-root"]';
const KEY = 'fandhe-workshop/meta/browser-notice';
const MESSAGE = 'この教材は Chrome の最新版で確かめています';

const CHROME = 'AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36';
const UA = {
  edge: `Mozilla/5.0 (Windows NT 10.0; Win64; x64) ${CHROME} Edg/153.0.0.0`,
  safari: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15',
  firefox: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0',
  old: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
  winChrome: `Mozilla/5.0 (Windows NT 10.0; Win64; x64) ${CHROME}`,
  macChrome: `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) ${CHROME}`,
};

let browser;

before(async () => {
  const test = process.env.FANDHE_WEB_TEST_DIR;
  const web = process.env.FANDHE_WEB_DIR;
  assert.ok(test && fs.statSync(test, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  assert.ok(web && fs.statSync(web, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to a web directory built without TEST_HOOKS');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function open(srv, userAgent) {
  const opened = await openPage(browser, { origin: srv.origin, userAgent });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

async function safe(page, rec, name) {
  const counts = await collectSafety(page, rec);
  assert.deepEqual(nonZero(counts), [], name);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  for (const [name, ua] of [['Edge', UA.edge], ['Safari', UA.safari], ['Firefox', UA.firefox], ['旧版の Chrome', UA.old]]) {
    test(`${name} では締め出さずに案内が 1 件出る（CSP ${label}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv, ua);
        assert.equal(await page.locator(ITEM).count(), 1, name);
        assert.equal(await page.locator(TEXT).textContent(), MESSAGE);
        assert.equal(await page.locator(INPUT).isEnabled(), true, name);
        await safe(page, rec, name);
        await context.close();
      });
    });
  }

  for (const [name, ua] of [['Windows の Chrome 153', UA.winChrome], ['macOS の Chrome 153', UA.macChrome], ['既定の HeadlessChrome', undefined]]) {
    test(`${name} では案内が出ない（CSP ${label}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv, ua);
        assert.equal(await page.locator(ITEM).count(), 0, name);
        await safe(page, rec, name);
        await context.close();
      });
    });
  }

  test(`閉じると消え、再読み込みでも出ず、閉じずに再読み込みすると再び出る（CSP ${label}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv, UA.edge);
      assert.equal(await page.locator(ITEM).count(), 1);
      // 閉じずに再読み込み: 再び 1 件。
      await page.reload();
      await waitMounted(page);
      assert.equal(await page.locator(ITEM).count(), 1);
      const root = await page.locator(ROOT).elementHandle();
      await page.locator(DISMISS).click();
      assert.equal(await page.locator(ITEM).count(), 0);
      assert.equal(await page.evaluate((r) => document.querySelector(r) !== null, ROOT), true);
      assert.equal(await root.evaluate((el) => el.isConnected), true, 'root が置き換わった');
      assert.equal(await page.evaluate((k) => localStorage.getItem(k), KEY), 'dismissed');
      // 入力しても再表示されない。
      await page.locator(INPUT).fill('x');
      assert.equal(await page.locator(ITEM).count(), 0);
      await page.reload();
      await waitMounted(page);
      assert.equal(await page.locator(ITEM).count(), 0);
      // 同じオリジンへ開き直しても出ない。
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal(await page.locator(ITEM).count(), 0);
      await safe(page, rec, 'dismiss');
      await context.close();
      // 新しい context（記録なし）では出る。
      const fresh = await open(srv, UA.edge);
      assert.equal(await fresh.page.locator(ITEM).count(), 1);
      await fresh.context.close();
    });
  });

  test(`幅 360px の成果物のタブでも案内が見える（CSP ${label}）`, async () => {
    await run(async (srv) => {
      const opened = await openPage(browser, { origin: srv.origin, userAgent: UA.safari, viewport: { width: 360, height: 800 } });
      await opened.page.goto(srv.url);
      await waitMounted(opened.page);
      await opened.page.getByRole('tab').nth(1).click();
      assert.equal(await opened.page.locator(ITEM).isVisible(), true);
      await opened.context.close();
    });
  });
}

test('配信する設定でも出て閉じられる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, UA.firefox);
    assert.equal(await page.locator(ITEM).count(), 1);
    await page.locator(DISMISS).click();
    assert.equal(await page.locator(ITEM).count(), 0);
    await safe(page, rec, 'delivery');
    await context.close();
  });
});
