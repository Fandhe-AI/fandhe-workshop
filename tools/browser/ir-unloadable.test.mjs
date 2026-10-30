// 教材 IR を読めないときの画面の試験（SHELL-31。TASK-545。404 の理由は SHELL-32・TASK-546）。
//
// 入力: FANDHE_WEB_AUTHOR_DIR（IR_FILE つき・TEST_HOOKS なしのビルド。IR の印の meta と compile 済みの `ir/main.json` を持つ）。
// 未設定・不在は skip にせず失敗にする。配信は自前の http サーバで、`/ir/main.json` の本体（と状態）だけを差し替える。
// 確かめること（CSP あり・なしの両方）:
//   (U1) 上限超え（20,971,521 バイト = too_large・65 段の入れ子 = too_deep）、(U2) 不正（uid の重複 = malformed・`{` = not_json）、
//   (U3) IR が無い（404 は not_found。本文が読める・不正でも本文を見ずに 404 で判定する。200 で本文が空は not_json のまま）のとき、枠の通知欄の `pack-unloadable` 項目がちょうど 1 件で、
//   理由のコードが期待どおり、文にコードが出る。教材（Step の題・入力欄・左右のエリア・タブ）は 0 件、「模擬」は 1 件、
//   通知欄は 1 件で role=status。タブが落ちず（data-mounted）、ページ内エラー・配信元以外への通信・CSP 違反は 0 件。
//   (C) 巡回する IR（読み替え）は読み込みの時点では断られず、断りの通知が 0 件で、タブが落ちない。
//       巡回を止めるのは実行時の遷移の上限（SAFE-23・34・35。transition-limit）である。
//   (U4) 配信物に `ir/main.json` が無い（静的配信の 404 そのもの）とき not_found で、「教材が見つかりません」が出て
//        「教材を読み込めませんでした」は出ない。not_found 以外では逆になる。
//   (N) 負の対照: 土台の IR では通知が 0 件で Step の題が出る（セレクタの空振りを防ぐ）。
// 読み替え・限界: 404 以外の失敗（5xx・通信の例外）は決定（D-74）に記述が無く、期待として固定せず観察だけを残す（U5）。
// 印の無いビルド（IR 未同梱の枠）は対象外。文面は仮置き（TASK-549・TASK-534）。出力はコード・件数・真偽だけで、IR の本文は出さない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { launchBrowser, channelFromEnv, withCspMode, openPage, waitMounted, summarize } from './harness.mjs';

const LIMIT = 20_971_520;
const NOTICE = '[data-testid="shell-notice-item"][data-notice-kind="pack-unloadable"]';
const ABSENT = ['step-title', 'draft-input', 'shell-left', 'shell-right', 'pane-tabs'];
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.svg': 'image/svg+xml', '.json': 'application/json' };
const CYCLES = '../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir';

let browser;
let authorDir;
let base;

before(async () => {
  authorDir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(authorDir && fs.statSync(authorDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS');
  authorDir = path.resolve(authorDir);
  base = fs.readFileSync(path.join(authorDir, 'ir', 'main.json'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

function paddedBase(size) {
  const b = Buffer.alloc(size, 0x20);
  base.copy(b);
  return b;
}

function nested(depth) {
  return Buffer.from('['.repeat(depth) + ']'.repeat(depth));
}

function duplicatedUid() {
  const v = JSON.parse(base.toString('utf8'));
  v.nodes.push(structuredClone(v.nodes[0]));
  return Buffer.from(JSON.stringify(v));
}

// ir/main.json を { status, body } で差し替えて静的ファイルを配る。ir が null のときは `ir/main.json` が無い配信物として 404 だけを返す。
function serve(dir, ir) {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    if (url.pathname === '/ir/main.json') {
      if (ir === null) {
        res.writeHead(404).end();
        return;
      }
      res.writeHead(ir.status, { 'content-type': 'application/json' });
      res.end(ir.body);
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
    server.listen(0, '127.0.0.1', () => resolve({ server, origin: `http://127.0.0.1:${server.address().port}` }));
  });
}

async function observe(csp, ir, query = '') {
  return withCspMode({ dir: authorDir, csp }, async (dir) => {
    const { server, origin } = await serve(path.resolve(dir), ir);
    try {
      const { page, context, rec } = await openPage(browser, { origin });
      await page.goto(`${origin}/${query}`);
      await waitMounted(page);
      await new Promise((r) => setTimeout(r, 300));
      // 0 件のときに locator が既定の 30 秒を待たないよう、件数を見てから読む。
      const notices = await page.locator(NOTICE).count();
      const area = await page.locator('[data-testid="shell-notice"]').count();
      const out = {
        notices,
        reason: notices ? await page.locator(NOTICE).first().getAttribute('data-notice-reason') : null,
        text: notices ? await page.locator(NOTICE).first().innerText() : '',
        area,
        role: area ? await page.locator('[data-testid="shell-notice"]').first().getAttribute('role') : null,
        mock: await page.locator('[data-testid="shell-mock"]').count(),
        absent: {},
        alive: await page.evaluate(() => document.documentElement.dataset.mounted),
        counts: summarize(rec),
      };
      for (const id of ABSENT) out.absent[id] = await page.locator(`[data-testid="${id}"]`).count();
      await context.close();
      return out;
    } finally {
      server.closeAllConnections();
      server.close();
    }
  });
}

function assertSafe(r) {
  assert.equal(r.counts.crashed, 0, 'tab must not crash');
  assert.equal(r.counts.pageErrors, 0);
  assert.equal(r.counts.external, 0);
  assert.equal(r.counts.csp, 0);
  assert.equal(r.alive, 'true', 'page must stay responsive');
}

function assertUnloadable(r, code) {
  assert.equal(r.notices, 1, `notices for ${code}`);
  assert.equal(r.reason, code);
  assert.ok(r.text.includes(`（${code}）`), `text for ${code}`);
  assert.equal(r.text.includes('教材が見つかりません'), code === 'not_found', `not-found wording for ${code}`);
  assert.equal(r.text.includes('教材を読み込めませんでした'), code !== 'not_found' && !code.startsWith('start_'), `load wording for ${code}`);
  assert.equal(r.area, 1);
  assert.equal(r.role, 'status');
  assert.equal(r.mock, 1);
  for (const id of ABSENT) assert.equal(r.absent[id], 0, `${id} must be absent for ${code}`);
  assertSafe(r);
}

const CASES = [
  ['U1 too_large', 'too_large', () => ({ status: 200, body: paddedBase(LIMIT + 1) })],
  ['U1 too_deep', 'too_deep', () => ({ status: 200, body: nested(65) })],
  ['U2 malformed', 'malformed', () => ({ status: 200, body: duplicatedUid() })],
  ['U2 not_json', 'not_json', () => ({ status: 200, body: Buffer.from('{') })],
  ['U3 404', 'not_found', () => ({ status: 404, body: Buffer.alloc(0) })],
  ['U3 404 with body', 'not_found', () => ({ status: 404, body: base })],
  ['U3 404 with broken body', 'not_found', () => ({ status: 404, body: Buffer.from('{') })],
  ['U4 missing file', 'not_found', () => null],
  ['U3 empty body', 'not_json', () => ({ status: 200, body: Buffer.alloc(0) })],
];

for (const csp of [true, false]) {
  for (const [name, code, make] of CASES) {
    test(`${name}: 教材を出さず読み込めない旨が枠の通知欄に出る（CSP ${csp ? 'あり' : 'なし'}）`, async (t) => {
      const r = await observe(csp, make());
      assertUnloadable(r, code);
      t.diagnostic(`ir-unloadable ${name} csp=${csp} code=${r.reason} notices=${r.notices}`);
    });
  }

  test(`U5: 404 以外の失敗は観察だけ（期待は固定しない。D-74 の範囲外）（CSP ${csp ? 'あり' : 'なし'}）`, async (t) => {
    const r = await observe(csp, { status: 500, body: Buffer.alloc(0) });
    t.diagnostic(`ir-unloadable U5 500 csp=${csp} code=${r.reason} notices=${r.notices}`);
    assertSafe(r);
  });

  test(`C: 巡回する IR は読み込みの時点では断られない（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
    for (const [file, start] of [['cycle-enter-self', ''], ['cycle-enter-self', '?start=s-enter-self'], ['cycle-exit-2', '?start=s-exit-2']]) {
      const body = fs.readFileSync(new URL(`${CYCLES}/${file}.json`, import.meta.url));
      const r = await observe(csp, { status: 200, body }, start);
      assert.equal(r.notices, 0, `${file}${start}`);
      assertSafe(r);
    }
  });

  test(`N: 負の対照 土台の IR では通知が 0 件で教材が出る（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
    const r = await observe(csp, { status: 200, body: base });
    assert.equal(r.notices, 0);
    assert.ok(r.absent['step-title'] >= 1, 'step title must be shown for a loadable IR');
    assertSafe(r);
  });
}
