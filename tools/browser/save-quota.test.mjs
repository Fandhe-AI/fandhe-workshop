// localStorage の容量超過で状態を壊さないことの試験（TASK-323 1/2。SAVE-10・SHELL-18。PoC-6 の S-17 相当）。
//
// 実際の画面で、次を確かめる。
//   (1) 1 手を送信すると、離れる前に保存の鍵が書かれる（変更のたびに保存する経路の陽性の確認）。`save-notice` は空。
//   (2) 保存領域を履歴の鍵ではない鍵で満たし、状態を変える 2 手目を送信すると、`save-notice` が
//       「保存できませんでした（QuotaExceededError）」になる。保存値は 1 手目のまま変わらず、エンジンのログは伸びている。
//   (3) 容量を空けて 3 手目を送信すると、保存に成功して `save-notice` が空に戻り、保存値が更新される。
//   (4) 再読み込みすると `restored` で、3 手目まで進んだ状態が戻る（最新の保存が成功している裏付け）。
//
// IndexedDB を無効にする理由: 容量が足りないとき、保存先は IndexedDB が使えれば切り替えて成功する（D-52）。
// 本試験は localStorage だけの保存先の失敗（SAVE-10）を確かめるため、`window.indexedDB` を隠して起動する。
// IndexedDB の非同期の失敗の表示は TASK-323 2/2 の持ち分である。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
// 試験の出力・失敗の文に、保存した値を出さない（履歴は受講者のデータのため。長さと真偽だけを比べる）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-save-quota/';
const PACK = 'min-3step';
const START = 's-1';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const NOTICE = '[data-testid="save-notice"]';
const ACCEPTED = '/make-note first-note';
const QUOTA_TEXT = '保存できませんでした（QuotaExceededError）';
const PAD_PREFIX = 'pad/';

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.context.addInitScript(() => {
    Object.defineProperty(window, 'indexedDB', { get: () => undefined, configurable: true });
  });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const keyOf = (page) => page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, START]);

// 保存値の長さと簡易の digest（値そのものは返さない）。無ければ null。
const saved = (page, key) => page.evaluate((k) => {
  const v = localStorage.getItem(k);
  if (v === null) return null;
  let h = 0;
  for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
  return { len: v.length, h };
}, key);

const noticeText = (page) => page.$eval(NOTICE, (el) => el.textContent);

// 2 手目以降は同じ入力が拒否される（下書きが残る）が、拒否もログを伸ばす状態の変更なので、ログの長さの増加で待つ。
async function sendStep(page) {
  const before = (await engine(page)).logLen;
  await page.fill(INPUT, ACCEPTED);
  await page.click(SUBMIT);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before);
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null');
}

// 履歴の鍵ではない鍵で localStorage を 1 文字も入らなくなるまで満たす。
const fillStorage = (page) => page.evaluate((prefix) => {
  let n = 0;
  let size = 1 << 20;
  while (size >= 1) {
    try {
      localStorage.setItem(`${prefix}${n}`, 'x'.repeat(size));
      n += 1;
    } catch {
      size >>= 1;
    }
  }
  return n;
}, PAD_PREFIX);

const clearPad = (page) => page.evaluate((prefix) => {
  Object.keys(localStorage).filter((k) => k.startsWith(prefix)).forEach((k) => localStorage.removeItem(k));
}, PAD_PREFIX);

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`容量超過で理由が出て状態は進み既存の保存は変わらず、空けた後の次の変更で表示が消える（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      const key = await keyOf(page);
      assert.equal(await saved(page, key), null, 'nothing is saved before the first operation');

      // (1) 変更の時点で保存される（離れる前）。
      await sendStep(page);
      const first = await saved(page, key);
      assert.notEqual(first, null, 'saved at the change, before leaving');
      assert.equal(await noticeText(page), '');
      const len1 = (await engine(page)).logLen;

      // (2) 容量を満たして 2 手目。
      assert.equal((await fillStorage(page)) > 0, true, 'padding was written');
      await sendStep(page);
      await page.waitForFunction(([sel, t]) => document.querySelector(sel).textContent === t, [NOTICE, QUOTA_TEXT]);
      assert.deepEqual(await saved(page, key), first, 'the saved value is unchanged');
      const len2 = (await engine(page)).logLen;
      assert.equal(len2 > len1, true, 'the state advanced in memory');

      // (3) 空けた後の次の変更で成功し、表示が消える。
      await clearPad(page);
      await sendStep(page);
      await page.waitForFunction((sel) => document.querySelector(sel).textContent === '', NOTICE);
      const third = await saved(page, key);
      assert.equal(third !== null && third.len > first.len, true, 'the saved value is updated');
      const len3 = (await engine(page)).logLen;
      assert.equal(len3 > len2, true);

      // (4) 再読み込みで 3 手目までの状態が戻る。
      await page.reload();
      await waitMounted(page);
      const after = await engine(page);
      assert.equal(after.bootKind, 'restored');
      assert.equal(after.logLen, len3 + 1, 'restored state plus one reloaded event');
      clean(rec);
      await context.close();
    }));
  });
}
