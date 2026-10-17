// 文書のビュー、変種 (b) の試験（TASK-476 1/2。ART-1・ART-7）。
//
// 代役の部品（view_fixture）が document_view で描く文書 (b) を、検査器（artifact-view-check.mjs）で流す。
// - DV1: 前後 2 往復で違反 0、行も項目の内側も置き換わらない、状態の数は 5、項目数は [10,13,10,13,10]
// - DV2: 各状態で、全 li がちょうど 1 つの doc-block を持ち、見出し・一覧・表・引用・行内のコードの数と表のセルの文字が
//        Markdown（doc-before.md・doc-after.md）を目で数えた値と一致する。role・aria-* は 0 件
// - DV3: 外部へのリクエスト・ページのエラー・ダイアログがどれも 0 件
// - DV4: 対照。構造の検査が正しい理由で失敗する（doc-block の中身を textContent だけの span にすると、表・引用の数が 0 になる）
// `-` と `+` の組の主張と変種 (a) の対照は TASK-476 2/2 の担当で、ここでは書かない。
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { runViewCase } from './artifact-view-check.mjs';

const BASE = '/fandhe-doc-view/';
const ROOT = '#view-fixture-root';
const LIST = `${ROOT} [data-testid="view-list"][data-view="document"]`;
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


// 目で数えた値（testdata/artifact-view-parts/test-data/sample/doc-before.md・doc-after.md）。
// 後の状態は、変更された番号つき一覧・表・番号なし一覧が「前の旧 + 後の新」の 2 つずつ並ぶ。
const COUNTS = { before: 10, after: 13 };
const STRUCT = {
  before: { h1: 1, h2: 3, ol: 1, ul: 1, table: 1, thead: 1, tbody: 1, th: 2, td: 4, blockquote: 1, code: 3 },
  after: { h1: 1, h2: 3, ol: 2, ul: 2, table: 2, thead: 2, tbody: 2, th: 4, td: 10, blockquote: 1, code: 5 },
};
const CELLS = {
  before: ['項目', '内容', '形式', 'Markdown', '場所', 'notes/'],
  after: ['項目', '内容', '形式', 'Markdown', '場所', 'notes/', '項目', '内容', '形式', 'Markdown', '場所', 'notes/', '文字コード', 'UTF-8'],
};

function observeDoc(page) {
  return page.evaluate((sel) => {
    const list = document.querySelector(sel);
    const items = [...list.querySelectorAll(':scope > li')];
    const q = (tag) => list.querySelectorAll(tag).length;
    return {
      items: items.length,
      wrapOk: items.every((li) => li.querySelectorAll(':scope > [data-testid="doc-block"]').length === 1),
      struct: {
        h1: q('h1'), h2: q('h2'), ol: q('ol'), ul: q('ul'), table: q('table'), thead: q('thead'), tbody: q('tbody'),
        th: q('th'), td: q('td'), blockquote: q('blockquote'), code: q('code'),
      },
      cells: [...list.querySelectorAll('th, td')].map((c) => c.textContent),
      roles: list.querySelectorAll('[role]').length,
      arias: [...list.querySelectorAll('*')].filter((e) => e.getAttributeNames().some((n) => n.startsWith('aria-'))).length,
    };
  }, LIST);
}

test('DV1: 前後 2 往復で違反 0、行も内側も置き換わらない', async () => {
  await withFixturePage(async (page, rec) => {
    const { violations, stats } = await runViewCase(page, fixtureAdapter(page), {
      kind: 'document', variant: 'b', rootSelector: ROOT, rounds: 2, deep: true,
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

test('DV2: 前後それぞれで、構造の要素の数と表のセルの文字が Markdown と一致する', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('document', 'b')).ok);
    for (const phase of ['before', 'after', 'before']) {
      assert.ok((await adapter.phase(phase)).ok);
      const o = await observeDoc(page);
      const detail = JSON.stringify({ phase, o });
      assert.equal(o.items, COUNTS[phase], detail);
      assert.ok(o.wrapOk, detail);
      assert.deepEqual(o.struct, STRUCT[phase], detail);
      assert.deepEqual(o.cells, CELLS[phase], detail);
      assert.equal(o.roles, 0, detail);
      assert.equal(o.arias, 0, detail);
    }
  });
});

test('DV3: 外部へのリクエスト・ページのエラー・ダイアログは 0 件', async () => {
  await withFixturePage(async (page, rec) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('document', 'b')).ok);
    for (const phase of ['after', 'before', 'after']) assert.ok((await adapter.phase(phase)).ok);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});

test('DV4 対照: 構造の検査は、doc-block の中身を文字だけにすると失敗する', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('document', 'b')).ok);
    assert.ok((await adapter.phase('after')).ok);
    const ok = await observeDoc(page);
    assert.deepEqual(ok.struct, STRUCT.after, JSON.stringify(ok));
    await page.evaluate((sel) => {
      for (const w of document.querySelectorAll(`${sel} [data-testid="doc-block"]`)) {
        const flat = document.createElement('span');
        flat.textContent = w.textContent;
        w.replaceChildren(flat);
      }
    }, LIST);
    const flat = await observeDoc(page);
    assert.notDeepEqual(flat.struct, STRUCT.after, JSON.stringify(flat));
    assert.equal(flat.struct.table, 0, JSON.stringify(flat));
    assert.equal(flat.struct.blockquote, 0, JSON.stringify(flat));
    assert.equal(flat.struct.ol + flat.struct.ul, 0, JSON.stringify(flat));
    assert.equal(flat.items, COUNTS.after, JSON.stringify(flat));
  });
});
