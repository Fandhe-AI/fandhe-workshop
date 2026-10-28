// WASM の MIME とストリーミング compile の試験（DLV-3。TASK-107 1/2）。配信する設定の web を手元の静的サーバで配り、
// .wasm が application/wasm・gzip で届き、MIME に関するコンソールの警告が 0 件で、WebAssembly.instantiateStreaming が
// 実際に使われて成功し、アプリがマウントされることを確かめる。
//
// 入力: 環境変数 FANDHE_WEB_DIR（配信する設定。scripts/test-browser.sh が用意する）。未設定・不在は skip にせず失敗にする。
// 0 件が空振りでないことの対照: .wasm を .bin に改名して application/octet-stream で配る写しでは、MIME の警告が出て
// ストリーミングが拒否され、バイト列の instantiate へ倒れる。倒れても画面は動くため、警告とストリーミングの直接観察が要る。
// 範囲外: 大きさ（gzip -9 で 1,048,576 バイト以下。DLV-4）は TASK-107 2/2。配信先の実物の MIME（DLV-24・DLV-25）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted, installWasmProbe, readWasmProbe, mimeConsoleMessages,
} from './harness.mjs';

const BASE = '/fandhe-wasm-mime/';
const WASM = 'wasm/fandhe_workshop_web_bg.wasm';
const BIN = 'wasm/fandhe_workshop_web_bg.bin';
const GLUE = 'wasm/fandhe_workshop_web.js';
let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to a built web directory');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 配信物を一時ディレクトリへ写し、.wasm を .bin へ改名して glue の参照も書き換える。置換が 1 件でなければ glue の形が変わったとして失敗。
async function withMisnamedWasm(fn) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-wasm-mime-'));
  try {
    const dir = path.join(tmp, 'web');
    fs.cpSync(process.env.FANDHE_WEB_DIR, dir, { recursive: true });
    fs.renameSync(path.join(dir, WASM), path.join(dir, BIN));
    const glue = path.join(dir, GLUE);
    const src = fs.readFileSync(glue, 'utf8');
    const from = "'fandhe_workshop_web_bg.wasm'";
    assert.equal(src.split(from).length - 1, 1, 'glue must reference the wasm file name exactly once');
    fs.writeFileSync(glue, src.replace(from, "'fandhe_workshop_web_bg.bin'"));
    return await fn(dir);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

async function run(dir, check) {
  await withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await installWasmProbe(context);
    await page.goto(srv.url);
    await waitMounted(page);
    const probe = await readWasmProbe(page);
    await check({ rec, probe, detail: JSON.stringify({ rec, probe }) });
    await context.close();
  });
}

test('.wasm は application/wasm・gzip で届き、MIME の警告 0 件でストリーミングの compile が成功してマウントされる', async () => {
  await run(process.env.FANDHE_WEB_DIR, ({ rec, probe, detail }) => {
    const wasm = rec.requests.filter((r) => r.url.endsWith('.wasm'));
    assert.equal(wasm.length, 1, detail);
    assert.equal(wasm[0].status, 200, detail);
    assert.equal(wasm[0].contentType, 'application/wasm', detail);
    assert.equal(wasm[0].contentEncoding, 'gzip', detail);
    assert.deepEqual(mimeConsoleMessages(rec), [], detail);
    assert.equal(probe.streaming.called, 1, detail);
    assert.equal(probe.streaming.resolved, 1, detail);
    assert.equal(probe.streaming.rejected, 0, detail);
    assert.equal(probe.instantiate.fromBytes, 0, `no fallback to bytes: ${detail}`);
    const s = summarize(rec);
    for (const k of ['csp', 'pageErrors', 'consoleErrors', 'external', 'failed', 'httpErrors']) assert.equal(s[k], 0, `${k}: ${detail}`);
  });
});

test('対照: application/wasm でない MIME では警告が出てストリーミングが拒否されバイト列へ倒れる（検出器が空振りでない）', async () => {
  await withMisnamedWasm((dir) => run(dir, ({ rec, probe, detail }) => {
    const bin = rec.requests.find((r) => r.url.endsWith('.bin'));
    assert.ok(bin, detail);
    assert.notEqual(bin.contentType, 'application/wasm', detail);
    assert.ok(mimeConsoleMessages(rec).length >= 1, detail);
    assert.ok(probe.streaming.rejected >= 1, detail);
    assert.ok(probe.instantiate.fromBytes >= 1, detail);
  }));
});
