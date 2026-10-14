// 解釈の前の入れ子の深さの検査が wasm の中で動くことの試験（TASK-157 / SAFE-20）。
//
// 10,000 段の入れ子でもスタックを使い切らず、1 秒未満で理由 too_deep が返り、ページ内エラーが出ないことを確かめる。
// 画面の IR 読み込みの経路は対象外（読み込み口は TASK-154・155、画面は TASK-545）。
//
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。
// 未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted } from './harness.mjs';

const BASE = '/fandhe-json-depth/';
let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(
    dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(),
    'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1',
  );
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

test('64 段は通り、65 段・10,000 段は too_deep（1 秒未満・ページ内エラー 0 件）', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);

    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.checkJsonDepth), 'function', 'test hooks are missing');
    const run = (n) =>
      page.evaluate((depth) => {
        const text = '['.repeat(depth) + ']'.repeat(depth);
        const t0 = performance.now();
        const out = window.__fandheTest.checkJsonDepth(text);
        return { out: JSON.parse(out), ms: performance.now() - t0 };
      }, n);

    assert.deepEqual((await run(64)).out, { ok: true });
    assert.deepEqual((await run(65)).out, { ok: false, reason: 'too_deep' });
    const deep = await run(10000);
    assert.deepEqual(deep.out, { ok: false, reason: 'too_deep' });
    assert.ok(deep.ms < 1000, `10,000 levels took ${deep.ms}ms`);

    // 呼び出しの後もページが応答する。
    const state = JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson()));
    assert.equal(state.mounted, true);

    const s = summarize(rec);
    const detail = JSON.stringify(rec);
    assert.equal(s.csp, 0, detail);
    assert.equal(s.pageErrors, 0, detail);
    assert.equal(s.consoleErrors, 0, detail);
    await context.close();
  });
});
