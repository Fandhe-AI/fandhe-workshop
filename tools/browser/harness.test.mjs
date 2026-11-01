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
  resolveLaunchOptions, channelFromEnv, packBinFromEnv, previewBinFromEnv, launchBrowser, withServer, openPage, summarize, waitMounted, cspConsoleMessages, mimeConsoleMessages, requestKinds, makeRequestIdOf,
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

test('previewBinFromEnv: 未設定・存在しない・通常ファイルでないは FANDHE_PACK_PREVIEW_BIN を名指して失敗し、ENOENT を漏らさない', () => {
  for (const env of [{}, { FANDHE_PACK_PREVIEW_BIN: '' }, { FANDHE_PACK_PREVIEW_BIN: path.join(tmp, 'no-such-bin') }, { FANDHE_PACK_PREVIEW_BIN: tmp }]) {
    assert.throws(() => previewBinFromEnv(env), (e) => /FANDHE_PACK_PREVIEW_BIN/.test(e.message) && !/ENOENT/.test(e.message));
  }
  const file = path.join(tmp, 'csp.html');
  assert.equal(previewBinFromEnv({ FANDHE_PACK_PREVIEW_BIN: file }), file);
});

// 純関数の対照（SAFE-13。TASK-151 2/2）。csp-runs.test.mjs の「CSP 関連のコンソール出力 0 件」と「要求の種類が同じ」が
// 空振りでないこと（CSP の文言を拾う・差を検出する・除外は明示した分だけ）を示す。
test('cspConsoleMessages: error と warning のうち CSP の文言だけを拾う', () => {
  const rec = {
    consoleErrors: ["Refused to load the image 'data:...' because it violates the following Content Security Policy directive", 'boom'],
    consoleWarnings: ['The Content-Security-Policy directive x is ignored', 'deprecated api'],
  };
  assert.equal(cspConsoleMessages(rec).length, 2);
  assert.deepEqual(cspConsoleMessages({ consoleErrors: ['boom'], consoleWarnings: [] }), []);
  assert.deepEqual(cspConsoleMessages({ consoleErrors: [] }), [], 'consoleWarnings が無い記録でも落ちない');
});

test('mimeConsoleMessages: WASM の MIME 不一致の文言だけを拾う（DLV-3）', () => {
  const rec = {
    consoleErrors: ["Failed to execute 'compile' on 'WebAssembly': Incorrect response MIME type. Expected 'application/wasm'."],
    consoleWarnings: ['`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type.', 'The Content-Security-Policy directive x is ignored', 'deprecated api'],
  };
  assert.equal(mimeConsoleMessages(rec).length, 2);
  assert.deepEqual(mimeConsoleMessages({ consoleErrors: ['boom'], consoleWarnings: ['deprecated api'] }), []);
  assert.deepEqual(mimeConsoleMessages({ consoleErrors: [] }), [], 'consoleWarnings が無い記録でも落ちない');
});

test('requestKinds: 配信元の要求を相対パス・種類・状態の組にし、クエリを除き、重複と除外を畳む', () => {
  const o = 'http://127.0.0.1:1';
  const mk = (u, resourceType, status) => ({ url: u, resourceType, status });
  const rec = {
    requests: [
      mk(`${o}/b/`, 'document', 200), mk(`${o}/b/?fixture=x`, 'document', 200), mk(`${o}/b/app.js`, 'script', 200),
      mk(`${o}/b/icon.svg`, 'other', 200), mk(`${o}/other/x.js`, 'script', 200),
    ],
  };
  const k = requestKinds(rec, { origin: o, base: '/b/' });
  assert.deepEqual(k, ['(root) document 200', 'app.js script 200']);
  assert.deepEqual(requestKinds(rec, { origin: o, base: '/b/', exclude: [] }).length, 3, '除外は引数で明示した分だけ');
  const withExtra = { requests: [...rec.requests, mk(`${o}/b/x.png`, 'image', 200)] };
  assert.notDeepEqual(requestKinds(withExtra, { origin: o, base: '/b/' }), k, '余計な要求は差に現れる');
});

test('makeRequestIdOf: 別の Request には別の整数の id、同じ Request には同じ id（NaN で同一化しない）', () => {
  const idOf = makeRequestIdOf();
  const a = {};
  const b = {};
  const ia = idOf(a);
  const ib = idOf(b);
  assert.ok(Number.isInteger(ia) && Number.isInteger(ib), 'id は整数');
  assert.notEqual(ia, ib);
  assert.equal(idOf(a), ia);
  assert.equal(idOf(b), ib);
  // 完了した IR の取得の後に別の要求が ERR_ABORTED になっても、完了した側の id と一致しない。
  const done = new Set([ia]);
  assert.equal(done.has(ib), false);
});
