// 入力欄のキー操作の不変条件の基準（TASK-351。FEND-21）。Enter 系（Enter・Shift+Enter・修飾キー付き Enter）と
// Tab 系（Tab・Shift+Tab）を 1 つの表で固定する。
//
// 変換中でない状態のキー操作の「観察できる結果」（keydown の preventDefault の有無・ログの行・欄の値・
// 押した後のフォーカス先）を、受け持ち（属性化・上流の口・glue）に依らない表として固定する。
// TASK-352（受け持ちの置き換え）は、置き換えの前後でこのファイルを同じ件数で通すことで、結果が変わっていないことを比べる。
// 現状の受け持ちは glue（D-42）。修飾キー付き Enter を送信にも改行にもしない扱いは JUDGE-22・D-60 の担当で、ここは現状を固定するだけ。
// Tab は「候補の一覧が開いていて欄に文字があるとき」だけ preventDefault される。Shift+Tab は常に preventDefault されない（JUDGE-19）。
//
// 観察の方法: 内部の印（__fandheTest・data-action-keydown 等）は読まず、DOM（候補の一覧の hidden・欄の値・ログの行・
// document.activeElement の data-testid）と、捕捉した keydown の Event の defaultPrevented だけで観察する。
// 入力は信頼入力（page.keyboard.press）。preventDefault は、window の捕捉の段で keydown の Event を保持し、
// 押した後にその defaultPrevented を読む。誰がどの段で呼んでも観察できる。
// 配信設定（web: 'delivery'）は IR を読み込まず候補の一覧が常に閉じているため、一覧が開いている行（TAB-OPEN 等）は
// 試験の設定（web: 'test'。埋め込み教材 min-3step）で観察する。基準を 1 ファイルに置き、置き換えの前後で件数を比べるため。
//
// 表の列: id・web・open（押す前の一覧の開閉。前提として照合する）・press・key（捕捉する KeyboardEvent.key）・
// draft・prevented・lines（ログの増分）・value・focus（押した後のフォーカス先の data-testid。body は 'BODY'）。
// 行を足すときは EXPECTED_ROWS と docs/testing.md の記録を同時に更新する。
// 変換中（isComposing・229・Process）は FEND-21 の前提の外で、ime-input・enter-submit の担当。
// 限界: Playwright の keyboard.press は、Shift 以外の修飾キーを押している間は text を送らない。
// 実キーボードの OS ごとの差（とくに Windows）は代わりに確かめない（D-48 で別タスク）。
// 一覧が開いていて欄が空の組み合わせは画面から作れない（空にすると閉じる）ため、contract の key_decide_contract
// （JUDGE19-1）に任せる。Esc で閉じた後の Tab は candidates（K-06）の担当。Tab の一巡・罠 0 件は tab-cycle の担当。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-key-invariants/';
const INPUT = '[data-testid="draft-input"]';
const ITEMS = '[data-testid="log-list"] > li';
const LIST = '[data-testid="candidate-list"]';

const EXPECTED_ROWS = 19;
const enter = (id, press, draft, prevented, lines, value) => ({
  id, web: 'delivery', open: false, press, key: 'Enter', draft, prevented, lines, value, focus: 'draft-input',
});
const ROWS = [
  enter('ENTER-TEXT', 'Enter', 'x', true, ['x'], ''),
  enter('ENTER-EMPTY', 'Enter', '', true, [], ''),
  enter('SHIFT-ENTER-TEXT', 'Shift+Enter', 'x', false, [], 'x\n'),
  enter('SHIFT-ENTER-EMPTY', 'Shift+Enter', '', false, [], '\n'),
  enter('CTRL-ENTER', 'Control+Enter', 'x', false, [], 'x'),
  enter('ALT-ENTER', 'Alt+Enter', 'x', false, [], 'x'),
  enter('META-ENTER', 'Meta+Enter', 'x', false, [], 'x'),
  enter('SHIFT-CTRL-ENTER', 'Shift+Control+Enter', 'x', false, [], 'x'),
  enter('SHIFT-ALT-ENTER', 'Shift+Alt+Enter', 'x', false, [], 'x'),
  enter('SHIFT-META-ENTER', 'Shift+Meta+Enter', 'x', false, [], 'x'),
  enter('CTRL-ALT-ENTER', 'Control+Alt+Enter', 'x', false, [], 'x'),
  { id: 'TAB-TEXT', web: 'delivery', open: false, press: 'Tab', key: 'Tab', draft: 'x', prevented: false, lines: [], value: 'x', focus: 'submit-input' },
  { id: 'TAB-EMPTY', web: 'delivery', open: false, press: 'Tab', key: 'Tab', draft: '', prevented: false, lines: [], value: '', focus: 'submit-input' },
  { id: 'SHIFT-TAB-TEXT', web: 'delivery', open: false, press: 'Shift+Tab', key: 'Tab', draft: 'x', prevented: false, lines: [], value: 'x', focus: 'BODY' },
  { id: 'SHIFT-TAB-EMPTY', web: 'delivery', open: false, press: 'Shift+Tab', key: 'Tab', draft: '', prevented: false, lines: [], value: '', focus: 'BODY' },
  { id: 'TAB-OPEN', web: 'test', open: true, press: 'Tab', key: 'Tab', draft: '/he', prevented: true, lines: [], value: '/help', focus: 'draft-input' },
  { id: 'SHIFT-TAB-OPEN', web: 'test', open: true, press: 'Shift+Tab', key: 'Tab', draft: '/he', prevented: false, lines: [], value: '/he', focus: 'keydown-probe' },
  { id: 'TAB-NOMATCH', web: 'test', open: false, press: 'Tab', key: 'Tab', draft: '/zzz', prevented: false, lines: [], value: '/zzz', focus: 'submit-input' },
  { id: 'TAB-EXACT', web: 'test', open: false, press: 'Tab', key: 'Tab', draft: '/help', prevented: false, lines: [], value: '/help', focus: 'submit-input' },
];

const DIR_ENV = { delivery: 'FANDHE_WEB_DIR', test: 'FANDHE_WEB_TEST_DIR' };

let browser;

before(async () => {
  for (const [name, hint] of [
    ['FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS'],
    ['FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1'],
  ]) {
    const dir = process.env[name];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  }
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

const lineTexts = (page) => page.$$eval(ITEMS, (lis) => lis.map((li) => li.textContent));

for (const row of ROWS) {
  test(`${row.id}: ${row.press}（下書き ${JSON.stringify(row.draft)}）`, async () => {
    await withServer({ dir: process.env[DIR_ENV[row.web]], base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.addInitScript(CAPTURE, INPUT);
      await page.goto(srv.url);
      await waitMounted(page);
      await page.fill(INPUT, row.draft);
      await page.focus(INPUT);
      assert.equal(
        await page.locator(LIST).evaluate((el) => !el.hasAttribute('hidden')),
        row.open,
        '押す前の候補の一覧の開閉（前提）',
      );
      const before = await lineTexts(page);
      await page.evaluate(() => { window.__keyEvents.length = 0; });
      await page.keyboard.press(row.press);

      if (row.lines.length > 0) {
        await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [ITEMS, before.length + row.lines.length]);
      }
      if (row.value !== row.draft) {
        await page.waitForFunction(([sel, v]) => document.querySelector(sel).value === v, [INPUT, row.value]);
      }
      await settle(page);

      const seen = await page.evaluate((key) => window.__keyEvents
        .filter((ev) => ev.key === key)
        .map((ev) => ev.defaultPrevented), row.key);
      assert.deepEqual(seen, [row.prevented], `${row.key} の keydown は 1 回で、defaultPrevented が期待どおり`);
      assert.deepEqual(await lineTexts(page), [...before, ...row.lines], 'ログの行');
      assert.equal(await page.inputValue(INPUT), row.value, '欄の値');
      assert.equal(
        await page.evaluate(() => {
          const a = document.activeElement;
          return !a || a === document.body ? 'BODY' : a.getAttribute('data-testid');
        }),
        row.focus,
        '押した後のフォーカス先',
      );

      const s = summarize(rec);
      const detail = JSON.stringify(rec);
      assert.equal(s.csp, 0, detail);
      assert.equal(s.pageErrors, 0, detail);
      assert.equal(s.consoleErrors, 0, detail);
      await context.close();
    });
  });
}
