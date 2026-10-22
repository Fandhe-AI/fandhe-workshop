// IndexedDB への保存の失敗で状態を壊さないことの試験（TASK-323 2/2。SAVE-11・SHELL-19）。
//
// 実際の画面で、次を確かめる。
//   (a) 起動の前に localStorage を閾値の手前まで別の鍵で埋めると、1 手目の保存は IndexedDB へ届く。`shell-notice` の `save-failed` 項目 は空。
//   (b) IndexedDB の put を中断させる（`put` の直後に `transaction.abort()` を呼ぶ包み）と、2 手目の後、次の `update`
//       （下書きの打鍵）で `shell-notice` の `save-failed` 項目 が理由の文になる。IndexedDB の値は 1 手目のまま変わらず、エンジンのログは伸びている。
//   (c) 中断を外して 3 手目を送ると IndexedDB の値が更新され、次の打鍵で表示が消える。再読み込みすると 3 手目まで復元される。
//   (d) IndexedDB を開けたが全件の読み取りが失敗すると、起動の直後から `shell-notice` の `save-failed` 項目 が出て、操作しても保存せず、
//       既存の localStorage の値は変わらない（旧い値で巻き戻さない）。
//
//   (e) 表示の形（TASK-534 2/2。SHELL-19）: 通知欄が 1 個・項目が 1 件・ボタン 0・旧来の `save-notice` が 0 件で、localStorage の失敗
//       （`save-quota`）と同じ形である。理由コードと文面 ID は理由ごと（ここでは `write_failed`・`msg-save-failed`）。幅 360px で
//       成果物のタブを選んでも項目が見える。
//
// 失敗の起こし方: put の包み（試験の設定の初期化スクリプトの中だけ）で `abort` を呼ぶ。`tx.error` が無いので失敗の種類は
// `Failed` になる。実際の容量超過（QuotaExceededError）は再現できなかった: CDP の `Storage.overrideQuotaForOrigin` で原点の quota を
// 1,024 バイトにしても（`getUsageAndQuota` は `overrideActive: true`・usage 超過を返す）、IndexedDB への 8,000,000 文字の書き込みまで
// 成功する（Chromium 1243・2026-10-22。`docs/testing.md` 5.233）。種類ごとの文は native の試験（`save_failure_idb`）が確かめる。
// 限界: 非同期の失敗の表示は、失敗した操作の時点ではなく次の `update` で出る（本試験は打鍵で `update` を起こす）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
// 試験の出力・失敗の文に、保存した値を出さない（履歴は受講者のデータのため。長さと簡易 digest だけを比べる）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-save-idb-failure/';
const PACK = 'min-3step';
const START = 's-1';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
// 保存の失敗は枠の通知欄（shell-notice）の save-failed 項目に出る（TASK-534 1/2）。項目が無いときの文は空文字列。
const ITEM = '[data-testid="shell-notice-item"][data-notice-kind="save-failed"]';
const ACCEPTED = '/make-note first-note';
const NOTICE = '[data-testid="shell-notice"]';
const OLD_NOTICE = '[data-testid="save-notice"]';
const FAILED_TEXT = '保存できませんでした（書き込みに失敗しました）';
// 形は `save-quota` と同じ（欄 1・項目 1・ボタン 0・旧来 0）。理由コードと文面 ID は中断（`Failed`）のもの。
const FAILED_SHAPE = { areas: 1, items: 1, buttons: 0, old: 0, reason: 'write_failed', message: 'msg-save-failed' };
const UNAVAILABLE_TEXT = '保存できませんでした（保存領域を使えません）';
// src/storage_select.rs の SWITCH_THRESHOLD_UTF16（5,242,877 の 80%）。
const THRESHOLD = 4_194_301;
const PAD_KEY = 'unrelated-pad';

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 閾値の手前まで別の鍵で埋め、put を `window.__failPut` が真の間だけ中断する包みを入れる。
async function openIdbPage(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.context.addInitScript(([padKey, n]) => {
    try {
      if (localStorage.getItem(padKey) === null) localStorage.setItem(padKey, 'x'.repeat(n));
    } catch { /* 容量の都合で書けない場合は、そのまま続ける */ }
    const orig = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function wrapped(...args) {
      const req = orig.apply(this, args);
      if (window.__failPut === true) this.transaction.abort();
      return req;
    };
  }, [PAD_KEY, THRESHOLD - 100 - PAD_KEY.length]);
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const keyOf = (page) => page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, START]);

// IndexedDB の値の長さと簡易の digest（値そのものは返さない）。無ければ null。
const idbValue = (page, key) => page.evaluate((k) => new Promise((resolve, reject) => {
  const open = indexedDB.open('fandhe-workshop', 1);
  open.onerror = () => reject(new Error('open failed'));
  open.onsuccess = () => {
    const db = open.result;
    const get = db.transaction('history').objectStore('history').get(k);
    get.onsuccess = () => {
      db.close();
      const v = get.result;
      if (typeof v !== 'string') { resolve(null); return; }
      let h = 0;
      for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
      resolve({ len: v.length, h });
    };
  };
}), key);

const localValue = (page, key) => page.evaluate((k) => {
  const v = localStorage.getItem(k);
  if (v === null) return null;
  let h = 0;
  for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
  return { len: v.length, h };
}, key);

const noticeText = (page) => page.evaluate((sel) => {
  const item = document.querySelector(sel);
  return item ? item.querySelector('[data-testid="shell-notice-text"]').textContent : '';
}, ITEM);

// 通知欄の表示の形（文は `noticeText`）。localStorage の失敗（`save-quota`）と同じ形かを比べる。
const noticeShape = (page) => page.evaluate(([area, item, old]) => {
  const el = document.querySelector(item);
  return {
    areas: document.querySelectorAll(area).length,
    items: document.querySelectorAll(item).length,
    buttons: el ? el.querySelectorAll('button').length : -1,
    old: document.querySelectorAll(old).length,
    reason: el ? el.getAttribute('data-notice-reason') : null,
    message: el ? el.getAttribute('data-notice-message') : null,
  };
}, [NOTICE, ITEM, OLD_NOTICE]);

async function sendStep(page) {
  const before = (await engine(page)).logLen;
  await page.fill(INPUT, ACCEPTED);
  await page.click(SUBMIT);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before);
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null');
}

// IndexedDB の確定（または中断）は後から届く。通知欄の文が期待どおりになるまで、打鍵で `update` を起こし続ける。
async function typeUntilNotice(page, expected) {
  for (let i = 0; i < 100; i += 1) {
    if ((await noticeText(page)) === expected) return;
    await page.fill(INPUT, `k${i}`);
    await page.waitForTimeout(50);
  }
  assert.equal(await noticeText(page), expected, 'notice did not reach the expected text');
}

async function waitIdbChange(page, key, pred, message) {
  for (let i = 0; i < 100; i += 1) {
    const v = await idbValue(page, key);
    if (pred(v)) return v;
    await page.waitForTimeout(50);
  }
  assert.fail(message);
}

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`IndexedDB の書き込みが中断されると理由が出て状態は進み既存の保存は変わらず、成功の後に表示が消える（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openIdbPage(srv);
      const key = await keyOf(page);

      // (a) 1 手目は IndexedDB へ届く。
      await sendStep(page);
      const first = await waitIdbChange(page, key, (v) => v !== null, 'the first save did not reach IndexedDB');
      assert.equal(await noticeText(page), '');
      assert.equal(await localValue(page, key), null, 'no localStorage copy once IndexedDB holds the value');
      const len1 = (await engine(page)).logLen;

      // (b) put を中断させて 2 手目。次の update で理由が出る。
      await page.evaluate(() => { window.__failPut = true; });
      await sendStep(page);
      await typeUntilNotice(page, FAILED_TEXT);
      assert.deepEqual(await noticeShape(page), FAILED_SHAPE, 'same area and item shape as the localStorage failure');
      assert.deepEqual(await idbValue(page, key), first, 'the stored value is unchanged');
      assert.equal(await localValue(page, key), null);
      const len2 = (await engine(page)).logLen;
      assert.equal(len2 > len1, true, 'the state advanced in memory');

      // (c) 中断を外して 3 手目。IndexedDB が更新され、次の update で表示が消える。
      await page.evaluate(() => { window.__failPut = false; });
      await sendStep(page);
      const third = await waitIdbChange(page, key, (v) => v !== null && v.len > first.len, 'the third save did not reach IndexedDB');
      await typeUntilNotice(page, '');
      const len3 = (await engine(page)).logLen;
      assert.equal(len3 > len2, true);
      assert.equal(third.len > first.len, true);

      await page.reload();
      await waitMounted(page);
      const after = await engine(page);
      assert.equal(after.bootKind, 'restored');
      assert.equal(after.logLen, len3 + 1, 'restored state plus one reloaded event');
      clean(rec);
      await context.close();
    }));
  });

  test(`IndexedDB の全件を読めないと、起動の直後から理由が出て保存を試さず既存の localStorage の値は変わらない（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const opened = await openPage(browser, { origin: srv.origin });
      const { page, context, rec } = opened;
      await context.addInitScript(() => {
        IDBObjectStore.prototype.getAll = function throwing() { throw new DOMException('blocked', 'UnknownError'); };
      });
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal(await noticeText(page), UNAVAILABLE_TEXT, 'the reason shows before any operation');

      const key = await keyOf(page);
      await page.evaluate((k) => localStorage.setItem(k, 'seed'), key);
      const seeded = await localValue(page, key);
      const before = (await engine(page)).logLen;
      await sendStep(page);
      assert.equal((await engine(page)).logLen > before, true, 'the state advanced in memory');
      assert.deepEqual(await localValue(page, key), seeded, 'the existing localStorage value is unchanged');
      assert.equal(await noticeText(page), UNAVAILABLE_TEXT);
      clean(rec);
      await context.close();
    }));
  });
}

// 幅 360px では選んでいないタブの面が隠れる。IndexedDB の失敗の通知も、成果物のタブを選んだままで見える（SHELL-19）。
test('幅 360px で成果物のタブに切り替えても IndexedDB の失敗の通知が見える', async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openIdbPage(srv);
    await page.setViewportSize({ width: 360, height: 800 });
    const key = await keyOf(page);
    await sendStep(page);
    await waitIdbChange(page, key, (v) => v !== null, 'the first save did not reach IndexedDB');
    await page.evaluate(() => { window.__failPut = true; });
    await sendStep(page);
    await typeUntilNotice(page, FAILED_TEXT);
    await page.click('#pane-tab-artifacts');
    assert.equal(await page.locator('#pane-ops').isVisible(), false, 'the operations pane is hidden');
    assert.equal(await page.locator(ITEM).isVisible(), true, 'the notice is visible with the artifacts tab selected');
    assert.notEqual(await page.locator(ITEM).boundingBox(), null);
    assert.deepEqual(await noticeShape(page), FAILED_SHAPE);
    clean(rec);
    await context.close();
  }));
});
