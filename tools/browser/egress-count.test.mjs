// 受講者の入力・履歴を外部へ送らないことの通信の計数（SAFE-16。TASK-153。記録は docs/testing.md 5.371）。
//
// 「送らない」は完了した要求ではなく、送ろうとした呼び出しの件数で確かめる。egress.mjs の記録器が、通信の API の呼び出し
// （fetch・XHR・WebSocket・EventSource・sendBeacon・RTCPeerConnection・ServiceWorker・Worker）・WebSocket と Worker のイベント・
// 配信元への GET 以外の要求・構成要素以外のパス／サブリソースのクエリを数え、harness の rec（外部への要求・ポップアップ・CSP 違反）と
// 合わせて全区分 0 を主張する。CSP ありの回は止められた試みが route に届かないため、CSP なしの回と呼び出しの計数が本命になる。
//
// 場面:
//   G1（試験の設定）  CH-0〜CH-3 の形の 20 Step を shell-journey と同じ誘導で通し、途中でチェックポイントの保存→復帰・再読み込み・
//                      書き出し→別のオリジンでの読み込み・保存の失敗（localStorage 側）を挟む。IndexedDB と localStorage の両方の保存先を通る。
//                      別のオリジンの context にも記録器を付け、両方で全区分 0。
//   G2（配信する設定）  D3 と同じ準備（IR を足した写し）で 20 Step を DOM だけで通し、続けて画面の操作だけで保存→復帰・書き出し・
//                      再読み込み・別のオリジンでの読み込みを行う。試験用の口が無いこと、操作が空振りしていないこと（ダウンロードの発生・
//                      チェックポイントの一覧・読み込み後の位置の一致）も確かめる。fetch の許可は wasm と IR の GET だけで、件数を固定する。
//   C（対照）          記録器が検出できることの確認。1 種類ずつ通信を試み、対応する区分がちょうど 1 増える。宛先は予約ドメイン
//                      example.invalid と閉じたローカルのポートに限り、route で abort するため実際の外部へは出ない。
//
// 確かめないこと（docs/known-gaps.md「SAFE-16」）: 本物の CH-0〜CH-15・2 周目の形・履歴の削除の操作（TASK-539）・L2（miniserve）。
// 入力: FANDHE_WEB_DIR（配信する設定）・FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は失敗にする。
// 出力: 場面ごとの区分の件数の 1 行。入力・履歴・URL の値は出さない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { attachEgress, egressCounts, egressFailures, egressLine } from './egress.mjs';
import {
  ledger, journey, install, mouseDriver,
} from './ch0-3-journey.mjs';
import { runShellJourney, POSITION } from './shell-journey.mjs';
import {
  makeTripHooks, newTrips, tripsOk, addHideIdbInitScript, exportByUi, SAVE, RESTORE,
} from './shell-journey-trips.mjs';
import { withDeliveryCopy } from './ch4-9-journey.mjs';
import { ORDER, domObserver03, variantOf } from './author-preview.mjs';

const BASE = '/fandhe-egress/';
const BASE_B = '/another-egress/';
const QUERY = '?fixture=ch0-3-chapters';
const IR_03 = fileURLToPath(new URL('../../crates/web/tests/data/ch0_3/ir.json', import.meta.url));
const FILE_INPUT = '[data-testid="shell-import"] input[type="file"]';
const EXPORT_BTN = '[data-testid="shell-export"]';
let browser;
let tmpDir;

before(async () => {
  for (const name of ['FANDHE_WEB_DIR', 'FANDHE_WEB_TEST_DIR']) {
    const dir = process.env[name];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a built web directory`);
  }
  browser = await launchBrowser({ channel: channelFromEnv() });
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-egress-'));
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

// 閉じた context でも例外にせず、取れない区分は 0 として扱わずに失敗にしたいので、閉じる前の呼び出しで使う。
const countsOf = (eg, rec) => egressCounts(eg, rec);

// 配信する設定の写し（CSP の有無を選ぶ）に IR を足して配る。D3（csp-runs.test.mjs）と同じ準備。
async function withDelivery(csp, fn) {
  await withDeliveryCopy(async (copy) => {
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), JSON.stringify(variantOf(JSON.parse(fs.readFileSync(IR_03, 'utf8')))));
    await withCspMode({ dir: copy, csp }, fn);
  });
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`G1: 試験の設定の 20 Step と保存・復帰・再読み込み・書き出し・別オリジンでの読み込みで、通信の試みが全区分 0 件（${mode}）`, { timeout: 600000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, (srv) => withServer({ dir, base: BASE_B }, async (srvB) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
      const bs = [];
      try {
        const eg = await attachEgress(context, { origin: srv.origin, base: BASE });
        const L = ledger();
        await addHideIdbInitScript(context);
        await page.goto(`${srv.url}${QUERY}`);
        await waitMounted(page);
        assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
        const openB = async () => {
          const o = await openPage(browser, { origin: srvB.origin, viewport: { width: 1280, height: 800 } });
          const egB = await attachEgress(o.context, { origin: srvB.origin, base: BASE_B });
          await o.page.goto(`${srvB.url}${QUERY}`);
          await waitMounted(o.page);
          // 閉じる前に区分を読むため、close を包んで最後の値を残す。
          const close = o.context.close.bind(o.context);
          const entry = { eg: egB, rec: o.rec, last: null };
          o.context.close = async () => {
            entry.last = countsOf(egB, o.rec);
            return close();
          };
          bs.push(entry);
          return o;
        };
        const trips = newTrips();
        const trip = makeTripHooks({
          openB, tmpDir, scene: 'g1', L, trips,
        });
        const result = await runShellJourney(page, { width: 1280, scene: 'g1', L, hooks: { afterStep: trip.afterStep } });
        await trip.finish(page, 'main/st-3-2');
        assert.deepEqual(L.failed(), []);
        assert.equal(result.achieved, 20);
        assert.ok(tripsOk(trips), '挟んだ 4 区間がすべて対象 1 以上で全合格');
        assert.ok(bs.length >= 1, '別のオリジンでの読み込みが行われている');
        const s = summarize(rec);
        assert.equal(s.pageErrors, 0);
        const counts = countsOf(eg, rec);
        const safety = await collectSafety(page, rec);
        assert.deepEqual(nonZero(safety), [], '安全性の計数が全区分 0');
        console.log(egressLine('G1 A', csp, counts, eg, `trips=${JSON.stringify(trips)}`));
        assert.deepEqual(egressFailures(counts), [], 'A の通信の区分');
        for (const [i, b] of bs.entries()) {
          const c = b.last ?? countsOf(b.eg, b.rec);
          console.log(egressLine(`G1 B${i}`, csp, c, b.eg));
          assert.deepEqual(egressFailures(c), [], `B${i} の通信の区分`);
        }
        // fetch は許可した宛先（wasm の取得）だけ。件数は読み込みの回数（再読み込みの区間を含む）に比例するため下限だけを見る。
        assert.ok(eg.allowedFetch >= 1 && eg.allowedFetch === eg.calls, 'fetch は許可した宛先だけで、1 件以上');
      } finally {
        await context.close();
      }
    })));
  });

  test(`G2: 配信する設定の 20 Step と画面の操作だけの保存・復帰・書き出し・再読み込み・別オリジンでの読み込みで、通信の試みが全区分 0 件（${mode}）`, { timeout: 600000 }, async () => {
    await withDelivery(csp, (dir) => withServer({ dir, base: BASE }, (srv) => withServer({ dir, base: BASE_B }, async (srvB) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
      let b = null;
      try {
        const eg = await attachEgress(context, { origin: srv.origin, base: BASE });
        await page.goto(srv.url);
        await waitMounted(page);
        assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', '配信する設定に試験用の口がある');
        await install(page);
        const L = ledger();
        await journey(page, mouseDriver(page), 'egress ch0-3 配信', L, null, { chaptered: true, observe: domObserver03() });
        assert.deepEqual(L.failed(), []);
        const review = await page.evaluate(() => [...document.querySelectorAll('[data-testid="part-review-list"] > li')].map((li) => li.dataset.achieved));
        assert.equal(review.length, ORDER.length);
        assert.ok(review.every((a) => a === 'true'), '全 Step が達成');

        const position = () => page.locator(POSITION).textContent();
        const posEnd = await position();
        // チェックポイントの保存→復帰（画面のボタンだけ）。
        await page.click(SAVE);
        await page.locator(RESTORE).first().waitFor({ state: 'attached', timeout: 30000 });
        const listed = await page.locator(RESTORE).count();
        assert.ok(listed >= 1, 'チェックポイントの一覧に項目がある');
        await page.locator(RESTORE).first().click();
        assert.equal(await position(), posEnd, '復帰の後も位置が同じ');
        // 書き出し（ダウンロードのリンク）。
        const file = await exportByUi(page, tmpDir);
        assert.ok(fs.statSync(file).size > 0, '書き出したファイルが空でない');
        // 再読み込み。
        await page.reload();
        await waitMounted(page);
        await page.waitForFunction(([sel, t]) => document.querySelector(sel)?.textContent === t, [POSITION, posEnd], { timeout: 30000 });
        // 別のオリジンでの読み込み（ファイル入力）。
        const o = await openPage(browser, { origin: srvB.origin, viewport: { width: 1280, height: 800 } });
        b = o;
        const egB = await attachEgress(o.context, { origin: srvB.origin, base: BASE_B });
        await o.page.goto(srvB.url);
        await waitMounted(o.page);
        await o.page.locator(FILE_INPUT).setInputFiles(file);
        await o.page.waitForFunction(([sel, t]) => document.querySelector(sel)?.textContent === t, [POSITION, posEnd], { timeout: 30000 });
        assert.ok(await o.page.locator(EXPORT_BTN).count() >= 1, '読み込んだ側にも書き出しの操作がある');

        const counts = countsOf(eg, rec);
        const countsB = countsOf(egB, o.rec);
        console.log(egressLine('G2 A', csp, counts, eg, `checkpoints=${listed} export=1 reload=1 import=1`));
        console.log(egressLine('G2 B', csp, countsB, egB));
        assert.deepEqual(egressFailures(counts), [], 'A の通信の区分');
        assert.deepEqual(egressFailures(countsB), [], 'B の通信の区分');
        assert.equal(summarize(rec).pageErrors, 0);
        assert.equal(summarize(o.rec).pageErrors, 0);
        // fetch は wasm の取得と IR の取得の 2 件（再読み込みの前の分。再読み込みで再び 2 件ずつ増えるため 2 の倍数になる）。
        assert.ok(eg.allowedFetch >= 2 && eg.allowedFetch % 2 === 0, `fetch は wasm と IR の組（${eg.allowedFetch}）`);
        assert.equal(egB.allowedFetch, 2, 'B の fetch は wasm と IR の 2 件');
      } finally {
        if (b) await b.context.close();
        await context.close();
      }
    })));
  });
}

// ---- C: 対照。記録器が 1 種類ずつの試みを検出できること ----

// 試みの一覧。attempt はページ内で実行する関数、key は増えるべき区分。宛先は予約ドメインと閉じたローカルのポートに限る。
const ATTEMPTS = [
  { name: '外部への fetch', key: 'offOriginCalls', run: () => { fetch('https://example.invalid/a').catch(() => {}); } },
  { name: 'sendBeacon', key: 'offOriginCalls', run: () => { navigator.sendBeacon('https://example.invalid/b', 'x'); } },
  { name: 'XMLHttpRequest', key: 'offOriginCalls', run: () => { const x = new XMLHttpRequest(); x.open('POST', 'https://example.invalid/c'); try { x.send('x'); } catch { /* 送れなくてよい */ } } },
  { name: 'WebSocket', key: 'ws', run: () => { const w = new WebSocket('ws://127.0.0.1:1/'); w.onerror = () => {}; } },
  { name: 'EventSource', key: 'offOriginCalls', run: () => { const e = new EventSource('https://example.invalid/d'); e.onerror = () => e.close(); } },
  { name: '外部の画像', key: 'external', run: () => { new Image().src = 'https://example.invalid/e.png'; } },
  { name: '配信元への POST', key: 'nonGet', run: (base) => { fetch(`${base}ir/main.json`, { method: 'POST', body: 'x' }).catch(() => {}); } },
  { name: '配信元の許可外のパス', key: 'offAllowlist', run: (base) => { fetch(`${base}api/save`).catch(() => {}); } },
  { name: '配信元へのクエリつき GET', key: 'offAllowlist', run: (base) => { fetch(`${base}ir/main.json?q=1`).catch(() => {}); } },
  { name: 'Worker', key: 'workers', run: () => { new Worker(URL.createObjectURL(new Blob(['']))); } },
];

test('C: 通信を 1 種類ずつ試みると、記録器の対応する区分がちょうど 1 増える（CSP なしの配信する設定の写し）', { timeout: 300000 }, async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp: false }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      const eg = await attachEgress(context, { origin: srv.origin, base: BASE });
      await page.goto(srv.url);
      await waitMounted(page);
      assert.deepEqual(egressFailures(countsOf(eg, rec)), [], '起動直後は全区分 0');
      for (const a of ATTEMPTS) {
        const before = countsOf(eg, rec);
        await page.evaluate(`(${a.run.toString()})(${JSON.stringify(BASE)})`);
        await page.waitForTimeout(300);
        const after = countsOf(eg, rec);
        assert.equal(after[a.key] - before[a.key], 1, `${a.name}: ${a.key} がちょうど 1 増える`);
        assert.ok(egressFailures(after).length > 0, `${a.name}: 失敗として検出される`);
        // 次の試みの差を見るため、増えた分は引き算で扱う（記録器は累積）。
      }
      console.log(egressLine('C', false, countsOf(eg, rec), eg, `attempts=${ATTEMPTS.length}`));
    } finally {
      await context.close();
    }
  }));
});
