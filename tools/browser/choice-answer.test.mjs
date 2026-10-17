// 選択肢の誤答 → 選び直し → 達成の流れの試験（TERM-24。TASK-445 2/2）。
//
// 試験の設定（TEST_HOOKS=1）の画面は、mount の教材（min-3step。st-1 の確認は `point_at`）で始まる。`answer` の Step が
// 無いため、見えない `raw-trigger` へ固定の接頭辞 `load-fixture:answer` を書いて click し、st-1 の確認を選択肢 2 件
// （誤り・正解）の `answer` に置き換えた教材へ差し替える（試験の設定だけ。配信物には入らない）。
// 続けて `Op` の JSON で受付を送り（reject-lines と同じく決定的にする）、進行が終わって確認待ちになるまで待つ。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい画面を開く）:
//   - 確認待ちで選択肢のボタンが 2 件出る。文字は表示名で、ID は文字に出ず、`data-choice` にだけある
//   - 誤った選択肢を Playwright の実クリックで押すと、`rejected` の行が 1 件増え、理由と次の行動がエンジンの値と一致する
//   - 誤答の後もボタンが 2 件残り、復習に達成が無い（選び直せる）
//   - 正しい選択肢をキーボード（フォーカスして Enter）で押すと達成になる: 復習の st-1 が `data-achieved="true"`、
//     エンジンの書き出しの `log` に `achieved` が出て `current` が st-1 から進み、answer の選択肢が消えて次の Step（入力方式 `choice`。TASK-450 1/3）の選択肢 2 件（opt-memo・opt-todo）に替わる
//   - 拒否の行は正答で増えない。危険な要素・実行・CSP 違反・ダイアログが 0 件である
// Step の状態表示（`step-status` の「達成」）は confirm-status.test.mjs（TASK-435 2/2）が確かめるため、ここでは確かめない。
// 達成は上の観察点で確かめる。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-choice-answer/';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const REVIEW = '[data-testid="part-review-list"]';
const OFFERS = '[data-bind-list="offer_choices"]';
const BTN = '[data-testid="choice-btn"]';
const WRONG = { id: 'opt-wrong', label: '誤った案' };
const RIGHT = { id: 'opt-right', label: '正しい案' };
const REASON = '公開しない場合は、この Step を先へ進めません。';
const NEXT = '公開してよいかをもう一度決めます。';

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

const engineOf = (page) => page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine);

// 1 回の読みで DOM とエンジンの書き出しを揃える（進行のタイマーが間に入らない）。
function snapshot(page) {
  return page.evaluate(([log, review, offers, btn]) => {
    const engine = JSON.parse(window.__fandheTest.stateJson()).engine;
    const rows = [...document.querySelectorAll(`${log} > li[data-kind="rejected"]`)].map((li) => ({
      event: Number(li.dataset.event),
      texts: [...li.children].map((c) => c.textContent),
      children: [...li.children].map((c) => c.dataset.testid),
    }));
    const ul = document.querySelector(offers);
    return {
      engine,
      rows,
      buttons: [...document.querySelectorAll(btn)].map((b) => ({
        text: b.textContent, choice: b.dataset.choice, action: b.dataset.action, payload: b.dataset.payload,
      })),
      ulHidden: ul ? ul.hasAttribute('hidden') : null,
      reviewAchieved: [...document.querySelectorAll(`${review} > li`)]
        .filter((li) => li.dataset.achieved === 'true').map((li) => li.dataset.step),
    };
  }, [LOG, REVIEW, OFFERS, BTN]);
}

async function waitFor(page, predicateSource, arg) {
  await page.waitForFunction(predicateSource, arg);
}

async function runFlow(page) {
  await waitMounted(page);
  await sendRaw(page, 'load-fixture:answer');
  await waitFor(page, () => JSON.parse(window.__fandheTest.stateJson()).engine.current === 'main/st-1');
  await sendRaw(page, JSON.stringify({ input: { command: '/make-note first-note' } }));
  // 受付の後、進行が終わって選択肢が出るまで待つ。
  await page.waitForSelector(BTN, { timeout: 30000 });

  let s = await snapshot(page);
  assert.equal(s.engine.runStatus, 'idle', JSON.stringify(s.engine.runStatus));
  assert.equal(s.engine.current, 'main/st-1');
  assert.deepEqual(s.buttons.map((b) => b.text), [WRONG.label, RIGHT.label]);
  assert.deepEqual(s.buttons.map((b) => b.choice), [WRONG.id, RIGHT.id]);
  assert.deepEqual(s.buttons.map((b) => b.payload), [WRONG.id, RIGHT.id]);
  assert.deepEqual(s.buttons.map((b) => b.action), ['choose', 'choose']);
  for (const b of s.buttons) assert.ok(!b.text.includes(WRONG.id) && !b.text.includes(RIGHT.id), b.text);
  assert.equal(s.ulHidden, false);
  assert.deepEqual(s.reviewAchieved, []);
  const rejectedBefore = s.rows.length;

  // 誤答: 実クリック。
  await page.getByRole('button', { name: WRONG.label }).click();
  await waitFor(page, ([log, n]) => document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length > n, [LOG, rejectedBefore]);
  s = await snapshot(page);
  assert.equal(s.rows.length, rejectedBefore + 1, JSON.stringify(s.rows));
  const row = s.rows[s.rows.length - 1];
  assert.deepEqual(row.children, ['line-reason', 'line-next-action']);
  const judged = s.engine.judgedCategories.find((j) => j.event === row.event);
  assert.ok(judged, JSON.stringify(s.engine.judgedCategories));
  assert.equal(row.texts[0], judged.reason);
  assert.equal(row.texts[1], judged.nextAction);
  assert.equal(row.texts[0], REASON);
  assert.equal(row.texts[1], NEXT);
  assert.equal(s.buttons.length, 2, '誤答の後もボタンが残る');
  assert.deepEqual(s.reviewAchieved, []);
  assert.equal(s.engine.current, 'main/st-1');

  // 選び直し: 正答。キーボード（フォーカスして Enter）で押す。
  await page.getByRole('button', { name: RIGHT.label }).focus();
  await page.keyboard.press('Enter');
  await page.waitForFunction(([btn]) => [...document.querySelectorAll(btn)].every((b) => b.dataset.choice !== 'opt-right')
    && document.querySelectorAll(btn).length === 2, [BTN]);
  s = await snapshot(page);
  assert.deepEqual(s.reviewAchieved, ['main/st-1'], JSON.stringify(s.reviewAchieved));
  assert.ok(s.engine.log.some((r) => r.kind === 'achieved'), JSON.stringify(s.engine.log));
  assert.notEqual(s.engine.current, 'main/st-1');
  assert.deepEqual(s.buttons.map((b) => b.choice), ['opt-memo', 'opt-todo'], '次の Step の入力の選択肢に替わる');
  assert.equal(s.ulHidden, false);
  assert.equal(s.rows.length, rejectedBefore + 1, '正答で拒否の行は増えない');
}

for (const csp of [true, false]) {
  test(`誤答の理由が出て選び直せ 正答で達成になる（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
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
