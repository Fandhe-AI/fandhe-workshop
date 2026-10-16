// 処理中の Esc の停止（TASK-277 1/2。JUDGE-17 (1)・TERM-7）のうち、今の画面で観察できる行の試験。
//
// K-11・K-14（処理中の停止）は、native の key_route_contract が経路を固定し、画面の結果は stop-redo.test.mjs と
// esc-outcomes.test.mjs（TASK-441 1/2）が見る。ここでは、処理中でない画面の下書きの欄で Esc の keydown が preventDefault されず、
// 欄の値が変わらず、送信済みの一覧の項目が増えないこと、CSP 違反・pageerror・console error が 0 件であることを確かめる。
//
// 入力: FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-esc-key/';
const INPUT = '[data-testid="draft-input"]';
const ITEMS = '[data-testid="log-list"] > li';

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

for (const csp of [true, false]) {
  for (const text of ['hello', '']) {
    const label = text === '' ? '文字なし' : '文字あり';
    test(`処理中でないとき、Esc は preventDefault されず欄も一覧も変わらない（${label}・CSP ${csp ? 'あり' : 'なし'}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        await page.goto(srv.url);
        await waitMounted(page);
        await page.fill(INPUT, text);
        await page.focus(INPUT);
        const count = await page.locator(ITEMS).count();
        // dispatchEvent は preventDefault されていなければ true を返す。
        const notPrevented = await page.evaluate((sel) => {
          const el = document.querySelector(sel);
          return el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
        }, INPUT);
        assert.equal(notPrevented, true);
        await page.keyboard.press('Escape');
        assert.equal(await page.inputValue(INPUT), text);
        assert.equal(await page.locator(ITEMS).count(), count);
        const s = summarize(rec);
        const detail = JSON.stringify(rec);
        assert.equal(s.csp, 0, detail);
        assert.equal(s.pageErrors, 0, detail);
        assert.equal(s.consoleErrors, 0, detail);
        await context.close();
      }));
    });
  }
}

// TASK-350・FEND-5: root（main）が Esc の属性を持ち、prevent-default・ignore-repeat は持たない。
for (const csp of [true, false]) {
  test(`main は Esc の keydown の属性だけを持つ（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.goto(srv.url);
      await waitMounted(page);
      const attrs = await page.evaluate(() => {
        const m = document.querySelector('main[data-testid="hello-root"]');
        return {
          action: m.getAttribute('data-action-keydown'),
          keys: m.getAttribute('data-keys'),
          prevent: m.hasAttribute('data-keydown-prevent-default'),
          repeat: m.hasAttribute('data-keydown-ignore-repeat'),
        };
      });
      assert.deepEqual(attrs, { action: 'esc', keys: 'Escape', prevent: false, repeat: false });
      const s = summarize(rec);
      const detail = JSON.stringify(rec);
      assert.equal(s.csp, 0, detail);
      assert.equal(s.pageErrors, 0, detail);
      assert.equal(s.consoleErrors, 0, detail);
      await context.close();
    }));
  });
}
