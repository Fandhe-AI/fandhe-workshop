// 同じ教材の別の章のリンクを同じオリジンで順に開いても、履歴が混ざらないことの試験（TASK-326 1/2。SAVE-14・DLV-8）。
//
// 試作の F-1: 同じオリジンで、同じ教材の別の章（開始点の違う IR）のリンクを順に開くと、先に保存した別の章の履歴が出た。
// 鍵が固定の文字列で、章ごとの IR は内容のハッシュが違うため「変換」の経路で開いたためである。
// 試験の設定（TEST_HOOKS=1）の画面を `?fixture=chapter-a`・`?fixture=chapter-b` で開くと、起動の前に、
// 同じ `pack.id`（chapter-links）で開始点だけが違う教材 2 本（`src/test_fixture.rs` の `chapter_ir_bytes`）が選ばれる。
//
// 1 つの browser context（同じオリジン・同じ保存領域）で次を順に確かめる。
//   1. 章 A を開き、実際の画面で 1 手進めて保存を待つ（鍵が書かれる）。
//   2. 同じタブで章 B へ移る。起動の種類は `fresh`、現在の Step は章 B の開始、`reloaded` の行は 0 件、章 A の値の長さは不変。
//   3. 章 B を 1 手進める。履歴の鍵がちょうど 2 本になり、章 A・章 B の鍵と一致する（章 B が保存を止めていない）。
//   4. 章 A へ戻る。`restored` で、手順 1 の終わりの Step に戻る。
//   5. 章 B へ戻る。`restored` で、手順 3 の Step に戻る。
//   6. 各章で再読み込みしても、4・5 と同じになる。
//   7. 別の教材（クエリなしの既定）を開くと `fresh` で、鍵が 3 本目になり、章の 2 本の値の長さは不変。
// 手順 2 の判定に失敗したときは、固定の印 `CHAPTER-HISTORY-MIXED`（先の章の履歴が出た）を失敗の文に含める。
// scripts/test-chapter-key-control.sh は、鍵を修正前の固定の文字列にした写しでこの印の失敗を確かめる（対照）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
// 試験の出力・失敗の文に、保存した値・鍵の文字列・下書きの中身を出さない（履歴は受講者のデータ。有無・長さ・本数・比較の結果だけを出す）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-chapter-keys/';
const PACK = 'chapter-links';
const START_A = 's-1';
const START_B = 's-4';
const DEFAULT_PACK = 'min-3step';
const DEFAULT_START = 's-1';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
const MIXED = 'CHAPTER-HISTORY-MIXED';
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
const savedLen = (page, key) => page.evaluate((k) => {
  const v = localStorage.getItem(k);
  return v === null ? null : v.length;
}, key);
const historyKeys = (page) => page.evaluate((prefix) => {
  const keys = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    if (k.startsWith(prefix)) keys.push(k);
  }
  return keys.sort();
}, HISTORY_PREFIX);
const reloadedRows = (eng) => eng.log.filter((r) => r.kind === 'reloaded').length;

async function go(page, srv, query) {
  await page.goto(`${srv.url}${query}`);
  await waitMounted(page);
}

// 保存の書き出し（変更から 1 秒以内。SAVE-4）を待つ。
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

  test(`同じ教材の別の章を順に開いても履歴が混ざらず戻ると各章の履歴が出る（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });

      // 1. 章 A を 1 手進めて保存する。
      await go(page, srv, '?fixture=chapter-a');
      const keyA = await keyOf(page, PACK, START_A);
      const keyB = await keyOf(page, PACK, START_B);
      assert.notEqual(keyA, keyB, 'chapter keys differ');
      assert.equal((await engine(page)).bootKind, 'fresh', 'chapter A starts fresh');
      assert.equal((await engine(page)).current, 'main/st-1');
      assert.deepEqual(await historyKeys(page), [], 'nothing is saved yet');
      await moveA(page);
      // 保存された鍵は、試験の口が求めた鍵とは別に、画面が実際に書いた 1 本を見る（鍵の退行の対照で、鍵の違いが
      // 「章 B を開いたときの判定」より前の別の失敗にならないようにするため）。
      await page.waitForFunction((prefix) => {
        for (let i = 0; i < localStorage.length; i += 1) if (localStorage.key(i).startsWith(prefix)) return true;
        return false;
      }, HISTORY_PREFIX, { timeout: T });
      const written = await historyKeys(page);
      assert.equal(written.length, 1, 'one history key after the first operation');
      const [writtenKey] = written;
      const aEng = await engine(page);
      const aLen = await savedLen(page, writtenKey);
      assert.notEqual(aLen, null);

      // 2. 同じタブで章 B へ移る。章 A の履歴は出ない。
      await go(page, srv, '?fixture=chapter-b');
      const b0 = await engine(page);
      const mixed = b0.bootKind !== 'fresh' || b0.current !== 'main/st-4'
        || reloadedRows(b0) !== 0 || (await savedLen(page, writtenKey)) !== aLen;
      assert.equal(mixed, false, `${MIXED}: chapter B shows chapter A history (bootKind=${b0.bootKind}, current=${b0.current}, reloadedRows=${reloadedRows(b0)})`);
      assert.equal(b0.generation, 1, 'chapter B generation stays at the first');
      assert.equal(writtenKey, keyA, 'the written key is the chapter A key');

      // 3. 章 B を 1 手進める。鍵がちょうど 2 本になる。
      await moveB(page);
      await waitSaved(page, keyB);
      const bEng = await engine(page);
      assert.notEqual(bEng.current, 'main/st-4', 'chapter B moved');
      assert.deepEqual(await historyKeys(page), [keyA, keyB].sort(), 'exactly the two chapter keys');
      assert.equal(await savedLen(page, keyA), aLen, 'chapter A value is unchanged');

      // 4・5・6. 各章へ戻ると、保存済みの履歴が出る。再読み込みしても同じ。
      for (const [query, expected, name] of [
        ['?fixture=chapter-a', aEng, 'chapter A'],
        ['?fixture=chapter-b', bEng, 'chapter B'],
      ]) {
        await go(page, srv, query);
        for (const how of ['navigated', 'reloaded']) {
          if (how === 'reloaded') {
            await page.reload();
            await waitMounted(page);
          }
          const e = await engine(page);
          assert.equal(e.bootKind, 'restored', `${name} (${how}): restored`);
          assert.equal(e.current, expected.current, `${name} (${how}): current step`);
          assert.equal(
            JSON.stringify(e.stateWithoutLog.run.slices),
            JSON.stringify(expected.stateWithoutLog.run.slices),
            `${name} (${how}): step statuses`,
          );
          assert.equal(e.incrementalMatches, true, `${name} (${how}): incremental projection`);
          assert.equal(e.worldMatchesState, true, `${name} (${how}): world matches state`);
        }
      }
      assert.deepEqual(await historyKeys(page), [keyA, keyB].sort(), 'still exactly the two chapter keys');

      // 復元（世代の加算）は離れるときに値を書き直しうる。章 A は最後に離れた後なので長さを基準にでき、
      // 章 B は今のページを離れるときに書き直されうるため、別の教材を開いた後も値があることだけを見る。
      const aBefore = await savedLen(page, keyA);

      // 7. 別の教材（既定）は新しく始まり、鍵が 3 本目になる。章の 2 本の値は変わらない。
      await go(page, srv, '');
      const d = await engine(page);
      assert.equal(d.bootKind, 'fresh', 'another pack starts fresh');
      assert.equal(d.current, 'main/st-1');
      assert.equal(reloadedRows(d), 0);
      const keyD = await keyOf(page, DEFAULT_PACK, DEFAULT_START);
      assert.equal(await savedLen(page, keyD), null, 'another pack key is not written before an operation');
      await page.fill(INPUT, '/make-note first-note');
      await page.click(SUBMIT);
      await waitSaved(page, keyD);
      assert.deepEqual(await historyKeys(page), [keyA, keyB, keyD].sort(), 'three keys');
      assert.equal(await savedLen(page, keyA), aBefore, 'chapter A value is unchanged');
      assert.notEqual(await savedLen(page, keyB), null, 'chapter B value is kept');

      clean(rec);
      await context.close();
    }));
  });
}
