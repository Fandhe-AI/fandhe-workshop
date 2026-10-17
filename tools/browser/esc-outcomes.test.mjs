// 変換中でない入力欄での Esc の画面の結果の試験（TASK-441 1/2・2/2。TERM-7・TERM-8。PoC-2 の K-09〜K-11・K-14）。
//
// 試験の設定（FANDHE_WEB_TEST_DIR。教材は min-3step.json）で、右のログ・状態表示・候補の一覧が揃った画面を見る。
//   (1) 処理中なら、候補の一覧が開いていても Esc は停止になり、候補の一覧も閉じる（下書きは残る）
//   (2) 処理中でなく候補の一覧が開いていれば（待機中・停止の後）、Esc は一覧を閉じるだけで、停止もログの増加も起きない
//   (3) どちらでもなければ取り消しの入力になる。試験の教材はどの Step も取り消しを受け付けないので、拒否
//       （not_in_this_step）の行と理由・次の行動が出る。世代と状態は変わらず、下書きは残る（TASK-441 2/2）
// 範囲外: Esc がちょうど 1 回届くことは TASK-353 の持ち分。(1)(2) のケースは Esc を 1 回押すところで止める。
// 待ちは固定の sleep ではなく、上限つきのポーリングで書く。prefers-reduced-motion は no-preference。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-esc-outcomes/';
const STATUS = '[data-testid="run-status"]';
const DRAFT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const LIST = '[data-testid="candidate-list"]';
const BTNS = '[data-testid="candidate-btn"]';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const PROGRESS = `${LOG} li[data-kind="progress"]`;
const STOPPED = `${LOG} li[data-kind="stopped"]`;
const REJECTED = `${LOG} li[data-kind="rejected"]`;
const BODY = '/make-note first-note';
const TOTAL_TICKS = 5;

// 取り消しを受け付けない Step の既定の拒否（defaults.not_in_this_step）の文面。IR から読み、期待を固定する。
const IR_PATH = fileURLToPath(new URL('../../testdata/engine-event-consistency/test-data/min-3step.json', import.meta.url));
const ir = JSON.parse(fs.readFileSync(IR_PATH, 'utf8'));
const defaultRejection = ir.rejections[ir.defaults.not_in_this_step];

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

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;

const statusOf = (page) => page.evaluate((sel) => {
  const els = document.querySelectorAll(sel);
  const el = els[0];
  return { count: els.length, state: el?.getAttribute('data-state') ?? null, word: el?.textContent ?? null };
}, STATUS);

const isHidden = (page) => page.locator(LIST).evaluate((el) => el.hasAttribute('hidden'));
const labels = (page) => page.locator(BTNS).allTextContents();

async function waitUntil(what, read, pred, timeout = 8000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await read();
    if (await pred(last)) return last;
    await new Promise((r) => setTimeout(r, 20));
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.emulateMedia({ reducedMotion: 'no-preference' });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

// Esc の直前の前提: 処理中で、途中（段が残っている）で、候補の一覧が開いている。競合で完了していたらここで失敗する。
async function assertBusyWithCandidates(page) {
  assert.equal((await statusOf(page)).state, 'busy', 'Esc の前に処理中でない');
  assert.equal(await isHidden(page), false, 'Esc の前に候補が閉じている');
  assert.ok((await page.locator(PROGRESS).count()) < TOTAL_TICKS, 'Esc の前に完了してしまった');
}

async function assertStoppedAndClosed(page, before0, draft) {
  const st = await waitUntil('stopped', () => statusOf(page), (s) => s.state === 'stopped');
  assert.deepEqual(st, { count: 1, state: 'stopped', word: '停止' });
  const e = await engine(page);
  assert.equal(e.runStatus, 'stopped');
  assert.equal(e.generation, before0.generation + 1);
  assert.equal(await page.locator(STOPPED).count(), 1);
  assert.equal(await isHidden(page), true, '停止と同時に候補の一覧が閉じる');
  assert.equal(await page.locator(BTNS).count(), 0);
  assert.equal(await page.inputValue(DRAFT), draft, '下書きは残る');
  // 古い世代の進行が Ignored として届いても、「停止」のまま・候補は閉じたまま。
  await waitUntil('stale tick', async () => (await engine(page)).ignoredReasons.length, (n) => n >= 1);
  assert.equal((await statusOf(page)).state, 'stopped');
  assert.equal(await isHidden(page), true);
}

// 取り消しの入力の拒否が 1 件、行として出ていることを確かめる。InputReceived は行にならないので logLen と行数は同一視しない。
async function assertCancelRejected(page, before0, draft) {
  const e = await waitUntil('cancel judged', () => engine(page), (x) => x.judgedCategories.length === before0.judgedCategories.length + 1);
  assert.equal(JSON.stringify(e.judgedCategories.at(-1)).includes('not_in_this_step'), true, JSON.stringify(e.judgedCategories.at(-1)));
  assert.equal(e.logLen, before0.logLen + 2);
  assert.equal(e.generation, before0.generation);
  assert.equal(await page.locator(REJECTED).count(), 1);
  const row = await page.evaluate((sel) => {
    const li = document.querySelector(sel);
    const kids = [...li.children].map((c) => ({
      id: c.getAttribute('data-testid'), nodes: c.childNodes.length, text: c.textContent, textNode: c.firstChild?.nodeType === 3,
    }));
    return { category: li.getAttribute('data-category'), kids };
  }, REJECTED);
  assert.equal(row.category, 'not_in_this_step');
  assert.deepEqual(row.kids.map((k) => k.id), ['line-reason', 'line-next-action']);
  for (const k of row.kids) {
    assert.equal(k.nodes, 1, `${k.id} は文字ノード 1 つ`);
    assert.equal(k.textNode, true);
    assert.ok(k.text.trim().length > 0);
  }
  assert.ok(defaultRejection, '既定の拒否が IR に無い');
  assert.equal(row.kids[0].text, defaultRejection.message);
  assert.equal(row.kids[1].text, defaultRejection.next_action);
  assert.equal(await page.locator(STOPPED).count(), 0);
  assert.equal(await page.inputValue(DRAFT), draft, '下書きは残る');
  assert.equal(await isHidden(page), true);
  return e;
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  test(`処理中に候補が開いていても Esc は停止になり候補も閉じる（K-11・K-14。送信は raw。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const before0 = await engine(page);
      await page.fill(DRAFT, '/he');
      assert.equal(await isHidden(page), false);
      assert.deepEqual(await labels(page), ['/help']);
      // 下書きと候補を残したまま、処理中の入力だけを core へ送る。
      await sendRaw(page, { input: { command: BODY } });
      await waitUntil('busy', () => statusOf(page), (s) => s.state === 'busy');
      await assertBusyWithCandidates(page);
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      await assertStoppedAndClosed(page, before0, '/he');
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });

  test(`送信の後の処理中に /he を打って Esc を押すと停止になり候補も閉じる（K-11。実 UI。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const before0 = await engine(page);
      await page.fill(DRAFT, BODY);
      await page.click(SUBMIT);
      await waitUntil('busy', () => statusOf(page), (s) => s.state === 'busy');
      await page.fill(DRAFT, '/he');
      await assertBusyWithCandidates(page);
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      await assertStoppedAndClosed(page, before0, '/he');
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });

  test(`待機中に候補が開いているときの Esc は閉じるだけで、状態もログも変えない（K-09。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const before0 = await engine(page);
      await page.fill(DRAFT, '/he');
      assert.equal(await isHidden(page), false);
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      assert.equal(await isHidden(page), true);
      assert.deepEqual(await statusOf(page), { count: 1, state: 'idle', word: '待機中' });
      const e = await engine(page);
      assert.equal(e.logLen, before0.logLen);
      assert.equal(e.generation, before0.generation);
      assert.equal(e.runStatus, 'idle');
      assert.equal(await page.locator(STOPPED).count(), 0);
      assert.equal(await page.inputValue(DRAFT), '/he');
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });

  test(`停止の後に候補が開いているときの Esc は閉じるだけで、停止を重ねない（K-09。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const before0 = await engine(page);
      await page.fill(DRAFT, '/he');
      await sendRaw(page, { input: { command: BODY } });
      await waitUntil('busy', () => statusOf(page), (s) => s.state === 'busy');
      await assertBusyWithCandidates(page);
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      await assertStoppedAndClosed(page, before0, '/he');
      // 下書きを変えて候補を再び開き、もう一度 Esc。処理中ではないので閉じるだけ。
      await page.fill(DRAFT, '/hel');
      assert.equal(await isHidden(page), false);
      const e1 = await engine(page);
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      assert.equal(await isHidden(page), true);
      assert.deepEqual(await statusOf(page), { count: 1, state: 'stopped', word: '停止' });
      const e2 = await engine(page);
      assert.equal(e2.logLen, e1.logLen);
      assert.equal(e2.generation, e1.generation);
      assert.equal(await page.locator(STOPPED).count(), 1);
      assert.equal(await page.inputValue(DRAFT), '/hel');
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });
  for (const draft of ['', 'hello']) {
    test(`待機中で候補が閉じているときの Esc は取り消しの入力になり、拒否の行と次の行動が出る（K-10・TERM-8。下書き ${JSON.stringify(draft)}。CSP ${cspLabel}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv);
        const before0 = await engine(page);
        await page.fill(DRAFT, draft);
        assert.equal(await isHidden(page), true);
        assert.equal((await statusOf(page)).state, 'idle');
        await page.focus(DRAFT);
        await page.keyboard.press('Escape');
        await assertCancelRejected(page, before0, draft);
        assert.deepEqual(await statusOf(page), { count: 1, state: 'idle', word: '待機中' });
        assert.equal((await engine(page)).runStatus, 'idle');
        const safety = await collectSafety(page, rec);
        assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
        await context.close();
      });
    });
  }

  test(`候補が開いているときの Esc は 1 回目で閉じ、2 回目で取り消しの入力になる（K-09・JUDGE-18。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const before0 = await engine(page);
      await page.fill(DRAFT, '/he');
      assert.equal(await isHidden(page), false);
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      assert.equal(await isHidden(page), true);
      assert.equal((await engine(page)).logLen, before0.logLen, '1 回目はログを変えない');
      assert.equal(await page.locator(REJECTED).count(), 0);
      await page.keyboard.press('Escape');
      await assertCancelRejected(page, before0, '/he');
      assert.deepEqual(await statusOf(page), { count: 1, state: 'idle', word: '待機中' });
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });

  test(`停止の後に候補が閉じているときの Esc は取り消しの入力になり、停止を重ねない（TERM-8。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const before0 = await engine(page);
      await sendRaw(page, { input: { command: BODY } });
      await waitUntil('busy', () => statusOf(page), (s) => s.state === 'busy');
      assert.ok((await page.locator(PROGRESS).count()) < TOTAL_TICKS, 'Esc の前に完了してしまった');
      await page.focus(DRAFT);
      await page.keyboard.press('Escape');
      await waitUntil('stopped', () => statusOf(page), (s) => s.state === 'stopped');
      await waitUntil('stale tick', async () => (await engine(page)).ignoredReasons.length, (n) => n >= 1);
      const stopped = await engine(page);
      assert.equal(stopped.generation, before0.generation + 1);
      assert.equal(await page.locator(STOPPED).count(), 1);
      await page.keyboard.press('Escape');
      const after = await waitUntil('cancel logged', () => engine(page), (x) => x.logLen === stopped.logLen + 2);
      assert.equal(after.generation, stopped.generation);
      assert.equal(await page.locator(STOPPED).count(), 1, '停止の行は 1 件のまま');
      assert.equal(await page.locator(REJECTED).count(), 1);
      assert.deepEqual(await statusOf(page), { count: 1, state: 'idle', word: '待機中' });
      assert.equal(after.runStatus, 'idle');
      assert.equal(await page.inputValue(DRAFT), '');
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });
}
