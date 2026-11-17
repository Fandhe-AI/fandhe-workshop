// 早送りの入・切の切り替えと状態の文字の試験（SHELL-24・SHELL-27。TASK-540 1/2。docs/testing.md 5.526）。
//
// 試験の設定（TEST_HOOKS=1）の既定の教材で、見えない `raw-trigger` から `/make-note`（進行 5 件）を送り、早送りの切り替えを
// 実際のボタンの click・キーだけで押す。native の同一性・区切りの間の停止は crates/web/tests/fast_forward_host.rs の担当である。
//   場面 A  状態の文字が「早送り: 切」（data-fast-forward=off）で、区画は shell-right の中・停止ボタンの後ろ・状態表示（run-status）の前
//   場面 B  click と Enter・Space で入・切が替わり、ボタンの要素が同じノードのまま・押した後もフォーカスがボタンに残り、読み込み直していない
//   場面 C  入で `/make-note` を送ると「早送り中」が出て消え、送ってから待機中に戻るまで 1000ms 未満（通常の速さは 5 × 300ms）
//   場面 D  入と切で、最後の状態（ログを除く）・進行の記録・書き出しの ops と snapshot が一致する（SHELL-27）
//   場面 E  処理中に切→入にすると残りがすぐ終わり、ignored が増えない（予約済みのタイマーは捨てる）
//   場面 G  通常の速さに固定した Step（SHELL-26。`normal_speed`）に入のまま入ると、ボタン 0・文言 1・「早送り中」0 で 1 件ずつ 300ms で進む
//   場面 H  切り替えにフォーカスを置いたまま固定の Step に入ると、フォーカスは body に戻り、処理中の Esc で止まる
//   場面 F  配信する設定（IR を同梱した写し）でも A・B が動き、試験用の口が無い
//   場面 I  試験の設定で、設定なしは切で開き、入にしても保存先が変わらず読み込み直すと切。`pack.fast_forward: true` の IR に差し替えると入（SHELL-25・SESS-3）
//   場面 J  配信する設定で、設定あり・なしの IR を同梱して開くとそれぞれ入・切で、切り替えても保存先が変わらず、読み込み直すと初期値に戻る
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
const PINNED = '[data-testid="fast-forward-pinned"]';
const ROW = '[data-testid="part-artifact-view"] li[data-row="notes/draft"]';
const CHOICE = { input: { choice: 'opt-memo' } };
const TOTAL_TICKS = 5;
const PINNED_TICKS = 3;
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

// min-3step の st-2（選択）に処理 3 件と `normal_speed: true` を差した IR（試験の設定の `loadIr` で差し替える）。
function pinnedIrText() {
  const ir = JSON.parse(fs.readFileSync(MIN_IR, 'utf8'));
  ir.nodes[1].step.processing = ['q1', 'q2', 'q3'].map((name) => ({ name, effects: [] }));
  ir.nodes[1].step.normal_speed = true;
  return JSON.stringify(ir);
}

const clickRaw = (page, sel) => page.evaluate((x) => document.querySelector(x).click(), sel);

// 差し替え → 入 → st-1 を進めて確認し、固定の st-2 の入力待ちに入る。フォーカスは click 以外では動かさない。
async function enterPinned(page, { focusToggle = false } = {}) {
  assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), pinnedIrText()), true, 'load-ir sent');
  await waitMounted(page);
  await page.click(TOGGLE);
  await waitStatus(page, 'on');
  await sendRaw(page, INPUT);
  await waitRun(page, 'idle');
  await page.waitForFunction((p) => document.querySelectorAll(p).length === 5, PROGRESS, { timeout: 15000 });
  if (focusToggle) await page.focus(TOGGLE);
  await clickRaw(page, ROW);
  await page.waitForSelector(PINNED, { timeout: 15000 });
}

// `pack.fast_forward` を `on` にした min-3step の IR（SHELL-25）。`false` は省略と同じ（書き出さない）。
function ffIrText(on) {
  const ir = JSON.parse(fs.readFileSync(MIN_IR, 'utf8'));
  if (on) ir.pack.fast_forward = true;
  return JSON.stringify(ir);
}

// 保存先（localStorage）の全体の写し。鍵の順に並べる。
const storage = (page) => page.evaluate(() => JSON.stringify(Object.keys(localStorage).sort().map((k) => [k, localStorage.getItem(k)])));

// 切り替えを 1 回押し、保存の遅延（1 秒以内。SAVE-7）を過ぎても保存先が変わらないことを確かめる。
async function toggleKeepsStorage(page, want) {
  const before = await storage(page);
  await page.click(TOGGLE);
  await waitStatus(page, want);
  await page.waitForTimeout(1500);
  assert.equal(await storage(page), before, '切り替えで保存先が変わらない');
}

async function reloadMounted(page) {
  await page.reload();
  await waitMounted(page);
}

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

  test(`G: 固定の Step は入のままでも 1 件ずつ進み、ボタン 0・文言 1・早送り中 0（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, async (page) => {
      await enterPinned(page);
      assert.equal(await page.locator(PINNED).textContent(), 'この場面は通常の速さです');
      assert.equal(await page.locator(TOGGLE).count(), 0, 'ボタンは出ない');
      assert.deepEqual(await status(page), { count: 1, text: '早送り: 入', state: 'on' });
      await watchActive(page);
      const before = await page.locator(PROGRESS).count();
      const t0 = Date.now();
      await sendRaw(page, CHOICE);
      await page.waitForFunction(([p, n]) => document.querySelectorAll(p).length === n, [PROGRESS, before + PINNED_TICKS], { timeout: 15000 });
      await waitRun(page, 'idle');
      const elapsed = Date.now() - t0;
      // 通常の速さは 3 × 300ms。連続適用なら数 ms で終わる。
      assert.ok(elapsed >= 600, `1 件ずつ進んだ（${elapsed}ms）`);
      assert.deepEqual(await page.evaluate(() => window.__ffSeen), { appeared: false, now: false }, '「早送り中」は出ない');
    });
  });

  test(`H: 切り替えにフォーカスを置いたまま固定の Step に入り、Esc で止まる（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, async (page) => {
      await enterPinned(page, { focusToggle: true });
      assert.equal(await page.locator(TOGGLE).count(), 0);
      assert.ok(await page.evaluate(() => document.activeElement === document.body), 'フォーカスは body に戻る');
      const before = await page.locator(PROGRESS).count();
      await sendRaw(page, CHOICE);
      await page.waitForFunction(([p, n]) => document.querySelectorAll(p).length >= n, [PROGRESS, before + 1], { timeout: 15000 });
      assert.equal(await runState(page), 'busy');
      await page.keyboard.press('Escape');
      await waitRun(page, 'stopped');
      // 止まる件数は native の「固定の step の処理中の停止は通常の速さと同じ結果」が持つ（CI の待ちで件数が揺れるため判定しない）。
      const eng = await engine(page);
      assert.ok(eng.log.some((x) => x.kind === 'stopped'), '停止が記録される');
    });
  });

  test(`I: 試験の設定で初期値が教材の設定に従い、切り替えは保存されない（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, async (page) => {
      assert.equal((await status(page)).state, 'off', '設定なしは切');
      // 履歴を作り、保存が落ち着いてから切り替える（切り替えが保存を起こさないこと）。
      await sendRaw(page, INPUT);
      await waitRun(page, 'idle');
      await page.waitForFunction(() => localStorage.length > 0, null, { timeout: 15000 });
      await page.waitForTimeout(1500);
      await toggleKeepsStorage(page, 'on');
      await reloadMounted(page);
      assert.equal((await status(page)).state, 'off', '読み込み直すと切に戻る');
      assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), ffIrText(true)), true, 'load-ir sent');
      await waitStatus(page, 'on');
      assert.equal((await status(page)).text, '早送り: 入', '設定ありの教材は入');
      await toggleKeepsStorage(page, 'off');
    });
  });

  test(`J: 配信する設定で設定あり・なしの初期値に、読み込み直すと戻る（${mode}）`, { timeout: 300000 }, async () => {
    for (const on of [true, false]) {
      await withDeliveryCopy(async (copy) => {
        fs.writeFileSync(path.join(copy, 'ir', 'main.json'), ffIrText(on));
        await openScene(csp, async (page) => {
          assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', '試験用の口が無い');
          const [first, flipped] = on ? ['on', 'off'] : ['off', 'on'];
          assert.equal((await status(page)).state, first, `開いた直後（設定 ${on}）`);
          await toggleKeepsStorage(page, flipped);
          await reloadMounted(page);
          assert.equal((await status(page)).state, first, `読み込み直すと初期値（設定 ${on}）`);
        }, { dir: copy, hooks: false });
      });
    }
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
