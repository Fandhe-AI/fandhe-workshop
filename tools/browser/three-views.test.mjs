// 3 つの表示（右のログ・左の成果物・復習）が、同じイベント列から導かれて一致することの試験（TASK-347 2/2。FEND-2）。
//
// 試験の設定（TEST_HOOKS=1）の画面には、試験データの教材と開始直後の状態が入っている。見えない `raw-trigger` ボタンの
// `data-payload` に `Op` の JSON を書いて click し、契約の試料（testdata/contract/event-sequence.json）の操作列を 1 件ずつ流す。
// 操作のたびに、DOM の 3 部品が、ログの全体から一括で導いた値（`__fandheTest.stateJson().engine`）と一致することを確かめる。
// 一致の定義と範囲は、docs の該当節ではなく、このファイルの各 assert が持つ。spec の 52 項目の全場面は TASK-220 の担当で、
// ここでは主要な項目（3 表示の一致）に絞る。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定。試験の口が無いことの確認）。どちらも未設定・不在は
// skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero, summarize,
} from './harness.mjs';

const BASE = '/fandhe-three-views/';
const RAW = '[data-testid="raw-trigger"]';
const DRAFT = '[data-testid="draft-input"]';
const SAMPLE = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../testdata/contract/event-sequence.json');
const PARTS = ['part-log', 'part-artifact-view', 'part-review'];

let browser;
let ops;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  const sample = JSON.parse(fs.readFileSync(SAMPLE, 'utf8'));
  ops = sample.entries.map((e) => e.op);
  assert.ok(ops.length > 0, 'sample has no ops');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// `data-payload` を書き換えて click する。input 経由の click はフォーカスを動かすため、要素の click() を直接呼ぶ。
async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

// 状態の JSON と DOM を 1 回の evaluate で同時に読む（固定の関数だけを渡す）。
// 実タイマーの進行（TASK-349）は、別々の evaluate の間に届いて状態と DOM をずらすことがある。
// 同じタスクの中で両方を読めば、その間に進行は届かない。
async function readBoth(page) {
  return page.evaluate(() => {
    const engine = JSON.parse(window.__fandheTest.stateJson()).engine;
    const q = (id) => document.querySelector(`[data-testid="${id}"]`);
    const rows = (id) => [...q(id).querySelectorAll('li')];
    const text = (id) => q(id).textContent;
    const dom = {
      log: rows('part-log').map((li) => ({
        event: Number(li.dataset.event),
        generation: Number(li.dataset.gen),
        step: li.dataset.step,
        kind: li.dataset.kind,
      })),
      artifacts: rows('part-artifact-view').map((li) => ({ path: li.dataset.path, mark: li.getAttribute('data-mark') })),
      review: rows('part-review').map((li) => ({
        step: li.dataset.step,
        achieved: li.dataset.achieved === 'true',
        skipped: li.dataset.skipped === 'true',
        changeCount: Number(li.dataset.changeCount),
        paths: li.textContent === '' ? [] : li.textContent.split(', '),
      })),
      consumed: [text('part-log-consumed'), text('part-artifact-consumed'), text('part-review-consumed')],
      html: ['part-log', 'part-artifact-view', 'part-review'].map((id) => q(id).outerHTML),
    };
    return { engine, dom };
  });
}

const engineState = async (page) => (await readBoth(page)).engine;
const readDom = async (page) => (await readBoth(page)).dom;

// 進行が止まるまで待つ。条件は、次の進行の予約が無く、ログの長さが進行の間隔の 2 倍の間変わらないこと。
async function waitQuiet(page) {
  const interval = await page.evaluate(() => window.__fandheTest.tickIntervalMs());
  const deadline = Date.now() + 10000;
  let lastLen = -1;
  let since = Date.now();
  while (Date.now() < deadline) {
    const { len, next } = await page.evaluate(() => ({
      len: JSON.parse(window.__fandheTest.stateJson()).engine.logLen,
      next: window.__fandheTest.nextTick(),
    }));
    if (len !== lastLen || next !== 'null') {
      lastLen = len;
      since = Date.now();
    } else if (Date.now() - since >= interval * 2) {
      return;
    }
    await page.waitForTimeout(50);
  }
  throw new Error('ticks did not settle');
}

function assertMatches(dom, engine, detail) {
  assert.equal(engine.irLoaded, true, detail);
  // (a) ログの行は、一括で導いた行と一致する。
  // 行にしないイベント（line が null）は DOM に行が無い。kind は行の種類（data-kind。TASK-431）で照合する。
  const expectedLog = engine.log
    .filter((r) => r.line !== null)
    .map(({ event, generation, step, line }) => ({ event, generation, step, kind: line }));
  assert.deepEqual(dom.log, expectedLog, `log ${detail}`);
  // (c) 成果物は、一括で導いた世界状態と印と一致する。
  assert.deepEqual(dom.artifacts, engine.artifacts, `artifacts ${detail}`);
  // (b) 復習は、一括で導いた復習と一致する。
  assert.deepEqual(
    dom.review,
    engine.review.map(({ step, achieved, skipped, changeCount, paths }) => ({ step, achieved, skipped, changeCount, paths })),
    `review ${detail}`,
  );
  // (d) 3 部品の consumed は、どれもログの長さと等しい。
  assert.deepEqual(dom.consumed, [String(engine.logLen), String(engine.logLen), String(engine.logLen)], `consumed ${detail}`);
  // 増分の導出と一括の導出が一致し、世界状態が core の状態と一致する。
  assert.equal(engine.incrementalMatches, true, detail);
  assert.equal(engine.worldMatchesState, true, detail);
  // (e) 復習のうち現在の世界に残る経路は、成果物にも出ている。
  const shown = new Set(dom.artifacts.map((a) => a.path));
  for (const item of engine.review) {
    for (const p of item.keptInWorld) assert.ok(shown.has(p), `kept path ${p} missing ${detail}`);
  }
}

test('操作列を流すたびに、3 つの表示が同じイベント列から導いた値と一致する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const first = await engineState(page);
    assert.equal(first.irLoaded, true, '試験用の教材が読み込まれていない');
    assertMatches(await readDom(page), first, 'initial');
    let lastLen = first.logLen;
    let grew = 0;
    for (let i = 0; i < ops.length; i += 1) {
      await sendRaw(page, ops[i]);
      const { engine, dom } = await readBoth(page);
      assertMatches(dom, engine, `after op #${i} ${JSON.stringify(ops[i])}`);
      if (engine.logLen > lastLen) grew += 1;
      lastLen = engine.logLen;
    }
    assert.ok(grew > 0, '操作列でログが 1 件も増えていない');
    const last = await engineState(page);
    assert.ok(last.log.length > 0 && last.artifacts.length >= 0);
    const kinds = new Set(last.log.map((r) => r.kind));
    assert.ok(kinds.has('input_received'), JSON.stringify([...kinds]));

    // (f) 丸ごと作り直しても、3 部品の DOM は同じ（差分更新が作り直しと一致する）。
    await waitQuiet(page);
    const before = (await readDom(page)).html;
    assert.equal(await page.evaluate(() => window.__fandheTest.rerender()), true);
    const after = (await readDom(page)).html;
    assert.deepEqual(after, before);
    const settled = await readBoth(page);
    assertMatches(settled.dom, settled.engine, 'after rerender');

    // (g) CSP 違反・ページ内エラー・コンソールのエラーが 0 件。
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
    const s = summarize(rec);
    assert.equal(s.csp + s.pageErrors + s.consoleErrors, 0, JSON.stringify(rec));
    await context.close();
  });
});

test('壊れた payload は何も起こさず、状態も DOM も変わらない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const before = await readDom(page);
    for (const bad of ['', '{', '{"control":"nope"}', '[]']) {
      await page.evaluate(([sel, json]) => {
        const el = document.querySelector(sel);
        el.setAttribute('data-payload', json);
        el.click();
      }, [RAW, bad]);
    }
    assert.deepEqual(await readDom(page), before);
    assert.equal((await engineState(page)).logLen, 0);
    assert.deepEqual(rec.pageErrors, []);
    await context.close();
  });
});

test('raw-trigger を押しても、入力欄の途中の値とフォーカスは保たれる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    await page.fill(DRAFT, '途中まで入力');
    await page.focus(DRAFT);
    await sendRaw(page, ops[0]);
    assert.ok((await engineState(page)).logLen > 0, '操作が届いていない');
    assert.equal(await page.inputValue(DRAFT), '途中まで入力');
    assert.equal(await page.evaluate((sel) => document.activeElement === document.querySelector(sel), DRAFT), true);
    await context.close();
  });
});

test('配信設定には raw-trigger が無く、3 部品は空の表示で描かれる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.locator(RAW).count(), 0);
    for (const id of PARTS) {
      assert.equal(await page.locator(`[data-testid="${id}"]`).count(), 1, id);
      assert.equal(await page.locator(`[data-testid="${id}"] li`).count(), 0, id);
    }
    await context.close();
  });
});
