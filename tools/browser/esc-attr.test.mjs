// root の中の Esc が属性（data-action-keydown="esc"）で Component に届くことの試験（TASK-350 / FEND-5）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の出力）。未設定・不在は skip にせず失敗にする。
// 受信回数は stateJson().escCount で読み、各操作の後に「ちょうどの値」を確かめる（二重配送を見逃さないため）。
// 変換中の Esc は、CDP で keyCode 229 の keydown を作る。これはシミュレーターであり、実機の IME での確認ではない
// （実機は FEND-9・TASK-354）。
// keydown-probe のボタンは対象にしない。そのボタンにフォーカスがあると closest が probe で止まり、esc ではなく
// keydown-probe に届くため（試験の設定にしかない要素）。
// root の外の Esc は FEND-8 の glue の担当で、document の keydown で届く（TASK-353 1/2）。root の中の Esc の属性は main と workshop-root の
// 両方に付く（上流は最も近い 1 つにだけ届ける。TASK-353 2/2）。各場面の「ちょうど 1 回」は esc-once.test.mjs が確かめる。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize } from './harness.mjs';

const BASE = '/fandhe-esc-attr/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const ITEMS = '[data-testid="log-list"] > li';

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(
    dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(),
    'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1',
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

const settle = (page) => page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(r, 50)))));
const escCount = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).escCount;

function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

test('入力欄の Esc は押した回数だけ届き、欄も一覧も変わらない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    await page.fill(INPUT, 'abc');
    await page.focus(INPUT);
    const items = await page.locator(ITEMS).count();
    assert.equal(await escCount(page), 0);
    await page.keyboard.press('Escape');
    await settle(page);
    assert.equal(await escCount(page), 1);
    await page.keyboard.press('Escape');
    await settle(page);
    assert.equal(await escCount(page), 2);
    assert.equal(await page.inputValue(INPUT), 'abc');
    assert.equal(await page.locator(ITEMS).count(), items);
    assertClean(rec);
    await context.close();
  });
});

test('root の中のボタンの Esc も 1 回だけ届く', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    await page.focus(SUBMIT);
    await page.keyboard.press('Escape');
    await settle(page);
    assert.equal(await escCount(page), 1);
    assertClean(rec);
    await context.close();
  });
});

test('Esc は preventDefault されず、合成イベントでもちょうど 1 回届く', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    let expected = 0;
    for (const sel of [INPUT, SUBMIT]) {
      const notPrevented = await page.evaluate((s) => {
        const el = document.querySelector(s);
        return el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
      }, sel);
      assert.equal(notPrevented, true, sel);
      await settle(page);
      expected += 1;
      assert.equal(await escCount(page), expected, sel);
    }
    assertClean(rec);
    await context.close();
  });
});

test('変換中（isComposing・keyCode 229）の Esc は届かず、通常の Esc は届く', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    await page.focus(INPUT);
    await page.evaluate(() => {
      window.__seen = [];
      document.addEventListener('keydown', (e) => window.__seen.push({ key: e.key, keyCode: e.keyCode, isComposing: e.isComposing }), true);
    });
    // isComposing が真の合成イベント。
    await page.evaluate((sel) => {
      document.querySelector(sel).dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', isComposing: true, bubbles: true, cancelable: true }));
    }, INPUT);
    await settle(page);
    assert.equal(await escCount(page), 0);

    // CDP で keyCode 229 の Escape を作る（シミュレーター。実機の IME ではない）。
    const cdp = await context.newCDPSession(page);
    const ev = { key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 229, nativeVirtualKeyCode: 229 };
    await cdp.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', ...ev });
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', ...ev });
    await settle(page);
    const seen = await page.evaluate(() => window.__seen);
    const real = seen.filter((e) => e.key === 'Escape' && e.keyCode === 229);
    assert.ok(real.length >= 1, `the 229 Escape did not reach the page as key=Escape: ${JSON.stringify(seen)}`);
    assert.equal(await escCount(page), 0);

    // 対照: 通常の Esc は届く（経路が生きている）。
    await page.keyboard.press('Escape');
    await settle(page);
    assert.equal(await escCount(page), 1);
    assertClean(rec);
    await context.close();
  });
});

test('修飾付き（Shift+Escape）は届かない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    await page.focus(INPUT);
    await page.keyboard.press('Shift+Escape');
    await settle(page);
    assert.equal(await escCount(page), 0);
    assertClean(rec);
    await context.close();
  });
});

test('root の外（body）の Esc はちょうど 1 回届く（FEND-8。glue の document の keydown。TASK-353 1/2）', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    await page.evaluate(() => document.activeElement && document.activeElement.blur());
    await page.keyboard.press('Escape');
    await settle(page);
    assert.equal(await escCount(page), 1);
    // 2 回押して 2（二重配送が無い）。
    await page.keyboard.press('Escape');
    await settle(page);
    assert.equal(await escCount(page), 2);
    assertClean(rec);
    await context.close();
  });
});
