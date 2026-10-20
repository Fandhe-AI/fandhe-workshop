// 保存領域が失われたときの案内の試験（TASK-340。SAVE-29・D-57）。
//
// 試験の設定（TEST_HOOKS=1）の画面（教材 `min-3step`、開始点 `s-1`）で、次を確かめる。
//   (A) 保存領域を消した（CDP の `Storage.clearDataForOrigin`。ブラウザの自動削除（eviction）も、保存済みの履歴が消える
//       点では同じ状態になるため、同じ手順で模擬する。本物の eviction は起こせない）: 再び開くと `fresh`・世代 1・
//       `reloaded` 0 件・最初の Step・鍵なしで始まり、案内は操作の前は空、最初の操作で初回の案内が再び出る。
//   (C) 保存できない状態（シークレットに近い状態。`localStorage` の取得が例外・`indexedDB` なし）: `fresh` で始まり、
//       最初の操作で初回の案内が出て、再読み込みしても `fresh`・世代 1 のまま。
//   (D) 章の終わり: `load-fixture:approval` の 3 Step を最後まで進めると、案内が章の終わりの文になる。
//   (E) 対照: 保存してから消さずに再読み込みする（`restored`）と、その後に操作しても案内は空のまま。
// 文面・回数は仮置き（D-57。ユーザーが決める）のため、文そのものでなく「空か・種類で違うか」を確かめる。
// 試験の出力・失敗の文に、保存した値を出さない（履歴は受講者のデータのため。有無だけを見る）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-storage-lost/';
const PACK = 'min-3step';
const START = 's-1';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const NOTICE = '[data-testid="export-notice"]';
const RAW = '[data-testid="raw-trigger"]';
const BTN = '[data-testid="approval-btn"]';
const ACCEPTED = '/make-note first-note';
const T = 30000;

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const keyOf = (page) => page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, START]);
const hasKey = (page, key) => page.evaluate((k) => localStorage.getItem(k) !== null, key);
const noticeText = (page) => page.$eval(NOTICE, (el) => el.textContent ?? '');

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

async function submitOne(page) {
  await page.fill(INPUT, ACCEPTED);
  await page.click(SUBMIT);
  await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT);
}

async function sendRaw(page, payload) {
  await page.evaluate(([sel, p]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', p);
    el.click();
  }, [RAW, payload]);
}

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

async function assertFresh(page, label) {
  const eng = await engine(page);
  assert.equal(eng.bootKind, 'fresh', label);
  assert.equal(eng.generation, 1, label);
  assert.equal(eng.log.filter((r) => r.kind === 'reloaded').length, 0, label);
  assert.equal(eng.current, 'main/st-1', label);
}

// 1 手を送信して保存を済ませたページを閉じ（離れる直前の書き出しも済ませ）、空のページから保存領域を消して開き直す。
async function loseStorage(srv, context, page) {
  const key = await keyOf(page);
  await page.waitForFunction((k) => localStorage.getItem(k) !== null, key, { timeout: T });
  // 離れる直前の書き出しを済ませるため、同じ origin のエンジンのないページへ移ってから消す（新しいページはハーネスが閉じる）。
  await page.goto(`${srv.url}app.css`);
  const cdp = await context.newCDPSession(page);
  await cdp.send('Storage.clearDataForOrigin', { origin: srv.origin, storageTypes: 'all' });
  await cdp.detach();
  const again = page;
  await again.goto(srv.url);
  await waitMounted(again);
  return { again, key };
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  test(`(A) 保存領域を消すと新しい状態で始まり、最初の操作で案内が再び出る（CSP ${label}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      assert.equal(await noticeText(page), '', 'no hint before the first operation');
      await submitOne(page);
      const first = await noticeText(page);
      assert.notEqual(first, '', 'first-save hint appears');
      const { again, key } = await loseStorage(srv, context, page);
      assert.equal(await hasKey(again, key), false, 'storage is gone');
      await assertFresh(again, 'after losing storage');
      assert.equal(await noticeText(again), '', 'no hint before the first operation');
      await submitOne(again);
      assert.equal(await noticeText(again), first, 'first-save hint appears again');
      clean(rec);
      await context.close();
    });
  });

  test(`(C) 保存できない状態でも新しい状態で始まり、最初の操作で案内が出る（CSP ${label}）`, async () => {
    await run(async (srv) => {
      const opened = await openPage(browser, { origin: srv.origin });
      const { page, context, rec } = opened;
      await context.addInitScript(() => {
        Object.defineProperty(window, 'localStorage', { configurable: true, get() { throw new DOMException('denied', 'SecurityError'); } });
        Object.defineProperty(window, 'indexedDB', { configurable: true, get() { return undefined; } });
      });
      await page.goto(srv.url);
      await waitMounted(page);
      await assertFresh(page, 'unavailable');
      assert.equal(await noticeText(page), '');
      await submitOne(page);
      const text = await noticeText(page);
      assert.notEqual(text, '', 'first-save hint appears even when saving is impossible');
      await page.reload();
      await waitMounted(page);
      await assertFresh(page, 'unavailable, after reload');
      assert.equal(await noticeText(page), '');
      clean(rec);
      await context.close();
    });
  });

  test(`(D) 章の最後の Step を達成すると章の終わりの案内になる（CSP ${label}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await sendRaw(page, 'load-fixture:approval');
      await page.waitForSelector(BTN, { timeout: T });
      assert.equal(await noticeText(page), '', 'loading the fixture shows no hint');
      const yes = () => page.getByRole('button', { name: 'はい', exact: true });
      const at = (uid) => page.waitForFunction((u) => JSON.parse(window.__fandheTest.stateJson()).engine.current === u, uid, { timeout: T });
      await page.getByRole('button', { name: '直す（対象: t-alpha）' }).click();
      await page.waitForFunction((b) => document.querySelectorAll(b).length === 3, BTN, { timeout: T });
      await yes().click();
      await at('main/st-2');
      const mid = await noticeText(page);
      assert.equal(mid, '', 'no hint mid-chapter (the fixture replaces the boot state, so no first-save hint)');
      await page.click('[data-testid="choice-btn"][data-choice="opt-memo"]', { timeout: T });
      await at('main/st-3');
      await page.waitForSelector(BTN, { timeout: T });
      await yes().click();
      await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.stateWithoutLog
        .run.slices.main.status['main/st-3'] === 'accepted', null, { timeout: T });
      await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.runStatus === 'idle', null, { timeout: T });
      await yes().click();
      await page.waitForFunction(([sel, before]) => {
        const t = document.querySelector(sel).textContent ?? '';
        return t !== '' && t !== before;
      }, [NOTICE, mid], { timeout: T });
      const end = await noticeText(page);
      assert.notEqual(end, '', 'chapter-end hint appears');
      clean(rec);
      await context.close();
    });
  });

  test(`(E) 対照: 消さずに再読み込みすると復元され、操作しても案内は出ない（CSP ${label}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await submitOne(page);
      await page.reload();
      await waitMounted(page);
      assert.equal((await engine(page)).bootKind, 'restored');
      assert.equal(await noticeText(page), '');
      const len = (await engine(page)).logLen;
      await page.fill(INPUT, '/make-note second-note');
      await page.click(SUBMIT);
      await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, len, { timeout: T });
      assert.equal(await noticeText(page), '', 'no first-save hint when the history was restored');
      clean(rec);
      await context.close();
    });
  });
}
