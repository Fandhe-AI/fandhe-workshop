// 同梱の IR が無い配信物をクエリなしで開いたときの表示の試験（BUILD-11・SHELL-31・SHELL-32。TASK-82 2/2）。
//
// 読み替え: 仕様の「`?ir` なし」は、D-68 以降は「クエリなし（`?start=` も無し）で開く」である。IR の場所は固定の `./ir/main.json`。
// 入力: FANDHE_WEB_DIR（印の無いビルド。IR_FILE なし）と FANDHE_WEB_AUTHOR_DIR（IR_FILE つき。どちらも TEST_HOOKS なし）。
// 未設定・不在は skip にせず失敗にする。配信は実物の静的サーバ（tools/serve.mjs と同じサブパス配信）。
// 確かめること（CSP あり・なしの両方）:
//   (A) 印の無いビルド: `ir/main.json` を取得しない（要求 0 件）。試験用の資源も取得せず、`__fandheTest` が無い。
//       通知の有無・入力欄の有無は観察だけ（印の無いビルドは従来の IR 未同梱の枠を出す。docs/known-gaps.md の SHELL-31）。
//   (B) 印つきの配信物から `ir/main.json` を消す: 要求がちょうど 1 件で 404、`pack-unloadable` の通知が 1 件で not_found、
//       教材の部品は 0 件、「模擬」は 1 件、通知欄は role=status。
//   (N) 負の対照: 消さない配信物では通知が 0 件、Step の題が出て、`ir/main.json` が 200 で 1 件。
// 限界: 印の無いビルドで通知を出すかは人の判断待ちのため assert しない。Chromium だけ。
// 出力はコード・件数・真偽だけで、IR の本文は出さない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize } from './harness.mjs';

const NOTICE = '[data-testid="shell-notice-item"][data-notice-kind="pack-unloadable"]';
const ABSENT = ['step-title', 'draft-input', 'shell-left', 'shell-right', 'pane-tabs'];
const BASE = '/fandhe-test/';
const FORBIDDEN = ['test-hooks.js', '.d.ts', 'min-3step'];

let browser;
let webDir;
let authorDir;

function requireDir(name) {
  const v = process.env[name];
  assert.ok(v && fs.statSync(v, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a web directory built without TEST_HOOKS`);
  return path.resolve(v);
}

before(async () => {
  webDir = requireDir('FANDHE_WEB_DIR');
  authorDir = requireDir('FANDHE_WEB_AUTHOR_DIR');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// クエリなしで開き、画面と要求の観察を返す。
async function observe(dir) {
  return withServer({ dir, base: BASE }, async (srv) => {
    const origin = srv.origin;
    const { page, context, rec } = await openPage(browser, { origin });
    try {
      const irReqs = [];
      let forbidden = 0;
      page.on('response', (res) => {
        const u = res.url();
        if (u.includes('/ir/main.json')) irReqs.push(res.status());
        if (FORBIDDEN.some((f) => u.includes(f))) forbidden += 1;
      });
      await page.goto(srv.url);
      await waitMounted(page);
      await new Promise((r) => setTimeout(r, 300));
      const count = (sel) => page.locator(sel).count();
      const notices = await count(NOTICE);
      const area = await count('[data-testid="shell-notice"]');
      const out = {
        irReqs,
        forbidden,
        notices,
        reason: notices ? await page.locator(NOTICE).first().getAttribute('data-notice-reason') : null,
        text: notices ? await page.locator(NOTICE).first().innerText() : '',
        area,
        role: area ? await page.locator('[data-testid="shell-notice"]').first().getAttribute('role') : null,
        mock: await count('[data-testid="shell-mock"]'),
        absent: {},
        hook: await page.evaluate(() => typeof window.__fandheTest),
        counts: summarize(rec),
      };
      for (const id of ABSENT) out.absent[id] = await count(`[data-testid="${id}"]`);
      return out;
    } finally {
      await context.close();
    }
  });
}

function assertSafe(r) {
  assert.equal(r.counts.crashed, 0, 'tab must not crash');
  assert.equal(r.counts.pageErrors, 0);
  assert.equal(r.counts.external, 0);
  assert.equal(r.counts.csp, 0);
  assert.equal(r.forbidden, 0, 'test-only resources must not be fetched');
  assert.equal(r.hook, 'undefined', '__fandheTest must not exist');
}

// 写しを作って fn に渡し、必ず消す。drop が真なら写しの ir/main.json を消す。
async function withAuthorCopy(csp, drop, fn) {
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-irabsent-'));
  try {
    fs.cpSync(authorDir, copy, { recursive: true });
    if (drop) fs.rmSync(path.join(copy, 'ir', 'main.json'));
    return await withCspMode({ dir: copy, csp }, fn);
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}

for (const csp of [true, false]) {
  const label = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`A: 印の無いビルドをクエリなしで開くと ir/main.json を取得せず教材も出ない（${label}）`, async (t) => {
    const r = await withCspMode({ dir: webDir, csp }, (dir) => observe(dir));
    assert.deepEqual(r.irReqs, [], 'ir/main.json must not be requested without the marker');
    assert.equal(r.absent['step-title'], 0);
    assertSafe(r);
    // 通知の有無は期待を固定しない（known-gaps の SHELL-31。人の判断待ち）。
    t.diagnostic(`ir-absent-open A csp=${csp} notices=${r.notices} draft-input=${r.absent['draft-input']}`);
  });

  test(`B: 印つきで ir/main.json が無い配信物は not_found の通知だけで教材を出さない（${label}）`, async (t) => {
    const r = await withAuthorCopy(csp, true, (dir) => observe(dir));
    assert.deepEqual(r.irReqs, [404]);
    assert.equal(r.notices, 1);
    assert.equal(r.reason, 'not_found');
    assert.ok(r.text.includes('（not_found）'));
    assert.ok(r.text.includes('教材が見つかりません'));
    assert.ok(!r.text.includes('教材を読み込めませんでした'));
    assert.equal(r.area, 1);
    assert.equal(r.role, 'status');
    assert.equal(r.mock, 1);
    for (const id of ABSENT) assert.equal(r.absent[id], 0, `${id} must be absent`);
    assertSafe(r);
    t.diagnostic(`ir-absent-open B csp=${csp} code=${r.reason} notices=${r.notices}`);
  });

  test(`N: 負の対照 ir/main.json を消さない配信物では通知が 0 件で教材が出る（${label}）`, async () => {
    const r = await withAuthorCopy(csp, false, (dir) => observe(dir));
    assert.deepEqual(r.irReqs, [200]);
    assert.equal(r.notices, 0);
    assert.ok(r.absent['step-title'] >= 1, 'step title must be shown');
    assertSafe(r);
  });
}
