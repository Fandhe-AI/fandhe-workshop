// tab-focus.mjs（Tab の出入りの観察の共通の部品）の自己確認（TASK-373）。
//
// 固定の HTML を page.setContent で置くだけなので、サーバもビルドも要らない。
// 退出の判定 exitProblems と一巡の歩行 walkCycle（#2124）も、罠を入れた対照で検出できることを確かめる。
// 列挙の選び方（隠れた要素・disabled・tabindex=-1 を除く）と、押下ごとの位置・defaultPrevented の記録が正しいことを確かめる。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { launchBrowser, channelFromEnv } from './harness.mjs';
import {
  listFocusables, focusIndex, activeTestId, installTabProbe, pressTracked, focusAt, walkTracked, findCycleProblems, exitProblems, reverseProblems, completionProblems, walkCycle, OUT,
} from './tab-focus.mjs';

const HTML = `<!doctype html><body>
<button data-testid="a">a</button>
<textarea data-testid="b"></textarea>
<button data-testid="hidden" style="display:none">h</button>
<button data-testid="off" disabled>d</button>
<button data-testid="skip" tabindex="-1">s</button>
<button data-testid="c">c</button></body>`;

let browser;
before(async () => { browser = await launchBrowser({ channel: channelFromEnv() }); });
after(async () => { if (browser) await browser.close(); });

async function fresh() {
  const page = await browser.newPage();
  await page.setContent(HTML);
  await installTabProbe(page);
  return page;
}

test('列挙は見える・有効・tabindex が 0 以上の要素を DOM の順に返す', async () => {
  const page = await fresh();
  assert.deepEqual(await listFocusables(page), { ids: ['a', 'b', 'c'], positive: 0 });
  await page.close();
});

test('押す前後の位置と defaultPrevented が記録され、末尾の次は OUT になる', async () => {
  const page = await fresh();
  await listFocusables(page);
  await focusAt(page, 0);
  assert.equal(await activeTestId(page), 'a');
  assert.deepEqual(await pressTracked(page, 'Tab'), { before: 0, after: 1, prevented: [false] });
  await focusAt(page, 2);
  const out = await pressTracked(page, 'Tab');
  assert.equal(out.before, 2);
  assert.equal(await focusIndex(page), out.after);
  await page.close();
});

test('preventDefault する listener を足すと prevented: true が記録される', async () => {
  const page = await fresh();
  await listFocusables(page);
  await page.evaluate(() => {
    document.querySelector('[data-testid="a"]').addEventListener('keydown', (e) => e.preventDefault());
  });
  await focusAt(page, 0);
  assert.deepEqual(await pressTracked(page, 'Tab'), { before: 0, after: 0, prevented: [true] });
  await page.close();
});

test('先頭が入力欄のとき Shift+Tab は preventDefault されず OUT（body）へ出る', async () => {
  const page = await browser.newPage();
  await page.setContent('<!doctype html><body><textarea data-testid="t"></textarea><button data-testid="z">z</button></body>');
  await installTabProbe(page);
  await listFocusables(page);
  await focusAt(page, 0);
  assert.deepEqual(await pressTracked(page, 'Shift+Tab'), { before: 0, after: OUT, prevented: [false] });
  await page.close();
});

test('walkTracked は押下ごとの位置と全押下の prevented を返す', async () => {
  const page = await fresh();
  await listFocusables(page);
  await focusAt(page, 0);
  const r = await walkTracked(page, 'Tab', 4);
  assert.deepEqual(r.seq, [1, 2, OUT, 0]);
  assert.deepEqual(r.prevented, [false, false, false, false]);
  const back = await walkTracked(page, 'Shift+Tab', 2);
  assert.equal(back.seq.length, 2);
  assert.deepEqual(back.prevented, [false, false]);
  await page.close();
});

test('findCycleProblems は一巡と body の経由を許し、罠と食い違いを検出する', () => {
  assert.deepEqual(findCycleProblems([1, 0, 1, 0], 2, 0, 1), []);
  assert.deepEqual(findCycleProblems([1, OUT, 0, 1, OUT, 0], 2, 0, 1), []);
  assert.deepEqual(findCycleProblems([OUT, 1, 0, OUT, 1, 0], 2, 0, -1), []);
  assert.ok(findCycleProblems([1, 1, 1], 2, 0, 1).length > 0);
  assert.ok(findCycleProblems([1, OUT, OUT], 2, 0, 1).length > 0);
  assert.ok(findCycleProblems([2, 0, 1], 3, 0, 1).length > 0);
});

test('findCycleProblems を body から歩く始点（Tab は n-1・Shift+Tab は 0 で dir -1）で使える', () => {
  assert.deepEqual(findCycleProblems([0, 1, 2, 0, 1, 2], 3, 2, 1), []);
  assert.deepEqual(findCycleProblems([2, 1, 0, 2, 1, 0], 3, 0, -1), []);
  assert.ok(findCycleProblems([0, 0, 1, 2], 3, 2, 1).length > 0);
  assert.ok(findCycleProblems([2, 2, 1, 0], 3, 0, -1).length > 0);
  assert.ok(findCycleProblems([0, 1, 1, 2], 3, 2, 1).length > 0);
});

test('exitProblems は期待どおりの退出を受理し、食い違いを検出する', () => {
  const ok = { before: 1, after: 2, prevented: [false], value: 'x', expectedValue: 'x' };
  assert.deepEqual(exitProblems(ok), []);
  assert.ok(exitProblems({ ...ok, prevented: [true] }).length >= 1);
  assert.ok(exitProblems({ ...ok, prevented: [] }).length >= 1);
  assert.ok(exitProblems({ ...ok, after: 1 }).length >= 1);
  assert.ok(exitProblems({ ...ok, after: 3 }).length >= 1);
  assert.ok(exitProblems({ ...ok, after: OUT }).length >= 1);
  assert.ok(exitProblems({ ...ok, value: 'y' }).length >= 1);
  assert.ok(exitProblems({ ...ok, value: 'x\t', expectedValue: 'x\t' }).length >= 1);
});

test('exitProblems: 罠が無ければ [] で、Tab の握りつぶし（N1・N2 型）は検出される', async () => {
  const page = await fresh();
  await listFocusables(page);
  await focusAt(page, 1);
  const clean = await pressTracked(page, 'Tab');
  assert.deepEqual(exitProblems({ ...clean, value: '', expectedValue: '' }), []);
  await page.close();
  for (const refocus of [false, true]) {
    const trapped = await fresh();
    await listFocusables(trapped);
    await trapped.evaluate((r) => {
      document.querySelector('[data-testid="b"]').addEventListener('keydown', (e) => {
        if (e.key !== 'Tab') return;
        e.preventDefault();
        if (r) document.querySelector('[data-testid="b"]').focus();
      });
    }, refocus);
    await focusAt(trapped, 1);
    const r = await pressTracked(trapped, 'Tab');
    assert.ok(exitProblems({ ...r, value: '', expectedValue: '' }).length >= 1, JSON.stringify(r));
    await trapped.close();
  }
});

test('walkCycle は一巡を受理し、前提（require・key）の不足は reject する', async () => {
  for (const key of ['Tab', 'Shift+Tab']) {
    const page = await fresh();
    await listFocusables(page);
    await focusAt(page, 0);
    const r = await walkCycle(page, key);
    assert.deepEqual(r.problems, [], JSON.stringify(r));
    assert.equal(r.n, 3);
    assert.equal(r.seq.length, 8);
    assert.equal(r.start, 0);
    await page.close();
  }
  const page = await fresh();
  await assert.rejects(walkCycle(page, 'Tab', { require: ['nope'] }));
  await assert.rejects(walkCycle(page, 'Enter'));
  await page.close();
});

test('walkCycle は body から歩く（fromBody）。フォーカスが body に無ければ reject する', async () => {
  // fill・focus を挟まない新しいページで歩く（blur の後も Chromium は直前の位置から Tab を始めるため）。
  for (const key of ['Tab', 'Shift+Tab']) {
    const page = await fresh();
    const r = await walkCycle(page, key, { fromBody: true });
    assert.deepEqual(r.problems, [], JSON.stringify(r));
    assert.equal(r.start, key === 'Tab' ? 2 : 0);
    await page.close();
  }
  const page = await fresh();
  await page.focus('[data-testid="a"]');
  await assert.rejects(walkCycle(page, 'Tab', { fromBody: true }));
  await page.close();
});

test('walkCycle: 罠（Tab の握りつぶし・先頭へ戻す）は problems か prevented で検出される', async () => {
  for (const refocus of [false, true]) {
    const page = await fresh();
    await page.evaluate((r) => {
      document.addEventListener('keydown', (e) => {
        if (e.key === 'Tab' && e.target.getAttribute('data-testid') === 'c') {
          e.preventDefault();
          if (r) document.querySelector('[data-testid="a"]').focus();
        }
      }, true);
    }, refocus);
    await page.focus('[data-testid="a"]');
    const r = await walkCycle(page, 'Tab');
    assert.ok(r.problems.length >= 1 || r.prevented.some((p) => p === true), JSON.stringify(r));
    await page.close();
  }
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

test('completionProblems は補完の 1 押下を受理し、食い違いを拒否する（TASK-451）', () => {
  const ok = { before: 3, after: 3, prevented: [true], value: '/help', expectedValue: '/help' };
  assert.deepEqual(completionProblems(ok), []);
  assert.ok(completionProblems({ ...ok, prevented: [false] }).length > 0);
  assert.ok(completionProblems({ ...ok, prevented: [] }).length > 0);
  assert.ok(completionProblems({ ...ok, prevented: [true, true] }).length > 0);
  assert.ok(completionProblems({ ...ok, after: 4 }).length > 0);
  assert.ok(completionProblems({ ...ok, after: OUT }).length > 0);
  assert.ok(completionProblems({ ...ok, value: '/he' }).length > 0);
  assert.ok(completionProblems({ ...ok, value: '/help\t', expectedValue: '/help\t' }).length > 0);
});
