// 名前の違うサブパス 2 か所とルート配信で同じ成果物を確かめる試験（DLV-2。TASK-106 3/3）。
//
// 役割: 1 つの写しのディレクトリを 3 通りの base（2 つのサブパスと '/'）で順に配り、どれにも同じ判定（pageRequestProblems）で
// 入口と直接リンク 10 件×2 形を流す。ビルド時に base を書き換えない（DLV-2）ことは、配信の前後で写しの sha256 が一致することで確かめる。
// 呼び出し元: scripts/test-browser.sh。判定・直接リンクの定義は delivery.mjs（1/3・2/3）を import し、再宣言しない。
// 確かめること: 3 通りとも取得資源が構成要素だけ・状態 200・警告 0 件で、base からの相対の要求の集合が同じであること。
// 負の対照: ルート絶対の参照はルート配信だけが見逃し、片方のサブパス名に固定した参照はそのサブパスだけが見逃す。
// 限界: web は実行時に IR を取得しない（TASK-109・TASK-121）ので ir/main.json の取得は確かめない。章の開始点（DLV-7）は TASK-110。
// 入力: FANDHE_WEB_DIR（配信する設定）・FANDHE_PACK_BIN。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer, openPage, waitMounted, requestKinds,
} from './harness.mjs';
import {
  DIRECT_LINKS, DIRECT_LINK_FORMS, directLinkUrl, pageRequestProblems, inspectDeliveryRefs,
} from './delivery.mjs';

const BASE_A = '/fandhe-bases-a/';
const BASE_B = '/another-repo-name/';
const BASE_ROOT = '/';
const BASES = [BASE_A, BASE_B, BASE_ROOT];
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
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-bases-'));
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

// 写し全体のファイル一覧と sha256（相対パス順）。配信が成果物を変えないことの確認に使う。
function treeDigest(dir) {
  const out = [];
  const walk = (rel) => {
    for (const ent of fs.readdirSync(path.join(dir, rel), { withFileTypes: true })) {
      const r = rel ? `${rel}/${ent.name}` : ent.name;
      if (ent.isDirectory()) walk(r);
      else out.push(`${r} ${crypto.createHash('sha256').update(fs.readFileSync(path.join(dir, r))).digest('hex')}`);
    }
  };
  walk('');
  return out.sort();
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

// 入口で相対参照の解決と試験用の口の不在を確かめ、直接リンク 10 件×2 形を開く。base ごとに同じ関数で判定する。
async function checkBase(dir, base) {
  const kindsByPage = [];
  await withServer({ dir, base }, async (srv) => {
    const opts = { origin: srv.origin, base };
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    let entryStatus = 0;
    try {
      const res = await page.goto(srv.url);
      entryStatus = res ? res.status() : 0;
      await waitMounted(page);
      await page.waitForLoadState('networkidle');
      assert.equal(page.url(), srv.url, `${base}: entry url rewritten`);
      // fetch の確認は要求を増やすため、入口の判定と要求の集合は確認の前に取る。
      assert.deepEqual(pageRequestProblems(rec, opts), [], `${base}: entry`);
      kindsByPage.push(JSON.stringify(requestKinds(rec, opts)));
      assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', `${base}: test hook exposed`);
      for (const rel of ['ir/main.json', 'icon.svg']) {
        const r = await page.evaluate(async (p) => {
          const u = new URL(`./${p}`, location.href).href;
          return { href: u, status: (await fetch(u)).status };
        }, rel);
        assert.equal(r.href, `${srv.url}${rel}`, `${base}: ./${rel} resolved elsewhere`);
        assert.equal(r.status, 200, `${base}: ./${rel} status`);
      }
    } finally {
      await context.close();
    }
    assert.equal(entryStatus, 200, `${base}: entry status`);
    for (const form of DIRECT_LINK_FORMS) {
      for (const link of DIRECT_LINKS) {
        const url = directLinkUrl(srv.url, link, form);
        const r = await openLink(srv, url);
        const problems = pageRequestProblems(r.rec, opts);
        if (r.entryStatus !== 200) problems.push(`entry status ${r.entryStatus}`);
        if (r.finalUrl !== url) problems.push(`redirected or rewritten: ${r.finalUrl}`);
        assert.deepEqual(problems, [], `${base} ${form} ${link.chapter}/${link.start}`);
        kindsByPage.push(JSON.stringify(requestKinds(r.rec, opts)));
      }
    }
  });
  return kindsByPage;
}

test('3 つの base は互いに違い、サブパス 2 か所はルートでない', () => {
  assert.equal(new Set(BASES).size, 3);
  assert.notEqual(BASE_A, '/');
  assert.notEqual(BASE_B, '/');
  assert.equal(DIRECT_LINKS.length, 10);
});

test('同じ成果物が 2 つのサブパスとルート配信で通り、要求の集合が同じで、配信が成果物を変えない', async () => {
  await withCopy(async (dir) => {
    assert.deepEqual(inspectDeliveryRefs(dir), []);
    const before = treeDigest(dir);
    const perBase = [];
    for (const base of BASES) perBase.push(await checkBase(dir, base));
    const all = new Set(perBase.flat());
    assert.equal(all.size, 1, `request kinds differ across pages or bases: ${[...all].join(' | ')}`);
    assert.deepEqual(treeDigest(dir), before, 'delivered files changed during serving');
  });
});

// 負の対照（実ブラウザ）。入口 1 ページの判定を 3 通りの base で並べ、期待の並びと比べる。
async function verdicts(dir) {
  const out = [];
  for (const base of BASES) {
    await withServer({ dir, base }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        await page.goto(srv.url);
        await page.waitForLoadState('networkidle');
        await page.waitForTimeout(500);
      } finally {
        await context.close();
      }
      out.push(pageRequestProblems(rec, { origin: srv.origin, base }).length === 0 ? 'pass' : 'fail');
    });
  }
  return out;
}

async function rewritten(from, to, expected) {
  await withCopy(async (dir) => {
    const p = path.join(dir, 'index.html');
    const s = fs.readFileSync(p, 'utf8');
    assert.ok(s.includes(from));
    fs.writeFileSync(p, s.replace(from, to));
    assert.ok(inspectDeliveryRefs(dir).some((x) => x.includes('root-absolute')), 'static check must flag it');
    assert.deepEqual(await verdicts(dir), expected);
  });
}

test('負の対照: ルート絶対の参照はルート配信だけが見逃す', async () => {
  await rewritten('./app.css', '/app.css', ['fail', 'fail', 'pass']);
});

test('負の対照: 片方のサブパス名に固定した参照はそのサブパスだけが見逃す', async () => {
  await rewritten('./app.css', `${BASE_A}app.css`, ['pass', 'fail', 'fail']);
});
