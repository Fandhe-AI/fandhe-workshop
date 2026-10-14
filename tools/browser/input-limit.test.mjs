// 入力欄の 20,000 文字の上限（SAFE-21。TASK-158）の試験。
//
// 上限はコードポイントで数える（HTML の maxlength は UTF-16 の単位なので使わない）。超過分は受け付けず、
// 固定の案内文を出し、1 文字消すと案内も消える。切り詰めた値は glue が <textarea> へ書き戻す（上流の F-1 への対処）。
//
// 入力: FANDHE_WEB_DIR（配信設定。利用者から見える振る舞い）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1。状態と DOM の一致）。
// どちらも未設定・不在は skip にせず失敗にする。文字列は試験の中で組み立て、fixture のファイルは置かない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted } from './harness.mjs';

const BASE = '/fandhe-input-limit/';
const MAX = 20000;
// contract の INPUT_TOO_LONG_MESSAGE と同じ文面（limits.rs が正）。
const NOTICE = '入力は 20,000 文字までです。超えた分は受け付けていません。';
const INPUT = '[data-testid="draft-input"]';
const NOTICE_SEL = '[data-testid="input-notice"]';
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

const cpLen = (s) => [...s].length;

// 欄を空にしてクリップボード経由で text を貼り付ける。
async function paste(page, text) {
  await page.evaluate((t) => navigator.clipboard.writeText(t), text);
  await page.focus(INPUT);
  await page.keyboard.press('ControlOrMeta+A');
  await page.keyboard.press('Delete');
  await page.keyboard.press('ControlOrMeta+V');
}

const value = (page) => page.inputValue(INPUT);
const notice = async (page) => (await page.textContent(NOTICE_SEL)).trim();

async function open(srv) {
  const o = await openPage(browser, { origin: srv.origin });
  await o.context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: srv.origin });
  await o.page.goto(srv.url);
  await waitMounted(o.page);
  return o;
}

function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

test('上限ちょうどは案内なし・超過は切って案内・打鍵は受け付けず・1 文字消すと案内が消える', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);

    // P-3: 20,000 文字は通り、案内は空。
    await paste(page, 'あ'.repeat(MAX));
    assert.equal(cpLen(await value(page)), MAX);
    assert.equal(await notice(page), '');

    // P-4: 20,001 文字は先頭の 20,000 文字になり、案内が出る。
    await paste(page, 'あ'.repeat(MAX + 1));
    assert.equal(await value(page), 'あ'.repeat(MAX));
    assert.equal(await notice(page), NOTICE);

    // P-5: 20,000 文字の状態でもう 1 文字打っても受け付けず、案内が出る。
    await paste(page, 'あ'.repeat(MAX));
    assert.equal(await notice(page), '');
    await page.focus(INPUT);
    await page.keyboard.press('ControlOrMeta+End');
    await page.keyboard.type('x');
    assert.equal(cpLen(await value(page)), MAX);
    assert.equal(await notice(page), NOTICE);

    // P-6: 1 文字消すと案内も消える。
    await page.keyboard.press('Backspace');
    assert.equal(cpLen(await value(page)), MAX - 1);
    assert.equal(await notice(page), '');

    assertClean(rec);
    await context.close();
  });
});

test('サロゲートペアは 1 文字と数え_超過でもペアを割らない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);

    // P-7: UTF-16 では 20,001 単位でも、コードポイントでは 20,000 なので通る。
    const text = 'あ'.repeat(MAX - 1) + '😀';
    await paste(page, text);
    const v = await value(page);
    assert.equal(v.length, MAX + 1);
    assert.equal(cpLen(v), MAX);
    assert.equal(await notice(page), '');

    // 超過: ペアの途中で切らない。
    await paste(page, '😀'.repeat(MAX + 1));
    const w = await value(page);
    assert.equal(cpLen(w), MAX);
    assert.equal(w, '😀'.repeat(MAX));
    assert.equal(w.isWellFormed(), true);
    assert.equal(await notice(page), NOTICE);

    assertClean(rec);
    await context.close();
  });
});

test('200,000 文字の入力も 20,000 文字になり理由が出る（時間は記録のみ）', async (t) => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);

    const ms = await page.evaluate(
      async ({ sel, nsel }) => {
        const el = document.querySelector(sel);
        const note = document.querySelector(nsel);
        const t0 = performance.now();
        el.value = 'あ'.repeat(200000);
        el.dispatchEvent(new Event('input', { bubbles: true }));
        // 案内が出るまで rAF で待つ（上限は 5 秒。出なければ例外にせず戻り、外側の assert で落とす）。
        while (note.textContent.trim() === '' && performance.now() - t0 < 5000) {
          await new Promise((r) => requestAnimationFrame(r));
        }
        return performance.now() - t0;
      },
      { sel: INPUT, nsel: NOTICE_SEL },
    );
    t.diagnostic(`200,000 chars: notice shown after ${ms.toFixed(1)} ms (PoC reference: 14 ms)`);
    assert.equal(cpLen(await value(page)), MAX);
    assert.equal(await notice(page), NOTICE);
    // 環境で揺れるため、緩い上限だけを assert する。
    assert.ok(ms < 1000, `took ${ms} ms`);

    assertClean(rec);
    await context.close();
  });
});

test('試験の設定: 状態の下書き・超過の印が欄の value と一致する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.draft), 'function', 'test hooks are missing');
    const state = async () => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson()));

    await paste(page, 'あ'.repeat(MAX + 1));
    let s = await state();
    assert.equal(s.draftCodePoints, MAX);
    assert.equal(s.inputOverLimit, true);
    assert.equal(await page.evaluate(() => window.__fandheTest.draft()), await value(page));

    await page.evaluate((t) => {
      const el = document.querySelector('[data-testid="draft-input"]');
      el.value = t;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'あ'.repeat(200000));
    s = await state();
    assert.equal(s.draftCodePoints, MAX);
    assert.equal(s.inputOverLimit, true);
    assert.equal(await page.evaluate(() => window.__fandheTest.draft()), await value(page));

    await page.focus(INPUT);
    await page.keyboard.press('Backspace');
    s = await state();
    assert.equal(s.draftCodePoints, MAX - 1);
    assert.equal(s.inputOverLimit, false);

    assertClean(rec);
    await context.close();
  });
});
