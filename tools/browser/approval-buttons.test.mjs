// 承認のボタンと対象つきの「直す」の試験（TERM-27。TASK-447 1/2）。
//
// 試験の設定（TEST_HOOKS=1）の画面へ、見えない `raw-trigger` の固定の接頭辞 `load-fixture:approval` で、入力方式が
// `approval` の Step（対象 `t-alpha`・`t-beta` の「直す」の規則つき）の教材へ差し替える（試験の設定だけ。配信物には入らない）。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい画面を開く）:
//   - 入力待ちで承認のボタンが 5 件（はい・いいえ・直す ＋ 対象つき 2 件）出る。対象つきは `data-revise-target` と
//     文字 `直す（対象: …）` が規則の対象と一致する
//   - 対象なしの「直す」を Playwright の実クリックで押すと、`rejected` の行が 1 件増え、理由と次の行動がエンジンの値と
//     一致して次の行動が空でない。押した後もボタンは 5 件残る
//   - 対象つきの「直す」を押すと拒否の行は増えず、進行が終わると確認の段の 3 件（対象つきなし）に切り替わる
//   - 危険な要素・実行・CSP 違反・ダイアログが 0 件である
// ログの文言「承認の操作: …」と「はい」で先へ進むこと・CH-2 の完走は TASK-447 2/2 が確かめる。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-approval-buttons/';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const OFFERS = '[data-bind-list="offer_choices"]';
const BTN = '[data-testid="approval-btn"]';
const TARGETS = ['t-alpha', 't-beta'];
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

// 1 回の読みで DOM とエンジンの書き出しを揃える（進行のタイマーが間に入らない）。
function snapshot(page) {
  return page.evaluate(([log, offers, btn]) => {
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
        text: b.textContent, decision: b.dataset.decision, target: b.dataset.reviseTarget ?? null,
        action: b.dataset.action, payload: b.dataset.payload,
      })),
      ulHidden: ul ? ul.hasAttribute('hidden') : null,
    };
  }, [LOG, OFFERS, BTN]);
}

async function runFlow(page) {
  await waitMounted(page);
  await sendRaw(page, 'load-fixture:approval');
  await page.waitForSelector(BTN, { timeout: 30000 });

  let s = await snapshot(page);
  assert.equal(s.engine.current, 'main/st-1');
  assert.equal(s.buttons.length, 3 + TARGETS.length, JSON.stringify(s.buttons));
  assert.deepEqual(s.buttons.slice(0, 3).map((b) => b.text), ['はい', 'いいえ', '直す']);
  assert.deepEqual(s.buttons.slice(0, 3).map((b) => b.decision), ['approve', 'reject', 'revise']);
  assert.deepEqual(s.buttons.slice(0, 3).map((b) => b.target), [null, null, null]);
  const targeted = s.buttons.slice(3);
  assert.deepEqual(targeted.map((b) => b.target), TARGETS);
  assert.deepEqual(targeted.map((b) => b.payload), TARGETS);
  assert.deepEqual(targeted.map((b) => b.text), TARGETS.map((t) => `直す（対象: ${t}）`));
  assert.deepEqual(targeted.map((b) => b.action), ['revise-target', 'revise-target']);
  assert.equal(s.ulHidden, false);
  const rejectedBefore = s.rows.length;

  // 対象なしの「直す」: 実クリック。拒否になる（判定は core）。
  await page.getByRole('button', { name: '直す', exact: true }).click();
  await page.waitForFunction(([log, n]) => document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length > n, [LOG, rejectedBefore]);
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
  assert.ok(row.texts[1].trim().length > 0, '次の行動が空でない');
  assert.equal(s.buttons.length, 3 + TARGETS.length, '拒否の後もボタンが残る');
  assert.equal(s.engine.current, 'main/st-1');

  // 対象つきの「直す」: 受付になり、進行の後に確認の段の 3 件へ切り替わる。
  await page.getByRole('button', { name: `直す（対象: ${TARGETS[0]}）` }).click();
  await page.waitForFunction(([btn]) => document.querySelectorAll(btn).length === 3
    && !document.querySelector(`${btn}[data-revise-target]`), [BTN]);
  s = await snapshot(page);
  assert.equal(s.rows.length, rejectedBefore + 1, '対象つきの直すで拒否の行は増えない');
  assert.deepEqual(s.buttons.map((b) => b.text), ['はい', 'いいえ', '直す']);
  assert.equal(s.engine.runStatus, 'idle', JSON.stringify(s.engine.runStatus));
}

for (const csp of [true, false]) {
  test(`承認のボタンと対象つきの直すが出て 対象なしは拒否になる（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
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
