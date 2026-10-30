// 起動時に壊れた履歴を検出し保存を止めることのブラウザ試験（SAVE-21・SHELL-17。TASK-333 1/2）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。
// 入力: FANDHE_WEB_TEST_DIR（試験の設定）。未設定は skip にせず失敗にする。
// 確かめること（CSP あり・なしの両方。壊れた値は保存先へ直接置いて再読み込みで起動する）:
//   壊れた履歴 7 通り（B-1〜B-7。作り方は testdata/broken-histories の表と同じ規則で、ページの中の `opsHistoryText` を基準に作る）:
//     not_opened で始まり、`corrupt-history` の通知がちょうど 1 件で理由が表のコードと一致し、文が空でなく、`import-refused` は 0 件、
//     「捨てて始める」のボタンが 1 個、最初の Step で始まる。1 手送っても、「捨てて始める」を押しても、保存先がバイト単位で変わらず、
//     再読み込みでも同じ通知が出る。入力に入れた番兵が DOM・通知・コンソールに無い。外部通信・CSP 違反・ページのエラー・ダイアログが 0 件。
//   対照: 開ける値は restored で通知 0 件、版の判定の段で断られる値は `import-refused` だけで `corrupt-history` は 0 件。
// 限界: B-4（10MB 超）が保存先に置けない場合は diagnostic に残して置けた範囲で確かめる。作り方がページの中の IR で表と違う理由になった
//   場合は失敗にする（表を黙って直さない）。「捨てて始める」の処理は 2/2（TASK-333 2/2）。Chromium のみ。
// 失敗の文・diagnostic には、バイト数・件数・固定のコード・真偽だけを出す（入力の中身と番兵の値は出さない）。

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import { readScreen } from './screen-snapshot.mjs';

const MAX_HISTORY = 10_485_760;
const BASE = '/fandhe-boot-corrupt-history/';
const DRAFT = '[data-testid="draft-input"]';
const OPERATION = '/make-note second-note';
const PACK = 'min-3step';
const START = 's-1';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

let browser;
let tmp;

before(async () => {
  const testDir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(testDir && fs.statSync(testDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a directory');
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-boot-corrupt-history-'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
  if (tmp) fs.rmSync(tmp, { recursive: true, force: true });
});
// ---- 共通の部品 ----

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);
const engine = async (page) => JSON.parse(await call(page, 'stateJson')).engine;
const settle = (page) => page.waitForFunction(() => window.__fandheTest.nextTick() === 'null', null, { timeout: 15000 });
const fingerprint = (e) => JSON.stringify({ s: e.stateWithoutLog, l: e.log, g: e.generation, c: e.current });

// 保存先の全体（localStorage の全件と IndexedDB の全ストアの全件）の長さと hash の一覧。比較にだけ使う。
function storageDigest(page) {
  return page.evaluate(async () => {
    const hash = (v) => {
      let h = 0;
      for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
      return { len: v.length, h };
    };
    const local = [];
    for (let i = 0; i < localStorage.length; i += 1) {
      const k = localStorage.key(i);
      local.push([k, hash(localStorage.getItem(k))]);
    }
    local.sort((a, b) => (a[0] < b[0] ? -1 : 1));
    const idb = [];
    const dbs = indexedDB.databases ? await indexedDB.databases() : [];
    for (const info of dbs) {
      await new Promise((resolve) => {
        const req = indexedDB.open(info.name);
        req.onerror = () => resolve();
        req.onsuccess = () => {
          const db = req.result;
          const names = [...db.objectStoreNames];
          if (names.length === 0) { db.close(); resolve(); return; }
          const tx = db.transaction(names);
          let left = names.length;
          for (const n of names) {
            const store = tx.objectStore(n);
            const keys = store.getAllKeys();
            const vals = store.getAll();
            vals.onsuccess = () => {
              keys.result.forEach((k, i) => {
                const v = vals.result[i];
                idb.push([`${info.name}/${n}/${String(k)}`, typeof v === 'string' ? hash(v) : { len: -1, h: 0 }]);
              });
              left -= 1;
              if (left === 0) { db.close(); resolve(); }
            };
            vals.onerror = () => { left -= 1; if (left === 0) { db.close(); resolve(); } };
          }
        };
      });
    }
    idb.sort((a, b) => (a[0] < b[0] ? -1 : 1));
    return JSON.stringify({ local, idb });
  });
}
const hasHistory = (digest) => {
  const d = JSON.parse(digest);
  return d.local.length + d.idb.length > 0;
};

// 正常な起動は起動の印（meta/boot）を書き直すため、本体を戻した後は履歴の項目だけで比べる。
const withoutBoot = (digest) => {
  const d = JSON.parse(digest);
  d.local = d.local.filter(([k]) => !k.includes('/meta/boot/'));
  return JSON.stringify(d);
};

const screenOf = (page) => page.evaluate(readScreen);

function track(opened) {
  opened.consoleTexts = [];
  opened.page.on('console', (m) => opened.consoleTexts.push(m.text()));
}

// 番兵が DOM 全体（属性を含む）と通知の文字とコンソールのどこにも無い。
async function sentinelAbsent(opened, sentinel) {
  const html = await opened.page.evaluate(() => document.documentElement.outerHTML);
  const text = await opened.page.evaluate(() => document.body.innerText);
  return !html.includes(sentinel) && !text.includes(sentinel) && !opened.consoleTexts.some((t) => t.includes(sentinel));
}

function assertQuiet(rec, label) {
  const s = summarize(rec);
  assert.equal(s.crashed, 0, `${label}: tab must not crash`);
  assert.equal(s.external, 0, `${label}: no external request`);
  assert.equal(s.csp, 0, `${label}: no CSP violation`);
  assert.equal(s.pageErrors, 0, `${label}: no page error`);
  assert.equal(s.dialogs, 0, `${label}: no dialog`);
  assert.equal(s.popups, 0, `${label}: no popup`);
}

async function freshPage(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  track(opened);
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  await settle(opened.page);
  return opened;
}

async function openStorage(page) {
  await call(page, 'storageOpen');
  await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [PACK, START], { timeout: 10000 });
}

// 保存先を開いて 1 手を送り、保存され切るまで待つ。保存済みの履歴がある状態にする。
async function saveOneStep(page) {
  await openStorage(page);
  await page.locator(DRAFT).fill(OPERATION);
  await page.locator(DRAFT).press('Enter');
  await settle(page);
  await page.waitForFunction(([p, s]) => window.__fandheTest.storageRead(p, s) != null, [PACK, START], { timeout: 10000 });
  await page.waitForTimeout(1500);
}


// ---- 壊れた履歴の作り方（ページの中の基準の履歴 JSON から。実行時に作り、repo に置かない） ----

const EXPECTATIONS = JSON.parse(fs.readFileSync(path.join(ROOT, 'testdata/broken-histories/expectations.json'), 'utf8'));
const expectCode = (id) => EXPECTATIONS.rows.find((r) => r.id === id).expect.code;

const edit = (base, fn) => {
  const v = JSON.parse(base);
  fn(v);
  return JSON.stringify(v);
};

const CASES = [
  { id: 'B-1', make: (s, b) => edit(b, (v) => { v.ops.find((o) => o.submit !== undefined).submit = `/make-note ${s}`; }) },
  { id: 'B-2', make: (s, b) => b.slice(0, Math.floor(b.length * 0.55)) },
  { id: 'B-3', make: (s, b) => { const at = b.indexOf('"ops":[') + '"ops":['.length; return `${b.slice(0, at)}}${b.slice(at)}`; } },
  { id: 'B-4', make: (s, b) => edit(b, (v) => { v.padding = s.padEnd(MAX_HISTORY + 1 - edit(b, (w) => { w.padding = ''; }).length, 'x'); }) },
  { id: 'B-5', make: (s, b) => edit(b, (v) => { delete v.versions; }) },
  { id: 'B-6', make: (s, b) => edit(b, (v) => { v.pack_id = s; }) },
  { id: 'B-7', make: (s, b) => edit(b, (v) => { v.snapshot.run.generation = s; }) },
];

const NOTICE = (kind) => `[data-testid="shell-notice-item"][data-notice-kind="${kind}"]`;
const DISCARD = '[data-testid="shell-discard-start"]';

async function bootWith(page, text) {
  const written = JSON.parse(await call(page, 'storageWrite', PACK, START, text));
  if (written.ok !== true) return false;
  await page.waitForFunction(([p, s]) => (JSON.parse(window.__fandheTest.storageState(p, s)).lastAsyncError ?? null) === null, [PACK, START], { timeout: 10000 });
  await page.waitForTimeout(500);
  await page.reload();
  await waitMounted(page);
  await settle(page);
  return true;
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  for (const c of CASES) {
    test(`壊れた履歴 ${c.id}: 起動時に corrupt-history が出て新しい状態で始まり、保存先が変わらない（CSP ${label}）`, async (t) => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const opened = await freshPage(srv);
        const { page } = opened;
        await openStorage(page);
        const base = await call(page, 'opsHistoryText', 100);
        const sentinel = `SAVE21-SENTINEL-${c.id}`;
        const text = c.make(sentinel, base);
        t.diagnostic(`case=${c.id} bytes=${text.length}`);
        if (!(await bootWith(page, text))) {
          assert.equal(c.id, 'B-4', 'only the oversized value may fail to be stored');
          await opened.context.close();
          return;
        }
        const e = await engine(page);
        assert.equal(e.bootKind, 'not_opened');
        const code = expectCode(c.id);
        assert.equal(await page.locator(NOTICE('corrupt-history')).count(), 1, 'one corrupt-history notice');
        assert.equal(await page.locator(NOTICE('corrupt-history')).getAttribute('data-notice-reason'), code);
        assert.ok((await page.locator(NOTICE('corrupt-history')).textContent()).trim().length > 0, 'notice has text');
        assert.equal(await page.locator(NOTICE('import-refused')).count(), 0, 'no import-refused');
        assert.equal(await page.locator(DISCARD).count(), 1, 'one discard button');
        assert.equal(await page.locator(`${NOTICE('corrupt-history')} [data-testid="shell-dismiss"]`).count(), 0, 'not dismissable');
        assert.equal(e.log.length, 0, 'starts fresh (empty log)');
        assert.equal(await sentinelAbsent(opened, sentinel), true, 'input value is not shown or logged');

        const stored = await storageDigest(page);
        await page.locator(DRAFT).fill(OPERATION);
        await page.locator(DRAFT).press('Enter');
        await settle(page);
        await page.waitForTimeout(1500);
        assert.equal(await storageDigest(page), stored, 'storage unchanged after an operation');
        await page.locator(DISCARD).click();
        await page.waitForTimeout(500);
        assert.equal(await storageDigest(page), stored, 'storage unchanged after pressing discard (not handled until 2/2)');
        assert.equal(await page.locator(NOTICE('corrupt-history')).count(), 1, 'notice stays');

        await openStorage(page);
        assert.equal((await call(page, 'storageRead', PACK, START)) === text, true, 'the stored value is byte-identical');
        await page.reload();
        await waitMounted(page);
        await settle(page);
        assert.equal(await page.locator(NOTICE('corrupt-history')).count(), 1, 'notice again after reload');
        await openStorage(page);
        assert.equal((await call(page, 'storageRead', PACK, START)) === text, true, 'byte-identical after reload');
        assert.equal(await sentinelAbsent(opened, sentinel), true, 'no sentinel after reload');
        assertQuiet(opened.rec, c.id);
        await opened.context.close();
      }));
    });
  }

  test(`対照: 開ける値は通知 0 件、版の判定の段の拒否は import-refused だけ（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const opened = await freshPage(srv);
      const { page } = opened;
      await openStorage(page);
      const base = await call(page, 'opsHistoryText', 100);
      assert.equal(await bootWith(page, base), true);
      assert.equal((await engine(page)).bootKind, 'restored');
      assert.equal(await page.locator(NOTICE('corrupt-history')).count(), 0, 'openable value: no corrupt-history');

      assertQuiet(opened.rec, 'control-open');
      await opened.context.close();

      // 開けた起動の再読み込みは保存し直すため、版の判定の段の拒否は新しい context で確かめる。
      const newer = edit(base, (v) => { v.versions.ir_format += 1; });
      const second = await freshPage(srv);
      const page2 = second.page;
      await openStorage(page2);
      assert.equal(await bootWith(page2, newer), true);
      assert.equal((await engine(page2)).bootKind, 'not_opened');
      assert.equal(await page2.locator(NOTICE('import-refused')).count(), 1, 'version-stage refusal: import-refused');
      assert.equal(await page2.locator(NOTICE('corrupt-history')).count(), 0, 'version-stage refusal: no corrupt-history');
      assertQuiet(second.rec, 'control');
      await second.context.close();
    }));
  });
}
