// 履歴ファイルの文字列の欄を注入用の文字列で置き換えた履歴が、読み込みで断られ、何も実行されず、断る文面に履歴の中の文字列が出ない
// ことの試験（SAFE-4。TASK-146 1/2。配信する設定・CSP あり・なし）。
//
// データの正は testdata の xss-cases.md の「入り口に E4 を含む」14 件（xss-cases.mjs の e4Payloads が読む。ペイロードをここへ書き写さない）。
// 履歴の変種の作り方・画面に出ていないことの判定の断片・画面を読む関数は e4-history.mjs（単体試験は e4-history.test.mjs）。
// 方法は PoC-7 の E4 と同じ: 実操作の書き出しを基準に、`ops`（submit・draft）・`snapshot.log`・メタ（start・pack_id・versions）の
// 3 変種 × 読み込みの 2 通り（ファイル選択・保存先からの起動時の復元）× 14 件 = CSP ごとに 84 回。各回は新しい context で開く。
//
// 確かめること（毎回）: 断りの通知がちょうど 1 件で、種類が経路と合い、理由が SAFE-4 の 3 種（replay_mismatch・other_pack・malformed）の
//   どれかで、実行・通信・CSP 違反・危険な要素・ページのエラーが 0 件、`Object.prototype` が汚れず、置き換えた文字列の断片が
//   HTML・文字・全属性の値・入力欄の値・コンソールのどこにも出ない。復元の経路では保存先の値が置いた値のままで、新しく始まる。
//   ファイル選択の経路では Step の題が読み込みの前と同じ。集計（C）で、(経路, 種類, 理由) ごとに通知の文が全件で完全に一致する
//   （固定の文面）。
// 対照: B0（無害な番兵で同じ理由・同じ文面になる）、D0（置き換えない履歴は 2 経路とも開く。断りの空振りと読まれていないことの見分け）、
//   D1（実ページに生の HTML を入れると計数が 0 でなく数える）、D2（14 件を文字として画面に入れると判定が「出ている」になる）。
//
// 入力: FANDHE_WEB_DIR（配信設定）・FANDHE_WEB_TEST_DIR（CSP の meta の同一性の確認だけに使う）。未設定は skip にせず失敗にする。
// 限界: 保存先は localStorage だけ（この大きさでは IndexedDB に切り替わらず、確かめていない）。snapshot は log に限る。
//   正規の経路（入力して保存し、復元すると文字として出ること）は TASK-146 2/2 が受け持つ。通知の最終の文面は別の TASK が決める。Chromium のみ。
// 書き込み先は mkdtemp の一時ディレクトリだけ。ペイロードからパス・セレクタ・コードを作らない（保存先の鍵は列挙して見つけ、組み立て直さない）。
// 失敗の文には件の ID・変種・経路・CSP・理由コードだけを出し、ペイロードと履歴の中身は出さない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { pageRequestProblems } from './delivery.mjs';
import { irFetchCheck } from './l1-l2-runner.mjs';
import { DRAFT, LOG, mouseDriver, waitCount } from './ch0-3-journey.mjs';
import { exportByUi } from './shell-journey-trips.mjs';
import { withDeliveryCopy } from './ch4-9-journey.mjs';
import { e4Payloads } from './xss-cases.mjs';
import { loadBaseIr, withStarts } from './e2-ir.mjs';
import {
  PACK_ID, START, KINDS, WAYS, ALLOWED_REASONS, historyVariant, payloadProbes, observeLeak, leakedPlaces,
} from './e4-history.mjs';

const BASE = '/fandhe-history-xss/';
const TITLE = '[data-testid="step-title"]';
const NOTICE_ITEM = '[data-testid="shell-notice-item"]';
const IMPORT_INPUT = '[data-testid="shell-import"] input[type="file"]';
const REFUSED_KINDS = { file: ['import-refused'], restore: ['corrupt-history', 'import-refused'] };
const HISTORY_PREFIX = 'fandhe-workshop/history/';
const DRAFT_SENTINEL = 'SAFE4-BASE-DRAFT 下書き';
const SENTINEL = 'SAFE4-SENTINEL';
const T = 30000;

let browser;
let cases;
let tmp;
const passed = new Map(); // `${csp}/${kind}` -> 合格した回数（C が集計する）
const texts = new Map(); // `${way}/${noticeKind}/${reason}` -> 通知の文の集合（C が固定の文面を確かめる）

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
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-history-xss-'));
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

// 教材 ID を固定の値にした開始点つきの IR（他の試験の保存先の鍵と分ける）。
function packIr() {
  const ir = withStarts(loadBaseIr());
  ir.pack = { ...ir.pack, id: PACK_ID };
  return ir;
}

// 配信する設定の写しに IR を置き、CSP あり・なしの設定にして、サーバを立てて fn に渡す。
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
}

// 履歴の鍵（接頭辞が合い、値が履歴の包みとして読めるもの）を列挙する。
const historyKeys = (page) => page.evaluate(([prefix]) => {
  const out = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    if (!k.startsWith(prefix)) continue;
    try {
      if (JSON.parse(localStorage.getItem(k) ?? '').format === 'fandhe-workshop-history') out.push(k);
    } catch { /* 履歴でない値は数えない */ }
  }
  return out;
}, [HISTORY_PREFIX]);

const logRows = (page) => page.locator(`${LOG} li`).count();

// 基準の履歴（実操作の書き出し）と、その履歴の保存先の鍵を得る。TASK-146 2/2 も再利用する。
async function makeBaseline(srv) {
  const { page, context } = await openPage(browser, { origin: srv.origin });
  try {
    await openStart(page, srv);
    const d = mouseDriver(page);
    await page.waitForSelector(TITLE, { timeout: T });
    await d.send('/zzz-unknown');
    await page.waitForFunction((sel) => document.querySelectorAll(sel).length >= 1, `${LOG} li`, { timeout: T });
    await d.send('/new-idea');
    await waitCount(page, `${LOG} li[data-kind="clarified"]`, 1);
    await page.waitForFunction((sel) => document.querySelector(sel)?.value === '', DRAFT, { timeout: T });
    await page.fill(DRAFT, DRAFT_SENTINEL);
    // 自動保存が着地するまで待つ（値が現れるまで。上限 15 秒）。
    let keys = [];
    for (let i = 0; i < 60 && keys.length === 0; i += 1) {
      keys = await historyKeys(page);
      if (keys.length === 0) await page.waitForTimeout(250);
    }
    assert.equal(keys.length, 1, 'baseline: exactly one history key');
    const file = await exportByUi(page, tmp);
    const text = fs.readFileSync(file, 'utf8');
    fs.rmSync(file, { force: true });
    const h = JSON.parse(text);
    assert.ok(h.ops.filter((o) => 'submit' in o).length >= 2, 'baseline: submit ops');
    assert.ok(h.ops.filter((o) => 'draft' in o).length >= 1, 'baseline: draft op');
    assert.ok(h.snapshot.log.length >= 1, 'baseline: log is not empty');
    const rows = await logRows(page);
    assert.ok(rows >= 1, 'baseline: log rows');
    return { text, key: keys[0], rows, title: (await page.textContent(TITLE)) ?? '' };
  } finally {
    await context.close();
  }
}

const readNotices = (page) => page.$$eval(NOTICE_ITEM, (els) => els.map((e) => ({
  kind: e.getAttribute('data-notice-kind'),
  reason: e.getAttribute('data-notice-reason'),
  text: e.textContent ?? '',
})));

const short = (e) => String(e?.message ?? e).replaceAll(/\s+/g, ' ').slice(0, 300);

// 1 回（履歴 × 経路）を新しい context で流す。履歴が開く場合（open）と断られる場合の観察を返す。
// payload は画面に出ていないことを判定する元の文字列（対照では null）。
async function runOnce(srv, base, text, way, label, { payload, expectOpen = false } = {}) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    if (way === 'restore') {
      await context.addInitScript(([k, v]) => {
        try {
          if (sessionStorage.getItem('__e4seeded') !== '1') {
            localStorage.setItem(k, v);
            sessionStorage.setItem('__e4seeded', '1');
          }
        } catch { /* 書けなければ試験側が検出する */ }
      }, [base.key, text]);
    }
    await openStart(page, srv);
    await page.waitForSelector(TITLE, { timeout: T });
    const titleBefore = (await page.textContent(TITLE)) ?? '';
    if (way === 'file') {
      await page.setInputFiles(IMPORT_INPUT, { name: 'history.json', mimeType: 'application/json', buffer: Buffer.from(text, 'utf8') });
    }
    if (expectOpen) {
      await page.waitForFunction((sel) => document.querySelectorAll(sel).length >= 1, `${LOG} li`, { timeout: T });
      await page.waitForTimeout(300);
      assert.deepEqual(await readNotices(page), [], `${label}: no notice when the history opens`);
      // 保存先からの復元は書き出しと行の数が 1 件違うことがある（製品の挙動。ここでは新しく始まっていない = 基準以上の行があることを見る）。
      assert.ok((await logRows(page)) >= base.rows, `${label}: log rows are at least the baseline`);
      return { opened: true };
    }
    const sel = REFUSED_KINDS[way].map((k) => `${NOTICE_ITEM}[data-notice-kind="${k}"]`).join(', ');
    await page.waitForSelector(sel, { timeout: T });
    await page.waitForTimeout(300);
    const notices = await readNotices(page);
    assert.equal(notices.length, 1, `${label}: exactly one notice (${notices.map((n) => n.kind).join(',')})`);
    const n = notices[0];
    assert.ok(REFUSED_KINDS[way].includes(n.kind), `${label}: notice kind ${n.kind}`);
    assert.ok(ALLOWED_REASONS.includes(n.reason), `${label}: reason ${n.reason} is outside SAFE-4`);
    assert.ok(n.text.length > 0, `${label}: notice text is not empty`);
    if (payload !== null) {
      const probes = payloadProbes(payload);
      const obs = await page.evaluate(observeLeak);
      assert.deepEqual(leakedPlaces(obs, probes), [], `${label}: payload fragment appeared`);
      const consoleText = { html: '', text: '', textContent: '', attrs: rec.consoleAll, values: [] };
      assert.deepEqual(leakedPlaces(consoleText, probes), [], `${label}: payload fragment in console`);
    }
    assert.ok(!n.text.includes(DRAFT_SENTINEL), `${label}: baseline draft appeared in the notice`);
    assert.equal(await page.evaluate(() => Object.prototype.polluted === undefined && ({}).polluted === undefined), true, `${label}: prototype polluted`);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], `${label}: ${JSON.stringify(counts)}`);
    assert.deepEqual(rec.hits, [], `${label}: executed`);
    const chk = irFetchCheck(rec, `${srv.url}ir/main.json`);
    assert.equal(chk.irCount, 1, `${label}: ir/main.json request count`);
    assert.ok(chk.irOk, `${label}: ir/main.json status`);
    const problems = pageRequestProblems({ ...rec, failed: chk.failed }, { origin: srv.origin, base: BASE });
    assert.deepEqual(problems, [], `${label}: ${JSON.stringify(problems)}`);
    assert.equal(await logRows(page), 0, `${label}: starts fresh`);
    if (way === 'restore') {
      const stored = await page.evaluate((k) => localStorage.getItem(k), base.key);
      assert.equal(stored, text, `${label}: storage changed after the refusal`);
    } else {
      assert.equal((await page.textContent(TITLE)) ?? '', titleBefore, `${label}: step changed after the refusal`);
    }
    return { opened: false, kind: n.kind, reason: n.reason, text: n.text };
  } finally {
    await context.close();
  }
}

function addText(way, r) {
  const key = `${way}/${r.kind}/${r.reason}`;
  if (!texts.has(key)) texts.set(key, new Set());
  texts.get(key).add(r.text);
}

test('A: 前提（E4 が 14 件・CSP の meta が両方の設定で同一・基準の履歴の形）', { timeout: 300000 }, async () => {
  assert.equal(cases.length, 14);
  assert.equal(cspMeta(process.env.FANDHE_WEB_TEST_DIR), cspMeta(process.env.FANDHE_WEB_DIR));
  await withDelivery(true, async (srv) => {
    const base = await makeBaseline(srv);
    assert.ok(base.key.startsWith(HISTORY_PREFIX));
    for (const kind of KINDS) {
      const { replaced } = historyVariant(base.text, 'x', kind);
      assert.ok(Object.values(replaced).every((n) => n >= 1), kind);
    }
  });
});

for (const csp of [true, false]) {
  for (const kind of KINDS) {
    test(`B: 置き換えた履歴（${kind}）が 14 件 × 2 経路で断られ、何も実行されず文面に出ない（CSP ${csp ? 'あり' : 'なし'}）`, { timeout: 600000 }, async (t) => {
      const failures = [];
      let ok = 0;
      const dist = {};
      await withDelivery(csp, async (srv) => {
        const base = await makeBaseline(srv);
        // B0（対照）: 無害な番兵で同じ変種を作り、断られて文面が集計に入る。
        for (const way of WAYS) {
          const label = `sentinel ${kind} ${way} csp=${csp ? 'on' : 'off'}`;
          const r = await runOnce(srv, base, historyVariant(base.text, SENTINEL, kind).text, way, label, { payload: null });
          addText(way, r);
        }
        // D0（対照）: 置き換えない履歴は 2 経路とも開く（断りの空振りと読まれていないことの見分け）。
        for (const way of WAYS) {
          await runOnce(srv, base, base.text, way, `open ${way} csp=${csp ? 'on' : 'off'}`, { payload: null, expectOpen: true });
        }
        for (const c of cases) {
          const { text } = historyVariant(base.text, c.payload, kind);
          for (const way of WAYS) {
            const label = `${c.id} ${kind} ${way} csp=${csp ? 'on' : 'off'}`;
            try {
              const r = await runOnce(srv, base, text, way, label, { payload: c.payload });
              addText(way, r);
              dist[`${way}/${r.reason}`] = (dist[`${way}/${r.reason}`] ?? 0) + 1;
              ok += 1;
            } catch (e) {
              failures.push(`${label}: ${short(e)}`);
            }
          }
        }
      });
      t.diagnostic(`${kind} csp=${csp}: ${ok}/${cases.length * WAYS.length} ${JSON.stringify(dist)}`);
      passed.set(`${csp}/${kind}`, ok);
      assert.deepEqual(failures, []);
      assert.equal(ok, cases.length * WAYS.length);
    });
  }
}

test('C: 合格が CSP あり・なしのそれぞれで 84 回中 84 回で、断る文面が (経路, 種類, 理由) ごとに全件で一致する', () => {
  for (const csp of [true, false]) {
    const total = KINDS.reduce((a, k) => a + (passed.get(`${csp}/${k}`) ?? -1000), 0);
    assert.equal(total, 84, `csp=${csp ? 'on' : 'off'}: ${total}/84 (run test B first)`);
  }
  assert.ok(texts.size >= 1);
  for (const [key, set] of texts) assert.equal(set.size, 1, `${key}: ${set.size} distinct notice texts`);
});

test('D1: 対照。実ページに生の HTML を入れると、数え方が 0 でなく数える', { timeout: 300000 }, async () => {
  const byId = new Map(cases.map((c) => [c.id, c.payload]));
  await withDelivery(true, async (srv) => {
    for (const id of ['XSS-01', 'XSS-07']) {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        await openStart(page, srv);
        await page.waitForSelector(TITLE, { timeout: T });
        await page.evaluate(([sel, html]) => {
          document.querySelector(sel).innerHTML = html;
        }, [TITLE, byId.get(id)]);
        const counts = await collectSafety(page, rec);
        assert.notDeepEqual(nonZero(counts), [], `${id}: control should be counted ${JSON.stringify(counts)}`);
      } finally {
        await context.close();
      }
    }
  });
});

test('D2: 対照。14 件を文字として画面に入れると、出ていないことの判定が「出ている」になる', { timeout: 300000 }, async () => {
  await withDelivery(true, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await openStart(page, srv);
      await page.waitForSelector(TITLE, { timeout: T });
      for (const c of cases) {
        await page.evaluate(([sel, text]) => {
          document.querySelector(sel).textContent = text;
        }, [TITLE, c.payload]);
        const places = leakedPlaces(await page.evaluate(observeLeak), payloadProbes(c.payload));
        assert.notDeepEqual(places, [], `${c.id}: control should be detected`);
      }
    } finally {
      await context.close();
    }
  });
});
