// 右のログの文字を選択してコピーできることの画面の試験（TERM-37。TASK-455 1/2。#996）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。試験の設定（TEST_HOOKS=1）の `?fixture=ch0-3-chapters&start=s-1-1` を開き、
// 入力欄からコマンドを 1 つ送って右のログに受付の行を出し、次を確かめる。
//   C1 ログの行の文字は選択できる（計算された `user-select` が none でない）。行の文字の範囲を選択すると、選択の文字が行の文字と一致する
//   C2 選択したまま Cmd+C（macOS）・Ctrl+C（それ以外）を押すと、クリップボードの文字が選択の文字と一致する
//   C3 コピーの操作の前後で、入力欄の値と右のログの行の数が変わらない（コピーは何も送らない）
//   共通 ページ内のエラー・CSP 違反が 0 件。CSP あり・なしで流す
// クリップボードは Playwright の文脈に読み書きの権限を与えて読む（受講者の操作のときだけ書かれる。ページは書かない）。
// 範囲外: 「入力欄に入れる」ボタン（TASK-455 2/2。#997。教材が「例」と明示したコマンドの印が要る）。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted,
} from './harness.mjs';

const BASE = '/fandhe-log-copy/';
const QUERY = '?fixture=ch0-3-chapters&start=s-1-1';
const DRAFT = '[data-testid="draft-input"]';
const ROWS = '[data-testid="part-log-list"] li';
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
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  test(`C1〜C3: ログの行の文字を選択してコピーできる（${mode}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (d) => withServer({ dir: d, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
      try {
        await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: srv.origin });
        await page.goto(`${srv.url}${QUERY}`);
        await waitMounted(page);
        await page.fill(DRAFT, '/help');
        await page.press(DRAFT, 'Enter');
        await page.waitForFunction((s) => document.querySelectorAll(s).length >= 1, ROWS, { timeout: 30000 });
        const rowsBefore = await page.locator(ROWS).count();
        // C1: 最後の行の文字を選択する。
        const picked = await page.evaluate((s) => {
          const rows = [...document.querySelectorAll(s)];
          const row = rows[rows.length - 1];
          const range = document.createRange();
          range.selectNodeContents(row);
          const sel = window.getSelection();
          sel.removeAllRanges();
          sel.addRange(range);
          return {
            userSelect: getComputedStyle(row).userSelect,
            rowText: row.textContent ?? '',
            selected: sel.toString(),
          };
        }, ROWS);
        assert.notEqual(picked.userSelect, 'none', 'log rows are selectable');
        assert.ok(picked.selected.trim().length > 0, 'something is selected');
        assert.equal(picked.selected.replace(/\s+/g, ' ').trim(), picked.rowText.replace(/\s+/g, ' ').trim(), 'selection equals the row text');
        const draftBefore = await page.inputValue(DRAFT);
        // C2: 受講者の操作（キー）でコピーする。
        await page.keyboard.press(process.platform === 'darwin' ? 'Meta+C' : 'Control+C');
        const copied = await page.evaluate(() => navigator.clipboard.readText());
        assert.equal(copied.replace(/\s+/g, ' ').trim(), picked.selected.replace(/\s+/g, ' ').trim(), 'clipboard holds the selection');
        // C3: コピーは何も送らない。
        assert.equal(await page.inputValue(DRAFT), draftBefore, 'draft unchanged');
        assert.equal(await page.locator(ROWS).count(), rowsBefore, 'log unchanged');
        const s = summarize(rec);
        assert.equal(s.pageErrors, 0, JSON.stringify(s));
        assert.equal(s.csp, 0, JSON.stringify(s));
      } finally {
        await context.close();
      }
    }));
  });
}

// 対照: ログに `user-select: none` を当てると C1 の判定が none を読み、コピーした文字は行の文字にならない。
test('対照: user-select: none のログは選択できない形として検出される', { timeout: 120000 }, async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    try {
      await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: srv.origin });
      await page.goto(`${srv.url}${QUERY}`);
      await waitMounted(page);
      await page.fill(DRAFT, '/help');
      await page.press(DRAFT, 'Enter');
      await page.waitForFunction((s) => document.querySelectorAll(s).length >= 1, ROWS, { timeout: 30000 });
      await page.evaluate(() => navigator.clipboard.writeText('sentinel'));
      const got = await page.evaluate((s) => {
        document.querySelector('[data-testid="part-log-list"]').style.userSelect = 'none';
        const rows = [...document.querySelectorAll(s)];
        const row = rows[rows.length - 1];
        const range = document.createRange();
        range.selectNodeContents(row);
        window.getSelection().removeAllRanges();
        window.getSelection().addRange(range);
        return { userSelect: getComputedStyle(row).userSelect, rowText: row.textContent ?? '' };
      }, ROWS);
      assert.equal(got.userSelect, 'none', 'the detector reads user-select: none');
      await page.keyboard.press(process.platform === 'darwin' ? 'Meta+C' : 'Control+C');
      const copied = await page.evaluate(() => navigator.clipboard.readText());
      assert.notEqual(copied.trim(), got.rowText.trim(), 'the row text is not copied');
    } finally {
      await context.close();
    }
  });
});
