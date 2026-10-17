// 指せる行のクリックの試験（TASK-480 2/2。ART-11）。
//
// 代役の部品（view_fixture）は、データごとの規則で指せる行を決める。既定は先頭の行だけ表示だけ、名札のデータは screen/top だけが指せる行である。
// クリックは実際のマウスで行い、上流の委譲（root で closest("[data-action]") を解決する）の経路を通す。
// - PC1: 7 種のビュー × 許される変種（差分・文書の (a) は対照の口）× 前後で、指せる行を 1 件クリックすると、
//   その行の宛先（view と row）が「指した行」になり、受け付けた件数が 1 増える
// - PC2: 同じ組合せで、表示だけの行（先頭の行。出ているとき）をクリックしても何も受け付けない
// - PC3: アプリのデモ (a)・(b) で名札のデータに差し替えると、screen/top だけが指せる行になる。クリックで受け付けられ、
//   screen/left・screen/right は 4 属性を持たず、クリックしても何も受け付けない
// - PC4: 観察。アプリのデモ (b) の内側のボタンの上のクリックは、上流が行の action として届けない（独立した対話要素の境界）。
//   この観察は期待として固定せず、結果を記録する
// - PC5: 外部へのリクエスト・ページのエラー・ダイアログがどれも 0 件
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
// Enter・Space の結線は TASK-358・TASK-375、製品の画面への結線は TASK-470・TASK-347 の後続の担当。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-point-click/';
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

// [kind, variant, 対照の口か]
const CASES = [
  ['file_tree', 'a', false], ['file_tree', 'b', false],
  ['task_tree', 'a', false], ['task_tree', 'b', false],
  ['diff', 'b', false], ['diff', 'a', true],
  ['document', 'b', false], ['document', 'a', true],
  ['card_list', 'a', false], ['card_list', 'b', false],
  ['check_list', 'a', false], ['check_list', 'b', false],
  ['app_demo', 'a', false], ['app_demo', 'b', false],
];

const INTERACTIVE = 'button, input, textarea, select, a, [role], [tabindex]';

// 一覧の li のうち、selector に合う最初のものを画面の中央へ出し、行の action に解決される点を探す。
// 点の上の要素から li までの間に、独立した対話要素（button・input・role・tabindex など）が無いことを条件にする。
// 見つからなければ null を返し、呼び出し側が試験を失敗にする（黙って飛ばさない）。
function findClickPoint(page, kind, rowId) {
  return page.evaluate(([root, k, id, interactive]) => {
    const list = document.querySelector(`${root} [data-testid="view-list"][data-view="${k}"]`);
    const li = [...list.querySelectorAll(':scope > li')].find((x) => (id === null ? !x.hasAttribute('data-action') : x.getAttribute('data-row') === id));
    if (!li) return { found: false, reason: 'no row' };
    li.scrollIntoView({ block: 'center' });
    const r = li.getBoundingClientRect();
    for (let iy = 1; iy <= 7; iy += 1) {
      for (let ix = 1; ix <= 9; ix += 1) {
        const x = r.left + (r.width * ix) / 10;
        const y = r.top + (r.height * iy) / 8;
        const el = document.elementFromPoint(x, y);
        if (!el || !li.contains(el)) continue;
        // 行の action に解決されるのは、li に達するまで data-action を持たない点。
        if (id !== null && el.closest('[data-action]') !== li) continue;
        let blocked = false;
        for (let n = el; n && n !== li; n = n.parentElement) {
          if (n.matches(interactive)) { blocked = true; break; }
        }
        if (!blocked) return { found: true, x, y };
      }
    }
    return { found: false, reason: 'no clean point' };
  }, [ROOT, kind, rowId, INTERACTIVE]);
}

// 指せる行（data-action="ui"）と表示だけの行（持たない行）の data-row を、見えている順に返す。
function listRows(page, kind) {
  return page.evaluate(([root, k]) => {
    const list = document.querySelector(`${root} [data-testid="view-list"][data-view="${k}"]`);
    const items = [...list.querySelectorAll(':scope > li')];
    return {
      pointed: items.filter((li) => li.getAttribute('data-action') === 'ui').map((li) => li.getAttribute('data-row')),
      plain: items.filter((li) => !li.hasAttribute('data-action')).length,
      total: items.length,
    };
  }, [ROOT, kind]);
}

async function mouseClick(page, point) {
  await page.mouse.click(point.x, point.y);
}

for (const [kind, variant, contrast] of CASES) {
  test(`PC1・PC2: ${kind} (${variant}) の指せる行のクリックと表示だけの行のクリック`, async () => {
    await withFixturePage(async (page, rec) => {
      if (contrast) await dispatch(page, 'fixture_contrast', `${kind}:${variant}`);
      else await dispatch(page, 'fixture_select', `${kind}:${variant}`);
      for (const phase of ['before', 'after']) {
        await dispatch(page, 'fixture_phase', phase);
        const detail = `${kind} ${variant} ${phase}`;
        const rows = await listRows(page, kind);
        assert.ok(rows.pointed.length >= 1, detail);

        // PC1: 指せる行を 1 件クリックする。クリックできる点が見つかる行を順に探す。
        let clicked = null;
        for (const id of rows.pointed) {
          const p = await findClickPoint(page, kind, id);
          if (p.found) { clicked = { id, p }; break; }
        }
        assert.ok(clicked, `PC1: クリックできる点を持つ指せる行が無い: ${detail}`);
        const s0 = await fixtureState(page);
        await mouseClick(page, clicked.p);
        const s1 = await fixtureState(page);
        assert.deepEqual(s1.pointed, { view: `fixture-${kind}`, row: clicked.id }, `PC1: ${detail}`);
        assert.equal(s1.point_count, s0.point_count + 1, `PC1: ${detail}`);

        // PC2: 表示だけの行（出ているとき）をクリックしても何も受け付けない。
        if (rows.plain > 0) {
          const p = await findClickPoint(page, kind, null);
          assert.ok(p.found, `PC2: 表示だけの行をクリックする点が無い: ${detail} ${JSON.stringify(p)}`);
          await mouseClick(page, p);
          const s2 = await fixtureState(page);
          assert.deepEqual(s2.pointed, s1.pointed, `PC2: ${detail}`);
          assert.equal(s2.point_count, s1.point_count, `PC2: ${detail}`);
        }
      }
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ kind, variant, counts, rec }));
    });
  });
}

for (const variant of ['a', 'b']) {
  test(`PC3: アプリのデモ (${variant}) の名札の行`, async () => {
    await withFixturePage(async (page, rec) => {
      await dispatch(page, 'fixture_select', `app_demo:${variant}`);
      await dispatch(page, 'fixture_data', 'appdemo:nameplate');
      for (const phase of ['before', 'after']) {
        await dispatch(page, 'fixture_phase', phase);
        const detail = `app_demo ${variant} ${phase}`;
        const o = await page.evaluate(([root]) => {
          const list = document.querySelector(`${root} [data-testid="view-list"][data-view="app_demo"]`);
          const top = [...list.querySelectorAll('[data-row="screen/top"]')];
          const attrs = (li) => ['data-action', 'data-payload', 'data-row', 'tabindex'].map((a) => li.hasAttribute(a));
          const keyed = (id) => [...list.querySelectorAll(':scope > li')].filter((li) => li.getAttribute('data-key') === `k${id}`);
          return {
            topCount: top.length,
            topAction: top[0]?.getAttribute('data-action'),
            topTab: top[0]?.getAttribute('tabindex'),
            all: [...list.querySelectorAll(':scope > li')].length,
            withData: list.querySelectorAll('[data-row]').length,
            left: keyed('screen/left').map(attrs),
            right: keyed('screen/right').map(attrs),
          };
        }, [ROOT]);
        assert.equal(o.topCount, 1, detail);
        assert.equal(o.topAction, 'ui', detail);
        assert.equal(o.topTab, '0', detail);
        assert.equal(o.all, 3, detail);
        assert.equal(o.withData, 1, `指せる行は screen/top だけ: ${detail}`);
        assert.deepEqual(o.left, [[false, false, false, false]], detail);
        assert.deepEqual(o.right, [[false, false, false, false]], detail);

        const s0 = await fixtureState(page);
        const top = await findClickPoint(page, 'app_demo', 'screen/top');
        assert.ok(top.found, `${detail} ${JSON.stringify(top)}`);
        await mouseClick(page, top);
        const s1 = await fixtureState(page);
        assert.deepEqual(s1.pointed, { view: 'fixture-app_demo', row: 'screen/top' }, detail);
        assert.equal(s1.point_count, s0.point_count + 1, detail);

        // 表示だけの行（screen/left・screen/right）は、クリックしても何も受け付けない。
        for (const id of ['screen/left', 'screen/right']) {
          const p = await page.evaluate(([root, key]) => {
            const li = [...document.querySelectorAll(`${root} [data-testid="view-list"][data-view="app_demo"] > li`)].find((x) => x.getAttribute('data-key') === `k${key}`);
            li.scrollIntoView({ block: 'center' });
            const r = li.getBoundingClientRect();
            return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
          }, [ROOT, id]);
          await mouseClick(page, p);
          const s2 = await fixtureState(page);
          assert.deepEqual(s2.pointed, s1.pointed, `${id} ${detail}`);
          assert.equal(s2.point_count, s1.point_count, `${id} ${detail}`);
        }
      }
      // 元のデータに戻すと、既定の規則（先頭の行だけ表示だけ）に戻る。
      await dispatch(page, 'fixture_data', 'appdemo:sample');
      const rows = await listRows(page, 'app_demo');
      assert.equal(rows.plain, 1, '既定の規則では先頭の行だけが表示だけ');
      assert.equal(rows.pointed.length, 8, '既定の規則では残りの行が指せる行');
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ variant, counts, rec }));
    });
  });
}

test('PC3: 閉じていない payload の差し替えは受け付けない', async () => {
  await withFixturePage(async (page) => {
    await dispatch(page, 'fixture_select', 'app_demo:b');
    for (const bad of ['appdemo:', 'appdemo:20', 'Appdemo:nameplate', 'cards:nameplate']) {
      const r = JSON.parse(await page.evaluate(([n, p]) => window.__fandheTest.viewFixtureDispatch(n, p), ['fixture_data', bad]));
      assert.notEqual(r.outcome, 'applied', bad);
    }
    const s = await fixtureState(page);
    assert.equal(s.point_count, 0);
  });
});

test('PC4: アプリのデモ (b) の内側のボタンの上のクリックの観察', async () => {
  await withFixturePage(async (page, rec) => {
    await dispatch(page, 'fixture_select', 'app_demo:b');
    const p = await page.evaluate(([root]) => {
      const list = document.querySelector(`${root} [data-testid="view-list"][data-view="app_demo"]`);
      const btn = list.querySelector('li[data-action="ui"] button');
      if (!btn) return null;
      btn.scrollIntoView({ block: 'center' });
      const r = btn.getBoundingClientRect();
      return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
    }, [ROOT]);
    assert.ok(p, '内側のボタンを持つ指せる行が無い');
    const s0 = await fixtureState(page);
    await mouseClick(page, p);
    const s1 = await fixtureState(page);
    // 期待としては固定しない。上流は、点が独立した対話要素の上なら行の action を届けない。観察の結果を出力に残す。
    console.log(`PC4 observation: point_count ${s0.point_count} -> ${s1.point_count} (inner button click)`);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});
