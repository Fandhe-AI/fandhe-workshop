// L1（手元のサブパス）と L2（展開して ./start.sh）のオリジンが互いに分離していることの試験
// （TASK-115。DLV-16・D-58。`make test-l2-run` から呼ばれる。`make test-browser` には入れない）。
//
// 目的: 受講者の履歴は保存領域（localStorage）に入り、オリジン（スキーム・ホスト・ポート）ごとに分かれる。L1 で進めた履歴が L2 から
// 見えないこと、L2 で操作しても L1 の履歴が変わらないこと、同じサーバへ別名 `localhost` で入ると保存が見えないことを、
// 同じ browser context の中で固定する（W-29。PoC-10）。
// 3 つのオリジン（D-58 が L2 の案 (i) を採らないため、PoC の「L2 の 2 案」は repo にある L2 の形で読み替える。l1-l2-match と同じ前例）:
//   L1    tools/serve.mjs・base /fandhe-workshop/
//   L2-a  zip を展開した ./start.sh（miniserve）・base /
//   L2-b  同じ zip を別の展開先から別のポートで同時に起動したもの
// 3 つのサーバは試験の間ずっと同時に起動しておく。オリジンはポートを含むため、L1 を止めて起動し直すと別オリジンになり
// 「L1 を開き直す」が成り立たない。L2-a を止めてから L2-b を起動すると、同じポートが再利用されて同じオリジンになりうる。
// 8 項目（W-29）: (1) L1 で 2 操作して履歴が保存される、L2 の各オリジンで (2) 初期の Step から始まる・(3) 保存領域に履歴の鍵が 0 本・
// (4) L2 で操作した後も L1 の状態が残る、(8) 3 つのオリジンがすべて異なる。別名 `localhost`（同じサーバ・同じ base）は 9 項目めとして別に出す。
// 履歴の鍵には配信の区間（base のパス）が入るため、L1（/fandhe-workshop/）と L2（/）は鍵の名前が違う。「L1 の鍵が無い」だけでは区間の違いで
// 通ってしまうので、(3) は「履歴の鍵が 0 本」で判定し、オリジンだけが違う `localhost` を対照に置く。
// 対照: 同じオリジンでは開き直すと復元される（読み取りが 0 を返す壊れ方を検出する）／画面の判定は復元した画面で初期と見分けられる／
// 許可していないオリジンへの要求は数えられる。
// 限界: 教材は試験データ（ch0_3）で本物の CH-0〜CH-1 ではない。保存先は localStorage の範囲で、IndexedDB の分離は確かめない。
// Chromium・Linux の miniserve だけ（実機は TASK-118・119）。`localhost` の名前解決はブラウザの内部の解決に依る。
//
// 入力（環境変数。未設定・不在は skip にせず失敗にする）:
//   FANDHE_L2_ZIP   tools/build-l2.mjs が作った zip
//   FANDHE_L2_IR    zip に同梱した IR の JSON
//   FANDHE_L2_SITE  zip の元にした配信する設定の木（L1 が配る）
// 出力に履歴の本文・入力した値・DOM の文字・鍵の文字列を載せない（件数・ハッシュの先頭 12 桁・合否だけ）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { launchBrowser, channelFromEnv, openPage, waitMounted, summarize, cspConsoleMessages } from './harness.mjs';
import { startServer } from '../serve.mjs';
import { stepOrder, inspectDom, domObserver } from './l2-journey.mjs';
import { mouseDriver } from './ch0-3-journey.mjs';
import { startL2, stopL2 } from './l2-server.mjs';

const L1_BASE = '/fandhe-workshop/';
const FIRST = 'main/st-0-1';
const SECOND = 'main/st-0-2a';
const PREFIX = 'fandhe-workshop/history/';
const T = 30000;
const ctx = { browser: null, order: null, tmpDir: null, srv: null, runs: [], l2: [], page: null, context: null, rec: null, l1: null, localhost: null };
const items = []; // { name, ok }: 8 項目と localhost の項目

before(async () => {
  const zip = process.env.FANDHE_L2_ZIP;
  const irFile = process.env.FANDHE_L2_IR;
  const siteDir = process.env.FANDHE_L2_SITE;
  assert.ok(zip && fs.statSync(zip, { throwIfNoEntry: false })?.isFile(), 'FANDHE_L2_ZIP must point to the L2 zip');
  assert.ok(irFile && fs.statSync(irFile, { throwIfNoEntry: false })?.isFile(), 'FANDHE_L2_IR must point to the IR json');
  assert.ok(siteDir && fs.statSync(siteDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_L2_SITE must point to the delivery directory');
  ctx.order = stepOrder(fs.readFileSync(irFile, 'utf8'));
  ctx.tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l1-l2-origins-'));
  ctx.srv = await startServer({ dir: siteDir, base: L1_BASE, port: 0 });
  for (const name of ['l2-a', 'l2-b']) {
    const work = path.join(ctx.tmpDir, name);
    fs.mkdirSync(work);
    ctx.runs.push(await startL2(zip, work));
  }
  ctx.browser = await launchBrowser({ channel: channelFromEnv() });
  const l1 = new URL(ctx.srv.url).origin;
  ctx.l1 = { origin: l1, url: ctx.srv.url, asset: `${ctx.srv.url}icon.svg` };
  ctx.l2 = ctx.runs.map((r) => ({ origin: new URL(r.url).origin, url: r.url }));
  const port = new URL(ctx.srv.url).port;
  ctx.localhost = { origin: `http://localhost:${port}`, url: `http://localhost:${port}${L1_BASE}` };
  Object.assign(ctx, await openPage(ctx.browser, { origin: l1, extraOrigins: [...ctx.l2.map((x) => x.origin), ctx.localhost.origin] }));
});

after(async () => {
  if (ctx.context) await ctx.context.close().catch(() => {});
  if (ctx.browser) await ctx.browser.close().catch(() => {});
  for (const r of ctx.runs) await stopL2(r).catch(() => {});
  if (ctx.srv) await ctx.srv.close().catch(() => {});
  if (ctx.tmpDir) fs.rmSync(ctx.tmpDir, { recursive: true, force: true });
});

const short = (h) => h.slice(0, 12);
const sha = (s) => crypto.createHash('sha256').update(s).digest('hex');
const record = (name, ok) => {
  items.push({ name, ok });
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}`);
};

/** 保存領域の履歴の鍵（`fandhe-workshop/history/` で始まるもの）と値の SHA-256 の対を、鍵で整列して返す。鍵・値は呼び出し側が出さない。 */
async function historyEntries(page) {
  const raw = await page.evaluate((prefix) => {
    const out = [];
    for (let i = 0; i < localStorage.length; i += 1) {
      const k = localStorage.key(i);
      if (k !== null && k.startsWith(prefix)) out.push([k, localStorage.getItem(k) ?? '']);
    }
    return out;
  }, PREFIX);
  return raw.map(([k, v]) => ({ key: k, hash: sha(v) })).sort((a, b) => (a.key < b.key ? -1 : 1));
}

/** 履歴の鍵がちょうど n 本になるのを待つ（保存は変更から 1 秒以内。SAVE-4。余裕を見て待つ）。 */
async function waitHistoryKeys(page, n) {
  await page.waitForFunction(([prefix, k]) => {
    let c = 0;
    for (let i = 0; i < localStorage.length; i += 1) if ((localStorage.key(i) ?? '').startsWith(prefix)) c += 1;
    return c === k;
  }, [PREFIX, n], { timeout: T });
}

/** 右のログの行の (data-step, data-kind) の並び（文字は読まない）。 */
const logShape = (page) => page.evaluate(() => [...document.querySelectorAll('[data-testid="part-log-list"] li')]
  .map((li) => `${li.getAttribute('data-step')}|${li.getAttribute('data-kind')}`));

const isStep = async (page, uid, status) => (await page.evaluate(inspectDom, [ctx.order, uid, status])).ok;
/** 初期の Step（ログ 0 行・選択肢 3 件）か。 */
const isInitial = async (page) => (await isStep(page, FIRST, 'entered'))
  && (await logShape(page)).length === 0
  && (await page.locator('[data-testid="choice-btn"]').count()) === 3;

async function open(url) {
  await ctx.page.goto(url);
  await waitMounted(ctx.page);
}

/** アプリを起動せず、同じオリジンの静的な資源へ移って保存領域を読む（起動の記録で値が変わらない読み方）。 */
async function readWithoutApp(asset) {
  const res = await ctx.page.goto(asset);
  assert.equal(res?.status(), 200, 'static asset is served');
  return historyEntries(ctx.page);
}

const sameEntries = (a, b) => a.length === b.length && a.every((e, i) => e.key === b[i].key && e.hash === b[i].hash);
const prefixOf = (a, b) => a.length <= b.length && a.every((s, i) => b[i] === s);

let l1Keys = null; // L1 で 2 操作した後の履歴（鍵と値のハッシュ）
let l1Log = null; // 同じ時点の右のログの並び

test('(1) L1 で 2 操作して履歴が保存される', async () => {
  await open(ctx.l1.url);
  assert.ok(await isInitial(ctx.page), 'L1 starts at the first step');
  assert.deepEqual(await historyEntries(ctx.page), [], 'no history before the first operation');
  const d = mouseDriver(ctx.page);
  await d.choose('opt-right');
  await domObserver(ctx.order).at(ctx.page, FIRST, 'accepted');
  await d.point('cards/notice');
  await domObserver(ctx.order).at(ctx.page, SECOND, 'entered');
  await waitHistoryKeys(ctx.page, 1);
  l1Log = await logShape(ctx.page);
  assert.ok(l1Log.length >= 2, 'the log has rows');
  // 保存の値が落ち着くのを待つ（鍵が 1 本になった直後は最後の書き込みの前でありうる）。
  let prev = '';
  for (let n = 0; n < 50; n += 1) {
    const cur = JSON.stringify(await historyEntries(ctx.page));
    if (cur === prev) break;
    prev = cur;
    await ctx.page.waitForTimeout(100);
  }
  l1Keys = await readWithoutApp(ctx.l1.asset);
  assert.equal(l1Keys.length, 1);
  console.log(`L1 の履歴: 鍵 ${l1Keys.length} 本・値 ${short(l1Keys[0].hash)}・ログ ${l1Log.length} 行`);
  record('(1) L1 で 2 操作して履歴が保存される', true);
});

test('対照: 同じオリジンの L1 を開き直すと保存した状態が復元され、画面の判定は初期と見分けられる', async () => {
  assert.ok(l1Keys, '(1) must run first');
  await open(ctx.l1.url);
  assert.equal(await isInitial(ctx.page), false, 'the restored screen is not the initial one');
  assert.equal(await ctx.page.locator('[data-testid="choice-btn"]').count(), 1, 'the second step has one choice');
  assert.ok(prefixOf(l1Log, await logShape(ctx.page)), 'the saved log is kept (reload rows may be appended)');
  // 復元の後は値が変わりうるので、控えを取り直す（静的な資源の経路で）。
  l1Keys = await readWithoutApp(ctx.l1.asset);
  assert.equal(l1Keys.length, 1);
});

for (const [i, name] of ['L2-a', 'L2-b'].entries()) {
  test(`${name}: (2) 初期の Step から始まり (3) 履歴の鍵が 0 本で (4) 操作した後も L1 の状態が残る`, async () => {
    assert.ok(l1Keys, '(1) must run first');
    const l2 = ctx.l2[i];
    await open(l2.url);
    const initial = await isInitial(ctx.page);
    record(`(2) ${name} は初期の Step から始まる`, initial);
    assert.ok(initial);
    const none = (await historyEntries(ctx.page)).length === 0;
    record(`(3) ${name} の保存領域に履歴の鍵が 0 本`, none);
    assert.ok(none);
    // L2 でも保存が働くこと（0 本が「保存できない」ことによるものでない）と、鍵が L1 と違うこと。
    await mouseDriver(ctx.page).choose('opt-right');
    await domObserver(ctx.order).at(ctx.page, FIRST, 'accepted');
    await waitHistoryKeys(ctx.page, 1);
    const mine = await historyEntries(ctx.page);
    assert.notEqual(mine[0].key, l1Keys[0].key, 'L2 key differs from the L1 key');
    // L1 の保存領域は、L2 の操作の後も鍵と値が同じ。
    const kept = sameEntries(await readWithoutApp(ctx.l1.asset), l1Keys);
    // 続けて L1 のアプリを開き直すと、保存した状態の続きが出る。
    await open(ctx.l1.url);
    const restored = !(await isInitial(ctx.page)) && prefixOf(l1Log, await logShape(ctx.page));
    record(`(4) ${name} で操作した後も L1 の状態が残る`, kept && restored);
    assert.ok(kept, 'L1 storage is unchanged by the L2 operation');
    assert.ok(restored, 'L1 is restored after the L2 operation');
    l1Keys = await readWithoutApp(ctx.l1.asset);
  });
}

test('(8) L1・L2-a・L2-b のオリジンがすべて異なる', () => {
  const all = [ctx.l1.origin, ...ctx.l2.map((x) => x.origin)];
  const distinct = new Set(all).size === 3;
  record('(8) 3 つのオリジンがすべて異なる', distinct);
  assert.ok(distinct);
});

test('別名 localhost: 同じサーバ・同じ base でもオリジンが違い、L1 の保存が見えず、L1 は消えない', async () => {
  assert.notEqual(new URL(ctx.localhost.url).origin, ctx.l1.origin);
  await open(ctx.localhost.url);
  const initial = await isInitial(ctx.page);
  const none = (await historyEntries(ctx.page)).length === 0;
  record('(9) localhost の別名は別オリジンで初期の Step から始まり履歴の鍵が 0 本', initial && none);
  assert.ok(initial && none);
  await mouseDriver(ctx.page).choose('opt-right');
  await waitHistoryKeys(ctx.page, 1);
  // 127.0.0.1 の L1 は、別名での操作の後も変わらない。
  assert.ok(sameEntries(await readWithoutApp(ctx.l1.asset), l1Keys));
  await open(ctx.l1.url);
  assert.equal(await isInitial(ctx.page), false);
});

test('対照: 許可していないオリジンへの要求は数えられる', async () => {
  const before = { external: ctx.rec.external.length, failed: ctx.rec.failed.length };
  // どのサーバでもない 127.0.0.1:1 への遷移。route が abort して external に数える（遷移は失敗する）。
  await ctx.page.goto('http://127.0.0.1:1/probe').catch(() => null);
  assert.equal(ctx.rec.external.length, before.external + 1);
  // 意図した 1 件は計数から戻す。
  ctx.rec.external.length = before.external;
  ctx.rec.failed.length = before.failed;
});

test('8 項目が揃って通り、ページの計数が 0 件', () => {
  const eight = items.filter((x) => !x.name.startsWith('(9)'));
  console.log(`W-29: ${eight.filter((x) => x.ok).length}/${eight.length}`);
  assert.equal(eight.length, 8, '8 項目（(1)・(2)(3)(4) の 2 組・(8)）が揃う');
  assert.ok(eight.every((x) => x.ok));
  const c = summarize(ctx.rec);
  assert.deepEqual(
    { pageErrors: c.pageErrors, csp: c.csp, consoleErrors: c.consoleErrors, external: c.external, dialogs: c.dialogs, popups: c.popups, crashed: c.crashed },
    { pageErrors: 0, csp: 0, consoleErrors: 0, external: 0, dialogs: 0, popups: 0, crashed: 0 },
  );
  assert.equal(ctx.rec.consoleWarnings.length, 0);
  assert.equal(cspConsoleMessages(ctx.rec).length, 0);
  // 除くのは、応答が 200 で届いた同じ Request（id が同じ）の IR の取得が後から `net::ERR_ABORTED` と通知される競合だけ
  // （l1-l2-runner.mjs の irFetchCheck と同じ。docs/testing.md 5.268）。アプリは本文を読み切って起動しており、応答のない中断・
  // 別の資源・別のエラー種別は数える。
  const raced = (f) => f.error === 'net::ERR_ABORTED' && /\/ir\/main\.json$/.test(new URL(f.url).pathname)
    && ctx.rec.requests.some((r) => r.id === f.id && r.url === f.url && r.status === 200);
  assert.deepEqual(ctx.rec.failed.filter((f) => !raced(f)).map((f) => `${new URL(f.url).pathname}=${f.error}`), [], 'no failed request');
  assert.ok(ctx.rec.requests.every((r) => r.status >= 200 && r.status < 300), 'no non-2xx response');
});
