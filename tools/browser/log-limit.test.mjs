// 右のログの元の行が 4,096 件を超えたときの見え方の試験（TASK-454 2/2。TERM-35・FEND-22・D-46・D-47）。
//
// 試験の設定の画面で、既定の教材の拒否される入力（`/make-note Bad_Name`）を `raw-trigger` から送り続け、右のログの元の行
// （表示前の行。ログの `li` と畳んだ行の数の和）を 4,097 件より十分に増やす。拒否の入力は Step を変えないので全行が 1 つの
// (世代, Step) のグループに入り、`plan_fold` の最新グループの先頭を畳む段（`data-fold="head"`）だけが働く。確かめるのは次である。
//   (i) どの時点でもログの `li` が 1,000 件以下。畳みが始まった後はちょうど 1,000 件
//   (ii) `data-list-error`・`data-list-limit` が文書全体で 0 件（一覧の構築が `Err` にならず、安全弁にも当たらない）
//   (iii) 畳みが始まった後は `li[data-fold="head"]` がちょうど 1 件で ul の先頭にあり、`data-event` を持たない。文言は固定の文と
//         `data-fold-rows` の 3 桁区切りの数だけで、数は元の行の増えた分だけ増える
//   (iv) 要約行を除く `li` はすべて `data-event` を持ち、最後の `li` の `data-event` は入力のたびに増える（最新の側が隠れない）
//   (v) 元の行が 4,096 件の前後をまたぐ入力を 1 件ずつ送り、そのたびに (i)〜(iv) が保たれる
//   (vi) 全体を通して `#app` の子の取り除きが 0 回で、根・部品の枠・ログの ul が同じ要素のまま（FEND-15。全再描画なし）
//   (vii) CSP 違反・ページ内エラー・コンソールのエラー・安全性の観察が 0 件
// 右のログでは、`plan_fold` が先に 1,000 項目へ畳むため、4,096 件の安全弁（`keyed_view_or_notice` の `data-list-limit`）へ構造上
// 届かない。安全弁そのものは native の `crates/web/tests/list_limit.rs` と、左のビューの `artifact-limit` が守る。
// 呼び出し元は make test-browser（scripts/test-browser.sh）。1 件ごとに新しい context を使う。
//
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero, summarize,
} from './harness.mjs';

const BASE = '/fandhe-log-limit/';
const RAW = '[data-testid="raw-trigger"]';
const LOG_LIST = '[data-testid="part-log-list"]';
const LOG_PART = '[data-testid="part-log"]';
const OP = { input: { command: '/make-note Bad_Name' } };
const BATCH = 100;
const BATCH_UNTIL = 3_900; // これを超えたら 1 件ずつ送る
const TARGET_ROWS = 4_200;
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

const logLen = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine.logLen;

// ログの観察。値は件数・属性・固定の文言だけ。`rows` は元の行の数（畳んだ行 + 畳まずに出した行）。
const observe = (page) => page.evaluate((sel) => {
  const list = document.querySelector(sel);
  const items = [...list.querySelectorAll(':scope > li')];
  const folds = items.filter((li) => li.hasAttribute('data-fold'));
  const heads = items.filter((li) => li.getAttribute('data-fold') === 'head');
  const rowsAttr = heads[0]?.getAttribute('data-fold-rows') ?? null;
  const data = items.filter((li) => !li.hasAttribute('data-fold'));
  const foldRows = rowsAttr !== null && /^[0-9]+$/.test(rowsAttr) ? Number(rowsAttr) : 0;
  const lastEvent = data.length > 0 ? data[data.length - 1].getAttribute('data-event') : null;
  return {
    items: items.length,
    folds: folds.length,
    heads: heads.length,
    headFirst: heads.length === 1 && items[0] === heads[0],
    headHasEvent: heads.some((li) => li.hasAttribute('data-event')),
    rowsAttr,
    foldRows,
    headText: heads[0]?.textContent ?? null,
    dataWithoutEvent: data.filter((li) => !li.hasAttribute('data-event')).length,
    lastEvent: lastEvent !== null && /^[0-9]+$/.test(lastEvent) ? Number(lastEvent) : lastEvent,
    rows: foldRows + data.length,
    errors: document.querySelectorAll('[data-list-error]').length,
    limits: document.querySelectorAll('[data-list-limit]').length,
  };
}, LOG_LIST);

// 固定の文と、`data-fold-rows` の 3 桁区切りの数だけを照合する。
const headText = (rows) => `この Step の前の ${rows.toLocaleString('en-US')} 行を畳みました`;

function checkInvariants(c, label) {
  const ctx = JSON.stringify(c);
  assert.ok(c.items <= 1_000, `${label}: 1 リストが 1,000 項目を超えた: ${ctx}`);
  assert.equal(c.errors, 0, `${label}: data-list-error が出た: ${ctx}`);
  assert.equal(c.limits, 0, `${label}: data-list-limit が出た: ${ctx}`);
  assert.equal(c.dataWithoutEvent, 0, `${label}: 要約行以外に data-event の無い li がある: ${ctx}`);
  assert.equal(c.headHasEvent, false, `${label}: 要約行が data-event を持つ: ${ctx}`);
  assert.equal(c.folds, c.heads, `${label}: head 以外の畳みが出た: ${ctx}`);
  if (c.heads > 0) {
    assert.equal(c.heads, 1, `${label}: 要約行が 1 件でない: ${ctx}`);
    assert.ok(c.headFirst, `${label}: 要約行が先頭でない: ${ctx}`);
    assert.equal(c.items, 1_000, `${label}: 畳んだ後の項目数が 1,000 でない: ${ctx}`);
    assert.equal(c.headText, headText(c.foldRows), `${label}: 要約行の文言が違う: ${ctx}`);
  }
}

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
  }, [LOG_PART, LOG_LIST]);
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
  }, [LOG_PART, LOG_LIST]);
}

// 値は引数で渡し、コードの文字列を組み立てない。入力の受付（ログの増加）まで待つ。
async function send(page, times) {
  const before = await logLen(page);
  await page.evaluate(([sel, json, n]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    for (let i = 0; i < n; i += 1) el.click();
  }, [RAW, JSON.stringify(OP), times]);
  await page.waitForFunction(
    ([n, add]) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen >= n + add,
    [before, times],
    WAIT,
  );
}

test('右のログの元の行が 4,096 件を超えても、一覧は Err にならず 1,000 項目に畳まれ、全再描画は起きない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    await installGuards(page);

    let c = await observe(page);
    let maxItems = c.items;
    let prev = c;
    let sawFold = false;
    const seen = [c.rows];
    const crossings = [];

    while (c.rows < TARGET_ROWS) {
      const batch = c.rows < BATCH_UNTIL;
      await send(page, batch ? BATCH : 1);
      c = await observe(page);
      maxItems = Math.max(maxItems, c.items);
      checkInvariants(c, `rows=${c.rows}`);
      assert.ok(c.rows > prev.rows, `入力で元の行が増えていない: ${JSON.stringify([prev.rows, c.rows])}`);
      if (c.heads > 0) {
        sawFold = true;
        if (prev.heads > 0) {
          assert.equal(c.foldRows - prev.foldRows, c.rows - prev.rows, `畳んだ行の増えた分が元の行の増えた分と違う: ${JSON.stringify([prev.foldRows, c.foldRows, prev.rows, c.rows])}`);
        }
      }
      assert.ok(typeof c.lastEvent === 'number' && c.lastEvent > (prev.lastEvent ?? -1), `最新の行が進んでいない: ${JSON.stringify([prev.lastEvent, c.lastEvent])}`);
      if (!batch) {
        seen.push(c.rows);
        if (prev.rows <= 4_096 && c.rows >= 4_096) crossings.push([prev.rows, c.rows]);
      }
      prev = c;
    }

    assert.ok(sawFold, '畳みが一度も起きていない');
    assert.equal(maxItems, 1_000, `項目数の最大が 1,000 でない: ${maxItems}`);
    assert.ok(seen.some((r) => r <= 4_095), `4,095 件以下の観察が無い: ${JSON.stringify(seen)}`);
    assert.ok(seen.some((r) => r >= 4_097), `4,097 件以上の観察が無い: ${JSON.stringify(seen)}`);
    assert.ok(crossings.length >= 1, `4,096 件をまたぐ 1 件の入力が観察できていない: ${JSON.stringify(seen)}`);

    const g = await readGuards(page);
    assert.deepEqual(g, { swaps: 0, root: true, part: true, list: true }, `全再描画が起きた: ${JSON.stringify(g)}`);

    const safety = await collectSafety(page, rec);
    assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
    const s = summarize(rec);
    assert.equal(s.csp + s.pageErrors + s.consoleErrors, 0, JSON.stringify(rec));
    await context.close();
  });
});
