// 入力欄なしの教材（`pack.profile.input_field: hidden`）の表示のブラウザ試験（TASK-450 2/3。PACK-20・TERM-23）。
//
// 試験の設定（TEST_HOOKS=1）の画面を `?fixture=no-input` で開くと、起動の前に入力欄なしの 3 Step の教材
// （pack `min-3step-no-input`。`crates/web/tests/data/input_field_hidden/ir.json`）が選ばれる。`load-fixture:` は mount の後の
// 差し替えで、入力欄の有無の構造の変化が束縛点の差分更新に乗らないため使わない。
// 入力方式は選択肢（st-1）・画面操作（st-2）・承認（st-3）、確認は指す（st-1）・答える（st-2）・承認する（st-3）で、
// すべて実クリックで進める（`raw-trigger` は使わない）。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい画面を開く）:
//   - 各段（入力待ち・確認待ち）で、入力欄・送信・補完の候補の一覧とボタン・`select`（OS の選択の置き場）が 0 件
//   - 各段で、停止ボタンが 1 件で見えている
//   - 選択肢・左の行・答えの選択肢・承認のボタン・確認の「はい」の実クリックで、最後まで進む
//   - 危険な要素・実行・CSP 違反・ダイアログが 0 件である
//   - 対照: クエリなしの既定の教材では、入力欄・送信・候補の一覧が 1 件ずつ出る
//
// 確かめないこと:
//   - CH-0〜CH-3 の通しとキーボードだけでの完走は TASK-450 3/3（#406）の担当
//   - キーの案内・OS の選択の部品そのものは未実装（TERM-38）。置き場の `select` 要素の 0 件だけを数える
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-input-field-hidden/';
const QUERY = '?fixture=no-input';
const CHOICE = '[data-testid="choice-btn"]';
const APPROVAL = '[data-testid="approval-btn"]';
const STOP = '[data-testid="stop-btn"]';
const ROW = '[data-testid="part-artifact-view"] li[data-row="notes/draft"]';
const ABSENT = [
  'textarea',
  '[data-testid="draft-input"]',
  '[data-testid="submit-input"]',
  '[data-testid="candidate-list"]',
  '[data-testid="candidate-btn"]',
  'select',
];
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

// 入力の口 5 種が 0 件で、停止が 1 件で見えていること。
async function assertNoInputArea(page, at) {
  const counts = await page.evaluate((sels) => sels.map((s) => document.querySelectorAll(s).length), ABSENT);
  assert.deepEqual(counts, ABSENT.map(() => 0), `${at}: ${ABSENT.join(', ')}`);
  assert.equal(await page.locator(STOP).count(), 1, `${at}: stop`);
  assert.equal(await page.locator(STOP).isVisible(), true, `${at}: stop is visible`);
}

const count = (page, sel) => page.locator(sel).count();

async function runNoInput(page) {
  await waitMounted(page);
  const yes = () => page.getByRole('button', { name: 'はい', exact: true }).click();

  // st-1（選択肢）: 選択肢 2 件。承認は 0 件。選ぶと受付になり、確認の point_at は左の行を指す。
  await entered(page, 'main/st-1');
  await assertNoInputArea(page, 'st-1 入力待ち');
  assert.equal(await count(page, CHOICE), 2);
  assert.equal(await count(page, APPROVAL), 0);
  await page.click(`${CHOICE}[data-choice="opt-a"]`);
  await accepted(page, 'main/st-1');
  await assertNoInputArea(page, 'st-1 確認待ち');
  await page.click(ROW, { timeout: T });

  // st-2（画面操作）: 選択肢・承認とも 0 件。対象の行を押すと受付になり、確認の answer の選択肢が出る。
  await entered(page, 'main/st-2');
  await assertNoInputArea(page, 'st-2 入力待ち');
  assert.equal(await count(page, CHOICE), 0);
  assert.equal(await count(page, APPROVAL), 0);
  await page.click(ROW, { timeout: T });
  await accepted(page, 'main/st-2');
  await assertNoInputArea(page, 'st-2 確認待ち');
  assert.equal(await count(page, CHOICE), 2);
  await page.click(`${CHOICE}[data-choice="opt-right"]`);

  // st-3（承認）: 承認のボタン 3 件。承認して、確認の「はい」で完了する。
  await entered(page, 'main/st-3');
  await assertNoInputArea(page, 'st-3 入力待ち');
  assert.equal(await count(page, APPROVAL), 3);
  assert.equal(await count(page, CHOICE), 0);
  await yes();
  await accepted(page, 'main/st-3');
  await assertNoInputArea(page, 'st-3 確認待ち');
  await yes();
  await page.waitForFunction(() => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    return e.stateWithoutLog.run.slices.main?.status?.['main/st-3'] === 'achieved' && window.__fandheTest.nextTick() === 'null';
  }, null, { timeout: T });
  await assertNoInputArea(page, '完了');
  assert.equal((await engine(page)).stateWithoutLog.run.slices.main.status['main/st-3'], 'achieved');
}

// 対照: 既定の教材（`input_field` の指定なし）では入力の口が 1 件ずつ出る。
async function runDefault(page) {
  await waitMounted(page);
  for (const sel of ['[data-testid="draft-input"]', '[data-testid="submit-input"]', '[data-testid="candidate-list"]', STOP]) {
    assert.equal(await count(page, sel), 1, sel);
  }
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  test(`入力欄なしの教材は入力欄・送信・候補を出さず最後まで進む（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.goto(`${srv.url}${QUERY}`);
      await runNoInput(page);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
      await context.close();
    }));
  });

  test(`対照: 既定の教材では入力欄・送信・候補の一覧が出る（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.goto(srv.url);
      await runDefault(page);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
      await context.close();
    }));
  });
}
