// ページを離れる直前の書き込み（TASK-317 2/2。SAVE-9）の試験。localStorage が保存先のとき、保存の遅延を待たずに
// 再読み込み・タブを閉じても、保留に積んだ直前の値が再び開いたときに残ることを確かめる。
//
// 入力: FANDHE_WEB_TEST_DIR（試験の設定。`debug_save_*` を持つ）。未設定・不在は skip にせず失敗にする。
// 試験用の保留（`debug_save_*`）から保存先へ値が届く経路は pagehide のリスナーと saveFlush だけで、遅延保存のタイマーは触れない（本番の遅延保存は save-latency。TASK-318）。
// 対照の試験（リスナー無しでは値が残らない）が、値が残ったのは pagehide の書き込みによることを示す。
// 限界: IndexedDB へ切り替えた後は、直前の変更が残ることを期待にしない（SAVE-9 の検討中の論点）。既存の保存を壊さないことだけを確かめる。
//       Chromium だけで確かめ、Firefox・Safari は確かめていない（localStorage の上限と同じ限界）。
// 試験の出力・失敗の文に、保存した値を出さない（履歴は受講者のデータのため。長さと真偽だけを比べる）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-leave/';
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

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);

async function state(page) {
  return JSON.parse(await call(page, 'storageState', PACK, START));
}

async function openStorage(page) {
  await call(page, 'storageOpen');
  for (let i = 0; i < 100; i += 1) {
    if ((await state(page)).opened === true) return;
    await page.waitForTimeout(50);
  }
  assert.fail('storage did not become ready');
}

async function boot(page, url) {
  await page.goto(url);
  await waitMounted(page);
  await openStorage(page);
}

// localStorage の値の長さ。無ければ null。
const localLen = (page, key) => page.evaluate((k) => {
  const v = localStorage.getItem(k);
  return v === null ? null : v.length;
}, key);

const keyOf = (page) => call(page, 'storageKey', PACK, START);

// 安全の記録（CSP の違反・外部への要求・ページのエラー）が 0 件であること。
function assertClean(rec) {
  const s = summarize(rec);
  assert.equal(s.csp, 0, 'no CSP violation');
  assert.equal(s.external, 0, 'no external request');
  assert.equal(s.pageErrors, 0, 'no page error');
}

test('再読み込みの直前に積んだ値が、pagehide の書き込みで残る', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await boot(page, srv.url);
    const key = await keyOf(page);
    assert.equal(JSON.parse(await call(page, 'saveInstallPagehide')).ok, true);
    const value = 'leave-reload-'.repeat(20);
    assert.equal(JSON.parse(await call(page, 'saveStage', PACK, START, value)).pending, 1);
    // 積んだだけでは書かれていない。
    assert.equal(await localLen(page, key), null);
    await page.reload();
    await waitMounted(page);
    assert.equal(await localLen(page, key), value.length, 'value written by pagehide');
    await openStorage(page);
    const read = await call(page, 'storageRead', PACK, START);
    assert.equal(read === value, true, 'value is readable after reload');
    assertClean(rec);
    await context.close();
  });
});

test('タブを閉じる直前に積んだ値が、開き直したときに残る', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await boot(page, srv.url);
    const key = await keyOf(page);
    await call(page, 'saveInstallPagehide');
    const value = 'leave-close-'.repeat(20);
    await call(page, 'saveStage', PACK, START, value);
    assert.equal(await localLen(page, key), null);
    // 開き直すページは、ハーネスのポップアップの自動閉じの対象にしない。
    context.removeAllListeners('page');
    await page.close({ runBeforeUnload: true });
    const reopened = await context.newPage();
    reopened.on('pageerror', (err) => rec.pageErrors.push(String(err && err.message ? err.message : err)));
    await boot(reopened, srv.url);
    assert.equal(await localLen(reopened, key), value.length, 'value written by pagehide on tab close');
    const read = await call(reopened, 'storageRead', PACK, START);
    assert.equal(read === value, true, 'value is readable after reopening');
    assertClean(rec);
    await context.close();
  });
});

test('対照: pagehide のリスナーが無ければ、積んだだけの値は再読み込みの後に残らない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await boot(page, srv.url);
    const key = await keyOf(page);
    await call(page, 'saveStage', PACK, START, 'never-written-'.repeat(20));
    await page.reload();
    await waitMounted(page);
    assert.equal(await localLen(page, key), null, 'staged value is not written without the listener');
    await openStorage(page);
    // 無いときの storageRead は undefined（wasm の Option::None）。
    assert.equal(await call(page, 'storageRead', PACK, START), undefined);
    assertClean(rec);
    await context.close();
  });
});

test('saveFlush は積んだ値を保存先へ書き、件数と種類だけを返す', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    // 保存先が未準備のときは書かない。
    const notOpened = JSON.parse(await call(page, 'saveFlush'));
    assert.equal(notOpened.ok, false);
    await openStorage(page);
    const key = await keyOf(page);
    await call(page, 'saveStage', PACK, START, 'a');
    await call(page, 'saveStage', PACK, START, 'last-value');
    const out = JSON.parse(await call(page, 'saveFlush'));
    assert.deepEqual(out, { ok: true, written: 1, failed: 0, error: null });
    assert.equal(await localLen(page, key), 'last-value'.length, 'only the last value is written');
    assertClean(rec);
    await context.close();
  });
});

test('IndexedDB へ切り替えた後に離れても、既存の保存は壊れない（直前の変更が残ることは期待にしない）', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    await page.evaluate((n) => {
      localStorage.setItem('unrelated-pad', 'x'.repeat(n));
    }, THRESHOLD - 100 - 'unrelated-pad'.length);
    await openStorage(page);
    const confirmed = 'c'.repeat(400);
    assert.equal(JSON.parse(await call(page, 'storageWrite', PACK, START, confirmed)).ok, true);
    assert.equal((await state(page)).kind, 'idb');
    // IndexedDB の確定を待つ。
    await page.waitForTimeout(500);
    await call(page, 'saveInstallPagehide');
    const staged = 's'.repeat(400);
    await call(page, 'saveStage', PACK, START, staged);
    await page.reload();
    await waitMounted(page);
    await openStorage(page);
    const read = await call(page, 'storageRead', PACK, START);
    assert.ok(read === confirmed || read === staged, 'confirmed or staged value is kept, never lost');
    assertClean(rec);
    await context.close();
  });
});
