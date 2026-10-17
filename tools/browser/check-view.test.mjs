// チェックの一覧のビュー、変種 (a)・(b) と 20 件の規模の試験（TASK-478。ART-1・ART-17）。
//
// 代役の部品（view_fixture）が check_view で描くチェックの一覧を、検査器（artifact-view-check.mjs）で流す。
// - CK1: (a) の前後 2 往復で違反 0、行も項目の内側も置き換わらない、状態の数は 5
// - CK2: (a) の前と後の各状態で、項目数・rich_text と tag の数・印の宿主（li には無く内側の div）・印の数・禁じた要素が期待どおり。
//   前の状態で 5 種の状態が記号と文字の両方で出る
// - CK3: (a) の前後の切り替えで、外部へのリクエスト・ページのエラー・ダイアログがどれも 0 件
// - CK4: (b) の前後 2 往復で違反 0、行も項目の内側も置き換わらない
// - CK5: (b) の前と後の各状態で、項目数・marker・ラベルと補足・印の宿主（li 自身）・禁じた要素（role を含む）が期待どおり
// - CK6: (b) の切り替えで、外部へのリクエスト・ページのエラー・ダイアログがどれも 0 件
// - CK7: a -> b -> a と選び直しても違反 0
// - CK8: 20 件のデータの写し（crates/web/tests/data/artifact_view_scale/checks-20.json）が generate.mjs の出力と一致する
// - CK9: 20 件を (a)・(b) とも検査器に流して違反 0（項目 20 件、保持が 20、後の印は変更 20）
// - CK10: 画面を 20 件に切り替えずに 20 件の rows で流すと、項目数の違反が出る（rows の口が効いていることの対照）
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { runViewCase } from './artifact-view-check.mjs';

const BASE = '/fandhe-check-view/';
const ROOT = '#view-fixture-root';
const LIST = `${ROOT} [data-testid="view-list"][data-view="check_list"]`;
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
    data: (name) => dispatch('fixture_data', name),
    state: () => page.evaluate(() => window.__fandheTest.viewFixtureState()),
    keyOf: (id) => `k${id}`,
  };
}

// 目で数えた値（testdata/artifact-view-parts/test-data/sample/checks.json）。
const COUNTS = { before: 5, after: 6 };

test('CK1: 変種 (a) の前後 2 往復で違反 0、行も内側も置き換わらない', async () => {
  await withFixturePage(async (page, rec) => {
    const { violations, stats } = await runViewCase(page, fixtureAdapter(page), {
      kind: 'check_list', variant: 'a', rootSelector: ROOT, rounds: 2, deep: true,
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
    return {
      items: items.length,
      richTexts: list.querySelectorAll('.fw-wire-rich-text').length,
      glyphs: list.querySelectorAll('.fw-wire-rich-text > span.icon').length,
      stateTags: list.querySelectorAll('.fw-wire-rich-text > .fw-wire-tag').length,
      liMarks: items.filter((li) => li.hasAttribute('data-mark')).length,
      divHosts: list.querySelectorAll('li > div[data-mark]').length,
      marks: list.querySelectorAll('li > div[data-mark]:not([data-mark=""])').length,
      forbidden: list.querySelectorAll('[role], [aria-hidden], svg, [data-part]').length + (list.hasAttribute('role') ? 1 : 0),
      text: list.textContent,
    };
  }, LIST);
}

test('CK2: 変種 (a) の前後それぞれで、項目数・部品の数・印の宿主・印の数が期待どおり', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('check_list', 'a')).ok);
    for (const [phase, n, marks] of [['before', COUNTS.before, 0], ['after', COUNTS.after, 5], ['before', COUNTS.before, 0]]) {
      assert.ok((await adapter.phase(phase)).ok);
      const o = await observe(page);
      const detail = JSON.stringify({ phase, o });
      assert.equal(o.items, n, detail);
      assert.equal(o.richTexts, n, detail);
      assert.equal(o.glyphs, n, detail);
      assert.equal(o.stateTags, n, detail);
      assert.equal(o.liMarks, 0, detail);
      assert.equal(o.divHosts, n, detail);
      assert.equal(o.marks, marks, detail);
      assert.equal(o.forbidden, 0, detail);
      if (phase === 'before') {
        for (const w of ['✓', '成功', '✗', '失敗', '実行中', '待機中', '省略']) assert.ok(o.text.includes(w), `${w}: ${detail}`);
      }
    }
  });
});

test('CK3: 変種 (a) の切り替えで外部へのリクエスト・ページのエラー・ダイアログは 0 件', async () => {
  await withFixturePage(async (page, rec) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('check_list', 'a')).ok);
    for (const phase of ['after', 'before', 'after']) assert.ok((await adapter.phase(phase)).ok);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});

const SCALE_JSON = fileURLToPath(new URL('../../crates/web/tests/data/artifact_view_scale/checks-20.json', import.meta.url));
const GENERATE = fileURLToPath(new URL('../../testdata/artifact-view-parts/test-data/generate.mjs', import.meta.url));

test('CK4: 変種 (b) の前後 2 往復で違反 0、行も内側も置き換わらない', async () => {
  await withFixturePage(async (page, rec) => {
    const { violations, stats } = await runViewCase(page, fixtureAdapter(page), {
      kind: 'check_list', variant: 'b', rootSelector: ROOT, rounds: 2, deep: true,
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

function observeB(page) {
  return page.evaluate((sel) => {
    const list = document.querySelector(sel);
    const items = [...list.querySelectorAll(':scope > li')];
    return {
      items: items.length,
      markers: list.querySelectorAll('[data-scope="marker"][data-part="root"]').length,
      labels: list.querySelectorAll('[data-testid="check-label"]').length,
      details: list.querySelectorAll('[data-testid="check-detail"]').length,
      liMarks: items.filter((li) => li.hasAttribute('data-mark')).length,
      marks: items.filter((li) => li.hasAttribute('data-mark') && li.getAttribute('data-mark') !== '').length,
      innerMarks: list.querySelectorAll('li [data-mark]').length,
      forbidden: list.querySelectorAll('[role], [aria-hidden], svg, [data-part="icon"]').length + (list.hasAttribute('role') ? 1 : 0),
      text: list.textContent,
    };
  }, LIST);
}

test('CK5: 変種 (b) の前後それぞれで、項目数・部品の数・印の宿主・印の数が期待どおり', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('check_list', 'b')).ok);
    for (const [phase, n, marks] of [['before', COUNTS.before, 0], ['after', COUNTS.after, 5], ['before', COUNTS.before, 0]]) {
      assert.ok((await adapter.phase(phase)).ok);
      const o = await observeB(page);
      const detail = JSON.stringify({ phase, o });
      assert.equal(o.items, n, detail);
      assert.equal(o.markers, n, detail);
      assert.equal(o.labels, n, detail);
      assert.equal(o.details, n, detail);
      assert.equal(o.liMarks, n, detail);
      assert.equal(o.marks, marks, detail);
      assert.equal(o.innerMarks, 0, detail);
      assert.equal(o.forbidden, 0, detail);
      if (phase === 'before') {
        for (const w of ['✓ 成功', '✗ 失敗', '… 実行中', '… 待機中', '– 省略']) assert.ok(o.text.includes(w), `${w}: ${detail}`);
      }
    }
  });
});

test('CK6: 変種 (b) の切り替えで外部へのリクエスト・ページのエラー・ダイアログは 0 件', async () => {
  await withFixturePage(async (page, rec) => {
    const adapter = fixtureAdapter(page);
    assert.ok((await adapter.select('check_list', 'b')).ok);
    for (const phase of ['after', 'before', 'after']) assert.ok((await adapter.phase(phase)).ok);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});

test('CK7: a -> b -> a と選び直しても違反 0', async () => {
  await withFixturePage(async (page) => {
    for (const variant of ['a', 'b', 'a']) {
      const { violations } = await runViewCase(page, fixtureAdapter(page), {
        kind: 'check_list', variant, rootSelector: ROOT, rounds: 1, deep: true,
      });
      assert.deepEqual(violations, [], `${variant}: ${JSON.stringify(violations)}`);
    }
  });
});

test('CK8: 20 件のデータの写しは generate.mjs の出力と一致する', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'checks20-'));
  try {
    execFileSync(process.execPath, [GENERATE, dir], { stdio: 'ignore' });
    const generated = JSON.parse(fs.readFileSync(path.join(dir, 'checks-20.json'), 'utf8'));
    const copy = JSON.parse(fs.readFileSync(SCALE_JSON, 'utf8'));
    assert.deepEqual(copy, generated);
    assert.equal(copy.rows.length, 20);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

function observeChanges(page) {
  return page.evaluate((sel) => document.querySelectorAll(`${sel} [data-mark="change"]`).length, LIST);
}

const rows20 = () => JSON.parse(fs.readFileSync(SCALE_JSON, 'utf8')).rows;

for (const variant of ['a', 'b']) {
  test(`CK9: 20 件を変種 (${variant}) で検査器に流すと違反 0、項目 20、保持 20`, async () => {
    await withFixturePage(async (page, rec) => {
      const adapter = fixtureAdapter(page);
      assert.ok((await adapter.data('checks:20')).ok);
      const { violations, stats } = await runViewCase(page, adapter, {
        kind: 'check_list', variant, rootSelector: ROOT, rounds: 2, deep: true, rows: rows20(),
      });
      const counts = await collectSafety(page, rec);
      const detail = JSON.stringify({ violations, stats, counts, rec });
      assert.deepEqual(violations, [], detail);
      assert.deepEqual(stats.itemCounts, [20, 20, 20, 20, 20], detail);
      assert.deepEqual(stats.preserved, [20, 20, 20, 20], detail);
      assert.deepEqual(stats.innerPreserved, [20, 20, 20, 20], detail);
      assert.ok((await adapter.phase('after')).ok);
      assert.equal(await observeChanges(page), 20, detail);
      assert.deepEqual(nonZero(counts), [], detail);
    });
  });
}

test('CK10: 画面を 20 件に切り替えずに 20 件の rows で流すと、項目数の違反が出る', async () => {
  await withFixturePage(async (page) => {
    const { violations } = await runViewCase(page, fixtureAdapter(page), {
      kind: 'check_list', variant: 'b', rootSelector: ROOT, rounds: 1, rows: rows20(),
    });
    assert.ok(violations.some((m) => m.includes('item count differs from the expected count')), JSON.stringify(violations));
  });
});
