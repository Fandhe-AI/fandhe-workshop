// 推定した OS ごとの修飾キーの表示のブラウザ試験（TASK-285 1/2。JUDGE-28・D-48。TERM-38 の案内の一部）。
//
// `static/app.js` が `navigator.userAgent` を `mount_app` へ渡し、Rust 側がその文字列から OS を推定して
// 入力欄の下の案内（`[data-testid="key-hint"]`）に貼り付けのキーを出す。User-Agent は Playwright の
// `newContext({ userAgent })`（`openPage` の `userAgent`）で与え、配信物には差し替えの経路を持たない。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい context を開く）:
//   - Windows の UA は「貼り付け: Ctrl+V」、macOS の UA は「貼り付け: Cmd+V」、Linux の UA は空（推定できない）
//   - 入力欄なしの教材（`?fixture=no-input`）では案内が 0 件（入力欄の判定の下にあること）
//   - 案内は role・aria を持たない。危険な要素・実行・CSP 違反・ダイアログが 0 件
//
// 確かめないこと:
//   - 受講者が選ぶ欄と選んだときの切り替え（TASK-285 2/2）。Windows の実機・人の目での確認（未実施）
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-modifier-key-hint/';
const HINT = '[data-testid="key-hint"]';

const UA = {
  windows: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  mac: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  linux: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
};

const CASES = [
  { name: 'Windows', ua: UA.windows, text: '貼り付け: Ctrl+V' },
  { name: 'macOS', ua: UA.mac, text: '貼り付け: Cmd+V' },
  { name: 'Linux（推定できない）', ua: UA.linux, text: '' },
];

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  for (const c of CASES) {
    test(`${c.name} の UA では修飾キーの案内が合う（CSP ${label}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin, userAgent: c.ua });
        await page.goto(srv.url);
        await waitMounted(page);
        assert.equal(await page.locator(HINT).count(), 1);
        assert.equal(await page.locator(HINT).textContent(), c.text);
        const attrs = await page.locator(HINT).evaluate((el) => el.getAttributeNames().filter((n) => n === 'role' || n.startsWith('aria-')));
        assert.deepEqual(attrs, []);
        const counts = await collectSafety(page, rec);
        assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
        await context.close();
      }));
    });
  }

  test(`入力欄なしの教材では案内が 0 件（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, userAgent: UA.mac });
      await page.goto(`${srv.url}?fixture=no-input`);
      await waitMounted(page);
      assert.equal(await page.locator(HINT).count(), 0);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
      await context.close();
    }));
  });
}
