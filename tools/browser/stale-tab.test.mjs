// 古いタブの保存を止め、新しいタブの操作が再読み込みで戻ることの試験（TASK-324 1/2・2/2。SAVE-12。PoC-6 の S-8 相当）。
//
// 同じ context（同じオリジンの localStorage を共有する）で 2 枚のタブを開き、次を確かめる。
//   (1) タブ A で 1 手を送ると保存の鍵が書かれ、A は古いタブではない。
//   (2) タブ B を開いて 1 手を送り、左の成果物の行で確認して st-2 へ進める。保存値が変わり、B は古いタブではない
//       （保存先が書ける陽性の対照）。この値を基準にする。確認まで進めるのは、再読み込みの後の復元を
//       「A のメモリ上の状態」「復元に失敗した初期の状態」と区別できる、Step が変わる操作にするため。
//   (3) A で 1 手を送ると、A のエンジンのログは伸びる（メモリ上は進む。Step は st-1 のまま）が、保存値は基準のままで、
//       A は古いタブになる。
//   (4) B を再読み込みすると、B の操作（st-2）が復元される。A は開いたままである。
//   (5) 再読み込みの後も A の操作は保存されず、A は古いタブのままである。
//   (6) 再読み込みの後の B の操作（選択で st-3 へ）は保存される（陽性の対照）。
//   (7) A を `pagehide` つきで閉じても、保存値は変わらない。
//   (8) B をもう一度再読み込みすると、st-3 が復元される。
// 照合と書き込みの間の競合の窓は対象外（逐次の場面だけを確かめる。docs/known-gaps.md の SAVE-12）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
// 試験の出力・失敗の文に、保存した値・起動番号を出さない（長さと簡易の digest だけを比べる）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-stale-tab/';
const PACK = 'min-3step';
const START = 's-1';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const ACCEPTED = '/make-note first-note';
const ROW = '[data-testid="part-artifact-view"] li[data-row="notes/draft"]';
const CHOICE = '[data-testid="choice-btn"][data-choice="opt-memo"]';

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
const stale = (page) => page.evaluate(() => window.__fandheTest.stale());

// 保存値の長さと簡易の digest（値そのものは返さない）。無ければ null。
const saved = (page, key) => page.evaluate((k) => {
  const v = localStorage.getItem(k);
  if (v === null) return null;
  let h = 0;
  for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
  return { len: v.length, h };
}, key);

// 保存値が prev と変わるまで待つ（保存の遅延が入っても揺れないよう、即時の読み取りにしない）。値は返さない。
async function waitSavedChange(page, key, prev) {
  await page.waitForFunction(([k, p]) => {
    const v = localStorage.getItem(k);
    if (v === null) return false;
    let h = 0;
    for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
    return !p || p.len !== v.length || p.h !== h;
  }, [key, prev]);
  return saved(page, key);
}

const waitCurrent = (page, uid) => page.waitForFunction(
  (u) => JSON.parse(window.__fandheTest.stateJson()).engine.current === u, uid,
);
const idle = (page) => page.waitForFunction(() => window.__fandheTest.nextTick() === 'null');

// 状態の写し（Step の uid・各スライスの JSON）。比べた結果だけを失敗の文に出す。
async function snapshot(page) {
  const e = await engine(page);
  return { current: e.current, slices: JSON.stringify(e.stateWithoutLog.run.slices) };
}

// 再読み込みの後に復元された状態が写しと一致することを確かめる。
async function assertRestored(page, snap, name) {
  const e = await engine(page);
  assert.equal(e.bootKind, 'restored', `${name}: restored`);
  assert.equal(e.current, snap.current, `${name}: current step`);
  assert.equal(JSON.stringify(e.stateWithoutLog.run.slices) === snap.slices, true, `${name}: slices match`);
  assert.equal(e.incrementalMatches, true, `${name}: incremental projection`);
  assert.equal(e.worldMatchesState, true, `${name}: world matches state`);
}

// ログの長さの増加で待つ（2 手目以降の拒否もログを伸ばす状態の変更）。
async function sendStep(page) {
  const before = (await engine(page)).logLen;
  await page.fill(INPUT, ACCEPTED);
  await page.click(SUBMIT);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before);
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null');
}

// 同じ context に 2 枚目のタブを開く。openPage はポップアップを閉じる監視を付けるため、2 枚目を作る前に外す。
async function secondTab(context, url) {
  context.removeAllListeners('page');
  const page = await context.newPage();
  await page.goto(url);
  await waitMounted(page);
  return page;
}

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`古いタブの操作は保存領域に書かれず、新しいタブの操作が再読み込みで戻る（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page: a, context, rec } = await openPage(browser, { origin: srv.origin });
      await a.goto(srv.url);
      await waitMounted(a);
      const key = await keyOf(a);

      // (1) A の 1 手は保存される。
      await sendStep(a);
      assert.notEqual(await saved(a, key), null, 'A saved its first change');
      assert.equal(await stale(a), false);

      // (2) B を開いて 1 手を送り、確認して st-2 へ進める（陽性の対照）。
      const b = await secondTab(context, srv.url);
      const beforeB = await saved(b, key);
      await sendStep(b);
      await b.click(ROW);
      await waitCurrent(b, 'main/st-2');
      const dB = await waitSavedChange(b, key, beforeB);
      assert.notDeepEqual(dB, beforeB, 'B wrote to the store');
      assert.equal(await stale(b), false);
      await idle(b);
      const bSnap = await snapshot(b);

      // (3) A の操作はメモリ上は進むが、保存値は B のままで、A は古いタブになる。
      const logA = (await engine(a)).logLen;
      await sendStep(a);
      const eA = await engine(a);
      assert.equal(eA.logLen > logA, true, 'A advanced in memory');
      assert.equal(eA.current !== bSnap.current, true, 'A is on a different step from B (discriminates the restore)');
      assert.deepEqual(await saved(b, key), dB, 'A did not overwrite the newer tab');
      assert.equal(await stale(a), true);
      assert.equal(await stale(b), false);

      // (4) B を再読み込みすると B の操作が戻る。
      await b.reload();
      await waitMounted(b);
      await assertRestored(b, bSnap, 'B after reload');
      assert.equal(await stale(b), false, 'B stays the newer tab after reload');
      await idle(b);
      const dR = await saved(b, key);
      assert.notEqual(dR, null);

      // (5) 再読み込みの後も A は書けない。
      await sendStep(a);
      assert.deepEqual(await saved(b, key), dR, 'A still did not overwrite after the reload');
      assert.equal(await stale(a), true);
      assert.equal(await stale(b), false);

      // (6) 再読み込みの後の B の操作は保存される（陽性の対照）。
      await b.click(CHOICE);
      await waitCurrent(b, 'main/st-3');
      const dC = await waitSavedChange(b, key, dR);
      assert.notDeepEqual(dC, dR, 'B wrote to the store after the reload');
      assert.equal(await stale(b), false);
      await idle(b);
      const cSnap = await snapshot(b);

      // (7) A を pagehide つきで閉じても、保存値は変わらない。
      await a.close({ runBeforeUnload: true });
      assert.deepEqual(await saved(b, key), dC, 'pagehide of the stale tab did not write');

      // (8) B をもう一度再読み込みしても、再読み込みの後の操作が戻る。
      await b.reload();
      await waitMounted(b);
      await assertRestored(b, cSnap, 'B after second reload');
      clean(rec);
      await context.close();
    }));
  });
}
