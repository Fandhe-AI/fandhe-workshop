// 貼り付けと上限の案内文の画面の観察の隙間を埋める試験（TASK-442。TERM-15・TERM-19。PoC-2 の P-1〜P-10）。
//
// 切り捨ての境界と打鍵の経路は input-limit.test.mjs、4 種類の貼り付けの改行・自動送信 0 件・合成の paste は
// paste.test.mjs が確かめる。ここでは、それらが確かめていない次の点だけを足す（重複して流さない）。
//   - P-3・P-4・P-7: PoC-2 のバイト一致の写し（paste-20000.txt ほか）での貼り付けを、CSP あり・なしで流し、
//     毎回右のログが増えないこと（P-3〜P-7 の「ログが増えない」）。
//   - P-5・P-6: 上限ちょうどで 1 文字打つ → 案内 → Backspace で消える、を CSP あり・なしで流す。
//   - 案内欄の DOM の性質（TASK-431 の契約）: 1 個・role=status・子の要素が 0・入力欄と同じ親で後ろ。
//   - P-9: `/` 始まりの複数行を貼り付けても候補の一覧が開かない（教材のコマンド表がある試験の設定でだけ観察できる）。
//   - 試験の設定で 4 種類の貼り付けの毎回、ログの件数が変わらず案内が空であること。
//   - P-10 の後に案内が出ず、ログが増えないこと。
//
// 対象外: 他アプリからの実際の貼り付け（TERM-20。TASK-287 の人の確認）、途中への貼り付けで貼った分だけを切る
// beforeinput（TERM-16・JUDGE-24。input-limit.test.mjs が受け持つ）、案内文の文言の確定（spec の側）。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1。教材は min-3step.json）。
// どちらも未設定・不在は skip にせず失敗にする。失敗の文面には、長い値そのものでなく長さを出す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-paste-notice/';
const MAX = 20000;
// contract の INPUT_TOO_LONG_MESSAGE と同じ文面（limits.rs が正）。
const NOTICE = '入力は 20,000 文字までです。超えた分は受け付けていません。';
const INPUT = '[data-testid="draft-input"]';
const NOTICE_SEL = '[data-testid="input-notice"]';
const ITEMS = '[data-testid="log-list"] > li';
const LIST = '[data-testid="candidate-list"]';
const SAMPLES = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../testdata/input-field-ime/test-data/paste-samples',
);
const read = (name) => fs.readFileSync(path.join(SAMPLES, name), 'utf8');
const normalize = (s) => s.replace(/\r\n?/g, '\n');
const cpLen = (s) => [...s].length;

const KINDS = ['paste-3lines.txt', 'paste-100lines.txt', 'paste-crlf-tabs.txt', 'paste-command-lines.txt'];

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

const value = (page) => page.inputValue(INPUT);
const notice = async (page) => (await page.textContent(NOTICE_SEL)).trim();
const lines = (page) => page.$$eval(ITEMS, (lis) => lis.length);
const logLen = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine.logLen;
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
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  test(`PoC-2 の写しの貼り付け: 20,000 は通り_20,001 は切って案内_UTF-16 で 20,001 単位は通る。ログは増えない（P-3・P-4・P-7。${mode}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);

      const exact = read('paste-20000.txt');
      assert.equal(cpLen(exact), MAX);
      await paste(page, exact);
      assert.equal(await value(page), normalize(exact), 'P-3: value differs');
      assert.equal(await notice(page), '', 'P-3');
      await frames(page, 2);
      assert.equal(await lines(page), 0, 'P-3: log grew');

      const over = read('paste-20001.txt');
      assert.equal(cpLen(over), MAX + 1);
      await paste(page, over);
      const v = await value(page);
      assert.equal(cpLen(v), MAX, `P-4: length ${cpLen(v)}`);
      assert.equal(v, [...over].slice(0, MAX).join(''), 'P-4: not the leading part');
      assert.equal(await notice(page), NOTICE, 'P-4');
      await frames(page, 2);
      assert.equal(await lines(page), 0, 'P-4: log grew');

      const astral = read('paste-20000-astral.txt');
      assert.equal(cpLen(astral), MAX);
      assert.equal(astral.length, MAX + 1);
      await paste(page, astral);
      const w = await value(page);
      assert.equal(w.length, MAX + 1, 'P-7: utf-16 length');
      assert.equal(w, astral, 'P-7: value differs');
      assert.equal(await notice(page), '', 'P-7');
      await frames(page, 2);
      assert.equal(await lines(page), 0, 'P-7: log grew');

      assertClean(rec);
      await context.close();
    });
  });

  test(`上限ちょうどでもう 1 文字打つと案内が出て、Backspace で消える。ログは増えない（P-5・P-6。${mode}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await paste(page, 'あ'.repeat(MAX));
      assert.equal(await notice(page), '');
      await page.focus(INPUT);
      await page.keyboard.press('ControlOrMeta+End');
      await page.keyboard.type('x');
      assert.equal(cpLen(await value(page)), MAX);
      assert.equal(await notice(page), NOTICE);
      await frames(page, 2);
      assert.equal(await lines(page), 0, 'P-5: log grew');
      await page.keyboard.press('Backspace');
      assert.equal(cpLen(await value(page)), MAX - 1);
      assert.equal(await notice(page), '');
      await frames(page, 2);
      assert.equal(await lines(page), 0, 'P-6: log grew');
      assertClean(rec);
      await context.close();
    });
  });

  test(`案内欄は 1 個で、role=status、子の要素が無く、入力欄と同じ親の後ろにある（${mode}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await paste(page, 'あ'.repeat(MAX + 1));
      assert.equal(await notice(page), NOTICE);
      const facts = await page.evaluate(({ input, note }) => {
        const n = document.querySelectorAll(note);
        const i = document.querySelector(input);
        const el = n[0];
        return {
          count: n.length,
          role: el.getAttribute('role'),
          children: el.children.length,
          sameParent: el.parentElement === i.parentElement,
          after: Boolean(i.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING),
        };
      }, { input: INPUT, note: NOTICE_SEL });
      assert.deepEqual(facts, { count: 1, role: 'status', children: 0, sameParent: true, after: true });
      assertClean(rec);
      await context.close();
    });
  });

  test(`合成の paste の後も案内は出ず、ログは増えない（P-10。${mode}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.focus(INPUT);
      const notPrevented = await page.evaluate((sel) => {
        const dt = new DataTransfer();
        dt.setData('text/plain', 'a\nb\nc');
        return document.querySelector(sel)
          .dispatchEvent(new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: dt }));
      }, INPUT);
      assert.equal(notPrevented, true);
      await frames(page, 2);
      assert.equal(await value(page), '');
      assert.equal(await notice(page), '');
      assert.equal(await lines(page), 0);
      assertClean(rec);
      await context.close();
    });
  });
}

test('試験の設定: `/` 始まりの複数行を貼り付けても候補の一覧は開かず、ログは増えない（P-9）', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    const before = await logLen(page);
    const raw = read('paste-command-lines.txt');
    await paste(page, raw);
    assert.equal(await value(page), normalize(raw));
    assert.ok((await value(page)).startsWith('/'));
    await frames(page, 2);
    assert.equal(await page.locator(LIST).evaluate((el) => el.hasAttribute('hidden')), true, 'candidate list opened');
    assert.equal(await logLen(page), before);
    assert.equal(await notice(page), '');
    assertClean(rec);
    await context.close();
  });
});

test('試験の設定: 4 種類の貼り付けの毎回、ログの件数は変わらず案内は空（R2）', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    const before = await logLen(page);
    for (const file of KINDS) {
      await paste(page, read(file));
      await frames(page, 2);
      assert.equal(await logLen(page), before, `${file}: log changed`);
      assert.equal(await notice(page), '', file);
      assert.equal(await lines(page), 0, file);
    }
    assertClean(rec);
    await context.close();
  });
});
