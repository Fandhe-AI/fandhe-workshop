// 最小の画面（Hello）の試験。共通ヘルパで配信設定の web をサブパス配信し、CSP 違反・ページ内エラーが 0 件であることを確かめる。
//
// 入力: 環境変数 FANDHE_WEB_DIR（tools/build-web.sh が出力した配信設定のディレクトリ。scripts/test-browser.sh が用意する）。
// 未設定・不在は skip にせず失敗にする（試験が走らないまま通ることを防ぐ）。
// 0 件が空振りでないことは harness.test.mjs の対照が示す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted } from './harness.mjs';

const BASE = '/fandhe-smoke/';
let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to a built web directory');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

test('最小の画面がサブパスで起動し、CSP 違反・ページ内エラーが 0 件', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);

    const s = summarize(rec);
    const detail = JSON.stringify(rec);
    assert.equal(s.csp, 0, detail);
    assert.equal(s.pageErrors, 0, detail);
    assert.equal(s.consoleErrors, 0, detail);
    assert.equal(s.external, 0, detail);
    assert.equal(s.failed, 0, detail);
    assert.equal(s.httpErrors, 0, detail);

    // 要求の件数は icon の取得の有無で揺れるため、含まれるべき集合だけを確かめる。
    const urls = rec.requests.map((r) => r.url);
    for (const u of urls) assert.ok(u.startsWith(`${srv.origin}${BASE}`), u);
    for (const rel of ['app.js', 'wasm/fandhe_workshop_web.js', 'wasm/fandhe_workshop_web_bg.wasm']) {
      assert.ok(urls.includes(`${srv.url}${rel}`), `missing request: ${rel}`);
    }
    assert.ok(urls.includes(srv.url) || urls.includes(`${srv.url}index.html`), 'missing request: index');
    const wasm = rec.requests.find((r) => r.url.endsWith('.wasm'));
    assert.equal(wasm.status, 200);
    assert.equal(wasm.contentType, 'application/wasm');
    assert.equal(wasm.contentEncoding, 'gzip');

    // 配信する設定には試験用の口が無い。
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    await context.close();
  });
});
