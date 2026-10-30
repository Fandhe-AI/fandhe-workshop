// 「保存した履歴を削除する」の確認の表示が、配信する設定の画面で DOM だけの観察で出ることの試験（SHELL-38・D-75。TASK-539 1/2）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。試験用の口の無い配信する設定（IR_FILE つき・TEST_HOOKS なし）で、
// 実際のボタンの操作と localStorage の読み取りだけで確かめる。
// 入力: FANDHE_WEB_AUTHOR_DIR・FANDHE_AUTHOR_IR。未設定・不在は skip にせず失敗にする。
// 確かめること（CSP あり・なし。マウスの click と、キーボード（focus して Enter）の 2 通り）:
//   実操作の後に保存先へ履歴が書かれた状態から「保存した履歴を削除する」を押すと、削除の確認の通知がちょうど 1 件見え、
//   文に「この教材の保存済みの履歴」と「書き出し済みのファイルは消えません」があり、書き出しを促す語が無く、
//   「削除する」「やめる」が 1 個ずつ見えて Tab で両方へ届く。確認の前に保存先の鍵の集合と値のハッシュが変わらない
//   （押した直後と、遅延保存の間隔を超えて待った後）。幅 360px で成果物のタブを選んでも通知とボタンが見える。
//   確認待ちのまま再読み込みすると確認は出ず、保存先が不変で保存した位置から復元される。
//   外部通信・CSP 違反・ページのエラー・ダイアログ・ポップアップが 0 件。
// 範囲外: 「削除する」「やめる」を押した後の画面と保存先（TASK-539 2/2）。
// 失敗の文・出力には、件数・真偽・ハッシュの先頭だけを出す（履歴の中身は出さない）。

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import { stepOrder, inspectDom, domObserver } from './l2-journey.mjs';
import { mouseDriver } from './ch0-3-journey.mjs';

const BASE = '/fandhe-delete-confirm/';
const PREFIX = 'fandhe-workshop/history/';
const BOOT_MARK = 'fandhe-workshop/meta/boot/';
const T = 30000;
const NOTICE = '[data-testid="shell-notice-item"][data-notice-kind="delete-confirm"]';
const DELETE = '[data-testid="shell-delete"]';
const CONFIRM = '[data-testid="shell-delete-confirm"]';
const CANCEL = '[data-testid="shell-delete-cancel"]';

let browser;
let webDir;
let order;
let first;

before(async () => {
  webDir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(webDir && fs.statSync(webDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS');
  const irFile = process.env.FANDHE_AUTHOR_IR;
  assert.ok(irFile && fs.statSync(irFile, { throwIfNoEntry: false })?.isFile(), 'FANDHE_AUTHOR_IR must point to the compiled IR');
  order = stepOrder(fs.readFileSync(irFile, 'utf8'));
  [first] = order;
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const sha = (s) => crypto.createHash('sha256').update(s).digest('hex');

/** 履歴の鍵（起動の印を除く）と値の長さ・ハッシュの対を鍵で整列して返す。 */
async function historyEntries(page) {
  const raw = await page.evaluate(([prefix, boot]) => {
    const out = [];
    for (let i = 0; i < localStorage.length; i += 1) {
      const k = localStorage.key(i);
      if (k !== null && k.startsWith(prefix) && !k.startsWith(boot)) out.push([k, localStorage.getItem(k) ?? '']);
    }
    return out;
  }, [PREFIX, BOOT_MARK]);
  return raw.map(([k, v]) => ({ key: k, len: v.length, hash: sha(v) })).sort((a, b) => (a.key < b.key ? -1 : 1));
}

function assertQuiet(rec, label) {
  const s = summarize(rec);
  assert.equal(s.crashed, 0, `${label}: tab must not crash`);
  assert.equal(s.external, 0, `${label}: no external request`);
  assert.equal(s.csp, 0, `${label}: no CSP violation`);
  assert.equal(s.pageErrors, 0, `${label}: no page error`);
  assert.equal(s.dialogs, 0, `${label}: no dialog`);
  assert.equal(s.popups, 0, `${label}: no popup`);
}

/** 実操作を 2 回行い、保存の値が落ち着くまで待つ。 */
async function saveHistory(page, srv) {
  await page.goto(srv.url);
  await waitMounted(page);
  const d = mouseDriver(page);
  await d.choose('opt-right');
  await domObserver(order).at(page, first, 'accepted');
  await d.point('cards/notice');
  await page.waitForFunction(([prefix, boot]) => {
    for (let i = 0; i < localStorage.length; i += 1) {
      const key = localStorage.key(i) ?? '';
      if (key.startsWith(prefix) && !key.startsWith(boot)) return true;
    }
    return false;
  }, [PREFIX, BOOT_MARK], { timeout: T });
  let prev = '';
  for (let n = 0; n < 50; n += 1) {
    const cur = JSON.stringify(await historyEntries(page));
    if (cur === prev) break;
    prev = cur;
    await page.waitForTimeout(100);
  }
  await page.waitForTimeout(1500);
}

const sameEntries = (a, b) => JSON.stringify(a) === JSON.stringify(b);

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  for (const mode of ['click', 'keyboard']) {
    test(`削除の確認: ${mode} で押すと確認が 1 件出て、確認の前に保存先が変わらない（CSP ${label}）`, async () => {
      await withCspMode({ dir: webDir, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const opened = await openPage(browser, { origin: srv.origin });
        const { page } = opened;
        await saveHistory(page, srv);
        const saved = await historyEntries(page);
        assert.ok(saved.length >= 1, 'a history key was saved');
        assert.equal(await page.locator(NOTICE).count(), 0, 'no confirm before the request');
        assert.equal(await page.locator(DELETE).count(), 1, 'one delete button');

        if (mode === 'click') {
          await page.locator(DELETE).click();
        } else {
          await page.locator(DELETE).focus();
          await page.keyboard.press('Enter');
        }
        await page.locator(NOTICE).waitFor({ state: 'visible', timeout: T });
        assert.equal(await page.locator(NOTICE).count(), 1, 'exactly one confirm');
        const text = await page.locator(`${NOTICE} [data-testid="shell-notice-text"]`).innerText();
        assert.ok(text.includes('この教材の保存済みの履歴'), 'names the target');
        assert.ok(text.includes('書き出し済みのファイルは消えません'), 'says exported files stay');
        assert.ok(!text.includes('書き出してください'), 'does not urge exporting');
        assert.equal(await page.locator(CONFIRM).count(), 1);
        assert.equal(await page.locator(CANCEL).count(), 1);
        assert.ok(await page.locator(CONFIRM).isVisible() && await page.locator(CANCEL).isVisible(), 'both buttons visible');

        assert.ok(sameEntries(saved, await historyEntries(page)), 'storage unchanged right after the request');
        await page.waitForTimeout(2000);
        assert.ok(sameEntries(saved, await historyEntries(page)), 'storage unchanged after waiting');

        // Tab で「削除する」「やめる」へ届く。
        await page.locator(DELETE).focus();
        const reached = new Set();
        for (let i = 0; i < 40 && reached.size < 2; i += 1) {
          await page.keyboard.press('Tab');
          const id = await page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? '');
          if (id === 'shell-delete-confirm' || id === 'shell-delete-cancel') reached.add(id);
        }
        assert.equal(reached.size, 2, 'Tab reaches both confirm buttons');

        // 確認待ちは保存されない。再読み込みでは確認が出ず、保存先が不変である。
        await page.reload();
        await waitMounted(page);
        assert.equal(await page.locator(NOTICE).count(), 0, 'confirm is not restored');
        assert.ok(sameEntries(saved, await historyEntries(page)), 'storage unchanged after reload');
        assertQuiet(opened.rec, 'run');
      }));
    });
  }

  test(`削除の確認: 幅 360px で成果物のタブを選んでも通知と両ボタンが見える（CSP ${label}）`, async () => {
    await withCspMode({ dir: webDir, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const opened = await openPage(browser, { origin: srv.origin });
      const { page } = opened;
      await saveHistory(page, srv);
      const saved = await historyEntries(page);
      await page.setViewportSize({ width: 360, height: 800 });
      await page.reload();
      await waitMounted(page);
      await page.locator(DELETE).click();
      await page.locator(NOTICE).waitFor({ state: 'visible', timeout: T });
      const tab = page.locator('[role="tab"]').first();
      if (await tab.count()) await tab.click();
      assert.ok(await page.locator(NOTICE).isVisible(), 'notice visible at 360px');
      assert.ok(await page.locator(CONFIRM).isVisible() && await page.locator(CANCEL).isVisible(), 'both buttons visible at 360px');
      assert.ok(sameEntries(saved, await historyEntries(page)), 'storage unchanged');
      assertQuiet(opened.rec, 'narrow');
    }));
  });
}
