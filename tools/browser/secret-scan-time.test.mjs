// 秘密情報の検査を含む保存の時間の測定（TASK-166。SAFE-31・R-20・D-45）の試験。10MB に近い履歴で、変更から履歴が保存先へ
// 書き込まれる（IndexedDB の transaction の complete）までが 1 秒以内に収まることを WASM で測る。
//
// 入力: FANDHE_WEB_TEST_DIR（試験の設定。`largeHistoryText`・`secretGateLen`・`historySaveLen` を持つ）。未設定・不在は skip にせず失敗にする。
// 履歴は Rust の口（largeHistoryText）が作る合成の包みで、同じ evaluate の中で importText に渡す（10MB を CDP に通さない）。
// 送信を重ねて伸ばさないのは、送信のたびに履歴の全体を再直列化・再検査・再書き込みして準備の費用が二次になるため。
// 費用の構成は、(a) 直列化 1 回（historySaveLen）、(b) 直列化 + JSON の読み直し + 走査（secretGateLen）、(c) 書き込み、(d) 変更から完了まで。
// 検査の費用の見積りは (b) - (a)。時刻はページ側の performance.now() で取り、書き込みの記録器は時刻と長さだけを持つ。
// 暖機を 1 回捨てる（最初の保存で IndexedDB へ切り替わる）。10MB の ASCII は localStorage の閾値を超えるため、測るのは IndexedDB だけ。
// 対照（marker）は、10MB の中ほどに検出に掛かる値を 1 つ置き、関門が実際に調べていること（警告が出て書き込みが 0 件）を示す。
// 履歴の中身・鍵・値は出さない。diagnostic は大きさ・時間・件数・ブラウザの版だけ。
// 限界: 自動運転の端末は基準端末 A ではない。CPU の減速なし・Chromium だけ・合成の履歴・wasm-opt なし。記録は docs/testing.md 5.186。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize,
} from './harness.mjs';
import { median, p95 } from '../measure/browser.mjs';

const BASE = '/fandhe-secret-scan-time/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const NOTICE = '[data-testid="secret-notice"]';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
const TRIALS = 10;
const LIMIT_MS = 1000;
// 9.5MiB〜10MiB（10,485,760 B が保存の上限）。
const RANGE = { min: 9_961_472, max: 10_485_760 };
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a built web directory');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 書き込みの記録器（save-latency と同じ。固定の関数だけを渡す）。
function installRecorder(prefix) {
  window.__saveWrites = [];
  const mine = (key) => typeof key === 'string' && key.startsWith(prefix);
  const origSet = Storage.prototype.setItem;
  Storage.prototype.setItem = function setItem(key, value) {
    if (!mine(key)) return origSet.call(this, key, value);
    const rec = { kind: 'local', len: String(value).length, tIn: performance.now(), tOut: null };
    window.__saveWrites.push(rec);
    try {
      return origSet.call(this, key, value);
    } finally {
      rec.tOut = performance.now();
    }
  };
  const origPut = IDBObjectStore.prototype.put;
  IDBObjectStore.prototype.put = function put(value, key) {
    const req = origPut.apply(this, arguments);
    if (mine(key)) {
      const rec = { kind: 'idb', len: typeof value === 'string' ? value.length : -1, tIn: performance.now(), tOut: null };
      window.__saveWrites.push(rec);
      this.transaction.addEventListener('complete', () => {
        rec.tOut = performance.now();
      });
    }
    return req;
  };
}

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);
const autosave = async (page) => JSON.parse(await call(page, 'autosaveState'));
const histLen = (page) => call(page, 'historySaveLen');
const writesOf = (page) => page.evaluate(() => window.__saveWrites.map((w) => ({ ...w })));
const r = (v) => Math.round(v * 10) / 10;

async function settle(page) {
  await page.waitForFunction(() => JSON.parse(window.__fandheTest.autosaveState()).scheduled === false, null, { timeout: 15000 });
  await page.waitForFunction(() => window.__saveWrites.every((w) => w.tOut !== null), null, { timeout: 15000 });
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null', null, { timeout: 15000 });
  await page.waitForTimeout(500);
}

// 入力欄の下書きを変える（遅延保存の経路）。t0 は操作の直前に取る。
function changeDraft(page, value) {
  return page.evaluate(([sel, v]) => {
    const t0 = performance.now();
    const ta = document.querySelector(sel);
    ta.value = v;
    ta.dispatchEvent(new Event('input', { bubbles: true }));
    return t0;
  }, [INPUT, value]);
}

// 受け付けられない短い入力を送信する（即時の保存の経路）。値の設定・input・click を同じ evaluate の中で行い、t0 はその直前に取る。
function submitOne(page, value) {
  return page.evaluate(([inputSel, submitSel, v]) => {
    const t0 = performance.now();
    const ta = document.querySelector(inputSel);
    ta.value = v;
    ta.dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector(submitSel).click();
    return t0;
  }, [INPUT, SUBMIT, value]);
}

// 10MB に近い履歴を、1 回の evaluate の中で作って取り込む。取り込めなければ失敗にする。
async function openLarge(srv, kind) {
  const opened = await openPage(browser, { origin: srv.origin });
  const { page, context } = opened;
  await context.addInitScript(installRecorder, HISTORY_PREFIX);
  await page.goto(srv.url);
  await waitMounted(page);
  const code = await page.evaluate((k) => {
    const t = window.__fandheTest;
    const text = t.largeHistoryText(k);
    if (text.length === 0) return -1;
    return t.importText(text) ? 0 : -2;
  }, kind);
  assert.equal(code, 0, `large history (${kind}) was built and sent (code ${code})`);
  await page.waitForFunction(() => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    return e && e.lastImport && e.lastImport.verdict === 'open';
  }, null, { timeout: 60000 });
  await settle(page);
  await page.evaluate(() => { window.__saveWrites.length = 0; });
  return opened;
}

function playwrightVersion() {
  try {
    return JSON.parse(fs.readFileSync(path.join(ROOT, 'node_modules', 'playwright', 'package.json'), 'utf8')).version;
  } catch {
    return 'unknown';
  }
}

// 1 条件を測る。暖機 1 回 + 計測 TRIALS 回。
async function measure(srv, kind) {
  const { page, context, rec } = await openLarge(srv, kind);
  // 暖機: 最初の保存（IndexedDB への切り替えを含む）を捨てる。
  await changeDraft(page, 'warm-up');
  await settle(page);
  assert.equal((await autosave(page)).kind, 'idb', 'a 10 MB history is stored in IndexedDB');
  const bytes = await histLen(page);
  assert.ok(bytes >= RANGE.min && bytes <= RANGE.max, `history size ${bytes} B is in ${RANGE.min}..${RANGE.max}`);

  const serialize = [];
  const gate = [];
  const total = [];
  const writeOnly = [];
  const immediate = [];
  const immediateWrites = [];
  let debounce = 0;
  for (let i = 0; i < TRIALS; i += 1) {
    // (a)(b): 直列化だけと、直列化 + 読み直し + 走査を、本番と同じ関数で測る。保存先へは書かない。
    const ab = await page.evaluate(() => {
      const t = window.__fandheTest;
      const a0 = performance.now();
      t.historySaveLen();
      const a1 = performance.now();
      const g = JSON.parse(t.secretGateLen());
      const b1 = performance.now();
      return { a: a1 - a0, b: b1 - a1, g };
    });
    assert.equal(ab.g.shapes, 0, 'no secret shape in the synthetic history');
    assert.ok(ab.g.bytes >= RANGE.min, 'the gate sees the whole history');
    serialize.push(ab.a);
    gate.push(ab.b);

    // (d1): 遅延保存。変更から書き込みの完了まで。
    await settle(page);
    await page.evaluate(() => { window.__saveWrites.length = 0; });
    const before = await autosave(page);
    debounce = before.debounce_ms;
    const t0 = await changeDraft(page, `trial-${i}-${Math.random().toString(36).slice(2, 8)}`);
    await page.waitForFunction(() => window.__saveWrites.length > 0 && window.__saveWrites[0].tOut !== null, null, { timeout: 15000, polling: 5 });
    await settle(page);
    const writes = await writesOf(page);
    assert.equal(writes.length, 1, 'exactly one history write per change');
    assert.equal(writes[0].kind, 'idb');
    total.push(writes[0].tOut - t0);
    writeOnly.push(writes[0].tOut - writes[0].tIn);
    const after = await autosave(page);
    assert.ok(after.saves > before.saves, 'a real deferred save happened');
    assert.equal(after.secretHolds, before.secretHolds, 'no hold');
  }
  // (d2): 即時の保存。受け付けられない短い入力の送信から書き込みの完了まで。
  for (let i = 0; i < TRIALS; i += 1) {
    await settle(page);
    await page.evaluate(() => { window.__saveWrites.length = 0; });
    const before = await autosave(page);
    const lenBefore = await histLen(page);
    const t0 = await submitOne(page, `imm-${i}`);
    await page.waitForFunction(() => window.__saveWrites.length > 0 && window.__saveWrites.every((w) => w.tOut !== null), null, { timeout: 15000, polling: 5 });
    await settle(page);
    const writes = await writesOf(page);
    const after = await autosave(page);
    // 遅延保存（約 200 ms 後）を即時保存として測らないための確認。saves は変更後の保存の全試行を数えるため、即時の 1 回だけなら +1、遅延が続けば +2 になる。
    assert.equal(writes.length, 1, 'exactly one history write for an immediate save');
    assert.equal(writes[0].kind, 'idb');
    assert.equal(after.saves, before.saves + 1, 'exactly one save was attempted (the immediate one; no deferred save followed)');
    assert.ok((await histLen(page)) > lenBefore, 'the submission was recorded in the history');
    assert.equal(after.secretHolds, before.secretHolds, 'no hold');
    immediateWrites.push(writes.length);
    immediate.push(writes[0].tOut - t0);
  }
  const s = summarize(rec);
  assert.equal(s.external, 0, 'no external request');
  assert.equal(s.pageErrors, 0, 'no page error');
  await context.close();
  const diff = gate.map((g, i) => g - serialize[i]);
  const stat = (xs) => ({ median: r(median(xs)), p95: r(p95(xs)), max: r(Math.max(...xs)) });
  return {
    kind, trials: TRIALS, historyBytes: bytes, debounceMs: debounce,
    serializeMs: stat(serialize), gateMs: stat(gate), scanEstimateMs: stat(diff), writeOnlyMs: stat(writeOnly),
    deferredTotalMs: stat(total), deferredMinusDebounceMs: stat(total.map((t) => t - debounce)),
    immediateTotalMs: stat(immediate), immediateWriteCounts: [Math.min(...immediateWrites), Math.max(...immediateWrites)],
  };
}

for (const kind of ['plain', 'dense']) {
  test(`10MB に近い履歴（${kind}）で、変更から保存の完了までの p95 が 1 秒以内`, async (t) => {
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      const result = await measure(srv, kind);
      t.diagnostic(`[secret-scan-time] ${JSON.stringify({ ...result, browser: `chromium ${browser.version()}`, playwright: playwrightVersion() })}`);
      assert.ok(result.deferredTotalMs.p95 <= LIMIT_MS, `deferred p95 ${result.deferredTotalMs.p95} ms exceeds ${LIMIT_MS} ms`);
      assert.ok(result.immediateTotalMs.p95 <= LIMIT_MS, `immediate p95 ${result.immediateTotalMs.p95} ms exceeds ${LIMIT_MS} ms`);
    });
  });
}

test('対照: 10MB の中ほどに検出に掛かる値があると、警告が出て履歴の鍵への書き込みは 0 件', async (t) => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openLarge(srv, 'marker');
    const bytes = await histLen(page);
    assert.ok(bytes >= RANGE.min && bytes <= RANGE.max, 'marker history is in range');
    const gate = JSON.parse(await call(page, 'secretGateLen'));
    assert.ok(gate.shapes >= 1, 'the gate finds the value in the middle of the history');
    await page.evaluate(() => { window.__saveWrites.length = 0; });
    const before = await autosave(page);
    const t0 = await changeDraft(page, 'marker-trial');
    await page.waitForSelector(NOTICE, { timeout: 15000 });
    const noticeMs = await page.evaluate((t) => performance.now() - t, t0);
    await page.waitForTimeout(1500);
    const after = await autosave(page);
    assert.equal((await writesOf(page)).length, 0, 'no history write while the warning is shown');
    assert.equal(after.secretHolds, before.secretHolds + 1, 'one hold');
    const s = summarize(rec);
    assert.equal(s.external, 0, 'no external request');
    assert.equal(s.pageErrors, 0, 'no page error');
    t.diagnostic(`[secret-scan-time] ${JSON.stringify({ control: 'marker', historyBytes: bytes, noticeMs: r(noticeMs), shapes: gate.shapes })}`);
    await context.close();
  });
});
