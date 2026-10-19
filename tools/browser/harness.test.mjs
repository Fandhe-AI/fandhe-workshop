// tools/browser/harness.mjs の試験（`make test-browser` から呼ばれる）。
//
// 前半は channel の切り替え（純関数）。後半は対照: 各区分が実際に 1 件以上数えられることを示し、
// 最小の画面の試験（smoke-hello.test.mjs）の「0 件」が空振りでないことを裏づける。
// Chromium は要るが、wasm のビルドは要らない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  resolveLaunchOptions, channelFromEnv, packBinFromEnv, launchBrowser, withServer, openPage, summarize, waitMounted,
} from './harness.mjs';

const CSP = "default-src 'self'; script-src 'self'; connect-src 'self'; img-src 'self'";
let tmp;
let browser;

function page(name, csp, body) {
  const head = csp ? `<meta http-equiv="Content-Security-Policy" content="${csp}">` : '';
  fs.writeFileSync(path.join(tmp, name), `<!doctype html><html><head><meta charset="utf-8">${head}</head><body>${body}</body></html>`);
}

before(async () => {
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-harness-test-'));
  page('csp.html', CSP, '<script>window.x = 1;</script>');
  page('err.html', CSP, '<script type="module" src="./boom.js"></script>');
  fs.writeFileSync(path.join(tmp, 'boom.js'), 'throw new Error("boom");\n');
  // 実ネットワークに出ない宛先（127.0.0.1 の別ポート）。route が abort するため接続は試みられない。
  page('ext.html', null, '<img src="http://127.0.0.1:9/x.png">');
  page('ok.html', CSP, '<p>ok</p>');
  page('mount-error.html', null, '<script>document.documentElement.setAttribute("data-mounted","error")</script>');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
  if (tmp) fs.rmSync(tmp, { recursive: true, force: true });
});

test('resolveLaunchOptions: chromium・未指定は channel なし、chrome は channel 指定', () => {
  assert.deepEqual(resolveLaunchOptions({ channel: 'chromium' }), { headless: true });
  assert.deepEqual(resolveLaunchOptions({}), { headless: true });
  assert.deepEqual(resolveLaunchOptions(), { headless: true });
  assert.deepEqual(resolveLaunchOptions({ channel: 'chrome' }), { channel: 'chrome', headless: true });
});

test('resolveLaunchOptions: 許可リスト外は例外（黙って切り替えない）', () => {
  for (const c of ['firefox', '', 'CHROME ', 'chrome-beta', 1]) {
    assert.throws(() => resolveLaunchOptions({ channel: c }), /unsupported browser channel/, String(c));
  }
});

test('channelFromEnv: 空・未設定は chromium', () => {
  assert.equal(channelFromEnv({}), 'chromium');
  assert.equal(channelFromEnv({ FANDHE_BROWSER_CHANNEL: '' }), 'chromium');
  assert.equal(channelFromEnv({ FANDHE_BROWSER_CHANNEL: 'chrome' }), 'chrome');
});

// 起動したブラウザの channel と版を診断の行に残す。Chrome の原本で流したことの証拠（docs/testing.md 5.128）に使う。
// 観察値の版は assert しない（Chrome の最新版は日々変わる。D-9）。空でない文字列であることだけを確かめる。
test('起動したブラウザの channel と版を記録する', (t) => {
  const channel = channelFromEnv();
  const version = browser.version();
  t.diagnostic(`browser: channel=${channel} version=${version}`);
  assert.equal(typeof version, 'string');
  assert.ok(version.length > 0);
});

async function visit(file) {
  return withServer({ dir: tmp, base: '/h/' }, async (srv) => {
    const { page: p, context, rec } = await openPage(browser, { origin: srv.origin });
    await p.goto(`${srv.url}${file}`);
    await p.waitForTimeout(500);
    const s = summarize(rec);
    await context.close();
    return { s, rec };
  });
}

test('対照: 何も違反しないページは全区分 0', async () => {
  const { s } = await visit('ok.html');
  assert.equal(s.csp + s.pageErrors + s.consoleErrors + s.external + s.failed + s.httpErrors, 0);
  assert.ok(s.requests >= 1);
});

test('対照: CSP 違反が数えられる', async () => {
  const { s, rec } = await visit('csp.html');
  assert.ok(s.csp >= 1, JSON.stringify(rec.csp));
});

test('対照: ページ内エラーが数えられる', async () => {
  const { s } = await visit('err.html');
  assert.ok(s.pageErrors >= 1);
});

test('対照: 配信元以外への通信が数えられ、abort される', async () => {
  const { s, rec } = await visit('ext.html');
  assert.ok(s.external >= 1);
  assert.ok(rec.external.every((u) => u.startsWith('http://127.0.0.1:9/')));
});

test('対照: HTTP エラーが数えられる', async () => {
  const { s } = await visit('missing.html');
  assert.ok(s.httpErrors >= 1);
});

test('waitMounted: data-mounted="error" は例外', async () => {
  await withServer({ dir: tmp, base: '/h/' }, async (srv) => {
    const { page: p, context } = await openPage(browser, { origin: srv.origin });
    await p.goto(`${srv.url}mount-error.html`);
    await assert.rejects(() => waitMounted(p, { timeout: 5000 }), /data-mounted="error"/);
    await context.close();
  });
});

test('packBinFromEnv: 未設定・存在しない・通常ファイルでないは FANDHE_PACK_BIN を名指して失敗し、ENOENT を漏らさない', () => {
  for (const env of [{}, { FANDHE_PACK_BIN: '' }, { FANDHE_PACK_BIN: path.join(tmp, 'no-such-bin') }, { FANDHE_PACK_BIN: tmp }]) {
    assert.throws(() => packBinFromEnv(env), (e) => /FANDHE_PACK_BIN/.test(e.message) && !/ENOENT/.test(e.message));
  }
  const file = path.join(tmp, 'csp.html');
  assert.equal(packBinFromEnv({ FANDHE_PACK_BIN: file }), file);
});
