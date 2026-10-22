// Shift+Tab の逆向きのフォーカス移動の試験（TASK-278 2/3。JUDGE-19・TERM-10・A11Y-16・FEND-21・D-42）。
//
// Shift+Tab は補完にならず、preventDefault されない。ブラウザの通常の逆向きのフォーカス移動になる。
// 入力欄が画面の最初の操作対象のときは、フォーカスが文書の外（body）へ出る。
// 候補の一覧が開いている行は TASK-440 が入れるまで画面から作れないため、native の key_route_contract・
// key_decide_contract が固定する。ここでは候補が無い画面の行だけを確かめる。
// body への退出は配信設定（FANDHE_WEB_DIR）でだけ確かめる。試験の設定では keydown-probe が入力欄の前にあるため。
// Tab の一巡・罠 0 件は tab-cycle.test.mjs、部品がそろった画面での再確認は TASK-451 の担当。
//
// 入力: FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-shift-tab/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(
    dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(),
    'FANDHE_WEB_DIR must point to a web directory built without TEST_HOOKS',
  );
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

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  for (const text of ['hello', '']) {
    const label = text === '' ? '文字なし' : '文字あり';
    test(`入力欄の Shift+Tab は preventDefault されずフォーカスが履歴の読み込みの部品へ移る（${label}・CSP ${cspLabel}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await open(srv);
        await page.fill(INPUT, text);
        await page.focus(INPUT);
        // dispatchEvent は preventDefault されていなければ true を返す。
        const notPrevented = await page.evaluate((sel) => {
          const el = document.querySelector(sel);
          return el.dispatchEvent(new KeyboardEvent('keydown', {
            key: 'Tab', shiftKey: true, bubbles: true, cancelable: true,
          }));
        }, INPUT);
        assert.equal(notPrevented, true);
        // 信頼入力の keydown が bubble 段（glue の listener より後）で止められていないことも確かめる。
        await page.evaluate(() => {
          window.__shiftTabPrevented = [];
          window.addEventListener('keydown', (e) => {
            if (e.key === 'Tab' && e.shiftKey) window.__shiftTabPrevented.push(e.defaultPrevented);
          });
        });
        await page.keyboard.press('Shift+Tab');
        assert.deepEqual(await page.evaluate(() => window.__shiftTabPrevented), [false]);
        // 入力欄の前に履歴の操作（TASK-531 1/2）があるため、フォーカスは body でなく、その最後の部品（読み込みのファイル入力）へ移る。
        assert.equal(await page.evaluate(() => {
          const a = document.activeElement;
          return !!a && a.matches('input[type="file"]') && !!a.closest('[data-testid="shell-import"]');
        }), true);
        assert.equal(await page.inputValue(INPUT), text);
        assertClean(rec);
        await context.close();
      }));
    });
  }

  test(`送信ボタンの Shift+Tab でフォーカスが入力欄へ戻る（CSP ${cspLabel}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.focus(SUBMIT);
      await page.keyboard.press('Shift+Tab');
      assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'draft-input');
      assertClean(rec);
      await context.close();
    }));
  });
}
