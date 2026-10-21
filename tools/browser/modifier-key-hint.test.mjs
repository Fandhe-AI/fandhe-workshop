// 推定した OS ごと・受講者が選んだ OS ごとの修飾キーの表示のブラウザ試験（TASK-285 1/2・2/2。JUDGE-28・D-48。TERM-38 の案内の一部）。
//
// `static/app.js` が `navigator.userAgent` を `mount_app` へ渡し、Rust 側がその文字列から OS を推定して
// 入力欄の下の案内（`[data-testid="key-hint"]`）に貼り付けのキーを出す。User-Agent は Playwright の
// `newContext({ userAgent })`（`openPage` の `userAgent`）で与え、配信物には差し替えの経路を持たない。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい context を開く）:
//   - Windows の UA は「貼り付け: Ctrl+V」で選ぶ欄（`os-select`）が windows、macOS の UA は「貼り付け: Cmd+V」で mac、
//     Linux の UA は Cmd・Ctrl を出さず OS を選ぶよう促し、欄は空（推定できない）。PoC-2 の OS-1・OS-3・OS-4 に相当
//   - 選ぶ欄で切り替えると案内が変わる（Windows の UA から mac で Cmd+V、Linux の UA から windows で Ctrl+V）。OS-2・OS-5 に相当
//   - 切り替えの前後で案内と選ぶ欄の要素が同じノードのまま（root の全置換でない）
//   - 選ぶ欄は 24x24 以上で、role・aria を持たず、label の中にある
//   - 入力欄なしの教材（`?fixture=no-input`）では案内も選ぶ欄も 0 件（入力欄の判定の下にあること）
//   - 案内は role・aria を持たない。危険な要素・実行・CSP 違反・ダイアログが 0 件
//
// 確かめないこと:
//   - 推定が外れたときに受講者が選ぶ欄で直せるかの人の確認（TASK-393・TASK-460）と、Windows の実機・人の目での
//     確認（TASK-20・TASK-286・TASK-395）。ここは User-Agent の差し替えによる模擬で、どちらも未実施
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
const SELECT = '[data-testid="os-select"]';
const PROMPT = 'OS を選ぶと、貼り付けのキーが表示されます';

const UA = {
  windows: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  mac: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  linux: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
};

const CASES = [
  { name: 'Windows', ua: UA.windows, text: '貼り付け: Ctrl+V', value: 'windows' },
  { name: 'macOS', ua: UA.mac, text: '貼り付け: Cmd+V', value: 'mac' },
  { name: 'Linux（推定できない）', ua: UA.linux, text: PROMPT, value: '' },
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
        assert.equal(await page.locator(SELECT).count(), 1);
        assert.equal(await page.locator(SELECT).inputValue(), c.value);
        if (c.value === '') {
          const t = await page.locator(HINT).textContent();
          assert.ok(!t.includes('Cmd') && !t.includes('Ctrl'), t);
        }
        const counts = await collectSafety(page, rec);
        assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
        await context.close();
      }));
    });
  }

  for (const sw of [
    { name: 'Windows の推定を mac に直す', ua: UA.windows, to: 'mac', text: '貼り付け: Cmd+V' },
    { name: 'Linux（推定できない）から windows を選ぶ', ua: UA.linux, to: 'windows', text: '貼り付け: Ctrl+V' },
  ]) {
    test(`選ぶ欄で切り替えると案内が変わり要素は同じノードのまま: ${sw.name}（CSP ${label}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin, userAgent: sw.ua });
        await page.goto(srv.url);
        await waitMounted(page);
        await page.evaluate(([h, s]) => {
          window.__keep = { hint: document.querySelector(h), select: document.querySelector(s) };
        }, [HINT, SELECT]);
        await page.locator(SELECT).selectOption(sw.to);
        await page.waitForFunction(([h, t]) => document.querySelector(h)?.textContent === t, [HINT, sw.text]);
        assert.equal(await page.locator(SELECT).inputValue(), sw.to);
        const same = await page.evaluate(([h, s]) => ({
          hint: window.__keep.hint.isSameNode(document.querySelector(h)),
          select: window.__keep.select.isSameNode(document.querySelector(s)),
        }), [HINT, SELECT]);
        assert.deepEqual(same, { hint: true, select: true });
        const counts = await collectSafety(page, rec);
        assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
        await context.close();
      }));
    });
  }

  test(`選ぶ欄は 24x24 以上で role・aria を持たず label の中にある（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, userAgent: UA.windows });
      await page.goto(srv.url);
      await waitMounted(page);
      const info = await page.locator(SELECT).evaluate((el) => {
        const r = el.getBoundingClientRect();
        return {
          w: r.width,
          h: r.height,
          attrs: el.getAttributeNames().filter((n) => n === 'role' || n.startsWith('aria-')),
          inLabel: el.closest('label') !== null,
        };
      });
      assert.ok(info.w >= 24 && info.h >= 24, JSON.stringify(info));
      assert.deepEqual(info.attrs, []);
      assert.equal(info.inLabel, true);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
      await context.close();
    }));
  });

  test(`入力欄なしの教材では案内が 0 件（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, userAgent: UA.mac });
      await page.goto(`${srv.url}?fixture=no-input`);
      await waitMounted(page);
      assert.equal(await page.locator(HINT).count(), 0);
      assert.equal(await page.locator(SELECT).count(), 0);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
      await context.close();
    }));
  });
}
