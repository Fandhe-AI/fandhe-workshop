// Shift+Tab の逆向き移動と、キーボードの罠（フォーカスの閉じ込め）が 0 件であることの試験（TASK-373 2/2。A11Y-16・JUDGE-19。PoC-2 の K-02・K-04・K-15）。
//
// 試験用の口が無い製品の画面（教材の IR を同梱した配信する設定の `?start=s-1-1`）で固定する。Shift+Tab は補完にならず、
// preventDefault もされず、ブラウザの通常の逆向きの移動になる（JUDGE-19）。入力欄が画面の最初の操作対象なら文書の外（body）へ出る。
//   R1 送信ボタンで Shift+Tab → 入力欄（K-02）。R2 一覧が開いた欄で Shift+Tab → 補完せず DOM 順で 1 つ前（K-04）。
//   R3 一覧が閉じた 4 状態の欄で Shift+Tab → 1 つ前。R4 欄が最初の操作対象のとき Shift+Tab → body。
//   C1 先頭から Tab・C2 先頭から Shift+Tab で全要素を通り行き止まりが無い（K-15）。C2b 一覧が開いた欄からの Shift+Tab の一巡。
//   C3 開始点ごとに C1・C2（入力欄の無い Step でも成り立つ）。
// Tab の出入り（K-01・K-03・K-05・K-06・K-08）は tab-exit.test.mjs、K-07（変換中の Tab）は candidates.test.mjs（合成の isComposing）と
// ime-input.test.mjs の I-4（CDP の模擬）が担当する。対応表は docs/testing.md 5.497。
// 範囲外: 一覧が開いた状態の Tab（順方向）の一巡（補完が挟まる。TASK-451）・幅 959px 以下（shell-boundary）・実 IME と人の確認（TASK-93・TASK-386）。
// 対照として、罠と配置の変更を試験のページの中だけに入れ、判定が検出することを確かめる（配信物には残さない）。
// R4 の tabindex=-1 の付与は、入力欄を最初の操作対象にするための試験のページの中の配置の変更であり、観察の口ではない。
//
// 入力: FANDHE_WEB_AUTHOR_DIR（IR 同梱の配信する設定）・FANDHE_AUTHOR_IR（C3 の開始点。その IR を compile した出力）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import {
  OUT, listFocusables, installTabProbe, pressTracked, walkCycle, focusAt,
} from './tab-focus.mjs';
import { startLinksOf } from './author-preview.mjs';

const BASE = '/fandhe-tab-trap/';
const INPUT = '[data-testid="draft-input"]';
const LIST = '[data-testid="candidate-list"]';
const LOG_ITEMS = '[data-testid="log-list"] li';

let browser;
let ir;

before(async () => {
  const dir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(
    dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(),
    'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS',
  );
  const irPath = process.env.FANDHE_AUTHOR_IR;
  assert.ok(irPath && fs.statSync(irPath, { throwIfNoEntry: false })?.isFile(), 'FANDHE_AUTHOR_IR must point to the compiled IR');
  ir = JSON.parse(fs.readFileSync(irPath, 'utf8'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 欄で Shift+Tab を 1 回押したときの問題の一覧を返す。空なら期待どおり（preventDefault なし・expectedAfter へ移動・値が不変）。
export function reverseProblems({ before: from, after: to, prevented, value, expectedValue, expectedAfter }) {
  const problems = [];
  if (prevented.length !== 1 || prevented[0] !== false) problems.push(`Shift+Tab was prevented or not seen: ${JSON.stringify(prevented)}`);
  if (typeof from !== 'number') problems.push(`focus did not start on a listed element: ${from}`);
  else if (to !== expectedAfter) problems.push(`focus did not move to ${expectedAfter}: ${from} -> ${to}`);
  if (value !== expectedValue) problems.push('draft value changed (completion?)');
  if (value.includes('\t')) problems.push('tab character entered the draft');
  return problems;
}

async function open(srv, start = 's-1-1') {
  const opened = await openPage(browser, { origin: srv.origin, viewport: { width: 1024, height: 768 } });
  await opened.page.goto(`${srv.url}?start=${encodeURIComponent(start)}`);
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

// 値を入れて欄にフォーカスし、一覧の開閉が期待どおりであることを前提として照合する。列挙と欄の位置を返す。
async function arm(page, value, listOpen) {
  await page.fill(INPUT, value);
  await page.focus(INPUT);
  assert.equal(await listHidden(page), !listOpen, `candidate list ${listOpen ? 'should be open' : 'should be closed'} for ${value}`);
  const lst = await listFocusables(page);
  const idx = lst.ids.indexOf('draft-input');
  assert.ok(idx >= 1, `draft-input must be preceded by a focusable: ${lst.ids.join(',')}`);
  return { ...lst, idx };
}

// 先頭から（body にフォーカスがある状態から）key を 2 周ぶん押した結果を返す。start は findCycleProblems の始点（body の 1 つ手前）。
const walkFromTop = (page, key) => walkCycle(page, key, { fromBody: true });

const REVERSE_SCENES = [
  { id: 'R3a 空欄', value: '' },
  { id: 'R3b 候補なし（/zzz）', value: '/zzz' },
  { id: 'R3c 完全一致（/help）', value: '/help' },
  { id: 'R3d Esc で閉じた後（/he）', value: '/he', esc: true },
];

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_AUTHOR_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  test(`R1 送信ボタンで Shift+Tab: 入力欄へ戻り preventDefault されない（K-02）（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const { ids, idx } = await arm(page, 'abc', false);
      const sub = ids.indexOf('submit-input');
      assert.equal(sub, idx + 1, `submit must follow the field: ${ids.join(',')}`);
      await focusAt(page, sub);
      const r = await pressTracked(page, 'Shift+Tab');
      assert.deepEqual(reverseProblems({ ...r, value: await draftValue(page), expectedValue: 'abc', expectedAfter: idx }), []);
      assertClean(rec);
      await context.close();
    });
  });

  test(`R2 一覧が開いた欄で Shift+Tab: 補完せず DOM 順で 1 つ前へ移る（K-04）（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const { ids, idx } = await arm(page, '/he', true);
      const logs = await logCount(page);
      const r = await pressTracked(page, 'Shift+Tab');
      const hiddenAfter = await listHidden(page);
      console.log(`R2 csp=${cspLabel} ids=${ids.join(',')} before=${r.before} after=${r.after} listHiddenAfter=${hiddenAfter}`);
      assert.deepEqual(reverseProblems({ ...r, value: await draftValue(page), expectedValue: '/he', expectedAfter: idx - 1 }), []);
      assert.equal(await logCount(page), logs, 'Shift+Tab must not add log rows');
      assertClean(rec);
      await context.close();
    });
  });

  for (const scene of REVERSE_SCENES) {
    test(`${scene.id}: 一覧が閉じた欄で Shift+Tab は 1 つ前へ移る（CSP ${cspLabel}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv);
        let armed = await arm(page, scene.value, Boolean(scene.esc));
        if (scene.esc) {
          await page.keyboard.press('Escape');
          assert.equal(await listHidden(page), true, 'Esc closes the list');
          await page.focus(INPUT);
          armed = { ...armed, ...(await listFocusables(page)) };
        }
        const logs = await logCount(page);
        const r = await pressTracked(page, 'Shift+Tab');
        assert.equal(r.before, armed.idx);
        assert.deepEqual(reverseProblems({ ...r, value: await draftValue(page), expectedValue: scene.value, expectedAfter: armed.idx - 1 }), []);
        assert.equal(await logCount(page), logs);
        assertClean(rec);
        await context.close();
      });
    });
  }

  // 欄より前の操作対象を tabindex=-1 にして、欄を画面の最初の操作対象にする（試験のページの中だけの配置の変更）。
  for (const [label, value, listOpen] of [['空欄', '', false], ['一覧が開いた状態（/he）', '/he', true]]) {
    test(`R4 欄が最初の操作対象のとき Shift+Tab は body へ出る: ${label}（CSP ${cspLabel}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv);
        const { idx } = await arm(page, value, listOpen);
        await page.evaluate((i) => window.__tabFocusables.slice(0, i).forEach((e) => e.setAttribute('tabindex', '-1')), idx);
        await page.focus(INPUT);
        const lst = await listFocusables(page);
        assert.equal(lst.ids.indexOf('draft-input'), 0, `field must be first: ${lst.ids.join(',')}`);
        const r = await pressTracked(page, 'Shift+Tab');
        assert.deepEqual(reverseProblems({ ...r, value: await draftValue(page), expectedValue: value, expectedAfter: OUT }), []);
        assertClean(rec);
        await context.close();
      });
    });
  }

  for (const [id, key] of [['C1 先頭から Tab', 'Tab'], ['C2 先頭から Shift+Tab', 'Shift+Tab']]) {
    test(`${id}: 全要素を通り行き止まりが無い（K-15）（CSP ${cspLabel}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv);
        // fill や focus を挟むと、blur の後も Chromium は直前の位置から Tab を始めるため、開いた直後の状態のまま歩く。
        assert.equal(await draftValue(page), '');
        const r = await walkFromTop(page, key);
        const detail = JSON.stringify(r);
        assert.ok(r.ids.includes('draft-input') && r.ids.includes('submit-input'), detail);
        assert.deepEqual(r.problems, [], detail);
        assert.ok(r.prevented.length >= r.seq.length && r.prevented.every((p) => p === false), detail);
        assert.equal(await draftValue(page), '');
        console.log(`# ${id} CSP ${cspLabel} ids=${JSON.stringify(r.ids)} seq=${JSON.stringify(r.seq)}`);
        assertClean(rec);
        await context.close();
      });
    });
  }

  test(`C2b 一覧が開いた欄からの Shift+Tab の一巡: 罠が無く補完されない（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const { idx } = await arm(page, '/he', true);
      const { ids, start, seq, prevented, problems } = await walkCycle(page, 'Shift+Tab');
      assert.equal(start, idx);
      const detail = JSON.stringify({ ids, seq, prevented, problems });
      console.log(`# C2b csp=${cspLabel} ${detail}`);
      assert.deepEqual(problems, [], detail);
      assert.ok(prevented.length >= seq.length && prevented.every((p) => p === false), detail);
      assert.equal(await draftValue(page), '/he');
      assertClean(rec);
      await context.close();
    });
  });
}

// C3: 開始点ごとの罠 0 件。入力欄の無い Step でも成り立つ判定にする（欄の存在を前提にしない）。
test('C3 全開始点で先頭からの両方向の一巡に罠が無い（CSP あり）', async () => {
  const links = startLinksOf(ir);
  assert.ok(links.length >= 1);
  await withCspMode({ dir: process.env.FANDHE_WEB_AUTHOR_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    for (const link of links) {
      const { page, context, rec } = await open(srv, link.id);
      for (const key of ['Tab', 'Shift+Tab']) {
        const r = await walkFromTop(page, key);
        const detail = JSON.stringify({ start: link.id, key, ...r });
        assert.deepEqual(r.problems, [], detail);
        assert.ok(r.prevented.length >= r.seq.length && r.prevented.every((p) => p === false), detail);
      }
      const { ids } = await listFocusables(page);
      const uiRows = await page.evaluate(() => document.querySelectorAll('[data-action="ui"]').length);
      console.log(`# C3 start=${link.id} n=${ids.length} draft=${ids.includes('draft-input')} choice=${ids.includes('choice-btn')} uiRows=${uiRows} ids=${JSON.stringify(ids)}`);
      assertClean(rec);
      await context.close();
    }
  }));
});

// 対照: 罠と配置の変更を試験のページの中だけに入れ、判定が検出することを確かめる（配信物には残さない）。
const KEYDOWN_TRAPS = [
  { id: 'N1 欄の Shift+Tab を preventDefault', refocus: false },
  { id: 'N2 欄の Shift+Tab を preventDefault して欄へ focus を戻す', refocus: true },
];
for (const trap of KEYDOWN_TRAPS) {
  test(`${trap.id}: reverseProblems が検出する（CSP あり）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_AUTHOR_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context } = await open(srv);
      const { idx } = await arm(page, '/he', true);
      await page.evaluate((refocus) => {
        document.querySelector('[data-testid="draft-input"]').addEventListener('keydown', (e) => {
          if (e.key !== 'Tab' || !e.shiftKey) return;
          e.preventDefault();
          if (refocus) e.currentTarget.focus();
        });
      }, trap.refocus);
      const r = await pressTracked(page, 'Shift+Tab');
      const problems = reverseProblems({ ...r, value: await draftValue(page), expectedValue: '/he', expectedAfter: idx - 1 });
      assert.ok(problems.length >= 1, `trap must be detected: ${JSON.stringify(r)}`);
      await context.close();
    }));
  });
}

test('N3 送信ボタンの Shift+Tab を捕まえる罠: 先頭からの一巡が検出する（CSP あり）', async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_AUTHOR_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.evaluate(() => {
      document.addEventListener('keydown', (e) => {
        if (e.key === 'Tab' && e.shiftKey && e.target.getAttribute('data-testid') === 'submit-input') e.preventDefault();
      }, true);
    });
    const r = await walkFromTop(page, 'Shift+Tab');
    assert.ok(r.problems.length >= 1 || r.prevented.some((p) => p === true), JSON.stringify(r));
    await context.close();
  }));
});

test('N4 欄が最初の操作対象で Shift+Tab を捕まえる罠: body へ出ないことを検出する（CSP あり）', async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_AUTHOR_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    const { idx } = await arm(page, '', false);
    await page.evaluate((i) => window.__tabFocusables.slice(0, i).forEach((e) => e.setAttribute('tabindex', '-1')), idx);
    await page.focus(INPUT);
    await listFocusables(page);
    await page.evaluate(() => {
      document.querySelector('[data-testid="draft-input"]').addEventListener('keydown', (e) => {
        if (e.key === 'Tab' && e.shiftKey) { e.preventDefault(); e.currentTarget.focus(); }
      });
    });
    const r = await pressTracked(page, 'Shift+Tab');
    const problems = reverseProblems({ ...r, value: await draftValue(page), expectedValue: '', expectedAfter: OUT });
    assert.ok(problems.length >= 1, JSON.stringify(r));
    await context.close();
  }));
});

test('reverseProblems は期待どおりを受理し、食い違いを拒否する', () => {
  const ok = { before: 5, after: 4, prevented: [false], value: '/he', expectedValue: '/he', expectedAfter: 4 };
  assert.deepEqual(reverseProblems(ok), []);
  assert.deepEqual(reverseProblems({ ...ok, before: 0, after: OUT, expectedAfter: OUT }), []);
  assert.ok(reverseProblems({ ...ok, prevented: [true] }).length > 0);
  assert.ok(reverseProblems({ ...ok, prevented: [] }).length > 0);
  assert.ok(reverseProblems({ ...ok, after: 5 }).length > 0);
  assert.ok(reverseProblems({ ...ok, after: OUT }).length > 0);
  assert.ok(reverseProblems({ ...ok, value: '/help' }).length > 0);
  assert.ok(reverseProblems({ ...ok, value: '/he\t', expectedValue: '/he\t' }).length > 0);
});
