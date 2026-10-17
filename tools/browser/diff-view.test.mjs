// 差分のビュー、変種 (b) の試験（TASK-475 1/2。ART-1・ART-8）。
//
// 代役の部品（view_fixture）が diff_view で描く差分 (b) を、検査器（artifact-view-check.mjs）で流す。
// - DF1: 前後 2 往復で違反 0、行も項目の内側も置き換わらない、状態の数は 5、項目数は [7,9,7,9,7]
// - DF2: 前と後の各状態で、全 li が行番号 2 列・記号・pre を持ち、pre は空白を保ち、文字が JSON の label と一致する。
//        層 1（行の印）と層 2（差分の記号）が別の要素に出る。role・aria-* は 0 件
// - DF3: 外部へのリクエスト・ページのエラー・ダイアログがどれも 0 件
// - DF4: 対照。インデントの検査が正しい理由で失敗する（pre の空白の扱いを normal にすると、字下げの差が消える）
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { runViewCase } from './artifact-view-check.mjs';

const BASE = '/fandhe-diff-view/';
const ROOT = '#view-fixture-root';
const LIST = `${ROOT} [data-testid="view-list"][data-view="diff"]`;
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

// 目で数えた値と、固定データ（testdata/artifact-view-parts/test-data/sample/diff.json）の label。
const COUNTS = { before: 7, after: 9 };
const LABELS = {
  before: ['@@ -1,6 +1,6 @@ fn main', 'use std::io;', '', 'fn main() {', '    println!("メモ帳");', '    old_menu();', '}'],
  after: [
    '@@ -1,6 +1,8 @@ fn main', 'use std::io;', '', 'fn main() {', '    println!("メモ帳 v2");',
    '    let notes = store::load();', '    show(&notes);', '    old_menu();', '}',
  ],
};

function observeDiff(page) {
  return page.evaluate((sel) => {
    const list = document.querySelector(sel);
    const items = [...list.querySelectorAll(':scope > li')];
    const one = (li, id) => li.querySelectorAll(`[data-testid="${id}"]`);
    const textOf = (li, id) => one(li, id)[0]?.textContent ?? null;
    return {
      items: items.length,
      shapeOk: items.every((li) => ['diff-old-no', 'diff-new-no', 'diff-sign', 'diff-code', 'mark-glyph', 'mark-text']
        .every((id) => one(li, id).length === 1)),
      preOk: items.every((li) => one(li, 'diff-code')[0].tagName === 'PRE'
        && ['pre', 'pre-wrap'].includes(getComputedStyle(one(li, 'diff-code')[0]).whiteSpace)),
      codes: items.map((li) => textOf(li, 'diff-code')),
      signs: items.map((li) => textOf(li, 'diff-sign')),
      glyphs: items.map((li) => textOf(li, 'mark-glyph')),
      marks: items.filter((li) => li.getAttribute('data-mark') !== '').length,
      roles: list.querySelectorAll('[role]').length,
      arias: [...list.querySelectorAll('*')].filter((e) => e.getAttributeNames().some((n) => n.startsWith('aria-'))).length,
    };
  }, LIST);
}

test('DF1: 前後 2 往復で違反 0、行も内側も置き換わらない', async () => {
  await withFixturePage(async (page, rec) => {
    const { violations, stats } = await runViewCase(page, fixtureAdapter(page), {
      kind: 'diff', variant: 'b', rootSelector: ROOT, rounds: 2, deep: true,
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

test('DF2: 前後それぞれで、列・pre・文字・2 層の印が期待どおり', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('diff', 'b')).ok);
    for (const phase of ['before', 'after', 'before']) {
      assert.ok((await adapter.phase(phase)).ok);
      const o = await observeDiff(page);
      const detail = JSON.stringify({ phase, o });
      assert.equal(o.items, COUNTS[phase], detail);
      assert.ok(o.shapeOk, detail);
      assert.ok(o.preOk, detail);
      assert.deepEqual(o.codes, LABELS[phase], detail);
      assert.equal(o.roles, 0, detail);
      assert.equal(o.arias, 0, detail);
      if (phase === 'before') {
        // 層 1 は 0 件だが、層 2 の - は削除の 2 行に出る。
        assert.equal(o.marks, 0, detail);
        assert.equal(o.signs.filter((s) => s === '-').length, 2, detail);
        assert.equal(o.signs.filter((s) => s === '+').length, 0, detail);
      } else {
        // l4（4 行目）は、層 1 が ~ で層 2 が + の別の要素。
        assert.equal(o.glyphs[4], '~', detail);
        assert.equal(o.signs[4], '+', detail);
        assert.equal(o.signs.filter((s) => s === '+').length, 3, detail);
        assert.equal(o.signs.filter((s) => s === '-').length, 1, detail);
        assert.equal(o.marks, 6, detail);
      }
    }
  });
});

test('DF3: 外部へのリクエスト・ページのエラー・ダイアログは 0 件', async () => {
  await withFixturePage(async (page, rec) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('diff', 'b')).ok);
    for (const phase of ['after', 'before', 'after']) assert.ok((await adapter.phase(phase)).ok);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});

// l3（fn main() {）と l4（4 つの空白のある行）の、最初の非空白の文字の左端の差（px）。
function indentGap(page) {
  return page.evaluate((sel) => {
    const lefts = [];
    for (const idx of [3, 4]) {
      const li = document.querySelectorAll(`${sel} > li`)[idx];
      const node = li.querySelector('[data-testid="diff-code"]').firstChild;
      const at = node.textContent.search(/\S/);
      const r = document.createRange();
      r.setStart(node, at);
      r.setEnd(node, at + 1);
      lefts.push(r.getBoundingClientRect().left);
    }
    return lefts[1] - lefts[0];
  }, LIST);
}

test('DF4 対照: インデントの検査は、pre の空白の扱いを normal にすると失敗する', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('diff', 'b')).ok);
    const gap = await indentGap(page);
    assert.ok(gap > 5, `indent should be visible: ${gap}`);
    await page.evaluate((sel) => {
      for (const pre of document.querySelectorAll(`${sel} [data-testid="diff-code"]`)) pre.style.whiteSpace = 'normal';
    }, LIST);
    const collapsed = await indentGap(page);
    assert.ok(collapsed <= 0.5, `indent should be gone when white-space is normal: ${collapsed}`);
  });
});
