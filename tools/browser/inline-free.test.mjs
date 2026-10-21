// インラインを使わない画面の検査（SAFE-15。TASK-151 2/2）。
//
// 配信する画面は、スクリプトを外部ファイル（app.js）、favicon を配信元の icon.svg にし、インラインのスクリプト・
// style 属性・on* 属性・style 要素・data: の画像・外部の取得を持たない。CSP（SAFE-13）が img-src 'self' などで
// これらを止める前提を崩さないよう、配信する index.html・app.css（配信・試験の両設定）を失敗で止める検査として確かめる。
//
// 確かめること:
//   - 正: 両設定の index.html・app.css が inspectInlineFree に合格し、app.js・icon.svg が配信元から直接取れる（200・型）
//   - 対照: 実物を加工した写しが、期待の理由で問題になる（検査が空振りしないことの記録）。
//     data: の favicon は PoC-7 の修正前の状態（違反が出る形）
//   - ブラウザの対照: 配信する設定の写しの body に data: の画像を 1 つ足すと、CSP ありでは CSP の違反がちょうど 1 件（img-src）、
//     CSP なしでは 0 件。加工前の実物では両方 0 件
//
// 確かめないこと: Rust が実行時に描く DOM の中身（csp-runs.test.mjs の scanDom が各場面で数える）。
// 入力: FANDHE_WEB_DIR（配信する設定）・FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在・ファイル欠落は skip にせず失敗にする。
// 加工した写しは mkdtemp に作り、成否にかかわらず消す。呼び出し元は scripts/test-browser.sh（make test-browser。CI の browser-test ジョブ）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, inspectInlineFree,
} from './harness.mjs';

const BASE = '/fandhe-inline-free/';
const SETTINGS = [['delivery', 'FANDHE_WEB_DIR', false], ['test', 'FANDHE_WEB_TEST_DIR', true]];

function dirOf(envName) {
  const dir = process.env[envName];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${envName} must point to a built web directory`);
  return dir;
}

function readPair(envName) {
  const dir = dirOf(envName);
  return {
    html: fs.readFileSync(path.join(dir, 'index.html'), 'utf8'),
    css: fs.readFileSync(path.join(dir, 'app.css'), 'utf8'),
  };
}

for (const [name, envName, testHooks] of SETTINGS) {
  test(`inline-free: ${name} 設定の index.html・app.css はインラインを使わない`, () => {
    const r = inspectInlineFree({ ...readPair(envName), testHooks });
    assert.deepEqual(r.problems, [], `${name}: ${r.problems.join('; ')}`);
    assert.equal(r.ok, true);
  });

  test(`inline-free: ${name} 設定で app.js は外部ファイル、favicon は配信元の icon.svg として取れる`, async () => {
    await withServer({ dir: dirOf(envName), base: BASE }, async (srv) => {
      const js = await fetch(`${srv.url}app.js`);
      assert.equal(js.status, 200);
      assert.match(js.headers.get('content-type') ?? '', /javascript/);
      const icon = await fetch(`${srv.url}icon.svg`);
      assert.equal(icon.status, 200);
      assert.match(icon.headers.get('content-type') ?? '', /^image\/svg\+xml/);
    });
  });
}

test('inline-free: 試験用の meta は試験の設定にだけ有り、配信する設定では問題になる（口の取り違えがない）', () => {
  assert.equal(inspectInlineFree({ ...readPair('FANDHE_WEB_DIR'), testHooks: false }).ok, true);
  assert.match(inspectInlineFree({ ...readPair('FANDHE_WEB_TEST_DIR'), testHooks: false }).problems.join(';'), /test hooks meta found/);
});

// 対照。実物（配信する設定）を加工した写しが、期待の理由で問題になる。問題の件数は expected で固定する。
const cases = [
  ['インラインの script を足す', /inline script found/, 1, ({ html }) => ({ html: html.replace('</body>', '<script>window.x = 1;</script></body>') })],
  ['style 属性を足す', /style attribute found on <div>/, 1, ({ html }) => ({ html: html.replace('<div id="app">', '<div id="app" style="color:red">') })],
  ['on* 属性を足す', /event handler attribute onclick/, 1, ({ html }) => ({ html: html.replace('<div id="app">', '<div id="app" onclick="x()">') })],
  ['style 要素を足す', /style element found/, 1, ({ html }) => ({ html: html.replace('</head>', '<style>a{}</style></head>') })],
  ['favicon を data: にする（PoC-7 の修正前）', /href on <link> uses a data\/javascript\/blob URL/, 2, ({ html }) => ({
    html: html.replace('href="./icon.svg"', 'href="data:image/svg+xml,%3Csvg%20xmlns=%27http://www.w3.org/2000/svg%27/%3E"'),
  })],
  ['外部の script を足す', /script with src count is 2/, 2, ({ html }) => ({
    html: html.replace('</body>', '<script src="https://example.invalid/x.js"></script></body>'),
  })],
  ['src つきの script に中身を同居させる', /script element has inline content/, 1, ({ html }) => ({
    html: html.replace(/(<script[^>]*src="\.\/app\.js"[^>]*>)(<\/script>)/, '$1alert(1)$2'),
  })],
  ['外部の画像を足す', /src on <img> is an external URL/, 1, ({ html }) => ({
    html: html.replace('</body>', '<img src="https://example.invalid/x.png"></body>'),
  })],
  ['プロトコル相対の画像を足す', /src on <img> is an external URL/, 1, ({ html }) => ({
    html: html.replace('</body>', '<img src="//example.invalid/x.png"></body>'),
  })],
  ['script の src を app.js 以外にする', /script is not type=module src=\.\/app\.js/, 1, ({ html }) => ({
    html: html.replace('src="./app.js"', 'src="./other.js"'),
  })],
  ['favicon の link を消す', /favicon link is not exactly one/, 1, ({ html }) => ({ html: html.replace(/\s*<link rel="icon"[^>]*>/, '') })],
  ['app.css に url(data:) を足す', /app\.css uses url\(/, 1, ({ css }) => ({ css: `${css}\nbody{background:url(data:image/gif;base64,AAAA)}\n` })],
  ['app.css に外部の @import を足す', /app\.css uses @import/, 1, ({ css }) => ({ css: `@import "https://example.invalid/a.css";\n${css}` })],
  ['配信する設定に試験用の meta を足す', /test hooks meta found in the delivery/, 1, ({ html }) => ({
    html: html.replace('</head>', '<meta name="fandhe-test-hooks" content="1"></head>'),
  })],
];
for (const [label, reason, count, fn] of cases) {
  test(`inline-free: 対照 ${label}`, () => {
    const real = readPair('FANDHE_WEB_DIR');
    const mutated = { ...real, ...fn(real) };
    assert.ok(mutated.html !== real.html || mutated.css !== real.css, '加工が適用されていない');
    const r = inspectInlineFree({ ...mutated, testHooks: false });
    assert.equal(r.ok, false);
    assert.match(r.problems.join('; '), reason);
    assert.equal(r.problems.length, count, r.problems.join('; '));
  });
}

// ブラウザの対照。data: の画像を body に足した写しで、CSP の有無により違反の数が変わる。
let browser;
before(async () => {
  browser = await launchBrowser({ channel: channelFromEnv() });
});
after(async () => {
  if (browser) await browser.close();
});

async function openAndCount(dir, csp) {
  return withCspMode({ dir, csp }, (served) => withServer({ dir: served, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      await new Promise((resolve) => setTimeout(resolve, 300));
      return { csp: rec.csp.map((c) => c.violatedDirective), external: rec.external.length, pageErrors: rec.pageErrors.length };
    } finally {
      await context.close();
    }
  }));
}

test('inline-free: ブラウザの対照 data: の画像は CSP ありで違反ちょうど 1 件（img-src）、CSP なしで 0 件。加工前は両方 0 件', async () => {
  const real = dirOf('FANDHE_WEB_DIR');
  for (const csp of [true, false]) {
    const r = await openAndCount(real, csp);
    assert.deepEqual(r, { csp: [], external: 0, pageErrors: 0 }, `加工前 CSP ${csp ? 'あり' : 'なし'}`);
  }
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-inline-free-'));
  try {
    fs.cpSync(real, copy, { recursive: true });
    const index = path.join(copy, 'index.html');
    const html = fs.readFileSync(index, 'utf8');
    const mutated = html.replace('</body>', '<img alt="" src="data:image/gif;base64,R0lGODlhAQABAAAAACw="></body>');
    assert.ok(mutated !== html);
    fs.writeFileSync(index, mutated);
    const withCsp = await openAndCount(copy, true);
    assert.equal(withCsp.csp.length, 1, JSON.stringify(withCsp));
    assert.match(withCsp.csp[0], /^img-src/);
    assert.equal(withCsp.pageErrors, 0);
    const without = await openAndCount(copy, false);
    assert.deepEqual(without.csp, []);
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
});
