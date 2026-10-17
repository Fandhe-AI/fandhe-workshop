// キーボードだけで確認（point_at）を含む Step を達成まで進める試験（TASK-358 2/2。A11Y-18・FEND-13。PoC-2 の CH1 相当）。
//
// 製品の画面（#app）で、Tab・Shift+Tab・文字の入力・Enter・Space だけを使う。流れは次のとおり。
//   入力欄へ Tab で移る → `/make-note first-note` を入力 → Enter（受付と進行）→ 左の成果物の行（notes/draft）へ Tab で移る
//   → Enter または Space（click と同じ扱いで確認が成立）→ st-1 が達成になり st-2 へ進む。
// `waitMounted` の後の操作は `page.keyboard` だけである。`page.click`・`page.fill`・`page.focus`・要素の click()・
// raw-trigger・load-fixture は使わない。`page.evaluate` は観察（フォーカス・属性・状態の読み取り）だけに使う。
// 信頼されたマウスのイベント（pointerdown・mousedown）が 0 件であることを window の capture で数えて確かめる。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい画面を開く）:
//   - KC1: Enter で完走。確認の前は st-1 が達成でなく、行へ Tab で着いて Enter すると st-1 が達成・st-2 が現在になる
//   - KC2: Space で同じ流れ。ページがスクロールしない
//   - KC3: 入力欄から Shift+Tab の逆順でも同じ行へ着き、Enter で達成になる
//   - KC4: 達成の後に同じ行で Enter・Space を押しても、達成は 1 回のままで現在の Step が戻らない
//   - KC5: 修飾キー付き（Shift+Enter）では達成にならず、続く Enter で達成になる
//   - 外部へのリクエスト・ページのエラー・ダイアログ・危険な要素・CSP 違反が 0 件である
//
// 確かめないこと:
//   - 製品で指せる行の規則は仮置き（選ばれたビューの宣言 ID が引ける行はすべて指せる）。確定は TASK-470・TASK-347 の後続と TASK-739
//   - st-2 の入力方式 `choice` の画面の口は TASK-450 1/3 で実装済みだが、キーボードだけでの完走は TASK-450 3/3（#406）の担当のため、本試験の完走は確認（point_at）の Step の達成までとする。
//     st-3 以降と 6 Step の CH-1 全体は扱わない
//   - 世界状態の行は st-1 の受付後も notes/draft の 1 件だけなので、外れた行を指す経路（rj-miss-point）は native の試験の範囲
//   - `step-status` の表示は confirm-status.test.mjs（TASK-435 2/2）が確かめる。人によるキーボードだけの完走は JUDGE-20 の担当で、本試験は人の評価に数えない
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。mount の教材は min-3step。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-keyboard-confirm/';
const INPUT = '[data-testid="draft-input"]';
const ROW = '[data-testid="part-artifact-view"] li[data-row="notes/draft"]';
const REVIEW = '[data-testid="part-review-list"]';
const T = 30000;
const TAB_LIMIT = 300;

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

const engineOf = (page) => page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine);

const activeMatches = (page, sel) => page.evaluate((s) => document.activeElement?.matches(s) ?? false, sel);

// key を押して sel に着くまで進める。押した回数を返す。着かなければ失敗にする。
async function keyTo(page, sel, key) {
  for (let n = 0; n <= TAB_LIMIT; n += 1) {
    if (await activeMatches(page, sel)) return n;
    if (n === TAB_LIMIT) break;
    await page.keyboard.press(key);
  }
  assert.fail(`${key} で ${TAB_LIMIT} 回押しても ${sel} に着かない`);
  return -1;
}

// 受付と進行までを、キーボードだけで終える。確認待ち（st-1 が accepted・runStatus が idle）にして返す。
async function submitByKeyboard(page) {
  const tabs = await keyTo(page, INPUT, 'Tab');
  console.log(`tab presses to input: ${tabs}`);
  await page.keyboard.type('/make-note first-note');
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    return e.runStatus === 'idle' && e.stateWithoutLog.run.slices.main.status['main/st-1'] === 'accepted';
  }, null, { timeout: T });
  await page.waitForSelector(ROW, { timeout: T });
  const e = await engineOf(page);
  assert.equal(e.current, 'main/st-1');
  assert.equal(e.log.filter((r) => r.kind === 'achieved').length, 0, '確認の前に達成になっていない');
  const marked = await page.evaluate((r) => document.querySelector(`${r}[data-achieved="true"]`)?.dataset.step ?? null, REVIEW + ' > li');
  assert.equal(marked, null, '確認の前に復習が達成を示していない');
}

// 行の 4 属性を観察する。
const rowAttrs = (page) => page.evaluate((sel) => {
  const li = document.querySelector(sel);
  return li && {
    action: li.getAttribute('data-action'),
    row: li.getAttribute('data-row'),
    tabindex: li.getAttribute('tabindex'),
    hasPayload: li.hasAttribute('data-payload'),
  };
}, ROW);

async function expectConfirmed(page) {
  await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.current === 'main/st-2',
    null, { timeout: T });
  const e = await engineOf(page);
  assert.equal(e.log.filter((r) => r.kind === 'achieved').length, 1, JSON.stringify(e.log));
  const achieved = await page.evaluate((r) => [...document.querySelectorAll(r)]
    .filter((li) => li.dataset.achieved === 'true').map((li) => li.dataset.step), `${REVIEW} > li`);
  assert.ok(achieved.includes('main/st-1'), JSON.stringify(achieved));
}

async function install(page) {
  await page.evaluate(() => {
    window.__kc = { keys: [], mouse: 0 };
    window.addEventListener('keydown', (e) => window.__kc.keys.push({ key: e.key, prevented: e.defaultPrevented, shift: e.shiftKey }));
    for (const t of ['pointerdown', 'mousedown']) {
      window.addEventListener(t, (e) => { if (e.isTrusted) window.__kc.mouse += 1; }, true);
    }
  });
}

const lastKey = (page) => page.evaluate(() => window.__kc.keys[window.__kc.keys.length - 1]);

async function scenario(csp, fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    await install(page);
    await fn(page);
    assert.equal(await page.evaluate(() => window.__kc.mouse), 0, '信頼されたマウスの操作が混ざっていない');
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
    await context.close();
  }));
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`KC1: Tab・文字・Enter だけで確認を完走する（${mode}）`, async () => {
    await scenario(csp, async (page) => {
      await submitByKeyboard(page);
      assert.deepEqual(await rowAttrs(page), { action: 'ui', row: 'notes/draft', tabindex: '0', hasPayload: true });
      const tabs = await keyTo(page, ROW, 'Tab');
      console.log(`tab presses input -> row: ${tabs}`);
      await page.keyboard.press('Enter');
      assert.equal((await lastKey(page)).prevented, true, 'Enter の既定の動作が止められた');
      await expectConfirmed(page);
    });
  });

  test(`KC2: Space でも同じ流れで完走し、ページがスクロールしない（${mode}）`, async () => {
    await scenario(csp, async (page) => {
      await submitByKeyboard(page);
      await keyTo(page, ROW, 'Tab');
      const before = await page.evaluate(() => window.scrollY);
      await page.keyboard.press('Space');
      assert.equal((await lastKey(page)).prevented, true, 'Space の既定の動作が止められた');
      await expectConfirmed(page);
      assert.equal(await page.evaluate(() => window.scrollY), before);
    });
  });

  test(`KC3: Shift+Tab の逆順でも同じ行に着き、Enter で達成になる（${mode}）`, async () => {
    await scenario(csp, async (page) => {
      await submitByKeyboard(page);
      const back = await keyTo(page, ROW, 'Shift+Tab');
      console.log(`shift+tab presses input -> row: ${back}`);
      await page.keyboard.press('Enter');
      await expectConfirmed(page);
    });
  });

  test(`KC4: 達成の後に同じ行で Enter・Space を押しても達成は 1 回のままで戻らない（${mode}）`, async () => {
    await scenario(csp, async (page) => {
      await submitByKeyboard(page);
      await keyTo(page, ROW, 'Tab');
      await page.keyboard.press('Enter');
      await expectConfirmed(page);
      if (await page.evaluate((s) => document.querySelector(s) !== null, ROW)) {
        if (!(await activeMatches(page, ROW))) await keyTo(page, ROW, 'Tab');
        await page.keyboard.press('Enter');
        await page.keyboard.press('Space');
      }
      const e = await engineOf(page);
      assert.equal(e.log.filter((r) => r.kind === 'achieved').length, 1, JSON.stringify(e.log));
      assert.notEqual(e.current, 'main/st-1', '現在の Step が戻っていない');
    });
  });

  test(`KC5: Shift+Enter では達成にならず、続く Enter で達成になる（${mode}）`, async () => {
    await scenario(csp, async (page) => {
      await submitByKeyboard(page);
      await keyTo(page, ROW, 'Tab');
      await page.keyboard.press('Shift+Enter');
      assert.equal((await lastKey(page)).prevented, false, '修飾キー付きは止めない');
      await page.waitForTimeout(300);
      let e = await engineOf(page);
      assert.equal(e.current, 'main/st-1');
      assert.equal(e.log.filter((r) => r.kind === 'achieved').length, 0);
      await page.keyboard.press('Enter');
      await expectConfirmed(page);
      e = await engineOf(page);
      assert.equal(e.current, 'main/st-2');
    });
  });
}
