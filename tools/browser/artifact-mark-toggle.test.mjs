// 前後の切り替えでの差分更新の試験（TASK-469 2/2。ART-1・ART-3）。
//
// 部品（artifact_mark の marked_item）を使う項目が、前後の切り替えで keyed list の項目として in-place に更新される
// こと（未変更の行は置き換わらず、変更のあった行もその場で更新される）を、検査器（artifact-view-check.mjs）の
// deep 検査で確かめる。deep は行の根に加えて、項目の内側（印の宿主・記号・文字・内容の span）の同一性も数える。
// - P1: 契約が許す 12 通りを前後 2 往復し、違反 0、行の保持が前の行数に等しく、内側の保持も全遷移で前の行数に等しい
// - P2: 印なしの行が空の 3 手段で実際にあり、後の状態では印のある行が現れる（検査が空振りでない）
// - C1: 負の対照（印の文字を消す）。deep の経路でも「3 手段が対応しない」違反が出る。対として何も変えなければ違反 0
// - C2: 負の対照（同一性の検査が置換を検出できる）。別のビューへ切り替えて戻すと保持が 0 件になる（PoC-8 の 0/11 の写し）
// - T4: マウントしない空の画面では deep でも違反が出る
// 拒否の 2 通り（文書 (a)・差分 (a)）は artifact-view-check.test.mjs の T3 が持つ。
// 代役の部品は #app の外の root にマウントする。1 件ごとに新しい context を使う。
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  CASES, DOC_EXPECTED, expectedFromRows, loadSampleRows, runViewCase, holdRows, compareHeldRows,
} from './artifact-view-check.mjs';

const BASE = '/fandhe-artifact-mark-toggle/';
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

// 目で数えた行数 [前, 後]。文書は検査器の固定値。
function rowCounts(kind) {
  if (kind === 'document') return [DOC_EXPECTED.before, DOC_EXPECTED.after];
  const e = expectedFromRows(loadSampleRows(kind));
  return [e.counts.before, e.counts.after];
}

for (const { kind, variant } of CASES.filter((c) => c.allowed)) {
  test(`P1: ${kind} の変種 (${variant}) は、前後 2 往復で行も内側も置き換わらない`, async () => {
    await withFixturePage(async (page, rec) => {
      const { violations, stats } = await runViewCase(page, fixtureAdapter(page), {
        kind, variant, rootSelector: ROOT, deep: true,
      });
      const counts = await collectSafety(page, rec);
      const detail = JSON.stringify({ violations, stats, counts, rec });
      const [b, a] = rowCounts(kind);
      assert.deepEqual(violations, [], detail);
      assert.equal(stats.states, 5, detail);
      assert.deepEqual(stats.itemCounts, [b, a, b, a, b], detail);
      assert.deepEqual(stats.preserved, [b, b, b, b], detail);
      assert.deepEqual(stats.innerPreserved, [b, b, b, b], detail);
      assert.deepEqual(nonZero(counts), [], detail);
    });
  });
}

test('P2: 印なしの行が実際にあり、検査が空振りでない（前の状態は全行が印なし）', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('card_list', 'b')).ok);
    assert.ok((await adapter.phase('before')).ok);
    const none = await page.evaluate((sel) => {
      const hosts = [...document.querySelectorAll(`${sel} [data-testid="view-list"][data-view="card_list"] [data-mark]`)];
      return {
        total: hosts.length,
        empty: hosts.filter((h) => h.getAttribute('data-mark') === ''
          && h.querySelector('[data-testid="mark-glyph"]')?.textContent === ''
          && h.querySelector('[data-testid="mark-text"]')?.textContent === '').length,
      };
    }, ROOT);
    assert.equal(none.total, rowCounts('card_list')[0]);
    assert.equal(none.empty, none.total);
    assert.ok((await adapter.phase('after')).ok);
    const marked = await page.evaluate((sel) => [...document.querySelectorAll(`${sel} [data-testid="view-list"][data-view="card_list"] [data-mark]`)]
      .filter((h) => h.getAttribute('data-mark') !== '').length, ROOT);
    assert.equal(marked, 4);
  });
});

test('C1 対: deep の経路でも、何も変えなければ file_tree (b) は違反 0', async () => {
  await withFixturePage(async (page) => {
    const { violations } = await runViewCase(page, fixtureAdapter(page), {
      kind: 'file_tree', variant: 'b', rootSelector: ROOT, deep: true,
    });
    assert.deepEqual(violations, []);
  });
});

test('C1: 印の文字を消すと、deep の経路でも 3 手段が対応しない違反として検出される', async () => {
  await withFixturePage(async (page) => {
    const base = fixtureAdapter(page);
    const adapter = {
      ...base,
      phase: async (name) => {
        const r = await base.phase(name);
        if (r.ok && name === 'after') {
          await page.evaluate((sel) => {
            const host = [...document.querySelectorAll(`${sel} [data-mark]`)].find((h) => h.getAttribute('data-mark') !== '');
            if (!host) throw new Error('no mark host to erase');
            host.querySelector('[data-testid="mark-text"]').textContent = '';
          }, ROOT);
        }
        return r;
      },
    };
    const { violations } = await runViewCase(page, adapter, { kind: 'file_tree', variant: 'b', rootSelector: ROOT, deep: true });
    assert.ok(violations.some((m) => m.includes('do not correspond')), JSON.stringify(violations));
  });
});

test('C2: 別のビューへ切り替えて戻すと、行の保持が 0 件になり、同一性の検査が置換を検出する', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    const [b] = rowCounts('card_list');
    assert.ok((await adapter.select('card_list', 'b')).ok);
    assert.ok((await adapter.phase('before')).ok);
    const held = await holdRows(page, ROOT, 'card_list');
    // 対: 何もせず取り直すと全件が同じ要素。
    const same = await compareHeldRows(page, held, await holdRows(page, ROOT, 'card_list'));
    assert.equal(same.same.length, b);
    assert.deepEqual(same.replaced, []);
    // 別のビューを経由して戻すと、行は作り直される。
    assert.ok((await adapter.select('check_list', 'b')).ok);
    assert.ok((await adapter.select('card_list', 'b')).ok);
    assert.ok((await adapter.phase('before')).ok);
    const back = await compareHeldRows(page, held, await holdRows(page, ROOT, 'card_list'));
    assert.equal(back.same.length, 0, JSON.stringify(back));
    assert.equal(back.replaced.length, b, JSON.stringify(back));
  });
});

test('T4: マウントしない空の画面では、deep でも検査器は違反を出す', async () => {
  await withFixturePage(async (page) => {
    const adapter = { select: async () => ({ ok: true }), phase: async () => ({ ok: true }), keyOf: (id) => `k${id}` };
    const { violations } = await runViewCase(page, adapter, { kind: 'file_tree', variant: 'b', rootSelector: ROOT, deep: true });
    assert.ok(violations.some((m) => m.includes('expected exactly 7 view lists')), JSON.stringify(violations));
  }, { mount: false });
});
