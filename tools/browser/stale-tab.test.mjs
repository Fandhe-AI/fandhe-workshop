// 古いタブの保存を止める試験（TASK-324 1/2。SAVE-12。PoC-6 の S-8 相当）。
//
// 同じ context（同じオリジンの localStorage を共有する）で 2 枚のタブを開き、次を確かめる。
//   (1) タブ A で 1 手を送ると保存の鍵が書かれ、A は古いタブではない。
//   (2) タブ B を開いて 1 手を送ると保存値が変わり、B は古いタブではない（保存先が書ける陽性の対照）。この値を基準にする。
//   (3) A で 1 手を送ると、A のエンジンのログは伸びる（メモリ上は進む）が、保存値は基準のままで、A は古いタブになる。
//   (4) A を `pagehide` つきで閉じても、保存値は基準のままである。
// B の再読み込みでの復元（新しいタブの操作が戻ること）は TASK-324 2/2 が足す（このファイルを広げる）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
// 試験の出力・失敗の文に、保存した値・起動番号を出さない（長さと簡易の digest だけを比べる）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-stale-tab/';
const PACK = 'min-3step';
const START = 's-1';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const ACCEPTED = '/make-note first-note';

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const keyOf = (page) => page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, START]);
const stale = (page) => page.evaluate(() => window.__fandheTest.stale());

// 保存値の長さと簡易の digest（値そのものは返さない）。無ければ null。
const saved = (page, key) => page.evaluate((k) => {
  const v = localStorage.getItem(k);
  if (v === null) return null;
  let h = 0;
  for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
  return { len: v.length, h };
}, key);

// ログの長さの増加で待つ（2 手目以降の拒否もログを伸ばす状態の変更）。
async function sendStep(page) {
  const before = (await engine(page)).logLen;
  await page.fill(INPUT, ACCEPTED);
  await page.click(SUBMIT);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before);
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null');
}

// 同じ context に 2 枚目のタブを開く。openPage はポップアップを閉じる監視を付けるため、2 枚目を作る前に外す。
async function secondTab(context, url) {
  context.removeAllListeners('page');
  const page = await context.newPage();
  await page.goto(url);
  await waitMounted(page);
  return page;
}

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`古いタブの操作は保存領域に書かれず、新しいタブの保存が残る（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page: a, context, rec } = await openPage(browser, { origin: srv.origin });
      await a.goto(srv.url);
      await waitMounted(a);
      const key = await keyOf(a);

      // (1) A の 1 手は保存される。
      await sendStep(a);
      assert.notEqual(await saved(a, key), null, 'A saved its first change');
      assert.equal(await stale(a), false);

      // (2) B を開いて 1 手を送ると保存値が変わる（陽性の対照）。
      const b = await secondTab(context, srv.url);
      const beforeB = await saved(b, key);
      await sendStep(b);
      const dB = await saved(b, key);
      assert.notDeepEqual(dB, beforeB, 'B wrote to the store');
      assert.equal(await stale(b), false);

      // (3) A の操作はメモリ上は進むが、保存値は B のままで、A は古いタブになる。
      const logA = (await engine(a)).logLen;
      await sendStep(a);
      assert.equal((await engine(a)).logLen > logA, true, 'A advanced in memory');
      assert.deepEqual(await saved(b, key), dB, 'A did not overwrite the newer tab');
      assert.equal(await stale(a), true);
      assert.equal(await stale(b), false);

      // (4) A を pagehide つきで閉じても、保存値は B のままである。
      await a.close({ runBeforeUnload: true });
      assert.deepEqual(await saved(b, key), dB, 'pagehide of the stale tab did not write');
      clean(rec);
      await context.close();
    }));
  });
}
