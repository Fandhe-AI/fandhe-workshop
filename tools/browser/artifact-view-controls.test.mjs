// 左の成果物ビュー共通の検査器（artifact-view-check.mjs）の負の対照 3 種（TASK-471 3/3。PoC-8 の対照の作り直し）。
//
// 検査器が「差を見つけられる」ことを、変えない側（違反 0）と変えた側（特定の違反）の対で示す。
// 違反は固定の文言で特定し、「違反が 1 件以上ある」だけでは通さない。
// - N1: 印を消す（記号・文字・data-mark の 3 通り）-> 「3 手段が対応しない」
// - N2: 部品を項目の外に置く（代役の部品の fixture_control）-> 全再描画による行の置換・根の置換
// - N3a: 同じ形の文字列を innerHTML に入れると、危険な要素の選択子が数える（textContent なら 0）。不活性の文書で数える
// - N3b: 本体が innerHTML を使った状態を、検査器が「危険な要素がある」違反として検出する
// 呼び出し元は make test-browser（scripts/test-browser.sh）。代役の部品は #app の外の root にマウントする
// （artifact-view-check.test.mjs と同じ）。1 件ごとに新しい context を使う。
// 通信・実行の副作用を起こさない形だけを使う（img・iframe に src を持たせない。javascript: の a はクリックしない）。
// 24 通り（6 ビュー × 2 変種 × 前後）で escape の行を実際に流す試験の骨格は view-escape.test.mjs（TASK-482 1/2）、
// 全通りの合格は TASK-482 2/2 の担当で、ここでは扱わない。
//
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { DANGER_SELECTOR, loadEscapeRows, runViewCase } from './artifact-view-check.mjs';

const BASE = '/fandhe-artifact-view-controls/';
const ROOT = '#view-fixture-root';
let browser;

// escape-rows.json の文字列（行の前・後の label・detail・tags の順。null のセルは除く）を innerHTML に入れたときに
// 危険な要素の選択子が数える件数。目で数えた値: e1 前（label の img・detail の script・tag 0）、e1 後（label の img・
// detail の svg[onload]・tag 0）、e2 前（label の iframe・detail の script）、他（実体参照・Markdown・属性の閉じ忘れの形は
// タグにならず 0）。
const EXPECTED_HTML = [1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0];

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
    control: (payload) => dispatch('fixture_control', payload),
    state: () => page.evaluate(() => window.__fandheTest.viewFixtureState()),
    keyOf: (id) => `k${id}`,
  };
}

// phase('after') の直後に mutate(page) を実行するアダプタ。変えるのは画面だけで、検査器には手を入れない。
function mutatingAdapter(page, mutate) {
  const base = fixtureAdapter(page);
  return {
    ...base,
    phase: async (name) => {
      const r = await base.phase(name);
      if (r.ok && name === 'after') await mutate(page);
      return r;
    },
  };
}

// --- N1: 印を消す ---

// 後の状態で印が空でない最初の宿主から、印の 3 手段のうち 1 つを消す。固定の関数で、消す手段だけを渡す。
function eraseMark(way) {
  return (page) => page.evaluate((w) => {
    const host = [...document.querySelectorAll('#view-fixture-root [data-mark]')].find((h) => h.getAttribute('data-mark') !== '');
    if (!host) throw new Error('no mark host to erase');
    if (w === 'data-mark') host.setAttribute('data-mark', '');
    else host.querySelector(w === 'glyph' ? '[data-testid="mark-glyph"]' : '[data-testid="mark-text"]').textContent = '';
  }, way);
}

test('N1 対: 何も変えなければ file_tree (b) は違反 0', async () => {
  await withFixturePage(async (page) => {
    const { violations } = await runViewCase(page, fixtureAdapter(page), { kind: 'file_tree', variant: 'b', rootSelector: ROOT });
    assert.deepEqual(violations, []);
  });
});

for (const way of ['glyph', 'text', 'data-mark']) {
  test(`N1: 印の ${way} を消すと、3 手段が対応しない違反として検出される`, async () => {
    await withFixturePage(async (page, rec) => {
      const { violations } = await runViewCase(page, mutatingAdapter(page, eraseMark(way)), {
        kind: 'file_tree', variant: 'b', rootSelector: ROOT,
      });
      const detail = JSON.stringify(violations);
      assert.ok(violations.some((m) => m.includes('glyph/text/data-mark do not correspond')), detail);
      assert.deepEqual(nonZero(await collectSafety(page, rec)), [], detail);
    });
  });
}

// --- N2: 部品を項目の外に置く ---

for (const variant of ['a', 'b']) {
  test(`N2 対: 部品が項目の中のままなら、card_list (${variant}) は違反 0 で行の要素が保たれる`, async () => {
    await withFixturePage(async (page) => {
      const adapter = fixtureAdapter(page);
      assert.equal((await adapter.control('inside')).ok, true);
      const { violations, stats } = await runViewCase(page, adapter, { kind: 'card_list', variant, rootSelector: ROOT });
      assert.deepEqual(violations, []);
      assert.deepEqual(stats.preserved, [3, 3, 3, 3]);
    });
  });

  test(`N2: 部品を項目の外に置くと、card_list (${variant}) は行と根の置換として検出される`, async () => {
    await withFixturePage(async (page, rec) => {
      const adapter = fixtureAdapter(page);
      assert.equal((await adapter.control('outside')).ok, true);
      const { violations, stats } = await runViewCase(page, adapter, { kind: 'card_list', variant, rootSelector: ROOT });
      const detail = JSON.stringify({ violations, stats });
      assert.ok(violations.some((m) => m.includes('rows present in both states were replaced')), detail);
      assert.ok(violations.some((m) => m.includes('the view root element was replaced')), detail);
      assert.deepEqual(stats.preserved, [0, 0, 0, 0], detail);
      assert.deepEqual(nonZero(await collectSafety(page, rec)), [], detail);
    });
  });
}

test('N2: fixture_control の不正な payload は拒否され、状態は変わらない', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    const before = await adapter.state();
    assert.equal((await adapter.control('Outside')).ok, false);
    assert.equal(await adapter.state(), before);
  });
});

// --- N3: innerHTML ---

// 不活性の文書（ブラウジングコンテキストを持たず、読み込みも実行も起きない）で、値を innerHTML / textContent に入れて数える。
function countDangerInert([values, selector]) {
  const doc = document.implementation.createHTMLDocument('');
  const count = (assign) => values.map((v) => {
    const box = doc.createElement('div');
    assign(box, v);
    return box.querySelectorAll(selector).length;
  });
  return {
    html: count((box, v) => { box.innerHTML = v; }),
    text: count((box, v) => { box.textContent = v; }),
  };
}

test('N3a: escape の値は innerHTML に入れると危険な要素として数えられ、textContent なら 0', async () => {
  await withFixturePage(async (page) => {
    const cells = loadEscapeRows('card_list').rows.flatMap((r) => [r.before, r.after])
      .filter((c) => c !== null)
      .flatMap((c) => [c.label, c.detail, ...c.tags]);
    assert.equal(cells.length, EXPECTED_HTML.length);
    assert.ok(cells.every((s) => typeof s === 'string'));
    const { html, text } = await page.evaluate(countDangerInert, [cells, DANGER_SELECTOR]);
    assert.deepEqual(html, EXPECTED_HTML, JSON.stringify(html));
    assert.deepEqual(text, text.map(() => 0));
    assert.equal(await page.evaluate(() => window.__escape_hit), undefined);
  });
});

test('N3b 対: innerHTML を使わなければ違反 0', async () => {
  await withFixturePage(async (page) => {
    const { violations } = await runViewCase(page, fixtureAdapter(page), { kind: 'card_list', variant: 'b', rootSelector: ROOT });
    assert.deepEqual(violations, []);
  });
});

test('N3b: ラベルへ innerHTML で同じ形の文字列を入れると、危険な要素の違反として検出される', async () => {
  await withFixturePage(async (page) => {
    const mutate = (p) => p.evaluate((html) => {
      const li = document.querySelector('#view-fixture-root [data-testid="view-list"]:not([hidden]) > *');
      const label = li.querySelector('[data-testid="card-label"]');
      label.innerHTML = html;
    }, '<img><iframe></iframe><a href="javascript:void(0)">x</a>');
    const { violations } = await runViewCase(page, mutatingAdapter(page, mutate), { kind: 'card_list', variant: 'b', rootSelector: ROOT });
    assert.ok(violations.some((m) => m.includes('dangerous elements or attributes are present')), JSON.stringify(violations));
  });
});
