// 開始点を指す直接リンク `?start=<開始点 ID>` の試験（TASK-109 1/2。DLV-9・SAFE-7・SAFE-33・D-68）。
//
// 直接リンクは IR を 1 つに保ち、URL の `start` を IR の `starts` の ID と完全一致で照らして開始点を選ぶ。
// 一致すればその開始点で開き、一致しなければ開かずに理由（固定のコードだけ）を示し、値は画面・コンソール・保存先に出さない。
// 配信する設定は実行時に IR を取得しないため、照合する IR を持つ試験の設定（TEST_HOOKS=1）の画面で確かめる
// （限界。`docs/testing.md` 5.198）。開始点を 3 件持つ教材は `?fixture=multi-start`（`src/test_fixture.rs` の `multi_start_ir_bytes`）。
//
// 確かめること（CSP あり・なしの両方）。
//   (T1) 指定なしは先頭の開始点（`s-1` の節点）で新しく始まる。
//   (T2) 各開始点（`s-1`・`s-4`・`s-7`）の指定で、その節点で開く。クエリの順序（`start` が先）でも同じ。
//   (T3) 開始点ごとに履歴の鍵が分かれる。`s-1` を 1 手進めて保存した後、`start=s-4` は新しく始まり、`start=s-1` は復元される。
//   (T4) 一致しない値（未知の ID・`../` を含む値・大文字違い・前方一致・空・重複・センチネル）は開かずに断る。
//        通知は固定の文言とコードだけで、値は DOM・タイトル・コンソールに出ない。保存先は断っている間も再読み込みを挟んでも変わらない。
//        断ったあとも、保存済みの履歴は指定なしの画面で開ける。
//   (T5) 配信元以外への通信・ページ内エラー・CSP 違反・コンソールのエラーが 0 件。
// 試験の出力・失敗の文に、入力した値・鍵の文字列・履歴の中身を出さない（有無・長さ・本数・比較の結果だけを出す）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-direct-link-start/';
const PACK = 'start-links';
const POINTS = [
  { id: 's-1', node: 'main/st-1' },
  { id: 's-4', node: 'main/st-4' },
  { id: 's-7', node: 'main/st-7' },
];
const HISTORY_PREFIX = 'fandhe-workshop/history/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const NOTICE = '[data-testid="shell-notice-item"][data-notice-kind="pack-unloadable"]';
const LOG_ITEMS = '[data-testid="part-log-list"] li';
const SENTINEL = 'START-SENTINEL-X';
const REFUSED_TEXT = (code) => `指定された開始点を開けませんでした。（${code}）`;
const T = 30000;

// 断られる指定。`query` は `?fixture=multi-start` に続けて付ける文字列。値そのものは失敗の文に出さず、`name` で呼ぶ。
const REFUSED = [
  { name: 'unknown-id', query: '&start=nope' },
  { name: 'dot-dot-slash', query: '&start=..%2Fsomething' },
  { name: 'dot-dot-in-path', query: '&start=s-1%2F..%2Fs-4' },
  { name: 'upper-case', query: '&start=S-4' },
  { name: 'prefix-only', query: '&start=s-' },
  { name: 'trailing-space', query: '&start=s-4%20' },
  { name: 'empty', query: '&start=' },
  { name: 'duplicate', query: '&start=s-1&start=s-4' },
  { name: 'sentinel', query: `&start=${SENTINEL}` },
  { name: 'long', query: `&start=${'s'.repeat(5000)}` },
];

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
const keyOf = (page, start) => page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, start]);
const historyKeys = (page) => page.evaluate((prefix) => {
  const keys = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    if (k.startsWith(prefix)) keys.push(k);
  }
  return keys.sort();
}, HISTORY_PREFIX);
const storageDump = (page) => page.evaluate(() => JSON.stringify(
  Object.keys(localStorage).sort().map((k) => [k, localStorage.getItem(k)]),
));

async function go(page, srv, query) {
  await page.goto(`${srv.url}${query}`);
  await waitMounted(page);
}

// 開始点 `s-1` の画面で 1 手進める（送信が受理され、進行のタイマーが止まるまで待つ）。
async function moveFromFirst(page) {
  const before = (await engine(page)).current;
  await page.fill(INPUT, '/make-note note-1');
  await page.click(SUBMIT);
  await page.waitForFunction((cur) => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    const st = e.stateWithoutLog.run.slices.main?.status ?? {};
    return st[cur] === 'accepted' && window.__fandheTest.nextTick() === 'null';
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

  test(`指定なしは先頭の開始点・各開始点の指定でその節点で開き、開始点ごとに履歴が分かれる（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });

      // T1: 指定なし。
      await go(page, srv, '?fixture=multi-start');
      const first = await engine(page);
      assert.equal(first.irLoaded, true);
      assert.equal(first.bootKind, 'fresh');
      assert.equal(first.current, POINTS[0].node, 'no start opens the first start');
      assert.equal(await page.locator(NOTICE).count(), 0, 'no notice');
      assert.equal(await page.locator(INPUT).count(), 1, 'input field is shown');

      // T2: 各開始点。`start` が先のクエリでも同じ。
      for (const { id, node } of POINTS) {
        for (const query of [`?fixture=multi-start&start=${id}`, `?start=${id}&fixture=multi-start`]) {
          await go(page, srv, query);
          const e = await engine(page);
          assert.equal(e.irLoaded, true, `${id}: loaded`);
          assert.equal(e.irRefusal, null, `${id}: not refused`);
          assert.equal(e.current, node, `${id}: opens its node`);
          assert.equal(await page.locator(NOTICE).count(), 0, `${id}: no notice`);
        }
      }

      // T3: 開始点ごとの履歴。`s-1` を進めて保存する。
      const keys = await Promise.all(POINTS.map(({ id }) => keyOf(page, id)));
      assert.equal(new Set(keys).size, POINTS.length, 'keys differ per start');
      await go(page, srv, '?fixture=multi-start&start=s-1');
      await moveFromFirst(page);
      await page.waitForFunction((k) => localStorage.getItem(k) !== null, keys[0], { timeout: T });
      assert.deepEqual(await historyKeys(page), [keys[0]], 'only the s-1 key is written');
      const slicesOf = (e) => JSON.stringify(e.stateWithoutLog.run.slices);
      const moved = slicesOf(await engine(page));
      assert.notEqual(moved, slicesOf(first), 'the s-1 history moved');

      await go(page, srv, '?fixture=multi-start&start=s-4');
      const s4 = await engine(page);
      assert.equal(s4.bootKind, 'fresh', 's-4 starts fresh (s-1 history is not opened)');
      assert.equal(s4.current, POINTS[1].node);
      assert.equal(await page.locator(NOTICE).count(), 0);

      await go(page, srv, '?fixture=multi-start&start=s-1');
      const back = await engine(page);
      assert.equal(back.bootKind, 'restored', 's-1 history is restored');
      assert.equal(slicesOf(back), moved, 'the s-1 step statuses are restored');
      assert.deepEqual(await historyKeys(page), [keys[0]], 'opening s-4 without an operation writes nothing');

      clean(rec);
      await context.close();
    }));
  });

  test(`一致しない start は開かずに断り、値を出さず、保存先を変えない（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      const consoleTexts = [];
      page.on('console', (m) => consoleTexts.push(m.text()));
      const origin = new URL(srv.url).origin;
      const requested = [];
      page.on('request', (r) => requested.push(r.url()));

      // 保存済みの履歴を先に作る（断られる画面が何も書かないことを、空でない保存先で確かめる）。
      await go(page, srv, '?fixture=multi-start&start=s-1');
      await moveFromFirst(page);
      const keyFirst = await keyOf(page, 's-1');
      await page.waitForFunction((k) => localStorage.getItem(k) !== null, keyFirst, { timeout: T });

      for (const { name, query } of REFUSED) {
        await go(page, srv, `?fixture=multi-start${query}`);
        // 離れる直前の書き出しは移動の前のページのもの。比較の基準は、断られる画面がマウントした後に取る。
        const baseline = await storageDump(page);
        assert.notEqual(baseline, '[]', `${name}: a saved history exists (non-vacuous)`);

        assert.equal(await page.locator(NOTICE).count(), 1, `${name}: one notice`);
        assert.equal(await page.locator(NOTICE).innerText(), REFUSED_TEXT('start_unknown'), `${name}: fixed wording and code only`);
        assert.equal(await page.locator('[data-testid="shell-notice"]').getAttribute('role'), 'status', `${name}: role`);
        assert.equal(await page.locator(INPUT).count(), 0, `${name}: no input field`);
        assert.equal(await page.locator(LOG_ITEMS).count(), 0, `${name}: no log rows`);
        const eng = await engine(page);
        assert.equal(eng.irLoaded, false, `${name}: not loaded`);
        assert.equal(eng.irRefusal, 'start_unknown', `${name}: reason code`);

        const html = await page.evaluate(() => document.documentElement.outerHTML);
        const title = await page.title();
        assert.equal(html.includes(SENTINEL), false, `${name}: value is not in the DOM`);
        assert.equal(title.includes(SENTINEL), false, `${name}: value is not in the title`);
        assert.equal(html.includes('..%2F') || html.includes('../something'), false, `${name}: path-like value is not in the DOM`);
        assert.equal(consoleTexts.some((t) => t.includes(SENTINEL)), false, `${name}: value is not logged`);

        assert.equal(await storageDump(page), baseline, `${name}: storage is unchanged while refusing`);
        await page.reload();
        await waitMounted(page);
        assert.equal(await page.locator(NOTICE).count(), 1, `${name}: still refused after reload`);
        assert.equal(await storageDump(page), baseline, `${name}: storage is unchanged after pagehide`);
      }

      // 陽性の対照: 断ったあとも、保存済みの履歴は指定なしの画面で開ける。
      await go(page, srv, '?fixture=multi-start');
      const again = await engine(page);
      assert.equal(again.irLoaded, true);
      assert.equal(again.bootKind, 'restored', 'the saved history is still openable');

      // T5: 配信元以外への通信が無い。
      const foreign = requested.filter((u) => !u.startsWith(origin) && !u.startsWith('data:') && !u.startsWith('blob:'));
      assert.equal(foreign.length, 0, `requests to other origins: ${foreign.length}`);
      clean(rec);
      await context.close();
    }));
  });
}
