// 履歴のファイルへの書き出し（TASK-330 1/2、SAVE-16・SAVE-17）の試験。
//
// 入力: FANDHE_WEB_TEST_DIR（試験の設定。`debug_export_*` と `exportClick` を持つ）。未設定・不在は skip にせず失敗にする。
// 確かめること: (1) 書き出しが ops と snapshot の両方を持つ JSON である、(2) 書き出しで外部への要求・CSP 違反・ページのエラーが 0 件、
// (3) URL（location・履歴・ハッシュ）に状態が載らない、(4) 破棄した blob: URL は読めない。
// ダウンロードの click は配信の本体に置けない（no_hidden_triggers）。試験の設定の test-hooks.js だけが <a download> を作る。
// 試験の出力・失敗の文に、履歴の中身を出さない（受講者のデータのため。長さと真偽だけを比べる）。
// 限界: 受講中の実際の ops の記録がホストに無い間は、契約の試料（2 周分）を書き出す。Chromium だけで確かめる。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize } from './harness.mjs';

const BASE = '/fandhe-export/';
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

// ブラウザが保存したダウンロードの中身（Buffer）。
async function readDownload(download) {
  const path = await download.path();
  assert.ok(path, 'download must be saved');
  return fs.readFileSync(path);
}

// blob: URL の読み出しは CSP の connect-src 'self' が fetch を止めるため、ダウンロードで確かめる。
// ダウンロードが起きない・失敗したときは null、成功したときは保存された中身を返す。
async function tryDownload(page, url, name) {
  const pending = page.waitForEvent('download', { timeout: 2000 }).catch(() => null);
  await call(page, 'exportClick', url, name);
  const download = await pending;
  if (!download || (await download.failure())) return null;
  return readDownload(download);
}

test('書き出しは ops と snapshot の両方を持つ JSON のファイルで、外部通信も URL の変化も無い', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const before = await page.evaluate(() => ({ href: location.href, hash: location.hash, length: history.length, state: history.state }));

    const prepared = JSON.parse(await call(page, 'exportPrepare'));
    assert.equal(prepared.ok, true, 'export must be prepared');
    assert.match(prepared.url, /^blob:/);
    assert.match(prepared.name, /^fandhe-workshop-history(-[a-z0-9-]{1,64})?\.json$/);

    const [download] = await Promise.all([
      page.waitForEvent('download'),
      call(page, 'exportClick', prepared.url, prepared.name),
    ]);
    assert.equal(download.suggestedFilename(), prepared.name);
    const bytes = await readDownload(download);
    assert.equal(bytes.length, prepared.bytes, 'downloaded length matches the prepared length');

    // ops と snapshot の両方がある（SAVE-17。ops だけの形ではない）。
    const parsed = JSON.parse(bytes.toString('utf8'));
    assert.ok(Array.isArray(parsed.ops), 'ops is an array');
    assert.ok(parsed.snapshot && typeof parsed.snapshot === 'object', 'snapshot is an object');
    assert.equal(parsed.format, 'fandhe-workshop-history');
    assert.equal(bytes.includes(0x0a), false, 'export is a single line');

    // URL に状態が載らない（SAVE-16）。
    const after = await page.evaluate(() => ({ href: location.href, hash: location.hash, length: history.length, state: history.state }));
    assert.deepEqual(after, before, 'location and history are unchanged by the export');
    assert.equal(after.hash, '');

    // 外部への通信・CSP 違反・ページのエラー・ダイアログ・ポップアップが 0 件（blob: は harness が許可する）。
    const s = summarize(rec);
    assert.equal(s.external, 0, 'no external request');
    assert.equal(s.csp, 0, 'no CSP violation');
    assert.equal(s.pageErrors, 0, 'no page error');
    assert.equal(s.dialogs, 0, 'no dialog');
    assert.equal(s.popups, 0, 'no popup');
    await context.close();
  });
});

test('破棄した blob: URL は読めない（URL を残さない）', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const prepared = JSON.parse(await call(page, 'exportPrepare'));
    assert.equal(prepared.ok, true);
    assert.notEqual(await tryDownload(page, prepared.url, prepared.name), null, 'downloadable before revoke');
    assert.equal(await call(page, 'exportRevoke'), true);
    assert.equal(await tryDownload(page, prepared.url, prepared.name), null, 'not downloadable after revoke');
    assert.equal(await call(page, 'exportRevoke'), false, 'nothing left to revoke');
    assert.equal(summarize(rec).external, 0);
    await context.close();
  });
});

test('作り直すと前の URL は破棄される', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const first = JSON.parse(await call(page, 'exportPrepare'));
    const second = JSON.parse(await call(page, 'exportPrepare'));
    assert.notEqual(first.url, second.url);
    assert.equal(await tryDownload(page, first.url, first.name), null, 'previous URL is revoked');
    assert.notEqual(await tryDownload(page, second.url, second.name), null, 'current URL is downloadable');
    await context.close();
  });
});
