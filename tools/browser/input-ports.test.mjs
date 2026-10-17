// 入力方式ごとの入力の口の切り替え（TASK-450 1/3。TERM-23）のブラウザ試験。
//
// 8 Step の教材（`?fixture=reload-points`）の各 Step に入った直後に、方式に合う口だけが出て他の口が出ないことを
// 数える。切り替わるのは Step ごとに変わる口（選択肢のボタン・承認のボタン）で、入力欄・送信は教材全体の設定
// （`input_field`。TASK-450 2/3）が決めるため全 Step で出ている。入力方式 `choice` の段では選択肢のボタンを実クリックして進む。
// 続けて `load-fixture:ui-action` に差し替え、入力方式 `ui_action` の Step で左の行を実クリックして達成になることを確かめる。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい画面を開く）:
//   - command・text・ui_action の Step に選択肢と承認のボタンが 0 件、choice の Step に宣言の数の選択肢（表示名で出て、ID は文字にならない）
//   - approval の Step に承認のボタン（3 件、対象つきの「直す」がある Step は 4 件）、選択肢が 0 件
//   - 危険な要素・実行・CSP 違反・ダイアログが 0 件である
//
// 確かめないこと:
//   - 入力欄を出さない設定（`input_field: hidden`）の表示は TASK-450 2/3、CH-0〜CH-3 の通しとキーボードだけでの完走は TASK-450 3/3
//   - 確認の point_at の行のクリックは keyboard-confirm.test.mjs と reload-points.test.mjs が受け持つため、ここでは `raw-trigger` で送る
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-input-ports/';
const QUERY = '?fixture=reload-points';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const RAW = '[data-testid="raw-trigger"]';
const CHOICE = '[data-testid="choice-btn"]';
const APPROVAL = '[data-testid="approval-btn"]';
const T = 30000;

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
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

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;

// 現在の Step が `uid` で、入力待ちで、処理中でないのを待つ。
const entered = (page, uid) => page.waitForFunction(([u]) => {
  const e = JSON.parse(window.__fandheTest.stateJson()).engine;
  return e.current === u && (e.stateWithoutLog.run.slices.main?.status?.[u] ?? 'entered') === 'entered'
    && window.__fandheTest.nextTick() === 'null';
}, [uid], { timeout: T });

// 確認待ち（受付済み）で、処理中でないのを待つ。
const accepted = (page, uid) => page.waitForFunction(([u]) => {
  const e = JSON.parse(window.__fandheTest.stateJson()).engine;
  return e.current === u && e.stateWithoutLog.run.slices.main?.status?.[u] === 'accepted'
    && window.__fandheTest.nextTick() === 'null';
}, [uid], { timeout: T });

const settled = (page) => page.waitForFunction(() => window.__fandheTest.nextTick() === 'null', null, { timeout: T });

// 4 つの口の数を (選択肢, 承認, 入力欄, 送信) の順で返す。
const ports = (page) => page.evaluate(([c, a, i, s]) => [c, a, i, s].map((sel) => document.querySelectorAll(sel).length),
  [CHOICE, APPROVAL, INPUT, SUBMIT]);

const uiOp = (n) => JSON.stringify({ input: { ui: { view: 'v-files', path: `notes/n${n}` } } });

async function check(page, uid, expected) {
  await entered(page, uid);
  assert.deepEqual(await ports(page), expected, uid);
}

async function runPorts(page) {
  await waitMounted(page);
  const yes = () => page.getByRole('button', { name: 'はい', exact: true }).click();

  // st-1（command）: 選択肢と承認が 0 件。入力欄から送って、確認の point_at を raw で指す。
  await check(page, 'main/st-1', [0, 0, 1, 1]);
  await page.fill(INPUT, '/make-note note-1');
  await page.click(SUBMIT);
  await accepted(page, 'main/st-1');
  await sendRaw(page, uiOp(1));

  // st-2（text）: 0 件。送って、確認の answer を選択肢のボタンで答える。
  await check(page, 'main/st-2', [0, 0, 1, 1]);
  await page.fill(INPUT, 'メモ 2');
  await page.click(SUBMIT);
  await accepted(page, 'main/st-2');
  await page.click(`${CHOICE}[data-choice="opt-a"]`);

  // st-3（approval）: 承認のボタン 3 件、選択肢 0 件。
  await check(page, 'main/st-3', [0, 3, 1, 1]);
  await yes();
  await accepted(page, 'main/st-3');
  await sendRaw(page, uiOp(3));

  // st-4・st-5（choice）: 宣言の 2 件が表示名で出る。ID は文字にならない。
  await check(page, 'main/st-4', [2, 0, 1, 1]);
  const labels = await page.$$eval(CHOICE, (bs) => bs.map((b) => ({ text: b.textContent, id: b.dataset.choice })));
  assert.deepEqual(labels, [{ text: '案 A', id: 'opt-a' }, { text: '案 B', id: 'opt-b' }]);
  await page.click(`${CHOICE}[data-choice="opt-a"]`);
  await check(page, 'main/st-5', [2, 0, 1, 1]);
  await page.click(`${CHOICE}[data-choice="opt-a"]`);
  await accepted(page, 'main/st-5');
  assert.equal((await ports(page))[0], 0, '確認待ちでは入力の選択肢が消える');
  await sendRaw(page, uiOp(5));

  // st-6（approval。対象つきの「直す」あり）: 4 件。
  await check(page, 'main/st-6', [0, 4, 1, 1]);
  await page.getByRole('button', { name: '直す（対象: expect）', exact: true }).click();
  await accepted(page, 'main/st-6');
  await yes();

  // st-7（ui_action）: 選択肢・承認とも 0 件。対象の行は入った時点では左に無く、raw で指す。
  await check(page, 'main/st-7', [0, 0, 1, 1]);
  await sendRaw(page, uiOp(7));
  await accepted(page, 'main/st-7');
  await page.click(`${CHOICE}[data-choice="opt-a"]`);

  // st-8（approval）: 3 件。
  await check(page, 'main/st-8', [0, 3, 1, 1]);
  await settled(page);
  assert.equal((await engine(page)).current, 'main/st-8');
}

async function runUiAction(page) {
  await waitMounted(page);
  await sendRaw(page, 'load-fixture:ui-action');
  await entered(page, 'main/st-1');
  // st-1 を終えて st-2（ui_action）へ。確認の point_at は左の行を実クリックして進む。
  await page.fill(INPUT, '/make-note first-note');
  await page.click(SUBMIT);
  await accepted(page, 'main/st-1');
  await page.click('[data-row="notes/draft"]', { timeout: T });
  await entered(page, 'main/st-2');
  assert.deepEqual(await ports(page), [0, 0, 1, 1], 'ui_action の Step に選択肢・承認は出ない');
  // 画面操作: 宣言の対象の行を実クリックすると受付・達成になり、次の Step へ進む。
  await page.click('[data-row="notes/draft"]', { timeout: T });
  await entered(page, 'main/st-3');
  const eng = await engine(page);
  assert.equal(eng.stateWithoutLog.run.slices.main.status['main/st-2'], 'achieved');
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  test(`入力方式ごとに入力の口が切り替わる（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.goto(`${srv.url}${QUERY}`);
      await runPorts(page);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
      await context.close();
    }));
  });

  test(`ui_action の Step は左の行のクリックで受付になる（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.goto(srv.url);
      await runUiAction(page);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
      await context.close();
    }));
  });
}
