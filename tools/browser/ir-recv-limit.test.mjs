// 同梱の IR の受信量が、メモリ確保の前に上限で止まることの試験（SAFE-33 の周辺。TASK-112 2/2 の指摘への対応）。
//
// 配信する設定の web（FANDHE_WEB_DIR。同梱の IR の印の meta は無い）の写しに、印の meta を足し、`./ir/main.json` として
// 上限（20 MiB）を大きく超える本体を流す。本体は 1 GiB の空白を 64 KiB ずつ書き続け、受け手が読むのをやめて接続を閉じたら止まる。
// 確かめること: (1) 画面が断りの通知（`pack-unloadable` 項目）に倒れ、ページ内エラーがない。(2) 配信側が書いた量が上限 + 数十 MiB に収まり、
// 1 GiB を最後まで送っていない（受信を中止した）。対照: 上限未満の本体（空白だけで JSON として不正）でも画面は断りに倒れるが、
// こちらは最後まで送られる（読み取りを途中で止めていない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { launchBrowser, channelFromEnv, openPage, waitMounted } from './harness.mjs';

const MIB = 1024 * 1024;
const LIMIT = 20 * MIB;
const HUGE = 1024 * MIB;
const NOTICE = '[data-testid="shell-notice-item"][data-notice-kind="pack-unloadable"]';
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.svg': 'image/svg+xml', '.json': 'application/json' };

let browser;
let tmp;

before(async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to the delivery web directory');
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-ir-recv-'));
  fs.cpSync(dir, tmp, { recursive: true });
  const index = path.join(tmp, 'index.html');
  const html = fs.readFileSync(index, 'utf8');
  assert.ok(html.includes('</head>'), 'index.html has a head');
  fs.writeFileSync(index, html.replace('</head>', '<meta name="fandhe-ir" content="1"></head>'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
  if (tmp) fs.rmSync(tmp, { recursive: true, force: true });
});

// 静的ファイルと、本体の大きさを指定できる ir/main.json を配る。state.sent は IR の本体として書いたバイト数。
function serve(irSize) {
  const state = { sent: 0, ended: false };
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    if (url.pathname === '/ir/main.json') {
      res.writeHead(200, { 'content-type': 'application/json' });
      const chunk = Buffer.alloc(64 * 1024, 0x20);
      let closed = false;
      res.on('close', () => {
        closed = true;
      });
      const pump = () => {
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
        }
      };
      pump();
      return;
    }
    const rel = url.pathname === '/' ? 'index.html' : url.pathname.slice(1);
    const file = path.resolve(tmp, rel);
    if (!file.startsWith(tmp + path.sep) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
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

async function run(irSize) {
  const { server, state, origin } = await serve(irSize);
  try {
    const { page, context, rec } = await openPage(browser, { origin });
    await page.goto(`${origin}/`);
    await waitMounted(page);
    await page.waitForSelector(NOTICE, { timeout: 30000 });
    // 配信側が止まるまで少し待つ（中止の伝わりは非同期）。
    await new Promise((r) => setTimeout(r, 500));
    const out = { notice: await page.locator(NOTICE).count(), sent: state.sent, ended: state.ended, pageErrors: rec.pageErrors.length };
    await context.close();
    return out;
  } finally {
    server.closeAllConnections();
    server.close();
  }
}

test('上限を大きく超える IR は、受信を途中で止めて断りの画面に倒れる', async () => {
  const r = await run(HUGE);
  assert.equal(r.notice, 1);
  assert.equal(r.pageErrors, 0);
  assert.equal(r.ended, false, 'server must not finish sending 1 GiB');
  assert.ok(r.sent < LIMIT + 64 * MIB, `sent ${r.sent} bytes; receiving was not stopped near the limit`);
});

test('対照: 上限未満の本体は最後まで送られる（読み取りを途中で止めていない）', async () => {
  const r = await run(LIMIT - 1);
  assert.equal(r.notice, 1);
  assert.equal(r.pageErrors, 0);
  assert.equal(r.ended, true);
  assert.equal(r.sent, LIMIT - 1);
});
