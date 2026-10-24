// カードのビューの変化の印が行単位で、どの欄が変わったかを示さないことの観察の固定（TASK-487。ART-17・D-49）。
//
// D-49（2026-10-12）は「行単位の印のまま。差のある欄の一覧は行データに持たせない」と決めた。この試験は、
// 欄単位の表示を足さない現状を固定し、足す変更が入ったときに落ちて見直しを促す。製品の画面へは未結線で、
// 代役の部品（TEST_HOOKS=1 の view_fixture）が card_view で描くカードを観察する（限界は docs/known-gaps.md の ART-17）。
// 土台は card-view.test.mjs と同じ（配信・代役の口・安全性の計数）。
// - RM1: (a)・(b) の各変種で、状態が下書きからオープンに変わる pr1 に、印が行の 1 か所（change・~・【変更】）だけ出て、欄の要素に印が無い
// - RM2: (a)・(b) の各変種で、状態の要素の属性が値だけで決まり（変更ありの pr1 とオープンで印なしの pr2 が一致）、変化の前後で欄の属性の名前が変わらない
// - RM3: 前後を 2 往復しても、各時点の観察が変わらない
// - RM4: 負の対照。状態の要素に欄単位の印を足すと、RM1・RM2 の判定が違反として検出する
// - RM5: 上の操作の間、外部へのリクエスト・ページのエラー・ダイアログがどれも 0 件
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-card-row-mark/';
const ROOT = '#view-fixture-root';
const LIST = `${ROOT} [data-testid="view-list"][data-view="card_list"]`;
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

async function dispatch(page, name, payload) {
  const r = JSON.parse(await page.evaluate(([n, p]) => window.__fandheTest.viewFixtureDispatch(n, p), [name, payload]));
  assert.equal(r.outcome, 'applied', `${name} ${payload}`);
}

// 欄ごとの要素を引く CSS（行の li からの相対）と、印の宿主。
const FIELDS = {
  a: { state: '.fw-wire-card-basic .fw-wire-tag', label: ':scope > div > .fw-wire-tag', card: '.fw-wire-card-basic', host: ':scope > div[data-mark]' },
  b: {
    state: '[data-scope="marker"][data-part="root"]',
    label: '[data-testid="card-label"]',
    detail: '[data-testid="card-detail"]',
    tags: '[data-testid="card-tags"]',
    host: ':scope[data-mark]',
  },
};

// 行 1 件の観察。欄の要素の署名（タグ名と属性の組）と、印の数・位置・文字を集める。
function observeRow(page, variant, key) {
  return page.evaluate(([listSel, fields, k]) => {
    const list = document.querySelector(listSel);
    const li = [...list.querySelectorAll(':scope > li')].find((x) => x.getAttribute('data-key') === k);
    if (!li) return null;
    const sig = (el) => (el ? { tag: el.tagName, attrs: [...el.attributes].map((a) => [a.name, a.value]).sort() } : null);
    const marksIn = (el) => ({
      dataMark: el ? el.querySelectorAll('[data-mark]').length + (el.hasAttribute('data-mark') ? 1 : 0) : 0,
      markText: el ? el.querySelectorAll('[data-testid="mark-text"], [data-testid="mark-glyph"]').length : 0,
      bracket: el ? (el.textContent.match(/【/g) || []).length : 0,
    });
    const field = {};
    const fieldMarks = {};
    for (const [name, sel] of Object.entries(fields)) {
      if (name === 'host') continue;
      const el = li.querySelector(sel);
      field[name] = sig(el);
      fieldMarks[name] = marksIn(el);
    }
    const hosts = fields.host === ':scope[data-mark]'
      ? (li.hasAttribute('data-mark') ? [li] : [])
      : [...li.querySelectorAll(fields.host)];
    const marked = [li, ...li.querySelectorAll('[data-mark]')]
      .filter((e) => e.hasAttribute('data-mark') && e.getAttribute('data-mark') !== '');
    const text = (id) => [...li.querySelectorAll(`[data-testid="${id}"]`)].map((e) => e.textContent);
    return {
      hostCount: hosts.length,
      markValues: marked.map((e) => e.getAttribute('data-mark')),
      markedAreHosts: marked.every((e) => hosts.includes(e)),
      markText: text('mark-text'),
      markGlyph: text('mark-glyph'),
      bracketNodes: (li.textContent.match(/【/g) || []).length,
      field,
      fieldMarks,
    };
  }, [LIST, FIELDS[variant], key]);
}

const zeroMarks = (o) => Object.values(o.fieldMarks).every((m) => m.dataMark === 0 && m.markText === 0 && m.bracket === 0);

// RM1 の判定。違反を文字列の配列で返す。
function violationsRm1(o, changed) {
  const v = [];
  if (!o) return ['row missing'];
  if (!zeroMarks(o)) v.push('field element has a mark');
  if (changed) {
    if (o.hostCount !== 1) v.push(`host count ${o.hostCount}`);
    if (JSON.stringify(o.markValues) !== '["change"]') v.push(`mark values ${o.markValues}`);
    if (!o.markedAreHosts) v.push('mark outside host');
    if (JSON.stringify(o.markText) !== '["【変更】"]') v.push(`mark-text ${o.markText}`);
    if (JSON.stringify(o.markGlyph) !== '["~"]') v.push(`mark-glyph ${o.markGlyph}`);
    if (o.bracketNodes !== 1) v.push(`bracket ${o.bracketNodes}`);
  // 変更のない行も印の要素は残す（前後で子の列を変えない）。文字と値が空であることを見る。
  } else if (o.markValues.length !== 0 || o.markText.some((t) => t !== '') || o.markGlyph.some((t) => t !== '') || o.bracketNodes !== 0) {
    v.push('unchanged row has a mark');
  }
  return v;
}

const attrNames = (s) => (s ? s.attrs.map(([n]) => n) : null);

// RM2 の判定。before の pr1・after の pr1・before の pr2 の観察を渡す。
function violationsRm2(pr1Before, pr1After, pr2Before) {
  const v = [];
  if (JSON.stringify(pr1After.field.state) !== JSON.stringify(pr2Before.field.state)) {
    v.push('state element differs between changed and unchanged rows of the same value');
  }
  for (const name of Object.keys(pr1After.field)) {
    if (JSON.stringify(attrNames(pr1Before.field[name])) !== JSON.stringify(attrNames(pr1After.field[name]))) {
      v.push(`attribute names of ${name} change with the row's change`);
    }
  }
  return v;
}

function stateText(page, variant, key) {
  return page.evaluate(([listSel, sel, k]) => {
    const li = [...document.querySelector(listSel).querySelectorAll(':scope > li')].find((x) => x.getAttribute('data-key') === k);
    return li.querySelector(sel).textContent.trim();
  }, [LIST, FIELDS[variant].state, key]);
}

// 負の対照の注入。固定の文字列だけを使い、innerHTML は使わない。
function inject(page, variant, n) {
  return page.evaluate(([listSel, sel, i]) => {
    const li = [...document.querySelector(listSel).querySelectorAll(':scope > li')].find((x) => x.getAttribute('data-key') === 'kpr1');
    const el = li.querySelector(sel);
    if (i === 0) el.setAttribute('data-mark', 'change');
    else if (i === 1) {
      const s = document.createElement('span');
      s.setAttribute('data-testid', 'mark-text');
      s.textContent = '【変更】';
      el.append(s);
    } else el.setAttribute('data-changed', '');
  }, [LIST, FIELDS[variant].state, n]);
}

for (const variant of ['a', 'b']) {
  test(`RM1: 変種 (${variant}) の下書き -> オープンで、印は行の 1 か所だけで欄の要素に無い`, async () => {
    await withFixturePage(async (page) => {
      await dispatch(page, 'fixture_select', `card_list:${variant}`);
      await dispatch(page, 'fixture_phase', 'before');
      assert.equal(await stateText(page, variant, 'kpr1'), '下書き');
      assert.deepEqual(violationsRm1(await observeRow(page, variant, 'kpr1'), false), []);
      await dispatch(page, 'fixture_phase', 'after');
      assert.equal(await stateText(page, variant, 'kpr1'), 'オープン');
      const o = await observeRow(page, variant, 'kpr1');
      assert.deepEqual(violationsRm1(o, true), [], JSON.stringify(o));
    });
  });

  test(`RM2: 変種 (${variant}) の欄の属性は値だけで決まり、変化の有無で変わらない`, async () => {
    await withFixturePage(async (page) => {
      await dispatch(page, 'fixture_select', `card_list:${variant}`);
      await dispatch(page, 'fixture_phase', 'before');
      const pr1Before = await observeRow(page, variant, 'kpr1');
      const pr2Before = await observeRow(page, variant, 'kpr2');
      assert.equal(await stateText(page, variant, 'kpr2'), 'オープン');
      await dispatch(page, 'fixture_phase', 'after');
      const pr1After = await observeRow(page, variant, 'kpr1');
      assert.deepEqual(violationsRm2(pr1Before, pr1After, pr2Before), [], JSON.stringify({ pr1Before, pr1After, pr2Before }));
    });
  });

  test(`RM3: 変種 (${variant}) の前後 2 往復で観察が変わらない`, async () => {
    await withFixturePage(async (page) => {
      await dispatch(page, 'fixture_select', `card_list:${variant}`);
      let firstAfter;
      for (let round = 0; round < 2; round += 1) {
        await dispatch(page, 'fixture_phase', 'before');
        assert.deepEqual(violationsRm1(await observeRow(page, variant, 'kpr1'), false), []);
        await dispatch(page, 'fixture_phase', 'after');
        const a = await observeRow(page, variant, 'kpr1');
        assert.deepEqual(violationsRm1(a, true), []);
        firstAfter ??= a;
        assert.deepEqual(a, firstAfter);
      }
    });
  });

  test(`RM4: 変種 (${variant}) の負の対照。状態の要素への欄単位の印を検出する`, async () => {
    for (const n of [0, 1, 2]) {
      await withFixturePage(async (page) => {
        await dispatch(page, 'fixture_select', `card_list:${variant}`);
        await dispatch(page, 'fixture_phase', 'before');
        const pr1Before = await observeRow(page, variant, 'kpr1');
        const pr2Before = await observeRow(page, variant, 'kpr2');
        await dispatch(page, 'fixture_phase', 'after');
        await inject(page, variant, n);
        const o = await observeRow(page, variant, 'kpr1');
        const found = [...violationsRm1(o, true), ...violationsRm2(pr1Before, o, pr2Before)];
        assert.ok(found.length > 0, `injection ${n} was not detected: ${JSON.stringify(o)}`);
      });
    }
  });

  test(`RM5: 変種 (${variant}) の切り替えで外部へのリクエスト・ページのエラー・ダイアログは 0 件`, async () => {
    await withFixturePage(async (page, rec) => {
      await dispatch(page, 'fixture_select', `card_list:${variant}`);
      for (const phase of ['after', 'before', 'after']) await dispatch(page, 'fixture_phase', phase);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
    });
  });
}
