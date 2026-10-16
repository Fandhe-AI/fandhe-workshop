// 再読み込み後の告知の縦断試験（TASK-322。SAVE-6・D-57・SHELL-21）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、処理中に再読み込みを 1 回・2 回挟み、右のログに「再読み込みしました」の
// 行（`data-kind="reloaded"`）が回数と同じだけ増えることを確かめる。処理は止まらず続く（SAVE-6）。
//   N-1 1 回: 行が 1 件。処理中のまま。完走の後も 1 件
//   N-2 2 回: 行が 2 件。`data-gen` は 1・2 の順。完走の後も 2 件
//   N-3 配信設定: 試験の口が無い
//
// 注意: 実際の `page.reload()` ではなく、ページ内の模擬の再読み込みである（reload-ignore.test.mjs と同じ理由。
// 保存した状態の復元の結線は TASK-319 で、#343 の後に実際の再読み込みの場面を足せる）。
// 時機の待ちは時間ではなく進行の段数で決める。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-reload-notice/';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const ROW = `${LOG} > li[data-kind="reloaded"]`;
const STATUS = '[data-testid="run-status"]';
const COMMAND = { input: { command: '/make-note first-note' } };
// ホストの固定の合図（crates/web/src/host.rs の RAW_RELOAD と同じ文字列）。
const RELOAD_PAYLOAD = '{"reload":{}}';
const NOTICE = '再読み込みしました';
const TOTAL_TICKS = 5;

let browser;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;

// 進行がちょうど n 段になった瞬間に、同じ同期区間で再読み込みの合図を times 回送る。
// 段を飛び越えた場合・上限時間を超えた場合は、ページの中でエラーにして失敗にする。
async function reloadWhenTicks(page, n, times, timeout = 8000) {
  const res = await page.evaluate(([sel, count, repeat, limit, payload]) => new Promise((resolve) => {
    const el = document.querySelector(sel);
    const len = () => JSON.parse(window.__fandheTest.stateJson()).engine.ticks.length;
    const t0 = performance.now();
    const poll = () => {
      const l = len();
      if (l === count) {
        for (let i = 0; i < repeat; i += 1) {
          el.setAttribute('data-payload', payload);
          el.click();
        }
        resolve({});
        return;
      }
      if (l > count) {
        resolve({ error: `ticks skipped past ${count}: ${l}` });
        return;
      }
      if (performance.now() - t0 > limit) {
        resolve({ error: `timeout waiting for ${count} ticks: ${l}` });
        return;
      }
      setTimeout(poll, 2);
    };
    poll();
  }), [RAW, n, times, timeout, RELOAD_PAYLOAD]);
  assert.ok(!res.error, res.error);
}

async function waitUntil(page, what, pred, timeout = 8000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await engine(page);
    if (pred(last)) return last;
    await page.waitForTimeout(25);
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

const reloadedRows = (page) => page.evaluate((sel) => [...document.querySelectorAll(sel)].map((li) => ({
  gen: li.getAttribute('data-gen'),
  body: li.querySelector('[data-testid="line-body"]')?.textContent,
  children: li.querySelector('[data-testid="line-body"]')?.children.length,
})), ROW);

async function openTestPage(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

async function assertSafe(page, rec) {
  const counts = await collectSafety(page, rec);
  assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
}

async function runOnce(times) {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    await sendRaw(page, COMMAND);
    assert.deepEqual(await reloadedRows(page), []);
    await reloadWhenTicks(page, 2, times);

    const rows = await reloadedRows(page);
    assert.equal(rows.length, times);
    rows.forEach((row, i) => {
      assert.equal(row.body, NOTICE);
      assert.equal(row.children, 0);
      assert.equal(row.gen, String(i + 1));
    });
    assert.equal(await page.locator(STATUS).getAttribute('data-state'), 'busy');

    // 処理は続き、完走の後（古いタイマーの無視を含む）も reloaded の行は回数のまま。
    const done = await waitUntil(
      page,
      'processing completes',
      (e) => e.ticks.length >= TOTAL_TICKS && e.stateWithoutLog.run.busy === null,
    );
    await page.waitForTimeout(700);
    assert.equal((await reloadedRows(page)).length, times);
    assert.equal((await engine(page)).log.filter((r) => r.kind === 'reloaded').length, times);
    assert.ok(done.ticks.length >= TOTAL_TICKS);
    assert.notEqual(await page.locator(STATUS).getAttribute('data-state'), 'busy');
    await assertSafe(page, rec);
    await context.close();
  });
}

test('N-1 処理中の再読み込み 1 回で、右のログに再読み込みの行が 1 件増え、処理は続く', async () => {
  await runOnce(1);
});

test('N-2 処理中の再読み込み 2 回で、右のログに再読み込みの行が 2 件増える', async () => {
  await runOnce(2);
});

test('N-3 配信設定には試験の口が無い', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    assert.equal(await page.locator(RAW).count(), 0);
    await context.close();
  });
});
