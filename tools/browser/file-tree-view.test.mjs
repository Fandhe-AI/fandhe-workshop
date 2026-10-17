// ファイルツリーのビュー、変種 (a)・(b) の試験（TASK-473。ART-1・ART-2・ART-3・ART-9・ART-13）。
//
// 代役の部品（view_fixture）が file_tree_view で描くファイルツリー (a)・(b) を、検査器（artifact-view-check.mjs）で流す。
// - FT1: 前後 2 往復で違反 0、行も項目の内側も置き換わらない（保持が前の行数に等しい）、状態の数は 5
// - FT2: 前と後の各状態で、選ばれた list が role=tree、全 li が treeitem と木の属性を持ち、ラベルが item_text、
//        aria-selected・aria-hidden・data-value は 0 件。項目数と変化の印の数が期待値（目で数えた値）と一致する
// - FT3: 外部へのリクエスト・ページのエラー・ダイアログがどれも 0 件
// - FT4: 対照。ファイルツリー (a) では親に role が無く、treeitem も 0 件
// - FT5: (a) で前後 2 往復の違反 0、行も項目の内側も置き換わらない
// - FT6: (a) の前後それぞれで、項目数・rich_text の数・印の宿主（li には無く内側の div）・印の数・role/aria/svg が 0 件
// - FT7: (a) の前後の切り替えで、外部へのリクエスト・ページのエラー・ダイアログが 0 件
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { runViewCase } from './artifact-view-check.mjs';

const BASE = '/fandhe-file-tree-view/';
const ROOT = '#view-fixture-root';
const LIST = `${ROOT} [data-testid="view-list"][data-view="file_tree"]`;
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

async function withFixturePage(fn) {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal(await page.evaluate(() => typeof window.__fandheTest?.viewFixtureMount), 'function', 'test hooks are missing');
      await page.evaluate(() => {
        const box = document.createElement('div');
        box.id = 'view-fixture-root';
        document.body.append(box);
      });
      const res = JSON.parse(await page.evaluate(() => window.__fandheTest.viewFixtureMount('view-fixture-root')));
      assert.deepEqual(res, { ok: true });
      await fn(page, rec);
    } finally {
      await context.close();
    }
  });
}

function fixtureAdapter(page) {
  const dispatch = async (name, payload) => {
    const r = JSON.parse(await page.evaluate(([n, p]) => window.__fandheTest.viewFixtureDispatch(n, p), [name, payload]));
    return { ok: r.outcome === 'applied' };
  };
  return {
    select: (kind, variant) => dispatch('fixture_select', `${kind}:${variant}`),
    phase: (name) => dispatch('fixture_phase', name),
    state: () => page.evaluate(() => window.__fandheTest.viewFixtureState()),
    keyOf: (id) => `k${id}`,
  };
}

// 目で数えた値（testdata/artifact-view-parts/test-data/sample/filetree.json）。
const COUNTS = { before: 10, after: 11 };

function observeTree(page) {
  return page.evaluate((sel) => {
    const list = document.querySelector(sel);
    const items = [...list.querySelectorAll('li')];
    return {
      role: list.getAttribute('role'),
      items: items.length,
      treeitems: items.filter((li) => li.getAttribute('role') === 'treeitem').length,
      withAttrs: items.filter((li) => ['aria-level', 'aria-posinset', 'aria-setsize'].every((a) => li.hasAttribute(a))).length,
      itemText: list.querySelectorAll('[data-part="item-text"]').length,
      banned: list.querySelectorAll('[aria-selected], [aria-hidden], [data-value], [role="group"]').length,
      expandedFalse: list.querySelectorAll('[aria-expanded="false"]').length,
      marks: items.filter((li) => li.getAttribute('data-mark') !== '').length,
      marked: items.filter((li) => li.getAttribute('data-mark') !== null).length,
    };
  }, LIST);
}

test('FT1: 前後 2 往復で違反 0、行も内側も置き換わらない', async () => {
  await withFixturePage(async (page, rec) => {
    const { violations, stats } = await runViewCase(page, fixtureAdapter(page), {
      kind: 'file_tree', variant: 'b', rootSelector: ROOT, rounds: 2, deep: true,
    });
    const counts = await collectSafety(page, rec);
    const detail = JSON.stringify({ violations, stats, counts, rec });
    const { before: b, after: a } = COUNTS;
    assert.deepEqual(violations, [], detail);
    assert.equal(stats.states, 5, detail);
    assert.deepEqual(stats.itemCounts, [b, a, b, a, b], detail);
    assert.deepEqual(stats.preserved, [b, b, b, b], detail);
    assert.deepEqual(stats.innerPreserved, [b, b, b, b], detail);
    assert.deepEqual(nonZero(counts), [], detail);
  });
});

test('FT2: 前後それぞれで、木の属性・item_text・項目数・印の数が期待どおり', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('file_tree', 'b')).ok);
    for (const [phase, n, marks] of [['before', COUNTS.before, 0], ['after', COUNTS.after, 4], ['before', COUNTS.before, 0]]) {
      assert.ok((await adapter.phase(phase)).ok);
      const o = await observeTree(page);
      const detail = JSON.stringify({ phase, o });
      assert.equal(o.role, 'tree', detail);
      assert.equal(o.items, n, detail);
      assert.equal(o.treeitems, n, detail);
      assert.equal(o.withAttrs, n, detail);
      assert.equal(o.itemText, n, detail);
      assert.equal(o.banned, 0, detail);
      assert.equal(o.expandedFalse, 0, detail);
      assert.equal(o.marked, n, detail);
      assert.equal(o.marks, marks, detail);
    }
  });
});

test('FT3: 外部へのリクエスト・ページのエラー・ダイアログは 0 件', async () => {
  await withFixturePage(async (page, rec) => {
    const adapter = fixtureAdapter(page);
    for (const phase of ['after', 'before', 'after']) assert.ok((await adapter.phase(phase)).ok);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});

test('FT4 対照: ファイルツリー (a) では親に role が無く、treeitem も 0 件', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('file_tree', 'a')).ok);
    const o = await observeTree(page);
    assert.equal(o.role, null, JSON.stringify(o));
    assert.equal(o.treeitems, 0, JSON.stringify(o));
    assert.equal(o.itemText, 0, JSON.stringify(o));
  });
});

test('FT5: 変種 (a) の前後 2 往復で違反 0、行も内側も置き換わらない', async () => {
  await withFixturePage(async (page, rec) => {
    const { violations, stats } = await runViewCase(page, fixtureAdapter(page), {
      kind: 'file_tree', variant: 'a', rootSelector: ROOT, rounds: 2, deep: true,
    });
    const counts = await collectSafety(page, rec);
    const detail = JSON.stringify({ violations, stats, counts, rec });
    const { before: b, after: a } = COUNTS;
    assert.deepEqual(violations, [], detail);
    assert.equal(stats.states, 5, detail);
    assert.deepEqual(stats.itemCounts, [b, a, b, a, b], detail);
    assert.deepEqual(stats.preserved, [b, b, b, b], detail);
    assert.deepEqual(stats.innerPreserved, [b, b, b, b], detail);
    assert.deepEqual(nonZero(counts), [], detail);
  });
});

function observeA(page) {
  return page.evaluate((sel) => {
    const list = document.querySelector(sel);
    const items = [...list.querySelectorAll(':scope > li')];
    return {
      items: items.length,
      rich: list.querySelectorAll('.fw-wire-rich-text').length,
      liMarks: items.filter((li) => li.hasAttribute('data-mark')).length,
      divHosts: list.querySelectorAll('li > div[data-mark]').length,
      marks: list.querySelectorAll('li > div[data-mark]:not([data-mark=""])').length,
      forbidden: list.querySelectorAll('[role], [aria-hidden], svg, [data-part]').length + (list.hasAttribute('role') ? 1 : 0),
    };
  }, LIST);
}

test('FT6: 変種 (a) の前後それぞれで、項目数・rich_text・印の宿主・印の数が期待どおり', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('file_tree', 'a')).ok);
    for (const [phase, n, marks] of [['before', COUNTS.before, 0], ['after', COUNTS.after, 4], ['before', COUNTS.before, 0]]) {
      assert.ok((await adapter.phase(phase)).ok);
      const o = await observeA(page);
      const detail = JSON.stringify({ phase, o });
      assert.equal(o.items, n, detail);
      assert.equal(o.rich, n, detail);
      assert.equal(o.liMarks, 0, detail);
      assert.equal(o.divHosts, n, detail);
      assert.equal(o.marks, marks, detail);
      assert.equal(o.forbidden, 0, detail);
    }
  });
});

test('FT7: 変種 (a) の切り替えで外部へのリクエスト・ページのエラー・ダイアログは 0 件', async () => {
  await withFixturePage(async (page, rec) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('file_tree', 'a')).ok);
    for (const phase of ['after', 'before', 'after']) assert.ok((await adapter.phase(phase)).ok);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});
