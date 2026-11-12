// Tab によるフォーカスの一巡と、行き止まり（Tab の罠）が 0 件であることの試験（TASK-278 3/3。JUDGE-19・A11Y-16・TERM-10・K-15）。
//
// 入力欄から Tab を繰り返すと、画面上のフォーカスできる全要素に 1 回ずつ移り、入力欄へ戻る。
// Shift+Tab でも逆順に一巡する。文書の外（body）へ一度出ることは罠に数えない（K-15 が許す）。
// 候補が無い画面なので、一巡の間の Tab は preventDefault されない（FEND-21）。
// 要素の数は固定せず、生きている DOM から列挙する。要素が増えても試験が素通りしないため。
// 候補の一覧が開いている画面・部品がそろった画面での再確認は tab-parts.test.mjs（TASK-451）。
// 一巡の歩行と判定（walkCycle・findCycleProblems）と列挙・押下の部品は tab-focus.mjs に集約した（単体の試験は tab-focus.test.mjs）。
// 対照の自己確認として、罠を試験のページの中だけに入れ、判定が検出できることを確かめる（配信物には残さない）。
//
// 入力: FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import {
  installTabProbe, walkCycle,
} from './tab-focus.mjs';

const BASE = '/fandhe-tab-cycle/';
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

async function cycle(page, key) {
  await page.focus(INPUT);
  await installTabProbe(page);
  const r = await walkCycle(page, key, { require: ['draft-input', 'submit-input'] });
  assert.equal(r.ids[r.start], 'draft-input');
  return r;
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const scenes = [
    ['入力欄が空のとき、Tab で 2 周しても罠が無い', 'Tab', async () => {}],
    ['入力欄に文字があるとき、Tab で 2 周しても罠が無く、値が変わらない', 'Tab', async (page) => { await page.fill(INPUT, 'hello'); }],
    ['送信して一覧が増えた後も、Tab で 2 周しても罠が無い', 'Tab', async (page) => {
      for (let i = 0; i < 2; i += 1) {
        await page.fill(INPUT, `cmd${i}`);
        await page.click(SUBMIT);
      }
      assert.equal(await page.locator('li').count() >= 2, true);
    }],
    ['Shift+Tab で 2 周しても逆順に罠が無い', 'Shift+Tab', async () => {}],
  ];
  for (const [name, key, setup] of scenes) {
    test(`${name}（CSP ${cspLabel}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir0) => withServer({ dir: dir0, base: BASE }, async (srv) => {
        const { page, context, rec } = await open(srv);
        await setup(page);
        // 送信の後の欄の書き戻しは非同期のため、空になってから基準を読む（揺れを避ける）。
        await page.waitForFunction((sel) => {
          const el = document.querySelector(sel);
          return el.value === '' || document.querySelectorAll('li').length === 0;
        }, INPUT);
        const before0 = await page.inputValue(INPUT);
        const r = await cycle(page, key);
        const detail = JSON.stringify(r);
        assert.deepEqual(r.problems, [], detail);
        assert.ok(r.prevented.length >= r.seq.length, detail);
        assert.ok(r.prevented.every((p) => p === false), detail);
        assert.equal(await page.inputValue(INPUT), before0);
        // 観察の記録: 末尾から先頭へ戻るとき body を経由するかは決め打ちせず、列を残す。
        console.log(`# ${key} CSP ${cspLabel} ids=${JSON.stringify(r.ids)} seq=${JSON.stringify(r.seq)}`);
        assertClean(rec);
        await context.close();
      }));
    });
  }
}

// 対照: 罠を入れると判定が検出する。罠はこの試験のページの中だけに入れる。
const traps = [
  ['送信ボタンの Tab を preventDefault する', () => {
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Tab' && e.target.getAttribute('data-testid') === 'submit-input') e.preventDefault();
    }, true);
  }],
  ['送信ボタンの Tab で入力欄へ戻す', () => {
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Tab' && e.target.getAttribute('data-testid') === 'submit-input') {
        e.preventDefault();
        document.querySelector('[data-testid="draft-input"]').focus();
      }
    }, true);
  }],
];
for (const [name, install] of traps) {
  test(`対照: ${name}と罠として検出される（CSP あり）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context } = await open(srv);
      await page.evaluate(install);
      const r = await cycle(page, 'Tab');
      // 位置の列だけでは正常な折り返しに見える罠は、preventDefault の記録で検出される。
      assert.ok(r.problems.length >= 1 || r.prevented.some((p) => p === true), JSON.stringify(r));
      await context.close();
    }));
  });
}
