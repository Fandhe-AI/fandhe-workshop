// 表示だけで開いた履歴の、保存領域と書き出しが変わらないことのブラウザでの確認（SAVE-25。TASK-335）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。履歴と開く側の教材は version-histories と同じ
// （FANDHE_VERSION_HISTORIES_DIR。表は testdata/version-histories/expectations.json を直接読む）。
// 状態・ログの不変は view-only.test.mjs（TASK-176）が持つ。ここは保存の側（保存先の値・書き出したファイル）だけを見る。
//
// 確かめること（表示だけの履歴に、入力・クリック・保存・停止の 4 種の操作を送った後）:
//   (A) 起動時の復元（保存先に主版違いの履歴を置いて再読み込み）: localStorage（CSP あり・なし）と IndexedDB（CSP あり）で、
//       保存の待ちを過ぎても保存の試行・履歴の鍵への書き込みが 0 件で、保存先の値が置いた値のまま。書き出したファイルが
//       置いた値とバイト単位で同一。`pagehide` を通す再読み込みの後も同じ
//   (B) 読み込み（表の view_only の全行。教材の内容・語彙・エンジンの主版違いを含む。PoC-6 の S-14・S-15 を含む）: 保存先の全体の
//       digest の一覧が読み込みの前後と再読み込みの後で同じ。書き出したファイルが読み込んだファイルとバイト単位で同一
//   (C) 陽性の対照: 開ける履歴では同じ操作で保存先の値が置いた値から変わり、書き出しも置いた値と異なる（比較が変化を検出できる）
//   全場面で、外部への要求・CSP 違反・ページのエラー・dialog・popup が 0 件
// 書き出しは `exportPrepareLive`（受講中のホストの `export_live` を直列化し直さずに blob: にする試験用の口）と実際のダウンロードで得る。
// 限界: Chromium だけで確かめる。IndexedDB への切り替え後の `pagehide` の書き込みは確定を保証しない（SAVE-9）。本試験は「書かない」ことを見る。
// 失敗の文には ID・件数・真偽だけを出す（履歴の中身を出さない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-view-only-storage/';
const FILE_INPUT = '[data-testid="history-file"]';
const RAW = '[data-testid="raw-trigger"]';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const OPERATION = '/make-note second-note';
const PACK = 'min-3step';
const START = 's-1';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
// src/storage_select.rs の SWITCH_THRESHOLD_UTF16（5,242,877 の 80%）。
const THRESHOLD = 4_194_301;
const PAD_KEY = 'unrelated-pad';
const TABLE_PATH = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'testdata', 'version-histories', 'expectations.json');

let browser;
let historiesDir;
let table;

before(async () => {
  for (const name of ['FANDHE_WEB_DIR', 'FANDHE_WEB_TEST_DIR', 'FANDHE_VERSION_HISTORIES_DIR']) {
    const dir = process.env[name];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a directory`);
  }
  historiesDir = process.env.FANDHE_VERSION_HISTORIES_DIR;
  table = JSON.parse(fs.readFileSync(TABLE_PATH, 'utf8'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 履歴の鍵への書き込みの記録器（長さだけを持つ）と、IndexedDB の経路を選ばせる埋め草（secret-hold と同じ方式）。
function installInit([prefix, padKey, padLen, pad]) {
  window.__saveWrites = [];
  const mine = (key) => typeof key === 'string' && key.startsWith(prefix);
  const origSet = Storage.prototype.setItem;
  Storage.prototype.setItem = function setItem(key, value) {
    if (mine(key)) window.__saveWrites.push({ kind: 'local', len: String(value).length });
    return origSet.call(this, key, value);
  };
  const origPut = IDBObjectStore.prototype.put;
  IDBObjectStore.prototype.put = function put(value, key) {
    if (mine(key)) window.__saveWrites.push({ kind: 'idb', len: typeof value === 'string' ? value.length : -1 });
    return origPut.apply(this, arguments);
  };
  if (pad) {
    try {
      if (localStorage.getItem(padKey) === null) localStorage.setItem(padKey, 'x'.repeat(padLen));
    } catch { /* 容量の都合で書けない場合は、そのまま続ける */ }
  }
}

async function open(srv, { pad = false } = {}) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.context.addInitScript(installInit, [HISTORY_PREFIX, PAD_KEY, THRESHOLD - 100 - PAD_KEY.length, pad]);
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);
const engine = async (page) => JSON.parse(await call(page, 'stateJson')).engine;
const autosave = async (page) => JSON.parse(await call(page, 'autosaveState'));
const writes = (page) => page.evaluate(() => window.__saveWrites.length);

async function settle(page) {
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null', null, { timeout: 15000 });
}

function assertQuiet(rec, label) {
  const s = summarize(rec);
  assert.equal(s.external, 0, `${label}: no external request`);
  assert.equal(s.csp, 0, `${label}: no CSP violation`);
  assert.equal(s.pageErrors, 0, `${label}: no page error`);
  assert.equal(s.dialogs, 0, `${label}: no dialog`);
  assert.equal(s.popups, 0, `${label}: no popup`);
}

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

// 入力・クリック・保存・停止の 4 種の操作を送る（view-only.test.mjs の assertFrozen と同じ並び）。
async function sendFourOps(page) {
  const step = async (fn) => { await fn(); await settle(page); };
  await step(async () => { await page.locator(INPUT).fill(OPERATION); });
  await step(async () => { await page.locator(INPUT).press('Enter'); });
  await step(async () => {
    await page.locator(INPUT).fill(OPERATION);
    await page.locator(SUBMIT).click({ force: true });
  });
  await step(async () => { await page.locator(INPUT).press('Escape'); });
  await step(async () => { await sendRaw(page, { control: 'save' }); });
  await step(async () => { await sendRaw(page, { control: 'stop' }); });
}

// 遅延保存が動くはずの時間（max_wait）を過ぎるまで待つ。「書かない」ことの確認は経過時間が要るため、固定の待ちを使う。
async function waitPastAutosave(page) {
  await page.waitForFunction(() => JSON.parse(window.__fandheTest.autosaveState()).scheduled === false, null, { timeout: 15000 });
  const a = await autosave(page);
  await page.waitForTimeout(a.max_wait_ms + a.debounce_ms + 300);
}

// 保存先の値の digest（値そのものは返さない）。
const hashOf = (v) => {
  let h = 0;
  for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
  return { len: v.length, h };
};

// 履歴の鍵を持つ localStorage と IndexedDB の全件の digest の一覧（JSON 文字列。比較にだけ使う）。
function storageDigests(page) {
  return page.evaluate(async (prefix) => {
    const hash = (v) => {
      let h = 0;
      for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
      return { len: v.length, h };
    };
    const local = [];
    for (let i = 0; i < localStorage.length; i += 1) {
      const k = localStorage.key(i);
      if (k.startsWith(prefix)) local.push([k, hash(localStorage.getItem(k))]);
    }
    local.sort((a, b) => (a[0] < b[0] ? -1 : 1));
    const idb = await new Promise((resolve, reject) => {
      const req = indexedDB.open('fandhe-workshop', 1);
      req.onerror = () => reject(new Error('open failed'));
      req.onupgradeneeded = () => { req.transaction.abort(); resolve([]); };
      req.onsuccess = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains('history')) { db.close(); resolve([]); return; }
        const store = db.transaction('history').objectStore('history');
        const keys = store.getAllKeys();
        const vals = store.getAll();
        vals.onsuccess = () => {
          db.close();
          const out = keys.result.map((k, i) => [String(k), typeof vals.result[i] === 'string' ? hash(vals.result[i]) : null]);
          out.sort((a, b) => (a[0] < b[0] ? -1 : 1));
          resolve(out);
        };
      };
    });
    return JSON.stringify({ local, idb });
  }, HISTORY_PREFIX);
}

// 受講中のホストの書き出しを実際にダウンロードして、保存されたバイト列を返す。
async function downloadLive(page, label) {
  assert.equal((await engine(page)).exportGate, 'ready', `${label}: export gate is ready`);
  const prepared = JSON.parse(await call(page, 'exportPrepareLive'));
  assert.equal(prepared.ok, true, `${label}: live export prepared`);
  assert.equal(prepared.gate, 'ready', `${label}: gate ready`);
  assert.match(prepared.url, /^blob:/);
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    call(page, 'exportClick', prepared.url, prepared.name),
  ]);
  const filePath = await download.path();
  assert.ok(filePath, `${label}: download saved`);
  const bytes = fs.readFileSync(filePath);
  assert.equal(bytes.length, prepared.bytes, `${label}: downloaded length matches`);
  return bytes;
}

async function writeStored(page, text) {
  await call(page, 'storageOpen');
  await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [PACK, START], { timeout: 10000 });
  assert.equal(JSON.parse(await call(page, 'storageWrite', PACK, START, text)).ok, true, 'stored history written');
}

async function readStored(page) {
  await call(page, 'storageOpen');
  await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [PACK, START], { timeout: 10000 });
  return call(page, 'storageRead', PACK, START);
}

// 開ける包みの content の主版だけを上げた履歴（結果は view_only の content_major）。
async function majorBumpedText(page) {
  const current = JSON.parse(await call(page, 'loadableHistoryText'));
  const parts = current.versions.content.split('.').map(Number);
  parts[0] += 1;
  return JSON.stringify({ ...current, versions: { ...current.versions, content: parts.join('.') } });
}

// (A) 起動時の復元で表示だけになった履歴に 4 種の操作を送り、保存先と書き出しを確かめる。
async function runBootCase({ csp, idb }) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const opened = await open(srv, { pad: idb });
    const { page } = opened;
    await settle(page);
    const storedText = await majorBumpedText(page);
    const expected = Buffer.from(storedText, 'utf8');
    await writeStored(page, storedText);
    if (idb) {
      assert.equal(JSON.parse(await call(page, 'storageState', PACK, START)).kindFor, 'idb', 'the IndexedDB path is selected');
      // IndexedDB の書き込みは非同期のため、値が届いてから再読み込みする。
      for (let i = 0; i < 100 && JSON.parse(await storageDigests(page)).idb.length === 0; i += 1) await page.waitForTimeout(50);
      assert.equal(JSON.parse(await storageDigests(page)).idb.length, 1, 'the stored value reached IndexedDB');
    }

    await page.reload();
    await waitMounted(page);
    await settle(page);
    const e0 = await engine(page);
    assert.equal(e0.bootKind, 'not_opened', 'boot did not open the history');
    assert.equal(e0.viewOnly, 'content_major', 'view-only by the major content version');
    const savesBefore = (await autosave(page)).saves;
    assert.equal(await writes(page), 0, 'no write before the operations');

    await sendFourOps(page);
    await waitPastAutosave(page);
    assert.equal((await autosave(page)).saves, savesBefore, 'no save attempt');
    assert.equal(await call(page, 'historySaveLen'), 0, 'nothing to save');
    assert.equal(await writes(page), 0, 'no write to the storage');
    assert.equal((await readStored(page)) === storedText, true, 'the stored value is untouched');
    const bytes = await downloadLive(page, 'boot');
    assert.equal(bytes.equals(expected), true, 'the exported file equals the stored history');

    // pagehide を通す再読み込みの後も、保存先は置いた値のままで、再び表示だけで開き、書き出しも同一である。
    await page.reload();
    await waitMounted(page);
    await settle(page);
    assert.equal((await engine(page)).viewOnly, 'content_major', 'still view-only after the reload');
    assert.equal((await readStored(page)) === storedText, true, 'the stored value is untouched after the reload');
    assert.equal((await downloadLive(page, 'boot-reload')).equals(expected), true, 'the export is the same after the reload');
    assert.equal(await writes(page), 0, 'no write after the reload');
    assertQuiet(opened.rec, 'boot');
    await opened.context.close();
  }));
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  test(`起動時の復元で表示だけの履歴は、操作の後も保存先が変わらず書き出しが同一（localStorage。CSP ${label}）`, async () => {
    await runBootCase({ csp, idb: false });
  });
}

test('起動時の復元で表示だけの履歴は、操作の後も保存先が変わらず書き出しが同一（IndexedDB。CSP あり）', async () => {
  await runBootCase({ csp: true, idb: true });
});

// (B) 読み込みで表示だけになった履歴。表の view_only の全行を回す。
async function runLoadCase({ csp, row }) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const opened = await open(srv);
    const { page, context } = opened;
    await settle(page);
    const irText = fs.readFileSync(path.join(historiesDir, 'packs', `${row.opening.pack}.ir.json`), 'utf8');
    const n0 = (await engine(page)).installCount;
    assert.equal(await call(page, 'loadIr', irText), true, `${row.id}: load-ir sent`);
    await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
    assert.equal(await call(page, 'openingOverride', JSON.stringify({ engine: row.opening.engine, vocab_max: row.opening.vocab_max })), true, `${row.id}: override sent`);
    await settle(page);
    const digestsBefore = await storageDigests(page);

    const file = path.join(historiesDir, `${row.id}.history.json`);
    const expected = fs.readFileSync(file);
    await call(page, 'importInstall');
    await page.setInputFiles(FILE_INPUT, file);
    await page.waitForFunction(() => {
      const e = JSON.parse(window.__fandheTest.stateJson()).engine;
      return e && e.lastImport != null && e.viewOnly != null;
    }, null, { timeout: 15000 });
    await settle(page);
    const e = await engine(page);
    assert.equal(e.lastImport.verdict, 'view_only', `${row.id}: view-only`);
    const savesBefore = (await autosave(page)).saves;

    await sendFourOps(page);
    await waitPastAutosave(page);
    assert.equal((await autosave(page)).saves, savesBefore, `${row.id}: no save attempt`);
    assert.equal(await call(page, 'historySaveLen'), 0, `${row.id}: nothing to save`);
    assert.equal(await writes(page), 0, `${row.id}: no write to the storage`);
    assert.equal(await storageDigests(page) === digestsBefore, true, `${row.id}: the storage is unchanged`);
    const bytes = await downloadLive(page, row.id);
    assert.equal(bytes.equals(expected), true, `${row.id}: the exported file equals the loaded file`);

    // pagehide を通して離れ、同じオリジンを開き直しても保存先は変わらない（操作はしない）。
    await page.goto('about:blank');
    await page.goto(srv.url);
    await waitMounted(page);
    await settle(page);
    assert.equal(await storageDigests(page) === digestsBefore, true, `${row.id}: the storage is unchanged after leaving`);
    assertQuiet(opened.rec, row.id);
    await context.close();
  }));
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  test(`読み込みで表示だけになった履歴は、操作の後も保存先が変わらず書き出しが読み込んだファイルと同一（CSP ${label}）`, async () => {
    const rows = table.rows.filter((r) => r.expect.result === 'view_only');
    assert.ok(rows.length >= 2, 'the table has view-only rows');
    for (const row of rows) {
      await runLoadCase({ csp, row });
    }
  });
}

// (C) 陽性の対照: 開ける履歴では同じ操作で保存先の値と書き出しが置いた値から変わる。
test('対照: 開ける履歴では同じ操作で保存先の値と書き出しが変わる', async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const opened = await open(srv);
    const { page } = opened;
    await settle(page);
    const storedText = await call(page, 'loadableHistoryText');
    assert.ok(storedText.length > 0, 'loadable history exists');
    await writeStored(page, storedText);
    await page.reload();
    await waitMounted(page);
    await settle(page);
    const e0 = await engine(page);
    assert.equal(e0.viewOnly, null, 'not view-only');
    const before = hashOf(storedText);

    await page.locator(INPUT).fill(OPERATION);
    await page.locator(SUBMIT).click();
    await settle(page);
    await waitPastAutosave(page);
    assert.equal(await writes(page) >= 1, true, 'the history was written');
    const after = await readStored(page);
    const hAfter = hashOf(after);
    assert.equal(hAfter.len === before.len && hAfter.h === before.h, false, 'the stored value changed');
    const bytes = await downloadLive(page, 'control');
    assert.equal(bytes.equals(Buffer.from(storedText, 'utf8')), false, 'the export differs from the stored text');
    assertQuiet(opened.rec, 'control');
    await opened.context.close();
  }));
});
