// IR・履歴の読み込みの判定の順序（大きさ → 入れ子の深さ → JSON の解釈）が wasm の中で守られることの試験（TASK-157 / SAFE-20）。
//
// IR と履歴の両方で、深さの検査が解釈より前・大きさの検査が深さより前に働くことを確かめる。
// 型・内容の段は native の試験（crates/core/tests/load_order.rs）が持つ。画面の読み込みの経路は対象外（TASK-154 は ir-size-limit.test.mjs、TASK-155・545）。
//
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。
// 未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted } from './harness.mjs';

const BASE = '/fandhe-json-load-order/';
// contract の MAX_IR_BYTES・MAX_HISTORY_BYTES。
const LIMIT = { ir: 20_971_520, history: 10_485_760 };
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

test('IR・履歴の両方で、大きさ → 深さ → 解釈の順に断る（ページ内エラー 0 件）', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);

    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.checkJsonLoad), 'function', 'test hooks are missing');
    const run = (entry, kind, n) =>
      page.evaluate(
        ([entry, kind, n, limit]) => {
          const open = '['.repeat(n);
          const close = ']'.repeat(n);
          let text;
          if (kind === 'valid') text = open + close;
          else if (kind === 'unclosed') text = open;
          else if (kind === 'padded') text = (open + close).padEnd(limit + 1, ' ');
          else text = '{"a":';
          const t0 = performance.now();
          const out = window.__fandheTest.checkJsonLoad(entry, text);
          return { out: JSON.parse(out), ms: performance.now() - t0 };
        },
        [entry, kind, n, LIMIT[entry]],
      );

    for (const entry of ['ir', 'history']) {
      assert.deepEqual((await run(entry, 'valid', 64)).out, { ok: true }, entry);
      assert.deepEqual((await run(entry, 'valid', 65)).out, { ok: false, stage: 'depth', reason: 'too_deep' }, entry);
      const deep = await run(entry, 'valid', 10000);
      assert.deepEqual(deep.out, { ok: false, stage: 'depth', reason: 'too_deep' }, entry);
      assert.ok(deep.ms < 1000, `${entry}: 10,000 levels took ${deep.ms}ms`);
      // 深すぎ かつ 不正な JSON は、解釈ではなく深さで断る。
      assert.deepEqual((await run(entry, 'unclosed', 65)).out, { ok: false, stage: 'depth', reason: 'too_deep' }, entry);
      // 深すぎ かつ 大きすぎは、大きさで断る。
      assert.deepEqual((await run(entry, 'padded', 65)).out, { ok: false, stage: 'size', reason: 'too_large' }, entry);
      // 浅い不正な JSON は解釈で断る。
      assert.deepEqual((await run(entry, 'broken', 0)).out, { ok: false, stage: 'json_parse' }, entry);
    }

    const unknown = await page.evaluate(() => JSON.parse(window.__fandheTest.checkJsonLoad('other', '[]')));
    assert.deepEqual(unknown, { ok: false, error: 'unknown_entry' });

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
