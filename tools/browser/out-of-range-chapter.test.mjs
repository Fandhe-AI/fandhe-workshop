// 範囲外の章へ進まないことの試験（TASK-527。SHELL-12・D-67・SAFE-33）。
//
// 試行や体験では教材の一部の章だけを使う。D-67 は「IR を絞る」案を採り、配る IR の `starts` に範囲外の開始点を含めない。
// 試験の設定（TEST_HOOKS=1）の `?fixture=ch0-3-trial`（`src/test_fixture.rs` の `ch03_trial_ir_bytes(false)`）は、範囲外の章 CH-4 の
// ノード（`main/st-4-1`）を IR に残したまま、開始点を CH-0〜CH-3 の先頭 4 件だけにした IR である。
// 対照の `?fixture=ch0-4-full` は同じ IR に CH-4 の開始点（`s-4-1`）を足したもの。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい画面を開く）:
//   (a) 絞った IR の通し: CH-0 から CH-3 の最後の達成まで進めても、範囲外の章へ入る操作（「次の章へ進む」・payload が範囲外の ID・「移動」）が 0 件。
//       範囲外の章の題が画面に無く、現在の Step は `main/st-3-2` のまま。再読み込みしても同じ
//   (b) 範囲外の開始点を指す `?start=s-4-1` は `start_unknown` で断る。値は画面・コンソールに出ず、保存先は変わらない
//   (c) 対照: CH-4 の開始点を足した IR では、CH-3 の後にボタンがちょうど 1 個出て押すと `main/st-4-1` に入る。`?start=s-4-1` はその節点で開く
//   安全の計数・配信元の外への通信・エンジンの警告と無視が 0 件
//
// 確かめないこと: IR を絞る `compile` の指定と他の章への `next` の拒否（TASK-67）。試験の出力に入力値は出さない。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  NEXT, T, engine, ledger, mouseDriver, journey, install, waitCount,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-out-of-range-chapter/';
const TRIAL = '?fixture=ch0-3-trial';
const FULL = '?fixture=ch0-4-full';
const OUT_ID = 's-4-1';
const OUT_NODE = 'main/st-4-1';
const OUT_TITLE = '範囲外の章の最初の Step';
const NOTICE = '[data-testid="ir-notice"]';
const REFUSED_TEXT = '指定された開始点を開けませんでした。（start_unknown）';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
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

async function scenario(csp, name, query, fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    const L = ledger();
    const consoleTexts = [];
    page.on('console', (m) => consoleTexts.push(m.text()));
    await page.goto(`${srv.url}${query}`);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    await install(page);
    await fn(page, rec, L, { srv, consoleTexts });
    console.log(`${name} (CSP ${csp ? 'あり' : 'なし'}): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  }));
}

async function safety(page, rec, scene, L) {
  const e = await engine(page);
  L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
  L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
  const counts = await collectSafety(page, rec);
  for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
  L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
  L.check(scene, '配信元の外への通信が 0 件', rec.external.length === 0);
  return e;
}

// 範囲外の章へ入る操作が画面に 0 件であること。
async function outOfRangeAbsent(page, scene, L, label) {
  const info = await page.evaluate(([next, id, node, title]) => ({
    next: document.querySelectorAll(next).length,
    payload: [...document.querySelectorAll('[data-payload]')]
      .filter((e) => [id, node].includes(e.getAttribute('data-payload'))).length,
    moves: [...document.querySelectorAll('button')].filter((b) => (b.textContent ?? '').startsWith('移動')).length,
    title: document.body.textContent.includes(title),
  }), [NEXT, OUT_ID, OUT_NODE, OUT_TITLE]);
  L.check(scene, `${label}: 次の章へ進むが 0 件`, info.next === 0);
  L.check(scene, `${label}: 範囲外の ID を持つ要素が 0 件`, info.payload === 0);
  L.check(scene, `${label}: 移動ボタンが 0 件`, info.moves === 0);
  L.check(scene, `${label}: 範囲外の章の題が画面に無い`, info.title === false);
}

const storageDump = (page) => page.evaluate(() => JSON.stringify(
  Object.keys(localStorage).sort().map((k) => [k, localStorage.getItem(k)]),
));

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`絞った IR では CH-3 の完了後に範囲外の章へ入る操作が 0 件で、再読み込みでも同じ（${mode}）`, { timeout: 300000 }, async () => {
    await scenario(csp, 'trial', TRIAL, async (page, rec, L) => {
      const scene = '絞った IR';
      await journey(page, mouseDriver(page), scene, L, null, { chaptered: true });
      // 描き直しが済むのを待つ意味で、少し置いて 2 回確かめる。
      await outOfRangeAbsent(page, scene, L, 'CH-3 の完了直後');
      await page.waitForTimeout(500);
      await outOfRangeAbsent(page, scene, L, 'CH-3 の完了の少し後');
      let e = await safety(page, rec, scene, L);
      const status = e.stateWithoutLog.run.slices.main.status;
      L.check(scene, '現在の Step が main/st-3-2 で達成', e.current === 'main/st-3-2' && status['main/st-3-2'] === 'achieved');
      L.check(scene, '範囲外の節点に入っていない', status[OUT_NODE] === undefined);
      await page.reload();
      await waitMounted(page);
      e = await engine(page);
      L.check(scene, '再読み込み後も main/st-3-2', e.current === 'main/st-3-2');
      await outOfRangeAbsent(page, scene, L, '再読み込み後');
    });
  });

  test(`範囲外の開始点を指す start は断り、値を出さず、保存先を変えない（${mode}）`, { timeout: 300000 }, async () => {
    await scenario(csp, 'refused', `${TRIAL}&start=${OUT_ID}`, async (page, rec, L, { srv, consoleTexts }) => {
      const scene = '範囲外の start';
      const baseline = await storageDump(page);
      L.check(scene, '断りの通知が 1 件で固定の文言', (await page.locator(NOTICE).count()) === 1
        && (await page.locator(NOTICE).innerText()) === REFUSED_TEXT);
      const e = await engine(page);
      L.check(scene, '教材を読まず理由が start_unknown', e.irLoaded === false && e.irRefusal === 'start_unknown');
      const html = await page.evaluate(() => document.documentElement.outerHTML);
      L.check(scene, '値が DOM に出ない', !html.includes(`start=${OUT_ID}`) && !html.includes(OUT_NODE));
      L.check(scene, '値がコンソールに出ない', !consoleTexts.some((t) => t.includes(OUT_ID) || t.includes(OUT_NODE)));
      await page.reload();
      await waitMounted(page);
      L.check(scene, '再読み込みでも断る', (await page.locator(NOTICE).count()) === 1);
      L.check(scene, '保存先が変わらない', (await storageDump(page)) === baseline);
      L.check(scene, '履歴の鍵が 0 件', (await page.evaluate((p) => Object.keys(localStorage).filter((k) => k.startsWith(p)).length, HISTORY_PREFIX)) === 0);
      L.check(scene, '配信元の外への通信が 0 件', rec.external.length === 0);
      await page.goto(`${srv.url}${TRIAL}&start=s-3-1`);
      await waitMounted(page);
      L.check(scene, '範囲内の開始点は開く（陽性の対照）', (await engine(page)).current === 'main/st-3-1');
    });
  });

  test(`対照: 範囲外の開始点を持つ IR では CH-3 の後に 1 個出て押すと入る（${mode}）`, { timeout: 300000 }, async () => {
    await scenario(csp, 'control', FULL, async (page, rec, L, { srv }) => {
      const scene = '対照';
      await journey(page, mouseDriver(page), scene, L, null, { chaptered: true });
      await waitCount(page, NEXT, 1);
      const payloads = await page.evaluate((sel) => [...document.querySelectorAll(sel)].map((b) => b.getAttribute('data-payload')), NEXT);
      L.check(scene, 'ボタンがちょうど 1 個で payload が範囲外の開始点', payloads.length === 1 && payloads[0] === OUT_ID);
      await page.click(NEXT, { timeout: T });
      await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.current === n, OUT_NODE, { timeout: T });
      L.check(scene, '押すと範囲外の節点に入る', (await engine(page)).current === OUT_NODE);
      await safety(page, rec, scene, L);
      await page.goto(`${srv.url}${FULL}&start=${OUT_ID}`);
      await waitMounted(page);
      L.check(scene, '?start= もその節点で開く', (await engine(page)).current === OUT_NODE);
    });
  });
}
