// 履歴の削除で「やめる」を選んだときの不変と、削除の操作の通信 0 件の試験（TASK-341 2/2。SAVE-32・SHELL-38・SAFE-16）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、試験の口 `deleteHistory`・`confirmDelete`・`cancelDelete` を使い、3 つの時点
// （受講の途中・章の終わり・復習中）のそれぞれで次を確かめる。
//   - 「やめる」: 確認待ちの間も、やめた後も、localStorage と IndexedDB の鍵と値・エンジンの状態・「次の章へ進む」・復習の一覧が変わらない。
//     やめた後に確定だけを送っても何も消えない。
//   - 「削除する」: この教材の鍵だけが両方の保存先から消え、別の教材・別の配信先・読めない鍵・履歴でない鍵は件数も値も変わらない。最初の Step から新しく始まる。
//   - 通信: 上の 2 つの操作の区間で、通信の API の呼び出しが 0 件（egress.mjs の計数）。場面の全体で配信元以外への通信・GET 以外・許可外のパスも 0 件。
//     記録器が付いていることは、起動時の wasm・IR の取得が数えられている（allowedFetch > 0）ことで示す。記録器自体の検出力は egress-count の場面 C が持つ。
//
// 「復習中」の代役（自動運転での判断。人の承認は無い）: 復習の画面（REVIEW-10）は未実装のため、20 Step をすべて達成し、復習の一覧に項目があり、
// 「次の章へ進む」が 0 件の状態を「復習中」とみなす。REVIEW-10 の実画面ができたら確かめ直す（docs/known-gaps.md）。
//
// 範囲外: 配信する設定には削除の action を復号する経路が無い（可視のボタン・確認の表示は TASK-539）。このため本試験は試験の設定だけで行う。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
// 試験の出力・失敗の文に、保存した値・鍵の文字列を出さない（有無・本数・比較の結果・区分の件数だけを出す）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize, collectSafety, nonZero,
} from './harness.mjs';
import { attachEgress, egressCounts, egressFailures, egressLine } from './egress.mjs';
import {
  ledger, journey, install, mouseDriver, engine, NEXT,
} from './ch0-3-journey.mjs';
import {
  localEntries, press, waitDeleteResult, idbDump, idbPut, sameScopeKey,
} from './delete-history.mjs';

const BASE = '/fandhe-delete-scenes/';
const PACK = 'ch0-3-chapters';
const START = 's-0-1';
const DEFAULT_PACK = 'min-3step';
const DEFAULT_START = 's-1';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const REVIEW_LI = '[data-testid="part-review-list"] > li';
const FIRST = 'main/st-0-1';
const T = 30000;
const SETTLE_MS = 1500;
const STOP = new Error('scene-stop');

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const isPackKey = (k) => k.startsWith('fandhe-workshop/history/') && k.includes(`/${PACK}/`) && !k.includes('/zz-other-scope/');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// 観察する値の束。鍵・値そのものは比較にだけ使い、出力には出さない。
async function observe(page, eg) {
  const e = await engine(page);
  return {
    local: await localEntries(page),
    idb: await idbDump(page),
    current: e.current,
    state: JSON.stringify(e.stateWithoutLog),
    logLen: e.log.length,
    next: await page.locator(NEXT).count(),
    review: await page.locator(REVIEW_LI).count(),
    achieved: await page.locator(`${REVIEW_LI}[data-achieved="true"]`).count(),
    calls: eg.calls,
    allowedFetch: eg.allowedFetch,
  };
}

function sameBoth(a, b, what) {
  assert.deepEqual(b.local, a.local, `${what}: localStorage が変わった`);
  assert.deepEqual(b.idb, a.idb, `${what}: IndexedDB が変わった`);
}

// 「やめる」を選ぶ。
async function cancelCheck(page, eg) {
  const a = await observe(page, eg);
  assert.ok(Object.keys(a.idb).some(isPackKey), '前提: この教材の鍵が IndexedDB にある');
  assert.equal(await press(page, 'deleteHistory'), true, '削除の要求が受け付けられる');
  assert.equal((await engine(page)).deletePending, true);
  sameBoth(a, await observe(page, eg), '確認待ちの間');
  await press(page, 'cancelDelete');
  const mid = await engine(page);
  assert.equal(mid.deletePending, false);
  assert.equal(mid.lastDelete, null);
  await sleep(SETTLE_MS);
  const b = await observe(page, eg);
  sameBoth(a, b, 'やめるの後');
  for (const k of ['current', 'state', 'logLen', 'next', 'review', 'achieved']) assert.equal(b[k], a[k], `やめるの後: ${k} が変わった`);
  // やめた後の確定は何も消さない。
  await press(page, 'confirmDelete');
  await sleep(SETTLE_MS);
  const c = await observe(page, eg);
  assert.equal((await engine(page)).lastDelete, null, 'やめるの後の確定は結果を残さない');
  sameBoth(a, c, 'やめるの後の確定');
  assert.equal(c.state, a.state);
  assert.equal(c.calls, a.calls, '「やめる」の区間で通信の API の呼び出しが 0 件');
  assert.equal(c.allowedFetch, a.allowedFetch);
}

// 「削除する」を選ぶ。通信の差は削除の区間だけで取り、その後の再読み込みは差に入れない。
async function deleteCheck(page, eg) {
  const a = await observe(page, eg);
  assert.ok(Object.keys(a.local).some(isPackKey), '前提: この教材の鍵が localStorage にある');
  assert.ok(Object.keys(a.idb).some(isPackKey), '前提: この教材の鍵が IndexedDB にある');
  await press(page, 'deleteHistory');
  await press(page, 'confirmDelete');
  await waitDeleteResult(page);
  const res = (await engine(page)).lastDelete;
  assert.equal(res.outcome, 'deleted');
  assert.ok(res.removed >= 1);
  // IndexedDB の delete の確定は非同期。
  const deadline = Date.now() + T;
  let b = await observe(page, eg);
  while (Object.keys(b.idb).some(isPackKey) && Date.now() < deadline) {
    await sleep(100);
    b = await observe(page, eg);
  }
  await sleep(SETTLE_MS);
  b = await observe(page, eg);
  assert.deepEqual(Object.keys(b.local).filter(isPackKey), [], 'この教材の鍵が localStorage に残らない');
  assert.deepEqual(Object.keys(b.idb).filter(isPackKey), [], 'この教材の鍵が IndexedDB に残らない');
  const keep = (m) => Object.fromEntries(Object.entries(m).filter(([k]) => !isPackKey(k)));
  assert.deepEqual(b.local, keep(a.local), '対象外の localStorage の鍵と値が変わらず、新しい鍵も増えない');
  assert.deepEqual(keep(b.idb), keep(a.idb), '対象外の IndexedDB の鍵と値が変わらない');
  assert.equal(b.current, FIRST, '最初の Step から新しく始まる');
  assert.equal(b.next, 0);
  assert.equal(b.achieved, 0);
  assert.equal(b.calls, a.calls, '「削除する」の区間で通信の API の呼び出しが 0 件');
  assert.equal(b.allowedFetch, a.allowedFetch);
  // 再読み込みしても新しく始まり、鍵は 0 件のまま。
  await page.reload();
  await waitMounted(page);
  const re = await engine(page);
  assert.equal(re.bootKind, 'fresh');
  assert.equal(re.current, FIRST);
  assert.deepEqual(Object.keys(await localEntries(page)).filter(isPackKey), []);
  assert.deepEqual(Object.keys(await idbDump(page)).filter(isPackKey), []);
}

const waitSaved = (page, key) => page.waitForFunction(
  ([k]) => localStorage.getItem(k) !== null && window.__fandheTest.nextTick() === 'null',
  [key],
  { timeout: T },
);

// 共通の準備。別の教材・別の配信先・読めない鍵・履歴でない鍵を同じオリジンに置き、対象の教材を開く。
async function prepare(page, srv) {
  await page.goto(srv.url);
  await waitMounted(page);
  const keyD = await page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [DEFAULT_PACK, DEFAULT_START]);
  await page.fill(INPUT, '/make-note first-note');
  await page.click(SUBMIT);
  await page.waitForFunction((k) => localStorage.getItem(k) !== null, keyD, { timeout: T });
  await page.goto(`${srv.url}?fixture=${PACK}`);
  await waitMounted(page);
  const keyP = await page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, START]);
  const packIdx = keyP.split('/').indexOf(PACK);
  assert.ok(packIdx > 0, '前提: 鍵の形');
  const foreign = sameScopeKey(keyP, (p) => { p[packIdx - 1] = 'zz-other-scope'; });
  const unreadable = sameScopeKey(keyP, (p) => { p[packIdx] = 'unreadable'; });
  const otherPack = sameScopeKey(keyP, (p) => { p[packIdx] = 'other-pack'; });
  await page.evaluate(([a, b]) => {
    localStorage.setItem(a, 'foreign-scope-value');
    localStorage.setItem(b, 'unreadable-value');
    localStorage.setItem('unrelated-key', 'u');
  }, [foreign, unreadable]);
  // 対象の教材の別の開始点の鍵を IndexedDB に置く（削除が IndexedDB の対象鍵にも作用することを見るため）。
  const idbTarget = sameScopeKey(keyP, (p) => { p[p.length - 1] = 'idb-other-start'; });
  assert.ok(isPackKey(idbTarget), '前提: IndexedDB の対象鍵はこの教材の鍵');
  await idbPut(page, { [otherPack]: 'idb-other-value', [idbTarget]: 'idb-target-value' });
  await page.reload();
  await waitMounted(page);
  assert.ok(Object.keys(await idbDump(page)).some(isPackKey), '前提: この教材の鍵が IndexedDB にある');
  await install(page);
  return keyP;
}

function finish(label, csp, page, eg, rec) {
  return async () => {
    const counts = egressCounts(eg, rec);
    assert.deepEqual(egressFailures(counts), [], `${label}: 通信の区分が 0 件でない`);
    assert.ok(eg.allowedFetch > 0, `${label}: 記録器が付いている（起動時の取得が数えられている）`);
    const safety = await collectSafety(page, rec);
    assert.deepEqual(nonZero(safety), [], `${label}: 安全性の計数が全区分 0`);
    const s = summarize(rec);
    const detail = JSON.stringify({ csp: s.csp, pageErrors: s.pageErrors, consoleErrors: s.consoleErrors });
    assert.equal(s.csp, 0, detail);
    assert.equal(s.pageErrors, 0, detail);
    assert.equal(s.consoleErrors, 0, detail);
    console.log(egressLine(label, csp, counts, eg));
  };
}

async function scene(csp, label, run) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    const eg = await attachEgress(context, { origin: srv.origin, base: BASE });
    const keyP = await prepare(page, srv);
    await run({ page, eg, keyP });
    await finish(label, csp, page, eg, rec)();
    await context.close();
  }));
}

for (const csp of [true, false]) {
  const mode = csp ? 'CSP あり' : 'CSP なし';

  test(`受講の途中で、やめるは何も変えず、削除するはこの教材だけを消し、通信が 0 件（${mode}）`, { timeout: 600000 }, async () => {
    await scene(csp, 'delete 途中', async ({ page, eg, keyP }) => {
      await journey(page, mouseDriver(page), 'delete 途中', ledger(), 'main/st-1-3', { chaptered: true });
      await waitSaved(page, keyP);
      await sleep(SETTLE_MS);
      await cancelCheck(page, eg);
      await deleteCheck(page, eg);
    });
  });

  test(`章の終わりで、やめるは何も変えず通しが続き、削除するはこの教材だけを消し、通信が 0 件（${mode}）`, { timeout: 600000 }, async () => {
    await scene(csp, 'delete 章の終わり', async ({ page, eg, keyP }) => {
      const base = mouseDriver(page);
      let crossed = 0;
      const driver = {
        ...base,
        async nextChapter() {
          crossed += 1;
          if (crossed <= 2) {
            await waitSaved(page, keyP);
            await sleep(SETTLE_MS);
            assert.equal(await page.locator(NEXT).count(), 1, '章の終わりでは次の章へ進むが 1 個');
          }
          if (crossed === 1) {
            await cancelCheck(page, eg);
          } else if (crossed === 2) {
            await deleteCheck(page, eg);
            throw STOP;
          }
          return base.nextChapter();
        },
      };
      await assert.rejects(
        journey(page, driver, 'delete 章の終わり', ledger(), null, { chaptered: true }),
        (err) => err === STOP,
      );
      assert.equal(crossed, 2);
    });
  });

  test(`復習中（代役）で、やめるは何も変えず、削除するはこの教材だけを消し、通信が 0 件（${mode}）`, { timeout: 600000 }, async () => {
    await scene(csp, 'delete 復習中', async ({ page, eg, keyP }) => {
      await journey(page, mouseDriver(page), 'delete 復習中', ledger(), null, { chaptered: true });
      await waitSaved(page, keyP);
      await sleep(SETTLE_MS);
      const review = await page.locator(REVIEW_LI).count();
      assert.ok(review >= 1, '前提: 復習の一覧に項目がある');
      assert.equal(await page.locator(`${REVIEW_LI}[data-achieved="true"]`).count(), review, '前提: 全 Step が達成');
      assert.equal(await page.locator(NEXT).count(), 0, '前提: 次の章へ進むが 0 件');
      await cancelCheck(page, eg);
      await deleteCheck(page, eg);
    });
  });
}
