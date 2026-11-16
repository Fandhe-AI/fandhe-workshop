// 早送りの入・切の切り替えと状態の文字の試験（SHELL-24・SHELL-27。TASK-540 1/2。docs/testing.md 5.526）。
//
// 試験の設定（TEST_HOOKS=1）の既定の教材で、見えない `raw-trigger` から `/make-note`（進行 5 件）を送り、早送りの切り替えを
// 実際のボタンの click・キーだけで押す。native の同一性・区切りの間の停止は crates/web/tests/fast_forward_host.rs の担当である。
//   場面 A  状態の文字が「早送り: 切」（data-fast-forward=off）で、区画は shell-right の中・停止ボタンの後ろ・状態表示（run-status）の前
//   場面 B  click と Enter・Space で入・切が替わり、ボタンの要素が同じノードのまま・押した後もフォーカスがボタンに残り、読み込み直していない
//   場面 C  入で `/make-note` を送ると「早送り中」が出て消え、送ってから待機中に戻るまで 1000ms 未満（通常の速さは 5 × 300ms）
//   場面 D  入と切で、最後の状態（ログを除く）・進行の記録・書き出しの ops と snapshot が一致する（SHELL-27）
//   場面 E  処理中に切→入にすると残りがすぐ終わり、ignored が増えない（予約済みのタイマーは捨てる）
//   場面 F  配信する設定（IR を同梱した写し）でも A・B が動き、試験用の口が無い
//   共通    ページ内のエラー・CSP 違反が 0 件。試験の設定では安全性の計数（通信を含む）も 0 件（書き出しの場面 D を除く）
// 「早送り中」は数 ms しか出ないため、ポーリングではなく MutationObserver で出現を記録する。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_WEB_DIR（配信する設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { withDeliveryCopy } from './ch4-9-journey.mjs';
import { exportByUi } from './shell-journey-trips.mjs';

const BASE = '/fandhe-fast-forward/';
const RAW = '[data-testid="raw-trigger"]';
const SECTION = '[data-testid="shell-fast-forward"]';
const STATUS = '[data-testid="fast-forward-status"]';
const TOGGLE = '[data-testid="fast-forward-toggle"]';
const ACTIVE = '[data-testid="fast-forward-active"]';
const STOP = '[data-testid="stop-btn"]';
const RUN = '[data-testid="run-status"]';
const PROGRESS = '[data-testid="part-log-list"] li[data-kind="progress"]';
const INPUT = { input: { command: '/make-note first-note' } };
const TOTAL_TICKS = 5;
const MIN_IR = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../testdata/engine-event-consistency/test-data/min-3step.json',
);

let browser;
let tmpDir;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-fast-forward-'));
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

async function openScene(csp, fn, { dir = process.env.FANDHE_WEB_TEST_DIR, hooks = true, safety = true } = {}) {
  await withCspMode({ dir, csp }, (d) => withServer({ dir: d, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await page.goto(srv.url);
    await waitMounted(page);
    await fn(page);
    const s = summarize(rec);
    assert.equal(s.pageErrors, 0, JSON.stringify({ rec }));
    assert.equal(s.csp, 0, JSON.stringify({ rec }));
    if (hooks && safety) assert.deepEqual(nonZero(await collectSafety(page, rec)), [], JSON.stringify({ rec }));
    await context.close();
  }));
}

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;

const status = (page) => page.evaluate((s) => {
  const e = [...document.querySelectorAll(s)];
  return { count: e.length, text: e[0]?.textContent ?? null, state: e[0]?.getAttribute('data-fast-forward') ?? null };
}, STATUS);

const runState = (page) => page.evaluate((s) => document.querySelector(s)?.getAttribute('data-state') ?? null, RUN);

const waitRun = (page, state) => page.waitForFunction(
  ([s, want]) => document.querySelector(s)?.getAttribute('data-state') === want, [RUN, state], { timeout: 15000 },
);

const waitStatus = (page, state) => page.waitForFunction(
  ([s, want]) => document.querySelector(s)?.getAttribute('data-fast-forward') === want, [STATUS, state], { timeout: 15000 },
);

// 「早送り中」の出現を DOM の変化ごとに記録する（数 ms しか出ないため）。
const watchActive = (page) => page.evaluate((a) => {
  window.__ffSeen = { appeared: false, now: false };
  const mark = () => {
    const on = !!document.querySelector(a);
    window.__ffSeen.now = on;
    if (on) window.__ffSeen.appeared = true;
  };
  new MutationObserver(mark).observe(document.body, { childList: true, subtree: true, characterData: true });
  mark();
}, ACTIVE);

// A と B。試験の設定と配信する設定で共通。
async function checkPlaceAndToggle(page) {
  assert.deepEqual(await status(page), { count: 1, text: '早送り: 切', state: 'off' }, 'A: 開いた直後は切');
  assert.equal(await page.locator(SECTION).count(), 1);
  assert.equal(await page.locator(TOGGLE).count(), 1);
  assert.equal(await page.locator(ACTIVE).count(), 0);
  const place = await page.evaluate(([sec, stop, run]) => {
    const s = document.querySelector(sec);
    const before = (a, b) => !!(a && b && (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING));
    return {
      inRight: !!s.closest('[data-testid="shell-right"]'),
      afterStop: before(document.querySelector(stop), s),
      beforeRun: before(s, document.querySelector(run)),
    };
  }, [SECTION, STOP, RUN]);
  assert.deepEqual(place, { inRight: true, afterStop: true, beforeRun: true }, 'A: 区画の位置');

  await page.evaluate((t) => { window.__ffBtn = document.querySelector(t); window.__ffNoReload = 1; }, TOGGLE);
  await page.click(TOGGLE);
  await waitStatus(page, 'on');
  assert.equal((await status(page)).text, '早送り: 入', 'B: click で入');
  await page.click(TOGGLE);
  await waitStatus(page, 'off');
  await page.focus(TOGGLE);
  await page.keyboard.press('Enter');
  await waitStatus(page, 'on');
  await page.keyboard.press('Space');
  await waitStatus(page, 'off');
  assert.equal((await status(page)).text, '早送り: 切', 'B: Space で切');
  assert.ok(await page.evaluate((t) => window.__ffBtn === document.querySelector(t) && window.__ffBtn.isConnected, TOGGLE), 'B: ボタンは同じノード');
  assert.ok(await page.evaluate((t) => document.activeElement === document.querySelector(t), TOGGLE), 'B: フォーカスがボタンに残る');
  assert.equal(await page.evaluate(() => window.__ffNoReload), 1, 'B: 読み込み直していない');
}

// make-note を送って待機中に戻るまで流し、ログを除いた状態・進行の記録・書き出しを返す。
async function runToIdle(page, { on }) {
  if (on) {
    await page.click(TOGGLE);
    await waitStatus(page, 'on');
  }
  await sendRaw(page, INPUT);
  await page.waitForFunction(([p, n]) => document.querySelectorAll(p).length === n, [PROGRESS, TOTAL_TICKS], { timeout: 15000 });
  await waitRun(page, 'idle');
  const eng = await engine(page);
  const file = await exportByUi(page, tmpDir);
  const h = JSON.parse(fs.readFileSync(file, 'utf8'));
  fs.rmSync(file, { force: true });
  return { state: JSON.stringify(eng.stateWithoutLog), kinds: eng.log.map((x) => x.kind), ticks: eng.ticks, ops: h.ops, snapshot: h.snapshot };
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`A・B: 状態の文字と区画の位置、click とキーで切り替わる（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, checkPlaceAndToggle);
  });

  test(`C: 入で「早送り中」が出て消え、1000ms 未満で終わる（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, async (page) => {
      await page.click(TOGGLE);
      await waitStatus(page, 'on');
      assert.equal(await page.locator(ACTIVE).count(), 0, '処理中でなければ出ない');
      await watchActive(page);
      const t0 = Date.now();
      await sendRaw(page, INPUT);
      await page.waitForFunction(([p, n]) => document.querySelectorAll(p).length === n, [PROGRESS, TOTAL_TICKS], { timeout: 15000 });
      await waitRun(page, 'idle');
      const elapsed = Date.now() - t0;
      assert.ok(elapsed < 1000, `送ってから待機中まで ${elapsed}ms`);
      const seen = await page.evaluate(() => window.__ffSeen);
      assert.deepEqual(seen, { appeared: true, now: false }, '「早送り中」が出て消えた');
      assert.equal((await status(page)).text, '早送り: 入', '終わった後も入のまま');
    });
  });

  test(`D: 入と切で最後の状態・進行・書き出しの ops が一致する（${mode}）`, { timeout: 300000 }, async () => {
    const results = [];
    for (const on of [false, true]) {
      await openScene(csp, async (page) => { results.push(await runToIdle(page, { on })); }, { safety: false });
    }
    const [off, on] = results;
    assert.equal(off.ticks.length, TOTAL_TICKS);
    assert.deepEqual(on, off, 'SHELL-27');
    assert.ok(!JSON.stringify(on.ops).includes('fast'), '入・切は ops に入らない');
  });

  test(`E: 処理中に切→入にすると残りがすぐ終わり、ignored が増えない（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, async (page) => {
      await sendRaw(page, INPUT);
      await page.waitForFunction((p) => document.querySelectorAll(p).length >= 1, PROGRESS, { timeout: 15000 });
      const done = await page.locator(PROGRESS).count();
      assert.ok(done < TOTAL_TICKS && (await runState(page)) === 'busy', `処理中の途中で切り替える（${done} 件）`);
      const t0 = Date.now();
      await page.click(TOGGLE);
      await waitRun(page, 'idle');
      const elapsed = Date.now() - t0;
      // 通常の速さなら残り（4 件以下）に少なくとも 300ms × 残りの件数かかる。
      assert.ok(elapsed < 300 * (TOTAL_TICKS - done), `切り替えから待機中まで ${elapsed}ms（残り ${TOTAL_TICKS - done} 件）`);
      const eng = await engine(page);
      assert.equal(eng.ticks.length, TOTAL_TICKS);
      assert.equal(eng.log.filter((x) => x.kind === 'ignored').length, 0, '捨てたタイマーは届かない');
    });
  });

  test(`F: 配信する設定でも状態の文字と切り替えが動く（${mode}）`, { timeout: 300000 }, async () => {
    await withDeliveryCopy(async (copy) => {
      fs.copyFileSync(MIN_IR, path.join(copy, 'ir', 'main.json'));
      await openScene(csp, async (page) => {
        assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', '試験用の口が無い');
        assert.equal(await page.locator(RAW).count(), 0);
        await checkPlaceAndToggle(page);
      }, { dir: copy, hooks: false });
    });
  });
}
