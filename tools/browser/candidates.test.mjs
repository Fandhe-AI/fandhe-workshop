// 候補の一覧の開閉と Tab の補完の試験（TASK-440。TERM-8・TERM-11・TERM-42・D-60・D-61。PoC-2 の K-03〜K-09）。
//
// 試験の設定（FANDHE_WEB_TEST_DIR。教材は min-3step.json。コマンドは help・make-note・publish-note と、
// unsupported の sync-cloud）で、入力欄の下書きに応じて一覧が開閉することを確かめる。
//   開く     /he → /help だけ。/ → unsupported を除く 3 件
//   開かない /help（完全一致）・/zzz（候補なし）・/he lp（空白入り）・/s（unsupported だけ）
//   閉じる   開いている一覧は Esc で閉じる。入力は残り、エンジンのログは増えない（K-09）。閉じた後の Tab は補完にならない（K-06）。
//            下書きを変えると再び開く
// 配信設定（FANDHE_WEB_DIR）は IR を読み込まないため、一覧は開かない。
//
// Tab の補完（2/2）: 一覧が開いていて文字があるとき、Tab は preventDefault され、下書きが先頭の候補になり
// <textarea> に書き戻される。フォーカスは入力欄のまま（K-03）。unsupported は候補に出ないため補完されない。
// 変換中の Tab は何もしない（K-07。実 IME での確認は人が行う）。配送と書き戻しは set_timeout(0) を 2 段通るため、
// 値は waitForFunction で待つ。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-candidates/';
const INPUT = '[data-testid="draft-input"]';
const LIST = '[data-testid="candidate-list"]';
const BTNS = '[data-testid="candidate-btn"]';
const PROBE = '[data-testid="keydown-probe"]';
const SUBMIT = '[data-testid="submit-input"]';

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

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

const isHidden = (page) => page.locator(LIST).evaluate((el) => el.hasAttribute('hidden'));
const labels = (page) => page.locator(BTNS).allTextContents();
const logLen = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine.logLen;
const completeCount = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).completeCount;
const waitValue = (page, value) => page.waitForFunction(
  ([sel, v]) => document.querySelector(sel)?.value === v, [INPUT, value]);
const tabEvent = (page, init) => page.evaluate(([sel, i]) => {
  const el = document.querySelector(sel);
  el.focus();
  return el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true, ...i }));
}, [INPUT, init]);
const focusedId = (page) => page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? null);

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  test(`接頭辞で一覧が開き 完全一致・候補なし・空白入り・unsupported だけでは開かない（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      assert.equal(await isHidden(page), true);
      await page.fill(INPUT, '/he');
      assert.equal(await isHidden(page), false);
      assert.deepEqual(await labels(page), ['/help']);
      await page.fill(INPUT, '/');
      assert.deepEqual(await labels(page), ['/help', '/make-note', '/publish-note']);
      for (const text of ['/help', '/zzz', '/he lp', '/s', 'he', '']) {
        await page.fill(INPUT, text);
        assert.equal(await isHidden(page), true, text);
        assert.deepEqual(await labels(page), [], text);
      }
      assertClean(rec);
      await context.close();
    });
  });

  test(`開かないときの Tab は補完にならず送信ボタンへ移る（K-05・K-08。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      for (const text of ['/help', '/zzz']) {
        await page.fill(INPUT, text);
        await page.focus(INPUT);
        await page.keyboard.press('Tab');
        assert.equal(await focusedId(page), 'submit-input', text);
        assert.equal(await page.inputValue(INPUT), text);
      }
      assertClean(rec);
      await context.close();
    });
  });

  test(`Esc は開いた一覧を閉じるだけで、入力もログも変えず、下書きを変えると再び開く（K-06・K-09。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, '/he');
      await page.focus(INPUT);
      const before = await logLen(page);
      const notPrevented = await page.evaluate((sel) => {
        const el = document.querySelector(sel);
        return el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
      }, INPUT);
      assert.equal(notPrevented, true);
      // 1 回目の Esc（preventDefault されない）で閉じ、ログは変わらない。閉じた後の Esc は取り消しの入力になり、
      // 取り消しを受け付けない Step では拒否が記録される（InputReceived と Judged の 2 件。TASK-441 2/2）。
      assert.equal(await isHidden(page), true);
      assert.equal(await logLen(page), before);
      await page.keyboard.press('Escape');
      assert.equal(await isHidden(page), true);
      assert.equal(await page.inputValue(INPUT), '/he');
      const afterCancel = await logLen(page);
      assert.equal(afterCancel, before + 2);
      // 閉じた後の Tab は補完にならない。
      await page.keyboard.press('Tab');
      assert.equal(await focusedId(page), 'submit-input');
      assert.equal(await page.inputValue(INPUT), '/he');
      // 下書きを変えると再び開き、次の Esc は閉じるだけ。
      await page.fill(INPUT, '/hel');
      assert.equal(await isHidden(page), false);
      assert.deepEqual(await labels(page), ['/help']);
      await page.focus(INPUT);
      await page.keyboard.press('Escape');
      assert.equal(await isHidden(page), true);
      assert.equal(await logLen(page), afterCancel);
      assertClean(rec);
      await context.close();
    });
  });

  test(`開いているとき Shift+Tab は欄の前の要素へ移り値は変わらない（K-04。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, '/he');
      await page.focus(INPUT);
      await page.keyboard.press('Shift+Tab');
      assert.equal(await focusedId(page), PROBE.match(/"(.+)"/)[1]);
      assert.equal(await page.inputValue(INPUT), '/he');
      assert.equal(await isHidden(page), false);
      assertClean(rec);
      await context.close();
    });
  });

  test(`開いているとき Tab は preventDefault され、先頭の候補で補完して欄に残る（K-03。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, '/he');
      assert.equal(await tabEvent(page, {}), false);
      await waitValue(page, '/help');
      assert.equal(await focusedId(page), 'draft-input');
      assert.equal(await isHidden(page), true);
      assert.equal(await completeCount(page), 1);
      assertClean(rec);
      await context.close();
    });
  });

  test(`実際の Tab 操作で補完され、複数候補では先頭になり、補完後の Tab は送信ボタンへ移る（K-03・K-08。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const before = await logLen(page);
      await page.fill(INPUT, '/');
      await page.focus(INPUT);
      await page.keyboard.press('Tab');
      await waitValue(page, '/help');
      assert.equal(await focusedId(page), 'draft-input');
      assert.equal(await logLen(page), before);
      // 補完後は完全一致で一覧が閉じているため、次の Tab は通常のフォーカス移動。
      await page.keyboard.press('Tab');
      assert.equal(await focusedId(page), 'submit-input');
      assert.equal(await page.inputValue(INPUT), '/help');
      assertClean(rec);
      await context.close();
    });
  });

  test(`unsupported だけに一致する下書きでは Tab は補完にならない（TERM-42。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, '/s');
      await page.focus(INPUT);
      await page.keyboard.press('Tab');
      assert.equal(await focusedId(page), 'submit-input');
      assert.equal(await page.inputValue(INPUT), '/s');
      assert.equal(await completeCount(page), 0);
      assertClean(rec);
      await context.close();
    });
  });

  test(`変換中の Tab はアプリが何もしない（K-07。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, '/he');
      assert.equal(await tabEvent(page, { isComposing: true }), true);
      await page.waitForTimeout(100);
      assert.equal(await page.inputValue(INPUT), '/he');
      assert.equal(await completeCount(page), 0);
      assertClean(rec);
      await context.close();
    });
  });

  test(`配信設定は IR を読み込まないため一覧は開かない（CSP ${cspLabel}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, '/he');
      assert.equal(await isHidden(page), true);
      assert.deepEqual(await labels(page), []);
      assert.equal(await page.locator(SUBMIT).count(), 1);
      assertClean(rec);
      await context.close();
    }));
  });
}
