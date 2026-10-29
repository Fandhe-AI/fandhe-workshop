// `ops` の 22 か所の書き換えのブラウザでの確認（SAVE-20・SHELL-16。TASK-332 2/2。#881）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。履歴と基準の教材は core の試験
// （crates/core/tests/ops_rewrites.rs の emit_ops_rewrites_for_browser）が書き出し、FANDHE_OPS_REWRITES_DIR で受ける。
// 期待の表は testdata/broken-histories/ops-rewrites.json を直接読み、定数（22 件・拒否 21・同一値の位置）と照合する。
//
// 確かめること（CSP あり・なしの両方、W-00〜W-22 を 1 件ずつ新しい context で）:
//   - 拒否（21 件）: 結果が refused で理由が replay_mismatch、表示だけにならず、実行環境の差し替えは 0 回、
//     状態・ログ・世代・入力欄の値・保存先の値・通知欄の外の画面が読み込みの前と同じ。import-refused の通知が 1 件出る
//   - 同一値（1 件）と対照 W-00: 開き、開いた状態が履歴ファイルの snapshot と完全に一致する。拒否の通知は出ない
//   - 外部への要求・CSP 違反・ページのエラー・dialog・popup が 0 件
// 失敗の文には ID・件数・真偽・固定のコードだけを出す（履歴の中身を出さない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { isDeepStrictEqual } from 'node:util';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-ops-rewrites/';
const FILE_INPUT = '[data-testid="history-file"]';
const ITEM = '[data-testid="shell-notice-item"][data-notice-kind="import-refused"]';
const INPUT = '[data-testid="draft-input"]';
const OPERATION = '/make-note second-note';
const PACK = 'min-3step';
const START = 's-1';
const TABLE_PATH = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'testdata', 'broken-histories', 'ops-rewrites.json');

const OPS = 22;
const REFUSED_COUNT = 21;
const SAME_VALUE_AT = 15;
const CODE = 'replay_mismatch';
const idOf = (i) => `W-${String(i + 1).padStart(2, '0')}`;
const IDS = ['W-00', ...Array.from({ length: OPS }, (_, i) => idOf(i))];

let browser;
let historiesDir;
let table;

before(async () => {
  for (const name of ['FANDHE_WEB_TEST_DIR', 'FANDHE_OPS_REWRITES_DIR']) {
    const dir = process.env[name];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a directory`);
  }
  historiesDir = process.env.FANDHE_OPS_REWRITES_DIR;
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

test('表の件数・分類・同一値の位置が定数と一致する', () => {
  assert.equal(table.table_version, 1);
  assert.equal(table.base_ops, OPS);
  assert.deepEqual(table.rows.map((r) => r.index), Array.from({ length: OPS }, (_, i) => i));
  const refused = table.rows.filter((r) => r.expect.result === 'refused');
  assert.equal(refused.length, REFUSED_COUNT);
  assert.equal(refused.every((r) => r.expect.code === CODE), true);
  assert.deepEqual(table.rows.filter((r) => r.expect.result === 'same_value').map((r) => r.index), [SAME_VALUE_AT]);
});

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  for (const id of IDS) {
    test(`書き換えた履歴 ${id} の読み込み（CSP ${label}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const row = id === 'W-00' ? { expect: { result: 'same_value' } } : table.rows[Number(id.slice(2)) - 1];
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

        await call(page, 'importInstall');
        // 読み込みの口（importInstall）が足す部品を含めた後の画面を、読み込みの前の基準にする。
        const screenBefore = await screenOutsideNotice(page);
        await page.setInputFiles(FILE_INPUT, path.join(historiesDir, `${id}.history.json`));
        await page.waitForFunction(() => {
          const e = JSON.parse(window.__fandheTest.stateJson()).engine;
          return e && e.lastImport != null;
        }, null, { timeout: 15000 });
        await settle(page);
        const e = await engine(page);

        if (row.expect.result === 'same_value') {
          // 開いた直後の写しと履歴ファイルの snapshot の完全な一致（誤った状態で開かないこと）。
          assert.equal(e.lastImport.verdict, 'open', `${id}: opens without conversion`);
          const snapshot = JSON.parse(fs.readFileSync(path.join(historiesDir, `${id}.history.json`), 'utf8')).snapshot;
          const restored = JSON.parse(await call(page, 'lastImportStateJson'));
          assert.equal(isDeepStrictEqual(restored, snapshot), true, `${id}: opened state equals the snapshot`);
          assert.equal(await page.locator(ITEM).count(), 0, `${id}: no refused notice`);
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
          assert.equal((await call(page, 'storageRead', PACK, START)) === storedBefore, true, `${id}: stored value untouched`);
          assert.equal((await screenOutsideNotice(page)) === screenBefore, true, `${id}: screen outside the notice unchanged`);
        }
        assertQuiet(opened.rec, id);
        await opened.context.close();
      }));
    });
  }
}
