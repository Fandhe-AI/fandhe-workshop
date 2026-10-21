// 拒否された入力が入力欄に残ることの試験（TASK-271。JUDGE-5・TERM-17）。
//
// 受け持ち: 拒否の 7 分類（unknown_command・wrong_order・missing_arg・bad_arg_shape・busy・not_in_this_step・not_supported）の
// それぞれで、入力欄から送った本文が、拒否の後も欄の値（DOM）と状態の下書き（__fandheTest.draft()）の両方で
// 1 文字も変わらないことを確かめる。送信は Enter と送信ボタンの click の両方（TERM-17 は経路を限らない）。
// 本番の保持の実装は host.rs の submit（keeps_draft）で、Rust 側の対は crates/web/tests/submit_wiring.rs。
//
// reject-lines との違い: reject-lines は見えない raw-trigger へ Op の JSON を書き、入力欄を通らない。
// この試験は入力欄に本文を入れて送る。そのため `choice` で作られている error_inputs の 2 件（e-busy・e-not-in-step）は
// 入力欄から送れず、入力欄の本文（試験の中の小さな対応表）に置き換える。cmd の 5 件は本文をそのまま使う。
// 自然文の複数行（前後の空白・空行を含む）の not_in_this_step も足し、trim されないことを確かめる。
// 実際に得た分類が error_inputs の 7 分類と一致することを最後に確かめる（分類が増減したら失敗。0 件のまま通さない）。
//
// busy の作り方: 受付になる本文を送り、run-status が busy になるのを待ってから、欄へ本文を入れて送る。
// 固定の sleep は使わない。窓（5 段 × 300ms）を外れたら分類が busy にならず失敗する（偶発的に通らない）。
// 処理が終わった後に、進行の描画・受付済みへの遷移で欄が書き換わっていないことも見る。
//
// 範囲外: 拒否の後の選択状態・フォーカスの位置（TASK-284 2/2・JUDGE-27・TERM-22。扱いは 2026-10-21 に確定した。D-67）。
// 問い返しの後の欄が空になることは clarify-draft.test.mjs が受け持つ（TASK-284 1/2）。
// 欄の値は読むだけで、DOM の値を状態へ流さない（BUILD-6）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { loadErrorInputs } from './error-inputs.mjs';

const BASE = '/fandhe-reject-keeps-draft/';
const STATUS = '[data-testid="run-status"]';
const DRAFT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const LOG = '[data-testid="part-log-list"]';
const REJECTED = `${LOG} li[data-kind="rejected"]`;

// 受付になる本文（busy の前段）。
const ACCEPTED = '/make-note first-note';

// 入力欄から送れない分類の本文。min-3step の開始点（s-1）で、その分類になる。
const SUBSTITUTE = {
  busy: '/make-note second-note',
  not_in_this_step: '/help',
};
const MULTILINE = 'メモを書く\n\n  二行目  ';

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(
    dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(),
    'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1',
  );
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 試験データから、入力欄の本文つきのケースを作る。対応表に無い分類は例外（fail-closed）。
function buildCases() {
  const cases = loadErrorInputs().map((c) => {
    const cmd = c.op.input.command;
    if (typeof cmd === 'string') return { name: c.id, category: c.category, body: cmd };
    const body = SUBSTITUTE[c.category];
    if (body === undefined) throw new Error(`${c.id}: no input-field body for category ${c.category}`);
    return { name: c.id, category: c.category, body };
  });
  cases.push({ name: 'multiline', category: 'not_in_this_step', body: MULTILINE });
  return cases;
}

const CASES = buildCases();

const snap = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson()));
const statusState = (page) => page.evaluate((sel) => document.querySelector(sel)?.getAttribute('data-state') ?? null, STATUS);
const stateDraft = (page) => page.evaluate(() => window.__fandheTest.draft());

async function waitUntil(what, read, pred, timeout = 8000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await read();
    if (await pred(last)) return last;
    await new Promise((r) => setTimeout(r, 10));
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

// 先に予約された 0 ミリ秒のタイマー（書き戻し）が走り終わるまで待つ。固定の sleep は使わない。
const settle = (page) => page.evaluate(() => new Promise((resolve) => { setTimeout(resolve, 0); }));

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.emulateMedia({ reducedMotion: 'no-preference' });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

async function send(page, how) {
  if (how === 'click') await page.click(SUBMIT);
  else {
    await page.focus(DRAFT);
    await page.keyboard.press('Enter');
  }
}

// 欄の値と状態の下書きが、どちらも送った本文のままであること。
async function assertKept(page, body, when) {
  await settle(page);
  await settle(page);
  assert.equal(await page.inputValue(DRAFT), body, `${when}: 欄の値が変わった`);
  assert.equal(await stateDraft(page), body, `${when}: 状態の下書きが変わった`);
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  for (const c of CASES) {
    for (const how of ['enter', 'click']) {
      test(`${c.category}（${c.name}）の拒否の後も入力欄の本文が変わらない: ${how}（CSP ${cspLabel}）`, async () => {
        await run(async (srv) => {
          const { page, context, rec } = await open(srv);
          if (c.category === 'busy') {
            await page.fill(DRAFT, ACCEPTED);
            await send(page, how);
            await waitUntil('draft emptied', () => page.inputValue(DRAFT), (v) => v === '');
            await waitUntil('busy', () => statusState(page), (s) => s === 'busy');
          }
          const before0 = await snap(page);
          await page.fill(DRAFT, c.body);
          await send(page, how);
          const e = await waitUntil(
            'judged',
            async () => (await snap(page)).engine,
            (x) => x.judgedCategories.length === before0.engine.judgedCategories.length + 1,
          );
          assert.equal(e.judgedCategories.at(-1).category, c.category, JSON.stringify(e.judgedCategories.at(-1)));
          await page.waitForSelector(REJECTED);
          await assertKept(page, c.body, '拒否の直後');
          if (c.category === 'busy') {
            // 処理が終わった後（進行の描画・受付済みへの遷移の後）も、欄は書き換わらない。
            await waitUntil('idle', () => statusState(page), (s) => s !== 'busy', 20000);
            await assertKept(page, c.body, '処理の終了後');
          }
          const safety = await collectSafety(page, rec);
          assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
          await context.close();
        });
      });
    }
  }

  // 網羅の確認: 試験が流す分類の集合が error_inputs の 7 分類と一致する。
  test(`試験が流す分類は拒否の 7 分類と一致する（CSP ${cspLabel}）`, () => {
    const got = [...new Set(CASES.map((c) => c.category))].sort();
    const want = [...new Set(loadErrorInputs().map((c) => c.category))].sort();
    assert.equal(want.length, 7, JSON.stringify(want));
    assert.deepEqual(got, want);
  });
}
