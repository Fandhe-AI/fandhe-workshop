// IR の大きさの上限の境界を、配信する設定の画面の経路で確かめる試験（SAFE-17・SAFE-45。TASK-154）。
//
// 入力: FANDHE_WEB_AUTHOR_DIR（IR_FILE つき・TEST_HOOKS なしのビルド。同梱の IR の印の meta と compile 済みの `ir/main.json` を持つ）。
// 未設定・不在は skip にせず失敗にする。test hook は使わない。
// 配信は自前の http サーバで、`/ir/main.json` の本体だけを差し替える。土台の IR のバイト列を先に送り、残りを 64 KiB の空白で
// 指定の大きさまで流す（JSON の末尾の空白は有効）。10 倍は受信側が上限 + 1 で打ち切るため、200MB をメモリに確保しない。
// 確かめること:
//   (E) 上限ちょうど: 断りの通知が 0 件で、教材（Step の題）が出る。
//   (O) 上限 + 1: 断りの通知がちょうど 1 件で、契約の文面「ファイルが上限の大きさを超えています」とコード `too_large` が出る。
//   (T) 上限の 10 倍: (O) と同じ通知で、配信側は最後まで送っていない（受信を中止した）。
//   (O)(T) とも、取得の終わり（(O) は配信側が書き終えた時刻、(T) は受け手が接続を閉じた時刻）から通知が出るまで 1 秒未満。
//   全ケースで、タブが落ちない・ページ内エラー 0・配信元以外への通信 0・CSP 違反 0、通知の後も画面が応答する。
// 対象外: 教材を読み込めない画面の枠と最終の文面（TASK-545）、履歴の大きさ（TASK-155）、1 GiB の受信の打ち切り（ir-recv-limit）。
// 時間は待ちと CDP の往復を含む保守的な値で、値は diagnostic に大きさと ms だけを残す（IR の本文は出さない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { launchBrowser, channelFromEnv, openPage, waitMounted, summarize } from './harness.mjs';

const MIB = 1024 * 1024;
// contract の MAX_IR_BYTES（決定 D-44）。値は native の crates/core/tests/ir_size.rs が契約と照合する。
const LIMIT = 20_971_520;
const NOTICE = '[data-testid="ir-notice"]';
const STEP_TITLE = '[data-testid="step-title"]';
const MESSAGE = 'ファイルが上限の大きさを超えています';
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.svg': 'image/svg+xml', '.json': 'application/json' };

let browser;
let dir;
let base;

before(async () => {
  dir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS');
  dir = path.resolve(dir);
  base = fs.readFileSync(path.join(dir, 'ir', 'main.json'));
  assert.ok(base.length < LIMIT, 'base IR must be smaller than the limit');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 静的ファイルと、大きさを指定できる ir/main.json を配る。state は取得の終わりの時刻（performance.now）を持つ。
function serve(irSize) {
  const state = { sent: 0, ended: false, endAt: null, closeAt: null };
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    if (url.pathname === '/ir/main.json') {
      res.writeHead(200, { 'content-type': 'application/json' });
      const chunk = Buffer.alloc(64 * 1024, 0x20);
      let closed = false;
      res.on('close', () => {
        closed = true;
        state.closeAt = performance.now();
      });
      let head = true;
      const pump = () => {
        if (head) {
          head = false;
          state.sent += base.length;
          if (!res.write(base)) {
            res.once('drain', pump);
            return;
          }
        }
        while (!closed && state.sent < irSize) {
          const n = Math.min(chunk.length, irSize - state.sent);
          state.sent += n;
          if (!res.write(n === chunk.length ? chunk : chunk.subarray(0, n))) {
            res.once('drain', pump);
            return;
          }
        }
        if (!closed) {
          state.ended = true;
          res.end();
          state.endAt = performance.now();
        }
      };
      pump();
      return;
    }
    const rel = url.pathname === '/' ? 'index.html' : url.pathname.slice(1);
    const file = path.resolve(dir, rel);
    if (!file.startsWith(dir + path.sep) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] ?? 'application/octet-stream' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => resolve({ server, state, origin: `http://127.0.0.1:${server.address().port}` }));
  });
}

async function run(irSize, { expectNotice }) {
  const { server, state, origin } = await serve(irSize);
  try {
    const { page, context, rec } = await openPage(browser, { origin });
    // 通知の待ちは取得の前から始める（待ちの開始が取得の終わりより後ろにならないように）。
    const noticeAt = expectNotice ? page.waitForSelector(NOTICE, { timeout: 60000 }).then(() => performance.now()) : null;
    await page.goto(`${origin}/`);
    await waitMounted(page);
    const shownAt = noticeAt ? await noticeAt : null;
    if (!expectNotice) await page.waitForSelector(STEP_TITLE, { timeout: 30000 });
    // 配信側が止まるまで少し待つ（中止の伝わりは非同期）。
    await new Promise((r) => setTimeout(r, 500));
    const out = {
      notices: await page.locator(NOTICE).count(),
      role: expectNotice ? await page.locator(NOTICE).getAttribute('role') : null,
      text: expectNotice ? await page.locator(NOTICE).innerText() : '',
      titles: await page.locator(STEP_TITLE).count(),
      drafts: await page.locator('[data-testid="draft-input"]').count(),
      alive: await page.evaluate(() => document.documentElement.dataset.mounted),
      sent: state.sent,
      ended: state.ended,
      ms: shownAt === null ? null : shownAt - (state.endAt ?? state.closeAt ?? shownAt),
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

function assertRefused(r) {
  assert.equal(r.notices, 1);
  assert.equal(r.role, 'status');
  assert.ok(r.text.includes(MESSAGE), r.text);
  assert.ok(r.text.includes('too_large'), r.text);
  assert.equal(r.titles, 0);
  assert.equal(r.drafts, 0);
  assertSafe(r);
}

test('E: 上限ちょうどの IR は読み込まれ、教材が出る', async () => {
  const r = await run(LIMIT, { expectNotice: false });
  assert.equal(r.notices, 0);
  assert.ok(r.titles >= 1, 'a step title must be shown');
  assert.equal(r.ended, true);
  assert.equal(r.sent, LIMIT);
  assertSafe(r);
});

test('O: 上限 + 1 バイトの IR は理由つきで断られ、取得の終わりから 1 秒未満で出る', async (t) => {
  const r = await run(LIMIT + 1, { expectNotice: true });
  assertRefused(r);
  t.diagnostic(`ir-size over-by-one bytes=${LIMIT + 1} ms=${Math.round(r.ms)}`);
  assert.ok(r.ms < 1000, `notice took ${r.ms} ms`);
});

test('T: 上限の 10 倍の IR は受信を中止して理由つきで断られ、1 秒未満で出る', async (t) => {
  const r = await run(LIMIT * 10, { expectNotice: true });
  assertRefused(r);
  assert.equal(r.ended, false, 'server must not finish sending the oversize body');
  assert.ok(r.sent < LIMIT + 64 * MIB, `sent ${r.sent} bytes; receiving was not stopped near the limit`);
  t.diagnostic(`ir-size tenfold bytes=${LIMIT * 10} sent=${r.sent} ms=${Math.round(r.ms)}`);
  assert.ok(r.ms < 1000, `notice took ${r.ms} ms`);
});
