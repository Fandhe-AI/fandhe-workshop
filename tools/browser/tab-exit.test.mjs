// Tab で入力欄から出入りできることの試験（TASK-373 1/2。A11Y-16・JUDGE-19）。
//
// 試験用の口が無い製品の画面（教材の IR を同梱した配信する設定の `?start=s-1-1`）で、候補の一覧が開きうる状態の
// Tab の出入りを固定する。Tab が preventDefault されるのは「一覧が開いていて欄に文字があるとき」だけ（JUDGE-19）。
//   X1 空欄・X2 候補なし・X3 完全一致・X4 unsupported だけ・X5 Esc で閉じた後: 1 回の Tab で欄から出る（preventDefault なし）。
//   X6 補完の直後: 1 回目は補完（preventDefault）、2 回目で出る。
//   E1 前の要素から Tab で入る。E2 出て戻っても値が残る。
// 退出先は ID を決め打ちにせず、DOM 順で欄の次のフォーカスできる要素とする（送信ボタンが無効のときの違いを吸収する）。
// 対照として、罠を試験のページの中だけに入れ、判定の関数 exitProblems（tab-focus.mjs に置く。#2124）が検出できることを確かめる（配信物には残さない）。
// Shift+Tab・一巡・罠 0 件の主張は TASK-373 2/2。既存の tab-complete・tab-cycle は IR なしの画面で、本試験は IR ありの画面。
//
// 入力: FANDHE_WEB_AUTHOR_DIR（IR 同梱の配信する設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import {
  listFocusables, installTabProbe, pressTracked, activeTestId, focusAt, exitProblems,
} from './tab-focus.mjs';

const BASE = '/fandhe-tab-exit/';
const INPUT = '[data-testid="draft-input"]';
const LIST = '[data-testid="candidate-list"]';
const LOG_ITEMS = '[data-testid="log-list"] li';

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(
    dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(),
    'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS',
  );
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin, viewport: { width: 1024, height: 768 } });
  await opened.page.goto(`${srv.url}?start=s-1-1`);
  await waitMounted(opened.page);
  assert.equal(await opened.page.evaluate(() => typeof window.__fandheTest), 'undefined', 'author build must have no test hooks');
  await installTabProbe(opened.page);
  return opened;
}

function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

const listHidden = (page) => page.evaluate((sel) => document.querySelector(sel).hidden, LIST);
const draftValue = (page) => page.inputValue(INPUT);
const logCount = (page) => page.evaluate((sel) => document.querySelectorAll(sel).length, LOG_ITEMS);

// 値を入れて欄にフォーカスし、一覧の開閉が期待どおりであることを前提として照合する。
async function arm(page, value, listOpen) {
  await page.fill(INPUT, value);
  await page.focus(INPUT);
  assert.equal(await listHidden(page), !listOpen, `candidate list ${listOpen ? 'should be open' : 'should be closed'} for ${value}`);
  const { ids } = await listFocusables(page);
  const idx = ids.indexOf('draft-input');
  assert.ok(idx >= 0 && idx < ids.length - 1, `draft-input must be followed by a focusable: ${ids.join(',')}`);
  return { ids, idx };
}

const EXIT_SCENES = [
  { id: 'X1 空欄（K-01）', value: '' },
  { id: 'X2 候補なし（K-05）', value: '/zzz' },
  { id: 'X3 完全一致（K-08）', value: '/help' },
  { id: 'X4 unsupported だけ（TERM-42）', value: '/sy' },
];

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_AUTHOR_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  for (const scene of EXIT_SCENES) {
    test(`${scene.id}: 一覧が閉じているとき Tab で欄から次の要素へ出る（CSP ${cspLabel}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv);
        const { ids, idx } = await arm(page, scene.value, false);
        console.log(`${scene.id} csp=${cspLabel} ids=${ids.join(',')}`);
        const r = await pressTracked(page, 'Tab');
        const value = await draftValue(page);
        assert.deepEqual(exitProblems({ ...r, value, expectedValue: scene.value }), []);
        assert.equal(r.after, idx + 1);
        assert.notEqual(await activeTestId(page), 'draft-input');
        assertClean(rec);
        await context.close();
      });
    });
  }

  test(`X5 Esc で閉じた後（K-06）: Tab で欄から出て、値とログが変わらない（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await arm(page, '/he', true);
      await page.keyboard.press('Escape');
      assert.equal(await listHidden(page), true, 'Esc closes the list');
      const logs = await logCount(page);
      await page.focus(INPUT);
      const lst = await listFocusables(page);
      const i = lst.ids.indexOf('draft-input');
      const r = await pressTracked(page, 'Tab');
      assert.equal(r.before, i);
      assert.deepEqual(exitProblems({ ...r, value: await draftValue(page), expectedValue: '/he' }), []);
      assert.equal(await logCount(page), logs, 'Esc and Tab must not add log rows');
      assertClean(rec);
      await context.close();
    });
  });

  test(`X6 補完の直後（K-03 の後）: 1 回目は補完で欄に残り、2 回目で出る（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const { idx } = await arm(page, '/he', true);
      const first = await pressTracked(page, 'Tab');
      assert.deepEqual(first.prevented, [true]);
      await page.waitForFunction((sel) => document.querySelector(sel).value === '/help', INPUT);
      assert.equal(first.after, idx, 'completion keeps the focus in the field');
      assert.equal(await activeTestId(page), 'draft-input');
      const second = await pressTracked(page, 'Tab');
      assert.deepEqual(exitProblems({ ...second, value: await draftValue(page), expectedValue: '/help' }), []);
      assertClean(rec);
      await context.close();
    });
  });

  test(`E1 前の要素から Tab で欄へ入る（空欄）（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const { idx } = await arm(page, '', false);
      await focusAt(page, idx - 1);
      const r = await pressTracked(page, 'Tab');
      assert.equal(await activeTestId(page), 'draft-input');
      assert.equal(r.after, idx);
      assert.deepEqual(r.prevented, [false]);
      assert.equal(await draftValue(page), '');
      assertClean(rec);
      await context.close();
    });
  });

  test(`E2 出て戻っても値が残る（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await arm(page, '/he', true);
      await page.keyboard.press('Escape');
      const idx = (await listFocusables(page)).ids.indexOf('draft-input');
      await pressTracked(page, 'Tab');
      assert.notEqual(await activeTestId(page), 'draft-input');
      await focusAt(page, idx - 1);
      const back = await pressTracked(page, 'Tab');
      assert.equal(await activeTestId(page), 'draft-input');
      assert.equal(back.after, idx);
      assert.equal(await draftValue(page), '/he');
      console.log(`E2 csp=${cspLabel} list hidden after returning: ${await listHidden(page)}`);
      assertClean(rec);
      await context.close();
    });
  });
}

// 対照: 欄の Tab を握りつぶす罠を試験のページの中だけに入れ、exitProblems が検出することを確かめる。
const TRAPS = [
  { id: 'N1 preventDefault', refocus: false },
  { id: 'N2 preventDefault + focus を戻す', refocus: true },
];
for (const trap of TRAPS) {
  test(`${trap.id}: 罠を入れると exitProblems が出られないことを検出する（CSP あり）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_AUTHOR_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context } = await open(srv);
      await arm(page, '', false);
      await page.evaluate((refocus) => {
        document.querySelector('[data-testid="draft-input"]').addEventListener('keydown', (e) => {
          if (e.key !== 'Tab') return;
          e.preventDefault();
          if (refocus) e.currentTarget.focus();
        });
      }, trap.refocus);
      const r = await pressTracked(page, 'Tab');
      const problems = exitProblems({ ...r, value: await draftValue(page), expectedValue: '' });
      assert.ok(problems.length >= 1, `trap must be detected: ${JSON.stringify(r)}`);
      await context.close();
    }));
  });
}
