// 再読み込み後の告知の縦断試験（TASK-322。SAVE-6・D-57・SHELL-21）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、処理中に再読み込みを 1 回・2 回挟み、右のログに「再読み込みしました」の
// 行（`data-kind="reloaded"`）が回数と同じだけ増えることを確かめる。処理は止まらず続く（SAVE-6）。
//   N-1 1 回: 行が 1 件。処理中のまま。完走の後も 1 件
//   N-2 2 回: 行が 2 件。`data-gen` は 1・2 の順。完走の後も 2 件
//   N-3 配信設定: 試験の口が無い
//
// TASK-536（SHELL-21）: 枠が二重に知らせないことも、N-1・N-2 の中で確かめる。再読み込みの運び手は右のログの行だけで、
// 枠の通知欄（`shell-notice`）に項目が増えず、読み上げに使われる欄（`role="status"`・`aria-live`）に
// 「再読み込みしました」が入らない。ページ内に MutationObserver を据え、その欄とその子孫への文字の変化を全件記録する。
// 「読み上げが 1 回だけ届く」は、現時点で観察できる範囲（行が回数と同数・他の欄に出ない）と、
// `terminal_area_contract` の native 試験（行が応答の読み上げの単位に数えられる）で固定する。
// 限界: ライブの欄（`live-response`）は実画面にまだ描かれておらず、その更新回数の観察は TASK-370、
// 支援技術での読まれ方は TASK-381 の範囲である。本試験は読み上げそのものの証拠ではない。
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
const NOTICE_ITEM = '[data-testid="shell-notice-item"]';
// 読み上げに使われる欄（aria-live="off" は読まれないので除く）。
const SPOKEN = '[role="status"], [role="alert"], [aria-live]:not([aria-live="off"])';

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

// 読み上げの欄の中で「再読み込みしました」を含む文字の変化を記録する観察者を据える。件数だけを返す口を持つ。
async function installSpokenObserver(page) {
  await page.evaluate(([spoken, notice]) => {
    const hits = [];
    const inSpoken = (node) => {
      const el = node.nodeType === 1 ? node : node.parentElement;
      return !!el && !!el.closest(spoken);
    };
    const obs = new MutationObserver((records) => {
      for (const r of records) {
        const nodes = [r.target, ...r.addedNodes];
        // 追加された部分木の内側にある読み上げの欄（外側の div ごと挿入された場合）も調べる。
        const inSubtree = [...r.addedNodes].some((n) => n.nodeType === 1
          && [...n.querySelectorAll(spoken)].some((el) => (el.textContent || '').includes(notice)));
        if (inSubtree || nodes.some((n) => inSpoken(n) && (n.textContent || '').includes(notice))) hits.push(r.type);
      }
    });
    obs.observe(document.documentElement, { subtree: true, childList: true, characterData: true, attributes: true });
    window.__spokenHits = hits;
  }, [SPOKEN, NOTICE]);
}

// 「再読み込みしました」を含む要素の置き場所を数える。右のログの行の外・読み上げの欄の中の件数が 0 でなければならない。
const noticeSites = (page) => page.evaluate(([spoken, notice, row, noticeItem]) => {
  const leaves = [...document.querySelectorAll('body *')].filter(
    (el) => (el.textContent || '').includes(notice) && ![...el.children].some((c) => (c.textContent || '').includes(notice)),
  );
  return {
    leaves: leaves.length,
    outsideRows: leaves.filter((el) => !el.closest(row)).length,
    inSpoken: leaves.filter((el) => el.closest(spoken)).length,
    noticeItems: document.querySelectorAll(noticeItem).length,
    noticeKinds: document.querySelectorAll('[data-testid="shell-notice"] [data-notice-kind], [data-testid="shell-notice"][data-notice-kind]').length,
    spokenHits: window.__spokenHits.length,
  };
}, [SPOKEN, NOTICE, ROW, NOTICE_ITEM]);

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
    await installSpokenObserver(page);
    assert.equal(await page.locator('[data-testid="shell-notice"]').count(), 1);
    await sendRaw(page, COMMAND);
    assert.deepEqual(await reloadedRows(page), []);
    assert.equal((await noticeSites(page)).leaves, 0);
    await reloadWhenTicks(page, 2, times);

    const rows = await reloadedRows(page);
    assert.equal(rows.length, times);
    rows.forEach((row, i) => {
      assert.equal(row.body, NOTICE);
      assert.equal(row.children, 0);
      assert.equal(row.gen, String(i + 1));
    });
    assert.equal(await page.locator(STATUS).getAttribute('data-state'), 'busy');
    // 枠は二重に知らせない（SHELL-21）: 文字の置き場所は右のログの行だけ（回数と同数）で、枠の通知欄は空のまま。
    const sites = await noticeSites(page);
    assert.deepEqual(sites, { leaves: times, outsideRows: 0, inSpoken: 0, noticeItems: 0, noticeKinds: 0, spokenHits: 0 });

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
    assert.deepEqual(await noticeSites(page), { leaves: times, outsideRows: 0, inSpoken: 0, noticeItems: 0, noticeKinds: 0, spokenHits: 0 });
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
