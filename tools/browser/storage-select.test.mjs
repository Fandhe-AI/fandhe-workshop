// 履歴の保存先の選択（TASK-317 1/2。D-52・SAVE-8）の試験。localStorage が既定で、閾値を超える見込みの書き込みから
// IndexedDB へ切り替わり、確定の後に localStorage の写しが消え、再読み込みの後は IndexedDB が正になることを確かめる。
//
// 入力: FANDHE_WEB_TEST_DIR（試験の設定。`debug_storage_*` を持つ）。未設定・不在は skip にせず失敗にする。
// 限界: 確定を待たずに離れる場合（pagehide の書き込み）は storage-leave.test.mjs（TASK-317 2/2）が確かめ、ここでは確かめない。
//       localStorage の上限（5,242,877）は Chrome での実測で、別のブラウザでは確かめていない。
// 試験の出力・失敗の文に、保存した値を出さない（履歴は受講者のデータのため。長さと真偽だけを比べる）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted,
} from './harness.mjs';

const BASE = '/fandhe-storage/';
const PACK = 'bbs-tutorial';
const START = 's-0-1';
// src/storage_select.rs の SWITCH_THRESHOLD_UTF16（5,242,877 の 80%）。
const THRESHOLD = 4_194_301;
let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a built web directory');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function openStorage(page) {
  await page.evaluate(() => window.__fandheTest.storageOpen());
  await waitState(page, (s) => s.opened === true, 'storage did not become ready');
}

async function state(page) {
  return JSON.parse(await page.evaluate(([p, s]) => window.__fandheTest.storageState(p, s), [PACK, START]));
}

async function waitState(page, pred, message) {
  for (let i = 0; i < 100; i += 1) {
    const s = await state(page);
    if (pred(s)) return s;
    await page.waitForTimeout(50);
  }
  assert.fail(message);
}

// localStorage の値の長さ。無ければ null。
const localLen = (page, key) => page.evaluate((k) => {
  const v = localStorage.getItem(k);
  return v === null ? null : v.length;
}, key);

// IndexedDB の値の長さ。無ければ null。
const idbLen = (page, key) => page.evaluate((k) => new Promise((resolve, reject) => {
  const open = indexedDB.open('fandhe-workshop');
  open.onerror = () => reject(new Error('idb open failed'));
  open.onsuccess = () => {
    const db = open.result;
    const get = db.transaction('history').objectStore('history').get(k);
    get.onsuccess = () => {
      db.close();
      resolve(get.result === undefined ? null : get.result.length);
    };
    get.onerror = () => reject(new Error('idb get failed'));
  };
}), key);

test('小さな履歴は localStorage に入り IndexedDB には入らない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    await openStorage(page);
    const key = await page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, START]);
    assert.ok(key.startsWith('fandhe-workshop/history/'), 'key has the history prefix');
    const w = JSON.parse(await page.evaluate(([p, s]) => window.__fandheTest.storageWrite(p, s, 'small history'), [PACK, START]));
    assert.equal(w.ok, true);
    assert.equal((await state(page)).kind, 'local');
    assert.equal(await localLen(page, key), 'small history'.length);
    assert.equal(await idbLen(page, key), null);
    await context.close();
  });
});

test('閾値を超える見込みなら IndexedDB に切り替わり、確定の後に localStorage の写しが消え、再読み込み後は IndexedDB が正になる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const key = await page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, START]);
    // 閾値の手前（残り約 100 符号単位）まで、ほかの鍵で埋める。使用量の見積もりは最初の書き込みで決まるため、先に埋める。
    await page.evaluate((n) => {
      localStorage.setItem('unrelated-pad', 'x'.repeat(n));
    }, THRESHOLD - 100 - 'unrelated-pad'.length);
    await openStorage(page);
    const value = 'y'.repeat(400);
    const w = JSON.parse(await page.evaluate(([p, s, v]) => window.__fandheTest.storageWrite(p, s, v), [PACK, START, value]));
    assert.equal(w.ok, true);
    assert.equal((await state(page)).kind, 'idb');
    // 確定の後に localStorage の写しが消える（この鍵は localStorage に書かれていないので、無いままである）。
    assert.equal(await localLen(page, key), null);
    // IndexedDB の確定を待つ（トランザクションの完了の後に読める）。
    let stored = null;
    for (let i = 0; i < 100 && stored === null; i += 1) {
      stored = await idbLen(page, key);
      if (stored === null) await page.waitForTimeout(50);
    }
    assert.equal(stored, value.length, 'value reached IndexedDB');
    assert.equal((await state(page)).lastAsyncError, null);
    // ほかの鍵（埋め草）は消えない。
    assert.ok((await localLen(page, 'unrelated-pad')) > THRESHOLD - 200, 'unrelated key is kept');

    // 再読み込み後に開き直すと、起動時の選択は IndexedDB で、書いた値が読める。
    await page.reload();
    await waitMounted(page);
    await openStorage(page);
    const s = await state(page);
    assert.equal(s.kindFor, 'idb');
    assert.equal(s.kind, 'idb');
    const read = await page.evaluate(([p, st]) => window.__fandheTest.storageRead(p, st), [PACK, START]);
    assert.equal(read === value, true, 'value is restored from IndexedDB');
    await context.close();
  });
});

test('切り替えの前に localStorage にあった写しは、確定の後に消える', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const key = await page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, START]);
    await openStorage(page);
    await page.evaluate(([p, s]) => window.__fandheTest.storageWrite(p, s, 'first'), [PACK, START]);
    assert.equal(await localLen(page, key), 'first'.length);
    // 埋め草を足すと、使用量の見積もりが古いので次の書き込みは localStorage に入る。新しい文書で開き直して切り替える。
    await page.evaluate((n) => {
      localStorage.setItem('unrelated-pad', 'x'.repeat(n));
    }, THRESHOLD - 100 - 'unrelated-pad'.length);
    await page.reload();
    await waitMounted(page);
    await openStorage(page);
    const value = 'z'.repeat(400);
    await page.evaluate(([p, s, v]) => window.__fandheTest.storageWrite(p, s, v), [PACK, START, value]);
    assert.equal((await state(page)).kind, 'idb');
    let gone = false;
    for (let i = 0; i < 100 && !gone; i += 1) {
      gone = (await localLen(page, key)) === null;
      if (!gone) await page.waitForTimeout(50);
    }
    assert.equal(gone, true, 'localStorage copy removed after the IndexedDB write was confirmed');
    assert.equal(await idbLen(page, key), value.length);
    await context.close();
  });
});
