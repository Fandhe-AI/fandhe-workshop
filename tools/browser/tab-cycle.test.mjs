// Tab によるフォーカスの一巡と、行き止まり（Tab の罠）が 0 件であることの試験（TASK-278 3/3。JUDGE-19・A11Y-16・TERM-10・K-15）。
//
// 入力欄から Tab を繰り返すと、画面上のフォーカスできる全要素に 1 回ずつ移り、入力欄へ戻る。
// Shift+Tab でも逆順に一巡する。文書の外（body）へ一度出ることは罠に数えない（K-15 が許す）。
// 候補が無い画面なので、一巡の間の Tab は preventDefault されない（FEND-21）。
// 要素の数は固定せず、生きている DOM から列挙する。要素が増えても試験が素通りしないため。
// 候補の一覧が開いている画面・部品がそろった画面での再確認は TASK-451 の担当。
// 対照の自己確認として、罠を試験のページの中だけに入れ、判定が検出できることを確かめる（配信物には残さない）。
//
// 入力: FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-tab-cycle/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const OUT = 'OUT';

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

// フォーカスできる要素を DOM の順に列挙して window に保持し、各要素の testid の一覧を返す。DOM には属性を足さない。
async function enumerate(page) {
  return page.evaluate(() => {
    const els = [...document.querySelectorAll('a[href], button, textarea, input, select, [tabindex]')]
      .filter((e) => e.tabIndex >= 0 && !e.disabled && e.getClientRects().length > 0);
    window.__tabCycle = els;
    return {
      ids: els.map((e) => e.getAttribute('data-testid') ?? e.tagName),
      positive: els.filter((e) => e.tabIndex > 0).length,
    };
  });
}

async function position(page) {
  return page.evaluate(() => {
    const a = document.activeElement;
    if (!a || a === document.body) return 'OUT';
    return window.__tabCycle.indexOf(a);
  });
}

// key を presses 回押し、押すたびの位置と defaultPrevented の列を返す。
async function walk(page, key, presses) {
  await page.evaluate(() => {
    window.__tabPrevented = [];
    window.addEventListener('keydown', (e) => {
      if (e.key === 'Tab') window.__tabPrevented.push(e.defaultPrevented);
    });
  });
  const seq = [];
  for (let i = 0; i < presses; i += 1) {
    await page.keyboard.press(key);
    seq.push(await position(page));
  }
  return { seq, prevented: await page.evaluate(() => window.__tabPrevented) };
}

// 位置の列 seq（始点 start から dir 向きに歩いた結果）の問題の一覧を返す。空なら一巡して罠が無い。
export function findCycleProblems(seq, n, start, dir) {
  const problems = [];
  let cur = start;
  let prevOut = false;
  seq.forEach((pos, i) => {
    if (pos === OUT) {
      if (prevOut) problems.push(`press ${i + 1}: stuck outside the document`);
      prevOut = true;
      return;
    }
    prevOut = false;
    const expected = (((cur + dir) % n) + n) % n;
    if (pos === -1) problems.push(`press ${i + 1}: focus moved to an unlisted element`);
    else if (pos === cur) problems.push(`press ${i + 1}: focus did not move (trap at ${pos})`);
    else if (pos !== expected) problems.push(`press ${i + 1}: expected ${expected} but got ${pos}`);
    cur = pos;
  });
  const first = seq.slice(0, n + 1);
  if (!first.includes(start)) problems.push(`did not return to start within ${n + 1} presses`);
  for (let k = 0; k < n; k += 1) {
    if (!first.includes(k)) problems.push(`element ${k} was not visited in the first cycle`);
  }
  return problems;
}

async function cycle(page, key, dir) {
  const { ids, positive } = await enumerate(page);
  assert.ok(ids.length >= 2, JSON.stringify(ids));
  assert.ok(ids.includes('draft-input') && ids.includes('submit-input'), JSON.stringify(ids));
  assert.equal(positive, 0, 'tabindex > 0 makes DOM order differ from Tab order');
  await page.focus(INPUT);
  const start = await position(page);
  assert.equal(ids[start], 'draft-input');
  const n = ids.length;
  const { seq, prevented } = await walk(page, key, 2 * (n + 1));
  return { ids, n, start, seq, prevented, problems: findCycleProblems(seq, n, start, dir) };
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const scenes = [
    ['入力欄が空のとき、Tab で 2 周しても罠が無い', 'Tab', 1, async () => {}],
    ['入力欄に文字があるとき、Tab で 2 周しても罠が無く、値が変わらない', 'Tab', 1, async (page) => { await page.fill(INPUT, 'hello'); }],
    ['送信して一覧が増えた後も、Tab で 2 周しても罠が無い', 'Tab', 1, async (page) => {
      for (let i = 0; i < 2; i += 1) {
        await page.fill(INPUT, `cmd${i}`);
        await page.click(SUBMIT);
      }
      assert.equal(await page.locator('li').count() >= 2, true);
    }],
    ['Shift+Tab で 2 周しても逆順に罠が無い', 'Shift+Tab', -1, async () => {}],
  ];
  for (const [name, key, dir, setup] of scenes) {
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
        const r = await cycle(page, key, dir);
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
      const r = await cycle(page, 'Tab', 1);
      // 位置の列だけでは正常な折り返しに見える罠は、preventDefault の記録で検出される。
      assert.ok(r.problems.length >= 1 || r.prevented.some((p) => p === true), JSON.stringify(r));
      await context.close();
    }));
  });
}

test('findCycleProblems は一巡と body の経由を許し、罠と食い違いを検出する', () => {
  assert.deepEqual(findCycleProblems([1, 0, 1, 0], 2, 0, 1), []);
  assert.deepEqual(findCycleProblems([1, OUT, 0, 1, OUT, 0], 2, 0, 1), []);
  assert.deepEqual(findCycleProblems([OUT, 1, 0, OUT, 1, 0], 2, 0, -1), []);
  assert.ok(findCycleProblems([1, 1, 1], 2, 0, 1).length > 0);
  assert.ok(findCycleProblems([1, OUT, OUT], 2, 0, 1).length > 0);
  assert.ok(findCycleProblems([2, 0, 1], 3, 0, 1).length > 0);
});
