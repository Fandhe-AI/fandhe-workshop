// Tab の補完の条件のうち、今の画面で観察できる行の試験（TASK-278 1/3。JUDGE-19・FEND-21・D-42）。
//
// 候補の一覧が開いていて欄に文字があるときだけ、Tab は補完として preventDefault される。それ以外の Tab は
// preventDefault されず、ブラウザの通常のフォーカス移動になる。候補の一覧の部品は TASK-440 が入れるまで画面に無く、
// 「候補が開いている」行はここでは観察できない。その行は native の key_route_contract が固定する。
// ここでは、候補が無い画面で Tab が欄に文字があってもなくても preventDefault されず、
// フォーカスが送信ボタンへ移り、欄の値が変わらない（タブ文字が入らない）ことを確かめる。
// Shift+Tab の確認は #290、Tab の一巡・罠 0 件のブラウザ試験は #291 の担当。
//
// 入力: FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-tab-complete/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';

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

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

for (const csp of [true, false]) {
  for (const text of ['hello', '']) {
    const label = text === '' ? '文字なし' : '文字あり';
    test(`候補が無いとき、Tab は preventDefault されずフォーカスが送信ボタンへ移る（${label}・CSP ${csp ? 'あり' : 'なし'}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await open(srv);
        await page.fill(INPUT, text);
        await page.focus(INPUT);
        // dispatchEvent は preventDefault されていなければ true を返す。
        const notPrevented = await page.evaluate((sel) => {
          const el = document.querySelector(sel);
          return el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }));
        }, INPUT);
        assert.equal(notPrevented, true);
        // 信頼入力の Tab は通常のフォーカス移動になる。
        await page.keyboard.press('Tab');
        assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'submit-input');
        assert.equal(await page.inputValue(INPUT), text);
        assert.ok(await page.$(SUBMIT));
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
