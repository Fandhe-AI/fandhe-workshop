// 章への直接リンク 10 件のサブパス配信の試験（DLV-1。TASK-106 2/3）。
//
// 確かめること: 直接リンク 10 件を、新しい文脈（保存領域が空）で順に開き、1 ページが取得する資源が構成要素だけで、
// すべて状態 200・期待どおりの Content-Type・gzip であること。期待しない要求・404・配信元以外への通信・ページ内エラー・
// CSP 違反・警告が 0 件であること。10 ページの要求の集合が同じであること。リンクの形は 2 つの候補（query・hash）の両方で流す。
// 限界: 配信する設定はクエリもハッシュも読まず、10 件とも既定の教材で開く。章の開始点の題と状態（DLV-7）は TASK-110、
// リンクの形は 2026-10-21 に `?start=` に確定（D-68）・実装は TASK-109。web は実行時に IR を取得しないので ir/main.json は写しに置くだけで、取得は確かめない。
// 名前の違う 2 か所のサブパスとルート配信は 3/3。判定（pageRequestProblems）は base を引数で受けるのでそのまま使える。
// 入力: FANDHE_WEB_DIR（配信する設定）・FANDHE_PACK_BIN。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer, openPage, waitMounted, requestKinds,
} from './harness.mjs';
import {
  DIRECT_LINKS, DIRECT_LINK_FORMS, directLinkUrl, pageRequestProblems, inspectDeliveryRefs, DELIVERY_COMPONENTS,
} from './delivery.mjs';

const BASE = '/fandhe-direct-links/';
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

// 配信物を一時ディレクトリへ写し、compile した IR を ir/main.json に置いて fn に渡し、必ず消す。
async function withCopy(fn) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-direct-'));
  try {
    const copy = path.join(tmp, 'web');
    fs.cpSync(process.env.FANDHE_WEB_DIR, copy, { recursive: true });
    const r = spawnSync(packBinFromEnv(), ['compile', PACK_DIR], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    assert.equal(r.status, 0, `compile exit ${r.status}: ${String(r.stderr).slice(0, 300)}`);
    fs.mkdirSync(path.join(copy, 'ir'));
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), r.stdout);
    return await fn(copy);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

// 1 件のリンクを新しい文脈で開き、記録を返す。
async function openLink(srv, url) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    const res = await page.goto(url);
    await waitMounted(page);
    await page.waitForLoadState('networkidle');
    return { rec, finalUrl: page.url(), entryStatus: res ? res.status() : 0 };
  } finally {
    await context.close();
  }
}

const baseOpts = (srv) => ({ origin: srv.origin, base: BASE });

for (const form of DIRECT_LINK_FORMS) {
  test(`直接リンク 10 件（${form}）の取得資源が構成要素だけである`, async () => {
    assert.equal(DIRECT_LINKS.length, 10);
    await withCopy(async (dir) => {
      assert.deepEqual(inspectDeliveryRefs(dir), []);
      await withServer({ dir, base: BASE }, async (srv) => {
        const results = [];
        for (const link of DIRECT_LINKS) {
          const url = directLinkUrl(srv.url, link, form);
          const r = await openLink(srv, url);
          const problems = pageRequestProblems(r.rec, baseOpts(srv));
          if (r.entryStatus !== 200) problems.push(`entry status ${r.entryStatus}`);
          if (r.finalUrl !== url) problems.push(`redirected or rewritten: ${r.finalUrl}`);
          assert.deepEqual(problems, [], `${link.chapter}/${link.start}`);
          results.push({ link, kinds: requestKinds(r.rec, baseOpts(srv)) });
        }
        const first = JSON.stringify(results[0].kinds);
        results.forEach((x, i) => {
          assert.equal(JSON.stringify(x.kinds), first, `link ${i + 1} (${x.link.start}) differs in request kinds`);
        });
      });
    });
  });
}

// 負の対照（純関数）。合成した記録に 1 件ずつ誤りを入れ、期待どおりの理由で検出されることを確かめる。
const O = 'http://127.0.0.1:1';
const U = (p) => `${O}${BASE}${p}`;
const good = () => ({
  requests: DELIVERY_COMPONENTS.filter((c) => c.required && c.path !== 'icon.svg').map((c) => ({
    url: U(c.path === 'index.html' ? '' : c.path), status: 200, resourceType: 'other', contentType: c.contentType, contentEncoding: 'gzip',
  })),
  external: [], failed: [], csp: [], pageErrors: [], consoleErrors: [], consoleWarnings: [], hits: [], dialogs: [], popups: [], crashed: false,
});
const PURE = [
  ['構成要素の外の要求', (r) => r.requests.push({ url: U('extra.txt'), status: 200, resourceType: 'other', contentType: 'text/plain', contentEncoding: '' }), /outside the delivery components/],
  ['404', (r) => { r.requests[1].status = 404; }, /status 404/],
  ['Content-Type の違い', (r) => { r.requests[1].contentType = 'text/plain'; }, /content-type/],
  ['gzip なし', (r) => { r.requests[1].contentEncoding = ''; }, /content-encoding/],
  ['配信元以外への通信', (r) => r.external.push('https://example.invalid/'), /external: 1/],
  ['重複した要求', (r) => r.requests.push({ ...r.requests[1] }), /duplicate request/],
  ['必須の要求の欠落', (r) => r.requests.pop(), /missing request/],
  ['CSP 以外の警告', (r) => r.consoleWarnings.push('something'), /consoleWarnings: 1/],
  ['ページ内エラー', (r) => r.pageErrors.push('boom'), /pageErrors: 1/],
  ['空の記録', (r) => { r.requests.length = 0; }, /missing request/],
];
test('負の対照の基準: 誤りの無い記録は問題 0 件', () => {
  assert.deepEqual(pageRequestProblems(good(), { origin: O, base: BASE }), []);
});
for (const [name, mutate, re] of PURE) {
  test(`負の対照（判定）: ${name}を検出する`, () => {
    const r = good();
    mutate(r);
    const problems = pageRequestProblems(r, { origin: O, base: BASE });
    assert.ok(problems.some((p) => re.test(p)), JSON.stringify(problems));
  });
}

// 負の対照（実ブラウザ）。
test('負の対照: ルート絶対の参照はサブパスの外で 404 になり検出される', async () => {
  await withCopy(async (dir) => {
    const p = path.join(dir, 'index.html');
    const s = fs.readFileSync(p, 'utf8');
    assert.ok(s.includes('./app.css'));
    fs.writeFileSync(p, s.replace('./app.css', '/app.css'));
    await withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        await page.goto(directLinkUrl(srv.url, DIRECT_LINKS[0], 'query'));
        await page.waitForLoadState('networkidle');
        await page.waitForTimeout(500);
      } finally {
        await context.close();
      }
      const problems = pageRequestProblems(rec, baseOpts(srv));
      // 外へ出た要求は 404 の応答か、読み込みの失敗（取得できず app.css が欠ける）として現れる。どちらでも検出できればよい。
      assert.ok(problems.some((x) => /outside the delivery components|status 404|missing request: app\.css/.test(x)), JSON.stringify(problems));
    });
  });
});

test('負の対照: パスの形のリンクは静的に解決されず入口が 404 になる', async () => {
  await withCopy(async (dir) => {
    await withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      let status = 0;
      try {
        const res = await page.goto(`${srv.url}${DIRECT_LINKS[0].chapter}/${DIRECT_LINKS[0].start}`);
        status = res ? res.status() : 0;
        await page.waitForLoadState('networkidle');
      } finally {
        await context.close();
      }
      assert.equal(status, 404);
      const problems = pageRequestProblems(rec, baseOpts(srv));
      assert.ok(problems.some((x) => /status 404|outside the delivery components/.test(x)), JSON.stringify(problems));
    });
  });
});
