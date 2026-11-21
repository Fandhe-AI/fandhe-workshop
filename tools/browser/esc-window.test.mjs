// 既定の速さ（進行の間隔 300ms のビルド時の定数）で、処理中に Esc を押して止められる長さがあることの画面の試験
// （TERM-34・D-43・D-47。TASK-453。#1044。docs/testing.md 5.536）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。配信する設定（試験用の口が無い）の写しに、試験用の最小の教材
// （min-3step）の st-1 の処理を 20 段にした IR を同梱して開く。20 段は試作の教材の ST-1.5 と同じ回数で、D-47 の「Esc の余裕は進行の回数で補う」の形。
// 操作は入力欄への入力と Enter・Esc のキーだけで、観察は DOM だけで行う（右のログの進行の行の出現時刻は MutationObserver で記録する）。
//   W1  止めずに待つと、進行の行が 20 件出て確認待ちになる（指す確認の行が指せる形で出る）。隣り合う行の間隔の中央値が 300ms 前後（250〜450ms）で、
//       最初の行から最後の行までが 5,000ms 以上（Esc を押せる時間の長さ）
//   W2  10 件目の進行の行が出た後（状態の確かめと記録の準備の後）に Esc を押すと、状態表示が「停止」（data-state=stopped）になり、停止の行が 1 件出て、
//       停止になった時点と 1,000ms 後の進行の行が、Esc の keydown が文書に届いた時点の件数のまま（20 件に届かない）
//   共通 早送りは切のまま（既定）。ページ内のエラー・CSP 違反が 0 件。CSP あり・なしで流す
// 範囲外: 人が押せるか（人の確認。TERM-34 の検討中の理由）、早送り（SHELL-24。fast-forward-toggle）、試験の設定での停止の網羅（esc-once・ch1-keyboard）。
// 入力: FANDHE_WEB_DIR（配信する設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted,
} from './harness.mjs';
import { withDeliveryCopy } from './ch4-9-journey.mjs';

const BASE = '/fandhe-esc-window/';
const DRAFT = '[data-testid="draft-input"]';
const RUN = '[data-testid="run-status"]';
const STATUS = '[data-testid="fast-forward-status"]';
const PROGRESS = '[data-testid="part-log-list"] li[data-kind="progress"]';
const STOPPED = '[data-testid="part-log-list"] li[data-kind="stopped"]';
// 確認（point_at）で指す行。受付の後に指せる形（data-action="ui"）で出る。
const CONFIRM_ROW = '[data-testid="part-artifact-view"] [data-row="notes/draft"][data-action="ui"]';
const STAGES = 20;
const STOP_AT = 10;
const T = 30000;
const MIN_IR = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../testdata/engine-event-consistency/test-data/min-3step.json',
);
let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to a web directory built without TEST_HOOKS');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// min-3step の st-1 の処理を 20 段にした IR。元の段は順のまま等間隔（0・5・10・15・19 段目）に置き、効果（左の行の状態の変化）を
// 1 つも落とさない。間は効果の無い待ちの段で埋める。
function irText() {
  const ir = JSON.parse(fs.readFileSync(MIN_IR, 'utf8'));
  const steps = ir.nodes[0].step.processing;
  const at = new Map(steps.map((p, k) => [Math.round((k * (STAGES - 1)) / (steps.length - 1)), p]));
  assert.equal(at.size, steps.length, 'every original stage keeps its own slot');
  ir.nodes[0].step.processing = Array.from({ length: STAGES }, (_, i) => at.get(i) ?? { name: `p-wait-${i}`, effects: [] });
  return JSON.stringify(ir);
}

const runState = (page) => page.evaluate((s) => document.querySelector(s)?.getAttribute('data-state') ?? null, RUN);
const progressRows = (page) => page.evaluate((s) => document.querySelectorAll(s).length, PROGRESS);

// 進行の行が増えた時刻を記録する（performance.now の ms）。
const watchProgress = (page) => page.evaluate((s) => {
  window.__escWindow = [];
  let seen = document.querySelectorAll(s).length;
  new MutationObserver(() => {
    const n = document.querySelectorAll(s).length;
    for (; seen < n; seen += 1) window.__escWindow.push(performance.now());
  }).observe(document.body, { childList: true, subtree: true });
}, PROGRESS);

async function scene(csp, fn) {
  await withDeliveryCopy(async (copy) => {
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), irText());
    await withCspMode({ dir: copy, csp }, (d) => withServer({ dir: d, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
      // 失敗しても文脈を閉じる（残ったページのタイマーが後の場面に影響しないように）。
      try {
        await page.emulateMedia({ reducedMotion: 'no-preference' });
        await page.goto(srv.url);
        await waitMounted(page);
        assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', '配信する設定（試験用の口が無い）');
        assert.equal(await page.evaluate((s) => document.querySelector(s)?.getAttribute('data-fast-forward'), STATUS), 'off', '早送りは切');
        await watchProgress(page);
        await page.fill(DRAFT, '/make-note first-note');
        await page.press(DRAFT, 'Enter');
        await fn(page);
        const s = summarize(rec);
        assert.equal(s.pageErrors, 0, JSON.stringify({ rec }));
        assert.equal(s.csp, 0, JSON.stringify({ rec }));
      } finally {
        await context.close();
      }
    }));
  });
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`W1: 止めずに待つと 300ms ごとに 20 段進み、Esc を押せる時間が 5,000ms 以上ある（${mode}）`, { timeout: 120000 }, async () => {
    await scene(csp, async (page) => {
      await page.waitForFunction(([s, n]) => document.querySelectorAll(s).length === n, [PROGRESS, STAGES], { timeout: T });
      await page.waitForFunction((s) => document.querySelector(s)?.getAttribute('data-state') === 'idle', RUN, { timeout: T });
      const times = await page.evaluate(() => window.__escWindow);
      assert.equal(times.length, STAGES, 'progress rows observed');
      const gaps = times.slice(1).map((t, i) => t - times[i]).sort((a, b) => a - b);
      const median = gaps[Math.floor(gaps.length / 2)];
      const span = times[times.length - 1] - times[0];
      console.log(`diagnostic esc-window: median-gap=${Math.round(median)}ms span=${Math.round(span)}ms`);
      assert.ok(median >= 250 && median <= 450, `median gap ${median}ms is about 300ms`);
      assert.ok(span >= 5000, `processing lasts ${span}ms (>= 5000ms)`);
      // 確認待ち: 指す確認の行が指せる形で出ている。
      assert.equal(await page.locator(CONFIRM_ROW).count(), 1, 'the confirm row is pointable after the processing');
    });
  });

  test(`W2: 処理の途中で Esc を押すと止まり、進行が増えない（${mode}）`, { timeout: 120000 }, async () => {
    await scene(csp, async (page) => {
      await page.waitForFunction(([s, n]) => document.querySelectorAll(s).length >= n, [PROGRESS, STOP_AT], { timeout: T });
      assert.equal(await runState(page), 'busy', 'still processing when Esc is pressed');
      // 基準は Esc の keydown が文書に届いた時点の件数（読み取りと押下の間に段が進んでも食い違わない）。capture で先に記録する。
      await page.evaluate((sel) => {
        document.addEventListener('keydown', (e) => {
          if (e.key === 'Escape' && window.__escRows === undefined) window.__escRows = document.querySelectorAll(sel).length;
        }, { capture: true });
      }, PROGRESS);
      await page.keyboard.press('Escape');
      const beforeEsc = await page.evaluate(() => window.__escRows);
      assert.equal(typeof beforeEsc, 'number', 'the Escape keydown reached the document');
      await page.waitForFunction((s) => document.querySelector(s)?.getAttribute('data-state') === 'stopped', RUN, { timeout: T });
      const atStop = await progressRows(page);
      await page.waitForTimeout(1000);
      assert.equal(atStop, beforeEsc, 'no progress between Esc and the stopped state');
      assert.equal(await progressRows(page), beforeEsc, 'no progress after the stop');
      assert.equal(await runState(page), 'stopped', 'still stopped after 1,000ms');
      assert.ok(atStop < STAGES, `stopped before the last stage (${atStop})`);
      assert.equal(await page.locator(STOPPED).count(), 1, 'one stopped row');
    });
  });
}
