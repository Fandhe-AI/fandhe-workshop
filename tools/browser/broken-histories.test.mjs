// 壊れた履歴 7 種の拒否のブラウザでの確認（SAVE-20・SHELL-16。TASK-332 1/2。#880）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。履歴と基準の教材は core の試験
// （crates/core/tests/broken_histories.rs の emit_broken_histories_for_browser）が書き出し、FANDHE_BROKEN_HISTORIES_DIR で受ける。
// 期待の表は testdata/broken-histories/expectations.json を直接読み、定数（7 種と理由コード）と照合する。
//
// 確かめること（CSP あり・なしの両方、B-0〜B-7 と境界の B-4b を 1 件ずつ新しい context で）:
//   - 開かない履歴（B-1〜B-7、B-4b）: 結果が refused で理由が表と一致し、表示だけにならず（viewOnly は null）、実行環境の差し替えは 0 回、
//     状態・ログ・世代・入力欄の値・保存先の値が読み込みの前と同じで、通知欄の外の画面（DOM）も同じ。
//     枠の通知欄に import-refused の通知が 1 件出て、理由のコードが一致し、本文が空でない
//   - 対照の B-0: 開く（検出器が「開く」を観測できること）。通知は出ない
//   - 外部への要求・CSP 違反・ページのエラー・dialog・popup が 0 件
// 失敗の文には ID・件数・真偽・固定のコードだけを出す（履歴の中身を出さない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-broken-histories/';
const FILE_INPUT = '[data-testid="history-file"]';
const ITEM = '[data-testid="shell-notice-item"][data-notice-kind="import-refused"]';
const INPUT = '[data-testid="draft-input"]';
const OPERATION = '/make-note second-note';
const PACK = 'min-3step';
const START = 's-1';
const TABLE_PATH = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'testdata', 'broken-histories', 'expectations.json');

const REAL_INPUT = '[data-testid="shell-import"] input[type="file"]';
// 理由ごとの固定の文の断片（仮置き。TASK-532 1/2・2/2）。受講者が 7 種（B-4b を含む）の違いを読めることの確認に使う。
const FRAGMENT = {
  'B-1': '書き換えられている',
  'B-2': '途中で切れて',
  'B-3': 'JSON として読めません',
  'B-4': '10MB の上限',
  'B-4b': '決められた形と合いません',
  'B-5': '版の欄がありません',
  'B-6': '別の教材で記録された',
  'B-7': '決められた形と合いません',
};

const IDS = ['B-0', 'B-1', 'B-2', 'B-3', 'B-4', 'B-4b', 'B-5', 'B-6', 'B-7'];
const REFUSED = {
  'B-1': 'replay_mismatch',
  'B-2': 'truncated',
  'B-3': 'not_json',
  'B-4': 'too_large',
  'B-4b': 'malformed',
  'B-5': 'missing_version',
  'B-6': 'other_pack',
  'B-7': 'malformed',
};

let browser;
let historiesDir;
let table;

before(async () => {
  for (const name of ['FANDHE_WEB_TEST_DIR', 'FANDHE_BROKEN_HISTORIES_DIR']) {
    const dir = process.env[name];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a directory`);
  }
  historiesDir = process.env.FANDHE_BROKEN_HISTORIES_DIR;
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

// 状態・ログ・世代の指紋（真偽の比較にだけ使い、内容は出さない）。
const fingerprint = (e) => JSON.stringify({ s: e.stateWithoutLog, l: e.log, g: e.generation, c: e.current });

// 通知欄の外の画面の指紋。通知欄の部品を除いた body の innerHTML（真偽の比較にだけ使う）。
const screenOutsideNotice = (page) => page.evaluate(() => {
  const copy = document.body.cloneNode(true);
  for (const el of copy.querySelectorAll('[data-testid="shell-notice"], [data-testid="shell-notice-item"]')) el.remove();
  return copy.innerHTML;
});

async function openStorage(page) {
  await call(page, 'storageOpen');
  await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [PACK, START], { timeout: 10000 });
}

test('表の ID・理由コードが定数と一致する', () => {
  assert.deepEqual(table.rows.map((r) => r.id), IDS);
  const refused = Object.fromEntries(table.rows.filter((r) => r.expect.result === 'refused').map((r) => [r.id, r.expect.code]));
  assert.deepEqual(refused, REFUSED);
  assert.deepEqual(table.rows.filter((r) => r.expect.result === 'open').map((r) => r.id), ['B-0']);
});

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  for (const real of [false, true]) for (const id of IDS) {
    if (real && !FRAGMENT[id] && id !== 'B-0') continue;
    test(`壊れた履歴 ${id} の読み込み（${real ? '本物の読み込み欄' : '試験の口'}・CSP ${label}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const row = table.rows.find((r) => r.id === id);
        const opened = await openPage(browser, { origin: srv.origin });
        const { page } = opened;
        await page.goto(srv.url);
        await waitMounted(page);
        await settle(page);
        const irText = fs.readFileSync(path.join(historiesDir, 'base.ir.json'), 'utf8');
        const n0 = (await engine(page)).installCount;
        assert.equal(await call(page, 'loadIr', irText), true, `${id}: load-ir sent`);
        await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
        await settle(page);
        assert.equal(await page.locator(ITEM).count(), 0, `${id}: no notice before the import`);

        // 1 回操作して保存先に値を作り、読み込みの前後で比べる。
        await openStorage(page);
        await page.locator(INPUT).fill(OPERATION);
        await page.locator(INPUT).press('Enter');
        await settle(page);
        // 下書きの変更も遅延保存を起こしうるため、下書きを作ってから待つ。
        await page.locator(INPUT).fill('keep');
        // 遅延保存（debounce）が済むまで待つ。待たないと、読み込みの後に操作の保存が着地して「保存先が変わった」ように見える。
        await page.waitForFunction(() => JSON.parse(window.__fandheTest.autosaveState()).scheduled === false, null, { timeout: 15000 });
        await page.waitForFunction(([p, s]) => window.__fandheTest.storageRead(p, s) != null, [PACK, START], { timeout: 10000 });
        const storedBefore = await call(page, 'storageRead', PACK, START);
        assert.equal(typeof storedBefore, 'string', `${id}: a stored value exists`);
        const fpBefore = fingerprint(await engine(page));
        const installBefore = (await engine(page)).installCount;

        if (!real) await call(page, 'importInstall');
        // 読み込みの口（importInstall）が足す部品を含めた後の画面を、読み込みの前の基準にする。
        const screenBefore = await screenOutsideNotice(page);
        await page.setInputFiles(real ? REAL_INPUT : FILE_INPUT, path.join(historiesDir, `${id}.history.json`));
        await page.waitForFunction(() => {
          const e = JSON.parse(window.__fandheTest.stateJson()).engine;
          return e && e.lastImport != null;
        }, null, { timeout: 15000 });
        await settle(page);
        const e = await engine(page);

        if (row.expect.result === 'open') {
          assert.equal(['open', 'converted'].includes(e.lastImport.verdict), true, `${id}: the control opens`);
          assert.equal(await page.locator(ITEM).count(), 0, `${id}: no refused notice for the control`);
        } else {
          assert.equal(e.lastImport.verdict, 'refused', `${id}: verdict`);
          assert.equal(e.lastImport.reason, row.expect.code, `${id}: reason matches the table`);
          assert.equal(e.viewOnly, null, `${id}: not view-only`);
          assert.equal(e.installCount, installBefore, `${id}: no environment was built`);
          assert.equal(fingerprint(e) === fpBefore, true, `${id}: state unchanged`);
          assert.equal(await page.locator(INPUT).inputValue(), 'keep', `${id}: draft unchanged`);
          assert.equal(await page.locator(ITEM).count(), 1, `${id}: one refused notice`);
          assert.equal(await page.locator(ITEM).getAttribute('data-notice-reason'), e.lastImport.reason, `${id}: notice reason`);
          assert.equal((await page.locator(ITEM).textContent()).trim().length > 0, true, `${id}: notice text is not empty`);
          if (FRAGMENT[id]) {
            const body = (await page.locator(ITEM).textContent());
            assert.equal(body.includes(FRAGMENT[id]), true, `${id}: notice carries the reason text`);
            assert.equal(/min-3step|other-tutorial|"ops"|\{/.test(body), false, `${id}: notice carries no history value`);
          }
          assert.equal((await call(page, 'storageRead', PACK, START)) === storedBefore, true, `${id}: stored value untouched`);
          assert.equal((await screenOutsideNotice(page)) === screenBefore, true, `${id}: screen outside the notice unchanged`);
        }
        assertQuiet(opened.rec, id);
        await opened.context.close();
      }));
    });
  }
}
