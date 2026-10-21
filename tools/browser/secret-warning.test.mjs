// 秘密情報らしい文字列の検出と、保存の直前の警告の試験（SAFE-27・SAFE-29。TASK-164 1/3。PoC-7 の手順 9）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、10 形（S-01〜S-10）を入力欄へ入れ、次を確かめる。
//   (A) 下書きの経路: input の発火の直後は警告が空で、遅延保存の後に `secret-notice` に形の名前が出る。
//       `engine.secretWarning` は期待の形の ID だけで、DOM の警告の文に値が無く、履歴の鍵への書き込みが 0 件。
//   (B) 送信の経路: 送信（即時の保存）でも同じ。
//   (C) 対照 N-01〜N-06: 警告が出ず、履歴の鍵への書き込みが 1 件以上ある。
//   (D) 値の非漏洩: console・pageerror の文に、どの値も含まれない。
// 値は `secret-values.mjs` が部品から組み立て、変数の中だけに置く。assert・diagnostic には試験データの ID と形の ID だけを出す。
// 保存の前は入力欄と画面に値が出る（PoC の限界 (2)）。画面からの消去は TASK-164 3/3 の範囲のため、ここでは確かめない。
// 警告が出るまでの時間は diagnostic に数値だけを残し、assert しない（遅延の定数は期待にしない。SAVE-7）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP ありの設定で流す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted,
} from './harness.mjs';
import { secretShapes, controls } from './secret-values.mjs';

const BASE = '/fandhe-secret-warning/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const NOTICE = '[data-testid="secret-notice"]';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
// 画面に出す名前（試験の側の表。tests の外のパスには置かない）。
const LABELS = {
  github_token: 'GitHub 風のトークン',
  github_pat: 'GitHub 風の細粒度トークン',
  slack_token: 'Slack 風のトークン',
  aws_access_key: 'AWS 風のアクセスキー',
  aws_secret_key: 'AWS 風のシークレットキー',
  stripe_live_key: 'Stripe 風の本番キー',
  google_api_key: 'Google 風の API キー',
  jwt: 'JWT 風のトークン',
  private_key: '秘密鍵',
  connection_string: '資格情報つきの接続文字列',
};

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 履歴の鍵への書き込みの記録器。記録するのは長さだけで、値・鍵の文字列は持たない。
function installRecorder(prefix) {
  window.__saveWrites = [];
  const mine = (key) => typeof key === 'string' && key.startsWith(prefix);
  const origSet = Storage.prototype.setItem;
  Storage.prototype.setItem = function setItem(key, value) {
    if (mine(key)) window.__saveWrites.push({ kind: 'local', len: String(value).length });
    return origSet.call(this, key, value);
  };
  const origPut = IDBObjectStore.prototype.put;
  IDBObjectStore.prototype.put = function put(value, key) {
    if (mine(key)) window.__saveWrites.push({ kind: 'idb', len: typeof value === 'string' ? value.length : -1 });
    return origPut.apply(this, arguments);
  };
}

const autosave = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.autosaveState()));
const warning = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine.secretWarning;
const writes = (page) => page.evaluate(() => window.__saveWrites.length);
const noticeText = (page) => page.$eval(NOTICE, (el) => el.textContent ?? '');

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.context.addInitScript(installRecorder, HISTORY_PREFIX);
  const logs = [];
  opened.page.on('console', (m) => logs.push(m.text()));
  opened.page.on('pageerror', (e) => logs.push(String(e && e.message)));
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return { ...opened, logs };
}

const typeValue = (page, value) => page.evaluate(([sel, v]) => {
  const ta = document.querySelector(sel);
  ta.value = v;
  ta.dispatchEvent(new Event('input', { bubbles: true }));
  return performance.now();
}, [INPUT, value]);

async function settle(page) {
  await page.waitForFunction(() => JSON.parse(window.__fandheTest.autosaveState()).scheduled === false, null, { timeout: 5000 });
  await page.waitForTimeout(400);
}

// 値が結果・ログに出ていないこと。メッセージには試験データの ID だけを入れる。
function assertNoLeak(id, texts, value) {
  for (const t of texts) assert.ok(!t.includes(value), `${id}: value leaked`);
}

for (const via of ['draft', 'submit']) {
  test(`10 形: ${via} の経路で警告に形の名前が出て、値は出ず、履歴は書かれない`, async (t) => {
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      for (const s of secretShapes()) {
        const { page, context, logs } = await open(srv);
        const t0 = await typeValue(page, s.value);
        assert.deepEqual(await warning(page), [], `${s.id}: warning before the save`);
        if (via === 'submit') await page.click(SUBMIT);
        await page.waitForFunction((sel) => (document.querySelector(sel)?.textContent ?? '') !== '', NOTICE, { timeout: 5000 });
        const shownMs = await page.evaluate((t00) => performance.now() - t00, t0);
        t.diagnostic(`[secret-warning] ${JSON.stringify({ via, id: s.id, shownMs: Math.round(shownMs) })}`);
        await settle(page);
        assert.deepEqual(await warning(page), [s.expect], `${s.id}: warning shapes`);
        const text = await noticeText(page);
        assert.ok(text.includes(LABELS[s.expect]), `${s.id}: name missing`);
        assertNoLeak(s.id, [text], s.value);
        assert.equal(await writes(page), 0, `${s.id}: history key written`);
        // 件数は遅延保存と離れる直前の止めだけを数える。送信の即時の止めはホストが行い、数えない。
        if (via === 'draft') assert.ok((await autosave(page)).secretHolds >= 1, `${s.id}: hold not counted`);
        assertNoLeak(s.id, logs, s.value);
        await context.close();
      }
    });
  });
}

test('対照 N-01〜N-06: 警告が出ず、履歴は書かれる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const cs = controls();
    assert.equal(cs.length, 6);
    for (const c of cs) {
      const { page, context } = await open(srv);
      await typeValue(page, c.text);
      await settle(page);
      assert.deepEqual(await warning(page), [], `${c.id}: false positive`);
      assert.equal(await noticeText(page), '', `${c.id}: notice not empty`);
      assert.ok((await writes(page)) >= 1, `${c.id}: no history write`);
      assert.equal((await autosave(page)).secretHolds, 0, `${c.id}: hold counted`);
      await context.close();
    }
  });
});
