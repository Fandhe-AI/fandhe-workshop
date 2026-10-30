// 構造の合わない巨大な入力の拒否のブラウザ試験（SAFE-24。TASK-161）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。
// 入力: FANDHE_WEB_AUTHOR_DIR（IR の経路。配信する設定。IR_FILE つき・TEST_HOOKS なし）と
//       FANDHE_WEB_TEST_DIR（履歴の経路。試験の設定。ops-limit と同じ入口）。未設定は skip にせず失敗にする。
// 読み替え: spec の SAFE-24 は `?ir=` を入口と書くが、D-68 で IR の入口は固定の `./ir/main.json` になった。
//   IR の経路は ir-size-limit と同じく、自前の http サーバで `/ir/main.json` の本体だけを差し替える。
// 断る側（A1: 100 万要素の配列 / A5: 500 万要素の配列 / S: 巨大な文字列 / M: 巨大な連想配列）:
//   大きさの上限（IR 20,971,520・履歴 10,485,760 バイト）に収め、理由が too_large にすり替わらないようにする。
//   IR は malformed、履歴は missing_version で断られる。通知は 1 件で、入力の値は画面に出ない。
//   通知までと長いタスクの最大は 1 秒未満（試作の実測は 281ms 程度。実測は diagnostic に残し、閾値は CI の揺れを見込む）。
//   履歴は断った後に状態・入力欄・保存先が変わらず、その後の操作が効く。タブが落ちず、外部通信・CSP 違反・ページ内エラーが 0 件。
// 開く側（形が合う対照）: step.lo の 100,000 要素・pack.title の 100,000 文字の IR は開く。履歴は実操作の履歴が開く。
//   100,000 は将来の要素数の上限（SAFE-25。TASK-162）と同じ値で、上限が入っても開き続ける最大の値である。
// 前提（TASK-162 への申し送り）: 断る側の期待は、要素数の数え上げが型の段の後で行われることに依る。生の値で先に数える設計なら見直す。
// 対象外: 受講者向けの最終の文面（TASK-545・TASK-549・TASK-534）。理由はコードで確かめる。
// 失敗の文・diagnostic には、バイト数・ms・固定のコードだけを出す（入力の中身は出さない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const MAX_IR = 20_971_520;
const MAX_HISTORY = 10_485_760;
const BUDGET_MS = 1000;
const NOTICE = '[data-testid="shell-notice-item"][data-notice-kind="pack-unloadable"]';
const STEP_TITLE = '[data-testid="step-title"]';
const DRAFT = '[data-testid="draft-input"]';
const FILE_INPUT = '[data-testid="history-file"]';
const ITEM = '[data-testid="shell-notice-item"][data-notice-kind="import-refused"]';
const DISMISS = `${ITEM} [data-testid="shell-dismiss"]`;
const OPERATION = '/make-note second-note';
const PACK = 'min-3step';
const START = 's-1';
const BASE = '/fandhe-shape-mismatch/';
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.svg': 'image/svg+xml', '.json': 'application/json' };

let browser;
let authorDir;
let baseIr;
let tmp;

before(async () => {
  authorDir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(authorDir && fs.statSync(authorDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS');
  authorDir = path.resolve(authorDir);
  const testDir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(testDir && fs.statSync(testDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a directory');
  baseIr = JSON.parse(fs.readFileSync(path.join(authorDir, 'ir', 'main.json'), 'utf8'));
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-shape-mismatch-'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
  if (tmp) fs.rmSync(tmp, { recursive: true, force: true });
});

// ---- 入力の組み立て（実行時に作り、repo に置かない） ----

const arrayOf = (n) => Buffer.from(`[${'0,'.repeat(n - 1)}0]`);
const stringOf = (total) => {
  const b = Buffer.alloc(total, 0x61);
  b[0] = 0x22;
  b[total - 1] = 0x22;
  return b;
};
const mapOf = (n) => {
  const parts = new Array(n);
  for (let i = 0; i < n; i++) parts[i] = `"k${String(i).padStart(7, '0')}":0`;
  return Buffer.from(`{${parts.join(',')}}`);
};
const shapes = (limit, mapKeys) => [
  ['A1', arrayOf(1_000_000)],
  ['A5', arrayOf(5_000_000)],
  ['S', stringOf(limit)],
  ['M', mapOf(mapKeys)],
];

// ---- 長いタスクの観測 ----

const OBSERVER = () => {
  window.__shapeLongTask = { max: 0, count: 0 };
  try {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        window.__shapeLongTask.count += 1;
        window.__shapeLongTask.max = Math.max(window.__shapeLongTask.max, e.duration);
      }
    }).observe({ type: 'longtask', buffered: true });
  } catch { /* longtask 非対応 */ }
};
const longTask = (page) => page.evaluate(() => window.__shapeLongTask ?? { max: -1, count: 0 });

// ---- IR の経路 ----

// 静的ファイルを配り、`/ir/main.json` だけ body に差し替える。待ち受けは 127.0.0.1 のみ。
function serveIr(body) {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    if (url.pathname === '/ir/main.json') {
      res.writeHead(200, { 'content-type': 'application/json' });
      res.end(body);
      return;
    }
    const rel = url.pathname === '/' ? 'index.html' : url.pathname.slice(1);
    const file = path.resolve(authorDir, rel);
    if (!file.startsWith(authorDir + path.sep) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] ?? 'application/octet-stream' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => resolve({ server, origin: `http://127.0.0.1:${server.address().port}` }));
  });
}

async function runIr(body, { expectNotice }) {
  assert.ok(body.length <= MAX_IR, 'IR body within the size limit');
  const { server, origin } = await serveIr(body);
  try {
    const { page, context, rec } = await openPage(browser, { origin });
    await context.addInitScript(OBSERVER);
    const shown = expectNotice ? page.waitForSelector(NOTICE, { timeout: 60000 }).then(() => performance.now()) : null;
    const t0 = performance.now();
    await page.goto(`${origin}/`);
    await waitMounted(page);
    const ms = shown ? (await shown) - t0 : null;
    if (!expectNotice) await page.waitForSelector(STEP_TITLE, { timeout: 30000 });
    const out = {
      notices: await page.locator(NOTICE).count(),
      role: expectNotice ? await page.locator('[data-testid="shell-notice"]').getAttribute('role') : null,
      text: expectNotice ? await page.locator(NOTICE).innerText() : '',
      titles: await page.locator(STEP_TITLE).count(),
      drafts: await page.locator(DRAFT).count(),
      alive: await page.evaluate(() => document.documentElement.dataset.mounted),
      long: await longTask(page),
      ms,
      counts: summarize(rec),
    };
    await context.close();
    return out;
  } finally {
    server.closeAllConnections();
    server.close();
  }
}

function assertSafe(r) {
  assert.equal(r.counts.crashed, 0, 'tab must not crash');
  assert.equal(r.counts.pageErrors, 0);
  assert.equal(r.counts.external, 0);
  assert.equal(r.counts.csp, 0);
  assert.equal(r.alive, 'true', 'page must stay responsive');
}

test('観測器の自己確認: 同期の長い処理が longtask として記録される', async () => {
  const { server, origin } = await serveIr(Buffer.from(JSON.stringify(baseIr)));
  try {
    const { page, context } = await openPage(browser, { origin });
    await context.addInitScript(OBSERVER);
    await page.goto(`${origin}/`);
    await waitMounted(page);
    // CDP の evaluate は longtask に数えられないため、ページ自身のタイマーの中で止める。
    await page.evaluate(() => { setTimeout(() => { const end = performance.now() + 300; while (performance.now() < end); }, 0); });
    // longtask の通知は非同期なので、記録されるまで少し待つ。
    await page.waitForFunction(() => window.__shapeLongTask.count >= 1, null, { timeout: 5000 }).catch(() => {});
    const l = await longTask(page);
    assert.ok(l.count >= 1 && l.max >= 250, `longtask observed (count=${l.count}, max=${Math.round(l.max)}ms)`);
    await context.close();
  } finally {
    server.closeAllConnections();
    server.close();
  }
});

for (const [id, body] of shapes(MAX_IR, 1_000_000)) {
  test(`IR ${id}: 構造の合わない巨大な入力は malformed で断られ、タブが落ちない`, async (t) => {
    const r = await runIr(body, { expectNotice: true });
    t.diagnostic(`shape=${id} entry=ir bytes=${body.length} notice_ms=${Math.round(r.ms)} longtask_max_ms=${Math.round(r.long.max)}`);
    assert.equal(r.notices, 1);
    assert.equal(r.role, 'status');
    assert.ok(r.text.includes('malformed'), 'notice carries the reason code');
    assert.ok(!r.text.includes('k0000000') && !r.text.includes('aaaa'), 'input values are not shown');
    assert.equal(r.titles, 0);
    assert.equal(r.drafts, 0);
    assert.ok(r.ms < BUDGET_MS, `notice took ${r.ms} ms`);
    assert.ok(r.long.max < BUDGET_MS, `longtask max ${r.long.max} ms`);
    assertSafe(r);
  });
}

function withLo(n) {
  const ir = structuredClone(baseIr);
  assert.ok(Array.isArray(ir.nodes) && ir.nodes[0]?.step, 'base IR has a first step');
  ir.nodes[0].step.lo = Array.from({ length: n }, () => 'x');
  return Buffer.from(JSON.stringify(ir));
}
function withTitle(n) {
  const ir = structuredClone(baseIr);
  ir.pack.title = 'あ'.repeat(n);
  return Buffer.from(JSON.stringify(ir));
}

for (const [id, make] of [['IR-lo', () => withLo(100_000)], ['IR-title', () => withTitle(100_000)]]) {
  test(`IR ${id}: 形の合う 100,000 要素・文字の入力は開く`, async (t) => {
    const body = make();
    const r = await runIr(body, { expectNotice: false });
    t.diagnostic(`shape=${id} entry=ir bytes=${body.length} longtask_max_ms=${Math.round(r.long.max)}`);
    assert.equal(r.notices, 0);
    assert.ok(r.titles >= 1, 'a step title must be shown');
    assertSafe(r);
  });
}

// ---- 履歴の経路（試験の設定。ops-limit と同じ入口） ----

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);
const engine = async (page) => JSON.parse(await call(page, 'stateJson')).engine;
const settle = (page) => page.waitForFunction(() => window.__fandheTest.nextTick() === 'null', null, { timeout: 15000 });
const fingerprint = (e) => JSON.stringify({ s: e.stateWithoutLog, l: e.log, g: e.generation, c: e.current });
const writeTmp = (name, body) => {
  const p = path.join(tmp, name);
  fs.writeFileSync(p, body);
  return p;
};

function assertQuiet(rec, label) {
  const s = summarize(rec);
  assert.equal(s.crashed, 0, `${label}: tab must not crash`);
  assert.equal(s.external, 0, `${label}: no external request`);
  assert.equal(s.csp, 0, `${label}: no CSP violation`);
  assert.equal(s.pageErrors, 0, `${label}: no page error`);
  assert.equal(s.dialogs, 0, `${label}: no dialog`);
  assert.equal(s.popups, 0, `${label}: no popup`);
}

async function freshPage(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.context.addInitScript(OBSERVER);
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  await settle(opened.page);
  return opened;
}

async function openStorage(page) {
  await call(page, 'storageOpen');
  await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [PACK, START], { timeout: 10000 });
}

// ファイル入力の change から通知の要素が現れるまでと、その間の長いタスクの最大をページの中で計る。
async function armTimer(page) {
  await page.evaluate(([sel, item]) => {
    window.__shapeT = { start: null, end: null };
    window.__shapeLongTask = { max: 0, count: 0 };
    document.querySelector(sel).addEventListener('change', () => { window.__shapeT.start = performance.now(); }, { capture: true, once: true });
    const done = () => {
      if (window.__shapeT.end == null && document.querySelector(item)) window.__shapeT.end = performance.now();
    };
    new MutationObserver(done).observe(document.body, { childList: true, subtree: true });
    done();
  }, [FILE_INPUT, ITEM]);
}

for (const [id, body] of shapes(MAX_HISTORY, 800_000)) {
  test(`履歴 ${id}: 構造の合わない巨大な入力は missing_version で断られ、状態も保存先も変わらない`, async (t) => {
    assert.ok(body.length <= MAX_HISTORY, 'history body within the size limit');
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const opened = await freshPage(srv);
      const { page } = opened;
      const file = writeTmp(`${id}.json`, body);

      await openStorage(page);
      await page.locator(DRAFT).fill(OPERATION);
      await page.locator(DRAFT).press('Enter');
      await settle(page);
      await page.waitForFunction(([p, s]) => window.__fandheTest.storageRead(p, s) != null, [PACK, START], { timeout: 10000 });
      await page.locator(DRAFT).fill('keep');
      await page.waitForTimeout(1500);
      const storedBefore = await call(page, 'storageRead', PACK, START);
      const fpBefore = fingerprint(await engine(page));
      const installBefore = (await engine(page)).installCount;

      await call(page, 'importInstall');
      await armTimer(page);
      await page.setInputFiles(FILE_INPUT, file);
      await page.waitForFunction(() => window.__shapeT.end != null, null, { timeout: 30000 });
      const { start, end } = await page.evaluate(() => window.__shapeT);
      const ms = end - start;
      const long = await longTask(page);
      t.diagnostic(`shape=${id} entry=history bytes=${body.length} notice_ms=${Math.round(ms)} longtask_max_ms=${Math.round(long.max)}`);
      assert.ok(start != null && ms < BUDGET_MS, `notice took ${ms} ms`);
      assert.ok(long.max < BUDGET_MS, `longtask max ${long.max} ms`);

      await settle(page);
      const e = await engine(page);
      assert.equal(e.lastImport.verdict, 'refused');
      assert.equal(e.lastImport.reason, 'missing_version');
      assert.equal(await page.locator(ITEM).count(), 1);
      assert.equal(await page.locator(ITEM).getAttribute('data-notice-reason'), 'missing_version');
      const text = await page.locator(ITEM).textContent();
      assert.ok(text.includes('missing_version'), 'notice carries the code');
      assert.ok(!text.includes('k0000000') && !text.includes('aaaa'), 'input values are not shown');
      assert.equal(e.installCount, installBefore, 'no environment was built');
      assert.equal(fingerprint(e) === fpBefore, true, 'state unchanged');
      assert.equal(await page.locator(DRAFT).inputValue(), 'keep', 'draft unchanged');
      assert.equal((await call(page, 'storageRead', PACK, START)) === storedBefore, true, 'stored value untouched');

      await page.locator(DRAFT).fill(OPERATION);
      await page.locator(DRAFT).press('Enter');
      await settle(page);
      assert.equal(fingerprint(await engine(page)) !== fpBefore, true, 'operation still works');
      await page.locator(DISMISS).click();
      assert.equal(await page.locator(ITEM).count(), 0, 'notice dismissed');
      assertQuiet(opened.rec, id);
      await opened.context.close();
    }));
  });
}

test('履歴 H-ok: 形の合う実操作の履歴は同じ入口から開く', async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const opened = await freshPage(srv);
    const { page } = opened;
    const text = await call(page, 'opsHistoryText', 100);
    assert.ok(text.length > 0 && text.length <= MAX_HISTORY);
    await call(page, 'importInstall');
    const before = (await engine(page)).installCount;
    await page.setInputFiles(FILE_INPUT, writeTmp('ok.json', text));
    await page.waitForFunction((n) => {
      const e = JSON.parse(window.__fandheTest.stateJson()).engine;
      return e && e.lastImport != null && e.lastImport.verdict === 'open' && e.installCount >= n;
    }, before, { timeout: 30000 });
    assert.equal(await page.locator(ITEM).count(), 0);
    assertQuiet(opened.rec, 'H-ok');
    await opened.context.close();
  }));
});
