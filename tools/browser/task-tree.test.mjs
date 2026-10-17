// タスクツリーの変種 (a)・(b) の試験（TASK-474。ART-1・ART-9・ART-13・A11Y-22）。
//
// 代役の部品が (b) で本体（task_tree）を通ることを、検査器の deep 検査と固定の数え上げで確かめる。
// - P1: task_tree (b) を前後 2 往復し、違反 0、項目数 [5,6,5,6,5]、行と内側の保持が前の行数に等しい
// - P2: 前後それぞれで、親の role=tree が 1、treeitem が項目数、木の 3 属性が全項目、aria-expanded が前 1・後 2、
//   状態の記号と文字が全項目で空でない。期待値は試料の JSON から Node 側で数える
// - C1: 負の対照。1 項目の状態の文字を空にすると、数え上げが違反として数える。何も変えなければ 0
// - P1a・P2a・C2: 変種 (a)。木の意味が無い・全項目にチェック・data-checked=true の数が完了の数・深さのクラスが
//   行の depth と一致。C2 は data-checked を反転すると違反を数える対照
// 代役の部品は #app の外の root にマウントする。1 件ごとに新しい context を使う。
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { expectedFromRows, loadSampleRows, runViewCase } from './artifact-view-check.mjs';

const BASE = '/fandhe-task-tree/';
const ROOT = '#view-fixture-root';
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

async function withFixturePage(fn, { mount = true } = {}) {
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
      if (mount) {
        const res = JSON.parse(await page.evaluate(() => window.__fandheTest.viewFixtureMount('view-fixture-root')));
        assert.deepEqual(res, { ok: true });
      }
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

const ROWS = loadSampleRows('task_tree');
const E = expectedFromRows(ROWS);
const [B, A] = [E.counts.before, E.counts.after];
// 子を持つ項目の数（深さが次の行より浅い）を、その状態で出る行だけから数える。
function expandedCount(phase) {
  const shown = ROWS.filter((r) => (phase === 'before' ? r.before : r.after));
  return shown.filter((r, i) => i + 1 < shown.length && shown[i + 1].depth > r.depth).length;
}

// 固定の関数。選ばれた task_tree の list を数え、違反の文字列を返す。
function countTree(sel) {
  const list = document.querySelector(`${sel} [data-testid="view-list"][data-view="task_tree"]`);
  const out = [];
  if (!list) return { out: ['no list'], items: 0, expanded: 0, trees: 0 };
  const trees = document.querySelectorAll(`${sel} [data-testid="view-list"][role="tree"]`).length;
  const items = [...list.querySelectorAll('[role="treeitem"]')];
  for (const it of items) {
    for (const a of ['aria-level', 'aria-posinset', 'aria-setsize']) {
      if (!it.hasAttribute(a)) out.push(`missing ${a}`);
    }
    const g = it.querySelector('[data-testid="state-glyph"]')?.textContent ?? '';
    const t = it.querySelector('[data-testid="state-text"]')?.textContent ?? '';
    if (g === '' || t === '') out.push('empty state glyph or text');
  }
  const expanded = items.filter((i) => i.getAttribute('aria-expanded') === 'true').length;
  return { out, items: items.length, expanded, trees };
}

test('P1: task_tree の変種 (b) は、前後 2 往復で行も内側も置き換わらず違反 0', async () => {
  await withFixturePage(async (page, rec) => {
    const { violations, stats } = await runViewCase(page, fixtureAdapter(page), {
      kind: 'task_tree', variant: 'b', rootSelector: ROOT, deep: true,
    });
    const counts = await collectSafety(page, rec);
    const detail = JSON.stringify({ violations, stats, counts, rec });
    assert.deepEqual(violations, [], detail);
    assert.deepEqual(stats.itemCounts, [B, A, B, A, B], detail);
    assert.deepEqual(stats.preserved, [B, B, B, B], detail);
    assert.deepEqual(stats.innerPreserved, [B, B, B, B], detail);
    assert.deepEqual(nonZero(counts), [], detail);
  });
});

test('P2: 前後の各状態で、木の属性と状態の記号・文字が揃っている', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('task_tree', 'b')).ok);
    for (const [phase, n] of [['before', B], ['after', A], ['before', B]]) {
      assert.ok((await adapter.phase(phase)).ok);
      const r = await page.evaluate(countTree, ROOT);
      const detail = JSON.stringify({ phase, r });
      assert.deepEqual(r.out, [], detail);
      assert.equal(r.trees, 1, detail);
      assert.equal(r.items, n, detail);
      assert.equal(r.expanded, expandedCount(phase), detail);
    }
  });
});

test('C1: 1 項目の状態の文字を空にすると、数え上げが違反として数える（対: 何も変えなければ 0）', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('task_tree', 'b')).ok);
    assert.deepEqual((await page.evaluate(countTree, ROOT)).out, []);
    await page.evaluate((sel) => {
      const el = document.querySelector(`${sel} [data-view="task_tree"] [data-testid="state-text"]`);
      if (!el) throw new Error('no state text');
      el.textContent = '';
    }, ROOT);
    const r = await page.evaluate(countTree, ROOT);
    assert.ok(r.out.includes('empty state glyph or text'), JSON.stringify(r));
  });
});

const DONE = (r, phase) => (phase === 'before' ? r.before : (r.after ?? r.before))?.state === 'done';
function expectedDone(phase) {
  return ROWS.filter((r) => DONE(r, phase)).length;
}

// 固定の関数。選ばれた task_tree (a) の list を数え、違反の文字列と数を返す。
function countTreeA(sel) {
  const out = [];
  const list = document.querySelector(`${sel} [data-testid="view-list"][data-view="task_tree"]`);
  if (!list) return { out: ['no list'], items: 0, checked: 0, trees: 0, treeitems: 0, depths: [] };
  const trees = document.querySelectorAll(`${sel} [data-testid="view-list"][role="tree"]`).length;
  const treeitems = list.querySelectorAll('[role="treeitem"]').length;
  const items = [...list.querySelectorAll(':scope > li')];
  const depths = [];
  for (const it of items) {
    const checks = it.querySelectorAll('[data-testid="task-check"]');
    if (checks.length !== 1) out.push('check count');
    const c = checks[0];
    const on = c?.getAttribute('data-checked') === 'true';
    if (on !== ((c?.textContent ?? '') !== '')) out.push('check glyph mismatch');
    const g = it.querySelector('[data-testid="state-glyph"]')?.textContent ?? '';
    const t = it.querySelector('[data-testid="state-text"]')?.textContent ?? '';
    if (g === '' || t === '') out.push('empty state glyph or text');
    depths.push(it.querySelector('[data-mark]')?.className ?? '');
  }
  const checked = list.querySelectorAll('[data-checked="true"]').length;
  return { out, items: items.length, checked, trees, treeitems, depths };
}

test('P1a: task_tree の変種 (a) は、前後 2 往復で行も内側も置き換わらず違反 0', async () => {
  await withFixturePage(async (page, rec) => {
    const { violations, stats } = await runViewCase(page, fixtureAdapter(page), {
      kind: 'task_tree', variant: 'a', rootSelector: ROOT, deep: true,
    });
    const counts = await collectSafety(page, rec);
    const detail = JSON.stringify({ violations, stats, counts, rec });
    assert.deepEqual(violations, [], detail);
    assert.deepEqual(stats.itemCounts, [B, A, B, A, B], detail);
    assert.deepEqual(stats.preserved, [B, B, B, B], detail);
    assert.deepEqual(stats.innerPreserved, [B, B, B, B], detail);
    assert.deepEqual(nonZero(counts), [], detail);
  });
});

test('P2a: 前後の各状態で、(a) は木の意味が無く、チェックと状態の記号・文字が揃っている', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('task_tree', 'a')).ok);
    for (const [phase, n] of [['before', B], ['after', A], ['before', B]]) {
      assert.ok((await adapter.phase(phase)).ok);
      const r = await page.evaluate(countTreeA, ROOT);
      const detail = JSON.stringify({ phase, r });
      assert.deepEqual(r.out, [], detail);
      assert.equal(r.trees, 0, detail);
      assert.equal(r.treeitems, 0, detail);
      assert.equal(r.items, n, detail);
      assert.equal(r.checked, expectedDone(phase), detail);
      // 後は削除も前の中身で残るため、全行が出る。
      const shown = ROWS.filter((row) => (phase === 'before' ? row.before : true));
      assert.deepEqual(r.depths, shown.map((row) => `d${row.depth}`), detail);
    }
  });
});

test('C2: 1 項目の data-checked を反転すると、数え上げが違反として数える（対: 何も変えなければ 0）', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('task_tree', 'a')).ok);
    assert.deepEqual((await page.evaluate(countTreeA, ROOT)).out, []);
    await page.evaluate((sel) => {
      const el = document.querySelector(`${sel} [data-view="task_tree"] [data-testid="task-check"]`);
      if (!el) throw new Error('no check');
      el.setAttribute('data-checked', el.getAttribute('data-checked') === 'true' ? 'false' : 'true');
    }, ROOT);
    const r = await page.evaluate(countTreeA, ROOT);
    assert.ok(r.out.includes('check glyph mismatch'), JSON.stringify(r));
  });
});
