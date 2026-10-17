// 承認の操作のログ文言と、CH-2 相当の完走の試験（TERM-27。TASK-447 2/2。spec の ch-run.mjs の CH-2 節に相当）。
//
// 本 repo には CH-2 の教材が無い。ST-2.6 と同じ構造（対象つきの「直す」でしか通れない承認の Step）を、試験の設定だけの
// `load-fixture:approval` の `st-1` が再現する。続く `st-2`（選択）・`st-3`（最終の承認）まで通し、最後の Step の達成を確かめる。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい画面を開く）:
//   - 対象なしの「直す」は拒否になり、承認の行の文字が `承認の操作: 直す`（`line-target` なし）になる
//   - 対象つきの「直す」を押すと承認の行の文字が `承認の操作: 直す（対象: t-alpha）`、`line-target` の文字が `t-alpha` になる
//   - 確認の段の「はい」で次の Step（st-2）へ進み、行の文字が `承認の操作: はい` になる
//   - st-2 の選択のあと st-3 に進み、「はい」を 2 回で st-3 が達成になる。3 Step すべてが達成で、承認のボタンが 0 件
//   - 危険な要素・実行・CSP 違反・ダイアログが 0 件である
//
// 確かめないこと:
//   - st-2 の入力方式 `choice` の画面の口は TASK-450 の担当で未実装のため、`raw-trigger` へ `Op` の JSON を送る
//   - 章の完了（`ChapterCompleted`）は core が未実装（TASK-233 以降）のため、最後の Step の達成までを確かめる
//   - `step-status` の表示は confirm-status.test.mjs（TASK-435 2/2）が確かめる
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-approval-flow/';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const REVIEW = '[data-testid="part-review-list"]';
const OFFERS = '[data-bind-list="offer_choices"]';
const BTN = '[data-testid="approval-btn"]';
const APPROVAL_ROW = `${LOG} > li[data-kind="received"][data-input="approval"]`;
const T = 30000;

let browser;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 文字列としてだけ渡し、コードの文字列を組み立てない。
async function sendRaw(page, payload) {
  await page.evaluate(([sel, p]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', p);
    el.click();
  }, [RAW, payload]);
}

// 1 回の読みで DOM とエンジンの書き出しを揃える（進行のタイマーが間に入らない）。
function snapshot(page) {
  return page.evaluate(([log, review, offers, btn, approvalRow]) => {
    const engine = JSON.parse(window.__fandheTest.stateJson()).engine;
    const ul = document.querySelector(offers);
    return {
      engine,
      rejected: document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length,
      approvalRows: [...document.querySelectorAll(approvalRow)].map((li) => ({
        text: li.textContent,
        decision: li.querySelector('[data-testid="line-decision"]')?.dataset.decision ?? null,
        decisionText: li.querySelector('[data-testid="line-decision"]')?.textContent ?? null,
        target: li.querySelector('[data-testid="line-target"]')?.textContent ?? null,
      })),
      buttons: [...document.querySelectorAll(btn)].map((b) => ({ text: b.textContent, target: b.dataset.reviseTarget ?? null })),
      ulHidden: ul ? ul.hasAttribute('hidden') : null,
      reviewAchieved: [...document.querySelectorAll(`${review} > li`)]
        .filter((li) => li.dataset.achieved === 'true').map((li) => li.dataset.step),
    };
  }, [LOG, REVIEW, OFFERS, BTN, APPROVAL_ROW]);
}

const currentIs = (page, step) => page.waitForFunction(
  (s) => JSON.parse(window.__fandheTest.stateJson()).engine.current === s, step, { timeout: T },
);

async function runFlow(page) {
  await waitMounted(page);
  await sendRaw(page, 'load-fixture:approval');
  await page.waitForSelector(BTN, { timeout: T });
  let s = await snapshot(page);
  assert.equal(s.engine.current, 'main/st-1');
  assert.equal(s.buttons.length, 5, JSON.stringify(s.buttons));
  const rejectedBefore = s.rejected;

  // 対象なしの「直す」: 実クリック。拒否になり、承認の行は対象なしの文言になる。
  await page.getByRole('button', { name: '直す', exact: true }).click();
  await page.waitForFunction(([log, n]) => document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length > n,
    [LOG, rejectedBefore], { timeout: T });
  s = await snapshot(page);
  assert.equal(s.rejected, rejectedBefore + 1);
  assert.deepEqual(s.approvalRows.map((r) => r.text), ['承認の操作: 直す']);
  assert.equal(s.approvalRows[0].target, null);
  assert.equal(s.approvalRows[0].decision, 'revise');
  assert.equal(s.approvalRows[0].decisionText, '直す');

  // 対象つきの「直す」: 実クリック。受付になり、進行の後に確認の段の 3 件へ切り替わる。
  await page.getByRole('button', { name: '直す（対象: t-alpha）' }).click();
  await page.waitForFunction(([btn]) => document.querySelectorAll(btn).length === 3
    && !document.querySelector(`${btn}[data-revise-target]`), [BTN], { timeout: T });
  s = await snapshot(page);
  assert.equal(s.rejected, rejectedBefore + 1, '対象つきの直すで拒否の行は増えない');
  assert.deepEqual(s.approvalRows.map((r) => r.text), ['承認の操作: 直す', '承認の操作: 直す（対象: t-alpha）']);
  assert.equal(s.approvalRows[1].target, 't-alpha');
  assert.equal(s.approvalRows[1].decision, 'revise');
  const lastReceived = s.engine.received[s.engine.received.length - 1];
  assert.deepEqual({ decision: lastReceived?.input?.approval?.decision, target: lastReceived?.input?.approval?.target }, { decision: 'revise', target: 't-alpha' },
    JSON.stringify(s.engine.received));
  assert.equal(s.engine.current, 'main/st-1');

  // 確認の「はい」: 先へ進む。
  await page.getByRole('button', { name: 'はい', exact: true }).click();
  await currentIs(page, 'main/st-2');
  s = await snapshot(page);
  assert.equal(s.approvalRows[s.approvalRows.length - 1].text, '承認の操作: はい');
  assert.equal(s.approvalRows[s.approvalRows.length - 1].decision, 'approve');
  assert.ok(s.reviewAchieved.includes('main/st-1'), JSON.stringify(s.reviewAchieved));

  // st-2（選択）: 画面の口が無いため Op で送る（TASK-450）。
  await sendRaw(page, JSON.stringify({ input: { choice: 'opt-memo' } }));
  await currentIs(page, 'main/st-3');

  // st-3（最終の承認）: 「はい」で受付、進行の後の「はい」で達成。
  await page.waitForSelector(BTN, { timeout: T });
  s = await snapshot(page);
  assert.equal(s.buttons.length, 3, JSON.stringify(s.buttons));
  await page.getByRole('button', { name: 'はい', exact: true }).click();
  await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.stateWithoutLog
    .run.slices.main.status['main/st-3'] === 'accepted', null, { timeout: T });
  await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.runStatus === 'idle', null, { timeout: T });
  await page.getByRole('button', { name: 'はい', exact: true }).click();
  await page.waitForFunction(([btn, offers]) => document.querySelectorAll(btn).length === 0
    && document.querySelector(offers)?.hasAttribute('hidden'), [BTN, OFFERS], { timeout: T });

  // 最終の照合。
  s = await snapshot(page);
  assert.deepEqual(s.reviewAchieved, ['main/st-1', 'main/st-2', 'main/st-3']);
  assert.equal(s.engine.log.filter((r) => r.kind === 'achieved').length, 3, JSON.stringify(s.engine.log));
  assert.equal(s.rejected, rejectedBefore + 1);
  assert.deepEqual(s.approvalRows.map((r) => r.text), [
    '承認の操作: 直す',
    '承認の操作: 直す（対象: t-alpha）',
    '承認の操作: はい',
    '承認の操作: はい',
    '承認の操作: はい',
  ]);
  assert.equal(s.engine.runStatus, 'idle');
}

for (const csp of [true, false]) {
  test(`承認の操作のログ文言と CH-2 相当の完走（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.goto(srv.url);
      await runFlow(page);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
      await context.close();
    }));
  });
}
