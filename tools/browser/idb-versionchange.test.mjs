// IndexedDB の versionchange の試験（Issue #1509）。保持中の接続が、別のタブの版上げの要求で閉じられ、
// 相手の open が `blocked` にならずに進むことと、閉じた後の書き込みが `unavailable` で失敗し手元の写しが残ることを確かめる。
//
// 入力: FANDHE_WEB_TEST_DIR（試験の設定。`debug_storage_*` を持つ）。未設定・不在は skip にせず失敗にする。
// 対照: wasm を使わない生の接続（versionchange のハンドラ無し）を保持すると、同じ要求が `blocked` になる。
//       主の試験が差を検出できることを示す（SAFE-8 の方式）。
// 限界: 実際の版上げ（DB_VERSION を上げた配信物）は使わず、別タブから版 2 を開いて模す。Chrome でだけ確かめている。
// 試験の出力・失敗の文に、保存した値を出さない（長さと真偽だけを比べる）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted,
} from './harness.mjs';

const BASE = '/fandhe-idb-vc/';
const PACK = 'bbs-tutorial';
const START = 's-0-1';
// src/storage_select.rs の SWITCH_THRESHOLD_UTF16（5,242,877 の 80%）。
const THRESHOLD = 4_194_301;
const WAIT_MS = 5000;
let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a built web directory');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

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

async function openStorage(page) {
  await page.evaluate(() => window.__fandheTest.storageOpen());
  return waitState(page, (s) => s.opened === true, 'storage did not become ready');
}

// 同じ context に 2 枚目のタブを開く（同じオリジンの IndexedDB を共有する）。
// openPage はポップアップを閉じる監視を context に付けるため、2 枚目を作る前に外す。
async function secondTab(context, url) {
  context.removeAllListeners('page');
  const page = await context.newPage();
  await page.goto(url);
  await waitMounted(page);
  return page;
}

// 別タブから版 2 で開く。blocked の発火の有無と、success までの結果を返す（時間切れは 'timeout'）。
const upgrade = (page) => page.evaluate((ms) => new Promise((resolve) => {
  let blocked = false;
  const req = indexedDB.open('fandhe-workshop', 2);
  const timer = setTimeout(() => resolve({ result: 'timeout', blocked }), ms);
  req.onblocked = () => { blocked = true; };
  req.onerror = () => { clearTimeout(timer); resolve({ result: 'error', blocked }); };
  req.onsuccess = () => {
    clearTimeout(timer);
    req.result.close();
    resolve({ result: 'success', blocked });
  };
}), WAIT_MS);

test('保持中の接続は別タブの版上げで閉じられ、相手の open は blocked にならずに進む', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const s = await openStorage(page);
    assert.equal(s.idbAvailable, true, 'IndexedDB is open');
    const other = await secondTab(context, srv.url);
    const r = await upgrade(other);
    assert.equal(r.result, 'success');
    assert.equal(r.blocked, false);
    await context.close();
  });
});

test('閉じた後の書き込みは unavailable で失敗し、手元の写しは値を持ち続ける', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const key = await page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, START]);
    // 閾値の手前まで埋めて、書き込みが IndexedDB へ向かう状態にする（storage-select.test.mjs と同じ）。
    await page.evaluate((n) => {
      localStorage.setItem('unrelated-pad', 'x'.repeat(n));
    }, THRESHOLD - 100 - 'unrelated-pad'.length);
    const s = await openStorage(page);
    assert.equal(s.idbAvailable, true, 'IndexedDB is open');
    const other = await secondTab(context, srv.url);
    assert.equal((await upgrade(other)).result, 'success');
    const value = 'v'.repeat(400);
    const w = JSON.parse(await page.evaluate(([p, st, v]) => window.__fandheTest.storageWrite(p, st, v), [PACK, START, value]));
    assert.equal(w.ok, true);
    const after = await waitState(page, (st) => st.lastAsyncError !== null, 'async error was not recorded');
    assert.equal(after.lastAsyncError, 'unavailable');
    const read = await page.evaluate(([p, st]) => window.__fandheTest.storageRead(p, st), [PACK, START]);
    assert.equal(read === value, true, 'written value is still readable in this session');
    assert.equal(await page.evaluate((k) => localStorage.getItem(k), key), null, 'no localStorage copy was invented');
    await context.close();
  });
});

test('対照: versionchange のハンドラが無い接続を保持すると、同じ版上げは blocked になる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await page.evaluate(() => new Promise((resolve, reject) => {
      const req = indexedDB.open('fandhe-workshop', 1);
      req.onupgradeneeded = () => req.result.createObjectStore('history');
      req.onerror = () => reject(new Error('open failed'));
      req.onsuccess = () => { window.__heldDb = req.result; resolve(); };
    }));
    const other = await secondTab(context, srv.url);
    const r = await upgrade(other);
    assert.equal(r.blocked, true);
    assert.equal(r.result, 'timeout');
    await page.evaluate(() => window.__heldDb.close());
    await context.close();
  });
});
