// 通常の操作の間、root の全置換が起きず、要素の同一性・フォーカス・入力途中の値が保たれることの試験
// （TASK-348 2/3。FEND-3。PoC-1 の B-7 相当）。
//
// 上流の `Runtime::apply_update_for_dirty` は、mount 時の束縛点の表にも `data-bind-list` にも無い field が dirty に
// あると構造フォールバック（`apply_subtree_swap`）に入り、mount の root の子を全部取り除いて作り直す。
// 作り直されると、フォーカス・入力途中の値・要素の参照が失われる。native の試験（crates/web/tests/dirty_binding_points.rs）が
// 条件を目印で模して守るのに対し、ここでは実際のブラウザの DOM で結果を確かめる。
// 確かめること（試験の設定の画面で、入力の受付・進行・保存・確認の順に操作する）:
//   (1) `#app` の子の取り除き（root の全置換）が 0 回。MutationObserver の計数と、根の要素の同一性の 2 つの信号で見る
//   (2) 参照を保持した 18 要素（リストの親 6 本を含む）が、操作の後も `===` かつ `isConnected` である
//   (3) 入力欄のフォーカスが保たれる（入力欄にフォーカスした後は、フォーカスを動かす操作をしない）
//   (4) 入力欄の入力途中の値が保たれる
//   (5) 空振りでない。送信・進行・保存・確認が実際に起きている
// 負の対照 B-7n（TASK-348 3/3）: 試験用の口の全再描画（`rerender`）を起こすと、参照を保持した 6 要素が 1 つも残らず、
// 入力欄のフォーカスが失われることを確かめる。B-7 の検査器（changedRefs・readSwaps・フォーカスの判定）が、
// 実際に差を検出できることの裏づけである（検査器が常に通る空振りの否定）。
// 呼び出し元は make test-browser（scripts/test-browser.sh）。1 件ごとに新しい context を使う。
//
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero, summarize,
} from './harness.mjs';

const BASE = '/fandhe-element-identity/';
const RAW = '[data-testid="raw-trigger"]';
const DRAFT = '[data-testid="draft-input"]';
const TOTAL_TICKS = 5;

// 参照を保持する 18 要素。製品の data-testid で選び、#app の中でちょうど 1 件に当たる。
// 試験の設定だけにある要素（raw-trigger・keydown-probe）は含めない。
const PINNED = {
  root: '[data-testid="workshop-root"]',
  hello: '[data-testid="hello-root"]',
  parts: '[data-testid="workshop-parts"]',
  draft: '[data-testid="draft-input"]',
  submit: '[data-testid="submit-input"]',
  stop: '[data-testid="stop-btn"]',
  notice: '[data-testid="input-notice"]',
  partLog: '[data-testid="part-log"]',
  logConsumed: '[data-testid="part-log-consumed"]',
  runStatus: '[data-testid="run-status"]',
  stepStatus: '[data-testid="step-status"]',
  partArtifact: '[data-testid="part-artifact-view"]',
  logList: '[data-testid="log-list"]',
  partLogList: '[data-testid="part-log-list"]',
  reviewList: '[data-testid="part-review-list"]',
  candidateList: '[data-testid="candidate-list"]',
  viewSelect: '[data-testid="view-select"]',
  viewList: '[data-testid="view-list"][data-view="file_tree"]',
};

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

// 値は引数で渡し、コードの文字列を組み立てない。
function sendRaw(page, op) {
  return page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const nextTick = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.nextTick()));
const count = (page, sel) => page.evaluate((s) => document.querySelectorAll(s).length, sel);

// 上限つきのポーリング。上限を超えたら、最後の状態の要約を添えて失敗にする。
async function waitUntil(page, what, pred, timeout = 8000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await engine(page);
    if (await pred(last)) return last;
    await page.waitForTimeout(25);
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify({ current: last?.current, logLen: last?.logLen })}`);
}

// 保持する要素を window.__pinned へ控える。各選択子が #app の中でちょうど 1 件に当たることを確かめる。
async function pinRefs(page, table) {
  const hits = await page.evaluate((t) => {
    const out = {};
    window.__pinned = {};
    for (const [name, sel] of Object.entries(t)) {
      const found = document.querySelectorAll(`#app ${sel}`);
      out[name] = found.length;
      window.__pinned[name] = found[0] ?? null;
    }
    return out;
  }, table);
  for (const [name, n] of Object.entries(hits)) assert.equal(n, 1, `pinned element ${name} must match exactly once`);
  return Object.keys(hits);
}

// 同一でない（別の要素になった・切り離された）要素の名前の列。失敗の文面にどれが外れたかが出る。
function changedRefs(page, table) {
  return page.evaluate((t) => Object.keys(window.__pinned).filter((name) => {
    const now = document.querySelector(`#app ${t[name]}`);
    return window.__pinned[name] !== now || !window.__pinned[name].isConnected;
  }), table);
}

// #app の子の取り除き（root の全置換）の計数器。
function installSwapCounter(page) {
  return page.evaluate(() => {
    window.__rootSwaps = 0;
    const mo = new MutationObserver((records) => {
      for (const r of records) if (r.removedNodes.length > 0) window.__rootSwaps += 1;
    });
    mo.observe(document.getElementById('app'), { childList: true, subtree: false });
    window.__swapObserver = mo;
  });
}

// 読むときは未配送の記録も数える（コールバックが非同期のため）。
function readSwaps(page) {
  return page.evaluate(() => {
    for (const r of window.__swapObserver.takeRecords()) if (r.removedNodes.length > 0) window.__rootSwaps += 1;
    return window.__rootSwaps;
  });
}

test('B-7: 入力・進行・保存・確認の間 root が置き換わらず 要素の同一性とフォーカスと入力途中の値が保たれる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      const names = await pinRefs(page, PINNED);
      assert.equal(names.length, 18);
      await installSwapCounter(page);
      const start = await engine(page);
      const logStart = await count(page, '[data-testid="part-log-list"] > li');

      // 入力欄にフォーカスし、打って送る。以降、フォーカスを動かす操作をしない（raw-trigger は JS の click）。
      await page.locator(DRAFT).focus();
      await page.keyboard.type('/make-note first-note');
      await page.keyboard.press('Enter');
      await waitUntil(page, 'submit accepted', (e) => e.logLen > start.logLen);
      const run = await waitUntil(page, 'all ticks', (e) => e.ticks.filter(([g]) => g === e.generation).length >= TOTAL_TICKS);
      assert.equal(await nextTick(page), null, '進行が完了している');

      await sendRaw(page, { control: 'save' });
      await page.keyboard.type('/make-no');
      const typed = await engine(page);

      // 確認の操作。受け付けられたことは engine のログの増加で確かめる。
      await sendRaw(page, { input: { ui: { view: 'v-files', path: 'notes/draft' } } });
      await waitUntil(page, 'ui op logged', (e) => e.logLen > typed.logLen);
      const afterUi = await engine(page);
      await sendRaw(page, { input: { choice: 'opt-memo' } });
      await waitUntil(page, 'choice logged', (e) => e.logLen > afterUi.logLen);
      const afterChoice = await engine(page);
      await sendRaw(page, { control: 'save' });
      await waitUntil(page, 'second save logged', (e) => e.logLen > afterChoice.logLen);

      // (1)(2)
      assert.equal(await readSwaps(page), 0, 'root の子の取り除きが 0 回');
      assert.deepEqual(await changedRefs(page, PINNED), [], '同一でなくなった要素');
      // (3)(4)
      assert.equal(
        await page.evaluate(() => document.activeElement === window.__pinned.draft),
        true,
        '入力欄のフォーカスが保たれている',
      );
      assert.equal(await page.evaluate(() => window.__pinned.draft.value), '/make-no');
      assert.equal(await page.evaluate(() => window.__fandheTest.draft()), '/make-no');
      // (5) 空振りでない
      assert.equal(await count(page, '[data-testid="log-list"] > li'), 1, '送信が 1 件反映された');
      assert.ok((await count(page, '[data-testid="part-log-list"] > li')) > logStart, 'ログの行が増えた');
      assert.ok(run.logLen > start.logLen);
      assert.equal(run.ticks.filter(([g]) => g === run.generation).length, TOTAL_TICKS);

      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
      const s = summarize(rec);
      assert.equal(s.csp + s.pageErrors + s.consoleErrors, 0, JSON.stringify(rec));
    } finally {
      await context.close();
    }
  });
});

// 負の対照の 6 要素。PINNED の部分集合で、選択子を B-7 と同じにする（PoC の B-7n の 6 要素に対応）。
const CONTROL_KEYS = ['root', 'draft', 'submit', 'stop', 'viewList', 'partLogList'];
const CONTROL = Object.fromEntries(CONTROL_KEYS.map((k) => [k, PINNED[k]]));

test('B-7n: 全再描画を起こすと 保持した 6 要素が 1 つも残らず 入力欄のフォーカスが失われる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      const names = await pinRefs(page, CONTROL);
      assert.equal(names.length, 6);
      await installSwapCounter(page);

      await page.locator(DRAFT).focus();
      await page.keyboard.type('/make-no');

      // 前提: 作り直しの前は、検査器が何も検出せず、フォーカスが入力欄にある（別の理由で失敗していないことの確認）。
      assert.deepEqual(await changedRefs(page, CONTROL), [], '作り直しの前に外れた要素が無い');
      assert.equal(await readSwaps(page), 0, '作り直しの前の取り除きが 0 回');
      assert.equal(await page.evaluate(() => document.activeElement === window.__pinned.draft), true);

      assert.equal(await page.evaluate(() => window.__fandheTest.rerender()), true, 'rerender が実行された');
      await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));

      // 期待どおりの失敗: B-7 の検査器が差を検出する。
      assert.ok((await readSwaps(page)) >= 1, 'root の子の取り除きが起きた');
      const changed = await changedRefs(page, CONTROL);
      const kept = CONTROL_KEYS.filter((k) => !changed.includes(k));
      assert.equal(changed.length, 6, `残った要素が 0 件であること。残った要素: ${JSON.stringify(kept)}`);
      const active = await page.evaluate(() => ({
        tag: document.activeElement?.tagName ?? null,
        testid: document.activeElement?.getAttribute?.('data-testid') ?? null,
        isPinned: document.activeElement === window.__pinned.draft,
        isNewDraft: document.activeElement === document.querySelector('#app [data-testid="draft-input"]'),
      }));
      assert.equal(active.isPinned || active.isNewDraft, false, `フォーカスが失われた。観察値: ${JSON.stringify(active)}`);

      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
      const s = summarize(rec);
      assert.equal(s.csp + s.pageErrors + s.consoleErrors, 0, JSON.stringify(rec));
    } finally {
      await context.close();
    }
  });
});
