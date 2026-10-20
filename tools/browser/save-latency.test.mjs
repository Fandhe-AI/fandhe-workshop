// 保存の遅延の測定（TASK-318。SAVE-7・PoC-6 の S-16）の試験。受講者の操作で状態が変わってから、履歴が保存先へ書き込まれる
// （localStorage は setItem の完了、IndexedDB は transaction の complete）までの時間を、{localStorage, IndexedDB} ×
// {小さい履歴 2〜9KB, 大きい履歴 85〜156KB} の 4 条件で各 10 回測り、各条件の p95（10 回では最大値）が 1,000ms 以内であることを確かめる。
//
// 入力: FANDHE_WEB_TEST_DIR（試験の設定。`debug_autosave_state`・`debug_history_save_len` を持つ）。未設定・不在は skip にせず失敗にする。
// 時刻はページ側の performance.now() で取る（Rust に時計を持ち込まない）。変更の時刻 t0 は、変更を起こす操作と同じ evaluate の中で取る。
// 書き込みの時刻は、context の初期化スクリプトが Storage.prototype.setItem・IDBObjectStore.prototype.put を包んで記録する。
// 記録するのは時刻と値の長さだけで、値・鍵の文字列は持たない（履歴は下書きを含む受講者のデータである）。
// 遅延（debounce）と最大の待ちの長さは実装の値で、期待にしない。assert するのは、4 条件の p95 が 1,000ms 以内であること、
// 書いたのが本番の遅延保存であること（saves が 1 増える）、履歴の大きさが条件の範囲に入ること、対照 2 件である。
// 各条件は暖機を 1 回捨てる（IndexedDB は最初の書き込みで localStorage から切り替わる。TASK-317 1/2）。
// 限界: 自動運転の端末は基準端末 A ではない。CPU の減速なし。CSP ありだけで流す（時間は CSP の meta に依らない）。
//       Chromium だけで測り、Firefox・Safari は測っていない。記録は docs/testing.md 5.169。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize,
} from './harness.mjs';
import { median, p95 } from '../measure/browser.mjs';

const BASE = '/fandhe-save-latency/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
// src/storage_select.rs の SWITCH_THRESHOLD_UTF16（5,242,877 の 80%）。
const THRESHOLD = 4_194_301;
const TRIALS = 10;
const LIMIT_MS = 1000;
const SMALL = { min: 2 * 1024, max: 9 * 1024 };
const LARGE = { min: 85 * 1024, max: 156 * 1024 };
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

// 書き込みの記録器。固定の関数だけを渡す（外部の文字列を評価しない）。履歴の鍵への書き込みだけを、呼び出しの順に積む。
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

// 入力欄へ値を入れて input を発火する（SetDraft。実際の画面の操作と同じ経路）。t0 は操作の直前に取る。
async function changeDraft(page, value) {
  return page.evaluate(([sel, v]) => {
    const t0 = performance.now();
    const ta = document.querySelector(sel);
    ta.value = v;
    ta.dispatchEvent(new Event('input', { bubbles: true }));
    return t0;
  }, [INPUT, value]);
}

// 受け付けられない入力を n 回送信して、履歴を伸ばす（1 回の evaluate の中。送信は click と同じ経路）。
async function submitMany(page, n, tag) {
  await page.evaluate(([inputSel, submitSel, count, label]) => {
    for (let i = 0; i < count; i += 1) {
      const ta = document.querySelector(inputSel);
      ta.value = `${label}${i}`;
      ta.dispatchEvent(new Event('input', { bubbles: true }));
      document.querySelector(submitSel).click();
    }
  }, [INPUT, SUBMIT, n, tag]);
}

// 保存が落ち着くまで待つ。待っている変更が無く、書き込みが完了していること。
async function settle(page) {
  await page.waitForFunction(() => JSON.parse(window.__fandheTest.autosaveState()).scheduled === false, null, { timeout: 5000 });
  await page.waitForFunction(() => window.__saveWrites.every((w) => w.tOut !== null), null, { timeout: 5000 });
  await page.waitForTimeout(300);
}

// 履歴の大きさ（UTF-8 のバイト数）を範囲の中まで伸ばす。範囲に入らなければ失敗にする（黙って範囲を変えない）。
async function growTo(page, range, step) {
  let n = 0;
  for (let guard = 0; guard < 400; guard += 1) {
    const len = await histLen(page);
    if (len >= range.min) {
      assert.ok(len <= range.max, `history size ${len} B overshoots ${range.max} B`);
      return len;
    }
    await submitMany(page, step, `g${n}-`);
    n += step;
  }
  assert.fail('history did not reach the target size');
}

async function openCondition(srv, { storage }) {
  const opened = await openPage(browser, { origin: srv.origin });
  const { page, context } = opened;
  await context.addInitScript(installRecorder, HISTORY_PREFIX);
  await page.goto(srv.url);
  await waitMounted(page);
  if (storage === 'idb') {
    // 閾値の手前まで無関係の鍵で埋めてから開き直す。最初の本番の保存で IndexedDB に切り替わる。
    await page.evaluate((n) => {
      localStorage.setItem('unrelated-pad', 'x'.repeat(n));
    }, THRESHOLD - 100 - 'unrelated-pad'.length);
    await page.reload();
    await waitMounted(page);
  }
  return opened;
}

// 1 条件を測る。暖機 1 回 + 計測 TRIALS 回。履歴の中身と鍵は返さない。
async function measure(srv, { storage, size }) {
  const { page, context, rec } = await openCondition(srv, { storage });
  const range = size === 'small' ? SMALL : LARGE;
  // 暖機: 最初の変更の保存（IndexedDB は切り替えを含む）を捨てる。
  await changeDraft(page, 'warm-up');
  await settle(page);
  assert.equal((await autosave(page)).kind, storage === 'idb' ? 'idb' : 'local', 'storage kind');
  const histBytes = await growTo(page, range, size === 'small' ? 1 : 25);
  await settle(page);
  const totals = [];
  const writeOnly = [];
  const lens = [];
  const bytesList = [];
  let debounce = 0;
  for (let i = 0; i < TRIALS; i += 1) {
    await settle(page);
    await page.evaluate(() => { window.__saveWrites.length = 0; });
    const before = await autosave(page);
    debounce = before.debounce_ms;
    const t0 = await changeDraft(page, `trial-${i}-${Math.random().toString(36).slice(2, 8)}`);
    await page.waitForFunction(() => window.__saveWrites.length > 0 && window.__saveWrites[0].tOut !== null, null, { timeout: 5000, polling: 5 });
    const writes = await page.evaluate(() => window.__saveWrites.map((w) => ({ ...w })));
    assert.equal(writes.length, 1, 'exactly one history write per change');
    const w = writes[0];
    assert.equal(w.kind, storage === 'idb' ? 'idb' : 'local', 'write kind');
    // 条件の大きさは UTF-8 のバイト数（historySaveLen）で縛る。w.len は書いた文字列の長さ（UTF-16 の単位。日本語の本文はバイト数より小さい）。
    const bytes = await histLen(page);
    assert.ok(bytes >= range.min && bytes <= range.max, `history size ${bytes} B is in the ${size} range`);
    bytesList.push(bytes);
    totals.push(w.tOut - t0);
    writeOnly.push(w.tOut - w.tIn);
    lens.push(w.len);
    await settle(page);
    assert.equal((await autosave(page)).saves, before.saves + 1, 'one deferred save per change');
  }
  const s = summarize(rec);
  assert.equal(s.external, 0, 'no external request');
  assert.equal(s.pageErrors, 0, 'no page error');
  await context.close();
  const rest = totals.map((t) => t - debounce);
  const r = (v) => Math.round(v * 10) / 10;
  return {
    storage, size, trials: TRIALS, histBytes: [Math.min(...bytesList), Math.max(...bytesList)], writtenUtf16Units: [Math.min(...lens), Math.max(...lens)],
    totalMs: { median: r(median(totals)), p95: r(p95(totals)), max: r(Math.max(...totals)) },
    waitPlusWriteMs: { median: r(median(rest)), p95: r(p95(rest)) },
    writeOnlyMs: { median: r(median(writeOnly)), p95: r(p95(writeOnly)), max: r(Math.max(...writeOnly)) },
    debounceMs: debounce,
  };
}

function playwrightVersion() {
  try {
    return JSON.parse(fs.readFileSync(path.join(ROOT, 'node_modules', 'playwright', 'package.json'), 'utf8')).version;
  } catch {
    return 'unknown';
  }
}

for (const storage of ['local', 'idb']) {
  for (const size of ['small', 'large']) {
    test(`変更から保存まで: ${storage} / ${size} の履歴で p95 が 1 秒以内`, async (t) => {
      await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
        const result = await measure(srv, { storage, size });
        t.diagnostic(`[save-latency] ${JSON.stringify({ ...result, browser: `chromium ${browser.version()}`, playwright: playwrightVersion() })}`);
        assert.ok(result.totalMs.p95 <= LIMIT_MS, `p95 ${result.totalMs.p95} ms exceeds ${LIMIT_MS} ms`);
      });
    });
  }
}

test('対照: 変更しなければ、待っても履歴の鍵への書き込みは 0 件', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openCondition(srv, { storage: 'local' });
    const before = await autosave(page);
    await page.waitForTimeout(1500);
    const writes = await page.evaluate(() => window.__saveWrites.length);
    assert.equal(writes, 0, 'no write without a change');
    assert.equal((await autosave(page)).saves, before.saves);
    await context.close();
  });
});

test('対照: 連続する変更は 1 回の書き込みに畳まれ、最初の変更から 1 秒以内に書かれる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openCondition(srv, { storage: 'local' });
    const t0 = await page.evaluate(async ([sel]) => {
      const start = performance.now();
      const ta = document.querySelector(sel);
      for (let i = 0; i < 5; i += 1) {
        ta.value = `burst-${i}`;
        ta.dispatchEvent(new Event('input', { bubbles: true }));
        await new Promise((resolve) => { setTimeout(resolve, 50); });
      }
      return start;
    }, [INPUT]);
    await page.waitForFunction(() => window.__saveWrites.length > 0 && window.__saveWrites[0].tOut !== null, null, { timeout: 5000, polling: 5 });
    await settle(page);
    const writes = await page.evaluate(() => window.__saveWrites.map((w) => ({ ...w })));
    assert.equal(writes.length, 1, 'five changes are folded into one write');
    assert.ok(writes[0].tOut - t0 <= LIMIT_MS, 'the folded write lands within 1 s of the first change');
    await context.close();
  });
});
