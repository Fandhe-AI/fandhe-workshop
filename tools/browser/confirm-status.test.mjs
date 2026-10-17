// 確認の 3 種の操作の後にだけ「達成」が出る表示の試験（TASK-435 2/2。TERM-31）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、入力の送信は見えない `raw-trigger` へ `Op` の JSON を送って決定的にし、
// 確認の操作は実際の UI（Playwright の実クリック）で行う。3 種の確認ごとに、送信 → 受付済み → 確認の操作 → 達成を確かめる。
//   CS-POINT:   mount の教材（min-3step）の st-1。左の行 notes/draft をクリックして指す（point_at）
//   CS-ANSWER:  `load-fixture:answer` の st-1。誤った選択肢の後に正しい選択肢を押す（answer）
//   CS-APPROVE: `load-fixture:approval` の st-1。対象なしの「直す」（拒否）・対象つきの「直す」（受付）の後に「はい」（approve）
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい画面を開く）:
//   - 送信と進行の後は「受付済み（確認待ち）」で、engine の導出（stateJson の stepStatus）は accepted
//   - 確認の操作の直前に印を記録へ入れ、印より前に「達成」が DOM の変化の記録に 0 件（MutationObserver が全件を記録する）
//   - 誤った選択肢・対象なしの「直す」で拒否になっても、語は受付済みのままか空で、「達成」は出ない
//   - 確認の操作の後に「達成」が出て、st-1 の achieved が 1 件・stepStatus が achieved・current が st-1 から進む
//   - 外部リクエスト・エラー・CSP 違反・ダイアログ・危険な要素が 0 件
//
// 確かめないこと:
//   - 停止の直後・`Entered` などの語・`processing` を持つ確認省略の Step の文言・配置は、step-status.test.mjs の限界・範囲外のとおり
//   - st-2 以降の通しは approval-flow.test.mjs の担当
//
// 待ちは固定の sleep ではなく、上限つきの待ちで書く。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-confirm-status/';
const RAW = '[data-testid="raw-trigger"]';
const STATUS = '[data-testid="step-status"]';
const ROW = '[data-testid="part-artifact-view"] li[data-row="notes/draft"]';
const LOG = '[data-testid="part-log-list"]';
const APPROVAL_BTN = '[data-testid="approval-btn"]';
const CHOICE_BTN = '[data-testid="choice-btn"]';
const MAKE_NOTE = JSON.stringify({ input: { command: '/make-note first-note' } });
const ACCEPTED = '受付済み（確認待ち）';
const ACHIEVED = '達成';
const MARK = '<<confirm>>';
const T = 30000;

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(),
    'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
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

const engine = (page) => page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine);
const achievedCount = (e, step) => e.log.filter((r) => r.kind === 'achieved' && r.step === step).length;
const wordOf = (page) => page.evaluate((sel) => document.querySelector(sel).textContent, STATUS);

// step-status の文字の変化をすべて記録する（一瞬だけ出る「達成」を取りこぼさないため）。
// MutationObserver は同じタスク内の変更をまとめて通知するため、現在の文字だけでなく、
// 各記録の oldValue と追加・削除されたノードの文字も記録する。
const observe = (page) => page.evaluate((sel) => {
  const el = document.querySelector(sel);
  window.__stepStatusSeen = [el.textContent];
  new MutationObserver((records) => {
    for (const r of records) {
      if (r.type === 'characterData' && r.oldValue !== null) window.__stepStatusSeen.push(r.oldValue);
      for (const n of r.removedNodes) window.__stepStatusSeen.push(n.textContent);
      for (const n of r.addedNodes) window.__stepStatusSeen.push(n.textContent);
    }
    window.__stepStatusSeen.push(document.querySelector(sel).textContent);
  }).observe(el, { characterData: true, characterDataOldValue: true, childList: true, subtree: true });
}, STATUS);
const mark = (page) => page.evaluate((m) => window.__stepStatusSeen.push(m), MARK);
const seen = (page) => page.evaluate(() => window.__stepStatusSeen);

const waitWord = (page, word) => page.waitForFunction(
  ([sel, w]) => document.querySelector(sel).textContent === w, [STATUS, word], { timeout: T },
);
// 受付後の進行が終わり（idle）、st-1 が受付済みで確認待ちになるまで待つ。
const waitAcceptedIdle = (page) => page.waitForFunction(() => {
  const e = JSON.parse(window.__fandheTest.stateJson()).engine;
  return e.runStatus === 'idle' && e.stateWithoutLog.run.slices.main.status['main/st-1'] === 'accepted';
}, null, { timeout: T });
const rejectedCount = (page) => page.evaluate((log) => document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length, LOG);
const waitRejectedOver = (page, n) => page.waitForFunction(
  ([log, k]) => document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length > k, [LOG, n], { timeout: T },
);

// 確認の前の検査。印より前に受付済みがあり、達成が 0 件であること。
async function expectBeforeConfirm(page, label) {
  assert.equal(await wordOf(page), ACCEPTED, label);
  const e = await engine(page);
  assert.equal(e.stepStatus, 'accepted', label);
  assert.equal(achievedCount(e, 'main/st-1'), 0, `${label}: 確認の前に達成が出ている`);
  assert.ok(!(await seen(page)).includes(ACHIEVED), `${label}: ${JSON.stringify(await seen(page))}`);
}

// 確認の後の検査。印より後に達成が出ていること。
async function expectAfterConfirm(page, { next }) {
  await waitWord(page, ACHIEVED);
  const e = await engine(page);
  assert.equal(e.stepStatus, 'achieved');
  assert.equal(achievedCount(e, 'main/st-1'), 1, JSON.stringify(e.log));
  if (next) assert.equal(e.current, next);
  else assert.notEqual(e.current, 'main/st-1');
  const s = await seen(page);
  const at = s.indexOf(MARK);
  assert.ok(at >= 0, JSON.stringify(s));
  assert.ok(s.slice(0, at).includes(ACCEPTED), `印より前に受付済みが無い: ${JSON.stringify(s)}`);
  assert.ok(!s.slice(0, at).includes(ACHIEVED), `印より前に達成が出ている: ${JSON.stringify(s)}`);
  assert.ok(s.slice(at + 1).includes(ACHIEVED), `印より後に達成が無い: ${JSON.stringify(s)}`);
}

async function csPoint(page) {
  assert.equal(await wordOf(page), '');
  assert.equal((await engine(page)).stepStatus, null);
  await observe(page);
  await sendRaw(page, MAKE_NOTE);
  await waitAcceptedIdle(page);
  await expectBeforeConfirm(page, 'point 送信後');
  await mark(page);
  await page.locator(ROW).click();
  await expectAfterConfirm(page, { next: 'main/st-2' });
}

async function csAnswer(page) {
  await sendRaw(page, 'load-fixture:answer');
  await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.current === 'main/st-1', null, { timeout: T });
  await observe(page);
  await sendRaw(page, MAKE_NOTE);
  await page.waitForSelector(CHOICE_BTN, { timeout: T });
  await waitAcceptedIdle(page);
  await expectBeforeConfirm(page, 'answer 送信後');

  const before = await rejectedCount(page);
  await page.getByRole('button', { name: '誤った案' }).click();
  await waitRejectedOver(page, before);
  await expectBeforeConfirm(page, 'answer 誤答後');

  await mark(page);
  await page.getByRole('button', { name: '正しい案' }).click();
  await expectAfterConfirm(page, {});
}

async function csApprove(page) {
  await sendRaw(page, 'load-fixture:approval');
  await page.waitForFunction((btn) => document.querySelectorAll(btn).length === 5, APPROVAL_BTN, { timeout: T });
  await observe(page);

  // 対象なしの「直す」は拒否になる。受付済みにも達成にもならない。
  const before = await rejectedCount(page);
  await page.getByRole('button', { name: '直す', exact: true }).click();
  await waitRejectedOver(page, before);
  assert.equal(await wordOf(page), '');
  const rejected = await engine(page);
  assert.equal(rejected.stepStatus, null);
  assert.equal(achievedCount(rejected, 'main/st-1'), 0);
  assert.ok(!(await seen(page)).includes(ACHIEVED));

  // 対象つきの「直す」で受付になり、進行の後に確認の段の 3 件へ切り替わる。
  await page.getByRole('button', { name: '直す（対象: t-alpha）' }).click();
  await page.waitForFunction((btn) => document.querySelectorAll(btn).length === 3
    && !document.querySelector(`${btn}[data-revise-target]`), APPROVAL_BTN, { timeout: T });
  await waitAcceptedIdle(page);
  await expectBeforeConfirm(page, 'approve 受付後');

  await mark(page);
  await page.getByRole('button', { name: 'はい', exact: true }).click();
  await expectAfterConfirm(page, { next: 'main/st-2' });
}

const CASES = [
  ['指す（point_at）', csPoint],
  ['答える（answer）', csAnswer],
  ['承認する（approve）', csApprove],
];

for (const [name, run] of CASES) {
  for (const csp of [true, false]) {
    test(`${name}: 確認の操作の前に達成が出ず、後に達成が出る（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        // 段階の表示を省かない文脈で確かめる。
        await page.emulateMedia({ reducedMotion: 'no-preference' });
        await page.goto(srv.url);
        await waitMounted(page);
        await run(page);
        const counts = await collectSafety(page, rec);
        assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
        await context.close();
      }));
    });
  }
}

// 負の対照: 同じタスクの中で「達成」を表示して戻しても、記録に残ること。
test('対照: 同一タスク内で達成を表示して戻しても記録に残る', async () => {
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.setContent('<p data-testid="step-status">受付済み（確認待ち）</p>');
  await observe(page);
  await page.evaluate((sel) => {
    const el = document.querySelector(sel);
    el.textContent = '達成';
    el.textContent = '受付済み（確認待ち）';
  }, STATUS);
  await page.evaluate(() => new Promise((r) => setTimeout(r, 0)));
  assert.ok((await seen(page)).includes(ACHIEVED), JSON.stringify(await seen(page)));
  await page.evaluate((sel) => {
    const t = document.querySelector(sel).firstChild;
    t.nodeValue = '達成';
    t.nodeValue = '受付済み（確認待ち）';
  }, STATUS);
  await page.evaluate(() => new Promise((r) => setTimeout(r, 0)));
  assert.ok((await seen(page)).filter((w) => w === ACHIEVED).length >= 2, JSON.stringify(await seen(page)));
  await context.close();
});
