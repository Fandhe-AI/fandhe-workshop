// サブパス配信の構成と相対パスの参照の試験（DLV-1・DLV-2。TASK-106 1/3）。
//
// 確かめること: (1) 配信物の資源の参照がすべて相対パスで、構成要素だけを指す（静的検査と、壊した写しでの負の対照）。
// (2) 1 つのサブパスで開いたページが取得する資源が構成要素だけで、status・Content-Type・gzip が表どおりである。
// (3) IR（ir/main.json）を含む構成でも同じで、相対の解決がサブパスの下に閉じる。
// 限界: web は実行時に IR を取得しないため、「ページが IR を取得する」ことは確かめない（TASK-109・TASK-121 の後に 2/3 以降で扱う）。
// ir/main.json が IR_FILE と同じバイトで入ることは scripts/test-build-web.sh の (b)(e) が保証する。
// 直接リンク 10 件は TASK-106 2/3、名前の違う 2 か所のサブパスとルート配信は 3/3 の範囲。
// 入力: FANDHE_WEB_DIR（配信する設定）・FANDHE_PACK_BIN。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer, openPage, summarize, waitMounted, cspConsoleMessages,
} from './harness.mjs';
import { DELIVERY_COMPONENTS, componentFromUrl, inspectDeliveryRefs } from './delivery.mjs';

const BASE = '/fandhe-delivery/';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const PACK_DIR = path.join(ROOT, 'testdata', 'engine-event-consistency', 'test-data', 'min-3step-pack');
let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to a built web directory');
  packBinFromEnv();
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 配信物を一時ディレクトリへ写して fn に渡し、必ず消す。
async function withCopy(fn) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-delivery-'));
  try {
    const copy = path.join(tmp, 'web');
    fs.cpSync(process.env.FANDHE_WEB_DIR, copy, { recursive: true });
    return await fn(copy);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

const edit = (dir, file, from, to) => {
  const p = path.join(dir, file);
  const s = fs.readFileSync(p, 'utf8');
  assert.ok(s.includes(from), `${file} does not contain ${from}`);
  fs.writeFileSync(p, s.replace(from, to));
};

test('配信物の参照がすべて相対パスで、構成要素だけを指す', () => {
  assert.deepEqual(inspectDeliveryRefs(process.env.FANDHE_WEB_DIR), []);
});

const NEGATIVES = [
  ['ルート絶対の参照', (d) => edit(d, 'index.html', './app.css', '/app.css'), /root-absolute/],
  ['外部の参照', (d) => edit(d, 'index.html', './app.js', 'https://example.invalid/app.js'), /has a scheme/],
  ['スキーム相対の参照', (d) => edit(d, 'index.html', './icon.svg', '//example.invalid/icon.svg'), /scheme-relative/],
  ['配信物の root の外へ出る参照', (d) => edit(d, 'wasm/fandhe_workshop_web.js', "'fandhe_workshop_web_bg.wasm'", "'../x.wasm'"), /"\.\." segment/],
  ['構成要素以外のファイル', (d) => fs.writeFileSync(path.join(d, 'extra.txt'), 'x'), /not a delivery component: extra\.txt/],
  ['必須の構成要素の欠落', (d) => fs.rmSync(path.join(d, 'app.css')), /missing required component: app\.css/],
  ['参照先の無い相対参照', (d) => edit(d, 'app.js', 'import init', 'import "./missing.js";\nimport init'), /missing\.js.*not a delivery component/],
  ['試験用の meta', (d) => edit(d, 'index.html', '</head>', '<meta name="fandhe-test-hooks" content="1"></head>'), /test-hooks meta/],
];
for (const [name, mutate, re] of NEGATIVES) {
  test(`負の対照: ${name}を検出する`, async () => {
    await withCopy(async (dir) => {
      mutate(dir);
      const problems = inspectDeliveryRefs(dir);
      assert.ok(problems.some((p) => re.test(p)), JSON.stringify(problems));
    });
  });
}

// 取得した資源がすべて構成要素で、status・Content-Type・gzip が表どおりであることを確かめる。
function assertRequests(rec, srv) {
  for (const r of rec.requests) {
    const c = componentFromUrl(r.url, { origin: srv.origin, base: BASE });
    assert.ok(c, `request outside the delivery components: ${r.url}`);
    assert.equal(r.status, 200, r.url);
    assert.equal(r.contentType, c.contentType, r.url);
    assert.equal(r.contentEncoding, c.gzip ? 'gzip' : '', r.url);
  }
}
function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  for (const k of ['external', 'failed', 'httpErrors', 'csp', 'pageErrors', 'consoleErrors']) assert.equal(s[k], 0, `${k}: ${detail}`);
  assert.deepEqual(cspConsoleMessages(rec), [], detail);
}

test('サブパスで開いたページの取得資源が構成要素だけである', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assertClean(rec);
    assertRequests(rec, srv);
    // icon.svg の取得はヘッドレスで揺れるため必須に含めない（smoke-hello・requestKinds と同じ扱い。直接取得は次の試験）。
    const got = new Set(rec.requests.map((r) => componentFromUrl(r.url, { origin: srv.origin, base: BASE })?.path));
    for (const p of ['index.html', 'app.js', 'app.css', 'wasm/fandhe_workshop_web.js', 'wasm/fandhe_workshop_web_bg.wasm']) {
      assert.ok(got.has(p), `missing request: ${p}`);
    }
    await context.close();
  });
});

test('IR を含む構成でも参照が相対で、直接の取得がサブパスの下に閉じる', async () => {
  const r = spawnSync(packBinFromEnv(), ['compile', PACK_DIR], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  assert.equal(r.status, 0, `compile exit ${r.status}: ${String(r.stderr).slice(0, 300)}`);
  await withCopy(async (dir) => {
    fs.mkdirSync(path.join(dir, 'ir'));
    fs.writeFileSync(path.join(dir, 'ir', 'main.json'), r.stdout);
    assert.deepEqual(inspectDeliveryRefs(dir), []);
    await withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.goto(srv.url);
      await waitMounted(page);
      for (const rel of ['ir/main.json', 'icon.svg']) {
        const resolved = await page.evaluate((x) => new URL(x, location.href).href, `./${rel}`);
        assert.equal(resolved, `${srv.url}${rel}`);
        const status = await page.evaluate(async (x) => (await fetch(x)).status, `./${rel}`);
        assert.equal(status, 200, rel);
      }
      assertClean(rec);
      assertRequests(rec, srv);
      const got = new Set(rec.requests.map((q) => componentFromUrl(q.url, { origin: srv.origin, base: BASE })?.path));
      for (const c of DELIVERY_COMPONENTS.filter((x) => x.required)) {
        if (c.path !== 'icon.svg') assert.ok(got.has(c.path), `missing request: ${c.path}`);
      }
      assert.ok(got.has('ir/main.json') && got.has('icon.svg'));
      await context.close();
    });
  });
});
