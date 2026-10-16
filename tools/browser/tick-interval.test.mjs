// 進行の間隔（TASK-229。EVT-18・D-47）の試験。配信する設定が `?tick=5` を無視することを確かめる。
//
// 入力: FANDHE_WEB_DIR（配信する設定）と FANDHE_WEB_TEST_DIR（試験の設定）。未設定・不在は skip にせず失敗にする。
// 限界: 現時点の画面にはタイマーが無く、遅延の記録は 0 件でも合格する。タイマー（TASK-349）が入った後に効く前向きの錠である。
// 画面上で 300ms ごとに進む観察は TASK-453 が持つ。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted,
} from './harness.mjs';

const BASE = '/fandhe-tick/';
let browser;

function requireDir(name) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a built web directory`);
  return dir;
}

before(async () => {
  requireDir('FANDHE_WEB_DIR');
  requireDir('FANDHE_WEB_TEST_DIR');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// ページが要求したタイマーの遅延を記録する（試験側の計測。配信物には何も足さない）。
const RECORD_TIMERS = () => {
  window.__delays = [];
  for (const name of ['setTimeout', 'setInterval']) {
    const orig = window[name].bind(window);
    window[name] = (fn, delay, ...rest) => {
      window.__delays.push(Number(delay ?? 0));
      return orig(fn, delay, ...rest);
    };
  }
};

test('配信する設定は ?tick=5 を無視する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const htmls = [];
    for (const query of ['', '?tick=5']) {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.addInitScript(RECORD_TIMERS);
      await page.goto(`${srv.url}${query}`);
      await waitMounted(page);
      htmls.push(await page.evaluate(() => document.querySelector('#app').innerHTML));
      assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
      const delays = await page.evaluate(() => window.__delays);
      assert.ok(!delays.includes(5), JSON.stringify(delays));
      const s = summarize(rec);
      const detail = JSON.stringify(rec);
      assert.equal(s.csp, 0, detail);
      assert.equal(s.pageErrors, 0, detail);
      assert.equal(s.external, 0, detail);
      await context.close();
    }
    assert.equal(htmls[0], htmls[1]);
  });
});

test('試験の設定でも ?tick=5 で間隔は変わらず 300 のまま', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(`${srv.url}?tick=5`);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => window.__fandheTest.tickIntervalMs()), 300);
    await context.close();
  });
});
