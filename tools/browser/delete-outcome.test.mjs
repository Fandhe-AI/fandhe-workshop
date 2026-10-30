// 「削除する」「やめる」と再読み込みの動作を、配信する設定の画面で確かめる試験（SHELL-38・SAVE-32・D-75・SAFE-16。TASK-539 2/2）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。試験用の口（`window.__fandheTest`）の無い配信する設定
// （IR_FILE つき・TEST_HOOKS なし）で、実際のボタンの操作と DOM・保存先（localStorage・IndexedDB）の読み取りだけで確かめる。
// 試験の設定の版（delete-history-scenes。5.391）は口で同じ 3 場面を見ており、本試験はその実画面の版である。
// 入力: FANDHE_WEB_AUTHOR_DIR・FANDHE_AUTHOR_IR。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
//
// 確かめること（受講の途中・章の終わり・復習中の 3 場面）:
//   「やめる」: 確認待ちの間も、やめた後も、保存先の鍵と値・位置・ログ・「次の章へ進む」・復習の一覧が変わらない。確認の通知は消える。
//     再読み込みしても同じ位置・同じ保存先から復元される。
//   「削除する」: この教材の鍵が localStorage と IndexedDB の両方で 0 件になり、別の教材・別の配信先・読めない鍵・履歴でない鍵・
//     起動の印は件数も値も変わらず、新しい鍵も増えない。最初の Step から始まり（位置・達成 0・次の章へ進むが 0）、再読み込みしても
//     最初の Step から始まり鍵は 0 件のまま（未操作の起動は保存しない。SAVE-28）。途中の場面では、その後の 1 操作で保存が再開する。
//   通信: 「やめる」「削除する」の区間で通信の API の呼び出しが 0 件。場面の全体で配信元以外への通信・GET 以外・許可外のパスも 0 件。
//     記録器が付いていることは、起動時の wasm・IR の取得が数えられている（allowedFetch > 0）ことで示す。
//   場面の途中場面だけ、ボタンの操作をキーボード（focus して Enter）で行う。ほかはマウスの click である。
//
// 「復習中」の代役（自動運転での判断。人の承認は無い）: 復習の画面（REVIEW-10）は未実装のため、20 Step をすべて達成し、復習の一覧が全件達成で、
// 「次の章へ進む」が 0 件の状態を「復習中」とみなす。REVIEW-10 の実画面ができたら確かめ直す（docs/known-gaps.md）。
//
// 範囲外: 削除の失敗の表示をブラウザで起こす確認（native の試験が持つ）。別のタブとの合流（SAVE-12）。
// 試験の出力・失敗の文に、保存した値・鍵の文字列を出さない（有無・本数・比較の結果・区分の件数だけを出す）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize, collectSafety, nonZero,
} from './harness.mjs';
import { attachEgress, egressCounts, egressFailures, egressLine } from './egress.mjs';
import {
  ledger, journey, install, mouseDriver, NEXT, LOG,
} from './ch0-3-journey.mjs';
import { ORDER, domObserver03, positionOf } from './author-preview.mjs';
import { localEntries, idbDump, idbPut, sameScopeKey } from './delete-history.mjs';

const BASE = '/fandhe-delete-outcome/';
const REVIEW_LI = '[data-testid="part-review-list"] > li';
const POSITION = '[data-testid="shell-position"]';
const NOTICE = '[data-testid="shell-notice-item"][data-notice-kind="delete-confirm"]';
const DELETE = '[data-testid="shell-delete"]';
const CONFIRM = '[data-testid="shell-delete-confirm"]';
const CANCEL = '[data-testid="shell-delete-cancel"]';
const HISTORY = 'fandhe-workshop/history/';
const BOOT = 'fandhe-workshop/meta/boot/';
const FIRST = ORDER[0];
const T = 30000;
const SETTLE_MS = 1500;
const STOP = new Error('scene-stop');

let browser;
let webDir;
let ir;
let packId;

before(async () => {
  webDir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(webDir && fs.statSync(webDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS');
  const irFile = process.env.FANDHE_AUTHOR_IR;
  assert.ok(irFile && fs.statSync(irFile, { throwIfNoEntry: false })?.isFile(), 'FANDHE_AUTHOR_IR must point to the compiled IR');
  ir = JSON.parse(fs.readFileSync(irFile, 'utf8'));
  packId = ir.pack.id;
  assert.ok(typeof packId === 'string' && packId.length > 0, 'pack.id');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const isPackKey = (k) => k.startsWith(HISTORY) && k.includes(`/${packId}/`) && !k.includes('/zz-other-scope/');
const packKeys = (m) => Object.keys(m).filter(isPackKey);
const keep = (m) => Object.fromEntries(Object.entries(m).filter(([k]) => !isPackKey(k)));

// 観察する値の束（DOM と保存先と通信）。鍵・値そのものは比較にだけ使い、出力には出さない。
async function observe(page, eg) {
  const dom = await page.evaluate(([pos, log, next, review, notice, del]) => {
    const rows = document.querySelectorAll(`${log} li`);
    const last = rows.length > 0 ? rows[rows.length - 1] : null;
    return {
      position: document.querySelector(pos)?.textContent ?? null,
      logLen: rows.length,
      lastStep: last?.getAttribute('data-step') ?? null,
      lastKind: last?.getAttribute('data-kind') ?? null,
      next: document.querySelectorAll(next).length,
      review: document.querySelectorAll(review).length,
      achieved: document.querySelectorAll(`${review}[data-achieved="true"]`).length,
      notice: document.querySelectorAll(notice).length,
      del: document.querySelectorAll(del).length,
    };
  }, [POSITION, LOG, NEXT, REVIEW_LI, NOTICE, DELETE]);
  return {
    dom, local: await localEntries(page), idb: await idbDump(page), calls: eg.calls, allowedFetch: eg.allowedFetch,
  };
}

function sameStorage(a, b, what) {
  assert.deepEqual(b.local, a.local, `${what}: localStorage が変わった`);
  assert.deepEqual(b.idb, a.idb, `${what}: IndexedDB が変わった`);
}

// ボタンを押す。keyboard なら focus して Enter、click なら click。
async function activate(page, sel, keyboard) {
  if (keyboard) {
    await page.locator(sel).focus();
    await page.keyboard.press('Enter');
  } else {
    await page.locator(sel).click({ timeout: T });
  }
}

const noticeCount = (page, n) => page.waitForFunction(([s, k]) => document.querySelectorAll(s).length === k, [NOTICE, n], { timeout: T });

// 対象の教材の鍵が localStorage に現れ、値が落ち着くまで待つ（遅延保存）。実際の鍵を返す。
async function waitSaved(page) {
  await page.waitForFunction(([h, b, p]) => {
    for (let i = 0; i < localStorage.length; i += 1) {
      const k = localStorage.key(i) ?? '';
      if (k.startsWith(h) && !k.startsWith(b) && k.includes(`/${p}/`)) return true;
    }
    return false;
  }, [HISTORY, BOOT, packId], { timeout: T });
  let prev = '';
  for (let n = 0; n < 50; n += 1) {
    const cur = JSON.stringify(await localEntries(page));
    if (cur === prev) break;
    prev = cur;
    await sleep(100);
  }
  await sleep(SETTLE_MS);
  const keys = packKeys(await localEntries(page));
  assert.ok(keys.length >= 1, '前提: この教材の鍵が localStorage にある');
  return keys[0];
}

// 別の教材・別の配信先・読めない鍵・履歴でない鍵を同じオリジンに置く。IndexedDB には別の教材と、この教材の別の開始点を置く。
async function seed(page, keyP) {
  const idx = keyP.split('/').indexOf(packId);
  assert.ok(idx > 0, '前提: 鍵の形');
  const foreign = sameScopeKey(keyP, (p) => { p[idx - 1] = 'zz-other-scope'; });
  const unreadable = sameScopeKey(keyP, (p) => { p[idx] = 'unreadable'; });
  const otherPack = sameScopeKey(keyP, (p) => { p[idx] = 'other-pack'; });
  const idbTarget = sameScopeKey(keyP, (p) => { p[p.length - 1] = 'idb-other-start'; });
  assert.ok(isPackKey(idbTarget), '前提: IndexedDB の対象鍵はこの教材の鍵');
  await page.evaluate(([a, b]) => {
    localStorage.setItem(a, 'foreign-scope-value');
    localStorage.setItem(b, 'unreadable-value');
    localStorage.setItem('unrelated-key', 'u');
  }, [foreign, unreadable]);
  await idbPut(page, { [otherPack]: 'idb-other-value', [idbTarget]: 'idb-target-value' });
  assert.ok(packKeys(await idbDump(page)).length >= 1, '前提: この教材の鍵が IndexedDB にある');
}

// 「やめる」を選ぶ。
async function cancelCheck(page, eg, keyboard = false) {
  const a = await observe(page, eg);
  assert.equal(a.dom.notice, 0, '前提: 確認が出ていない');
  await activate(page, DELETE, keyboard);
  await noticeCount(page, 1);
  sameStorage(a, await observe(page, eg), '確認待ちの間');
  await activate(page, CANCEL, keyboard);
  await noticeCount(page, 0);
  await sleep(2000);
  const b = await observe(page, eg);
  sameStorage(a, b, 'やめるの後');
  assert.deepEqual(b.dom, a.dom, 'やめるの後: 画面の状態が変わった');
  assert.equal(b.calls, a.calls, '「やめる」の区間で通信の API の呼び出しが 0 件');
  assert.equal(b.allowedFetch, a.allowedFetch);
  // 再読み込みしても同じ位置・同じ保存先から復元される。
  await page.reload();
  await waitMounted(page);
  await sleep(SETTLE_MS);
  const c = await observe(page, eg);
  // 起動の印は起動のたびに回数が進むので、再読み込みをまたぐ比較からは除く。
  const noBoot = (o) => ({
    local: Object.fromEntries(Object.entries(o.local).filter(([k]) => !k.startsWith(BOOT))),
    idb: o.idb,
  });
  assert.deepEqual(noBoot(c), noBoot(a), 'やめるの後の再読み込み: 保存先が変わった');
  assert.equal(Object.keys(c.local).filter((k) => k.startsWith(BOOT)).length, Object.keys(a.local).filter((k) => k.startsWith(BOOT)).length, '起動の印の本数が変わった');
  for (const k of ['position', 'next', 'achieved']) assert.equal(c.dom[k], a.dom[k], `やめるの後の再読み込み: ${k} が変わった`);
  await install(page);
}

// 「削除する」を選ぶ。通信の差は削除の区間だけで取り、その後の再読み込みは差に入れない。
async function deleteCheck(page, eg, { keyboard = false, resume = false } = {}) {
  const a = await observe(page, eg);
  assert.ok(packKeys(a.local).length >= 1, '前提: この教材の鍵が localStorage にある');
  assert.ok(packKeys(a.idb).length >= 1, '前提: この教材の鍵が IndexedDB にある');
  await activate(page, DELETE, keyboard);
  await noticeCount(page, 1);
  await activate(page, CONFIRM, keyboard);
  // 完了の目印は最初の Step に入った表示と、IndexedDB の対象鍵が 0 件になること。確認の通知は完了より前に消えるため目印にしない。
  await domObserver03().at(page, FIRST, 'entered');
  const deadline = Date.now() + T;
  let b = await observe(page, eg);
  while (packKeys(b.idb).length > 0 && Date.now() < deadline) {
    await sleep(100);
    b = await observe(page, eg);
  }
  await sleep(SETTLE_MS);
  b = await observe(page, eg);
  assert.deepEqual(packKeys(b.local), [], 'この教材の鍵が localStorage に残らない');
  assert.deepEqual(packKeys(b.idb), [], 'この教材の鍵が IndexedDB に残らない');
  assert.deepEqual(b.local, keep(a.local), '対象外の localStorage の鍵と値が変わらず、新しい鍵も増えない');
  assert.deepEqual(keep(b.idb), keep(a.idb), '対象外の IndexedDB の鍵と値が変わらない');
  assert.equal(b.dom.position, positionOf(ir, FIRST), '最初の Step から新しく始まる');
  assert.equal(b.dom.next, 0);
  assert.equal(b.dom.achieved, 0);
  assert.equal(b.dom.notice, 0);
  assert.equal(b.dom.del, 1, '削除のボタンが 1 個');
  assert.equal(b.calls, a.calls, '「削除する」の区間で通信の API の呼び出しが 0 件');
  assert.equal(b.allowedFetch, a.allowedFetch);
  // 再読み込みしても最初の Step から始まり、未操作の起動は保存しないので鍵は 0 件のまま。
  await page.reload();
  await waitMounted(page);
  await domObserver03().at(page, FIRST, 'entered');
  await sleep(SETTLE_MS);
  const c = await observe(page, eg);
  assert.equal(c.dom.position, positionOf(ir, FIRST));
  assert.deepEqual(packKeys(c.local), [], '再読み込みの後も localStorage の鍵が 0 件');
  assert.deepEqual(packKeys(c.idb), [], '再読み込みの後も IndexedDB の鍵が 0 件');
  if (resume) {
    // 削除の後も保存が再開する（保存の抑止が解けている）。
    await install(page);
    await mouseDriver(page).choose('opt-right');
    await domObserver03().at(page, FIRST, 'accepted');
    await page.waitForFunction(([h, b2, p]) => {
      for (let i = 0; i < localStorage.length; i += 1) {
        const k = localStorage.key(i) ?? '';
        if (k.startsWith(h) && !k.startsWith(b2) && k.includes(`/${p}/`)) return true;
      }
      return false;
    }, [HISTORY, BOOT, packId], { timeout: T });
  }
}

function finish(label, csp, page, eg, rec) {
  return async () => {
    const counts = egressCounts(eg, rec);
    assert.deepEqual(egressFailures(counts), [], `${label}: 通信の区分が 0 件でない`);
    assert.ok(eg.allowedFetch > 0, `${label}: 記録器が付いている（起動時の取得が数えられている）`);
    const safety = await collectSafety(page, rec);
    assert.deepEqual(nonZero(safety), [], `${label}: 安全性の計数が全区分 0`);
    const s = summarize(rec);
    const detail = JSON.stringify({ csp: s.csp, pageErrors: s.pageErrors, consoleErrors: s.consoleErrors, dialogs: s.dialogs, popups: s.popups });
    assert.equal(s.csp, 0, detail);
    assert.equal(s.pageErrors, 0, detail);
    assert.equal(s.consoleErrors, 0, detail);
    assert.equal(s.dialogs, 0, detail);
    assert.equal(s.popups, 0, detail);
    console.log(egressLine(label, csp, counts, eg));
  };
}

async function scene(csp, label, run) {
  await withCspMode({ dir: webDir, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    const eg = await attachEgress(context, { origin: srv.origin, base: BASE });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', '配信する設定に試験用の口が無い');
    await install(page);
    await run({ page, eg });
    await finish(label, csp, page, eg, rec)();
    await context.close();
  }));
}

for (const csp of [true, false]) {
  const mode = csp ? 'CSP あり' : 'CSP なし';

  test(`受講の途中で、やめるは何も変えず、削除するはこの教材だけを消し、通信が 0 件（キーボード。${mode}）`, { timeout: 600000 }, async () => {
    await scene(csp, 'delete-outcome 途中', async ({ page, eg }) => {
      await journey(page, mouseDriver(page), 'delete-outcome 途中', ledger(), 'main/st-1-3', { chaptered: true, observe: domObserver03() });
      await seed(page, await waitSaved(page));
      await cancelCheck(page, eg, true);
      await deleteCheck(page, eg, { keyboard: true, resume: true });
    });
  });

  test(`章の終わりで、やめるは何も変えず通しが続き、削除するはこの教材だけを消し、通信が 0 件（${mode}）`, { timeout: 600000 }, async () => {
    await scene(csp, 'delete-outcome 章の終わり', async ({ page, eg }) => {
      const base = mouseDriver(page);
      let crossed = 0;
      let keyP = null;
      const driver = {
        ...base,
        async nextChapter() {
          crossed += 1;
          if (crossed <= 2) {
            const k = await waitSaved(page);
            if (keyP === null) {
              keyP = k;
              await seed(page, keyP);
            }
            assert.equal((await observe(page, eg)).dom.next, 1, '章の終わりでは次の章へ進むが 1 個');
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
        journey(page, driver, 'delete-outcome 章の終わり', ledger(), null, { chaptered: true, observe: domObserver03() }),
        (err) => err === STOP,
      );
      assert.equal(crossed, 2);
    });
  });

  test(`復習中（代役）で、やめるは何も変えず、削除するはこの教材だけを消し、通信が 0 件（${mode}）`, { timeout: 600000 }, async () => {
    await scene(csp, 'delete-outcome 復習中', async ({ page, eg }) => {
      await journey(page, mouseDriver(page), 'delete-outcome 復習中', ledger(), null, { chaptered: true, observe: domObserver03() });
      await seed(page, await waitSaved(page));
      const o = await observe(page, eg);
      assert.equal(o.dom.review, ORDER.length, '前提: 復習の一覧に 20 項目がある');
      assert.equal(o.dom.achieved, ORDER.length, '前提: 全 Step が達成');
      assert.equal(o.dom.next, 0, '前提: 次の章へ進むが 0 件');
      await cancelCheck(page, eg);
      await deleteCheck(page, eg);
    });
  });
}
