// `pack.left_pane`（`world_list`・`view_tabs`）が左のビューに反映されることの試験（PACK-22。TASK-481 1/2）。CSP あり・なしの両方で流す。
//
// 試験用の教材は crates/web/tests/data/doc_body_limits_pack（Step が v-doc・v-tree を宣言し、最初の承認で 3 行が出る）を
// 一時ディレクトリへ写し、`pack.left_pane` を足して実物の `fandhe-pack compile` で IR にする。
//   B1 world_list 2 値 × view_tabs 3 値の全 6 通りで、ボタンの種類・選択の親の hidden・7 本の list の項目数が設定どおりになる
//      （`all` は 7 種、`declared` は宣言の 2 種、`hidden` は 0 件で親が hidden。`world_list: hidden` は項目 0 件）。
//      `view_tabs: hidden` の教材は Step の宣言を 1 つにする（E-LEFT-MULTI-VIEW）
//   B2 `declared`・`hidden` で、宣言外の種類を選ぶ操作をしても、見えている list と選択の属性が変わらない（対照: `all` では変わる）
//   B3 `all` で 7 種のボタンを押すと、押した list だけが見える
//   B4 Step を移ると宣言の先頭のビューに戻る
//   B5 CSP 違反・ページ内エラー・配信元以外への通信が 0 件
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_PACK_BIN（前提バイナリ）。未設定・不在は失敗にする。
// 限界: `world_list: hidden` は種類ごとの描画部品が結線されるまで、指せる行と文書の本文も消す（docs/known-gaps.md の PACK-22）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, summarize,
} from './harness.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const BASE = '/fandhe-left-pane/';
const PACK = path.resolve(HERE, '../../crates/web/tests/data/doc_body_limits_pack');
const APPROVE = '[data-testid="approval-btn"][data-decision="approve"]';
const BTN = '[data-testid="view-select-button"]';
const SELECT = '[data-testid="view-select"]';
const LISTS = '#app [data-testid="view-list"]';
const ALL_KINDS = ['file_tree', 'document', 'diff', 'task_tree', 'card_list', 'check_list', 'app_demo'];
const PACK_LINE = "vocab: '1.4'}";

let browser;
const irs = new Map();

function compile(bin, leftPane, singleView) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-left-pane-'));
  try {
    fs.cpSync(PACK, work, { recursive: true });
    const y = path.join(work, 'workshop.yaml');
    const text = fs.readFileSync(y, 'utf8');
    const next = text.replace(PACK_LINE, `vocab: '1.4', left_pane: ${leftPane}}`);
    assert.notEqual(next, text, 'mutation applied');
    fs.writeFileSync(y, next, 'utf8');
    if (singleView) {
      // `view_tabs: hidden` は Step が宣言するビューを 1 つに限る（E-LEFT-MULTI-VIEW）。
      const c = path.join(work, 'chapters/c1/chapter.yaml');
      const ct = fs.readFileSync(c, 'utf8');
      const cn = ct.replaceAll('views: [v-doc, v-tree]', 'views: [v-doc]');
      assert.notEqual(cn, ct, 'single view applied');
      fs.writeFileSync(c, cn, 'utf8');
    }
    const r = spawnSync(bin, ['compile', work], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    assert.equal(r.status, 0, `compile exit ${r.status}`);
    return r.stdout;
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  const bin = packBinFromEnv();
  for (const world of ['shown', 'hidden']) {
    for (const tabs of ['all', 'declared', 'hidden']) {
      irs.set(`${world}/${tabs}`, compile(bin, `{world_list: ${world}, view_tabs: ${tabs}}`, tabs === 'hidden'));
    }
  }
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function load(page, irText) {
  const n0 = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.installCount);
  assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), irText), true, 'load-ir sent');
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
}

// 画面を開いて教材を差し替える。承認の前の状態から始める。
async function openWith(page, srv, key) {
  await page.goto(srv.url);
  await waitMounted(page);
  await load(page, irs.get(key));
  await page.waitForSelector(APPROVE, { timeout: 10000 });
}

const payloads = (page) => page.evaluate((b) => [...document.querySelectorAll(`#app ${b}`)].map((x) => x.dataset.payload), BTN);
const parentHidden = (page) => page.evaluate((s) => document.querySelector(`#app ${s}`).hasAttribute('hidden'), SELECT);
const itemCounts = (page) => page.evaluate((l) => [...document.querySelectorAll(l)].map((x) => x.querySelectorAll('li').length), LISTS);
const visible = (page) => page.evaluate((l) => [...document.querySelectorAll(l)].filter((x) => !x.hasAttribute('hidden')).map((x) => x.dataset.view), LISTS);
const selectionSnapshot = (page) => page.evaluate((l) => JSON.stringify({
  lists: [...document.querySelectorAll(l)].map((x) => [x.dataset.view, x.hasAttribute('hidden')]),
  selected: [...document.querySelectorAll('#app [data-testid="view-select-button"]')].map((b) => [b.dataset.payload, b.dataset.selected]),
}), LISTS);

// 製品の画面には宣言外を選ぶ口が無い。試験のページの中で、画面の根の中にボタンを足してクリックする（配信物には残らない）。
async function forceSelect(page, kind) {
  await page.evaluate((k) => {
    const root = document.querySelector('#app [data-testid="workshop-root"]');
    const b = document.createElement('button');
    b.type = 'button';
    b.setAttribute('data-action', 'select-view');
    b.setAttribute('data-payload', k);
    b.setAttribute('data-testid', 'forced-select');
    root.appendChild(b);
    b.click();
    b.remove();
  }, kind);
  // 反映の時間を置く。変化が無いことを確かめるため、1 フレーム分は待つ。
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
}

async function safe(page, rec, csp) {
  const counts = await collectSafety(page, rec);
  assert.deepEqual(nonZero(counts), [], JSON.stringify({ csp, counts }));
  assert.equal(summarize(rec).pageErrors, 0);
}

async function withPage(csp, fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    try {
      await fn(page, srv, rec);
    } finally {
      await context.close();
    }
  }));
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`B1 6 通りでボタン・選択の親・行の件数が設定どおりになる（${mode}）`, { timeout: 240000 }, async () => {
    for (const world of ['shown', 'hidden']) {
      for (const tabs of ['all', 'declared', 'hidden']) {
        await withPage(csp, async (page, srv, rec) => {
          await openWith(page, srv, `${world}/${tabs}`);
          const cur0 = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.current);
          await page.click(APPROVE);
          if (world === 'shown') await page.waitForSelector('#app li[data-path="notes/md"]', { timeout: 10000 });
          else await page.waitForFunction((c) => JSON.parse(window.__fandheTest.stateJson()).engine.current !== c, cur0, { timeout: 10000 });
          const want = { all: ALL_KINDS, declared: ['document', 'file_tree'], hidden: [] }[tabs];
          assert.deepEqual(await payloads(page), want, `${world}/${tabs}: ボタン`);
          assert.equal(await parentHidden(page), tabs === 'hidden', `${world}/${tabs}: 選択の親の hidden`);
          const counts = await itemCounts(page);
          assert.equal(counts.length, 7);
          const total = counts.reduce((a, b) => a + b, 0);
          if (world === 'shown') assert.ok(total >= 3, `${world}/${tabs}: 行が出る`);
          else assert.equal(total, 0, `${world}/${tabs}: 行は 0 件`);
          // 先頭は宣言の先頭（document）が見える。`all` でも宣言があるので先頭は同じ。
          assert.deepEqual(await visible(page), ['document'], `${world}/${tabs}: 見える list`);
          await safe(page, rec, csp);
        });
      }
    }
  });

  test(`B2 declared・hidden では宣言外を選んでも変わらない（対照: all では変わる）（${mode}）`, { timeout: 180000 }, async () => {
    for (const tabs of ['declared', 'hidden']) {
      await withPage(csp, async (page, srv, rec) => {
        await openWith(page, srv, `shown/${tabs}`);
        const before = await selectionSnapshot(page);
        await forceSelect(page, 'task_tree');
        assert.equal(await selectionSnapshot(page), before, `${tabs}: 宣言外は無視される`);
        // 宣言内の種類は、declared では選べ、hidden（宣言は 1 つだけ）では先頭のままである。
        await forceSelect(page, 'file_tree');
        if (tabs === 'hidden') assert.equal(await selectionSnapshot(page), before, 'hidden: 宣言外の file_tree も無視される');
        else assert.deepEqual(await visible(page), ['file_tree'], 'declared: 宣言内は選べる');
        await safe(page, rec, csp);
      });
    }
    await withPage(csp, async (page, srv) => {
      await openWith(page, srv, 'shown/all');
      await forceSelect(page, 'task_tree');
      assert.deepEqual(await visible(page), ['task_tree'], '対照: all では宣言外も選べる');
    });
  });

  test(`B3 all では 7 種のボタンを押した list だけが見える（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page, srv, rec) => {
      await openWith(page, srv, 'shown/all');
      for (const kind of [...ALL_KINDS, ALL_KINDS[0]]) {
        await page.locator(`#app ${BTN}[data-payload="${kind}"]`).click();
        await page.waitForFunction(([b, k]) => document.querySelector(`#app ${b}[data-payload="${k}"]`).dataset.selected === 'true', [BTN, kind]);
        assert.deepEqual(await visible(page), [kind]);
      }
      await safe(page, rec, csp);
    });
  });

  test(`B4 Step を移ると宣言の先頭のビューに戻る（${mode}）`, { timeout: 120000 }, async () => {
    for (const tabs of ['all', 'declared']) {
      await withPage(csp, async (page, srv, rec) => {
        await openWith(page, srv, `shown/${tabs}`);
        await page.locator(`#app ${BTN}[data-payload="file_tree"]`).click();
        await page.waitForFunction((l) => [...document.querySelectorAll(l)].filter((x) => !x.hasAttribute('hidden')).map((x) => x.dataset.view).join() === 'file_tree', LISTS);
        await page.click(APPROVE);
        await page.waitForSelector('#app li[data-path="notes/md"]', { timeout: 10000 });
        assert.deepEqual(await visible(page), ['document'], `${tabs}: 次の Step の先頭に戻る`);
        await safe(page, rec, csp);
      });
    }
  });
}
