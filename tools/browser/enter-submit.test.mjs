// Enter による入力欄の送信と Shift+Enter の改行の試験（TASK-275。JUDGE-12・JUDGE-13・FEND-14）。
// Ctrl・Alt・Meta 付きの Enter は送信にも改行にもしない（TASK-282 2/2。JUDGE-22・TERM-14・D-60）。
//
// 修飾なしの Enter は、送信ボタンの click と同じ入力・同じ記録になる。変換中・修飾キー付き・Shift 付きでは送信しない。
// Shift+Enter は送信せず、preventDefault もしないので、欄に改行が入る。複数行の下書き（S-29・S-30）は、
// 改行を保った 1 件として送信される（textContent に残る。見た目の改行は後続のスタイルの担当）。
// 配信設定の画面で、Playwright の信頼入力（page.keyboard.press）を使って確かめる。
// 本格的な IME の模擬（CDP・試験文 30 文）は TASK-276 の担当で、
// ここでは変換中の Enter が送信にならないことの最小の確認だけを持つ。
// 送信後の下書きは空になる（TERM-1。TASK-438）。書き戻しは非同期のため、空になるのを待って確かめる。
//
// 入力: FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。入力は試験の中で組み立てる。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-enter-submit/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const ITEMS = '[data-testid="log-list"] > li';
// 改行は含めない（複数行は下の MULTILINE）。
const CASES = ['hello', '日本語の文', '# 見出し [x](https://example.invalid/a) <b>t</b>'];

// PoC-2 の試験文 S-29・S-30。行の間は Shift+Enter で入れる。
const MULTILINE = [
  ['一行目のメモ', '二行目のメモ'],
  ['確認事項：', 'ログインの手順'],
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

const lines = (page) => page.$$eval(ITEMS, (lis) => lis.map((li) => li.textContent));

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

async function sendAll(page, how) {
  for (let i = 0; i < CASES.length; i += 1) {
    await page.fill(INPUT, CASES[i]);
    if (how === 'click') await page.click(SUBMIT);
    else await page.keyboard.press('Enter');
    await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [ITEMS, i + 1]);
    await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT);
  }
}

for (const csp of [true, false]) {
  test(`Enter の送信は送信ボタンの click と同じ記録になる（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const a = await open(srv);
      await sendAll(a.page, 'click');
      const b = await open(srv);
      await b.page.focus(INPUT);
      await sendAll(b.page, 'enter');
      const viaClick = await lines(a.page);
      const viaEnter = await lines(b.page);
      assert.deepEqual(viaClick, CASES);
      assert.deepEqual(viaEnter, viaClick);
      // preventDefault が効き、欄に改行が入らない。送信の後は空になる。
      assert.equal(await b.page.inputValue(INPUT), '');
      for (const { rec } of [a, b]) {
        const s = summarize(rec);
        const detail = JSON.stringify(rec);
        assert.equal(s.csp, 0, detail);
        assert.equal(s.pageErrors, 0, detail);
        assert.equal(s.consoleErrors, 0, detail);
      }
      await a.context.close();
      await b.context.close();
    }));
  });
}

test('下書きが空のとき、Enter でも click でも一覧は増えない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.focus(INPUT);
    await page.keyboard.press('Enter');
    await page.click(SUBMIT);
    assert.deepEqual(await lines(page), []);
    await context.close();
  });
});

test('Shift 付きの Enter では送信しない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.fill(INPUT, 'x');
    await page.focus(INPUT);
    await page.keyboard.press('Shift+Enter');
    assert.deepEqual(await lines(page), []);
    await context.close();
  });
});

// JUDGE-22・TERM-14: 修飾キー付きの Enter は送信にも改行にもしない（欄の値は変わらない）。
// 限界: Playwright の keyboard.press は、Shift 以外の修飾キーを押している間は text を送らない。実キーボードの
// OS ごとの差（とくに Windows）は代わりに確かめない（D-48 で別タスク）。
test('Control・Alt・Meta 付き（Shift 併用を含む）の Enter は送信も改行もしない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.fill(INPUT, 'x');
    await page.focus(INPUT);
    for (const mod of ['Control', 'Alt', 'Meta', 'Shift+Control', 'Shift+Alt', 'Shift+Meta', 'Control+Alt']) {
      await page.keyboard.press(`${mod}+Enter`);
      assert.equal(await page.inputValue(INPUT), 'x', mod);
    }
    assert.deepEqual(await lines(page), []);
    await context.close();
  });
});

test('Control・Alt・Meta 付きの Enter の keydown は preventDefault せず送信もしない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.fill(INPUT, 'x');
    for (const flag of ['ctrlKey', 'altKey', 'metaKey']) {
      const notPrevented = await page.evaluate(([sel, f]) => {
        const el = document.querySelector(sel);
        return el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', [f]: true, bubbles: true, cancelable: true }));
      }, [INPUT, flag]);
      assert.equal(notPrevented, true, flag);
    }
    assert.deepEqual(await lines(page), []);
    await context.close();
  });
});

test('押しっぱなしの自動リピート（repeat）の Enter では送信しない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.fill(INPUT, 'x');
    // dispatchEvent は preventDefault されると false を返す。リピートでも改行は抑止される。
    const notPrevented = await page.evaluate((sel) => {
      const el = document.querySelector(sel);
      const make = () => new KeyboardEvent('keydown', { key: 'Enter', repeat: true, bubbles: true, cancelable: true });
      el.dispatchEvent(make());
      el.dispatchEvent(make());
      return el.dispatchEvent(make());
    }, INPUT);
    assert.equal(notPrevented, false);
    assert.deepEqual(await lines(page), []);
    // 最初の押下（repeat なし）は 1 回だけ送信される。
    await page.focus(INPUT);
    await page.keyboard.press('Enter');
    await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, ITEMS);
    assert.deepEqual(await lines(page), ['x']);
    await context.close();
  });
});

test('変換中の Enter（isComposing・Process）では送信しない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.fill(INPUT, 'x');
    await page.evaluate((sel) => {
      const el = document.querySelector(sel);
      el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', isComposing: true, bubbles: true, cancelable: true }));
      el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Process', bubbles: true, cancelable: true }));
    }, INPUT);
    assert.deepEqual(await lines(page), []);
    await context.close();
  });
});

// 信頼入力で複数行を打つ。行の間は Shift+Enter、最後は呼び出し側が Enter を押す。
async function typeLines(page, rows) {
  await page.fill(INPUT, '');
  await page.focus(INPUT);
  for (let i = 0; i < rows.length; i += 1) {
    if (i > 0) await page.keyboard.press('Shift+Enter');
    await page.keyboard.insertText(rows[i]);
  }
}

for (const csp of [true, false]) {
  test(`Shift+Enter は送信せず改行を入れ、複数行を 1 件として送信する（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const a = await open(srv);
      const b = await open(srv);
      const expected = MULTILINE.map((rows) => rows.join('\n'));
      for (let i = 0; i < MULTILINE.length; i += 1) {
        const rows = MULTILINE[i];
        // 途中まで: 1 行目 + Shift+Enter。送信されず、欄に改行が入る（JUDGE-13）。
        await typeLines(a.page, rows.slice(0, 1));
        await a.page.keyboard.press('Shift+Enter');
        assert.equal((await lines(a.page)).length, i);
        assert.equal(await a.page.inputValue(INPUT), `${rows[0]}\n`);
        // 続きを入れて Enter で送信する。
        await a.page.keyboard.insertText(rows[1]);
        await a.page.keyboard.press('Enter');
        await a.page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [ITEMS, i + 1]);
        // click 経路（fill で改行入りの値を入れる）と比べる。
        await b.page.fill(INPUT, expected[i]);
        await b.page.click(SUBMIT);
        await b.page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [ITEMS, i + 1]);
      }
      assert.deepEqual(await lines(a.page), expected);
      assert.deepEqual(await lines(b.page), expected);
      // 文字ノードだけで描く（SAFE-11）。子要素は作らない。
      const children = await a.page.$$eval(ITEMS, (lis) => lis.map((li) => li.children.length));
      assert.deepEqual(children, expected.map(() => 0));
      for (const { rec } of [a, b]) {
        const s = summarize(rec);
        const detail = JSON.stringify(rec);
        assert.equal(s.csp, 0, detail);
        assert.equal(s.pageErrors, 0, detail);
        assert.equal(s.consoleErrors, 0, detail);
      }
      await a.context.close();
      await b.context.close();
    }));
  });
}

test('Shift+Enter の keydown は既定の動作を妨げない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.fill(INPUT, 'x');
    // dispatchEvent は preventDefault されていなければ true を返す。
    const notPrevented = await page.evaluate((sel) => {
      const el = document.querySelector(sel);
      return el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', shiftKey: true, bubbles: true, cancelable: true }));
    }, INPUT);
    assert.equal(notPrevented, true);
    assert.deepEqual(await lines(page), []);
    await context.close();
  });
});

test('連続した Shift+Enter の空行は保たれる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    await page.fill(INPUT, '');
    await page.focus(INPUT);
    await page.keyboard.insertText('a');
    await page.keyboard.press('Shift+Enter');
    await page.keyboard.press('Shift+Enter');
    await page.keyboard.insertText('b');
    await page.keyboard.press('Enter');
    await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, ITEMS);
    assert.deepEqual(await lines(page), ['a\n\nb']);
    await context.close();
  });
});
