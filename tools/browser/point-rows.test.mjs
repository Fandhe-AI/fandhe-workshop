// 7 種のビューの指せる行の属性の試験（TASK-480 1/2。ART-11）。
//
// 代役の部品（view_fixture）は、各ビューのデータの先頭の行だけを表示だけにし、残りを指せる行にする。
// 7 種のビュー × 許される変種（差分・文書の (a) は対照の口）× 前後で次を確かめる。
// - PR1: 指せる行は data-action="ui"・data-payload・data-row・tabindex="0" の 4 属性を根の li に組で持つ
// - PR2: 先頭の行（表示だけ）は 4 属性を 1 つも持たない。内側の要素（印の宿主の div など）にも data-action は無い
// - PR3: 指せる行は li.focus() でフォーカスを受け、Tab の順にも入る
// - PR4: 指せる行の寸法が 24x24 CSS ピクセル以上（A11Y-3）
// - PR5: 外部へのリクエスト・ページのエラー・ダイアログがどれも 0 件
// クリックの効果と Enter・Space は確かめない（TASK-480 2/2 と point-keys.test.mjs の担当）。
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-point-rows/';
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

function observe(page, kind) {
  return page.evaluate(([root, k]) => {
    const list = document.querySelector(`${root} [data-testid="view-list"][data-view="${k}"]`);
    const items = [...list.querySelectorAll(':scope > li')];
    const four = (el) => ['data-action', 'data-payload', 'data-row', 'tabindex'].map((a) => el.hasAttribute(a));
    const rows = items.map((li) => {
      const r = li.getBoundingClientRect();
      return {
        action: li.getAttribute('data-action'),
        has: four(li),
        tabindex: li.getAttribute('tabindex'),
        w: r.width,
        h: r.height,
      };
    });
    return {
      rows,
      innerActions: list.querySelectorAll('li li[data-action], li div[data-action]').length,
    };
  }, [ROOT, kind]);
}

for (const [kind, variant, contrast] of CASES) {
  test(`PR1-PR4: ${kind} (${variant}) の指せる行の属性・フォーカス・寸法`, async () => {
    await withFixturePage(async (page, rec) => {
      if (contrast) await dispatch(page, 'fixture_contrast', `${kind}:${variant}`);
      else await dispatch(page, 'fixture_select', `${kind}:${variant}`);
      for (const phase of ['before', 'after']) {
        await dispatch(page, 'fixture_phase', phase);
        const o = await observe(page, kind);
        const detail = JSON.stringify({ kind, variant, phase, o });
        assert.ok(o.rows.length >= 2, detail);
        // 先頭の行だけが表示だけ。前の状態で先頭の行が出ない場合は、表示だけの行は 0 件になる。
        const plain = o.rows.filter((r) => !r.has.some(Boolean));
        const pointed = o.rows.filter((r) => r.has.every(Boolean));
        assert.ok(plain.length <= 1, detail);
        assert.equal(plain.length + pointed.length, o.rows.length, `4 属性は組で付く: ${detail}`);
        assert.ok(pointed.length >= o.rows.length - 1 && pointed.length >= 1, detail);
        for (const r of pointed) {
          assert.equal(r.action, 'ui', detail);
          assert.equal(r.tabindex, '0', detail);
          assert.ok(r.w >= 24 && r.h >= 24, `PR4: ${detail}`);
        }
        assert.equal(o.innerActions, 0, `PR2: 内側に data-action は無い: ${detail}`);

        // PR3: フォーカスと Tab の順。
        const focused = await page.evaluate(([root, k]) => {
          const list = document.querySelector(`${root} [data-testid="view-list"][data-view="${k}"]`);
          const li = list.querySelector(':scope > li[data-action="ui"]');
          li.focus();
          return document.activeElement === li;
        }, [ROOT, kind]);
        assert.equal(focused, true, `PR3: ${detail}`);
        await page.evaluate(() => document.activeElement.blur());
        let reached = false;
        for (let i = 0; i < 120 && !reached; i += 1) {
          await page.keyboard.press('Tab');
          reached = await page.evaluate(() => document.activeElement?.matches('li[data-action="ui"]') === true);
        }
        assert.equal(reached, true, `PR3: Tab で指せる行に届く: ${detail}`);
      }
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ kind, variant, counts, rec }));
    });
  });
}
