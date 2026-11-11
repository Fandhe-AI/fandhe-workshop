// キーボードだけの自動操作の部品と、マウスの click が 0 回であることの計数の試験（TASK-92 1/2。BUILD-17）。
//
// 確かめること:
//   U1〜U3（ブラウザなし）: 許すキーの集合に閉じたキーボード・ドライバ側の監視・教材テストの 1 操作の写し（playStep）の単体。
//   K1  試験の設定（`?fixture=ch0-3-chapters`）で、Tab・Enter・Space・Shift+Enter・Esc と文字だけで 20 Step を最後まで通す（CSP あり・なし）。
//       ページ内のマウスのイベント 0・許さないキー 0・ドライバ側の監視 0・5 つのキーがそれぞれ 1 回以上・raw-trigger の click 0・
//       エンジンの警告と無視 0・安全の計数 0・配信元以外への通信 0。
//   K2  `?fixture=ch4-9`（CH-4〜CH-9 の形の 17 Step）を同じ条件で通す（CSP あり）。承認・対象つきの「直す」・カード・差分の行・
//       「次の章へ進む」5 回をキーボードで初めて通す。この通しは Space・Esc を使わないので、必須のキーは Tab・Enter・Shift+Enter。
//   N1〜N5 負の対照: 実マウスの click が計数に出る・監視が click を数える・制限なしの矢印キーが禁止として出る・
//       再読み込みの後も累計が消えない・幅 959px の tablist で止まる。
//
// 確かめないこと: 教材 repo の全周（CH-0〜CH-15）と pt-ch0-15 の読み込み・期待の判定（2/2）。信頼されない click（detail 0）の計数
// （glue の正規の経路と見分けられない）。幅 960px 未満の tablist の切り替え（矢印キーが要る）。出力に入力した値は出さない。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages,
} from './harness.mjs';
import {
  engine, ledger, journey, install, RAW,
} from './ch0-3-journey.mjs';
import {
  FIXTURE_QUERY as FIXTURE_49, STEPS, LAST, journey49, hookObserver49,
} from './ch4-9-journey.mjs';
import {
  ALLOWED_KEYS, restrictedKeyboard, installInputCounter, guardPage, keyOnlyDriver, playStep, assertNoTablist,
} from './keyboard-only.mjs';

const BASE = '/fandhe-keyboard-only/';
let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 偽の page。押されたキーと入力を記録する。
function fakePage() {
  const calls = [];
  const page = {
    keyboard: { press: async (k) => calls.push(['press', k]), type: async (t) => calls.push(['type', t]) },
    click: async () => calls.push(['click']),
    fill: async () => calls.push(['fill']),
    focus: async () => calls.push(['focus']),
    evaluate: async () => 0,
    mouse: { click: async () => calls.push(['mouse.click']) },
    locator: () => ({ click: async () => calls.push(['loc.click']), count: async () => 0 }),
  };
  return { page, calls };
}

test('U1: restrictedKeyboard は許す 5 つのキーだけを押せる', async () => {
  const { page, calls } = fakePage();
  const kb = restrictedKeyboard(page);
  for (const k of ALLOWED_KEYS) await kb.press(k);
  assert.deepEqual(calls.map((c) => c[1]), [...ALLOWED_KEYS]);
  assert.deepEqual(Object.values(kb.used), [1, 1, 1, 1, 1]);
  for (const bad of ['ArrowRight', 'ControlOrMeta+A', 'Shift+Tab', 'Backspace', 'Home', 'Alt+Enter', 'a']) {
    await assert.rejects(() => kb.press(bad), /許さないキー/, bad);
  }
  assert.equal(calls.length, ALLOWED_KEYS.length, '例外のキーは page に届かない');
  await kb.type('abc /x');
  for (const bad of ['a\nb', 'a\tb', 'a\u0007']) await assert.rejects(() => kb.type(bad), /制御文字/);
  assert.equal(calls.length, ALLOWED_KEYS.length + 1);
  console.log('U1: 通る 5 つ・例外 7 つ・type 3 つ');
});

test('U2: guardPage は要素を指す呼び出しを数え、観察は数えない', async () => {
  const { page, calls } = fakePage();
  const g = guardPage(page);
  await g.page.click('x');
  await g.page.fill('x', 'y');
  await g.page.focus('x');
  await g.page.mouse.click(1, 2);
  await g.page.locator('x').click();
  assert.equal(g.log.calls, 5);
  assert.deepEqual(Object.keys(g.log.names).sort(), ['click', 'fill', 'focus', 'mouse.click']);
  assert.equal(g.log.names.click, 2);
  const before = g.log.calls;
  await g.page.evaluate(() => 0);
  assert.equal(await g.page.locator('x').count(), 0);
  assert.equal(g.log.calls, before, '観察は数えない');
  assert.equal(g.page.keyboard.restricted, true, 'keyboard は制限つきに差し替わる');
  await assert.rejects(() => g.page.keyboard.press('ArrowDown'), /許さないキー/);
  assert.equal(calls.length, 5);
  console.log('U2: 指す呼び出し 5 つを数え、観察 2 つは数えない');
});

test('U3: playStep は 7 種の入力をドライバの操作に写す', async () => {
  const seen = [];
  const rec = (n) => async (...a) => seen.push([n, ...a]);
  const d = {
    send: rec('send'), sendMultiline: rec('sendMultiline'), choose: rec('choose'), point: rec('point'),
    decide: rec('decide'), revise: rec('revise'), esc: rec('esc'),
  };
  const kind = (v) => (v === 'docs' ? 'document' : undefined);
  await playStep(d, { cmd: '/help' }, { viewKind: kind });
  await playStep(d, { text: 'a\nb' });
  await playStep(d, { choice: 'opt-a' });
  await playStep(d, { ui: 'docs cards/notice' }, { viewKind: kind });
  await playStep(d, { approval: 'approve' });
  await playStep(d, { approval: 'revise', target: 't-1' });
  await playStep(d, { cancel: true });
  await playStep(d, { tick: true });
  await playStep(d, { cmd: 'x', expect: {} });
  assert.deepEqual(seen, [
    ['send', '/help'], ['sendMultiline', 'a', 'b'], ['choose', 'opt-a'], ['point', 'cards/notice', 'document'],
    ['decide', 'approve'], ['revise', 't-1'], ['esc'], ['send', 'x'],
  ]);
  await assert.rejects(() => playStep(d, { unknown: 1 }), /unknown play step key/);
  await assert.rejects(() => playStep(d, { cmd: 'a', choice: 'b' }), /one operation key/);
  await assert.rejects(() => playStep(d, { approval: 'maybe' }), /approval must be/);
  console.log('U3: 8 種の写しと例外 3 つ');
});

// 試験の設定の画面を、計数つきの context で開く。幅は 1280px。
async function openCounted(csp, query, fn, { width = 1280 } = {}) {
  let L;
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width, height: 900 } });
    const counter = await installInputCounter(context);
    L = ledger();
    await page.goto(`${srv.url}${query}`);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    await install(page);
    await fn({ page, rec, L, counter, context });
    await context.close();
  }));
  return L;
}

// 共通の終わりの確認。台帳に項目名と合否だけを積む。
// required は、その通しが押すはずのキー。ページ内の Space は文字入力の空白も数える（区別できない）ので、押した回数の確かめはドライバ側が主。
async function finish({ page, rec, L, counter }, g, scene, last, steps, required = ALLOWED_KEYS) {
  const c = await counter.read(page);
  L.check(scene, 'ページ内のマウスのイベントが 0', c.mouse === 0);
  L.check(scene, '許さないキーの keydown が 0', c.forbidden.length === 0);
  L.check(scene, 'ドライバ側の監視が 0', g.log.calls === 0);
  for (const k of required) L.check(scene, `ページ内で ${k} が 1 回以上`, (c.keys[k] ?? 0) >= 1);
  for (const k of required) L.check(scene, `ドライバ側で ${k} が 1 回以上`, g.keyboard.used[k] >= 1);
  L.check(scene, 'キーの集合の外が 0', Object.keys(c.keys).every((k) => ALLOWED_KEYS.includes(k)));
  const run = await page.evaluate(() => window.__run);
  L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
  const e = await engine(page);
  L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
  L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
  const status = e.stateWithoutLog.run.slices.main.status;
  L.check(scene, '最後の Step が達成', e.current === last && status[last] === 'achieved');
  L.check(scene, `${steps} Step がすべて達成`, Object.keys(status).length === steps && Object.values(status).every((s) => s === 'achieved'));
  const counts = await collectSafety(page, rec);
  for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
  L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
  L.check(scene, 'ページ内エラーが 0 件', rec.pageErrors.length === 0 && !rec.crashed);
  L.check(scene, 'コンソールの CSP 警告が 0 件', cspConsoleMessages(rec).length === 0);
  L.check(scene, '配信元の外への通信が 0 件', rec.external.length === 0);
  return c;
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  test(`K1: 20 Step を Tab・Enter・Space・Shift+Enter・Esc と文字だけで最後まで通す（${mode}）`, { timeout: 300000 }, async () => {
    const L = await openCounted(csp, '?fixture=ch0-3-chapters', async (ctx) => {
      const g = guardPage(ctx.page);
      await journey(g.page, keyOnlyDriver(g.page), 'K1', ctx.L, null, { chaptered: true });
      const c = await finish(ctx, g, 'K1', 'main/st-3-2', 20);
      console.log(`K1 (${mode}): キー ${JSON.stringify(c.keys)}・文字 ${c.chars}・マウス ${c.mouse}・監視 ${g.log.calls}`);
    });
    console.log(`K1 (${mode}): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), []);
  });
}

test('K2: CH-4〜CH-9 の形の 17 Step をキーボードだけで CH-9 まで通す（CSP あり）', { timeout: 300000 }, async () => {
  const L = await openCounted(true, FIXTURE_49, async (ctx) => {
    const g = guardPage(ctx.page);
    const { presses } = await journey49(g.page, keyOnlyDriver(g.page), 'K2', ctx.L, { observe: hookObserver49 });
    ctx.L.check('K2', '次の章へ進むを 5 回', presses === 5);
    const c = await finish(ctx, g, 'K2', LAST, STEPS.length, ['Tab', 'Enter', 'Shift+Enter']);
    console.log(`K2: キー ${JSON.stringify(c.keys)}・文字 ${c.chars}・マウス ${c.mouse}・監視 ${g.log.calls}`);
  });
  console.log(`K2: ${L.summary()} 項目`);
  assert.deepEqual(L.failed(), []);
});

test('N1: 実マウスの click は、ページ内のマウスの計数に出る', { timeout: 120000 }, async () => {
  await openCounted(true, '?fixture=ch0-3-chapters', async ({ page, counter }) => {
    assert.equal((await counter.read(page)).mouse, 0);
    await page.click('body', { position: { x: 5, y: 5 } });
    assert.ok((await counter.read(page)).mouse >= 1);
  });
});

test('N2: 監視した page の click は、ドライバ側の計数に出る', { timeout: 120000 }, async () => {
  await openCounted(true, '?fixture=ch0-3-chapters', async ({ page }) => {
    const g = guardPage(page);
    assert.equal(g.log.calls, 0);
    await g.page.click('body', { position: { x: 5, y: 5 } });
    assert.equal(g.log.calls, 1);
    assert.equal(g.log.names.click, 1);
  });
});

test('N3: 制限なしの矢印キーは禁止として数えられ、制限つきは押せない', { timeout: 120000 }, async () => {
  await openCounted(true, '?fixture=ch0-3-chapters', async ({ page, counter }) => {
    const kb = restrictedKeyboard(page);
    await assert.rejects(() => kb.press('ArrowDown'), /許さないキー/);
    assert.deepEqual((await counter.read(page)).forbidden, []);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('ControlOrMeta+A');
    const c = await counter.read(page);
    // Ctrl+A は修飾キーの keydown と文字の keydown の 2 件になる。
    assert.equal(c.forbidden.length, 4);
    assert.ok(c.forbidden.includes('ArrowDown') && c.forbidden.includes('Shift+Tab'));
  });
});

test('N4: 計数は再読み込みの後も消えず、累計になる', { timeout: 120000 }, async () => {
  await openCounted(true, '?fixture=ch0-3-chapters', async ({ page, counter }) => {
    await page.click('body', { position: { x: 5, y: 5 } });
    const first = (await counter.read(page)).mouse;
    assert.ok(first >= 1);
    await page.reload();
    await waitMounted(page);
    await page.click('body', { position: { x: 5, y: 5 } });
    assert.ok((await counter.read(page)).mouse >= first + 1);
  });
});

test('N5: 幅 959px の tablist が見えるとドライバは例外で止まる（1280px では止まらない）', { timeout: 120000 }, async () => {
  await openCounted(true, '?fixture=ch0-3-chapters', async ({ page }) => {
    const g = guardPage(page);
    await assertNoTablist(g.page);
    await page.setViewportSize({ width: 959, height: 900 });
    await assert.rejects(() => keyOnlyDriver(g.page).choose('opt-left'), /tablist が見えている/);
    assert.equal(g.log.calls, 0);
  });
  // 範囲の確認: RAW の選択子が空でないこと（install の計数が空振りでない前提）。
  assert.ok(RAW.length > 0);
});
