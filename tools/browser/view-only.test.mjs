// 表示だけで開いた履歴の動作のブラウザでの確認（VER-8。TASK-176 1/2。#732）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。履歴と開く側の教材は version-histories と同じ
// （FANDHE_VERSION_HISTORIES_DIR。表は testdata/version-histories/expectations.json を直接読む）。
//
// 確かめること（CSP あり・なしの両方）:
//   - 読み込み（V-iii-1。教材内容の主版違い）: 結果が view_only で `viewOnly` が理由のコードになり、通知欄（view-only-notice）に
//     コードを含む文が出る。実行環境の差し替えは 0 回のまま
//   - 入力（入力欄への打鍵・Enter・送信ボタン）・保存（raw の save）・停止（raw の stop・Esc）を送っても、
//     状態・ログ・世代・下書きが変わらず、入力欄の値も空のまま（打った文字が DOM に残らない）
//   - 起動時の復元（保存先に同じ主版違いの履歴を置いて再読み込み）でも同じ。保存先の値は書き換わらない
//   - 外部への要求・CSP 違反・ページのエラー・dialog・popup が 0 件
// 保存領域の不変と書き出しの一致の正式な試験は TASK-335（view-only-storage.test.mjs）である。ここは操作が何も変えないことを確かめる。
// 通知の文は crates/web/src/view_only_notice.rs（仮置き。最終の文面は TASK-549・TASK-534）。
// 失敗の文には行 ID・件数・真偽・固定のコードだけを出す（履歴の中身を出さない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-view-only/';
const FILE_INPUT = '[data-testid="history-file"]';
const RAW = '[data-testid="raw-trigger"]';
const NOTICE = '[data-testid="view-only-notice"]';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const OPERATION = '/make-note second-note';
const PACK = 'min-3step';
const START = 's-1';
const TABLE_PATH = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'testdata', 'version-histories', 'expectations.json');

let browser;
let historiesDir;
let table;

before(async () => {
  for (const name of ['FANDHE_WEB_DIR', 'FANDHE_WEB_TEST_DIR', 'FANDHE_VERSION_HISTORIES_DIR']) {
    const dir = process.env[name];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a directory`);
  }
  historiesDir = process.env.FANDHE_VERSION_HISTORIES_DIR;
  table = JSON.parse(fs.readFileSync(TABLE_PATH, 'utf8'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);
const engine = async (page) => JSON.parse(await call(page, 'stateJson')).engine;

async function settle(page) {
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null', null, { timeout: 15000 });
}

function assertQuiet(rec, label) {
  const s = summarize(rec);
  assert.equal(s.external, 0, `${label}: no external request`);
  assert.equal(s.csp, 0, `${label}: no CSP violation`);
  assert.equal(s.pageErrors, 0, `${label}: no page error`);
  assert.equal(s.dialogs, 0, `${label}: no dialog`);
  assert.equal(s.popups, 0, `${label}: no popup`);
}

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

// 状態・ログ・世代の指紋（真偽の比較にだけ使い、内容は出さない）。
const fingerprint = (e) => JSON.stringify({ s: e.stateWithoutLog, l: e.log, g: e.generation, c: e.current });

// 4 種の操作を送り、どれも状態・ログ・世代を変えず、入力欄の値が空のままであることを確かめる。
async function assertFrozen(page, label) {
  const before = fingerprint(await engine(page));
  const step = async (name, fn) => {
    await fn();
    await settle(page);
    assert.equal(fingerprint(await engine(page)) === before, true, `${label}: ${name} changed nothing`);
    assert.equal(await page.locator(INPUT).inputValue(), '', `${label}: ${name} left the input empty`);
  };
  await step('typing', async () => { await page.locator(INPUT).fill(OPERATION); });
  await step('enter', async () => { await page.locator(INPUT).press('Enter'); });
  await step('submit-click', async () => {
    await page.locator(INPUT).fill(OPERATION);
    await page.locator(SUBMIT).click({ force: true });
  });
  await step('esc', async () => { await page.locator(INPUT).press('Escape'); });
  await step('raw-save', async () => { await sendRaw(page, { control: 'save' }); });
  await step('raw-stop', async () => { await sendRaw(page, { control: 'stop' }); });
}

async function assertNoticeShown(page, label) {
  const e = await engine(page);
  assert.equal(typeof e.viewOnly, 'string', `${label}: viewOnly is a reason code`);
  assert.equal(await page.locator(NOTICE).count(), 1, `${label}: one view-only notice`);
  assert.equal(await page.locator(NOTICE).getAttribute('role'), 'status', `${label}: role=status`);
  const text = await page.locator(NOTICE).textContent();
  assert.equal(text.includes(e.viewOnly), true, `${label}: notice carries the reason code`);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`読み込みで表示だけになると理由が出て、操作は何も変えない（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const row = table.rows.find((r) => r.id === 'V-iii-1');
      assert.equal(row.expect.result, 'view_only', 'table result');
      const opened = await openPage(browser, { origin: srv.origin });
      const { page } = opened;
      await page.goto(srv.url);
      await waitMounted(page);
      await settle(page);
      const irText = fs.readFileSync(path.join(historiesDir, 'packs', `${row.opening.pack}.ir.json`), 'utf8');
      const n0 = (await engine(page)).installCount;
      assert.equal(await call(page, 'loadIr', irText), true, 'load-ir sent');
      await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
      assert.equal(await call(page, 'openingOverride', JSON.stringify({ engine: row.opening.engine, vocab_max: row.opening.vocab_max })), true, 'override sent');
      await settle(page);
      assert.equal(await page.locator(NOTICE).textContent(), '', 'no notice before the import');
      const installBefore = (await engine(page)).installCount;
      await call(page, 'importInstall');
      await page.setInputFiles(FILE_INPUT, path.join(historiesDir, 'V-iii-1.history.json'));
      await page.waitForFunction(() => {
        const e = JSON.parse(window.__fandheTest.stateJson()).engine;
        return e && e.lastImport != null && e.viewOnly != null;
      }, null, { timeout: 15000 });
      await settle(page);
      const e = await engine(page);
      assert.equal(e.lastImport.verdict, 'view_only');
      assert.equal(e.viewOnly, e.lastImport.reason, 'viewOnly is the import reason');
      assert.equal(e.installCount, installBefore, 'no environment was built');
      await assertNoticeShown(page, 'load');
      await assertFrozen(page, 'load');
      assertQuiet(opened.rec, 'load');
      await opened.context.close();
    }));
  });

  test(`起動時の復元でも表示だけになり、保存先の値は変わらない（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const opened = await openPage(browser, { origin: srv.origin });
      const { page } = opened;
      await page.goto(srv.url);
      await waitMounted(page);
      await settle(page);
      // 開ける包みの content の主版だけを上げた履歴を保存先へ置く（結果は view_only の content_major）。
      const current = JSON.parse(await call(page, 'loadableHistoryText'));
      const parts = current.versions.content.split('.').map(Number);
      parts[0] += 1;
      const stored = { ...current, versions: { ...current.versions, content: parts.join('.') } };
      const storedText = JSON.stringify(stored);
      await call(page, 'storageOpen');
      await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [PACK, START], { timeout: 10000 });
      assert.equal(JSON.parse(await call(page, 'storageWrite', PACK, START, storedText)).ok, true, 'stored history written');

      await page.reload();
      await waitMounted(page);
      await settle(page);
      const e = await engine(page);
      assert.equal(e.bootKind, 'not_opened', 'boot did not open the history');
      assert.equal(e.viewOnly, 'content_major');
      await assertNoticeShown(page, 'boot');
      await assertFrozen(page, 'boot');

      await call(page, 'storageOpen');
      await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [PACK, START], { timeout: 10000 });
      assert.equal((await call(page, 'storageRead', PACK, START)) === storedText, true, 'the stored value is untouched');
      assertQuiet(opened.rec, 'boot');
      await opened.context.close();
    }));
  });
}
