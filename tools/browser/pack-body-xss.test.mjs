// Step の本文（教材パックの Markdown）への注入 49 件が、画面で何も実行されないことの試験（SAFE-1。TASK-144。CSP あり・なし）。
//
// データの正は testdata の xss-cases.md の「入り口に E1（Step の本文の Markdown）を含む」49 件（xss-cases.mjs が読む。
// ペイロードをここへ書き写さない）。1 件ごとに最小教材 min-3step-pack を一時ディレクトリへ写し、st-1.md をペイロードで置き換えて
// 実物の `fandhe-pack compile` を流し、IR 版 2 の `body_doc`（本文の木）を作る。画面では、その IR を読み込み、本文の木を関門に通して描画し、
// 計数（実行・通信・CSP 違反・ダイアログ・危険な要素・ページの例外）が全区分 0 であること、生の HTML が文字として出ることを確かめる。
//
// 注入の口（仕様の「配信する設定の画面で Step の本文が出るまで操作する」の代え方）: 配信する設定は IR をまだ読まず（TASK-109・TASK-121）、
// Step の本文も枠に出していない（TASK-525 1/2）。そのため、試験の設定（TEST_HOOKS=1）の `loadIr` で IR を入れ、
// 本文の木を `checkDoc` → `renderDoc` に通した HTML の文字列を #app の外の容器へ innerHTML で入れる（doc-links.test.mjs と同じ、
// 本体のマウントと同じ「render の文字列 → innerHTML」の経路）。この口は showStepBody の 1 か所にまとめてあり、
// 上の 2 件が入った後に、配信する設定の画面で実際の本文の区画を見る形へ差し替える。
// 補い: 試験 A で試験の設定と配信する設定の CSP の meta が同一であることを固定し、試験 C で得た HTML を配信する設定のページへ入れて数える。
//
// CSP あり・なしの両方を、下の csp のループが流す。CSP なしは index.html の CSP の meta を除いた一時の写し（withCspMode）で、
// CSP に頼らず描画の経路（checkDoc → renderDoc）だけで全区分が 0 になることを確かめる。
// 計数の語の対応（harness の SAFETY_KEYS）: 実行 = hits、外部通信 = external、ダイアログ = dialogs・popups、危険な要素 = danger、
// on* 属性 = onAttrs、style 属性 = styleAttrs、http(s) の出現 = extHttp・nonHttp、ページ内エラー = pageErrors・crashed・scanError。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_WEB_DIR（配信設定）・FANDHE_PACK_BIN（前提バイナリ）。
// どれも未設定・不在は skip にせず失敗にする。URL は example.invalid だけを使う（ペイロード側）。
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
import { e1Payloads } from './xss-cases.mjs';

const BASE = '/fandhe-pack-body-xss/';
const PACK = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../testdata/engine-event-consistency/test-data/min-3step-pack');
const BODY_FILE = path.join('chapters', 'ch-min', 'st-1.md');
const CSP_META_RE = /<meta\b[^>]*\bhttp-equiv\s*=\s*["']?content-security-policy["']?[^>]*>/gi;
// 生の HTML が文字として出ることを確かめた件数の下限（分類 1・2 の全件。空振りを防ぐ）。
const MIN_RAW_CHECKED = 12;

let browser;
let cases;
let built; // [{ id, irText, bodyDocJson, rawTexts }]

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
    const rawTexts = [];
    const walk = (n) => {
      if (Array.isArray(n)) return n.forEach(walk);
      if (n && typeof n === 'object') {
        if (n.t === 'text' && typeof n.text === 'string' && n.text.includes('<')) rawTexts.push(n.text);
        Object.values(n).forEach(walk);
      }
    };
    walk(doc);
    return { id: c.id, irText: r.stdout, bodyDocJson: JSON.stringify(doc), rawTexts };
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

// 本文の木を描画して容器へ入れる（注入の口。上の 2 件が入った後は、配信する設定の画面の本文の区画の観察へ差し替える）。
// ペイロードは引数としてだけ渡し、コードの文字列を組み立てない。
async function showStepBody(page, id, bodyDocJson) {
  const gate = JSON.parse(await page.evaluate((t) => window.__fandheTest.checkDoc(t), bodyDocJson));
  assert.equal(gate.ok, true, `${id}: gate ${JSON.stringify(gate)}`);
  return page.evaluate(([t, sel]) => {
    const html = window.__fandheTest.renderDoc(t);
    const box = document.createElement('div');
    box.setAttribute('data-testid', sel);
    document.body.append(box);
    box.innerHTML = html;
    return {
      html,
      text: box.textContent,
      anchors: [...box.querySelectorAll('a[href]')].map((a) => a.getAttribute('href')),
      imgs: box.querySelectorAll('img').length,
      blocked: box.querySelectorAll('[data-image-blocked]').length,
    };
  }, [bodyDocJson, 'e1-body-probe']);
}

test('A: 試験の設定と配信する設定の CSP の meta が同一', () => {
  assert.equal(cspMetas(process.env.FANDHE_WEB_TEST_DIR), cspMetas(process.env.FANDHE_WEB_DIR));
});

const htmlOf = new Map();

// CSP あり・なしのループ（なしは CSP の meta を除いた写しで流す）。
for (const csp of [true, false]) {
  test(`B: E1 の 49 件が Step の本文として画面に出ても、実行・通信・危険な要素が 0 件で生の HTML は文字（CSP ${csp ? 'あり' : 'なし'}）`, async (t) => {
    let rawChecked = 0;
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
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

          const info = await showStepBody(page, b.id, b.bodyDocJson);
          assert.ok(info.html.length > 0, `${b.id}: rendered html is empty`);
          if (csp) htmlOf.set(b.id, info.html);

          const counts = await collectSafety(page, rec);
          const detail = JSON.stringify({ id: b.id, csp, counts, rec });
          assert.deepEqual(nonZero(counts), [], detail);
          assert.deepEqual(rec.hits, [], detail);

          // 生の HTML は文字として出る（NUL は構文解析で落ちるので除いて比べる。XSS-47）。
          const shown = info.text.replaceAll('\u0000', '');
          for (const raw of b.rawTexts) {
            assert.ok(shown.includes(raw.replaceAll('\u0000', '')), `${b.id}: raw text not shown: ${JSON.stringify(raw)}`);
            rawChecked += 1;
          }
          if (b.id === 'XSS-01') assert.ok(info.text.includes('<script>'), 'XSS-01 shows <script> as text');

          assert.equal(info.imgs, 0, `${b.id}: img elements`);
          for (const href of info.anchors) {
            assert.match(href, /^(https?:\/\/|[^:]*$)/i, `${b.id}: ${href}`);
            assert.ok(!href.startsWith('//') && !href.startsWith('\\'), `${b.id}: ${href}`);
          }
        } finally {
          await context.close();
        }
      }
    }));
    t.diagnostic(`raw-text checks: ${rawChecked}`);
    assert.ok(rawChecked >= MIN_RAW_CHECKED, `raw-text checks too few: ${rawChecked}`);
  });
}

test('C: 配信する設定の画面（試験の口なし・CSP あり）へ同じ HTML を入れても全区分 0', async () => {
  assert.equal(htmlOf.size, 49, 'run test B first (CSP on)');
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
      await page.evaluate((list) => {
        for (const [id, html] of list) {
          const box = document.createElement('div');
          box.setAttribute('data-case', id);
          document.body.append(box);
          box.innerHTML = html;
        }
      }, [...htmlOf.entries()]);
      assert.equal(await page.locator('[data-case]').count(), 49);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
    } finally {
      await context.close();
    }
  });
});

test('D: 対照。描画を通さない生の入力は、数え方が 0 でなく数える', async () => {
  const byId = new Map(cases.map((c) => [c.id, c.payload]));
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    for (const id of ['XSS-01', 'XSS-07']) {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        await page.goto(srv.url);
        await waitMounted(page);
        await page.evaluate((html) => {
          const box = document.createElement('div');
          document.body.append(box);
          box.innerHTML = html;
        }, byId.get(id));
        const counts = await collectSafety(page, rec);
        assert.notDeepEqual(nonZero(counts), [], `${id}: control should be counted ${JSON.stringify(counts)}`);
      } finally {
        await context.close();
      }
    }
  });
});
