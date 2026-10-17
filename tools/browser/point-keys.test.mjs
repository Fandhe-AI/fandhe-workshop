// 指せる行の Enter・Space の試験（TASK-358 1/2。A11Y-18・FEND-13・D-42）。
//
// 指せる行（li に data-action="ui"・data-payload・data-row・tabindex="0" が組で付いた行）は標準の操作部品ではないため、
// glue が keydown を受けて行そのものへ click() を送る（wire_row_keys）。ここでは代役の部品（view_fixture）の root に付いた
// 結線を、フォーカスと信頼入力のキー（Playwright の keyboard）で確かめる。defaultPrevented は window の keydown で記録する。
// - PK1: 7 種のビュー × 許される変種（差分・文書の (a) は対照の口）× 前後で、フォーカスした指せる行の Enter・Space が
//   その行を指し、件数が 1 増え、defaultPrevented が真になる。Space でページがスクロールしない
// - PK2: 同じ行をマウスの click・Enter・Space で指したときの状態の変化が一致する
// - PK3: 押しっぱなし（repeat）は件数が 1 しか増えない。リピート中も defaultPrevented は真
// - PK4: 修飾キー付き（Shift+Enter・Control+Enter・Alt+Space・Meta+Enter）は件数が変わらず defaultPrevented は偽
// - PK5: 名札のデータ（appdemo:nameplate）の (a)・(b)。screen/top は Enter・Space で受け付けられ、
//   screen/left・screen/right はフォーカスを受けず、Enter を押しても件数が変わらない
// - PK6: 観察。アプリのデモ (b) の内側のボタンにフォーカスして Enter を押したときの件数（期待としては固定しない）
// - 各試験の末尾: 外部へのリクエスト・ページのエラー・ダイアログがどれも 0 件
// 変換中（IME）の行の Enter は native の row_key_effect の試験が固定する。
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
// 製品の画面（#app）の行への結線は TASK-470・TASK-347 の後続、キーボードだけの完走は TASK-358 2/2・TASK-375 の担当。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-point-keys/';
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
        // root のリスナーより後（window）で、keydown が既定の動作を止められたかを記録する。
        window.__keys = [];
        window.addEventListener('keydown', (e) => window.__keys.push({ key: e.key, prevented: e.defaultPrevented, repeat: e.repeat }));
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

const rowSel = (kind, id) => `${ROOT} [data-testid="view-list"][data-view="${kind}"] > li[data-row="${id}"]`;

// 指せる行の data-row を、見えている順に返す。
function pointedRows(page, kind) {
  return page.evaluate(([root, k]) => {
    const list = document.querySelector(`${root} [data-testid="view-list"][data-view="${k}"]`);
    return [...list.querySelectorAll(':scope > li[data-action="ui"]')].map((li) => li.getAttribute('data-row'));
  }, [ROOT, kind]);
}

const resetKeys = (page) => page.evaluate(() => { window.__keys.length = 0; });
const lastKey = (page) => page.evaluate(() => window.__keys.at(-1));

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

async function select(page, kind, variant, contrast) {
  await dispatch(page, contrast ? 'fixture_contrast' : 'fixture_select', `${kind}:${variant}`);
}

for (const [kind, variant, contrast] of CASES) {
  test(`PK1: ${kind} (${variant}) の指せる行の Enter と Space`, async () => {
    await withFixturePage(async (page, rec) => {
      await select(page, kind, variant, contrast);
      for (const phase of ['before', 'after']) {
        await dispatch(page, 'fixture_phase', phase);
        const detail = `${kind} ${variant} ${phase}`;
        const ids = await pointedRows(page, kind);
        assert.ok(ids.length >= 1, detail);
        for (const key of ['Enter', 'Space']) {
          const id = key === 'Enter' ? ids[0] : ids[ids.length - 1];
          const sel = rowSel(kind, id);
          await page.focus(sel);
          assert.equal(await page.evaluate((s) => document.activeElement === document.querySelector(s), sel), true, `${key} focus ${detail}`);
          const y0 = await page.evaluate(() => window.scrollY);
          const s0 = await fixtureState(page);
          await resetKeys(page);
          await page.keyboard.press(key);
          const s1 = await fixtureState(page);
          assert.deepEqual(s1.pointed, { view: `fixture-${kind}`, row: id }, `${key} ${detail}`);
          assert.equal(s1.point_count, s0.point_count + 1, `${key} ${detail}`);
          assert.equal((await lastKey(page)).prevented, true, `${key} defaultPrevented ${detail}`);
          assert.equal(await page.evaluate(() => window.scrollY), y0, `${key} scroll ${detail}`);
        }
      }
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ kind, variant, counts, rec }));
    });
  });
}

test('PK2: click・Enter・Space で状態の変化が一致する', async () => {
  await withFixturePage(async (page, rec) => {
    await select(page, 'check_list', 'a', false);
    await dispatch(page, 'fixture_phase', 'before');
    const ids = await pointedRows(page, 'check_list');
    const sel = rowSel('check_list', ids[0]);
    const results = [];
    for (const how of ['click', 'Enter', 'Space']) {
      const s0 = await fixtureState(page);
      if (how === 'click') {
        await page.evaluate((s) => document.querySelector(s).click(), sel);
      } else {
        await page.focus(sel);
        await page.keyboard.press(how);
      }
      const s1 = await fixtureState(page);
      results.push({ pointed: s1.pointed, delta: s1.point_count - s0.point_count });
    }
    assert.deepEqual(results[1], results[0], 'Enter は click と同じ');
    assert.deepEqual(results[2], results[0], 'Space は click と同じ');
    assert.equal(results[0].delta, 1);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});

test('PK3: 押しっぱなしは 1 回だけ受け付ける', async () => {
  await withFixturePage(async (page, rec) => {
    await select(page, 'card_list', 'a', false);
    await dispatch(page, 'fixture_phase', 'before');
    const ids = await pointedRows(page, 'card_list');
    const sel = rowSel('card_list', ids[0]);
    for (const key of ['Enter', 'Space']) {
      await page.focus(sel);
      const s0 = await fixtureState(page);
      await resetKeys(page);
      for (let i = 0; i < 3; i += 1) await page.keyboard.down(key);
      await page.keyboard.up(key);
      const s1 = await fixtureState(page);
      assert.equal(s1.point_count, s0.point_count + 1, `${key} repeat`);
      const keys = await page.evaluate(() => window.__keys.slice());
      assert.ok(keys.length >= 3 && keys.some((k) => k.repeat), `${key} repeat events: ${JSON.stringify(keys)}`);
      assert.ok(keys.every((k) => k.prevented), `${key} repeat prevented: ${JSON.stringify(keys)}`);
    }
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});

test('PK4: 修飾キー付きは何もしない', async () => {
  await withFixturePage(async (page, rec) => {
    await select(page, 'check_list', 'b', false);
    await dispatch(page, 'fixture_phase', 'before');
    const ids = await pointedRows(page, 'check_list');
    const sel = rowSel('check_list', ids[0]);
    for (const combo of ['Shift+Enter', 'Control+Enter', 'Alt+Space', 'Meta+Enter']) {
      await page.focus(sel);
      const s0 = await fixtureState(page);
      await resetKeys(page);
      await page.keyboard.press(combo);
      const s1 = await fixtureState(page);
      assert.equal(s1.point_count, s0.point_count, combo);
      const last = (await page.evaluate(() => window.__keys.slice())).filter((k) => k.key === 'Enter' || k.key === ' ').at(-1);
      assert.ok(last, `${combo} keydown が届いていない`);
      assert.equal(last.prevented, false, `${combo} defaultPrevented`);
    }
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});

for (const variant of ['a', 'b']) {
  test(`PK5: アプリのデモ (${variant}) の名札の行`, async () => {
    await withFixturePage(async (page, rec) => {
      await dispatch(page, 'fixture_select', `app_demo:${variant}`);
      await dispatch(page, 'fixture_data', 'appdemo:nameplate');
      for (const phase of ['before', 'after']) {
        await dispatch(page, 'fixture_phase', phase);
        const detail = `app_demo ${variant} ${phase}`;
        const top = rowSel('app_demo', 'screen/top');
        for (const key of ['Enter', 'Space']) {
          await page.focus(top);
          const s0 = await fixtureState(page);
          await page.keyboard.press(key);
          const s1 = await fixtureState(page);
          assert.deepEqual(s1.pointed, { view: 'fixture-app_demo', row: 'screen/top' }, `${key} ${detail}`);
          assert.equal(s1.point_count, s0.point_count + 1, `${key} ${detail}`);
        }
        // 表示だけの行は、focus() してもフォーカスを受けない。続けて Enter を押しても、その行は何も受け付けない。
        for (const id of ['screen/left', 'screen/right']) {
          await page.evaluate(() => document.activeElement?.blur());
          await page.evaluate(([root, key]) => {
            const li = [...document.querySelectorAll(`${root} [data-testid="view-list"][data-view="app_demo"] > li`)].find((x) => x.getAttribute('data-key') === `k${key}`);
            li.focus();
          }, [ROOT, id]);
          const tag = await page.evaluate(() => document.activeElement?.tagName);
          assert.equal(tag, 'BODY', `${id} はフォーカスを受けない ${detail}`);
          const s0 = await fixtureState(page);
          await page.keyboard.press('Enter');
          const s1 = await fixtureState(page);
          assert.equal(s1.point_count, s0.point_count, `${id} ${detail}`);
        }
      }
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ variant, counts, rec }));
    });
  });
}

test('PK6: アプリのデモ (b) の内側のボタンの Enter の観察', async () => {
  await withFixturePage(async (page, rec) => {
    await dispatch(page, 'fixture_select', 'app_demo:b');
    const ok = await page.evaluate(([root]) => {
      const btn = document.querySelector(`${root} [data-testid="view-list"][data-view="app_demo"] li[data-action="ui"] button`);
      if (!btn) return false;
      btn.focus();
      return document.activeElement === btn;
    }, [ROOT]);
    assert.ok(ok, '内側のボタンにフォーカスできない');
    const s0 = await fixtureState(page);
    await page.keyboard.press('Enter');
    const s1 = await fixtureState(page);
    // 期待としては固定しない。glue は li 以外の target には作用せず、ブラウザの既定の動作（ボタンの click）に任せる。
    console.log(`PK6 observation: point_count ${s0.point_count} -> ${s1.point_count} (inner button Enter)`);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});
