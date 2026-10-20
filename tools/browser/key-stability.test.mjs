// 教材の版・内容が変わっても保存先の鍵が変わらず、読めない IR で保存先が変わらないことの試験（TASK-326 2/2。SAVE-14・DLV-8）。
//
// 保存先の鍵は教材の ID と最初の開始点の ID だけで決まり、版・内容に依らない（試作 F-1）。試験の設定（TEST_HOOKS=1）の画面を
// `?fixture=chapter-a` で開いたあと、同じ `pack.id`・開始点で `pack.version` と題名だけが違う教材（`?fixture=chapter-a-next`。
// `src/test_fixture.rs` の `chapter_ir_bytes`）へ移る。新しい試験用の口は足さず、既存の `storageKey`・`stateJson`・`nextTick` と
// localStorage の列挙だけで確かめる。
//
// 1 つの browser context（同じオリジン・同じ保存領域）で次を順に確かめる。
//   1. 章 A を開き、実際の画面で 1 手進めて保存を待つ。履歴の鍵はちょうど 1 本で、`storageKey` の鍵と等しい。
//   2. 版・題名を変えた教材へ移る。履歴の鍵は同じ 1 本のまま（新しい鍵が増えない）。`restored` で、現在の Step・各 Step の状態が手順 1 と同じ。
//      外れたときは固定の印 `KEY-CHANGED-BY-VERSION` を失敗の文に含める。
//   3. もう 1 手進めて保存を待つ。履歴の鍵は同じ 1 本のまま。
//   4. 再読み込みしても `restored` で、鍵は同じ 1 本。
//   5. 読めない IR（`?fixture=future-ir-format`）へ移る。断った画面のマウント後の localStorage の全体を基準に取り、
//      断っている間・再読み込み（pagehide）の後も 1 文字も変わらない。履歴の鍵は同じ 1 本のまま。
//   6. 章 A へ戻ると `restored` で、手順 3 の後の Step に戻る（陽性の対照）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
// 試験の出力・失敗の文に、保存した値・鍵の文字列・下書きの中身を出さない（有無・長さ・本数・比較の結果だけを出す）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-key-stability/';
const PACK = 'chapter-links';
const START_A = 's-1';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
const CHANGED = 'KEY-CHANGED-BY-VERSION';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const NOTICE = '[data-testid="ir-notice"]';
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
const historyKeys = (page) => page.evaluate((prefix) => {
  const keys = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    if (k.startsWith(prefix)) keys.push(k);
  }
  return keys.sort();
}, HISTORY_PREFIX);
// 保存先（localStorage）の全体。鍵で並べた文字列にして、一致かどうかだけを比べる。
const storageDump = (page) => page.evaluate(() => JSON.stringify(
  Object.keys(localStorage).sort().map((k) => [k, localStorage.getItem(k)]),
));
const slicesOf = (eng) => JSON.stringify(eng.stateWithoutLog.run.slices);

async function go(page, srv, query) {
  await page.goto(`${srv.url}${query}`);
  await waitMounted(page);
}

// 1 手進める（コマンドを送り、受け付けられて処理が終わるまで待つ）。
async function move(page, text) {
  const before = (await engine(page)).current;
  await page.fill(INPUT, text);
  await page.click(SUBMIT);
  await page.waitForFunction((cur) => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    const st = e.stateWithoutLog.run.slices.main?.status ?? {};
    return st[cur] === 'accepted' && window.__fandheTest.nextTick() === 'null';
  }, before, { timeout: T });
}

// 保存の書き出し（変更から 1 秒以内。SAVE-4）を待つ。
const waitWritten = (page, key) => page.waitForFunction((k) => localStorage.getItem(k) !== null, key, { timeout: T });

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify({ csp: s.csp, pageErrors: s.pageErrors, consoleErrors: s.consoleErrors });
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`版・内容が変わっても同じ鍵で履歴が開け、読めない IR で保存先が変わらない（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });

      // 1. 章 A を 1 手進めて保存する。
      await go(page, srv, '?fixture=chapter-a');
      const keyA = await keyOf(page, PACK, START_A);
      assert.equal((await engine(page)).bootKind, 'fresh');
      assert.deepEqual(await historyKeys(page), [], 'nothing is saved yet');
      await move(page, '/make-note note-1');
      await waitWritten(page, keyA);
      assert.deepEqual(await historyKeys(page), [keyA], 'exactly the chapter A key');
      const first = await engine(page);

      // 2. 版・題名を変えた教材へ移る。鍵は増えず、同じ履歴で開く。
      await go(page, srv, '?fixture=chapter-a-next');
      const n0 = await engine(page);
      const keys0 = await historyKeys(page);
      const changed = n0.bootKind !== 'restored' || n0.current !== first.current
        || slicesOf(n0) !== slicesOf(first) || keys0.length !== 1 || keys0[0] !== keyA;
      assert.equal(changed, false, `${CHANGED}: the changed pack did not open the saved history (bootKind=${n0.bootKind}, keys=${keys0.length})`);
      assert.equal(n0.incrementalMatches, true, 'incremental projection');
      assert.equal(n0.worldMatchesState, true, 'world matches state');

      // 3. もう 1 手進める。鍵は増えない。
      const logBefore = n0.logLen;
      await page.fill(INPUT, '/make-note note-2');
      await page.click(SUBMIT);
      await page.waitForFunction((len) => {
        const e = JSON.parse(window.__fandheTest.stateJson()).engine;
        return e.logLen > len && window.__fandheTest.nextTick() === 'null';
      }, logBefore, { timeout: T });
      const second = await engine(page);
      assert.ok(second.logLen > logBefore, 'the second operation was recorded');
      assert.deepEqual(await historyKeys(page), [keyA], 'still exactly the chapter A key');

      // 4. 再読み込みしても同じ鍵・復元。
      await page.reload();
      await waitMounted(page);
      const r = await engine(page);
      assert.equal(r.bootKind, 'restored', 'restored after reload');
      assert.equal(r.current, second.current, 'current step after reload');
      assert.equal(slicesOf(r), slicesOf(second), 'step statuses after reload');
      assert.deepEqual(await historyKeys(page), [keyA], 'still exactly the chapter A key after reload');

      // 5. 読めない IR。断った画面のマウント後を基準に、保存先の全体が変わらない。
      await go(page, srv, '?fixture=future-ir-format');
      assert.equal(await page.locator(NOTICE).count(), 1, 'the refusal notice is shown');
      const baseline = await storageDump(page);
      assert.notEqual(baseline, '[]', 'saved history is still present (non-vacuous)');
      assert.deepEqual(await historyKeys(page), [keyA], 'no new key while refusing');
      await page.reload();
      await waitMounted(page);
      assert.equal(await page.locator(NOTICE).count(), 1);
      assert.equal(await storageDump(page), baseline, 'storage is unchanged after pagehide');
      assert.deepEqual(await historyKeys(page), [keyA]);

      // 6. 陽性の対照: 章 A へ戻ると、手順 3 の後の Step に戻る。
      await go(page, srv, '?fixture=chapter-a');
      const back = await engine(page);
      assert.equal(back.bootKind, 'restored');
      assert.equal(back.current, second.current);
      assert.deepEqual(await historyKeys(page), [keyA]);

      clean(rec);
      await context.close();
    }));
  });
}
