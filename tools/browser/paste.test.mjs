// 入力欄への貼り付けの取り扱いの試験（TASK-279。JUDGE-25・TERM-19。根拠は PoC-2 の P-1・P-2・P-8〜P-10）。
//
// 貼り付けはブラウザの既定の挿入 → input イベント → 状態の経路で入る。アプリは paste を横取りしない
// （glue は paste のリスナーを持たない。FEND-20・D-42）。そのため、改行が保たれること・貼り付けだけでは
// 送信が起きないこと（自動送信 0 件）・paste が止められないことを、実ブラウザの貼り付けで固定する。
// 試料は PoC-2 のバイト一致の写し（testdata/input-field-ime/test-data/paste-samples/）を読む。
// textarea の値は CRLF を LF に正規化するため、期待値は試料の CRLF を LF にしたものとする（P-8）。
//
// 対象外: 他アプリからの実際の貼り付け（JUDGE-26・TERM-20。TASK-287 の人の確認）、途中への貼り付けで
// 上限を超える分だけ切る beforeinput（JUDGE-24。別タスク）、候補の一覧（`/` 始まりでも候補が
// 開かないことは、コマンド表を持つ試験の設定が要るため paste-notice.test.mjs の P-9 が確かめる）。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1。状態と欄の一致）。
// どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-paste/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const ITEMS = '[data-testid="log-list"] > li';
const SAMPLES = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../testdata/input-field-ime/test-data/paste-samples',
);
const read = (name) => fs.readFileSync(path.join(SAMPLES, name), 'utf8');
const count = (s, ch) => s.split(ch).length - 1;
const normalize = (s) => s.replace(/\r\n?/g, '\n');

const KINDS = [
  { label: '3 行', file: 'paste-3lines.txt' },
  { label: '100 行', file: 'paste-100lines.txt' },
  { label: 'CRLF と Tab', file: 'paste-crlf-tabs.txt' },
  { label: '/ 始まりの複数行', file: 'paste-command-lines.txt' },
];

let browser;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const lines = (page) => page.$$eval(ITEMS, (lis) => lis.map((li) => li.textContent));
const value = (page) => page.inputValue(INPUT);
const frames = (page, n) => page.evaluate(async (k) => {
  for (let i = 0; i < k; i += 1) await new Promise((r) => requestAnimationFrame(r));
}, n);

async function open(srv) {
  const o = await openPage(browser, { origin: srv.origin });
  await o.context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: srv.origin });
  await o.page.goto(srv.url);
  await waitMounted(o.page);
  return o;
}

// 欄を空にしてクリップボード経由で貼り付ける。
async function paste(page, text) {
  await page.evaluate((t) => navigator.clipboard.writeText(t), text);
  await page.focus(INPUT);
  await page.keyboard.press('ControlOrMeta+A');
  await page.keyboard.press('Delete');
  await page.keyboard.press('ControlOrMeta+V');
}

function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`4 種類の貼り付けで改行が保たれ、自動送信は 0 件（${mode}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      // window の bubble で paste の defaultPrevented を記録する（アプリのリスナーより後に走る位置）。
      await page.evaluate(() => {
        window.__pasteLog = [];
        window.addEventListener('paste', (e) => window.__pasteLog.push(e.defaultPrevented));
      });
      for (const { label, file } of KINDS) {
        const raw = read(file);
        const expected = normalize(raw);
        await page.evaluate(() => { window.__pasteLog.length = 0; });
        await paste(page, raw);
        const v = await value(page);
        assert.equal(v, expected, label);
        assert.equal(count(v, '\n'), count(expected, '\n'), label);
        assert.ok(count(v, '\n') >= 1, `${label}: no newline kept`);
        if (file === 'paste-3lines.txt') assert.equal(v.split('\n').length, count(expected, '\n') + 1);
        if (file === 'paste-100lines.txt') assert.ok(count(v, '\n') >= 99, label);
        if (file === 'paste-crlf-tabs.txt') {
          assert.equal(count(v, '\r'), 0, label);
          assert.equal(count(v, '\t'), count(raw, '\t'), label);
          assert.ok(count(raw, '\t') > 0, label);
        }
        if (file === 'paste-command-lines.txt') assert.ok(v.startsWith('/'), label);
        // 遅れて起きる送信も拾うため、描画を 2 回待ってから数える。
        await frames(page, 2);
        assert.deepEqual(await lines(page), [], `${label}: auto-submitted`);
        assert.deepEqual(await page.evaluate(() => window.__pasteLog), [false], `${label}: paste was prevented`);
      }
      assertClean(rec);
      await context.close();
    }));
  });

  test(`貼り付けた複数行は送信ボタンで 1 件として送られ、改行を保つ（${mode}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      for (const file of ['paste-3lines.txt', 'paste-command-lines.txt']) {
        await page.reload();
        await waitMounted(page);
        const raw = read(file);
        await paste(page, raw);
        await page.click(SUBMIT);
        await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, ITEMS);
        assert.deepEqual(await lines(page), [normalize(raw)], file);
        // 文字ノードだけで描く（SAFE-11）。
        assert.equal(await page.$eval(ITEMS, (li) => li.children.length), 0, file);
      }
      assertClean(rec);
      await context.close();
    }));
  });

  test(`合成の paste と貼り付けの keydown はアプリに止められない（${mode}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.focus(INPUT);
      const sample = read('paste-3lines.txt');
      // P-10: dispatchEvent は preventDefault されていなければ true を返す。
      const notPrevented = await page.evaluate(({ sel, text }) => {
        const dt = new DataTransfer();
        dt.setData('text/plain', text);
        const el = document.querySelector(sel);
        return el.dispatchEvent(new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: dt }));
      }, { sel: INPUT, text: sample });
      assert.equal(notPrevented, true);
      // 信頼されていない paste は既定の挿入をしない。
      assert.equal(await value(page), '');
      for (const mod of ['ctrlKey', 'metaKey']) {
        const ok = await page.evaluate(({ sel, mod: m }) => {
          const el = document.querySelector(sel);
          return el.dispatchEvent(new KeyboardEvent('keydown', { key: 'v', [m]: true, bubbles: true, cancelable: true }));
        }, { sel: INPUT, mod });
        assert.equal(ok, true, mod);
      }
      await frames(page, 2);
      assert.deepEqual(await lines(page), []);
      assertClean(rec);
      await context.close();
    }));
  });
}

test('試験の設定: 貼り付け後の状態の下書きが欄の値と一致し、改行を保つ', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.draft), 'function', 'test hooks are missing');
    for (const { label, file } of KINDS) {
      const raw = read(file);
      await paste(page, raw);
      const draft = await page.evaluate(() => window.__fandheTest.draft());
      assert.equal(draft, await value(page), label);
      assert.equal(draft, normalize(raw), label);
    }
    assertClean(rec);
    await context.close();
  });
});
