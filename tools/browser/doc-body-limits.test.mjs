// 文書ビューの本文の表示の限界の観察の固定（ART-18。TASK-489 2/2。D-50・D-51）。CSP あり・なしの両方で流す。
//
// 目的: 載せる案（TASK-489 1/2。doc-body.test.mjs）の実装の後に残る限界を、現状の観察として固定する。限界の記録は
// docs/known-gaps.md の ART-18。ここで assert するのは「今こう見える」であり、望ましい姿ではない。仕様の「本文は IR に
// 載っていないため表示しません」は試作の文言で、製品には無い（理由の文は出さない）。その文言が 0 件であることも固定する。
// 観察（各項目に負の対照を付け、判定の空振りを防ぐ）:
//   L1 body を省略・hidden にした文書ビューでは、要素に fixture があっても本文の区画が 0 件で、経路の行は出る。理由の文は出ない
//   L2 body: shown でも、fixture を持たない要素の行には本文の区画が 0 件
//   L3 .md 以外（.txt）の練習データは解釈されず、区画の中は pre > code の 1 ブロックで、先頭の `# ` の行も h1 にならない
//   L4 文書以外のビューを選ぶと区画が 0 件になり、文書に戻すと戻る
//   L5 replace で本文が替わっても、区画の中にブロック単位の前後の印（del・ins・data-mark）が 0 件。替わった後の木だけを出す
// 試験用の教材は crates/web/tests/data/doc_body_limits_pack を一時ディレクトリへ写して実物の `fandhe-pack compile` で IR にする。
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
const BASE = '/fandhe-doc-body-limits/';
const PACK = path.resolve(HERE, '../../crates/web/tests/data/doc_body_limits_pack');
const BODY = '[data-testid="artifact-doc-body"]';
const ROW_MD = 'li[data-path="notes/md"]';
const ROW_TXT = 'li[data-path="notes/txt"]';
const ROW_BARE = 'li[data-path="notes/bare"]';
const APPROVE = '[data-testid="approval-btn"][data-decision="approve"]';
const SELECT = '[data-testid="view-select-button"]';
const PROTOTYPE_REASON = ['本文はこの教材の定義（IR）に載っていないため表示しません', '本文は IR に載っていない'];
const SHOWN = '{kind: document, body: shown}';

let browser;
let irShown;
let irOmitted;
let irHidden;

function compile(bin, mutate) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-doc-body-limits-'));
  try {
    fs.cpSync(PACK, work, { recursive: true });
    if (mutate) {
      const y = path.join(work, 'workshop.yaml');
      const before = fs.readFileSync(y, 'utf8');
      const after = mutate(before);
      assert.notEqual(after, before, 'mutation applied');
      fs.writeFileSync(y, after, 'utf8');
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
  irOmitted = compile(bin, (t) => t.replace(SHOWN, '{kind: document}'));
  assert.equal(JSON.parse(irOmitted).views['v-doc'].body, undefined);
  irHidden = compile(bin, (t) => t.replace(SHOWN, '{kind: document, body: hidden}'));
  assert.equal(JSON.parse(irHidden).views['v-doc'].body, 'hidden');
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

async function approve(page) {
  await page.waitForSelector(APPROVE, { timeout: 10000 });
  await page.click(APPROVE);
}

// 承認の前の状態は行が 0 件。1 回目の承認で 3 行が出るまで待つ。
async function openAndPut(page, srv, irText) {
  await page.goto(srv.url);
  await waitMounted(page);
  await load(page, irText);
  await approve(page);
  await page.waitForSelector(ROW_BARE, { timeout: 10000 });
  await page.waitForSelector(ROW_MD, { timeout: 10000 });
  await page.waitForSelector(ROW_TXT, { timeout: 10000 });
}

const appText = (page) => page.evaluate(() => document.querySelector('#app').textContent);

async function assertNoPrototypeReason(page) {
  const t = await appText(page);
  for (const s of PROTOTYPE_REASON) assert.equal(t.includes(s), false, '試作の理由の文は製品に無い');
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

  test(`L1 body を省略・hidden にすると本文の区画が 0 件で理由の文も出ない（${mode}）`, { timeout: 120000 }, async () => {
    for (const [name, ir] of [['省略', irOmitted], ['hidden', irHidden]]) {
      await withPage(csp, async (page, srv, rec) => {
        await openAndPut(page, srv, ir);
        assert.equal(await page.locator(BODY).count(), 0, `${name}: 区画は 0 件`);
        assert.equal(await page.locator(`${ROW_MD}`).count(), 1, `${name}: 経路の行は出る`);
        await assertNoPrototypeReason(page);
        await safe(page, rec, csp);
      });
    }
    // 負の対照: body: shown では同じ操作で区画が出る（判定が常に 0 件を返す空振りの防止）。
    await withPage(csp, async (page, srv) => {
      await openAndPut(page, srv, irShown);
      await page.waitForSelector(`${ROW_MD} ${BODY}`, { timeout: 10000 });
    });
  });

  test(`L2 fixture を持たない要素の行には本文の区画が 0 件（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page, srv, rec) => {
      await openAndPut(page, srv, irShown);
      await page.waitForSelector(`${ROW_MD} ${BODY}`, { timeout: 10000 });
      assert.equal(await page.locator(`${ROW_BARE} ${BODY}`).count(), 0, 'fixture の無い行は区画 0 件');
      assert.equal(await page.locator(`${ROW_MD} ${BODY}`).count(), 1, '対照: fixture のある行は 1 件');
      await assertNoPrototypeReason(page);
      await safe(page, rec, csp);
      // 負の対照: 区画を注入すると 0 件の判定は不合格になる。
      await page.evaluate((s) => {
        const d = document.createElement('div');
        d.setAttribute('data-testid', 'artifact-doc-body');
        document.querySelector(s).appendChild(d);
      }, ROW_BARE);
      assert.equal(await page.locator(`${ROW_BARE} ${BODY}`).count(), 1, 'control: injected body must be counted');
    });
  });

  test(`L3 .txt は解釈されず pre > code の 1 ブロックで出る（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page, srv, rec) => {
      await openAndPut(page, srv, irShown);
      await page.waitForSelector(`${ROW_TXT} ${BODY}`, { timeout: 10000 });
      const shape = await page.evaluate(([s, b]) => {
        const body = document.querySelector(`${s} ${b}`);
        return {
          blocks: body.children.length,
          pre: body.querySelectorAll(':scope > pre > code').length,
          headings: body.querySelectorAll('h1,h2,h3,h4,h5,h6').length,
          text: body.querySelector('pre > code')?.textContent ?? '',
        };
      }, [ROW_TXT, BODY]);
      assert.equal(shape.blocks, 1, '区画の直下は 1 ブロック');
      assert.equal(shape.pre, 1, 'pre > code の 1 つ');
      assert.equal(shape.headings, 0, '見出しにならない');
      assert.equal(shape.text.startsWith('# '), true, '先頭の記号の行がそのまま文字で出る');
      // 対照: .md の行は見出しの木で出る（判定が常に見出し 0 件を返す空振りの防止）。
      assert.equal(await page.locator(`${ROW_MD} ${BODY} h1`).count(), 1);
      await safe(page, rec, csp);
      // 負の対照: 区画の中へ h1 を注入すると見出し 0 件の判定は不合格になる。
      await page.evaluate(([s, b]) => {
        document.querySelector(`${s} ${b}`).appendChild(document.createElement('h1'));
      }, [ROW_TXT, BODY]);
      assert.equal(await page.locator(`${ROW_TXT} ${BODY} h1`).count(), 1, 'control: injected h1 must be counted');
    });
  });

  test(`L4 文書以外のビューでは区画が 0 件になり、戻すと戻る（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page, srv, rec) => {
      await openAndPut(page, srv, irShown);
      await page.waitForSelector(`${ROW_MD} ${BODY}`, { timeout: 10000 });
      const payloads = await page.evaluate((b) => [...document.querySelectorAll(b)].map((x) => x.dataset.payload), SELECT);
      assert.deepEqual(payloads, ['document', 'file_tree'], '選択のボタンは宣言した 2 種');
      const total = () => page.locator(BODY).count();
      assert.equal(await total(), 2, 'md と txt の 2 行に区画');

      await page.locator(`${SELECT}[data-payload="file_tree"]`).click();
      await page.waitForFunction((b) => document.querySelector(`${b}[data-payload="file_tree"]`).dataset.selected === 'true', SELECT);
      assert.equal(await total(), 0, 'ファイルツリーでは区画 0 件');
      await assertNoPrototypeReason(page);

      await page.locator(`${SELECT}[data-payload="document"]`).click();
      await page.waitForSelector(`${ROW_MD} ${BODY}`, { timeout: 10000 });
      assert.equal(await total(), 2, '文書に戻すと 2 件');
      assert.equal(await page.locator(`${ROW_MD} ${BODY} h1`).count(), 1);
      await safe(page, rec, csp);
      // 負の対照: 区画を注入すると 0 件の判定は不合格になる。
      await page.locator(`${SELECT}[data-payload="file_tree"]`).click();
      await page.waitForFunction((b) => document.querySelector(`${b}[data-payload="file_tree"]`).dataset.selected === 'true', SELECT);
      await page.evaluate(() => {
        const d = document.createElement('div');
        d.setAttribute('data-testid', 'artifact-doc-body');
        document.querySelector('[data-testid="part-artifact-view"]').appendChild(d);
      });
      assert.equal(await total(), 1, 'control: injected body must be counted');
    });
  });

  test(`L5 replace で本文が替わっても前後の印は出ず、替わった後の木だけが出る（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page, srv, rec) => {
      await openAndPut(page, srv, irShown);
      await page.waitForSelector(`${ROW_MD} ${BODY} h1`, { timeout: 10000 });
      const first = await page.locator(`${ROW_MD} ${BODY} h1`).textContent();
      await approve(page);
      await page.waitForFunction(([s, f]) => {
        const h = document.querySelector(`${s} [data-testid="artifact-doc-body"] h1`);
        return h && h.textContent !== f;
      }, [ROW_MD, first], { timeout: 10000 });
      const marks = () => page.evaluate(([s, b]) => document.querySelectorAll(`${s} ${b} del, ${s} ${b} ins, ${s} ${b} [data-mark]`).length, [ROW_MD, BODY]);
      assert.equal(await page.locator(`${ROW_MD} ${BODY}`).count(), 1, '区画は 1 件のまま');
      assert.equal(await page.locator(`${ROW_MD} ${BODY} h1`).count(), 1, '替わった後の見出しだけ');
      assert.equal(await page.locator(`${ROW_MD} ${BODY}`).evaluate((e, f) => e.textContent.includes(f), first), false, '替わる前の見出しの文字は残らない');
      assert.equal(await marks(), 0, '前後の印は 0 件');
      await safe(page, rec, csp);
      // 負の対照: 区画の中へ data-mark 付きの要素を注入すると 0 件の判定は不合格になる。
      await page.evaluate(([s, b]) => {
        const d = document.createElement('span');
        d.setAttribute('data-mark', 'changed');
        document.querySelector(`${s} ${b}`).appendChild(d);
      }, [ROW_MD, BODY]);
      assert.equal(await marks(), 1, 'control: injected mark must be counted');
    });
  });
}
