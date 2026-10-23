// Step の題・「いまやること」・本文の表示の試験（SHELL-7。TASK-525 1/2）。CSP あり・なしの両方で流す。
//
// (1) 試験用の教材 `ch0-3`（`?fixture=ch0-3-chapters`）の 20 Step を通し、各 Step の DOM の題（step-title）と do（step-do）が
//     教材の IR（crates/web/tests/data/ch0_3/ir.json。試験の中で読む）と一致すること、章の最初の Step でも出ること、
//     Step を進めても親（step-text）が同じ要素のまま残ること（要素の同一性。FEND-3）を確かめる。
// (2) 最小教材 min-3step-pack を一時ディレクトリへ写して st-1.md を構造のある Markdown に置き換え、実物の `fandhe-pack compile`
//     で IR 版 2 を作って `loadIr` で入れ、見出し・一覧・表が構造を保って出ること、外部の画像が img にならず代替の文字になること、
//     実行・通信・CSP 違反が全区分 0 であること、幅 360px で横スクロールが無いことを確かめる。
// 対照: step-body を display: none にすると判定が不合格になる（空振りの防止）。
// 案内レベル B・C の確かめは native の試験（crates/web/tests/step_body_host.rs）が持つ（開始値の決定は TASK-294）。
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
import { ledger, mouseDriver, journey } from './ch0-3-journey.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const BASE = '/fandhe-step-body/';
const QUERY = '?fixture=ch0-3-chapters';
const PACK = path.resolve(HERE, '../../testdata/engine-event-consistency/test-data/min-3step-pack');
const IR = JSON.parse(fs.readFileSync(path.join(HERE, '../../crates/web/tests/data/ch0_3/ir.json'), 'utf8'));
const MARKDOWN = [
  '# 大見出し',
  '',
  '## 小見出し',
  '',
  '- 項目 1',
  '- 項目 2',
  '',
  '1. 手順 1',
  '2. 手順 2',
  '',
  '| 列 A | 列 B |',
  '| --- | --- |',
  '| a1 | b1 |',
  '| a2 | b2 |',
  '',
  '![外部の画像](https://example.invalid/x.png)',
  '',
].join('\n');

let browser;
let irText;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  const bin = packBinFromEnv();
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-step-body-'));
  try {
    fs.cpSync(PACK, work, { recursive: true });
    fs.writeFileSync(path.join(work, 'chapters', 'ch-min', 'st-1.md'), MARKDOWN, 'utf8');
    const r = spawnSync(bin, ['compile', work], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    assert.equal(r.status, 0, `compile exit ${r.status}`);
    irText = r.stdout;
    assert.equal(JSON.parse(irText).format_version, 2);
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const visible = (sel) => `(() => { const e = document.querySelector('${sel}'); if (!e) return false; const r = e.getBoundingClientRect(); const c = getComputedStyle(e); return r.width > 0 && r.height > 0 && c.display !== 'none' && c.visibility !== 'hidden'; })()`;

async function stepTexts(page) {
  return page.evaluate(() => ({
    titles: [...document.querySelectorAll('[data-testid="step-title"]')].map((e) => e.textContent),
    dos: [...document.querySelectorAll('[data-testid="step-do"]')].map((e) => e.textContent),
    parents: document.querySelectorAll('[data-testid="step-text"]').length,
  }));
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`20 Step の題と do が IR のとおり出て、親の要素が同じまま残る（${mode}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
      const L = ledger();
      await page.goto(`${srv.url}${QUERY}`);
      await waitMounted(page);
      const parent = await page.evaluateHandle(() => document.querySelector('[data-testid="step-text"]'));
      let seen = 0;
      const onStep = async (uid, status) => {
        if (status !== 'entered') return;
        const cur = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.current);
        const node = IR.nodes.find((n) => n.uid === cur);
        const t = await stepTexts(page);
        seen += 1;
        L.check(uid, '親が 1 個', t.parents === 1);
        L.check(uid, '題が IR のとおり', t.titles.length === 1 && t.titles[0] === node.step.title);
        L.check(uid, 'do が IR のとおり', t.dos.length === 1 && t.dos[0] === `いまやること${node.step.do}`);
        L.check(uid, '題が見える', await page.evaluate(visible('[data-testid="step-title"]')));
      };
      await journey(page, mouseDriver(page), '通し', L, null, { chaptered: true, onStep });
      L.check('通し', '20 Step を訪れた', seen === 20);
      L.check('通し', '親の要素が同じまま', await parent.evaluate((e) => e.isConnected && e === document.querySelector('[data-testid="step-text"]')));
      L.check('共通', 'ページ内のエラーが 0 件', summarize(rec).pageErrors === 0);
      assert.deepEqual(L.failed(), []);
      await context.close();
    }));
  });

  test(`最小教材の本文が見出し・一覧・表の構造を保って出て、実行・通信が 0 件（${mode}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      for (const width of [1280, 360]) {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width, height: 800 } });
        try {
          await page.goto(srv.url);
          await waitMounted(page);
          const n0 = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.installCount);
          assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), irText), true, 'load-ir sent');
          await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
          await page.waitForSelector('[data-testid="step-body"]', { timeout: 10000 });
          if (width === 360) await page.locator('[role="tab"]').first().click();
          const c = await page.evaluate(() => {
            const b = document.querySelector('[data-testid="step-body"]');
            const n = (s) => b.querySelectorAll(s).length;
            return {
              h1: n('h1'), h2: n('h2'), ul: n('ul'), ol: n('ol'), li: n('li'), th: n('th'), td: n('td'), img: n('img'),
              level: b.getAttribute('data-body-level'),
              hasAlt: b.textContent.includes('外部の画像'),
              overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
            };
          });
          assert.deepEqual(
            { ...c, overflow: undefined },
            { h1: 1, h2: 1, ul: 1, ol: 1, li: 4, th: 2, td: 4, img: 0, level: 'A', hasAlt: true, overflow: undefined },
            `width ${width}`,
          );
          assert.equal(c.overflow, false, `width ${width}: horizontal scroll`);
          assert.equal(await page.evaluate(visible('[data-testid="step-body"]')), true, `width ${width}: visible`);
          const counts = await collectSafety(page, rec);
          assert.deepEqual(nonZero(counts), [], JSON.stringify({ width, csp, counts }));

          // 対照: 本文を hidden にすると「見える」の判定は不合格になる（style 属性を使わないので CSP ありでも流せる）。
          await page.evaluate(() => { document.querySelector('[data-testid="step-body"]').hidden = true; });
          assert.equal(await page.evaluate(visible('[data-testid="step-body"]')), false, 'control: hidden body must fail the visibility check');
        } finally {
          await context.close();
        }
      }
    }));
  });
}
