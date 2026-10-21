// 警告の間の保存の停止・書き出しの停止・放置したときの扱いの試験（SAFE-27・SAFE-28・SAFE-44・D-44。TASK-164 2/3）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、次を確かめる。
//   (A) 書き出しの停止: 10 形のそれぞれを送信で入れ、警告の後に `engine.exportGate` が held で `exportBytes` が null。
//       対照 N-01 は ready で `exportBytes` が正。結果の JSON に値が含まれない。
//   (B) IndexedDB の経路でも、警告の間は保存先が変わらない。localStorage を閾値の手前まで別の鍵で埋めて IndexedDB の経路を選ばせ、
//       無害な 1 手が IndexedDB へ届くこと（陽性の対照）を確かめた後、秘密情報の送信と続く無害な 1 手で、値の digest と
//       履歴の鍵への put の記録が増えない。localStorage の経路でも同じ。
//   (C) 放置して閉じる（SAFE-44）: 警告を出したまま `pagehide` つきで閉じて同じ context で開き直すと、保存先は基準のまま、
//       復元した状態は秘密情報の前の状態と等しく、警告は出ない。保存先のどこにも値が無い。
//       下書きだけの経路（入力して警告が出た後に閉じる）と、送信して続けて進めた経路の両方を、保存先ごとに確かめる。
//       保存の無い状態から始めて警告のまま閉じると、鍵が無く、開き直すと新しく始まる。
//   (D) 値の非漏洩: console・pageerror の文、assert・diagnostic に値を出さない（出すのは試験データの ID と形の ID だけ）。
// 値は `secret-values.mjs` が部品から組み立て、変数の中だけに置く。保存先の走査はページの中で行い、真偽だけを返す。
// 限界: 警告のまま閉じるまでの進行は保存されない。これは D-44 で決めて受け入れた振る舞いである（docs/known-gaps.md）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP ありの設定で流す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize,
} from './harness.mjs';
import { secretShapes, controls } from './secret-values.mjs';

const BASE = '/fandhe-secret-hold/';
const PACK = 'min-3step';
const START = 's-1';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const NOTICE = '[data-testid="secret-notice"]';
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
const noticeText = (page) => page.$eval(NOTICE, (el) => el.textContent ?? '');

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

async function sendHarmless(page) {
  const before = (await engine(page)).logLen;
  await page.fill(INPUT, ACCEPTED);
  await page.click(SUBMIT);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before);
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null');
}

// 秘密情報を送信して警告が出るまで待つ。値は入力欄・画面に出る（TASK-164 3/3 の範囲）。
async function sendSecret(page, value) {
  const before = (await engine(page)).logLen;
  await page.evaluate(([sel, v]) => {
    const ta = document.querySelector(sel);
    ta.value = v;
    ta.dispatchEvent(new Event('input', { bubbles: true }));
  }, [INPUT, value]);
  await page.click(SUBMIT);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before);
  await page.waitForFunction((sel) => (document.querySelector(sel)?.textContent ?? '') !== '', NOTICE, { timeout: 5000 });
}

// 下書きだけの経路。遅延保存の検出で警告が出るまで待つ。
async function typeSecret(page, value) {
  await page.evaluate(([sel, v]) => {
    const ta = document.querySelector(sel);
    ta.value = v;
    ta.dispatchEvent(new Event('input', { bubbles: true }));
  }, [INPUT, value]);
  await page.waitForFunction((sel) => (document.querySelector(sel)?.textContent ?? '') !== '', NOTICE, { timeout: 5000 });
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

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
}

function assertNoLeak(id, texts, values) {
  for (const t of texts) for (const v of values) assert.ok(!t.includes(v), `${id}: value leaked`);
}

test('(A) 10 形の送信で書き出しが止まり、対照は書き出せる', async (t) => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    for (const s of secretShapes()) {
      const { page, context, logs, rec } = await open(srv, { pad: false });
      await sendSecret(page, s.value);
      await settle(page);
      const e = await engine(page);
      assert.deepEqual(e.secretWarning, [s.expect], `${s.id}: warning shapes`);
      assert.equal(e.exportGate, 'held', `${s.id}: gate`);
      assert.equal(e.exportBytes, null, `${s.id}: bytes exposed`);
      // 状態 JSON の全体にはメモリ上のログ（入力）が入るため、関門の欄だけを見る。
      assertNoLeak(s.id, [JSON.stringify({ g: e.exportGate, b: e.exportBytes, w: e.secretWarning }), ...logs], [s.value]);
      clean(rec);
      t.diagnostic(`[secret-hold] ${JSON.stringify({ part: 'A', id: s.id, gate: e.exportGate })}`);
      await context.close();
    }
    const n01 = controls()[0];
    const ok = await open(srv, { pad: false });
    await sendHarmless(ok.page);
    await ok.page.fill(INPUT, n01.text);
    await ok.page.click(SUBMIT);
    await settle(ok.page);
    const e = await engine(ok.page);
    assert.deepEqual(e.secretWarning, [], `${n01.id}: false positive`);
    assert.equal(e.exportGate, 'ready', `${n01.id}: gate`);
    assert.equal(e.exportBytes > 0, true, `${n01.id}: bytes missing`);
    await ok.context.close();
  });
});

for (const kind of ['local', 'idb']) {
  const pad = kind === 'idb';

  test(`(B) 警告の間は保存先が変わらない（${kind}）`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      const shapes = secretShapes();
      const { page, context, rec } = await open(srv, { pad });
      const key = await keyOf(page);
      await sendHarmless(page);
      const base = await waitDigest(kind, page, key);
      await settle(page);
      const w0 = await writes(page);
      assert.equal(w0 >= 1, true, 'positive control: the harmless step wrote');
      if (kind === 'idb') assert.equal(await localDigest(page, key), null, 'the idb path is selected');
      const { id, value, expect } = shapes[0];
      await sendSecret(page, value);
      await sendHarmless(page);
      await settle(page);
      assert.deepEqual((await engine(page)).secretWarning, [expect], `${id}: still warned after progress`);
      assert.deepEqual(await digestOf(kind, page, key), base, `${id}: the store changed`);
      assert.equal(await writes(page), w0, `${id}: history key written`);
      clean(rec);
      await context.close();
    });
  });

  // (C) 放置して閉じる。via は警告の出し方（draft: 下書きだけ / submit: 送信して続けて進める）。
  for (const via of ['draft', 'submit']) {
    test(`(C) 警告のまま閉じて開き直すと保存先は基準のまま警告も出ない（${kind}・${via}）`, async () => {
      await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
        const values = secretShapes().map((s) => s.value);
        const { page, context, rec, logs } = await open(srv, { pad });
        const key = await keyOf(page);
        await sendHarmless(page);
        const base = await waitDigest(kind, page, key);
        await settle(page);
        const safe = await engine(page);
        if (via === 'draft') {
          await typeSecret(page, values[7]);
        } else {
          await sendSecret(page, values[7]);
          await sendHarmless(page);
        }
        await settle(page);
        assert.equal((await engine(page)).secretWarning.length > 0, true, 'warned before closing');
        await page.close({ runBeforeUnload: true });

        const next = await reopen(context, srv.url);
        assert.deepEqual(await digestOf(kind, next, key), base, 'the store is at the baseline');
        const e = await engine(next);
        assert.equal(e.bootKind, 'restored');
        assert.equal(e.logLen, safe.logLen + 1, 'restored state plus one reloaded event');
        assert.equal(e.current, safe.current, 'the state before the secret');
        assert.deepEqual(e.secretWarning, [], 'no warning after reopening');
        assert.equal(await noticeText(next), '');
        assert.equal(await storesContainAny(next, values), false, 'a secret value is in a store');
        assertNoLeak('C', logs, values);
        clean(rec);
        await context.close();
      });
    });
  }
}

test('(C) 保存の無い状態から警告のまま閉じると鍵が無く新しく始まる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const values = secretShapes().map((s) => s.value);
    const { page, context } = await open(srv, { pad: false });
    const key = await keyOf(page);
    await sendSecret(page, values[7]);
    await settle(page);
    await page.close({ runBeforeUnload: true });
    const next = await reopen(context, srv.url);
    assert.equal(await localDigest(next, key), null, 'no key was written');
    assert.equal(await idbDigest(next, key), null, 'no idb value was written');
    assert.equal((await engine(next)).bootKind, 'fresh');
    assert.equal(await storesContainAny(next, values), false, 'a secret value is in a store');
    await context.close();
  });
});
