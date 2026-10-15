// keydown の属性（data-action-keydown・data-keys）が届くことの試験（TASK-346 / FEND-1）。
//
// 上流の wasm-full は feature action-keydown が無いと keydown の配線を登録せず、属性がエラーも出さずに効かなくなる。
// この試験は試験の設定（test-hooks）の probe に Escape を送り、受信回数が増えることで配線を確かめる。
// scripts/test-keydown-control.sh は feature を外したビルドでこの試験を流し、固定の印
// ACTION-KEYDOWN-NOT-DISPATCHED で失敗することを確かめる（対照）。
//
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。
// 未設定・不在は skip にせず失敗にする。配信設定（FANDHE_WEB_DIR）は使わない（probe を含まない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted } from './harness.mjs';

const BASE = '/fandhe-keydown/';
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

test('data-action-keydown が Escape だけを受け取り、CSP 違反・ページ内エラーが 0 件', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);

    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    const count = async () => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).keydownProbe;
    assert.equal(await count(), 0);

    await page.focus('[data-testid="keydown-probe"]');
    await page.keyboard.press('Escape');
    const afterEscape = await count();
    assert.equal(
      afterEscape,
      1,
      `ACTION-KEYDOWN-NOT-DISPATCHED: Escape did not reach the action (keydownProbe=${afterEscape}); is feature action-keydown enabled?`,
    );

    // data-keys で絞り込めている（Escape 以外は届かない）。
    await page.keyboard.press('Enter');
    assert.equal(await count(), 1, 'a key outside data-keys reached the action');

    const s = summarize(rec);
    const detail = JSON.stringify(rec);
    assert.equal(s.csp, 0, detail);
    assert.equal(s.pageErrors, 0, detail);
    assert.equal(s.consoleErrors, 0, detail);
    await context.close();
  });
});
