// 正規の経路（入力欄から送った文字列を製品の自動保存が保存先に書き、復元・読み込みで戻す）で、入力した文字列が画面に文字として出ることの試験
// （SAFE-4。TASK-146 2/2。配信する設定・CSP あり・なし）。1/2（history-xss.test.mjs）は履歴ファイルの文字列の欄を置き換えた履歴が断られることを確かめた。
//
// データの正は testdata の xss-cases.md の「入り口に E4 を含む」14 件（xss-cases.mjs の e4Payloads が読む。ペイロードをここへ書き写さない）。
// 判定の関数・画面を読む関数・断片は e4-history.mjs（単体試験は e4-history.test.mjs）。
//
// 1 件ごとの流れ（新しい context。`?start=s-1-4`）: 入力欄にペイロードを入れて Enter で送る（そのまま）→ もう一度入れて送らずに残す（draft の操作）
//   → 自動保存が着地するまで待ち、保存先の履歴の `ops` に submit・draft の値としてペイロードがそのまま入ることを確かめる → 書き出したファイルにも同じ値が入る。
//   経路 restore: `page.reload()` で保存先から復元する。経路 file: 新しい context で書き出したファイルを読み込み欄から読み込む。
//   どちらも、受付の行の本文がペイロードそのものの文字ノード 1 つだけで、要素・属性にならず、続く拒否の行も文字だけであること。
//
// 確かめること（毎回）: 実行・通信・CSP 違反・危険な要素・ページのエラーが 0 件、`Object.prototype` が汚れず、拒否の通知（import-refused・
//   corrupt-history）が出ず、ペイロードの断片がコンソールに出ない。断片は画面の文字（text・textContent）に出て、属性の値には出ない。
//   通信は構成要素と ir/main.json 1 件だけ（再読み込みの前後で分けて数える）。
// 対照: B0（無害な番兵が同じ経路・同じ判定に通る）、D1（復元した後の行に生の HTML を入れると判定が不合格になり計数が 0 でなくなる）、
//   D2（保存先が空の新しい context では受付の行が 0 件で判定が不合格。復元した結果と新しく始めた画面を見分ける）。
//
// 入力: FANDHE_WEB_DIR（配信設定）・FANDHE_WEB_TEST_DIR（CSP の meta の同一性の確認だけに使う）。未設定は skip にせず失敗にする。
// 限界: 保存先は localStorage だけ（この大きさでは IndexedDB に切り替わらず、確かめていない）。送り方は「そのまま」の 1 通りだけ
//   （3 通りは input-field-xss.test.mjs が確かめる）。Chromium のみ。通知の最終の文面は別の TASK が決める。
// 書き込み先は mkdtemp の一時ディレクトリだけ。ペイロードからパス・セレクタ・コードを作らない（保存先の鍵は列挙して見つけ、組み立て直さない）。
// 失敗の文には件の ID・経路・CSP・理由コードだけを出し、ペイロードと履歴の中身は出さない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, DANGER_TAGS,
} from './harness.mjs';
import { pageRequestProblems } from './delivery.mjs';
import { irFetchCheck } from './l1-l2-runner.mjs';
import { DRAFT, LOG } from './ch0-3-journey.mjs';
import { exportByUi } from './shell-journey-trips.mjs';
import { withDeliveryCopy } from './ch4-9-journey.mjs';
import { e4Payloads } from './xss-cases.mjs';
import { loadBaseIr, withStarts } from './e2-ir.mjs';
import {
  START, payloadProbes, observeLeak, leakedPlaces, savedPayloadOps, judgeRegularLine, observeRegular,
} from './e4-history.mjs';

const BASE = '/fandhe-history-regular/';
const PACK_ID = 'safe4-regular';
const TITLE = '[data-testid="step-title"]';
const ITEMS = `${LOG} > li`;
const RECEIVED = `${ITEMS}[data-kind="received"]`;
const NOTICE_ITEM = '[data-testid="shell-notice-item"]';
const IMPORT_INPUT = '[data-testid="shell-import"] input[type="file"]';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
const SENTINEL = 'SAFE4-REGULAR 文字';
const WAYS = ['restore', 'file'];
const T = 30000;
const FORBIDDEN = [
  'a', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'pre', 'code', 'strong', 'em', 'b', 'i', 'blockquote', 'ul', 'ol', 'table', 'hr',
  'details', ...DANGER_TAGS,
].join(', ');
const SELECTORS = {
  items: ITEMS, received: RECEIVED, body: '[data-testid="line-body"]', forbidden: FORBIDDEN, title: TITLE,
  secret: '[data-testid="secret-notice"]', choice: '[data-testid="choice-btn"]', draft: DRAFT,
};

let browser;
let cases;
let tmp;
const passed = new Map(); // `${csp}/${way}` -> 合格した件数（C が集計する）

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  cases = e4Payloads();
  assert.equal(cases.length, 14);
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-history-regular-'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
  if (tmp) fs.rmSync(tmp, { recursive: true, force: true });
});

function cspMeta(dir) {
  const found = fs.readFileSync(path.join(dir, 'index.html'), 'utf8').match(/<meta\b[^>]*\bhttp-equiv\s*=\s*["']?content-security-policy["']?[^>]*>/gi) ?? [];
  assert.equal(found.length, 1, `${dir}: exactly one CSP meta`);
  return found[0];
}

// 教材 ID を固定の値にした開始点つきの IR（1/2 や他の試験の保存先の鍵と分ける）。
function packIr() {
  const ir = withStarts(loadBaseIr());
  ir.pack = { ...ir.pack, id: PACK_ID };
  return ir;
}

function withDelivery(csp, fn) {
  return withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, async (dir) => {
    fs.writeFileSync(path.join(dir, 'ir', 'main.json'), JSON.stringify(packIr()), 'utf8');
    return withServer({ dir, base: BASE }, fn);
  }));
}

async function openStart(page, srv) {
  await page.goto(`${srv.url}?start=${encodeURIComponent(START)}`);
  await waitMounted(page);
  assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'no test hooks');
  await page.waitForSelector(TITLE, { timeout: T });
}

// 履歴の鍵（接頭辞が合い、値が履歴の包みとして読めるもの）と値を列挙する。
const historyEntries = (page) => page.evaluate(([prefix]) => {
  const out = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    if (!k.startsWith(prefix)) continue;
    const v = localStorage.getItem(k) ?? '';
    try {
      if (JSON.parse(v).format === 'fandhe-workshop-history') out.push({ key: k, value: v });
    } catch { /* 履歴でない値は数えない */ }
  }
  return out;
}, [HISTORY_PREFIX]);

const observe = (page) => page.evaluate(observeRegular, SELECTORS);
const short = (e) => String(e?.message ?? e).replaceAll(/\s+/g, ' ').slice(0, 300);

const readNotices = (page) => page.$$eval(NOTICE_ITEM, (els) => els.map((e) => e.getAttribute('data-notice-kind')));

// 入力欄に入れて Enter で送る。受付の行が 1 つ増え、続く行が出揃って行の総数が落ち着くまで待つ（固定の sleep は使わない）。
async function sendOnce(page, value, before) {
  await page.fill(DRAFT, value);
  const sent = await page.inputValue(DRAFT);
  await page.press(DRAFT, 'Enter');
  await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [RECEIVED, before + 1], { timeout: T });
  await page.waitForFunction(([items]) => {
    const w = window;
    const n = document.querySelectorAll(items).length;
    if (w.__e4Last === n) w.__e4Same = (w.__e4Same ?? 0) + 1; else { w.__e4Last = n; w.__e4Same = 0; }
    return w.__e4Same >= 3;
  }, [ITEMS], { polling: 50, timeout: T });
  return sent;
}

function assertNetwork(rec, srv, label) {
  const chk = irFetchCheck(rec, `${srv.url}ir/main.json`);
  assert.equal(chk.irCount, 1, `${label}: ir/main.json request count`);
  assert.ok(chk.irOk, `${label}: ir/main.json status`);
  const problems = pageRequestProblems({ ...rec, failed: chk.failed }, { origin: srv.origin, base: BASE });
  assert.deepEqual(problems, [], `${label}: ${JSON.stringify(problems)}`);
}

// 復元・読み込みの後の画面を判定する。payload が null なら番兵（断片の判定は文字として出ることだけを見る）。
async function assertRendered(page, rec, srv, label, { P, sent, before }) {
  const obs = await observe(page);
  const bad = judgeRegularLine(obs, sent);
  assert.deepEqual(bad, [], `${label}: ${bad.join(',')}`);
  assert.equal(obs.received, before.received, `${label}: received rows ${obs.received}/${before.received}`);
  assert.deepEqual(await readNotices(page), [], `${label}: notices`);
  const leak = leakedPlaces(await page.evaluate(observeLeak), payloadProbes(P));
  assert.ok(leak.includes('text') && leak.includes('textContent'), `${label}: payload is not visible as text (${leak.join(',')})`);
  assert.ok(!leak.includes('attrs'), `${label}: payload reached an attribute value`);
  const consoleText = { html: '', text: '', textContent: '', attrs: rec.consoleAll, values: [] };
  assert.deepEqual(leakedPlaces(consoleText, payloadProbes(P)), [], `${label}: payload fragment in console`);
  assert.equal(await page.evaluate(() => Object.prototype.polluted === undefined && ({}).polluted === undefined), true, `${label}: prototype polluted`);
  const counts = await collectSafety(page, rec);
  assert.deepEqual(nonZero(counts), [], `${label}: ${JSON.stringify(counts)}`);
  assert.deepEqual(rec.hits, [], `${label}: executed`);
  assertNetwork(rec, srv, label);
  return obs;
}

// 1 件を流す。送って保存・書き出しまで行い、経路 restore（再読み込み）の後と経路 file（新しい context への読み込み）の後を判定する。
// 返す値は way ごとの成否（失敗は例外で、呼び出し側が件・経路・CSP つきの文にまとめる）。
async function runWay(srv, P, way, label, { checkDraft = true } = {}) {
  const first = await openPage(browser, { origin: srv.origin });
  let text;
  let sent;
  let before;
  try {
    const { page, rec } = first;
    await openStart(page, srv);
    assert.equal((await observe(page)).received, 0, `${label}: starts fresh`);
    sent = await sendOnce(page, P, 0);
    await page.fill(DRAFT, P);
    // 自動保存が着地するまで待つ（上限 15 秒）。値の比較だけを行う。
    let entries = [];
    let saved = { submit: 0, draft: 0 };
    for (let i = 0; i < 60 && !(saved.submit >= 1 && saved.draft >= 1); i += 1) {
      entries = await historyEntries(page);
      if (entries.length === 1) saved = savedPayloadOps(entries[0].value, P);
      if (!(saved.submit >= 1 && saved.draft >= 1)) await page.waitForTimeout(250);
    }
    assert.equal(entries.length, 1, `${label}: exactly one history key`);
    assert.ok(saved.submit >= 1, `${label}: stored submit op`);
    assert.ok(saved.draft >= 1, `${label}: stored draft op`);
    const file = await exportByUi(page, tmp);
    text = fs.readFileSync(file, 'utf8');
    fs.rmSync(file, { force: true });
    const exported = savedPayloadOps(text, P);
    assert.ok(exported.submit >= 1 && exported.draft >= 1, `${label}: exported ops`);
    before = await observe(page);
    assert.equal(before.draft, P, `${label}: draft before`);
    assert.deepEqual(judgeRegularLine(before, sent), [], `${label}: before`);
    assertNetwork(rec, srv, `${label}: before`);
    if (way === 'restore') {
      // 再読み込みの分だけを数え直す（記録のうち通信の計数だけを空にする）。
      rec.requests.length = 0;
      rec.finished.length = 0;
      rec.finishedIds.length = 0;
      rec.failed.length = 0;
      await page.reload();
      await waitMounted(page);
      await page.waitForFunction((sel) => document.querySelector(sel)?.dataset.kind === 'reloaded', `${ITEMS}:last-child`, { timeout: T });
      const obs = await assertRendered(page, rec, srv, `${label}: restored`, { P, sent, before });
      assert.equal(obs.total, before.total + 1, `${label}: one reloaded row is added`);
      if (checkDraft) assert.equal(obs.draft, P, `${label}: draft after reload`);
    }
  } finally {
    await first.context.close();
  }
  if (way === 'file') {
    const second = await openPage(browser, { origin: srv.origin });
    try {
      const { page, rec } = second;
      await openStart(page, srv);
      assert.equal((await observe(page)).received, 0, `${label}: fresh before import`);
      await page.setInputFiles(IMPORT_INPUT, { name: 'history.json', mimeType: 'application/json', buffer: Buffer.from(text, 'utf8') });
      await page.waitForFunction((sel) => document.querySelectorAll(sel).length >= 1, RECEIVED, { timeout: T });
      await page.waitForTimeout(300);
      await assertRendered(page, rec, srv, `${label}: imported`, { P, sent, before });
    } finally {
      await second.context.close();
    }
  }
}

test('A: 前提（E4 が 14 件でどれも `/` で始まらず kebab-case に合わない・CSP の meta が両方の設定で同一・番兵が正規の経路を通る）', { timeout: 300000 }, async () => {
  assert.equal(cases.length, 14);
  for (const c of cases) {
    assert.ok(!c.payload.startsWith('/'), `${c.id}: starts with /`);
    assert.ok(!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(c.payload), `${c.id}: matches kebab-case`);
  }
  assert.equal(cspMeta(process.env.FANDHE_WEB_TEST_DIR), cspMeta(process.env.FANDHE_WEB_DIR));
  await withDelivery(true, async (srv) => {
    await runWay(srv, SENTINEL, 'restore', 'sentinel restore');
  });
});

for (const csp of [true, false]) {
  test(`B: E4 の 14 件が入力欄から送られ、保存・復元・読み込みの後も文字として出て、実行・通信・危険な要素が 0 件（CSP ${csp ? 'あり' : 'なし'}）`, { timeout: 900000 }, async (t) => {
    const failures = [];
    const ok = { restore: 0, file: 0 };
    await withDelivery(csp, async (srv) => {
      // B0（対照）: 無害な番兵が同じ経路・同じ判定に通る。
      for (const way of WAYS) await runWay(srv, SENTINEL, way, `sentinel ${way} csp=${csp ? 'on' : 'off'}`);
      for (const c of cases) {
        for (const way of WAYS) {
          const label = `${c.id} ${way} csp=${csp ? 'on' : 'off'}`;
          try {
            await runWay(srv, c.payload, way, label);
            ok[way] += 1;
          } catch (e) {
            failures.push(`${label}: ${short(e)}`);
          }
        }
      }
    });
    t.diagnostic(`csp=${csp}: ${JSON.stringify(ok)}/${cases.length}`);
    for (const way of WAYS) passed.set(`${csp}/${way}`, ok[way]);
    assert.deepEqual(failures, []);
  });
}

test('C: 合格が CSP あり・なしの経路ごとに 14 件中 14 件', () => {
  for (const csp of [true, false]) {
    for (const way of WAYS) {
      assert.equal(passed.get(`${csp}/${way}`) ?? -1, 14, `csp=${csp ? 'on' : 'off'} ${way}: ${passed.get(`${csp}/${way}`)}/14 (run test B first)`);
    }
  }
});

test('D1: 対照。復元した後の行に生の HTML を入れると、判定が不合格になり計数が 0 でなくなる', { timeout: 300000 }, async () => {
  const byId = new Map(cases.map((c) => [c.id, c.payload]));
  await withDelivery(true, async (srv) => {
    for (const id of ['XSS-01', 'XSS-07', 'XSS-57']) {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        await openStart(page, srv);
        const sent = await sendOnce(page, SENTINEL, 0);
        // 自動保存が着地するまで待つ（上限 15 秒）。保存前の再読み込みは新しい開始になる。
        let saved = { submit: 0, draft: 0 };
        for (let i = 0; i < 60 && saved.submit < 1; i += 1) {
          const entries = await historyEntries(page);
          if (entries.length === 1) saved = savedPayloadOps(entries[0].value, SENTINEL);
          if (saved.submit < 1) await page.waitForTimeout(250);
        }
        assert.ok(saved.submit >= 1, `${id}: stored submit op before reload`);
        await page.reload();
        await waitMounted(page);
        await page.waitForSelector(RECEIVED, { timeout: T });
        assert.deepEqual(judgeRegularLine(await observe(page), sent), [], `${id}: control precondition`);
        await page.evaluate(([sel, body, html]) => {
          const rows = document.querySelectorAll(sel);
          rows[rows.length - 1].querySelector(body).innerHTML = html;
        }, [RECEIVED, SELECTORS.body, byId.get(id)]);
        assert.notDeepEqual(judgeRegularLine(await observe(page), sent), [], `${id}: control should fail the judge`);
        assert.notDeepEqual(nonZero(await collectSafety(page, rec)), [], `${id}: control should be counted`);
      } finally {
        await context.close();
      }
    }
  });
});

test('D2: 対照。保存先が空の新しい context では受付の行が 0 件で、判定が不合格になる', { timeout: 300000 }, async () => {
  await withDelivery(true, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await openStart(page, srv);
      const obs = await observe(page);
      assert.equal(obs.received, 0);
      assert.deepEqual(judgeRegularLine(obs, SENTINEL), ['no-received-line']);
    } finally {
      await context.close();
    }
  });
});
