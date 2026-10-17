// 入力欄のキー操作の不変条件の基準（TASK-351。FEND-21）。このファイルは Enter 系（Enter・Shift+Enter・修飾キー付き Enter）。
//
// 変換中でない状態のキー操作の「観察できる結果」（keydown の preventDefault の有無・ログの行・欄の値）を、
// 受け持ち（属性化・上流の口・glue）に依らない表として固定する。TASK-352（受け持ちの置き換え）は、
// 置き換えの前後でこのファイルを同じ件数で通すことで、結果が変わっていないことを比べる。
// 現状の受け持ちは glue（D-42）。修飾キー付き Enter を送信にも改行にもしない扱いは JUDGE-22・D-60 の担当で、ここは現状を固定するだけ。
//
// 観察の方法: 配信設定（TEST_HOOKS なし）だけを使い、内部の印（__fandheTest・data-action-keydown 等）は読まない。
// 入力は信頼入力（page.keyboard.press）。preventDefault は、window の捕捉の段で keydown の Event を保持し、
// 押した後にその defaultPrevented を読む。誰がどの段で呼んでも観察できる。
//
// 表の列（#346 の Tab・Shift+Tab の行と共有する契約）: id・press・draft・prevented・lines・value。
// 行を足すときは EXPECTED_ROWS と docs/testing.md の記録を同時に更新する。
// 変換中（isComposing・229・Process）は FEND-21 の前提の外で、ime-input・enter-submit の担当。
// 限界: Playwright の keyboard.press は、Shift 以外の修飾キーを押している間は text を送らない。
// 実キーボードの OS ごとの差（とくに Windows）は代わりに確かめない（D-48 で別タスク）。
//
// 入力: FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-key-invariants/';
const INPUT = '[data-testid="draft-input"]';
const ITEMS = '[data-testid="log-list"] > li';

const EXPECTED_ROWS = 11;
const ROWS = [
  { id: 'ENTER-TEXT', press: 'Enter', draft: 'x', prevented: true, lines: ['x'], value: '' },
  { id: 'ENTER-EMPTY', press: 'Enter', draft: '', prevented: true, lines: [], value: '' },
  { id: 'SHIFT-ENTER-TEXT', press: 'Shift+Enter', draft: 'x', prevented: false, lines: [], value: 'x\n' },
  { id: 'SHIFT-ENTER-EMPTY', press: 'Shift+Enter', draft: '', prevented: false, lines: [], value: '\n' },
  { id: 'CTRL-ENTER', press: 'Control+Enter', draft: 'x', prevented: false, lines: [], value: 'x' },
  { id: 'ALT-ENTER', press: 'Alt+Enter', draft: 'x', prevented: false, lines: [], value: 'x' },
  { id: 'META-ENTER', press: 'Meta+Enter', draft: 'x', prevented: false, lines: [], value: 'x' },
  { id: 'SHIFT-CTRL-ENTER', press: 'Shift+Control+Enter', draft: 'x', prevented: false, lines: [], value: 'x' },
  { id: 'SHIFT-ALT-ENTER', press: 'Shift+Alt+Enter', draft: 'x', prevented: false, lines: [], value: 'x' },
  { id: 'SHIFT-META-ENTER', press: 'Shift+Meta+Enter', draft: 'x', prevented: false, lines: [], value: 'x' },
  { id: 'CTRL-ALT-ENTER', press: 'Control+Alt+Enter', draft: 'x', prevented: false, lines: [], value: 'x' },
];

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

// 入力欄が対象の keydown の Event を、window の捕捉の段で保持する（init script として実行される）。
const CAPTURE = (sel) => {
  window.__keyEvents = [];
  window.addEventListener('keydown', (ev) => {
    const t = ev.target;
    if (t && t.matches && t.matches(sel)) window.__keyEvents.push(ev);
  }, true);
};

// 非同期の経路（送信後の空化・遅れた送信）を見逃さないよう、タイマーを 2 回挟む。
const settle = (page) => page.evaluate(() => new Promise((r) => {
  setTimeout(() => setTimeout(r, 0), 0);
}));

test(`表の行数は ${EXPECTED_ROWS} 件で固定されている`, () => {
  assert.equal(ROWS.length, EXPECTED_ROWS);
  assert.equal(new Set(ROWS.map((r) => r.id)).size, EXPECTED_ROWS);
});

for (const row of ROWS) {
  test(`${row.id}: ${row.press}（下書き ${JSON.stringify(row.draft)}）`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.addInitScript(CAPTURE, INPUT);
      await page.goto(srv.url);
      await waitMounted(page);
      await page.fill(INPUT, row.draft);
      await page.focus(INPUT);
      await page.evaluate(() => { window.__keyEvents.length = 0; });
      await page.keyboard.press(row.press);

      if (row.lines.length > 0) {
        await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [ITEMS, row.lines.length]);
      }
      await settle(page);
      if (row.id === 'ENTER-TEXT') {
        await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT);
      }

      const seen = await page.evaluate(() => window.__keyEvents
        .filter((ev) => ev.key === 'Enter')
        .map((ev) => ev.defaultPrevented));
      assert.deepEqual(seen, [row.prevented], 'Enter の keydown は 1 回で、defaultPrevented が期待どおり');
      assert.deepEqual(
        await page.$$eval(ITEMS, (lis) => lis.map((li) => li.textContent)),
        row.lines,
        'ログの行',
      );
      assert.equal(await page.inputValue(INPUT), row.value, '欄の値');

      const s = summarize(rec);
      const detail = JSON.stringify(rec);
      assert.equal(s.csp, 0, detail);
      assert.equal(s.pageErrors, 0, detail);
      assert.equal(s.consoleErrors, 0, detail);
      await context.close();
    });
  });
}
