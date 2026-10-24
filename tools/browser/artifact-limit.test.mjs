// 左の成果物の 4,096 項目の安全弁と、古い側の畳みが、実画面で働くことの試験（TASK-484 2/2。ART-14・FEND-22・D-46）。
//
// 試験の設定の画面を、世界状態の経路を開始状態のログへ積んだ教材（`src/test_fixture.rs` の `mount_seed_log`）で開く。
// 経路はどれも今の Step の印で積むので 1 本のランになり、畳まれずに安全弁まで届く（`artifact-fold` だけが古い Step の印を持つ）。
// 確かめること:
//   (A) 4,095・4,096 件では状態の行が出ず、4,097 件では先頭 4,095 件のデータ行と状態の行 1 件（`li[data-list-limit]`）になる。
//       文言は「上限超過: 先頭 4,095 件だけ表示（全 4,097 件）」で、状態の行は `data-event` を持たず、親の `ul` に `data-list-limit` と
//       `data-bind-list` が残る。`data-list-error` と `data-fold` は 0 件
//   (B) 境目をまたぐ入力（4,095 → 4,096 と 4,096 → 4,097）の前後で、`#app` の子の取り除き（root の全置換）が 0 回で、
//       部品の枠と選ばれたビューの親の `ul` が同じ要素のまま
//   (C) 古い Step の行が 1,100 件ある教材では、`li` が 1,000 件以下に畳まれ、入力の前後でも root が置き換わらない
//   (D) CSP 違反・ページ内エラー・コンソールのエラー・安全性の観察が 0 件
// 表示件数は「データ行 4,095 件 + 状態の行 1 件 = 4,096 項目」である（上流の keyed list の上限。`docs/testing.md` 5.165・5.294）。
// 呼び出し元は make test-browser（scripts/test-browser.sh）。1 件ごとに新しい context を使う。
//
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero, summarize,
} from './harness.mjs';

const BASE = '/fandhe-artifact-limit/';
const RAW = '[data-testid="raw-trigger"]';
const PART = '[data-testid="part-artifact-view"]';
const LIST = '#app [data-bind-list^="view_"]:not([data-bind-list="view_select"]):not([hidden])';
const NOTICE = '上限超過: 先頭 4,095 件だけ表示（全 4,097 件）';
const OP = { input: { command: '/make-note my-note' } };
const WAIT = { timeout: 60000 };

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

// 選ばれたビューの list（hidden でないもの）の観察。値は固定の文言と件数だけ。
const observe = (page) => page.evaluate(([sel, notice]) => {
  const lists = document.querySelectorAll(sel);
  if (lists.length !== 1) return { lists: lists.length };
  const ul = lists[0];
  const items = [...ul.querySelectorAll(':scope > li')];
  const rows = items.filter((li) => li.hasAttribute('data-path'));
  const limits = items.filter((li) => li.hasAttribute('data-list-limit'));
  return {
    lists: 1,
    items: items.length,
    rows: rows.length,
    limitRows: limits.length,
    limitIsLast: limits.length === 1 && items[items.length - 1] === limits[0],
    limitValue: limits[0]?.getAttribute('data-list-limit') ?? null,
    limitText: limits[0]?.textContent ?? null,
    limitHasEvent: limits.some((li) => li.hasAttribute('data-event')),
    ulLimit: ul.getAttribute('data-list-limit'),
    ulBind: ul.getAttribute('data-bind-list'),
    errors: document.querySelectorAll('[data-list-error]').length,
    folds: ul.querySelectorAll(':scope > li[data-fold]').length,
    foldWithPath: ul.querySelectorAll(':scope > li[data-fold][data-path]').length,
    firstPath: rows[0]?.getAttribute('data-path') ?? null,
    lastPath: rows[rows.length - 1]?.getAttribute('data-path') ?? null,
    noticeSeen: limits.some((li) => li.textContent === notice),
  };
}, [LIST, NOTICE]);

const logLen = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine.logLen;

// #app の子の取り除き（root の全置換）の計数器と、保持する要素の控え。
function installGuards(page) {
  return page.evaluate(([part, list]) => {
    window.__rootSwaps = 0;
    const mo = new MutationObserver((records) => {
      for (const r of records) if (r.removedNodes.length > 0) window.__rootSwaps += 1;
    });
    mo.observe(document.getElementById('app'), { childList: true, subtree: false });
    window.__swapObserver = mo;
    window.__pinned = {
      root: document.querySelector('#app > *'),
      part: document.querySelector(part),
      list: document.querySelector(list),
    };
  }, [PART, LIST]);
}

function readGuards(page) {
  return page.evaluate(([part, list]) => {
    for (const r of window.__swapObserver.takeRecords()) if (r.removedNodes.length > 0) window.__rootSwaps += 1;
    const p = window.__pinned;
    return {
      swaps: window.__rootSwaps,
      root: p.root === document.querySelector('#app > *') && p.root.isConnected,
      part: p.part === document.querySelector(part) && p.part.isConnected,
      list: p.list === document.querySelector(list) && p.list.isConnected,
    };
  }, [PART, LIST]);
}

// 値は引数で渡し、コードの文字列を組み立てない。入力の受付（ログの増加）まで待つ。
async function sendOp(page) {
  const before = await logLen(page);
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(OP)]);
  await page.waitForFunction(
    (n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n,
    before,
    WAIT,
  );
}

async function open(srv, fixture) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  await page.goto(`${srv.url}?fixture=${fixture}`);
  await waitMounted(page);
  // 4,000 件超の描画を待つ。
  await page.waitForFunction((sel) => document.querySelectorAll(`${sel} > li`).length > 0, LIST, WAIT);
  return { page, context, rec };
}

async function finish(page, context, rec) {
  const safety = await collectSafety(page, rec);
  assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
  const s = summarize(rec);
  assert.equal(s.csp + s.pageErrors + s.consoleErrors, 0, JSON.stringify(rec));
  await context.close();
}

const noNotice = (o, items) => {
  assert.equal(o.lists, 1, JSON.stringify(o));
  assert.equal(o.items, items, JSON.stringify(o));
  assert.equal(o.rows, items, JSON.stringify(o));
  assert.equal(o.limitRows, 0, JSON.stringify(o));
  assert.equal(o.ulLimit, null, JSON.stringify(o));
  assert.equal(o.errors, 0, JSON.stringify(o));
  assert.equal(o.folds, 0, JSON.stringify(o));
};

test('A-4095: 4,095 項目では状態の行が出ない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, 'artifact-limit-4095');
    noNotice(await observe(page), 4_095);
    await finish(page, context, rec);
  });
});

test('A-4096: 4,096 項目ちょうどでは状態の行が出ない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, 'artifact-limit-4096');
    noNotice(await observe(page), 4_096);
    await finish(page, context, rec);
  });
});

test('A-4097: 4,097 項目では先頭 4,095 件と状態の行になり、親は data-bind-list を保つ', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, 'artifact-limit-4097');
    const o = await observe(page);
    assert.equal(o.lists, 1, JSON.stringify(o));
    assert.equal(o.items, 4_096, JSON.stringify(o));
    assert.equal(o.rows, 4_095, JSON.stringify(o));
    assert.equal(o.limitRows, 1, JSON.stringify(o));
    assert.equal(o.limitIsLast, true, JSON.stringify(o));
    assert.equal(o.limitValue, '4097');
    assert.equal(o.limitText, NOTICE);
    assert.equal(o.noticeSeen, true);
    assert.equal(o.limitHasEvent, false, '状態の行が data-event を持つ');
    assert.equal(o.ulLimit, '4097');
    assert.match(o.ulBind ?? '', /^view_/);
    assert.equal(o.errors, 0, JSON.stringify(o));
    assert.equal(o.folds, 0, JSON.stringify(o));
    // 積んだ経路は p/0000 から p/4096。辞書順の先頭 4,095 件は p/0000 から p/4094。
    assert.equal(o.firstPath, 'p/0000');
    assert.equal(o.lastPath, 'p/4094');
    // 選ばれていない list は空で、状態の行も印も持たない。
    const others = await page.evaluate(() => [...document.querySelectorAll('#app [data-bind-list^="view_"][hidden]')].map((ul) => ({
      items: ul.querySelectorAll(':scope > li').length,
      limit: ul.hasAttribute('data-list-limit'),
      error: ul.hasAttribute('data-list-error'),
    })));
    assert.ok(others.length >= 1);
    for (const x of others) assert.deepEqual(x, { items: 0, limit: false, error: false });
    await finish(page, context, rec);
  });
});

test('B: 上限の境目をまたぐ入力で root が置き換わらず、状態の行が現れる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    // 4,095 -> 4,096: 状態の行は出ないまま。
    {
      const { page, context, rec } = await open(srv, 'artifact-limit-4095');
      await installGuards(page);
      await sendOp(page);
      await page.waitForFunction((sel) => document.querySelectorAll(`${sel} > li`).length === 4_096, LIST, WAIT);
      noNotice(await observe(page), 4_096);
      assert.deepEqual(await readGuards(page), { swaps: 0, root: true, part: true, list: true });
      await finish(page, context, rec);
    }
    // 4,096 -> 4,097: 状態の行が現れ、先頭が入れ替わる（notes/ は p/ より辞書順で前）。
    {
      const { page, context, rec } = await open(srv, 'artifact-limit-4096');
      await installGuards(page);
      await sendOp(page);
      await page.waitForFunction((sel) => document.querySelectorAll(`${sel} > li[data-list-limit]`).length === 1, LIST, WAIT);
      const o = await observe(page);
      assert.equal(o.items, 4_096, JSON.stringify(o));
      assert.equal(o.rows, 4_095, JSON.stringify(o));
      assert.equal(o.limitIsLast, true);
      assert.equal(o.limitText, NOTICE);
      assert.equal(o.ulLimit, '4097');
      assert.equal(o.errors, 0);
      assert.equal(o.folds, 0);
      assert.notEqual(o.firstPath, 'p/0000', '入力で増えた経路が先頭に来ていない');
      assert.deepEqual(await readGuards(page), { swaps: 0, root: true, part: true, list: true });
      await finish(page, context, rec);
    }
  });
});

test('C: 古い Step の行が 1,000 件を超える教材では要約へ畳まれ、入力の前後で root が置き換わらない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, 'artifact-fold');
    await installGuards(page);
    const check = (o) => {
      assert.equal(o.lists, 1, JSON.stringify(o));
      assert.ok(o.items <= 1_000, JSON.stringify(o));
      assert.ok(o.folds >= 1, JSON.stringify(o));
      assert.equal(o.foldWithPath, 0, '要約行が data-path を持つ');
      assert.equal(o.limitRows, 0, JSON.stringify(o));
      assert.equal(o.errors, 0, JSON.stringify(o));
    };
    check(await observe(page));
    await sendOp(page);
    await page.waitForFunction((sel) => [...document.querySelectorAll(`${sel} > li[data-path]`)].some((li) => li.getAttribute('data-path').startsWith('notes/')), LIST, WAIT);
    check(await observe(page));
    assert.deepEqual(await readGuards(page), { swaps: 0, root: true, part: true, list: true });
    await finish(page, context, rec);
  });
});
