// 別の起源のページの iframe に教材を入れたときに、内容を出さないことの試験（SAFE-32・DLV-12。TASK-167 2/2）。
//
// 背景: <meta> の CSP では frame-ancestors が効かず、GitHub Pages は応答ヘッダを設定できない。案 B（ヘッダ）は採れず、
// D-44 で案 A（最上位のウィンドウでなければ内容を出さない JS。static/app.js の isTopWindow。TASK-167 1/2）に決まった。
// 入力: FANDHE_WEB_AUTHOR_DIR（配信する設定 + IR 同梱。TEST_HOOKS なし）。未設定・不在は skip にせず失敗にする。
// 配信は実物の静的サーバ（tools/serve.mjs）。親ページは試験が作る静的な HTML で、インラインのスクリプトは使わない。
// 確かめること（CSP あり・なしの両方）:
//   (X)  別の起源（別ポート）の親の iframe: data-mounted が framed、操作要素・教材の部品が 0 件、wasm・IR・試験用の資源の要求が 0 件、
//        保存先（localStorage・IndexedDB）が空、キー入力を送っても変わらない。
//   (XS) 別のサイト（127.0.0.1 の親と localhost の子）。(SO) 同じ起源の親。どちらも X と同じ期待。
//   (S)  sandbox=""（スクリプト不可）の iframe: 操作要素が 0 件、data-mounted は付かない。
//   (T)  最上位で直接開く正の対照: data-mounted が true で、題・ボタン・入力欄が出る。
//   (N)  負の対照: 写しの関門を常に真にすると、同じ埋め込みで教材が動く。検出器が 0 件を空振りしていないことを示す。
// 案 B だけでは足りない点: 子の応答に x-frame-options・content-security-policy のヘッダが無いこと（ヘッダで弾かれたのではなく JS の関門で隠れた）
// を前提として固定し、DELIVERY_CSP に frame-ancestors が無いことを diagnostic に残す。
// 限界: JS が無効なら関門は効かない（S は画面が JS で作られる構造による）。Pages の実物の応答は確かめない。Chromium だけ。
// 出力はコード・件数・真偽だけで、IR の本文は出さない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, DELIVERY_CSP } from './harness.mjs';

const BASE = '/fandhe-test/';
const PARENT_BASE = '/parent/';
const OPERABLE = 'input, textarea, select, button, [role="button"], [role="textbox"], [contenteditable]';
const ABSENT = ['step-title', 'draft-input', 'shell-left', 'shell-right', 'pane-tabs'];
const FORBIDDEN = ['.wasm', 'ir/main.json', 'test-hooks.js'];
const GATE = 'return window === window.top;';

let browser;
let authorDir;

before(async () => {
  const v = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(v && fs.statSync(v, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built without TEST_HOOKS');
  authorDir = path.resolve(v);
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

function parentHtml(src, sandbox) {
  const attr = sandbox ? ' sandbox=""' : '';
  return `<!doctype html>\n<html lang="ja"><head><meta charset="utf-8"><title>parent</title></head><body><iframe id="lesson" src="${src}" width="900" height="700"${attr}></iframe></body></html>\n`;
}

// 教材の写しを作って fn に渡し、必ず消す。mutate が真なら関門を常に真にする（負の対照）。
async function withLessonCopy({ csp, mutate = false, embedHtml = false }, fn) {
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-iframe-'));
  try {
    fs.cpSync(authorDir, copy, { recursive: true });
    if (mutate) {
      const p = path.join(copy, 'app.js');
      const src = fs.readFileSync(p, 'utf8');
      assert.equal(src.split(GATE).length - 1, 1, 'gate expression must occur exactly once');
      fs.writeFileSync(p, src.replace(GATE, 'return true;'));
    }
    if (embedHtml) fs.writeFileSync(path.join(copy, 'embed.html'), parentHtml('./', false));
    return await withCspMode({ dir: copy, csp }, fn);
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}

// 親ページだけを配る別サーバ（別ポート）。
async function withParentServer(src, sandbox, fn) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-iframe-parent-'));
  try {
    fs.writeFileSync(path.join(dir, 'index.html'), parentHtml(src, sandbox));
    return await withServer({ dir, base: PARENT_BASE }, fn);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

const pause = () => new Promise((r) => setTimeout(r, 400));

// 親ページを開き、iframe の中の観察を返す。childPrefix は子の URL（lesson の配信元の base まで）。
async function observeFrame({ parentUrl, parentOrigin, childOrigin, childPrefix, sandbox = false }) {
  const { page, context, rec } = await openPage(browser, { origin: parentOrigin, extraOrigins: [childOrigin] });
  try {
    const childReqs = [];
    let indexHeaders = null;
    page.on('request', (req) => {
      if (req.url().startsWith(childPrefix)) childReqs.push(req.url());
    });
    page.on('response', (res) => {
      if (res.url() === childPrefix || res.url() === `${childPrefix}index.html`) indexHeaders = res.headers();
    });
    await page.goto(parentUrl);
    const handle = await page.$('iframe#lesson');
    const frame = await handle.contentFrame();
    assert.ok(frame, 'iframe must have a frame');
    const root = frame.locator('html');
    if (!sandbox) await frame.waitForSelector('html[data-mounted]', { state: 'attached', timeout: 30000 });
    await pause();
    const snap = async () => {
      const out = {
        mounted: await root.getAttribute('data-mounted'),
        operable: await frame.locator(OPERABLE).count(),
        appChildren: await frame.evaluate(() => document.getElementById('app')?.children.length ?? 0),
        absent: {},
      };
      for (const id of ABSENT) out.absent[id] = await frame.locator(`[data-testid="${id}"]`).count();
      return out;
    };
    const first = await snap();
    let storage = null;
    let hook = null;
    if (!sandbox) {
      const read = () => frame.evaluate(async () => ({ ls: localStorage.length, idb: (await indexedDB.databases()).length }));
      const s1 = await read();
      hook = await frame.evaluate(() => typeof window.__fandheTest);
      await handle.click();
      await page.keyboard.type('help');
      await page.keyboard.press('Enter');
      await pause();
      const s2 = await read();
      storage = { ls: s1.ls, idb: s1.idb, ls2: s2.ls, idb2: s2.idb };
    }
    const second = await snap();
    return {
      before: first,
      after: second,
      storage,
      hook,
      forbidden: childReqs.filter((u) => FORBIDDEN.some((f) => u.includes(f))).length,
      indexHeaders,
      counts: summarize(rec),
    };
  } finally {
    await context.close();
  }
}

function assertClean(r) {
  assert.equal(r.counts.crashed, 0);
  assert.equal(r.counts.pageErrors, 0);
  assert.equal(r.counts.external, 0);
  assert.equal(r.counts.csp, 0);
}

function assertHidden(r) {
  for (const s of [r.before, r.after]) {
    assert.equal(s.mounted, 'framed');
    assert.equal(s.operable, 0, 'no operable elements in the frame');
    assert.equal(s.appChildren, 0);
    for (const id of ABSENT) assert.equal(s.absent[id], 0, `${id} must be absent`);
  }
  assert.equal(r.forbidden, 0, 'wasm, IR and test-only resources must not be fetched');
  assert.equal(r.hook, 'undefined');
  assert.deepEqual(r.storage, { ls: 0, idb: 0, ls2: 0, idb2: 0 }, 'storage must stay untouched');
  assertClean(r);
}

// 案 B（ヘッダ）で弾かれたのではないことの前提。ヘッダが付いていると、JS の関門を試したことにならない。
function assertNoFrameHeaders(r) {
  assert.ok(r.indexHeaders, 'index response must be observed');
  assert.equal(r.indexHeaders['x-frame-options'], undefined);
  assert.equal(r.indexHeaders['content-security-policy'], undefined);
}

for (const csp of [true, false]) {
  const label = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`X: 別の起源の親の iframe では内容が出ない（${label}）`, async (t) => {
    const r = await withLessonCopy({ csp }, (dir) =>
      withServer({ dir, base: BASE }, (child) =>
        withParentServer(child.url, false, (parent) =>
          observeFrame({ parentUrl: parent.url, parentOrigin: parent.origin, childOrigin: child.origin, childPrefix: child.url }),
        ),
      ),
    );
    assertHidden(r);
    assertNoFrameHeaders(r);
    t.diagnostic(`iframe-embed X csp=${csp} mounted=${r.after.mounted} operable=${r.after.operable} frame-ancestors-in-delivery-csp=${DELIVERY_CSP.includes('frame-ancestors')}`);
  });

  test(`XS: 別のサイト（localhost の子）の親の iframe では内容が出ない（${label}）`, async () => {
    const r = await withLessonCopy({ csp }, (dir) =>
      withServer({ dir, base: BASE }, (child) => {
        const url = child.url.replace('127.0.0.1', 'localhost');
        assert.notEqual(url, child.url, 'child must be addressed by a different host name');
        return withParentServer(url, false, (parent) =>
          observeFrame({ parentUrl: parent.url, parentOrigin: parent.origin, childOrigin: new URL(url).origin, childPrefix: url }),
        );
      }),
    );
    assertHidden(r);
  });

  test(`SO: 同じ起源の親の iframe でも内容が出ない（${label}）`, async () => {
    const r = await withLessonCopy({ csp, embedHtml: true }, (dir) =>
      withServer({ dir, base: BASE }, (srv) =>
        observeFrame({ parentUrl: `${srv.url}embed.html`, parentOrigin: srv.origin, childOrigin: srv.origin, childPrefix: srv.url }),
      ),
    );
    assertHidden(r);
  });

  test(`S: sandbox=""（スクリプト不可）の iframe では操作要素が 0 件（${label}）`, async (t) => {
    const r = await withLessonCopy({ csp }, (dir) =>
      withServer({ dir, base: BASE }, (child) =>
        withParentServer(child.url, true, (parent) =>
          observeFrame({ parentUrl: parent.url, parentOrigin: parent.origin, childOrigin: child.origin, childPrefix: child.url, sandbox: true }),
        ),
      ),
    );
    for (const s of [r.before, r.after]) {
      assert.equal(s.mounted, null, 'no script runs, so no mount mark');
      assert.equal(s.operable, 0);
      for (const id of ABSENT) assert.equal(s.absent[id], 0);
    }
    assert.equal(r.forbidden, 0);
    t.diagnostic(`iframe-embed S csp=${csp} mounted=${r.after.mounted} operable=${r.after.operable} (structural: the screen is built by JS)`);
  });

  test(`T: 正の対照 最上位で直接開くと教材が動く（${label}）`, async () => {
    const r = await withLessonCopy({ csp }, (dir) =>
      withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        try {
          await page.goto(srv.url);
          await page.waitForSelector('html[data-mounted]', { state: 'attached', timeout: 30000 });
          await pause();
          return {
            mounted: await page.locator('html').getAttribute('data-mounted'),
            title: await page.locator('[data-testid="step-title"]').count(),
            buttons: await page.locator('button').count(),
            input: await page.locator('[data-testid="draft-input"]').count(),
            counts: summarize(rec),
          };
        } finally {
          await context.close();
        }
      }),
    );
    assert.equal(r.mounted, 'true');
    assert.ok(r.title >= 1 && r.buttons >= 1 && r.input >= 1);
    assertClean(r);
  });

  test(`N: 負の対照 関門を常に真にすると同じ埋め込みで教材が動く（${label}）`, async () => {
    const r = await withLessonCopy({ csp, mutate: true }, (dir) =>
      withServer({ dir, base: BASE }, (child) =>
        withParentServer(child.url, false, (parent) =>
          observeFrame({ parentUrl: parent.url, parentOrigin: parent.origin, childOrigin: child.origin, childPrefix: child.url }),
        ),
      ),
    );
    assert.equal(r.before.mounted, 'true');
    assert.ok(r.before.operable >= 1, 'detector must see operable elements when the gate is off');
    assert.ok(r.before.absent['step-title'] >= 1);
    assert.ok(r.before.absent['draft-input'] >= 1);
  });
}
