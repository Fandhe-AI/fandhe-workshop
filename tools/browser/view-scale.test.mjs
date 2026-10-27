// 再描画の速さの測定（TASK-485 1/3。ART-4）が使う規模のデータと測定の道具が壊れていないことの試験。
//
// 測定そのもの（tools/measure/view-redraw.sh）は遅く CI に入れないため、ここでは測定の前提だけを CI で固定する。
// - VS1: 規模のデータの写し（crates/web/tests/data/artifact_view_scale/）が generate.mjs の出力と一致する（JSON は構造、Markdown はバイト）
// - VS2: 代役の部品に 6 つの規模の payload を渡すと受理され、12 組の前・後の項目数が Node 側で数えた期待値と一致する
// - VS3: 測定のページ内の関数 viewRedrawInPage を小さな引数で流すと、24 組の値が正の有限数で返り、各操作の検査（項目数・隠れた一覧の空）が通る
// - VS4: 規模の差し替え payload のうち未知の値は拒否され、外部へのリクエスト・ページのエラー・ダイアログが 0 件
// 限界: 時間の値そのものは検査しない（遅い環境で不安定になるため）。測定の値は docs/testing.md に記録する。
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { cases, expectedCounts, viewRedrawInPage, DATA_PAYLOADS } from '../measure/view-redraw.mjs';

const BASE = '/fandhe-view-scale/';
const ROOT_ID = 'view-fixture-root';
const SCALE_DIR = fileURLToPath(new URL('../../crates/web/tests/data/artifact_view_scale/', import.meta.url));
const GENERATE = fileURLToPath(new URL('../../testdata/artifact-view-parts/test-data/generate.mjs', import.meta.url));
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

async function withFixturePage(fn) {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal(await page.evaluate(() => typeof window.__fandheTest?.viewFixtureMount), 'function', 'test hooks are missing');
      await page.evaluate((id) => {
        const box = document.createElement('div');
        box.id = id;
        document.body.append(box);
      }, ROOT_ID);
      const res = JSON.parse(await page.evaluate((id) => window.__fandheTest.viewFixtureMount(id), ROOT_ID));
      assert.deepEqual(res, { ok: true });
      await fn(page, rec);
    } finally {
      await context.close();
    }
  });
}

async function dispatch(page, name, payload) {
  return JSON.parse(await page.evaluate(([n, p]) => window.__fandheTest.viewFixtureDispatch(n, p), [name, payload])).outcome;
}

test('VS1: 規模のデータの写しは generate.mjs の出力と一致する', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'view-scale-'));
  try {
    execFileSync(process.execPath, [GENERATE, dir], { stdio: 'ignore' });
    for (const f of ['diff-500.json', 'tasks-50.json', 'filetree-200.json']) {
      const generated = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8'));
      const copy = JSON.parse(fs.readFileSync(path.join(SCALE_DIR, f), 'utf8'));
      assert.deepEqual(copy, generated, f);
    }
    for (const f of ['doc-large-before.md', 'doc-large-after.md']) {
      assert.ok(fs.readFileSync(path.join(dir, f)).equals(fs.readFileSync(path.join(SCALE_DIR, f))), f);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('VS2: 規模の payload を受理し、12 組の前・後の項目数が期待値と一致する', async () => {
  const counts = expectedCounts();
  await withFixturePage(async (page) => {
    for (const [, payload] of DATA_PAYLOADS) assert.equal(await dispatch(page, 'fixture_data', payload), 'applied', payload);
    const seen = new Set();
    for (const c of cases()) {
      const key = `${c.kind}:${c.variant}`;
      if (seen.has(key)) continue;
      seen.add(key);
      assert.equal(await dispatch(page, 'fixture_select', key), 'applied', key);
      for (const phase of ['before', 'after', 'before']) {
        assert.equal(await dispatch(page, 'fixture_phase', phase), 'applied');
        const n = await page.evaluate(
          ([id, kind]) => {
            const lists = [...document.getElementById(id).querySelectorAll('[data-testid="view-list"]')];
            const list = lists.find((l) => l.getAttribute('data-view') === kind);
            return list.querySelectorAll(':scope > li').length;
          },
          [ROOT_ID, c.kind],
        );
        assert.equal(n, counts[c.kind][phase], `${key} ${phase}`);
      }
    }
    assert.equal(seen.size, 12);
  });
});

test('VS3: 測定のページ内の関数が 24 組で値を返し、各操作の検査が通る', async () => {
  const list = cases();
  await withFixturePage(async (page, rec) => {
    const res = await page.evaluate(viewRedrawInPage, {
      cases: list, counts: expectedCounts(), payloads: DATA_PAYLOADS, samples: 2, gapMs: 0, rootId: ROOT_ID, rotate: 5,
    });
    assert.equal(Object.keys(res).length, 24);
    for (const c of list) {
      const v = res[`${c.kind}:${c.variant}:${c.op}`];
      assert.equal(v.length, 2);
      assert.ok(v.every((x) => Number.isFinite(x) && x >= 0), JSON.stringify(v));
    }
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});

test('VS4: 未知の規模の payload は拒否され、外部への通信・エラー・ダイアログが 0 件', async () => {
  await withFixturePage(async (page, rec) => {
    for (const p of ['diff:501', 'document:a', 'tasks:500', 'filetree:2000']) {
      assert.equal(await dispatch(page, 'fixture_data', p), 'rejected', p);
    }
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  });
});
