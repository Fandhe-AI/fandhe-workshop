// 開かない履歴の動作のブラウザでの確認（VER-8・SHELL-16。TASK-176 2/2。#733）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。履歴と開く側の教材は version-histories と同じ
// （FANDHE_VERSION_HISTORIES_DIR。表は testdata/version-histories/expectations.json を直接読む）。
//
// 確かめること（CSP あり・なしの両方）:
//   - 読み込み（表の result が refused の 3 行）: 結果が refused で、表示だけにならず（viewOnly は null）、実行環境の差し替えは 0 回、
//     状態・ログ・世代・入力欄の値・保存先の値が読み込みの前と同じ。枠の通知欄（shell-notice）に import-refused の通知が 1 件出て、
//     理由のコードが結果と一致し、文にコードが入る。読み込みの後は操作が効き、閉じるボタンで通知が消える
//   - 起動時の復元（保存先に ir_format の新しい履歴を置いて再読み込み）: 通知が 1 件出るが閉じるボタンは無く、表示だけにならず、
//     操作の後も保存先の値は書いた値のまま
//   - 外部への要求・CSP 違反・ページのエラー・dialog・popup が 0 件
// 通知の文は crates/web/src/shell_notice.rs（仮置き。最終の文面は TASK-549・TASK-534）。
// 失敗の文には行 ID・件数・真偽・固定のコードだけを出す（履歴の中身を出さない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-refused-history/';
const FILE_INPUT = '[data-testid="history-file"]';
const ITEM = '[data-testid="shell-notice-item"][data-notice-kind="import-refused"]';
const DISMISS = `${ITEM} [data-testid="shell-dismiss"]`;
const INPUT = '[data-testid="draft-input"]';
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

// 状態・ログ・世代の指紋（真偽の比較にだけ使い、内容は出さない）。
const fingerprint = (e) => JSON.stringify({ s: e.stateWithoutLog, l: e.log, g: e.generation, c: e.current });

async function openStorage(page) {
  await call(page, 'storageOpen');
  await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [PACK, START], { timeout: 10000 });
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  for (const id of ['V-iv-2', 'V-iv-3', 'V-v-2']) {
    test(`読み込みで開かないと理由が出て、状態も保存先も変わらない ${id}（CSP ${label}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const row = table.rows.find((r) => r.id === id);
        assert.equal(row.expect.result, 'refused', `${id}: table result`);
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
        assert.equal(await page.locator(ITEM).count(), 0, `${id}: no notice before the import`);

        // 1 回操作して保存先に値を作り、読み込みの前後で比べる。
        await openStorage(page);
        await page.locator(INPUT).fill(OPERATION);
        await page.locator(INPUT).press('Enter');
        await settle(page);
        await page.waitForFunction(([p, s]) => window.__fandheTest.storageRead(p, s) != null, [PACK, START], { timeout: 10000 });
        const storedBefore = await call(page, 'storageRead', PACK, START);
        assert.equal(typeof storedBefore, 'string', `${id}: a stored value exists`);
        await page.locator(INPUT).fill('keep');
        const fpBefore = fingerprint(await engine(page));
        const installBefore = (await engine(page)).installCount;

        await call(page, 'importInstall');
        await page.setInputFiles(FILE_INPUT, path.join(historiesDir, `${id}.history.json`));
        await page.waitForFunction(() => {
          const e = JSON.parse(window.__fandheTest.stateJson()).engine;
          return e && e.lastImport != null && e.lastImport.verdict === 'refused';
        }, null, { timeout: 15000 });
        await settle(page);
        const e = await engine(page);
        assert.equal(e.lastImport.verdict, 'refused', `${id}: verdict`);
        assert.equal(e.lastImport.reason, row.expect.code, `${id}: reason matches the table`);
        assert.equal(e.viewOnly, null, `${id}: not view-only`);
        assert.equal(e.installCount, installBefore, `${id}: no environment was built`);
        assert.equal(fingerprint(e) === fpBefore, true, `${id}: state unchanged`);
        assert.equal(await page.locator(INPUT).inputValue(), 'keep', `${id}: draft unchanged`);
        assert.equal(await page.locator(ITEM).count(), 1, `${id}: one refused notice`);
        assert.equal(await page.locator(ITEM).getAttribute('data-notice-reason'), e.lastImport.reason, `${id}: notice reason`);
        assert.equal((await page.locator(ITEM).textContent()).includes(e.lastImport.reason), true, `${id}: notice text carries the code`);
        assert.equal((await call(page, 'storageRead', PACK, START)) === storedBefore, true, `${id}: stored value untouched`);

        // 操作が効き、閉じるボタンで通知だけが消える。
        await page.locator(INPUT).fill(OPERATION);
        await page.locator(INPUT).press('Enter');
        await settle(page);
        assert.equal(fingerprint(await engine(page)) !== fpBefore, true, `${id}: operation still works`);
        await page.locator(DISMISS).click();
        assert.equal(await page.locator(ITEM).count(), 0, `${id}: notice dismissed`);
        assertQuiet(opened.rec, id);
        await opened.context.close();
      }));
    });
  }

  test(`起動時の復元で版の判定の段に開かないと閉じられない通知が出て、保存先の値は変わらない（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const opened = await openPage(browser, { origin: srv.origin });
      const { page } = opened;
      await page.goto(srv.url);
      await waitMounted(page);
      await settle(page);
      const current = JSON.parse(await call(page, 'loadableHistoryText'));
      const stored = { ...current, versions: { ...current.versions, ir_format: current.versions.ir_format + 1 } };
      const storedText = JSON.stringify(stored);
      await openStorage(page);
      assert.equal(JSON.parse(await call(page, 'storageWrite', PACK, START, storedText)).ok, true, 'stored history written');

      await page.reload();
      await waitMounted(page);
      await settle(page);
      const e = await engine(page);
      assert.equal(e.bootKind, 'not_opened', 'boot did not open the history');
      assert.equal(e.viewOnly, null, 'not view-only');
      assert.equal(await page.locator(ITEM).count(), 1, 'one refused notice');
      assert.equal(await page.locator(ITEM).getAttribute('data-notice-reason'), 'format_newer', 'notice reason');
      assert.equal(await page.locator(DISMISS).count(), 0, 'no dismiss button at boot');

      await page.locator(INPUT).fill(OPERATION);
      await page.locator(INPUT).press('Enter');
      await settle(page);
      await page.waitForTimeout(1500);
      await openStorage(page);
      assert.equal((await call(page, 'storageRead', PACK, START)) === storedText, true, 'the stored value is untouched');
      assert.equal(await page.locator(ITEM).count(), 1, 'notice stays');
      assertQuiet(opened.rec, 'boot');
      await opened.context.close();
    }));
  });
}
