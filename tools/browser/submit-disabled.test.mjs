// 送信ボタンが無効なときの Enter と、有効なときの Enter・click の一致の試験（TASK-352 2/3。FEND-14・D-42）。
//
// FEND-14: 送信ボタンが無効なら Enter を押しても送信は 0 件で、有効なら Enter の結果は送信ボタンの click と同じになる。
// glue は修飾なしの Enter で preventDefault し、実在の送信ボタンへ click を送る。無効なボタンへの click は
// ブラウザが何もしない（HTML の仕様）ため、「無効なら 0 件」はこの click 経路が持つ性質である。
// 製品の画面には送信ボタンが無効になる状態がまだ無い（空の下書きと処理中は core が拒否する。EVT-7）ので、
// 無効の場面は試験の側で disabled を立てて作る（safety の「属性を書き換えて click」と同じ前例）。
// dispatch_action("submit") の経路は試さない（無効なボタンを素通りするため。TASK-352 の検証方法）。
//
// 標準でない要素（左の行）の Enter・Space は、行の結線は point-keys.test.mjs（TASK-358 1/2）が確かめる。
// 判定そのものは key_route::row_key_effect が持ち、native の契約試験（key_route_contract）が固定している。
// 内部の印は読まない。window の捕捉の段で保持した keydown の Event の defaultPrevented と DOM だけを見る。
//
// 入力: FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-submit-disabled/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const ITEMS = '[data-testid="log-list"] > li';
const DRAFT = '送信の確認';

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

const CAPTURE = (sel) => {
  window.__keyEvents = [];
  window.addEventListener('keydown', (ev) => {
    const t = ev.target;
    if (t && t.matches && t.matches(sel)) window.__keyEvents.push(ev);
  }, true);
};

// 非同期の経路（遅れた送信）を見逃さないよう、タイマーを 2 回挟む。
const settle = (page) => page.evaluate(() => new Promise((r) => {
  setTimeout(() => setTimeout(r, 0), 0);
}));

const lines = (page) => page.$$eval(ITEMS, (lis) => lis.map((li) => li.textContent));

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.addInitScript(CAPTURE, INPUT);
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

test('DISABLED: 送信ボタンが無効なら、Enter でも click でも送信は 0 件', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    await page.fill(INPUT, DRAFT);
    await page.evaluate((sel) => { document.querySelector(sel).disabled = true; }, SUBMIT);
    assert.equal(await page.locator(SUBMIT).isDisabled(), true, '押す前の送信ボタンは無効（前提）');
    await page.focus(INPUT);
    await page.evaluate(() => { window.__keyEvents.length = 0; });
    await page.keyboard.press('Enter');
    await settle(page);

    const seen = await page.evaluate(() => window.__keyEvents
      .filter((ev) => ev.key === 'Enter')
      .map((ev) => ev.defaultPrevented));
    assert.deepEqual(seen, [true], 'Enter の keydown は 1 回で、preventDefault される');
    assert.deepEqual(await lines(page), [], 'Enter では送信されない');
    assert.equal(await page.inputValue(INPUT), DRAFT, '欄の値は変わらず、改行も入らない');
    assert.equal(await page.locator(SUBMIT).isDisabled(), true, '押した後も無効のまま（前提）');

    // click も同じ結果（0 件）。page.click は有効になるのを待つため、DOM の click() を直接呼ぶ。
    await page.evaluate((sel) => document.querySelector(sel).click(), SUBMIT);
    await settle(page);
    assert.deepEqual(await lines(page), [], 'click でも送信されない');
    assert.equal(await page.inputValue(INPUT), DRAFT);
    assertClean(rec);
    await context.close();
  });
});

test('ENABLED: 送信ボタンが有効なら、Enter の結果は click と同じ', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const a = await open(srv);
    const b = await open(srv);
    for (const { page } of [a, b]) {
      await page.fill(INPUT, DRAFT);
      assert.equal(await page.locator(SUBMIT).isDisabled(), false, '押す前の送信ボタンは有効（前提）');
    }
    await a.page.click(SUBMIT);
    await b.page.focus(INPUT);
    await b.page.keyboard.press('Enter');
    for (const { page } of [a, b]) {
      await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, ITEMS);
      await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT);
      await settle(page);
    }
    const viaClick = await lines(a.page);
    const viaEnter = await lines(b.page);
    assert.equal(viaClick.length, 1);
    assert.ok(viaClick[0].includes(DRAFT), 'ログの行に下書きが入る');
    assert.deepEqual(viaEnter, viaClick, 'Enter の記録は click と同じ');
    assert.equal(await b.page.inputValue(INPUT), '', '送信後の欄は空');
    for (const { rec, context } of [a, b]) {
      assertClean(rec);
      await context.close();
    }
  });
});
