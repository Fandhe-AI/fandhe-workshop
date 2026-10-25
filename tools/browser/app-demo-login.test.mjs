// 模擬ログインの画面にパスワード欄がないことの試験（TASK-492。ART-24・ROUND1-25）。
//
// アプリのデモのワイヤー (a)・(b) に、代役の部品の模擬ログインのデータ（appdemo:login。前 6 行・後 7 行）を渡し、
// パスワードの欄・属性・文言が 0 件であること、名前の行を指すだけで進めることを確かめる。
// 5.283（ch4-9-run。製品の画面の全 Step でパスワード欄 0 件）とは層が違う。本試験は左のワイヤーのビューの中を見る。
// - ML1: (a)・(b) × 前 → 後 → 前で、ページ全体の passwordFields()・一覧の文言・一覧の type=password がどれも 0 件。(b) は読み取り専用の type=text の欄がある（空振り防止）
// - ML2: 名前の行を実クリックすると「指した行」になり件数が 1 増える。その後に画面を後に切り替えても 0 件。入力欄への入力・送信は一度もしない
// - ML3: 表示だけの行（title・enter・who）は 4 属性を持たず、クリックしても受け付けない
// - ML4: 検出器の負の対照（データ）。appdemo:login-contrast では (a)・(b) とも文言が 1 件以上。(b) は passwordFields() も 1。(a) の欄は本物の input でないので 0 のまま（観察）
// - ML5: 検出器の負の対照（DOM）。(b) の一覧に type=password を 1 件足すと 1 件になり、外すと 0 に戻る
// - ML6: 外部へのリクエスト・ページのエラー・ダイアログがどれも 0 件
// 限界: 「進む」は代役の部品では指した行の記録までで、phase は試験が切り替える。製品での受付は 5.283 に頼る。
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { passwordFields } from './ch4-9-journey.mjs';

const BASE = '/fandhe-app-demo-login/';
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

async function dispatch(page, name, payload) {
  const r = JSON.parse(await page.evaluate(([n, p]) => window.__fandheTest.viewFixtureDispatch(n, p), [name, payload]));
  assert.equal(r.outcome, 'applied', `${name} ${payload}`);
}

async function fixtureState(page) {
  return JSON.parse(await page.evaluate(() => window.__fandheTest.viewFixtureState()));
}

/** 一覧の中の計数。ページ内で評価する自己完結の関数。 */
async function listCounts(page) {
  return page.evaluate((sel) => {
    const list = document.querySelector(sel);
    const word = /パスワード|password|暗証|パスコード|passcode/i;
    let words = list.textContent.match(new RegExp(word.source, 'gi'))?.length ?? 0;
    for (const el of list.querySelectorAll('*')) {
      for (const a of ['placeholder', 'aria-label', 'title', 'value', 'autocomplete']) {
        if (word.test(el.getAttribute(a) ?? '')) words += 1;
      }
    }
    const pw = [...list.querySelectorAll('input')].filter((i) => (i.getAttribute('type') ?? '').toLowerCase() === 'password').length;
    const inputs = [...list.querySelectorAll('input')].map((i) => ({ type: i.getAttribute('type'), ro: i.hasAttribute('readonly') }));
    return { items: list.querySelectorAll(':scope > li').length, words, pw, inputs };
  }, LIST);
}

async function zero(page, detail) {
  assert.equal(await page.evaluate(passwordFields), 0, `passwordFields ${detail}`);
  const c = await listCounts(page);
  assert.equal(c.words, 0, `文言 ${detail}`);
  assert.equal(c.pw, 0, `type=password ${detail}`);
  return c;
}

async function rowPoint(page, id) {
  return page.evaluate(([sel, key]) => {
    const li = [...document.querySelectorAll(`${sel} > li`)].find((x) => x.getAttribute('data-key') === `k${key}`);
    if (!li) return null;
    li.scrollIntoView({ block: 'center' });
    const r = li.getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  }, [LIST, id]);
}

async function setup(page, variant, data = 'appdemo:login') {
  await dispatch(page, 'fixture_select', `app_demo:${variant}`);
  await dispatch(page, 'fixture_data', data);
}

for (const variant of ['a', 'b']) {
  test(`ML1: 模擬ログイン (${variant}) の前後でパスワード欄・文言が 0 件`, async () => {
    await withFixturePage(async (page, rec) => {
      await setup(page, variant);
      for (const [phase, items] of [['before', 6], ['after', 7], ['before', 6]]) {
        await dispatch(page, 'fixture_phase', phase);
        const c = await zero(page, `${variant} ${phase}`);
        assert.equal(c.items, items, `${variant} ${phase}`);
        if (variant === 'b') {
          assert.equal(c.inputs.length, 1, '名前の欄が 1 件ある');
          assert.deepEqual(c.inputs[0], { type: 'text', ro: true });
        }
      }
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ variant, counts, rec })); // ML6
    });
  });

  test(`ML2・ML3: 模擬ログイン (${variant}) は名前の行を指すだけで進み、表示だけの行は受け付けない`, async () => {
    await withFixturePage(async (page, rec) => {
      await setup(page, variant);
      await dispatch(page, 'fixture_phase', 'before');
      // ML3: 表示だけの行。
      for (const id of ['login/title', 'login/enter', 'login/who']) {
        const attrs = await page.evaluate(([sel, key]) => {
          const li = [...document.querySelectorAll(`${sel} > li`)].find((x) => x.getAttribute('data-key') === `k${key}`);
          return ['data-action', 'data-payload', 'data-row', 'tabindex'].map((a) => li.hasAttribute(a));
        }, [LIST, id]);
        assert.deepEqual(attrs, [false, false, false, false], id);
        const s0 = await fixtureState(page);
        const p = await rowPoint(page, id);
        assert.ok(p, id);
        await page.mouse.click(p.x, p.y);
        assert.equal((await fixtureState(page)).point_count, s0.point_count, id);
      }
      // ML2: 名前の行。
      const s0 = await fixtureState(page);
      const p = await rowPoint(page, 'login/name-1');
      assert.ok(p, 'login/name-1');
      await page.mouse.click(p.x, p.y);
      const s1 = await fixtureState(page);
      assert.deepEqual(s1.pointed, { view: 'fixture-app_demo', row: 'login/name-1' });
      assert.equal(s1.point_count, s0.point_count + 1);
      await zero(page, `${variant} 指した後（前）`);
      await dispatch(page, 'fixture_phase', 'after');
      const c = await zero(page, `${variant} 指した後（後）`);
      assert.equal(c.items, 7);
      const text = await page.evaluate((sel) => document.querySelector(sel).textContent, LIST);
      assert.ok(text.includes('ログイン中（模擬）') && text.includes('さくら として入りました'), text);
      if (variant === 'b') {
        const v = await page.evaluate((sel) => document.querySelector(`${sel} input`).value, LIST);
        assert.equal(v, 'さくら');
      }
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ variant, counts, rec }));
    });
  });

  test(`ML4: 対照のデータ (${variant}) では検出器が文言を数える`, async () => {
    await withFixturePage(async (page) => {
      await setup(page, variant, 'appdemo:login-contrast');
      for (const phase of ['before', 'after']) {
        await dispatch(page, 'fixture_phase', phase);
        const c = await listCounts(page);
        assert.ok(c.words >= 1, `${variant} ${phase} ${JSON.stringify(c)}`);
        const pf = await page.evaluate(passwordFields);
        // (b) は label と input が結ばれるので検出する。(a) の欄は本物の input でないので 0 のまま（観察）。
        assert.equal(pf, variant === 'b' ? 1 : 0, `passwordFields ${variant} ${phase}`);
      }
      await dispatch(page, 'fixture_data', 'appdemo:login');
      await dispatch(page, 'fixture_phase', 'before');
      await zero(page, `${variant} 戻した後`);
    });
  });
}

test('ML5: 一覧に type=password を足すと検出され、外すと 0 件に戻る', async () => {
  await withFixturePage(async (page) => {
    await setup(page, 'b');
    await dispatch(page, 'fixture_phase', 'before');
    await zero(page, '注入の前');
    await page.evaluate((sel) => {
      const i = document.createElement('input');
      i.setAttribute('type', 'password');
      i.id = 'ml5-injected';
      document.querySelector(sel).append(i);
    }, LIST);
    assert.equal(await page.evaluate(passwordFields), 1);
    assert.equal((await listCounts(page)).pw, 1);
    await page.evaluate(() => document.getElementById('ml5-injected').remove());
    await zero(page, '注入を外した後');
  });
});
