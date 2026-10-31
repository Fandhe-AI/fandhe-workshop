// CSP の meta の下で、直接リンク 10 件を IR つきの配信する設定で開く試験（DLV-11。TASK-111 2/2）。
//
// 確かめること:
//   A  配信する設定（CSP の meta あり）に 10 件の開始点を持つ IR を置き、サブパス配信の直接リンク 10 件（`?start=`）を新しい文脈で開く。
//      各ページで CSP の違反・コンソールの CSP 警告・ページ内エラー・配信元以外への通信が 0 件で、`ir/main.json` を 1 回だけ取得し、
//      WASM のエンジンが開始点の節点まで動いている（DOM の観察が「入ったばかり」を示し、次の Step を示さない）。
//      `s-0-1` は既定の開始点と同じ節点なので `?start=` の効きを区別できないが、残りの 9 件は区別できる。
//   B  同じ 10 件を CSP の meta なしでも開き、配信元への要求の種類が A の同じリンクと一致する（CSP が黙って何かを止めていれば差に出る）。
//   C  実行時の対照。C1 は CSP の `'wasm-unsafe-eval'` だけを外した写しで mount が error になり script-src の違反が出る（meta が実際に効き、
//      この指令が要ることの確認）。C2 は外部の画像を足した写しで違反が数えられ pageRequestProblems が検出する（計数が空振りしないことの確認）。
//      純関数の対照は合成の記録に csp を 1 件入れて検出されることを確かめる。
// 限界: `<meta>` では効かない指令（frame-ancestors 等。DLV-12）・Pages の実物（DLV-24）・Windows（SAFE-39）・本物の CH-0〜CH-3 の教材は対象外。
//   メタの配置と内容は csp-placement.test.mjs、IR なしの 10 件は delivery-direct-links.test.mjs が持つ。
// 入力: FANDHE_WEB_DIR（配信する設定）。未設定・不在は skip にせず失敗にする。出力は名前・件数・合否だけで、値は出さない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, requestKinds, inspectCspMeta, cspConsoleMessages,
} from './harness.mjs';
import { DIRECT_LINKS, directLinkUrl, pageRequestProblems } from './delivery.mjs';
import { TITLES, inspect, ORDER } from './author-preview.mjs';
import { irFetchCheck } from './l1-l2-runner.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const HAND_IR = path.resolve(HERE, '../../crates/web/tests/data/ch0_3/ir.json');
const BASE = '/fandhe-dlv11/';
const NOTICE = '[data-testid="shell-notice-item"]';
const WASM_EVAL = " 'wasm-unsafe-eval'";
let browser;
let irText;
let irNodes;

before(async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to a built web directory');
  // 開始点 10 件の IR（Rust 側の ch03_links_ir_bytes と同じ規則。pack.id だけ他の試験と分ける）。
  const ir = JSON.parse(fs.readFileSync(HAND_IR, 'utf8'));
  ir.pack.id = 'dlv11-csp-links';
  ir.starts = DIRECT_LINKS.map(({ start }) => ({ id: start, node: `main/st-${start.slice(2)}`, effects: [] }));
  irNodes = new Set(ir.nodes.map((n) => n.uid));
  irText = JSON.stringify(ir);
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 配信物を一時ディレクトリへ写し、IR を置いて fn に渡し、必ず消す。mutate(html) で index.html を加工できる（CSP の meta は残す）。
async function withCopy({ csp = true, mutate = (h) => h } = {}, fn) {
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-dlv11-'));
  try {
    fs.cpSync(process.env.FANDHE_WEB_DIR, copy, { recursive: true });
    const index = path.join(copy, 'index.html');
    let html = fs.readFileSync(index, 'utf8');
    // 元の配信物の CSP の meta は正規のもの。足す前に確かめ、この回の CSP が加工前は正規であることを保証する。
    const meta = inspectCspMeta(html);
    assert.deepEqual(meta.problems, [], 'original CSP meta is canonical');
    assert.equal(html.split('</head>').length - 1, 1, 'exactly one </head>');
    assert.ok(!/name\s*=\s*["']?fandhe-ir/i.test(html), 'delivery index has no fandhe-ir meta');
    assert.ok(!/name\s*=\s*["']?fandhe-test-hooks/i.test(html), 'delivery index has no test-hooks meta');
    html = html.replace('</head>', '<meta name="fandhe-ir" content="1"></head>');
    if (!csp) {
      const stripped = html.replace(/<meta http-equiv="Content-Security-Policy"[^>]*>/i, '');
      assert.notEqual(stripped, html, 'CSP meta removed');
      html = stripped;
    }
    html = mutate(html);
    fs.writeFileSync(index, html);
    fs.mkdirSync(path.join(copy, 'ir'), { recursive: true });
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), irText);
    return await fn(copy);
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}

const link = (l) => ({ ...l, uid: `main/st-${l.start.slice(2)}` });

// 1 件のリンクを新しい文脈で開き、DOM の観察と記録を返す。mount を待つ（失敗は例外）。
async function openLink(srv, l, { wait = true } = {}) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    const url = directLinkUrl(srv.url, l, 'query');
    const res = await page.goto(url);
    if (wait) {
      await waitMounted(page);
      await page.waitForLoadState('networkidle');
    }
    const out = { rec, url, finalUrl: page.url(), entryStatus: res ? res.status() : 0 };
    if (wait) {
      out.hooks = await page.evaluate(() => typeof window.__fandheTest);
      out.entered = (await inspect(page, l.uid, 'entered')).ok === true;
      const next = ORDER[ORDER.indexOf(l.uid) + 1];
      out.nextShown = next ? (await inspect(page, next, 'entered')).ok === true : false;
      out.title = await page.locator('[data-testid="step-title"]').first().textContent();
      out.notices = await page.locator(NOTICE).count();
    }
    return out;
  } finally {
    await context.close();
  }
}

const opts = (srv) => ({ origin: srv.origin, base: BASE });
const links = DIRECT_LINKS.map(link);
const kindsByLink = new Map();

test('A: IR つきの配信する設定で直接リンク 10 件が CSP の違反なしに開き、開始点まで WASM が動く', { timeout: 300000 }, async () => {
  assert.equal(links.length, 10);
  for (const l of links) assert.ok(irNodes.has(l.uid), `IR has node ${l.uid}`);
  await withCopy({}, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    let ok = 0;
    let violations = 0;
    let cspConsole = 0;
    let pageErrors = 0;
    let external = 0;
    for (const l of links) {
      const r = await openLink(srv, l);
      // IR の取得が応答の後に ERR_ABORTED と通知される既知の競合（l1-l2-runner.mjs の irFetchCheck）だけを除いて数える。
      const chk = irFetchCheck(r.rec, `${srv.origin}${BASE}ir/main.json`);
      const problems = pageRequestProblems({ ...r.rec, failed: chk.failed }, opts(srv));
      if (chk.failed.length > 0) problems.push(`failed requests: ${JSON.stringify(chk.failed.map((f) => [f.url, f.error]))}`);
      if (r.entryStatus !== 200) problems.push(`entry status ${r.entryStatus}`);
      if (r.finalUrl !== r.url) problems.push('redirected or rewritten');
      const irReqs = r.rec.requests.filter((q) => q.url === `${srv.origin}${BASE}ir/main.json`);
      if (irReqs.length !== 1 || irReqs[0].status !== 200) problems.push(`ir/main.json requests: ${irReqs.length}`);
      const wasm = r.rec.requests.filter((q) => q.url.endsWith('.wasm'));
      if (wasm.length !== 1 || wasm[0].status !== 200 || wasm[0].contentType !== 'application/wasm') problems.push('wasm request');
      if (r.hooks !== 'undefined') problems.push('test hook present');
      if (!r.entered) problems.push('start node not shown as entered');
      if (r.nextShown) problems.push('next step shown');
      if (r.title !== TITLES[l.uid]) problems.push('step title mismatch');
      if (r.notices !== 0) problems.push(`notices ${r.notices}`);
      assert.deepEqual(problems, [], `${l.chapter}/${l.start}`);
      violations += r.rec.csp.length;
      cspConsole += cspConsoleMessages(r.rec).length;
      pageErrors += r.rec.pageErrors.length;
      external += r.rec.external.length;
      kindsByLink.set(l.start, JSON.stringify(requestKinds(r.rec, opts(srv))));
      ok += 1;
    }
    console.log(`DLV11 csp=on links=${ok}/10 violations=${violations} csp-console=${cspConsole} page-errors=${pageErrors} external=${external}`);
  }));
});

test('B: CSP の meta なしでも配信元への要求の種類が A と一致する', { timeout: 300000 }, async () => {
  await withCopy({ csp: false }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    assert.equal(kindsByLink.size, 10, 'test A ran first');
    let same = 0;
    for (const l of links) {
      const r = await openLink(srv, l);
      assert.equal(JSON.stringify(requestKinds(r.rec, opts(srv))), kindsByLink.get(l.start), `request kinds differ: ${l.start}`);
      assert.equal(r.entered, true, `entered: ${l.start}`);
      same += 1;
    }
    console.log(`DLV11 csp=off kinds-match=${same}/10`);
  }));
});

test("C1: 'wasm-unsafe-eval' を外すと mount が error になり script-src の違反が出る", { timeout: 120000 }, async () => {
  const mutate = (h) => {
    assert.equal(h.split(WASM_EVAL).length - 1, 1, "exactly one 'wasm-unsafe-eval'");
    return h.replace(WASM_EVAL, '');
  };
  await withCopy({ mutate }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(directLinkUrl(srv.url, links[2], 'query'));
      await page.waitForSelector('html[data-mounted]', { state: 'attached', timeout: 30000 });
      const mounted = await page.evaluate(() => document.documentElement.getAttribute('data-mounted'));
      assert.equal(mounted, 'error', 'mount fails without wasm-unsafe-eval');
      assert.ok(rec.csp.some((v) => v.violatedDirective.startsWith('script-src')), 'script-src violation recorded');
      assert.ok(pageRequestProblems(rec, opts(srv)).some((p) => p.startsWith('csp:')), 'detector reports csp');
      console.log(`DLV11 control-wasm=${mounted}`);
    } finally {
      await context.close();
    }
  }));
});

test('C2: 外部の画像を足すと CSP の違反が数えられ検出される', { timeout: 120000 }, async () => {
  const mutate = (h) => {
    assert.equal(h.split('</body>').length - 1, 1, 'exactly one </body>');
    return h.replace('</body>', '<img src="https://example.invalid/x.png" alt=""></body>');
  };
  await withCopy({ mutate }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(directLinkUrl(srv.url, links[2], 'query'));
      await waitMounted(page);
      await page.waitForLoadState('networkidle');
      assert.ok(rec.csp.length >= 1, 'img-src violation recorded');
      assert.ok(pageRequestProblems(rec, opts(srv)).some((p) => p.startsWith('csp:')), 'detector reports csp');
      console.log('DLV11 control-img=detected');
    } finally {
      await context.close();
    }
  }));
});

test('純関数の対照: 合成の記録に csp を 1 件入れると検出される', () => {
  const rec = {
    requests: [], external: [], failed: [], csp: [{ violatedDirective: 'script-src-elem', blockedURI: 'inline' }], pageErrors: [],
    consoleErrors: [], consoleWarnings: [], hits: [], dialogs: [], popups: [], crashed: false,
  };
  const problems = pageRequestProblems(rec, { origin: 'http://127.0.0.1:1', base: BASE });
  assert.ok(problems.some((p) => p === 'csp: 1'), 'csp counted');
});
