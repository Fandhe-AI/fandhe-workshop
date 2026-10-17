// 保存済みの履歴が無い場合と、未操作の場合に、新しい状態で始まり世代が上がらないことの試験（TASK-319 1/3。SAVE-3・SAVE-28）。
//
// 試験の設定（TEST_HOOKS=1）の画面には、試験データの教材（pack `min-3step`、開始点 `s-1`）が入っている。実際の画面で次を確かめる。
//   (1) 保存済みの履歴が無いとき: マウントの直後に保存先の鍵が無く、世代は 1、`reloaded` の行は 0 件。起動の種類は `fresh`。
//   (2) 未操作の再読み込み: 何もせずに再読み込みしても、鍵は書かれないまま、世代は 1 のまま、`reloaded` の行は 0 件。
//       状態（ログの長さ・Step・状態表示・画面のログの行数）はマウント直後と等しい。2 回続けても同じ（PoC-6 の F-9）。
//   (3) 陽性の対照（1 地点だけ）: 実際の画面で 1 手を送信してから再読み込みすると、鍵が書かれ、起動の種類が `restored` になり、
//       世代が 1 つ上がって `reloaded` の行がちょうど 1 件になる。(2) が「何も保存されないから新しい」だけで通っていないことを示す。
//   (4) 対照の確認: 未操作で再読み込みした後に操作すれば、その操作は保存される（鍵が書かれる）。
// 復元の一致の網羅（地点ごとの比較）は TASK-319 の後続の試験が持つ。ここでは分岐だけを確かめる。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
// 試験の出力・失敗の文に、保存した値を出さない（履歴は受講者のデータのため。有無と長さだけを比べる）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-reload-fresh/';
const PACK = 'min-3step';
const START = 's-1';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const LOG_ITEMS = '[data-testid="part-log-list"] li';
const ACCEPTED = '/make-note first-note';

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;

// 保存先の鍵の文字列（教材 ID と最初の開始点から、配信先の区間つきで求めた値）。
const keyOf = (page) => page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, START]);

// 保存された値の長さ。無ければ null。
const savedLen = (page, key) => page.evaluate((k) => {
  const v = localStorage.getItem(k);
  return v === null ? null : v.length;
}, key);

const logRows = (page) => page.$$eval(LOG_ITEMS, (lis) => lis.length);
const reloadedRows = (eng) => eng.log.filter((r) => r.kind === 'reloaded').length;

// 世代・ログ・Step・状態表示・画面のログの行数。再読み込みの前後で比べる。
async function observe(page) {
  const eng = await engine(page);
  return {
    bootKind: eng.bootKind,
    generation: eng.generation,
    logLen: eng.logLen,
    reloaded: reloadedRows(eng),
    current: eng.current,
    runStatus: eng.runStatus,
    stateWithoutLog: JSON.stringify(eng.stateWithoutLog),
    rows: await logRows(page),
  };
}

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

const sameExceptBoot = ({ bootKind, ...rest }) => rest;

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`保存済みの履歴が無いとき、新しく始まり世代は上がらない（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      const key = await keyOf(page);
      assert.equal(await savedLen(page, key), null, 'no saved history');
      const first = await observe(page);
      assert.equal(first.bootKind, 'fresh');
      assert.equal(first.generation, 1);
      assert.equal(first.reloaded, 0);
      clean(rec);
      await context.close();
    }));
  });

  test(`未操作で再読み込みしても、新しく始まるだけで世代は上がらず何も保存されない（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      const key = await keyOf(page);
      const first = await observe(page);
      for (let i = 0; i < 2; i += 1) {
        await page.reload();
        await waitMounted(page);
        const again = await observe(page);
        assert.equal(await savedLen(page, key), null, `reload ${i + 1}: nothing is saved`);
        assert.equal(again.bootKind, 'fresh', `reload ${i + 1}`);
        assert.equal(again.generation, 1, `reload ${i + 1}: generation is unchanged`);
        assert.equal(again.reloaded, 0, `reload ${i + 1}: no reloaded row`);
        assert.deepEqual(sameExceptBoot(again), sameExceptBoot(first), `reload ${i + 1}: state equals the first mount`);
      }
      clean(rec);
      await context.close();
    }));
  });

  test(`対照: 1 手を操作してから再読み込みすると、復元されて世代が 1 上がる（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      const key = await keyOf(page);
      assert.equal(await savedLen(page, key), null);
      await page.fill(INPUT, ACCEPTED);
      await page.click(SUBMIT);
      await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT);
      await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null');
      const before = await observe(page);
      assert.equal(before.generation >= 1, true);
      // 離れる直前（pagehide）の書き出しを経て、鍵が書かれる。
      await page.reload();
      await waitMounted(page);
      assert.notEqual(await savedLen(page, key), null, 'history is saved on leave');
      const after = await observe(page);
      assert.equal(after.bootKind, 'restored');
      assert.equal(after.generation, before.generation + 1, 'reload bumps the generation by one');
      assert.equal(after.reloaded, 1, 'exactly one reloaded row');
      assert.equal(after.logLen, before.logLen + 1);
      clean(rec);
      await context.close();
    }));
  });

  test(`未操作の再読み込みの後でも、操作すれば保存される（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      const key = await keyOf(page);
      await page.reload();
      await waitMounted(page);
      assert.equal(await savedLen(page, key), null);
      await page.fill(INPUT, ACCEPTED);
      await page.click(SUBMIT);
      await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT);
      await page.reload();
      await waitMounted(page);
      assert.notEqual(await savedLen(page, key), null, 'an operation after a fresh reload is saved');
      assert.equal((await observe(page)).bootKind, 'restored');
      clean(rec);
      await context.close();
    }));
  });
}
