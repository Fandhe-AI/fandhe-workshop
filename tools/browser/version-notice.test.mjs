// 変換して開いたときの版の違いの通知のブラウザでの確認（SHELL-20。TASK-535 1/2。#729）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。履歴と開く側の教材は version-histories と同じ
// （FANDHE_VERSION_HISTORIES_DIR。表は testdata/version-histories/expectations.json を直接読む）。
// 履歴ファイルを読み込み、枠の通知欄（shell-notice）に通知が出るかを DOM で確かめる。
//
// 確かめること（CSP あり・なしの両方）:
//   - V-i-1（変換して開く）: 通知が 1 件出て文が仮置きの定数と一致し、幅 360px で成果物のタブを選んでも見えている。
//     閉じると 0 件になり、root の要素は同じ要素のままである
//   - V-0（変換なしで開く）・V-iii-1（表示だけ）: 通知は 0 件
//   - 外部への要求・CSP 違反・ページのエラー・dialog・popup が 0 件
// 文の定数は crates/web/src/shell_notice.rs の VERSION_CONVERTED_NOTICE（仮置き。最終の文面は TASK-549・TASK-534）。
// 失敗の文には行 ID・件数・真偽だけを出す（履歴の中身を出さない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-version-notice/';
const FILE_INPUT = '[data-testid="history-file"]';
const TABLE_PATH = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'testdata', 'version-histories', 'expectations.json');
const NOTICE_TEXT = '変換して開きました。教材の内容または版が、この履歴を書いたときから変わっています。';
const ITEM = '[data-testid="shell-notice-item"]';

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

// 行を開いたページを返す（開いた後の画面で通知を確かめる）。表の結果が変わったら前提が崩れたとして落とす。
async function openRow(srv, id, wantResult) {
  const row = table.rows.find((r) => r.id === id);
  assert.ok(row, `${id}: row exists`);
  assert.equal(row.expect.result, wantResult, `${id}: table result`);
  const opened = await openPage(browser, { origin: srv.origin });
  const { page } = opened;
  await page.goto(srv.url);
  await waitMounted(page);
  await settle(page);
  const irText = fs.readFileSync(path.join(historiesDir, 'packs', `${row.opening.pack}.ir.json`), 'utf8');
  const installBeforeLoad = (await engine(page)).installCount;
  assert.equal(await call(page, 'loadIr', irText), true, `${id}: load-ir sent`);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, installBeforeLoad, { timeout: 15000 });
  assert.equal(await call(page, 'openingOverride', JSON.stringify({ engine: row.opening.engine, vocab_max: row.opening.vocab_max })), true, `${id}: override sent`);
  await settle(page);
  await call(page, 'importInstall');
  await page.setInputFiles(FILE_INPUT, path.join(historiesDir, `${id}.history.json`));
  await page.waitForFunction(() => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    return e && e.lastImport != null;
  }, null, { timeout: 15000 });
  assert.equal((await engine(page)).lastImport.verdict, wantResult, `${id}: verdict`);
  await settle(page);
  return opened;
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`変換して開くと通知が 1 件出て、閉じると消える（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const opened = await openRow(srv, 'V-i-1', 'converted');
      const { page } = opened;
      assert.equal(await page.locator('[data-testid="shell-notice"]').count(), 1, 'one notice area');
      assert.equal(await page.locator('[data-testid="shell-notice"]').getAttribute('role'), 'status');
      assert.equal(await page.locator(ITEM).count(), 1, 'one notice item');
      assert.equal(await page.locator(ITEM).getAttribute('data-notice-kind'), 'version-converted');
      assert.equal(await page.locator(ITEM).isVisible(), true, 'notice visible');
      assert.equal(await page.locator('[data-testid="shell-notice-text"]').textContent(), NOTICE_TEXT);

      // 幅 960px 未満で成果物のタブを選んでも見える（tabpanel の外にある）。
      await page.setViewportSize({ width: 360, height: 800 });
      await page.locator('[role="tab"]').nth(1).click();
      assert.equal(await page.locator(ITEM).isVisible(), true, 'notice visible at narrow width');
      await page.setViewportSize({ width: 1024, height: 768 });

      // 閉じる。root は同じ要素のまま、読み込みの結果は変わらない。
      const root = await page.locator('[data-testid="workshop-root"]').elementHandle();
      const importBefore = JSON.stringify((await engine(page)).lastImport);
      await page.locator('[data-testid="shell-dismiss"]').click();
      await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 0, ITEM, { timeout: 15000 });
      const rootAfter = await page.locator('[data-testid="workshop-root"]').elementHandle();
      assert.equal(await page.evaluate(([a, b]) => a === b, [root, rootAfter]), true, 'root element is kept');
      assert.equal(JSON.stringify((await engine(page)).lastImport), importBefore, 'import result unchanged');
      assertQuiet(opened.rec, 'V-i-1');
      await opened.context.close();
    }));
  });

  test(`変換なしで開く・表示だけの履歴では通知が出ない（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      for (const [id, result] of [['V-0', 'open'], ['V-iii-1', 'view_only']]) {
        const opened = await openRow(srv, id, result);
        assert.equal(await opened.page.locator(ITEM).count(), 0, `${id}: no notice`);
        assertQuiet(opened.rec, id);
        await opened.context.close();
      }
    }));
  });
}
