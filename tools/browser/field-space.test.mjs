// 入力欄の Space の受け持ちの試験（TASK-352 1/3。FEND-7・D-42）。
//
// 入力欄の Enter・Tab・Space の状態依存の preventDefault は、属性化も上流の口も採らず glue が受け持つ（D-42）。
// Enter・Tab は key-invariants が固定している。Space は判定が Nothing で、glue は何もせず、
// textarea の既定動作で空白が入る。ここでは「Space の keydown は preventDefault されない・送信されない・空白が入る」を固定する。
//
// key-invariants に行を足さない理由: TASK-351 が固定した基準の件数（20 件）を保ち、TASK-352 3/3 が
// 置き換えの前後で同じ件数と比べられるようにするため。観察の方法は key-invariants と同じ
// （内部の印は読まず、window の捕捉の段で保持した keydown の Event の defaultPrevented と DOM だけを見る）。
// 一覧が開いている行（SPACE-OPEN）は試験の設定（web: 'test'）で観察する。押した後の一覧の開閉は
// 候補の絞り込みの担当（candidates）で、キーの受け持ちとは別のため照合しない。
// 限界: 修飾キー付き Space・変換中の Space（ime-input の担当）・OS ごとの差（D-48）は対象外。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-field-space/';
const INPUT = '[data-testid="draft-input"]';
const ITEMS = '[data-testid="log-list"] > li';
const LIST = '[data-testid="candidate-list"]';

const EXPECTED_ROWS = 3;
const ROWS = [
  { id: 'SPACE-TEXT', web: 'delivery', open: false, draft: 'x', value: 'x ' },
  { id: 'SPACE-EMPTY', web: 'delivery', open: false, draft: '', value: ' ' },
  { id: 'SPACE-OPEN', web: 'test', open: true, draft: '/he', value: '/he ' },
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
  test(`${row.id}: Space（下書き ${JSON.stringify(row.draft)}）`, async () => {
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
      await page.keyboard.press('Space');
      await page.waitForFunction(([sel, v]) => document.querySelector(sel).value === v, [INPUT, row.value]);
      await settle(page);

      const seen = await page.evaluate(() => window.__keyEvents
        .filter((ev) => ev.key === ' ')
        .map((ev) => ev.defaultPrevented));
      assert.deepEqual(seen, [false], 'Space の keydown は 1 回で、preventDefault されない');
      assert.deepEqual(await lineTexts(page), before, '送信されずログの行は増えない');
      assert.equal(await page.inputValue(INPUT), row.value, '空白が欄に入る');
      assert.equal(
        await page.evaluate(() => {
          const a = document.activeElement;
          return !a || a === document.body ? 'BODY' : a.getAttribute('data-testid');
        }),
        'draft-input',
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
