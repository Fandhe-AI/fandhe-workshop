// root の外の Esc が「ちょうど 1 回」届くことの補助確認の試験（TASK-353 1/2。FEND-6・FEND-8・D-42。PoC の probe-esc に相当）。
//
// 受け持ち: root（#app）の外の Esc は glue が document の keydown で受けて届ける（root の中は上流の属性が届ける）。
// 4 つの場面（処理中か否か × フォーカスなし（body）か root の外のボタンか）のそれぞれで、
//   - Esc の受信回数（stateJson().escCount）の増分がちょうど 1
//   - 処理中でなければ取り消しの入力（input_received と judged（拒否）が各 1 件。停止は 0 件）
//   - 処理中なら停止（stopped が 1 件。input_received・judged・changed は 0 件）
// を確かめる。Esc の後に 2 フレームと数十 ms 待ち、遅れて届く 2 回目（配送は set_timeout(0)）が無いことも見る。
//
// 範囲外: 入力欄・root の中のボタンの 4 場面は TASK-353 2/2。SCENES に行を足して受け継ぐ。
//
// 処理中の作り方: 実 UI で下書きを送り、進行の行が 3 以上 5 未満（まだ終わっていない）になるのを待ってから Esc を押す。
// 停止は run を入口の写しへ戻すだけで、世界状態は戻さない。そのため停止では changed が出ない（2026-10-20 に実測。0 件を期待に固定する）。
// root の外のボタンは、この試験のページにだけ作る要素である（配信物には無い）。
// 変換中は CDP の模擬ではなく合成イベントで見る（シミュレーター。実 IME ではない）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-esc-once/';
const STATUS = '[data-testid="run-status"]';
const DRAFT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const LOG = '[data-testid="part-log-list"]';
const PROGRESS = `${LOG} li[data-kind="progress"]`;
const REJECTED = `${LOG} li[data-kind="rejected"]`;
const BODY = '/make-note first-note';

// 場面の表。#625 が textarea・inside-button の行を足す。
const SCENES = [
  { busy: false, where: 'body' },
  { busy: false, where: 'outside-button' },
  { busy: true, where: 'body' },
  { busy: true, where: 'outside-button' },
];

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

const snap = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson()));
const statusState = (page) => page.evaluate((sel) => document.querySelector(sel)?.getAttribute('data-state') ?? null, STATUS);
const count = (page, sel) => page.locator(sel).count();

async function waitUntil(what, read, pred, timeout = 8000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await read();
    if (await pred(last)) return last;
    await new Promise((r) => setTimeout(r, 10));
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

// 遅れて届く 2 回目の配送を拾うため、2 フレームと数十 ms 待つ。
async function settle(page) {
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  await new Promise((r) => setTimeout(r, 80));
}

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.emulateMedia({ reducedMotion: 'no-preference' });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

// フォーカスを作る。body は blur、outside-button は root の外にボタンを足して focus する。
async function focusAt(page, where) {
  if (where === 'outside-button') {
    await page.evaluate(() => {
      const b = document.createElement('button');
      b.type = 'button';
      b.setAttribute('data-testid', 'outside-btn');
      b.textContent = 'outside';
      document.body.appendChild(b);
      b.focus();
    });
    const ok = await page.evaluate(() => {
      const a = document.activeElement;
      return a?.getAttribute('data-testid') === 'outside-btn' && !document.getElementById('app').contains(a);
    });
    assert.equal(ok, true, 'フォーカスが root の外のボタンにない');
  } else {
    await page.evaluate(() => document.activeElement && document.activeElement.blur());
    const ok = await page.evaluate(() => document.activeElement === document.body);
    assert.equal(ok, true, 'フォーカスが body にない');
  }
}

// 処理中を作る。進行が 3 以上 5 未満（世界状態を変える段の後で、まだ終わっていない）まで待つ。
async function makeBusy(page) {
  await page.fill(DRAFT, BODY);
  await page.click(SUBMIT);
  await waitUntil('busy', () => statusState(page), (s) => s === 'busy');
  await waitUntil('progress>=3', () => count(page, PROGRESS), (n) => n >= 3);
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  for (const scene of SCENES) {
    const name = `${scene.busy ? '処理中' : '処理中でない'} × ${scene.where === 'body' ? 'フォーカスなし' : 'root の外のボタン'} の Esc はちょうど 1 回届く（CSP ${cspLabel}）`;
    test(name, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv);
        if (scene.busy) await makeBusy(page);
        await focusAt(page, scene.where);
        if (scene.busy) {
          assert.equal(await statusState(page), 'busy', 'Esc の前に処理中でない');
          assert.ok((await count(page, PROGRESS)) < 5, 'Esc の前に完了してしまった');
        }
        const b = await snap(page);
        const rowsBefore = b.engine.log.length;
        await page.keyboard.press('Escape');
        if (scene.busy) {
          await waitUntil('stopped', () => statusState(page), (s) => s === 'stopped');
        } else {
          await waitUntil('judged', () => snap(page), (s) => s.engine.judgedCategories.length === b.engine.judgedCategories.length + 1);
        }
        await settle(page);
        const a = await snap(page);
        assert.equal(a.escCount - b.escCount, 1, 'Esc の受信回数の増分がちょうど 1 でない');
        const added = a.engine.log.slice(rowsBefore).map((r) => r.kind);
        const n = (k) => added.filter((x) => x === k).length;
        if (scene.busy) {
          assert.equal(a.engine.generation, b.engine.generation + 1);
          assert.equal(await statusState(page), 'stopped');
          assert.equal(n('stopped'), 1, JSON.stringify(added));
          assert.equal(n('input_received'), 0, JSON.stringify(added));
          assert.equal(n('judged'), 0, JSON.stringify(added));
          assert.equal(n('changed'), 0, JSON.stringify(added));
        } else {
          assert.equal(a.engine.generation, b.engine.generation);
          assert.deepEqual(added, ['input_received', 'judged']);
          assert.equal(a.engine.logLen, b.engine.logLen + 2);
          assert.equal(n('stopped'), 0);
          assert.equal(JSON.stringify(a.engine.judgedCategories.at(-1)).includes('not_in_this_step'), true);
          assert.equal(await count(page, REJECTED), 1);
        }
        const safety = await collectSafety(page, rec);
        assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
        await context.close();
      });
    });
  }

  test(`root の外でも、修飾キー付き・変換中・Esc 以外のキーは届かない（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await focusAt(page, 'body');
      const b = await snap(page);
      await page.keyboard.press('Shift+Escape');
      await page.evaluate(() => {
        document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', isComposing: true, bubbles: true, cancelable: true }));
      });
      await page.keyboard.press('Enter');
      await settle(page);
      const a = await snap(page);
      assert.equal(a.escCount, b.escCount);
      assert.equal(a.engine.logLen, b.engine.logLen);
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    });
  });
}
