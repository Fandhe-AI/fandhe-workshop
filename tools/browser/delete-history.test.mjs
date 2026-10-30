// 保存済みの履歴の削除の試験（TASK-341 1/2。SAVE-32・SHELL-38）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、試験の口 `deleteHistory`・`confirmDelete`・`cancelDelete`（固定の合図を raw-trigger へ送るだけ）を使い、
// 削除の中核（保存先の鍵を消す・ホストの状態を新しくする）を確かめる。可視のボタン・確認の表示は TASK-539、「やめる」の不変と通信 0 件の計数は 2/2（delete-history-scenes.test.mjs。補助関数は delete-history.mjs）。
//
// A. localStorage・章をまたぐ: 既定の教材と、同じ `pack.id`（chapter-links）の章 A・章 B を保存し、別の配信先・unreadable・履歴でない鍵を置く。
//    章 B で削除を確定すると、chapter-links の鍵が 0 件（章 A の鍵も消える = 教材単位）、対象以外の鍵と値は不変、章 B は最初の Step に戻る。
//    再読み込みしても `fresh` で最初の Step、鍵は 0 件のまま。続けて章 A を開いても `fresh`。
// B. IndexedDB: 画面を開いた後、この教材の別の開始点の鍵と別の教材の鍵を IndexedDB へ直接置き、再読み込みして写しに読み込ませる。
//    削除を確定すると（delete の確定は非同期のため待って）この教材の鍵が IndexedDB から 0 件になり、別の教材の値は不変。再読み込みしても 0 件で `fresh`。
// C. 確認待ちの関門: 要求を送らずに確定だけを送っても、鍵の数と値が不変で結果は null。要求の後に「やめる」を送ってから確定しても同じ。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
// 試験の出力・失敗の文に、保存した値・鍵の文字列を出さない（有無・本数・比較の結果だけを出す）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import {
  localEntries, press, waitDeleteResult, idbDump, idbPut, sameScopeKey,
} from './delete-history.mjs';

const BASE = '/fandhe-delete-history/';
const PACK = 'chapter-links';
const START_A = 's-1';
const START_B = 's-4';
const DEFAULT_PACK = 'min-3step';
const DEFAULT_START = 's-1';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const CHOICE_A = '[data-testid="choice-btn"][data-choice="opt-a"]';
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
const keyOf = (page, pack, start) => page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [pack, start]);
const historyKeys = async (page) => Object.keys(await localEntries(page)).filter((k) => k.startsWith(HISTORY_PREFIX)).sort();
const packKeys = async (page) => (await historyKeys(page)).filter((k) => k.includes(`/${PACK}/`) && !k.includes('/zz-other-scope/'));

async function go(page, srv, query) {
  await page.goto(`${srv.url}${query}`);
  await waitMounted(page);
}

const waitSaved = (page, key) => page.waitForFunction((k) => localStorage.getItem(k) !== null, key, { timeout: T });

async function moveA(page) {
  const before = (await engine(page)).current;
  await page.fill(INPUT, '/make-note note-1');
  await page.click(SUBMIT);
  await page.waitForFunction((cur) => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    const st = e.stateWithoutLog.run.slices.main?.status ?? {};
    return st[cur] === 'accepted' && window.__fandheTest.nextTick() === 'null';
  }, before, { timeout: T });
}

async function moveB(page) {
  const before = (await engine(page)).current;
  await page.click(CHOICE_A);
  await page.waitForFunction((cur) => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    return e.current !== cur && window.__fandheTest.nextTick() === 'null';
  }, before, { timeout: T });
}

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify({ csp: s.csp, pageErrors: s.pageErrors, consoleErrors: s.consoleErrors });
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`localStorage の教材の履歴だけが消え、最初の Step から新しく始まる（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });

      // 既定の教材を 1 手進めて保存する。
      await go(page, srv, '');
      const keyD = await keyOf(page, DEFAULT_PACK, DEFAULT_START);
      await page.fill(INPUT, '/make-note first-note');
      await page.click(SUBMIT);
      await waitSaved(page, keyD);

      // 章 A・章 B を 1 手ずつ進める。
      await go(page, srv, '?fixture=chapter-a');
      const keyA = await keyOf(page, PACK, START_A);
      const keyB = await keyOf(page, PACK, START_B);
      await moveA(page);
      await waitSaved(page, keyA);
      await go(page, srv, '?fixture=chapter-b');
      await moveB(page);
      await waitSaved(page, keyB);
      assert.deepEqual(await packKeys(page), [keyA, keyB].sort(), 'precondition: two chapter keys exist');

      // 対象外の鍵（別の配信先・unreadable・履歴でない鍵）を置く。
      const segs = keyA.split('/');
      const packIdx = segs.indexOf(PACK);
      assert.ok(packIdx > 0, 'precondition: key shape');
      const foreignScope = sameScopeKey(keyA, (p) => { p[packIdx - 1] = 'zz-other-scope'; });
      const unreadable = sameScopeKey(keyA, (p) => { p[packIdx] = 'unreadable'; p[packIdx + 1] = 'main'; });
      await page.evaluate(([a, b]) => {
        localStorage.setItem(a, 'foreign-scope-value');
        localStorage.setItem(b, 'unreadable-value');
        localStorage.setItem('unrelated-key', 'u');
      }, [foreignScope, unreadable]);
      const beforeAll = await localEntries(page);
      const others = Object.keys(beforeAll).filter((k) => k !== keyA && k !== keyB);
      assert.ok(others.includes(keyD) && others.includes(foreignScope) && others.includes(unreadable), 'precondition: other keys exist');
      const othersBefore = Object.fromEntries(others.map((k) => [k, beforeAll[k]]));

      // 章 B で削除を確定する。
      const first = (await engine(page)).current;
      assert.notEqual(first, 'main/st-4', 'precondition: chapter B moved');
      assert.equal(await press(page, 'deleteHistory'), true);
      assert.equal((await engine(page)).deletePending, true);
      await press(page, 'confirmDelete');
      await waitDeleteResult(page);
      const after1 = await engine(page);
      assert.deepEqual(after1.lastDelete, { outcome: 'deleted', removed: 2 });
      assert.equal(after1.deletePending, false);
      assert.equal(after1.current, 'main/st-4', 'restarted at the first step of chapter B');

      assert.deepEqual(await packKeys(page), [], 'no key of this pack remains (chapter A included)');
      const afterAll = await localEntries(page);
      assert.deepEqual(Object.fromEntries(others.map((k) => [k, afterAll[k]])), othersBefore, 'other keys are unchanged');
      assert.equal(Object.keys(afterAll).length, others.length, 'no new key was written');

      // 再読み込みしても新しく始まり、鍵は 0 件のまま。
      await page.reload();
      await waitMounted(page);
      const re = await engine(page);
      assert.equal(re.bootKind, 'fresh');
      assert.equal(re.current, 'main/st-4');
      assert.equal(re.generation, 1);
      assert.equal(re.log.filter((r) => r.kind === 'reloaded').length, 0);
      assert.deepEqual(await packKeys(page), [], 'still no key after reload');
      await go(page, srv, '?fixture=chapter-a');
      const ra = await engine(page);
      assert.equal(ra.bootKind, 'fresh', 'chapter A also starts fresh');
      assert.equal(ra.current, 'main/st-1');

      clean(rec);
      await context.close();
    }));
  });

  test(`IndexedDB の教材の鍵も消え、別の教材は変わらない（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await go(page, srv, '?fixture=chapter-b');
      const keyB = await keyOf(page, PACK, START_B);
      const segs = keyB.split('/');
      const packIdx = segs.indexOf(PACK);
      const thisPack = sameScopeKey(keyB, (p) => { p[packIdx + 1] = 's-9'; });
      const otherPack = sameScopeKey(keyB, (p) => { p[packIdx] = 'other-pack'; });
      await idbPut(page, { [thisPack]: 'idb-target-value', [otherPack]: 'idb-other-value' });
      await page.reload();
      await waitMounted(page);
      assert.equal((await engine(page)).bootKind, 'fresh');
      const dumpBefore = await idbDump(page);
      assert.equal(Object.keys(dumpBefore).length, 2, 'precondition: both keys are in IndexedDB');

      await press(page, 'deleteHistory');
      await press(page, 'confirmDelete');
      await waitDeleteResult(page);
      assert.deepEqual((await engine(page)).lastDelete, { outcome: 'deleted', removed: 1 });
      // IndexedDB の delete の確定は非同期。
      const deadline = Date.now() + T;
      let dump = await idbDump(page);
      while (Object.keys(dump).some((k) => k.includes(`/${PACK}/`)) && Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, 100));
        dump = await idbDump(page);
      }
      assert.deepEqual(Object.keys(dump).filter((k) => k.includes(`/${PACK}/`)), [], 'no key of this pack in IndexedDB');
      assert.equal(dump[otherPack], 'idb-other-value', 'the other pack value is unchanged');
      assert.equal(Object.keys(dump).length, 1);

      await page.reload();
      await waitMounted(page);
      assert.equal((await engine(page)).bootKind, 'fresh');
      const dumpAfter = await idbDump(page);
      assert.deepEqual(Object.keys(dumpAfter).filter((k) => k.includes(`/${PACK}/`)), []);

      clean(rec);
      await context.close();
    }));
  });

  test(`確認待ちの前の確定ややめるの後の確定は何も消さない（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await go(page, srv, '?fixture=chapter-a');
      const keyA = await keyOf(page, PACK, START_A);
      await moveA(page);
      await waitSaved(page, keyA);
      const before = await localEntries(page);
      const cur = (await engine(page)).current;

      await press(page, 'confirmDelete');
      let e = await engine(page);
      assert.equal(e.lastDelete, null, 'confirm without a request does nothing');
      assert.equal(e.current, cur);
      assert.deepEqual(await localEntries(page), before);

      await press(page, 'deleteHistory');
      await press(page, 'cancelDelete');
      assert.equal((await engine(page)).deletePending, false);
      await press(page, 'confirmDelete');
      e = await engine(page);
      assert.equal(e.lastDelete, null, 'confirm after cancel does nothing');
      assert.equal(e.current, cur);
      assert.deepEqual(await localEntries(page), before);

      clean(rec);
      await context.close();
    }));
  });
}
