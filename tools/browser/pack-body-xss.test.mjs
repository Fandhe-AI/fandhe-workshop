// Step の本文（教材パックの Markdown）への注入 49 件が、画面の本文の区画で何も実行されないことの試験（SAFE-1・SHELL-7。TASK-144・TASK-525 2/2。CSP あり・なし）。
//
// データの正は testdata の xss-cases.md の「入り口に E1（Step の本文の Markdown）を含む」49 件（xss-cases.mjs が読む。
// ペイロードをここへ書き写さない）。1 件ごとに最小教材 min-3step-pack を一時ディレクトリへ写し、st-1.md をペイロードで置き換えて
// 実物の `fandhe-pack compile` を流し、IR 版 2 を作る。
//
// 注入の口: 配信する設定の web の一時の写しに `fandhe-ir` の meta を足し、`ir/main.json` としてその IR を配る。`?start=` なしで開くと
// `starts` の先頭（st-1）が開かれ、枠の本文の区画（`step-body`。製品の描画経路 doc_view::render_doc だけを通る）に本文が出る。
// 試験側は innerHTML で何も入れず、その区画を観察する（試験 D の対照だけが意図的に生の入力を入れる）。
// 計数（実行・通信・CSP 違反・ダイアログ・危険な要素・ページの例外）が全区分 0、通信は配信物の構成要素と ir/main.json 1 件だけ、
// 生の HTML が文字として出ること、外部の画像が img にならず代わりの文字になることを確かめる。
// 試験 C は、試験の設定（TEST_HOOKS=1）の `loadIr` で同じ IR を入れた画面の本文の区画が、B（CSP あり）と同じ HTML になることの照合。
// 試験 A は試験の設定と配信する設定の CSP の meta が同一であることを固定する。
//
// CSP あり・なしの両方を、下の csp のループが流す。CSP なしは index.html の CSP の meta を除いた一時の写し（withCspMode）で、
// CSP に頼らず描画の経路だけで全区分が 0 になることを確かめる。
// 計数の語の対応（harness の SAFETY_KEYS）: 実行 = hits、外部通信 = external、ダイアログ = dialogs・popups、危険な要素 = danger、
// on* 属性 = onAttrs、style 属性 = styleAttrs、http(s) の出現 = extHttp・nonHttp、ページ内エラー = pageErrors・crashed・scanError。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_WEB_DIR（配信設定）・FANDHE_PACK_BIN（前提バイナリ）。
// どれも未設定・不在は skip にせず失敗にする。書き込み先は mkdtemp の一時ディレクトリの固定のパスだけで、元の web は書き換えない。
// URL は example.invalid だけを使う（ペイロード側）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { pageRequestProblems } from './delivery.mjs';
import { e1Payloads } from './xss-cases.mjs';

const BASE = '/fandhe-pack-body-xss/';
const PACK = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../testdata/engine-event-consistency/test-data/min-3step-pack');
const BODY_FILE = path.join('chapters', 'ch-min', 'st-1.md');
const CSP_META_RE = /<meta\b[^>]*\bhttp-equiv\s*=\s*["']?content-security-policy["']?[^>]*>/gi;
// 生の HTML が文字として出ることを確かめた件数の下限（分類 1・2 の全件。空振りを防ぐ）。
const MIN_RAW_CHECKED = 12;
// 外部の画像が代わりの文字（data-image-blocked）になったことを確かめた件数の下限（XSS-20・26 の分。空振りを防ぐ）。
const MIN_IMAGES_CHECKED = 2;
const BODY = '#app [data-testid="step-body"]';
const TITLE = '[data-testid="step-title"]';

let browser;
let cases;
let built; // [{ id, irText, title, imageAlts, rawTexts }]

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

// 1 件分の教材を作ってコンパイルする。書き込むのは一時ディレクトリの固定のパスだけで、ペイロードからパスを作らない。
function compileCase(bin, c) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-e1-'));
  try {
    fs.cpSync(PACK, dir, { recursive: true });
    fs.writeFileSync(path.join(dir, BODY_FILE), c.payload, 'utf8');
    const r = spawnSync(bin, ['compile', dir], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    assert.equal(r.status, 0, `${c.id}: compile exit ${r.status}: ${String(r.stdout).slice(0, 300)} ${String(r.stderr).slice(0, 300)}`);
    const ir = JSON.parse(r.stdout);
    assert.equal(ir.format_version, 2, c.id);
    assert.equal(ir.pack.ir_format, 2, c.id);
    const nodes = ir.nodes.filter((n) => n.step?.id === 'st-1');
    assert.equal(nodes.length, 1, `${c.id}: st-1 node`);
    const doc = nodes[0].body_doc;
    assert.ok(doc && typeof doc === 'object', `${c.id}: body_doc`);
    // 配信する設定は ?start= なしで starts の先頭を開く。観察しているのが st-1 の本文だと保証する。
    assert.equal(ir.starts[0]?.node, nodes[0].uid, `${c.id}: starts[0] is st-1`);
    const rawTexts = [];
    const imageAlts = [];
    const walk = (n) => {
      if (Array.isArray(n)) return n.forEach(walk);
      if (n && typeof n === 'object') {
        if (n.t === 'text' && typeof n.text === 'string' && n.text.includes('<')) rawTexts.push(n.text);
        if (n.t === 'image') imageAlts.push(typeof n.alt === 'string' ? n.alt : '');
        Object.values(n).forEach(walk);
      }
    };
    walk(doc);
    return { id: c.id, irText: r.stdout, title: nodes[0].step.title, imageAlts, rawTexts };
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  const bin = packBinFromEnv();
  cases = e1Payloads();
  assert.equal(cases.length, 49);
  built = cases.map((c) => compileCase(bin, c));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

function cspMetas(dir) {
  const found = fs.readFileSync(path.join(dir, 'index.html'), 'utf8').match(CSP_META_RE) ?? [];
  assert.equal(found.length, 1, `${dir}: exactly one CSP meta`);
  return found[0];
}

// 配信する設定の一時の写しに fandhe-ir の meta と ir/ を足して fn に渡し、成否にかかわらず消す（ir-recv-limit.test.mjs と同じ足し方）。
// 前提（`</head>` が 1 個・fandhe-ir と fandhe-test-hooks の meta が 0 個・CSP の meta が 1 個）が崩れたら失敗にする。
async function withDeliveryCopy(fn) {
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-e1-web-'));
  try {
    fs.cpSync(process.env.FANDHE_WEB_DIR, copy, { recursive: true });
    const index = path.join(copy, 'index.html');
    const html = fs.readFileSync(index, 'utf8');
    assert.equal(html.split('</head>').length - 1, 1, 'exactly one </head>');
    assert.ok(!/name\s*=\s*["']?fandhe-ir/i.test(html), 'delivery index has no fandhe-ir meta');
    assert.ok(!/name\s*=\s*["']?fandhe-test-hooks/i.test(html), 'delivery index has no test-hooks meta');
    assert.equal((html.match(CSP_META_RE) ?? []).length, 1, 'exactly one CSP meta');
    fs.writeFileSync(index, html.replace('</head>', '<meta name="fandhe-ir" content="1"></head>'));
    fs.mkdirSync(path.join(copy, 'ir'), { recursive: true });
    return await fn(copy);
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}

// 本文の区画から観察値を読む。固定の関数だけを evaluate に渡し、ペイロードからコードを組み立てない。
function readBody(page) {
  return page.evaluate((sel) => {
    const el = document.querySelector(sel);
    return {
      html: el.innerHTML,
      text: el.textContent,
      level: el.getAttribute('data-body-level'),
      anchors: [...el.querySelectorAll('a[href]')].map((a) => a.getAttribute('href')),
      imgs: el.querySelectorAll('img').length,
      blocked: el.querySelectorAll('[data-image-blocked]').length,
    };
  }, BODY);
}

test('A: 試験の設定と配信する設定の CSP の meta が同一', () => {
  assert.equal(cspMetas(process.env.FANDHE_WEB_TEST_DIR), cspMetas(process.env.FANDHE_WEB_DIR));
});

const htmlOf = new Map();
let imagesChecked = 0;

// CSP あり・なしのループ（なしは CSP の meta を除いた写しで流す）。件ごとに ir/main.json を書き換えてサーバを起動し直す
// （tools/serve.mjs が gzip の結果をサーバごとに鍵つきで持つため、同じサーバで書き換えると古い IR を返しうる）。
for (const csp of [true, false]) {
  test(`B: E1 の 49 件が配信する設定の画面の本文の区画に出ても、実行・通信・危険な要素が 0 件で生の HTML は文字（CSP ${csp ? 'あり' : 'なし'}）`, { timeout: 600000 }, async (t) => {
    let rawChecked = 0;
    let imgChecked = 0;
    await withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, async (dir) => {
      for (const b of built) {
        fs.writeFileSync(path.join(dir, 'ir', 'main.json'), b.irText, 'utf8');
        await withServer({ dir, base: BASE }, async (srv) => {
          const { page, context, rec } = await openPage(browser, { origin: srv.origin });
          try {
            await page.goto(srv.url);
            await waitMounted(page);
            assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', `${b.id}: no test hooks`);
            await page.waitForSelector(BODY, { timeout: 15000 })
              .catch((e) => { throw new Error(`${b.id}: step body not shown: ${e.message}`); });
            assert.equal(await page.locator('[data-testid="shell-notice-item"][data-notice-kind="pack-unloadable"]').count(), 0, `${b.id}: ir-notice shown`);
            assert.equal(await page.locator(TITLE).count(), 1, `${b.id}: step-title count`);
            assert.equal(await page.locator(TITLE).textContent(), b.title, `${b.id}: step-title`);
            assert.equal(await page.locator(BODY).count(), 1, `${b.id}: step-body count`);
            const info = await readBody(page);
            assert.ok(info.html.length > 0, `${b.id}: body html is empty`);
            assert.equal(info.level, 'A', `${b.id}: body level`);
            if (csp) htmlOf.set(b.id, info.html);

            const counts = await collectSafety(page, rec);
            const detail = JSON.stringify({ id: b.id, csp, counts, rec });
            assert.deepEqual(nonZero(counts), [], detail);
            assert.deepEqual(rec.hits, [], detail);
            // 通信は配信物の構成要素と ir/main.json 1 件だけ。IR の読み取りは上限に達すると reader.cancel() で閉じるため、
            // 応答 200 の ir/main.json が ERR_ABORTED として failed に載ることがある（本文は描画済み）。その 1 形だけを除いて判定する。
            const irUrl = `${srv.origin}${BASE}ir/main.json`;
            const irReqs = rec.requests.filter((r) => r.url === irUrl);
            assert.equal(irReqs.length, 1, `${b.id}: ir/main.json request count`);
            assert.equal(irReqs[0].status, 200, `${b.id}: ir/main.json status`);
            const recForNet = { ...rec, failed: rec.failed.filter((f) => !(f.url === irUrl && f.error === 'net::ERR_ABORTED')) };
            assert.deepEqual(pageRequestProblems(recForNet, { origin: srv.origin, base: BASE }), [], detail);

            // 生の HTML は文字として出る（NUL は構文解析で落ちるので除いて比べる。XSS-47）。
            const shown = info.text.replaceAll('\u0000', '');
            for (const raw of b.rawTexts) {
              assert.ok(shown.includes(raw.replaceAll('\u0000', '')), `${b.id}: raw text not shown: ${JSON.stringify(raw)}`);
              rawChecked += 1;
            }
            if (b.id === 'XSS-01') assert.ok(info.text.includes('<script>'), 'XSS-01 shows <script> as text');

            // 画像は img にならず、外部の画像は代わりの文字（alt）の span になる。
            assert.equal(info.imgs, 0, `${b.id}: img elements`);
            assert.equal(info.blocked, b.imageAlts.length, `${b.id}: blocked image count`);
            for (const alt of b.imageAlts) {
              if (alt !== '') assert.ok(shown.includes(alt.replaceAll('\u0000', '')), `${b.id}: image alt not shown: ${JSON.stringify(alt)}`);
              imgChecked += 1;
            }
            if (b.id === 'XSS-20' || b.id === 'XSS-26') assert.ok(b.imageAlts.length >= 1, `${b.id}: has an image`);

            for (const href of info.anchors) {
              assert.match(href, /^(https?:\/\/|[^:]*$)/i, `${b.id}: ${href}`);
              assert.ok(!href.startsWith('//') && !href.startsWith('\\'), `${b.id}: ${href}`);
            }
          } finally {
            await context.close();
          }
        });
      }
    }));
    t.diagnostic(`raw-text checks: ${rawChecked}, image alt checks: ${imgChecked}`);
    assert.ok(rawChecked >= MIN_RAW_CHECKED, `raw-text checks too few: ${rawChecked}`);
    assert.ok(imgChecked >= MIN_IMAGES_CHECKED, `image checks too few: ${imgChecked}`);
    imagesChecked = imgChecked;
  });
}

test('C: 試験の設定の loadIr で入れた画面の本文の区画が、配信する設定の画面（CSP あり）と同じ HTML で全区分 0', async () => {
  assert.equal(htmlOf.size, 49, 'run test B first (CSP on)');
  assert.ok(imagesChecked >= MIN_IMAGES_CHECKED, 'run test B first');
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    for (const b of built) {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        await page.goto(srv.url);
        await waitMounted(page);
        assert.equal(await page.evaluate(() => typeof window.__fandheTest?.loadIr), 'function', `${b.id}: test hooks`);
        const before = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.installCount);
        assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), b.irText), true, `${b.id}: load-ir sent`);
        await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, before, { timeout: 15000 })
          .catch((e) => { throw new Error(`${b.id}: IR was not installed: ${e.message}`); });
        const current = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.current);
        assert.ok(typeof current === 'string' && current.endsWith('st-1'), `${b.id}: current ${JSON.stringify(current)}`);
        await page.waitForSelector(BODY, { timeout: 15000 })
          .catch((e) => { throw new Error(`${b.id}: step body not shown: ${e.message}`); });
        const info = await readBody(page);
        // U+0000 は経路によって残る・落ちるの差が出る（XSS-47）。B と同じく除いて比べる。
        assert.equal(info.html.replaceAll('\u0000', ''), htmlOf.get(b.id).replaceAll('\u0000', ''), `${b.id}: body html differs from the delivery screen`);
        const counts = await collectSafety(page, rec);
        assert.deepEqual(nonZero(counts), [], JSON.stringify({ id: b.id, counts, rec }));
      } finally {
        await context.close();
      }
    }
  });
});

test('D: 対照。本文の区画へ描画を通さない生の入力を入れると、数え方が 0 でなく数える', async () => {
  const byId = new Map(cases.map((c) => [c.id, c.payload]));
  const ok = built.find((b) => b.id === 'XSS-01');
  await withDeliveryCopy((copy) => withServer({ dir: copy, base: BASE }, async (srv) => {
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), ok.irText, 'utf8');
    for (const id of ['XSS-01', 'XSS-07']) {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        await page.goto(srv.url);
        await waitMounted(page);
        await page.waitForSelector(BODY, { timeout: 15000 });
        await page.evaluate(([sel, html]) => {
          document.querySelector(sel).innerHTML = html;
        }, [BODY, byId.get(id)]);
        const counts = await collectSafety(page, rec);
        assert.notDeepEqual(nonZero(counts), [], `${id}: control should be counted ${JSON.stringify(counts)}`);
      } finally {
        await context.close();
      }
    }
  }));
});
