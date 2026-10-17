// アプリのデモのワイヤーのビュー、変種 (a) の試験（TASK-479 1/2。ART-1・ART-17）。
//
// 代役の部品（view_fixture）が app_demo_view で描く変種 (a) を、検査器（artifact-view-check.mjs）で流す。
// - AD1: (a) の前後 2 往復で違反 0、行も項目の内側も置き換わらない（保持が前の行数に等しい）、状態の数は 5
// - AD2: 前・後・前の各状態で、項目数・部品の数・印の宿主（li には無く内側の div）・印の数・禁じた属性が期待どおり
// - AD3: タブの 2 つの li が app.css で横に並ぶ（display が inline-block、上端が等しく、2 つ目が右にある）
// - AD4: 切り替えで、外部へのリクエスト・ページのエラー・ダイアログがどれも 0 件
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { runViewCase } from './artifact-view-check.mjs';

const BASE = '/fandhe-app-demo-view/';
const ROOT = '#view-fixture-root';
const LIST = `${ROOT} [data-testid="view-list"][data-view="app_demo"]`;
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

// 目で数えた値（testdata/artifact-view-parts/test-data/sample/appdemo.json）。
const COUNTS = { before: 8, after: 9 };

test('AD1: 変種 (a) の前後 2 往復で違反 0、行も内側も置き換わらない', async () => {
  await withFixturePage(async (page, rec) => {
    const { violations, stats } = await runViewCase(page, fixtureAdapter(page), {
      kind: 'app_demo', variant: 'a', rootSelector: ROOT, rounds: 2, deep: true,
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

function observe(page) {
  return page.evaluate((sel) => {
    const list = document.querySelector(sel);
    const items = [...list.querySelectorAll(':scope > li')];
    const n = (s) => list.querySelectorAll(s).length;
    return {
      items: items.length,
      texts: n('.fw-wire-text'),
      tags: n('.fw-wire-tag'),
      richTexts: n('.fw-wire-rich-text'),
      inputs: n('.fw-wire-input'),
      buttons: n('.fw-wire-button'),
      cards: n('.fw-wire-card-basic'),
      liMarks: items.filter((li) => li.hasAttribute('data-mark')).length,
      divHosts: n('li > div[data-mark]'),
      marks: n('li > div[data-mark]:not([data-mark=""])'),
      forbidden: n('[role], [aria-hidden], svg, [data-part], [data-active], [data-disabled]') + (list.hasAttribute('role') ? 1 : 0),
    };
  }, LIST);
}

test('AD2: 前・後・前の各状態で、項目数・部品の数・印の宿主・印の数が期待どおり', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('app_demo', 'a')).ok);
    for (const [phase, n, marks, cards] of [['before', COUNTS.before, 0, 2], ['after', COUNTS.after, 4, 3], ['before', COUNTS.before, 0, 2]]) {
      assert.ok((await adapter.phase(phase)).ok);
      const o = await observe(page);
      const detail = JSON.stringify({ phase, o });
      assert.equal(o.items, n, detail);
      assert.equal(o.texts, 2, detail);
      assert.equal(o.tags, 2, detail);
      assert.equal(o.richTexts, 1, detail);
      assert.equal(o.inputs, 1, detail);
      assert.equal(o.buttons, 1, detail);
      assert.equal(o.cards, cards, detail);
      assert.equal(o.liMarks, 0, detail);
      assert.equal(o.divHosts, n, detail);
      assert.equal(o.marks, marks, detail);
      assert.equal(o.forbidden, 0, detail);
    }
  });
});

test('AD3: タブの li は app.css で横に並ぶ', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('app_demo', 'a')).ok);
    const tabs = await page.evaluate((sel) => [...document.querySelectorAll(`${sel} > li[data-kind="tab"]`)].map((li) => {
      const r = li.getBoundingClientRect();
      return { display: getComputedStyle(li).display, top: r.top, left: r.left };
    }), LIST);
    const detail = JSON.stringify(tabs);
    assert.equal(tabs.length, 2, detail);
    assert.ok(tabs.every((t) => t.display === 'inline-block'), detail);
    assert.equal(tabs[0].top, tabs[1].top, detail);
    assert.ok(tabs[1].left > tabs[0].left, detail);
  });
});

test('AD4: 切り替えで外部へのリクエスト・ページのエラー・ダイアログは 0 件', async () => {
  await withFixturePage(async (page, rec) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('app_demo', 'a')).ok);
    for (const phase of ['after', 'before', 'after']) assert.ok((await adapter.phase(phase)).ok);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});
