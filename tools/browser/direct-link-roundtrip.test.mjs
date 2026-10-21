// 章 A と章 B の直接リンクを同じオリジンで往復しても、保存済みの履歴を壊さないことの試験（TASK-110 2/2。DLV-8・SAVE-14・D-68）。
//
// D-68 の直接リンクは `?start=<開始点 ID>` で、IR は 1 つである。保存先の鍵は教材の ID と開始点の ID で決まる（SAVE-14。
// `host.rs` の `history_key`）。したがって同じ保存領域でも、章 A の履歴は章 B のリンクでは読まれず、章 A のリンクへ戻ると続きが出る。
// 教材は試験の設定の `?fixture=ch0-3-links`（開始点 10 件。5.204）を使い、リンクの表は `delivery.mjs` の `DIRECT_LINKS` を使う。
// 章 A はリンク 1（s-0-1）、章 B はリンク 7（s-2-1）。
//
// 1 つの browser context（同じオリジン・同じ保存領域）で、試験 1 は次を順に確かめる。
//   1. 章 A を開き、実クリックで 2 手進めて（どの開始点でもない main/st-0-2a に入る）保存を待つ。
//   2. 同じタブで章 B を開く。`fresh`・章 B の開始の節点・`reloaded` の行 0 件・章 A の値が不変・通知なし・鍵は章 A の 1 本のまま。
//   3. 章 B を再読み込みしても 2 と同じ。
//   4. 他の 8 件のリンクを順に開いても 2 と同じ（章 A の値が不変で鍵が 1 本のまま）。
//   5. 章 A へ戻ると `restored` で、手順 1 の終わりの節点・同じ Step ごとの状態で開く。再読み込みしても同じ。
//   6. 章 A を次の 1 手進めて受付済みにしても、鍵は章 A の 1 本のまま。
//   7. 章 B を開いて 1 手進めると、鍵がちょうど 2 本になる（章 B が保存を止めていない）。
// 手順 2 の判定は 1 つの式 `mixed` にまとめ、失敗の文に固定の印 `DIRECT-LINK-HISTORY-MIXED` を含める。
// 試験 2（対照）は、鍵が章で分かれない修正前の症状を、章 A の値を章 B の鍵の位置へ写して再現し、`mixed` が反応することを確かめる。
//
// 限界（`docs/testing.md` 5.206）。
//   - 起動時の「変換して開きました」の通知は、host がまだ描かない（TASK-535 1/2）。通知の 0 件の判定は描画された後に効き、
//     それまでは `bootKind === 'fresh'`（変換して開く経路は `restored` の一種）が実質の判定である。
//   - 配信する設定は実行時に IR を取得しないため、試験の設定（TEST_HOOKS=1）の画面で確かめる。
//   - 対照は修正前の配信物の再ビルドではなく、保存先の写しによる再現である。
//
// 試験の出力・失敗の文に、鍵の文字列・履歴の値・下書きの中身を出さない（リンクの番号・節点の ID・真偽・件数だけ。値の同一性は SHA-256 の一致で判定し、要約も出さない）。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import { DIRECT_LINKS, directLinkUrl } from './delivery.mjs';
import { engine, at, choiceSel, rowSel, DRAFT, SUBMIT, T } from './ch0-3-journey.mjs';

const BASE = '/fandhe-direct-link-roundtrip/';
const PACK = 'ch0-3-links';
const FIXTURE = '&fixture=ch0-3-links';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
const MIXED = 'DIRECT-LINK-HISTORY-MIXED';
const NOTICE_KIND = '[data-notice-kind="version-converted"]';
const NOTICE = '[data-testid="ir-notice"]';
const CONVERTED_TEXT = '変換して開きました';
const A = DIRECT_LINKS[0];
const B = DIRECT_LINKS[6];
const A_ADVANCED = 'main/st-0-2a';
const B_START = 'main/st-2-1';
const uidOf = (link) => `main/st-${link.start.slice(2)}`;

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  assert.equal(A.start, 's-0-1', 'link 1 is chapter A');
  assert.equal(B.start, 's-2-1', 'link 7 is chapter B');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const historyKeys = (page) => page.evaluate((prefix) => {
  const keys = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    if (k.startsWith(prefix)) keys.push(k);
  }
  return keys.sort();
}, HISTORY_PREFIX);
const keyOf = (page, start) => page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, start]);
// 値そのものは node へ出さず、page の中で SHA-256 の 16 進だけを返す。値の同一性はこの要約の一致で比べる（同じ長さの書き換えも検出する）。
const savedDigest = (page, key) => page.evaluate(async (k) => {
  const v = localStorage.getItem(k);
  if (v === null) return null;
  const buf = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(v));
  return Array.from(new Uint8Array(buf), (b) => b.toString(16).padStart(2, '0')).join('');
}, key);
const reloadedRows = (eng) => eng.log.filter((r) => r.kind === 'reloaded').length;
const slicesOf = (e) => JSON.stringify(e.stateWithoutLog.run.slices);
const url = (srv, link) => `${directLinkUrl(srv.url, link, 'query')}${FIXTURE}`;

async function open(page, srv, link) {
  await page.goto(url(srv, link));
  await waitMounted(page);
}

async function reopen(page) {
  await page.reload();
  await waitMounted(page);
}

// 通知が画面に出ていないこと（契約の属性・通知の枠・本文の語）。
async function noNotice(page) {
  const counts = await page.evaluate(([k, n, text]) => [
    document.querySelectorAll(k).length,
    document.querySelectorAll(n).length,
    document.body.innerText.includes(text) ? 1 : 0,
  ], [NOTICE_KIND, NOTICE, CONVERTED_TEXT]);
  return counts.every((c) => c === 0);
}

// 章 A を 2 手進めて保存を待つ。章 A の鍵と、保存した値の写し（page 内の window）を返す。
async function advanceA(page, srv) {
  await open(page, srv, A);
  const first = await engine(page);
  assert.equal(first.bootKind, 'fresh', 'chapter A starts fresh');
  assert.equal(first.current, uidOf(A), 'chapter A opens its start node');
  assert.deepEqual(await historyKeys(page), [], 'new context has no saved history');
  await page.click(choiceSel('opt-right'));
  await at(page, uidOf(A), 'accepted');
  await page.click(rowSel('cards/notice'));
  await at(page, A_ADVANCED, 'entered');
  await page.waitForFunction((prefix) => {
    for (let i = 0; i < localStorage.length; i += 1) if (localStorage.key(i).startsWith(prefix)) return true;
    return false;
  }, HISTORY_PREFIX, { timeout: T });
  const keyA = await keyOf(page, A.start);
  const keys = await historyKeys(page);
  assert.equal(keys.length, 1, 'one saved history after chapter A');
  assert.equal(keys[0], keyA, 'the saved key is the key of chapter A');
  return keyA;
}

// 章 B のリンクを開いたときの混入の判定（1 つの式）。章 A の値の SHA-256 が保存の前後で変わることも含める。
async function mixedAt(page, startLink, keyA, digestA) {
  const e = await engine(page);
  const digest = await savedDigest(page, keyA);
  return {
    e,
    mixed: e.bootKind !== 'fresh' || e.current !== uidOf(startLink) || reloadedRows(e) > 0 || digest !== digestA,
  };
}

async function assertNotMixed(page, startLink, keyA, digestA, tag) {
  const { e, mixed } = await mixedAt(page, startLink, keyA, digestA);
  assert.equal(mixed, false, `${MIXED}: ${tag}: boot=${e.bootKind} current=${e.current} reloaded=${reloadedRows(e)}`);
  assert.equal(e.generation, 1, `${tag}: generation`);
  assert.equal(await page.evaluate(() => window.__fandheTest.nextTick()), 'null', `${tag}: not processing`);
  assert.equal(await noNotice(page), true, `${tag}: no conversion notice`);
  assert.equal((await historyKeys(page)).length, 1, `${tag}: chapter A history is the only key (an unoperated link writes nothing)`);
  return e;
}

function clean(rec, tag) {
  const s = summarize(rec);
  const detail = JSON.stringify({ csp: s.csp, pageErrors: s.pageErrors, consoleErrors: s.consoleErrors });
  assert.equal(s.csp, 0, `${tag}: ${detail}`);
  assert.equal(s.pageErrors, 0, `${tag}: ${detail}`);
  assert.equal(s.consoleErrors, 0, `${tag}: ${detail}`);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`章 A を進めて保存した後に章 B のリンクを開いても A の履歴は消えず通知も出ず、A に戻ると続きが出る（CSP ${label}）`, { timeout: 600000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const origin = new URL(srv.url).origin;
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      const requested = [];
      page.on('request', (r) => requested.push(r.url()));

      const keyA = await advanceA(page, srv);
      const digestA = await savedDigest(page, keyA);
      assert.ok(typeof digestA === 'string' && digestA.length === 64, 'chapter A value is saved');
      const aEng = await engine(page);
      const aSlices = slicesOf(aEng);

      // 章 B（リンク 7）。
      await open(page, srv, B);
      await assertNotMixed(page, B, keyA, digestA, 'chapter B link');
      const bKey = await keyOf(page, B.start);
      assert.notEqual(bKey, keyA, 'chapter B has its own key');
      await reopen(page);
      await assertNotMixed(page, B, keyA, digestA, 'chapter B reload');

      // 他の 8 件。
      for (let i = 0; i < DIRECT_LINKS.length; i += 1) {
        if (i === 0 || i === 6) continue;
        await open(page, srv, DIRECT_LINKS[i]);
        await assertNotMixed(page, DIRECT_LINKS[i], keyA, digestA, `link ${i + 1}`);
      }

      // 章 A へ戻る。
      await open(page, srv, A);
      const back = await engine(page);
      assert.equal(back.bootKind, 'restored', 'chapter A is restored');
      assert.equal(back.current, A_ADVANCED, 'chapter A continues where it was');
      assert.equal(slicesOf(back), aSlices, 'chapter A has the same step statuses');
      assert.equal(back.incrementalMatches, true, 'incremental matches');
      assert.equal(back.worldMatchesState, true, 'world matches state');
      assert.equal(await noNotice(page), true, 'chapter A: no conversion notice');
      assert.equal(await page.evaluate(() => window.__fandheTest.nextTick()), 'null', 'chapter A: not processing');
      await reopen(page);
      const again = await engine(page);
      assert.equal(again.bootKind, 'restored', 'chapter A is restored after reload');
      assert.equal(again.current, A_ADVANCED, 'chapter A continues after reload');
      assert.equal(slicesOf(again), aSlices, 'chapter A has the same step statuses after reload');
      assert.equal(await noNotice(page), true, 'chapter A after reload: no conversion notice');
      assert.equal((await historyKeys(page)).length, 1, 'still one key');

      // 章 A を次の 1 手進める。
      await page.click(choiceSel('opt-read-a'));
      await at(page, A_ADVANCED, 'accepted');
      await page.waitForTimeout(1500);
      assert.deepEqual(await historyKeys(page), [keyA], 'chapter A stays the only key after its next step');

      // 章 B を 1 手進める。鍵が 2 本になる。
      await open(page, srv, B);
      assert.equal((await engine(page)).current, B_START, 'chapter B opens its start again');
      await page.fill(DRAFT, '/new-idea ideas');
      await page.click(SUBMIT);
      await at(page, B_START, 'accepted');
      await page.waitForFunction((k) => localStorage.getItem(k) !== null, bKey, { timeout: T });
      assert.deepEqual(await historyKeys(page), [keyA, bKey].sort(), 'two keys: chapter A and chapter B');
      assert.ok((await savedDigest(page, keyA)) > 0, 'chapter A value still exists');

      const foreign = requested.filter((u) => !u.startsWith(origin) && !u.startsWith('data:') && !u.startsWith('blob:'));
      assert.equal(foreign.length, 0, `requests to other origins: ${foreign.length}`);
      clean(rec, 'roundtrip');
      await context.close();
    }));
  });

  test(`対照: 鍵が章で分かれない修正前の症状（A の値が B の鍵の位置にある）では混入の判定が反応する（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      const keyA = await advanceA(page, srv);
      const digestA = await savedDigest(page, keyA);
      const keyB = await keyOf(page, B.start);
      // 値は page の中で写し、node へ出さない。
      await page.evaluate(([a, b]) => localStorage.setItem(b, localStorage.getItem(a)), [keyA, keyB]);

      await open(page, srv, B);
      const { e, mixed } = await mixedAt(page, B, keyA, digestA);
      assert.equal(mixed, true, `control: the mixing check must react (boot=${e.bootKind} current=${e.current})`);
      await page.evaluate((b) => localStorage.removeItem(b), keyB);
      await context.close();
      assert.equal(summarize(rec).pageErrors, 0, 'no page errors');
    }));
  });
}
