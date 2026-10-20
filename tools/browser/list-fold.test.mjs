// 1 リストが約 1,000 項目を超えないよう古い側が畳まれることの試験（TASK-360 1/2。FEND-22・D-46・PoC-8 の手順 5）。
//
// 試験の設定の画面で、既定の教材の拒否される入力（`/make-note Bad_Name`）を `raw-trigger` から送り続け、
// 右のログの表示前の行を 1,000 件より十分に増やす。確かめるのは次である。
// (i) 途中のどの時点でも、ログの `li`（データ行と要約行）が 1,000 件以下
// (ii) 畳みが始まった後は `li[data-fold]` が 1 件以上あり、要約行は `data-event` を持たない
// (iii) `data-list-error` が 0 件（上流の拒否・安全弁に当たらない）
// (iv) 保持したログの親 ul と部品の枠が同一の要素のまま（root の子の全置換が起きない。FEND-15）
// (v) CSP 違反・ページ内エラー・コンソールのエラー・安全性の観察が 0 件
// 復習の 1,000 件超は、IR から現実的に作れないため native の試験（`crates/web/tests/list_fold.rs`）で守る。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero, summarize,
} from './harness.mjs';

const BASE = '/fandhe-list-fold/';
const RAW = '[data-testid="raw-trigger"]';
const LOG_LIST = '[data-testid="part-log-list"]';
const LOG_PART = '[data-testid="part-log"]';
const OP = { input: { command: '/make-note Bad_Name' } };
// 1 回の入力で行が 2 件増える。1,000 件を十分に超えるまで送る。
const TARGET_EVENTS = 1_300;
const BATCH = 50;

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const logLen = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine.logLen;

async function sendMany(page, n) {
  await page.evaluate(([sel, json, times]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    for (let i = 0; i < times; i += 1) el.click();
  }, [RAW, JSON.stringify(OP), n]);
}

const counts = (page) => page.evaluate((sel) => {
  const list = document.querySelector(sel);
  return {
    items: list.querySelectorAll(':scope > li').length,
    folds: list.querySelectorAll(':scope > li[data-fold]').length,
    foldWithEvent: list.querySelectorAll(':scope > li[data-fold][data-event]').length,
    errors: document.querySelectorAll('[data-list-error]').length,
  };
}, LOG_LIST);

test('ログが 1,000 項目を超える入力を続けても、1 リストは 1,000 項目以下に畳まれ、親は同じ要素のままである', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    await page.evaluate(([list, part]) => {
      window.__pinned = { list: document.querySelector(list), part: document.querySelector(part) };
    }, [LOG_LIST, LOG_PART]);

    let firstFoldAt = null;
    let maxItems = 0;
    while ((await logLen(page)) < TARGET_EVENTS) {
      const before = await logLen(page);
      await sendMany(page, BATCH);
      const after = await logLen(page);
      assert.ok(after > before, '入力でログが増えていない');
      const c = await counts(page);
      maxItems = Math.max(maxItems, c.items);
      assert.ok(c.items <= 1_000, `1 リストが 1,000 項目を超えた: ${JSON.stringify(c)} logLen=${after}`);
      assert.equal(c.errors, 0, `上限の通知が出た: ${JSON.stringify(c)}`);
      assert.equal(c.foldWithEvent, 0, '要約行が data-event を持つ');
      if (firstFoldAt === null && c.folds > 0) firstFoldAt = after;
      if (firstFoldAt !== null) assert.ok(c.folds >= 1, `畳みが消えた: ${JSON.stringify(c)}`);
    }
    assert.notEqual(firstFoldAt, null, '畳みが一度も起きていない');
    assert.ok(maxItems > 100, `行が十分に増えていない: ${maxItems}`);

    const same = await page.evaluate(([list, part]) => ({
      list: window.__pinned.list === document.querySelector(list) && window.__pinned.list.isConnected,
      part: window.__pinned.part === document.querySelector(part) && window.__pinned.part.isConnected,
    }), [LOG_LIST, LOG_PART]);
    assert.deepEqual(same, { list: true, part: true }, '親または部品の枠が置き換わった（全再描画）');

    const safety = await collectSafety(page, rec);
    assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
    const s = summarize(rec);
    assert.equal(s.csp + s.pageErrors + s.consoleErrors, 0, JSON.stringify(rec));
    await context.close();
  });
});
