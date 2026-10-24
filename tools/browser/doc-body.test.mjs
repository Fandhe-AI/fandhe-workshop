// 文書ビューの練習データの本文の表示の試験（ART-18。TASK-489 1/2。D-50・D-51）。CSP あり・なしの両方で流す。
//
// 試験用の教材 crates/web/tests/data/doc_body_pack（`views.v-doc.body: shown`）を一時ディレクトリへ写して実物の
// `fandhe-pack compile` で IR 版 2 にし、`loadIr` で入れる。承認の前は本文が無く、承認で練習データ doc-a の本文が行に出て、
// 次の承認の replace で同じ行（同じ li の要素）の本文が doc-b に替わること、見出しが構造で出て目に見えること、
// 実行・通信・CSP 違反・ページ内エラーが全区分 0 であることを確かめる。
// 対照: `body` を外した写しでは本文の区画が 0 件。区画を hidden にすると見えることの判定が不合格になる（空振りの防止）。
// 書き込むパスは一時ディレクトリの固定のパスだけで、教材の値からパスを作らない。失敗の文面に本文の中身は出さない。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_PACK_BIN（前提バイナリ）。未設定・不在は skip にせず失敗にする。
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
const BASE = '/fandhe-doc-body/';
const PACK = path.resolve(HERE, '../../crates/web/tests/data/doc_body_pack');
const BODY = '[data-testid="artifact-doc-body"]';
const ROW = 'li[data-path="notes/draft"]';
const APPROVE = '[data-testid="approval-btn"][data-decision="approve"]';

let browser;
let irShown;
let irNone;

function compile(bin, mutate) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-doc-body-'));
  try {
    fs.cpSync(PACK, work, { recursive: true });
    if (mutate) {
      const y = path.join(work, 'workshop.yaml');
      fs.writeFileSync(y, mutate(fs.readFileSync(y, 'utf8')), 'utf8');
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
  irShown = compile(bin);
  assert.equal(JSON.parse(irShown).views['v-doc'].body, 'shown');
  irNone = compile(bin, (t) => t.replace('{kind: document, body: shown}', '{kind: document}'));
  assert.equal(JSON.parse(irNone).views['v-doc'].body, undefined);
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const visible = (sel) => `(() => { const e = document.querySelector('${sel}'); if (!e) return false; const r = e.getBoundingClientRect(); const c = getComputedStyle(e); return r.width > 0 && r.height > 0 && c.display !== 'none' && c.visibility !== 'hidden'; })()`;

async function load(page, irText) {
  const n0 = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.installCount);
  assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), irText), true, 'load-ir sent');
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
}

async function approve(page) {
  await page.waitForSelector(APPROVE, { timeout: 10000 });
  await page.click(APPROVE);
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`承認で本文が行に出て、次の承認で同じ行のまま本文が替わる（${mode}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
      try {
        await page.goto(srv.url);
        await waitMounted(page);
        await load(page, irShown);
        assert.equal(await page.locator(`#app ${BODY}`).count(), 0, '承認の前は本文が無い');

        await approve(page);
        await page.waitForSelector(`${ROW} ${BODY} h1`, { timeout: 10000 });
        const row = await page.evaluateHandle((s) => document.querySelector(s), ROW);
        const first = await page.locator(`${ROW} ${BODY} h1`).textContent();
        assert.equal(await page.locator(BODY).count(), 1);
        assert.equal(await page.evaluate(visible(`${ROW} ${BODY} h1`)), true, '見出しが見える');

        await approve(page);
        await page.waitForFunction(([s, f]) => {
          const h = document.querySelector(`${s} [data-testid="artifact-doc-body"] h1`);
          return h && h.textContent !== f;
        }, [ROW, first], { timeout: 10000 });
        assert.equal(await page.locator(BODY).count(), 1);
        assert.equal(await row.evaluate((e, s) => e.isConnected && e === document.querySelector(s), ROW), true, '同じ li のまま');

        const counts = await collectSafety(page, rec);
        assert.deepEqual(nonZero(counts), [], JSON.stringify({ csp, counts }));
        assert.equal(summarize(rec).pageErrors, 0);

        // 対照: 区画を hidden にすると「見える」の判定は不合格になる。
        await page.evaluate((s) => { document.querySelector(s).hidden = true; }, BODY);
        assert.equal(await page.evaluate(visible(BODY)), false, 'control: hidden body must fail the visibility check');
      } finally {
        await context.close();
      }
    }));
  });

  test(`body を宣言しない教材では本文が出ない（対照。${mode}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
      try {
        await page.goto(srv.url);
        await waitMounted(page);
        await load(page, irNone);
        await approve(page);
        await page.waitForSelector(ROW, { timeout: 10000 });
        assert.equal(await page.locator(BODY).count(), 0);
      } finally {
        await context.close();
      }
    }));
  });
}
