// 本体の画面の再描画の測定（tools/measure/product-redraw.mjs。TASK-485 2/3。ART-26）のブラウザ試験。
//
// 測定の固定データ（?fixture=artifact-redraw-<n>）が製品の画面に件数どおりの指せる行を出すこと、ページ内の測定の関数が 7 組すべてで
// 同期性の検査を通って値を返すこと、未知の件数は既定の教材に倒れること、CSP 違反・ページ内エラー・外部への通信が 0 件であることを確かめる。
// 値の大きさ（16ms との比較）は assert せず、測定の結果を docs/testing.md に記録する。
// 前提: FANDHE_WEB_TEST_DIR に TEST_HOOKS=1 でビルドした一式（scripts/test-browser.sh が渡す）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { cases, productRedrawInPage } from '../measure/product-redraw.mjs';

const BASE = '/fandhe-product-redraw/';
const SIZES = [9, 20, 50, 200, 500];
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

async function withFixture(query, fn) {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(`${srv.url}${query}`);
      await waitMounted(page);
      await fn(page);
      assert.deepEqual(nonZero(await collectSafety(page, rec)), []);
    } finally {
      await context.close();
    }
  });
}

// 既定の選択の list の行数と、指せる行の属性を持つ行数。
const shownRows = (page) => page.evaluate(() => {
  const l = document.querySelector('#app [data-testid="view-list"]:not([hidden])');
  const items = [...l.querySelectorAll(':scope > li')];
  const pointed = items.filter(
    (li) => li.hasAttribute('data-action') && li.hasAttribute('data-payload') && li.hasAttribute('data-row') && li.hasAttribute('tabindex'),
  ).length;
  return { view: l.getAttribute('data-view'), rows: items.length, pointed };
});

for (const n of SIZES) {
  test(`PR1: artifact-redraw-${n} は既定の選択の list が ${n} 行で全行が指せる行`, async () => {
    await withFixture(`?fixture=artifact-redraw-${n}`, async (page) => {
      const r = await shownRows(page);
      assert.equal(r.view, 'file_tree');
      assert.equal(r.rows, n);
      assert.equal(r.pointed, n);
    });
  });
}

test('PR2: ページ内の測定の関数が 7 組すべてで例外なく正の値を返す', async () => {
  for (const c of cases()) {
    await withFixture(`?fixture=artifact-redraw-${c.rows}`, async (page) => {
      const v = await page.evaluate(productRedrawInPage, {
        kind: c.kind, from: c.from, rows: c.rows, samples: 2, gapMs: 5,
      });
      assert.equal(v.length, 2);
      for (const x of v) assert.ok(Number.isFinite(x) && x >= 0, `${c.kind}: ${x}`);
    });
  }
});

test('PR3: 未知の件数は既定の教材に倒れ、測定の関数は行数の不一致で失敗する（負の対照）', async () => {
  await withFixture('?fixture=artifact-redraw-100', async (page) => {
    const buttons = await page.evaluate(() => document.querySelectorAll('#app [data-testid="view-select-button"]').length);
    // 既定の教材は 7 種のボタンを出すが、積んだ行は 0 件で、100 行にはならない。
    const rows = await page.evaluate(() => {
      const l = document.querySelector('#app [data-testid="view-list"]:not([hidden])');
      return l ? l.querySelectorAll(':scope > li').length : 0;
    });
    assert.notEqual(rows, 100);
    if (buttons > 0) {
      await assert.rejects(
        page.evaluate(productRedrawInPage, { kind: 'diff', from: 'document', rows: 100, samples: 1, gapMs: 5 }),
      );
    }
  });
});
