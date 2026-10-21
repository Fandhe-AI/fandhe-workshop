// 秘密情報の警告の選択の試験（SAFE-28・D-44。TASK-164 3/3）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、次を確かめる。
//   (A) 「このまま保存する」: 10 形のそれぞれを送信で入れて警告を出し、`secret-keep` を押すと、警告とボタンが消え、保存先が変わり
//       （localStorage）、`exportGate` が ready で、確認済みの件数が 1 以上になる。IndexedDB の経路と、下書きだけの経路も確かめる。
//   (B) 「この入力を残さない」（送信の後）: 10 形のそれぞれで、`secret-discard` を押すと、保存先は基準のまま・履歴の鍵への書き込みが増えない・
//       入力欄が空・画面の文字と HTML に値が無い・保存先のどこにも値が無い・警告とボタンが消える。IndexedDB の経路でも確かめる。
//   (C) 「この入力を残さない」（下書きだけ）: 同じ観察を下書きだけの経路で確かめる。
//   (D) 読み直しの後に閉じて開き直しても、保存先と画面に値が無い。
//   (E) 「このまま保存する」の後に開き直すと保存した値から復元され、次の変更で再び警告が出る（確認済みは再読み込みで失われる限界。
//       docs/known-gaps.md・docs/testing.md 5.183）。
//   (F) 対照 N-01〜N-06 では選択のボタンが出ない。
//   (G) 値の非漏洩: console・pageerror の文、assert・diagnostic に値を出さない（出すのは試験データの ID と形の ID だけ）。
// 値は `secret-values.mjs` が部品から組み立て、変数の中だけに置く。保存先・画面の走査はページの中で行い、真偽だけを返す。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP ありの設定で流す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize,
} from './harness.mjs';
import { secretShapes, controls } from './secret-values.mjs';

const BASE = '/fandhe-secret-choice/';
const PACK = 'min-3step';
const START = 's-1';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const NOTICE = '[data-testid="secret-notice"]';
const KEEP = '[data-testid="secret-keep"]';
const DISCARD = '[data-testid="secret-discard"]';
const ACCEPTED = '/make-note first-note';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
// src/storage_select.rs の SWITCH_THRESHOLD_UTF16（5,242,877 の 80%）。
const THRESHOLD = 4_194_301;
const PAD_KEY = 'unrelated-pad';

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 履歴の鍵への書き込みの記録器（長さだけを持つ）と、IndexedDB の経路を選ばせる埋め草。
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

async function open(srv, { pad }) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.context.addInitScript(installInit, [HISTORY_PREFIX, PAD_KEY, THRESHOLD - 100 - PAD_KEY.length, pad]);
  const logs = [];
  opened.page.on('console', (m) => logs.push(m.text()));
  opened.page.on('pageerror', (e) => logs.push(String(e && e.message)));
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return { ...opened, logs };
}

// 同じ context に 2 枚目のタブを開く。openPage はポップアップを閉じる監視を付けるため、2 枚目を作る前に外す。
async function reopen(context, url) {
  context.removeAllListeners('page');
  const page = await context.newPage();
  await page.goto(url);
  await waitMounted(page);
  return page;
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const keyOf = (page) => page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, START]);
const writes = (page) => page.evaluate(() => window.__saveWrites.length);
const count = (page, sel) => page.$$eval(sel, (els) => els.length);

// 保存先の値の長さと簡易の digest（値そのものは返さない）。無ければ null。
const localDigest = (page, key) => page.evaluate((k) => {
  const v = localStorage.getItem(k);
  if (v === null) return null;
  let h = 0;
  for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
  return { len: v.length, h };
}, key);

const idbDigest = (page, key) => page.evaluate((k) => new Promise((resolve, reject) => {
  const open = indexedDB.open('fandhe-workshop', 1);
  open.onerror = () => reject(new Error('open failed'));
  open.onupgradeneeded = () => { open.transaction.abort(); resolve(null); };
  open.onsuccess = () => {
    const db = open.result;
    if (!db.objectStoreNames.contains('history')) { db.close(); resolve(null); return; }
    const get = db.transaction('history').objectStore('history').get(k);
    get.onsuccess = () => {
      db.close();
      const v = get.result;
      if (typeof v !== 'string') { resolve(null); return; }
      let h = 0;
      for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
      resolve({ len: v.length, h });
    };
  };
}), key);

const digestOf = (kind, page, key) => (kind === 'idb' ? idbDigest(page, key) : localDigest(page, key));

// 保存先（localStorage の全部と IndexedDB の全件）のどれかが、与えた値のどれかを含むか。真偽だけを返す。
const storesContainAny = (page, values) => page.evaluate(async (vals) => {
  const hit = (v) => typeof v === 'string' && vals.some((x) => v.includes(x));
  for (let i = 0; i < localStorage.length; i += 1) {
    if (hit(localStorage.getItem(localStorage.key(i)))) return true;
  }
  const dbs = typeof indexedDB.databases === 'function' ? await indexedDB.databases() : [{ name: 'fandhe-workshop' }];
  for (const info of dbs) {
    if (!info.name) continue;
    const found = await new Promise((resolve) => {
      const open = indexedDB.open(info.name);
      open.onerror = () => resolve(false);
      open.onsuccess = () => {
        const db = open.result;
        const names = Array.from(db.objectStoreNames);
        if (names.length === 0) { db.close(); resolve(false); return; }
        const tx = db.transaction(names);
        let any = false;
        let pending = names.length;
        for (const n of names) {
          const req = tx.objectStore(n).getAll();
          req.onsuccess = () => {
            if (req.result.some(hit)) any = true;
            pending -= 1;
            if (pending === 0) { db.close(); resolve(any); }
          };
          req.onerror = () => { pending -= 1; if (pending === 0) { db.close(); resolve(any); } };
        }
      };
    });
    if (found) return true;
  }
  return false;
}, values);

// 画面（表示された文字・HTML・入力欄の値）のどれかが、与えた値のどれかを含むか。真偽だけを返す。
const pageContainsAny = (page, values) => page.evaluate((vals) => ({
  text: vals.some((v) => document.body.innerText.includes(v)),
  html: vals.some((v) => document.documentElement.outerHTML.includes(v)),
  field: vals.some((v) => Array.from(document.querySelectorAll('textarea,input')).some((e) => e.value.includes(v))),
}), values);

async function sendHarmless(page) {
  const before = (await engine(page)).logLen;
  await page.fill(INPUT, ACCEPTED);
  await page.click(SUBMIT);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before);
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null');
}

const setDraft = (page, value) => page.evaluate(([sel, v]) => {
  const ta = document.querySelector(sel);
  ta.value = v;
  ta.dispatchEvent(new Event('input', { bubbles: true }));
}, [INPUT, value]);

const waitWarned = (page) => page.waitForFunction((sel) => (document.querySelector(sel)?.textContent ?? '') !== '', NOTICE, { timeout: 5000 });
const waitCleared = (page) => page.waitForFunction((sel) => (document.querySelector(sel)?.textContent ?? '') === '', NOTICE, { timeout: 5000 });

// 秘密情報を送信して警告が出るまで待つ。
async function sendSecret(page, value) {
  const before = (await engine(page)).logLen;
  await setDraft(page, value);
  await page.click(SUBMIT);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before);
  await waitWarned(page);
}

// 下書きだけの経路。遅延保存の検出で警告が出るまで待つ。
async function typeSecret(page, value) {
  await setDraft(page, value);
  await waitWarned(page);
}

async function settle(page) {
  await page.waitForFunction(() => JSON.parse(window.__fandheTest.autosaveState()).scheduled === false, null, { timeout: 5000 });
  await page.waitForTimeout(400);
}

// 保存先が書かれるまで待つ（陽性の対照）。
async function waitDigest(kind, page, key) {
  for (let i = 0; i < 100; i += 1) {
    const d = await digestOf(kind, page, key);
    if (d !== null) return d;
    await page.waitForTimeout(50);
  }
  assert.fail(`${kind}: the harmless step did not reach the store`);
}

// 保存先の値が基準と変わるまで待つ。
async function waitChanged(kind, page, key, base) {
  for (let i = 0; i < 100; i += 1) {
    const d = await digestOf(kind, page, key);
    if (d !== null && JSON.stringify(d) !== JSON.stringify(base)) return d;
    await page.waitForTimeout(50);
  }
  assert.fail(`${kind}: keep did not reach the store`);
}

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
}

function assertNoLeak(id, texts, values) {
  for (const t of texts) for (const v of values) assert.ok(!t.includes(v), `${id}: value leaked`);
}

const ALL_VALUES = () => secretShapes().map((s) => s.value);

// 値が保存先に入ったことの陽性の対照。履歴は JSON なので、JSON の文字列としてのエスケープ形で探す。
const escaped = (v) => JSON.stringify(v).slice(1, -1);

test('(A) このまま保存するで 10 形が保存され、書き出しが通る（localStorage）', async (t) => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    for (const s of secretShapes()) {
      const { page, context, logs, rec } = await open(srv, { pad: false });
      const key = await keyOf(page);
      await sendHarmless(page);
      const base = await waitDigest('local', page, key);
      await settle(page);
      await sendSecret(page, s.value);
      await settle(page);
      assert.deepEqual(await digestOf('local', page, key), base, `${s.id}: stored while warned`);
      assert.equal(await count(page, KEEP), 1, `${s.id}: keep button`);
      assert.equal(await count(page, DISCARD), 1, `${s.id}: discard button`);
      await page.click(KEEP);
      await waitCleared(page);
      await waitChanged('local', page, key, base);
      await settle(page);
      const e = await engine(page);
      assert.deepEqual(e.secretWarning, [], `${s.id}: warning left`);
      assert.equal(e.exportGate, 'ready', `${s.id}: gate`);
      assert.equal(e.secretAcknowledged >= 1, true, `${s.id}: not acknowledged`);
      assert.equal(await count(page, KEEP), 0, `${s.id}: keep button left`);
      assert.equal(await count(page, DISCARD), 0, `${s.id}: discard button left`);
      assert.equal(await storesContainAny(page, [escaped(s.value)]), true, `${s.id}: value not stored`);
      assertNoLeak(s.id, [JSON.stringify({ g: e.exportGate, w: e.secretWarning }), ...logs], [s.value]);
      clean(rec);
      t.diagnostic(`[secret-choice] ${JSON.stringify({ part: 'A', id: s.id, gate: e.exportGate })}`);
      await context.close();
    }
  });
});

for (const kind of ['local', 'idb']) {
  const pad = kind === 'idb';

  test(`(A) このまま保存するで保存先に届く（${kind}・送信・下書きだけ）`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      const shape = secretShapes()[7];
      for (const via of ['submit', 'draft']) {
        const { page, context, rec } = await open(srv, { pad });
        const key = await keyOf(page);
        await sendHarmless(page);
        const base = await waitDigest(kind, page, key);
        await settle(page);
        if (kind === 'idb') assert.equal(await localDigest(page, key), null, 'the idb path is selected');
        if (via === 'submit') await sendSecret(page, shape.value);
        else await typeSecret(page, shape.value);
        await settle(page);
        assert.deepEqual(await digestOf(kind, page, key), base, `${via}: stored while warned`);
        await page.click(KEEP);
        await waitCleared(page);
        await waitChanged(kind, page, key, base);
        await settle(page);
        assert.equal((await engine(page)).exportGate, 'ready', `${via}: gate`);
        assert.equal(await storesContainAny(page, [escaped(shape.value)]), true, `${via}: value not stored`);
        clean(rec);
        await context.close();
      }
    });
  });

  // (B)(C) この入力を残さない。via は警告の出し方（submit: 送信して / draft: 下書きだけ）。
  for (const via of ['submit', 'draft']) {
    test(`(B)(C) この入力を残さないで値がどこにも残らない（${kind}・${via}）`, async (t) => {
      await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
        const shapes = kind === 'idb' ? [secretShapes()[7]] : secretShapes();
        const values = ALL_VALUES();
        for (const s of shapes) {
          const { page, context, logs, rec } = await open(srv, { pad });
          const key = await keyOf(page);
          await sendHarmless(page);
          const base = await waitDigest(kind, page, key);
          await settle(page);
          const w0 = await writes(page);
          if (via === 'submit') await sendSecret(page, s.value);
          else await typeSecret(page, s.value);
          await settle(page);
          // 陽性の対照: 警告の間は値が画面の入力欄か一覧にある（読み直しが効いたことを確かめる前提）。
          const before = await pageContainsAny(page, [s.value]);
          assert.equal(before.field || before.text, true, `${s.id}: the value was never on the page`);
          await page.click(DISCARD);
          await waitCleared(page);
          await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT, { timeout: 5000 });
          await settle(page);
          assert.deepEqual(await digestOf(kind, page, key), base, `${s.id}: the store changed`);
          assert.equal(await writes(page), w0, `${s.id}: history key written`);
          assert.deepEqual(await pageContainsAny(page, values), { text: false, html: false, field: false }, `${s.id}: value on the page`);
          assert.equal(await storesContainAny(page, values), false, `${s.id}: value in a store`);
          assert.equal(await count(page, KEEP), 0, `${s.id}: keep button left`);
          assert.equal(await count(page, DISCARD), 0, `${s.id}: discard button left`);
          const e = await engine(page);
          assert.deepEqual(e.secretWarning, [], `${s.id}: warning left`);
          assert.equal(e.secretAcknowledged, 0, `${s.id}: acknowledgement left`);
          assert.equal(e.exportGate, 'ready', `${s.id}: gate`);
          assertNoLeak(s.id, logs, values);
          clean(rec);
          t.diagnostic(`[secret-choice] ${JSON.stringify({ part: 'BC', kind, via, id: s.id })}`);
          await context.close();
        }
      });
    });
  }
}

test('(D) 読み直しの後に閉じて開き直しても保存先と画面に値が無い', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const values = ALL_VALUES();
    const { page, context, rec } = await open(srv, { pad: false });
    const key = await keyOf(page);
    await sendHarmless(page);
    await waitDigest('local', page, key);
    const safe = await engine(page);
    await settle(page);
    await sendSecret(page, values[7]);
    await page.click(DISCARD);
    await waitCleared(page);
    await settle(page);
    await page.close({ runBeforeUnload: true });
    const next = await reopen(context, srv.url);
    // 読み直した状態（復元は再読み込みとして記録される）は離れる直前に書かれるため、保存先は基準と同じとは限らない。値が無いことが要点。
    assert.notEqual(await digestOf('local', next, key), null, 'the clean state is stored');
    assert.equal((await engine(next)).bootKind, 'restored');
    assert.equal((await engine(next)).current, safe.current, 'the state before the secret');
    assert.deepEqual((await engine(next)).secretWarning, [], 'no warning after reopening');
    assert.deepEqual(await pageContainsAny(next, values), { text: false, html: false, field: false });
    assert.equal(await storesContainAny(next, values), false, 'a secret value is in a store');
    clean(rec);
    await context.close();
  });
});

test('(E) このまま保存するの後に開き直すと復元され、次の変更で再び警告が出る', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const shape = secretShapes()[7];
    const { page, context, rec } = await open(srv, { pad: false });
    const key = await keyOf(page);
    await sendHarmless(page);
    const base = await waitDigest('local', page, key);
    await settle(page);
    await sendSecret(page, shape.value);
    await page.click(KEEP);
    await waitCleared(page);
    await waitChanged('local', page, key, base);
    await settle(page);
    await page.close({ runBeforeUnload: true });
    const next = await reopen(context, srv.url);
    const e0 = await engine(next);
    assert.equal(e0.bootKind, 'restored');
    assert.deepEqual(e0.secretWarning, [], 'no warning right after reopening');
    assert.equal(e0.secretAcknowledged, 0, 'acknowledgement is not persisted');
    // 確認済みは保存しない。値が履歴に入っているため、次の変更の保存で再び警告が出る。
    await sendHarmless(next);
    await waitWarned(next);
    assert.deepEqual((await engine(next)).secretWarning, [shape.expect]);
    clean(rec);
    await context.close();
  });
});

test('(F) 対照 N-01〜N-06 では選択のボタンが出ない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    for (const c of controls()) {
      const { page, context, rec } = await open(srv, { pad: false });
      await sendHarmless(page);
      await page.fill(INPUT, c.text);
      await page.click(SUBMIT);
      await settle(page);
      assert.equal(await count(page, KEEP), 0, `${c.id}: keep button`);
      assert.equal(await count(page, DISCARD), 0, `${c.id}: discard button`);
      assert.deepEqual((await engine(page)).secretWarning, [], `${c.id}: false positive`);
      clean(rec);
      await context.close();
    }
  });
});
