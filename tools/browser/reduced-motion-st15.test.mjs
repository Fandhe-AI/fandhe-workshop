// reduce の文脈の ST-1.5 と、設定を反映する時期の試験（TASK-378 1/2。A11Y-12・A11Y-13・A11Y-29。決定は D-82）。
//
// D-82 は、reduce では処理中の段階の表示を省いて結果だけを出す現状の方式を採り、設定は読み込み時に 1 回だけ読む（開いたままの
// 切り替えには追随せず、開き直したときに反映される。既知の不足 H-12）と決めた。エンジンは reduce のために停止を体験させる仕組みを
// 足さない。ここでは、その形が実際の画面で成り立つことを確かめる。確かめること:
//   A. reduce で ST-1.5（`?fixture=ch0-3-links&start=s-1-5`。4 進行）を流すと、段階の行・progress 要素が全時点で 0 件。進行は 4 件届き、結果が出る
//   B. 同じ操作を no-preference でも流し、段階の行を除いた状態・成果物・ログが一致し、進行の回数と間隔が増えない（進行を遅らせる方式を採らない）
//   C. reduce でも Esc・停止ボタンの意味は変わらない（停止の行が 1 件・世代 +1・入口の状態へ戻る）。処理中の窓も no-preference と同じに残る
//   D. reduce だけの部品・案内が無い（画面の部品の集合・ボタンの数が no-preference と一致する）
//   E. 設定は読み込み時に 1 回だけ読む。開いたままの切り替えでは方式が変わらず、保存先の鍵にも入らない。開き直すと反映される（両方向）
// 左の成果物が処理の途中で変わること（5.307 に記録）は A11Y-11 の範囲の観察であり、ここでは数えない。
// 自動ブラウザの prefers-reduced-motion は模擬であり、実機の OS の設定の切り替えや支援技術での確認ではない。
// 待ちは固定の sleep ではなく、上限つきのポーリングで書く。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-reduced-motion-st15/';
const ST15 = '?fixture=ch0-3-links&start=s-1-5';
const STATUS = '[data-testid="run-status"]';
const DRAFT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const STOP = '[data-testid="stop-btn"]';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const PROGRESS_LI = `${LOG} li[data-kind="progress"]`;
const BODY = '/make-card sample';
const ST15_TICKS = 4;
const DEFAULT_TICKS = 5;

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

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;

async function waitUntil(what, read, pred, timeout = 10000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await read();
    if (await pred(last)) return last;
    await new Promise((r) => setTimeout(r, 20));
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

// 1 時点の標本。段階の行・progress 要素・状態の語・画面の部品（data-testid の集合）・ボタンの数。
const sample = (page) => page.evaluate(({ status, li }) => ({
  state: document.querySelector(status)?.getAttribute('data-state') ?? null,
  progressRows: document.querySelectorAll(li).length,
  progressEls: document.querySelectorAll('progress').length,
  testids: [...new Set([...document.querySelectorAll('[data-testid]')].map((e) => e.getAttribute('data-testid')))],
  buttons: document.querySelectorAll('button').length,
}), { status: STATUS, li: PROGRESS_LI });

// ページ内で段階の行・progress 要素が足されたことを数える。ポーリングの間に一瞬だけ現れて消える分も拾う。
const WATCH = () => {
  window.__rmAdded = 0;
  const isProgress = (n) => n.nodeType === 1
    && (n.matches('li[data-kind="progress"], progress') || n.querySelector('li[data-kind="progress"], progress'));
  new MutationObserver((ms) => {
    for (const m of ms) for (const n of m.addedNodes) if (isProgress(n)) window.__rmAdded += 1;
  }).observe(document.documentElement, { childList: true, subtree: true });
};

async function openSt15(srv, media) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.emulateMedia({ reducedMotion: media });
  await opened.page.goto(`${srv.url}${ST15}`);
  await waitMounted(opened.page);
  return opened;
}

async function submit(page) {
  await page.fill(DRAFT, BODY);
  await page.click(SUBMIT);
}

// 送信して busy → idle まで標本を採る。処理中を観察できなければ失敗にする。
async function runToIdle(page) {
  await submit(page);
  const samples = [];
  let sawBusy = false;
  await waitUntil('busy then idle', async () => {
    const s = await sample(page);
    samples.push(s);
    if (s.state === 'busy') sawBusy = true;
    return s;
  }, (s) => sawBusy && s.state === 'idle');
  assert.ok(sawBusy, '処理中を観察できなかった');
  return samples;
}

// 表示だけの差（reducedMotion と段階の行）を除いた、比較用の写し。
const outcome = (e) => ({
  current: e.current,
  generation: e.generation,
  runStatus: e.runStatus,
  artifacts: e.artifacts,
  stateWithoutLog: e.stateWithoutLog,
  // kind はイベントの種類（表示の有無に依らない）。line は表示する行の種類で、reduce では段階の行が null になる。
  eventKinds: e.log.map((r) => r.kind),
  lines: e.log.map((r) => r.line).filter((l) => l !== null && l !== 'progress'),
  ticks: e.ticks.map(([g, i]) => [g, i]),
  worldMatchesState: e.worldMatchesState,
});

for (const csp of [true, false]) {
  const mode = csp ? 'CSP あり' : 'CSP なし';

  test(`A: reduce の ST-1.5 で段階の行が 0 件で、進行は 4 件届き結果が出る（${mode}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openSt15(srv, 'reduce');
      await page.evaluate(WATCH);
      const before0 = await engine(page);
      assert.equal(before0.reducedMotion, true);
      assert.equal(before0.current, 'main/st-1-5');

      const samples = await runToIdle(page);
      assert.ok(samples.every((s) => s.progressRows === 0 && s.progressEls === 0), JSON.stringify(samples));
      assert.equal(await page.evaluate(() => window.__rmAdded), 0, '段階の行が一瞬でも足された');

      const e = await engine(page);
      assert.equal(e.ticks.filter(([g]) => g === before0.generation).length, ST15_TICKS, '進行は起き、表示だけを省く');
      assert.equal(e.runStatus, 'idle');
      assert.equal(e.log.filter((r) => r.line === 'progress').length, 0);
      assert.ok(e.log.some((r) => r.line === 'received'), '受付の行が出ている');
      assert.ok(e.log.length > 0 && e.worldMatchesState && e.incrementalMatches);
      assert.equal(await page.locator(`${LOG} li[data-kind="received"]`).count() >= 1, true, '結果の行が DOM に出ている');

      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    }));
  });

  test(`E: 開いたままの切り替えには追随せず、開き直すと反映される（${mode}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const storageKeys = (p) => p.evaluate(() => Object.keys(localStorage).sort());
      for (const [from, to] of [['no-preference', 'reduce'], ['reduce', 'no-preference']]) {
        const wantFrom = from === 'reduce';
        const { page, context, rec } = await openSt15(srv, from);
        assert.equal((await engine(page)).reducedMotion, wantFrom);
        const keysBefore = await storageKeys(page);

        // 開いたまま切り替える。matchMedia は変わるが、画面の方式は変わらない。
        await page.emulateMedia({ reducedMotion: to });
        assert.equal(await page.evaluate(() => matchMedia('(prefers-reduced-motion: reduce)').matches), !wantFrom);
        assert.equal((await engine(page)).reducedMotion, wantFrom, '切り替えに追随した');
        await page.evaluate(WATCH);
        const samples = await runToIdle(page);
        const maxRows = Math.max(...samples.map((s) => s.progressRows));
        if (wantFrom) {
          assert.equal(maxRows, 0, '開いたままの切り替えの後に段階の行が出た（reduce のまま）');
        } else {
          assert.equal(await page.evaluate(() => window.__rmAdded), ST15_TICKS, '開いたままの切り替えの後は段階の行が 1 回ずつ出る（no-preference のまま）');
        }
        // 動きの値は保存先の鍵にも履歴にも入らない。
        const keysAfter = await storageKeys(page);
        // 履歴の鍵は最初の保存で足される。切り替えの前から持っていた鍵は残り、動きの値を含む鍵は無い。
        assert.ok(keysBefore.every((k) => keysAfter.includes(k)));
        assert.ok(keysAfter.every((k) => !/motion|reduce/i.test(k.replace(BASE.replaceAll('/', ''), ''))), JSON.stringify(keysAfter));
        const stored = await page.evaluate(() => Object.keys(localStorage).map((k) => localStorage.getItem(k)).join('\n'));
        assert.ok(!/reducedMotion|reduced_motion|prefers-reduced/i.test(stored), '履歴・保存先に動きの値が入った');
        const stepAfter = (await engine(page)).current;

        // 開き直す。同じ保存先から続き、新しい設定が反映される。
        await page.goto(`${srv.url}${ST15}`);
        await waitMounted(page);
        const reopened = await engine(page);
        assert.equal(reopened.reducedMotion, !wantFrom, '開き直しても反映されない');
        assert.equal(reopened.current, stepAfter, '保存済みの履歴から続いていない');
        assert.deepEqual(await storageKeys(page), keysAfter);
        const rows = await page.locator(PROGRESS_LI).count();
        if (!wantFrom) assert.equal(rows, 0, '開き直した reduce で段階の行が出た');
        else assert.equal(rows, ST15_TICKS, '開き直した no-preference で復元した段階の行が出ない');

        const safety = await collectSafety(page, rec);
        assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
        await context.close();
      }
    }));
  });
}

test('B・D: reduce と no-preference で段階の行を除いた結果・部品が一致し、進行の間隔が増えない', { timeout: 120000 }, async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const runs = {};
    for (const media of ['reduce', 'no-preference']) {
      const { page, context } = await openSt15(srv, media);
      const started = Date.now();
      const samples = await runToIdle(page);
      const elapsed = Date.now() - started;
      runs[media] = { e: await engine(page), samples, elapsed };
      await context.close();
    }
    const r = runs.reduce;
    const n = runs['no-preference'];
    assert.equal(r.e.reducedMotion, true);
    assert.equal(n.e.reducedMotion, false);
    // B: 表示だけの差（reducedMotion・段階の行）を除くと同じ。
    assert.deepEqual(outcome(r.e), outcome(n.e));
    assert.equal(r.e.ticks.length, ST15_TICKS);
    assert.equal(n.e.ticks.length, ST15_TICKS);
    // 進行を遅らせる方式を採っていない。同じ試験の環境で、reduce の所要が no-preference より 1 進行分（300ms）以上長くならない。
    assert.ok(r.elapsed <= n.elapsed + 300, `reduce の処理が遅い: ${r.elapsed}ms / ${n.elapsed}ms`);
    // D: reduce だけの部品が無い。部品の集合とボタンの数が一致する（段階の行は data-testid を持たない）。
    const union = (samples) => [...new Set(samples.flatMap((s) => s.testids))].sort();
    // 差は段階の行の中の部品（line-stage）だけ。reduce にだけある部品は無い。
    assert.deepEqual(union(r.samples), union(n.samples).filter((t) => t !== 'line-stage'), 'reduce だけ・no-preference だけの部品がある');
    assert.ok(union(n.samples).includes('line-stage'), '対照の no-preference に段階の部品が出ていない');
    assert.equal(Math.max(...r.samples.map((s) => s.buttons)), Math.max(...n.samples.map((s) => s.buttons)), 'ボタンの数が違う');
    assert.ok(n.samples.some((s) => s.progressRows > 0), '対照の no-preference に段階の行が出ていない');
  });
});

for (const how of ['esc', 'button']) {
  test(`C: reduce でも ${how === 'esc' ? 'Esc' : '停止ボタン'}の意味は変わらず、no-preference と同じ結果になる`, { timeout: 120000 }, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      const results = {};
      for (const media of ['reduce', 'no-preference']) {
        const { page, context, rec } = await openSt15(srv, media);
        const before0 = await engine(page);
        await submit(page);
        // 処理中の窓は reduce でも残る（段階の手がかりが無いだけで、処理中の語と停止の操作は変わらない）。
        await waitUntil('busy', () => sample(page), (s) => s.state === 'busy');
        if (how === 'esc') {
          await page.focus(DRAFT);
          await page.keyboard.press('Escape');
        } else {
          await page.click(STOP);
        }
        await waitUntil('stopped', () => sample(page), (s) => s.state === 'stopped');
        const e = await engine(page);
        assert.equal(e.runStatus, 'stopped');
        assert.equal(e.generation, before0.generation + 1);
        assert.equal(await page.locator(`${LOG} li[data-kind="stopped"]`).count(), 1);
        if (media === 'reduce') assert.equal(await page.locator(PROGRESS_LI).count(), 0);
        assert.deepEqual(e.artifacts, before0.artifacts, '処理中の効果が取り消される');
        assert.equal(e.worldMatchesState, true);
        const strip = (run) => { const { generation, ...rest } = run; return rest; };
        assert.deepEqual(strip(e.stateWithoutLog.run), strip(before0.stateWithoutLog.run), '入口の状態へ戻る');
        results[media] = { generation: e.generation, runStatus: e.runStatus, stopped: 1, artifacts: e.artifacts };

        // 止めた後にやり直せる。
        await submit(page);
        await waitUntil('busy again', () => sample(page), (s) => s.state === 'busy');
        await waitUntil('idle', () => sample(page), (s) => s.state === 'idle');
        assert.equal((await engine(page)).runStatus, 'idle');
        const safety = await collectSafety(page, rec);
        assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
        await context.close();
      }
      assert.deepEqual(results.reduce, results['no-preference']);
    });
  });
}

test('E: 既定の教材でも、開いたまま reduce へ切り替えても段階の行が出続け、開き直すと 0 件になる', { timeout: 120000 }, async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await page.goto(srv.url);
    await waitMounted(page);
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.evaluate(([sel, json]) => {
      const el = document.querySelector(sel);
      el.setAttribute('data-payload', json);
      el.click();
    }, [RAW, JSON.stringify({ input: { command: '/make-note first-note' } })]);
    await waitUntil('idle', () => sample(page), (s) => s.state === 'idle' && s.progressRows > 0);
    assert.equal((await engine(page)).reducedMotion, false);
    assert.equal(await page.locator(PROGRESS_LI).count(), DEFAULT_TICKS);
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal((await engine(page)).reducedMotion, true);
    assert.equal(await page.locator(PROGRESS_LI).count(), 0, '開き直した reduce で復元した段階の行が出た');
    await context.close();
  });
});

test('F: 配信設定でも reduce で ST-1.5 の入口を開いてマウントでき、試験の口が無く、違反が 0 件である', { timeout: 120000 }, async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    assert.equal(await page.locator(RAW).count(), 0);
    assert.equal(await page.locator(STOP).count(), 1, '停止の操作は reduce でも 1 件のまま');
    const safety = await collectSafety(page, rec);
    assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
    await context.close();
  });
});
