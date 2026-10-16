// 送信（Enter・送信ボタン・Shift+Enter の複数行）の結線の試験（TASK-438。TERM-1・TERM-2）。
//
// 試験の設定（TEST_HOOKS=1）の画面には、試験データの教材が入っている。Playwright の信頼入力（page.keyboard・page.click）で、
// 入力欄から送信し、次を確かめる。
//   (a) Enter と送信ボタンの click が、エンジンの記録（本文つきの InputReceived）と右のログの行（属性・本文）で一致する。
//   (b) 拒否されなかった送信の後は欄が空になり、拒否された送信の後は本文が残る。
//   (c) Shift+Enter は送信せず改行を入れる。最後の Enter で改行を保った 1 件になり、ログの本文も改行を保つ（SAFE-11）。
//   (d) 配信設定（教材なし）でも Enter・click の後に欄が空になる。
// CDP による IME の模擬（試験文 30 文）は TASK-276・TASK-439 の担当で、ここでは扱わない。
// 書き戻しは非同期（set_timeout 0）のため、欄の値は空になるのを待って確かめる。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-submit-wiring/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const LOG = '[data-testid="part-log-list"]';
const RECEIVED = `${LOG} li[data-kind="received"]`;

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

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;

// 進行（処理中の段階）が止まるまで待つ。
async function waitQuiet(page) {
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null');
}

const waitEmpty = (page) => page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT);

// DOM の received の行（属性と本文）。
const receivedRows = (page) => page.$$eval(RECEIVED, (lis) => lis.map((li) => ({
  input: li.dataset.input,
  body: li.querySelector('[data-testid="line-body"]').textContent,
  children: li.querySelector('[data-testid="line-body"]').children.length,
})));

async function send(page, how) {
  if (how === 'click') await page.click(SUBMIT);
  else {
    await page.focus(INPUT);
    await page.keyboard.press('Enter');
  }
}

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

// 拒否される入力 1 件と、受け付けられる入力 1 件。
const REJECTED = '/unknown-x';
const ACCEPTED = '/make-note first-note';

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`Enter と送信ボタンは同じ記録と同じ行になり、拒否のときだけ下書きが残る（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const results = [];
      for (const how of ['click', 'enter']) {
        const { page, context, rec } = await open(srv);
        await page.fill(INPUT, REJECTED);
        await send(page, how);
        await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, RECEIVED);
        assert.equal(await page.inputValue(INPUT), REJECTED, `${how}: 拒否の後は本文が残る`);
        await page.fill(INPUT, ACCEPTED);
        await send(page, how);
        await waitEmpty(page);
        await waitQuiet(page);
        const eng = await engine(page);
        results.push({ received: eng.received.map((r) => r.input), rows: await receivedRows(page) });
        clean(rec);
        await context.close();
      }
      assert.deepEqual(results[0].received, [{ command: REJECTED }, { command: ACCEPTED }]);
      assert.deepEqual(results[1], results[0]);
      assert.deepEqual(results[0].rows.map((r) => r.body), [REJECTED, ACCEPTED]);
      assert.deepEqual(results[0].rows.map((r) => r.input), ['command', 'command']);
      assert.ok(results[0].rows.every((r) => r.children === 0));
    }));
  });

  test(`Shift+Enter は送信せず、最後の Enter で改行を保った 1 件になる（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      const bodies = [];
      // 1 件目は受け付けられる（欄が空になる）。2 件目は教材が自然文を拒否するため、本文が残る。
      // どちらも、改行を保った 1 件として記録される。
      const cases = [
        { lines: ['/make-note first-note', '二行目のメモ'], accepted: true },
        { lines: ['a', '', 'b'], accepted: false },
      ];
      for (const { lines, accepted } of cases) {
        await page.focus(INPUT);
        for (let i = 0; i < lines.length; i += 1) {
          await page.keyboard.insertText(lines[i]);
          if (i < lines.length - 1) {
            await page.keyboard.press('Shift+Enter');
            await page.waitForFunction(([sel, n]) => document.querySelector(sel).value.split('\n').length === n, [INPUT, i + 2]);
          }
        }
        const before = (await engine(page)).received.length;
        assert.equal(await page.inputValue(INPUT), lines.join('\n'));
        assert.equal(await page.$$eval(RECEIVED, (l) => l.length), bodies.length, 'Shift+Enter では送信しない');
        await page.keyboard.press('Enter');
        await page.waitForFunction((n) => document.querySelectorAll('li[data-kind="received"]').length === n, bodies.length + 1);
        if (accepted) await waitEmpty(page);
        await waitQuiet(page);
        const eng = await engine(page);
        assert.equal(eng.received.length, before + 1);
        if (!accepted) assert.equal(await page.inputValue(INPUT), lines.join('\n'));
        bodies.push(lines.join('\n'));
        if (!accepted) await page.fill(INPUT, '');
      }
      const rows = await receivedRows(page);
      assert.deepEqual(rows.map((r) => r.body), bodies);
      assert.ok(rows.every((r) => r.children === 0));
      clean(rec);
      await context.close();
    }));
  });

  test(`配信設定でも送信の後に欄が空になる（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      for (const how of ['click', 'enter']) {
        const { page, context, rec } = await open(srv);
        await page.fill(INPUT, 'hello');
        await send(page, how);
        await waitEmpty(page);
        assert.equal(await page.$$eval('[data-testid="log-list"] > li', (l) => l.length), 1);
        clean(rec);
        await context.close();
      }
    }));
  });
}
