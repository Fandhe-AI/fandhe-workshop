// 断った後に保存済みの履歴と画面が書き換わらないことのブラウザ試験（SAFE-40。TASK-169）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。
// 入力: FANDHE_WEB_TEST_DIR（試験の設定。履歴・起動後の IR・保存先からの起動の経路）と
//       FANDHE_WEB_AUTHOR_DIR（配信する設定。IR_FILE つき・TEST_HOOKS なし。起動時の IR の経路）。未設定は skip にせず失敗にする。
// 前提: どの経路も、先に保存済みの履歴を作ってから断られる入力を読む（空の保存先では「変わらない」が自明になるため）。
// 確かめること（保存先 = localStorage の全体と IndexedDB の履歴の全件。比較は長さと hash の一致だけ）:
//   (a) 履歴ファイル（CSP あり・なし）: 上限超過・入れ子・JSON でない・形が合わない・欄の型が違う・ops 件数超過・別教材の 7 通り。
//       通知が 1 件で固定の文（同じ理由で入力の中身だけ違う 2 本の文が一致）、保存先・エンジンの指紋・環境の差し替え回数・下書きが前と同じ、
//       通知を閉じた後の画面（html・下書き）が前と同じ、入力に入れた番兵が DOM・通知・コンソールに無い、保存済みの履歴は再読み込みで開ける。
//   (b) 起動時の IR（配信する設定）: 上限超過・入れ子・形・uid 重複の 4 通り。同じ待ち受けで /ir/main.json の本体だけ差し替える。
//       ir-notice が 1 件で教材の部品が 0 件、番兵が無く、断っている間と再読み込みの後も保存先が基準と一致し、本体を戻すと保存済みの履歴が開く。
//   (c) 起動後の loadIr: 入れ子・形・uid 重複。状態・ログ・差し替えの回数・URL・保存先・画面が変わらず、番兵が無い。
//   (d) 保存先からの起動: 保存済みの値そのものが断られる入れ子・欄の型の誤り・10MB 超。not_opened で、操作の後も保存先の値がバイト単位で同じ。
//   自己確認（負の対照）: 保存先の比較・画面の比較・番兵の検出器が、受け付けられる 1 手で実際に反応する。
// 限界: (b) の復帰後の画面は試験の口が無いため DOM の題とログの行だけで確かめる。(c) は too_large を扱わない（20MB の文字列を CDP で
//   渡す負荷が大きく、(b) が同じ判定を覆う）。(d) の 10MB 超が IndexedDB に置けない場合は diagnostic に残して置けた範囲で確かめる。Chromium のみ。
// 対象外: 受講者向けの最終の文面（TASK-545・TASK-549・TASK-534）、起動時の理由の案内（TASK-533）。
// 失敗の文・diagnostic には、バイト数・件数・固定のコード・真偽だけを出す（入力の中身と番兵の値は出さない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import { readScreen } from './screen-snapshot.mjs';

const MAX_IR = 20_971_520;
const MAX_HISTORY = 10_485_760;
const BASE = '/fandhe-refusal-keeps-saved/';
const DRAFT = '[data-testid="draft-input"]';
const STEP_TITLE = '[data-testid="step-title"]';
const IR_NOTICE = '[data-testid="ir-notice"]';
const LOG_ITEMS = '[data-testid="part-log-list"] li';
const FILE_INPUT = '[data-testid="history-file"]';
const ITEM = '[data-testid="shell-notice-item"][data-notice-kind="import-refused"]';
const DISMISS = `${ITEM} [data-testid="shell-dismiss"]`;
const OPERATION = '/make-note second-note';
const PACK = 'min-3step';
const START = 's-1';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const MIN3 = path.join(ROOT, 'testdata/engine-event-consistency/test-data/min-3step.json');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.svg': 'image/svg+xml', '.json': 'application/json' };

let browser;
let authorDir;
let tmp;

before(async () => {
  const testDir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(testDir && fs.statSync(testDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a directory');
  authorDir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(authorDir && fs.statSync(authorDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS');
  authorDir = path.resolve(authorDir);
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-refusal-keeps-saved-'));
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
const writeTmp = (name, body) => {
  const p = path.join(tmp, name);
  fs.writeFileSync(p, body);
  return p;
};

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

// ---- 入力の組み立て（実行時に作り、repo に置かない。番兵は呼び出しごとに渡す） ----

const deepText = (depth, inner) => `${'['.repeat(depth)}"${inner}"${']'.repeat(depth)}`;
const padded = (head, total) => {
  const b = Buffer.alloc(total, 0x20);
  b.write(head);
  return b;
};

// 履歴の入力の種類。reason は期待の理由。make(s, base) は番兵 s を入れた入力を返す（base は開ける包みの JSON）。
const HISTORY_CASES = [
  { id: 'H-large', reason: 'too_large', make: (s) => padded(s, MAX_HISTORY + 1) },
  { id: 'H-deep', reason: 'too_deep', make: (s) => Buffer.from(deepText(65, s)) },
  { id: 'H-notjson', reason: 'not_json', make: (s) => Buffer.from(`${s} is not json`) },
  { id: 'H-shape', reason: 'missing_version', make: (s) => Buffer.from(JSON.stringify({ [s]: s })) },
  { id: 'H-malformed', reason: 'malformed', make: (s, b) => Buffer.from(JSON.stringify({ ...b, ops: s })) },
  { id: 'H-ops', reason: 'too_many_ops', make: (s, b) => Buffer.from(JSON.stringify({ ...b, ops: Array.from({ length: 20_001 }, () => ({ draft: s })) })) },
  { id: 'H-otherpack', reason: 'other_pack', make: (s, b) => Buffer.from(JSON.stringify({ ...b, pack_id: s })) },
];

// ---- (a) 履歴ファイル ----

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  for (const c of HISTORY_CASES) {
    test(`履歴 ${c.id}: 断った後に保存先・状態・画面が変わらず、入力の値が出ない（CSP ${label}）`, async (t) => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const opened = await freshPage(srv);
        const { page } = opened;
        await saveOneStep(page);
        const baseText = JSON.parse(await call(page, 'opsHistoryText', 100));
        await page.locator(DRAFT).fill('keep');
        await page.waitForTimeout(1500);
        const storageBefore = await storageDigest(page);
        assert.equal(hasHistory(storageBefore), true, 'a saved history exists (non-vacuous)');
        const fpBefore = fingerprint(await engine(page));
        const installBefore = (await engine(page)).installCount;
        const screenBefore = await screenOf(page);
        await call(page, 'importInstall');

        const texts = [];
        for (const n of [1, 2]) {
          const sentinel = `SAFE40-SENTINEL-${c.id}-${n}`;
          const body = c.make(sentinel, baseText);
          assert.ok(body.length <= MAX_HISTORY + 1, 'input within the test bound');
          await page.setInputFiles(FILE_INPUT, writeTmp(`${c.id}-${n}.json`, body));
          await page.waitForSelector(ITEM, { timeout: 30000 });
          await settle(page);
          const e = await engine(page);
          assert.equal(e.lastImport.verdict, 'refused');
          assert.equal(e.lastImport.reason, c.reason);
          assert.equal(await page.locator(ITEM).count(), 1);
          assert.equal(await page.locator(ITEM).getAttribute('data-notice-reason'), c.reason);
          texts.push(await page.locator(ITEM).textContent());
          assert.ok(texts[n - 1].includes(c.reason), 'notice carries the code');
          assert.equal(await sentinelAbsent(opened, sentinel), true, 'input value is not shown or logged');
          assert.equal(e.installCount, installBefore, 'no environment was built');
          assert.equal(fingerprint(e), fpBefore, 'state unchanged');
          assert.equal(await page.locator(DRAFT).inputValue(), 'keep', 'draft unchanged');
          await page.waitForTimeout(1500);
          assert.equal(await storageDigest(page), storageBefore, 'storage unchanged');
          await page.locator(DISMISS).click();
          assert.equal(await page.locator(ITEM).count(), 0, 'notice dismissed');
          // 通知を閉じた後の画面は、断る前と同じ（フォーカスの欄は閉じる操作で動くため比べない）。
          const after = await screenOf(page);
          assert.equal(after.html === screenBefore.html, true, 'screen html unchanged after dismissing');
          assert.equal(after.draft, screenBefore.draft, 'draft in the screen unchanged');
        }
        assert.equal(texts[0], texts[1], 'notice text does not depend on the input');
        t.diagnostic(`case=${c.id} reason=${c.reason} notice_chars=${texts[0].length}`);

        // 陽性の対照: 保存済みの履歴は、断った後の再読み込みで開ける。
        await page.reload();
        await waitMounted(page);
        await settle(page);
        assert.equal((await engine(page)).bootKind, 'restored');
        assertQuiet(opened.rec, c.id);
        await opened.context.close();
      }));
    });
  }
}

// ---- 自己確認（負の対照） ----

test('自己確認: 保存先・画面・番兵の検出器は、受け付けられる 1 手に反応する', async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const opened = await freshPage(srv);
    const { page } = opened;
    await openStorage(page);
    const sentinel = 'SAFE40-SENTINEL-CONTROL';
    const s0 = await storageDigest(page);
    const h0 = (await screenOf(page)).html;
    await page.locator(DRAFT).fill(`/make-note ${sentinel}`);
    await page.locator(DRAFT).press('Enter');
    await settle(page);
    await page.waitForFunction(([p, s]) => window.__fandheTest.storageRead(p, s) != null, [PACK, START], { timeout: 10000 });
    await page.waitForTimeout(1500);
    assert.notEqual(await storageDigest(page), s0, 'storage comparison detects a write');
    assert.notEqual((await screenOf(page)).html, h0, 'screen comparison detects a change');
    assert.equal(await sentinelAbsent(opened, sentinel), false, 'sentinel detector finds an accepted value on the screen');
    await opened.context.close();
  }));
});

// ---- (b) 起動時の IR（配信する設定） ----

// 静的ファイルを配り、`/ir/main.json` の本体だけを getBody() の返す値にする。待ち受けは 127.0.0.1 のみ。
function serveSwitchable(getBody) {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    if (url.pathname === '/ir/main.json') {
      res.writeHead(200, { 'content-type': 'application/json' });
      res.end(getBody());
      return;
    }
    const rel = url.pathname === '/' ? 'index.html' : url.pathname.slice(1);
    const file = path.resolve(authorDir, rel);
    if (!file.startsWith(authorDir + path.sep) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file)] ?? 'application/octet-stream' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => resolve({ server, origin: `http://127.0.0.1:${server.address().port}` }));
  });
}

const dupUid = (s) => {
  const ir = JSON.parse(fs.readFileSync(MIN3, 'utf8'));
  ir.nodes[0].uid = s;
  ir.nodes[1].uid = s;
  return Buffer.from(JSON.stringify(ir));
};
const IR_CASES = [
  { id: 'I-large', reason: 'too_large', make: (s) => padded(s, MAX_IR + 1) },
  { id: 'I-deep', reason: 'too_deep', make: (s) => Buffer.from(`{"a":${deepText(65, s)}}`) },
  { id: 'I-shape', reason: 'malformed', make: (s) => Buffer.from(JSON.stringify({ [s]: s })) },
  { id: 'I-dupuid', reason: 'malformed', make: dupUid },
];

for (const c of IR_CASES) {
  test(`起動時の IR ${c.id}: 断った後に保存済みの履歴が変わらず、入力の値が出ない`, async (t) => {
    const good = fs.readFileSync(path.join(authorDir, 'ir', 'main.json'));
    let body = good;
    const { server, origin } = await serveSwitchable(() => body);
    try {
      const opened = await openPage(browser, { origin });
      track(opened);
      const { page } = opened;
      await page.goto(`${origin}/`);
      await waitMounted(page);
      await page.waitForSelector(STEP_TITLE, { timeout: 30000 });
      await page.locator(DRAFT).fill(OPERATION);
      await page.locator(DRAFT).press('Enter');
      await page.waitForFunction((sel) => document.querySelectorAll(sel).length > 0, LOG_ITEMS, { timeout: 15000 });
      await page.waitForTimeout(1500);
      await page.reload();
      await waitMounted(page);
      await page.waitForSelector(STEP_TITLE, { timeout: 30000 });
      const logBefore = await page.locator(LOG_ITEMS).count();
      assert.ok(logBefore > 0, 'a saved history is shown');
      // 基準は断る前の正常な画面で確定させる。再読み込み（pagehide）をもう 1 回挟んでも内容全体が変わらないことを確かめてから採る。
      await page.waitForTimeout(1500);
      const settled = await storageDigest(page);
      assert.equal(hasHistory(settled), true, 'the saved history is present before refusing');
      await page.reload();
      await waitMounted(page);
      await page.waitForSelector(STEP_TITLE, { timeout: 30000 });
      await page.waitForTimeout(1500);
      const baseline = await storageDigest(page);
      assert.equal(baseline, settled, 'the saved history is stable under pagehide before refusing');

      const sentinel = `SAFE40-SENTINEL-${c.id}`;
      body = c.make(sentinel);
      await page.goto(`${origin}/`);
      await waitMounted(page);
      // 基準は断る前に確定させた内容。断る画面のマウント後に採り直さず、初回の拒否での書き換えも検出する。
      await page.waitForSelector(IR_NOTICE, { timeout: 60000 });
      assert.equal(await storageDigest(page), baseline, 'storage unchanged at the first refusal (content, not only count)');
      assert.equal(await page.locator(IR_NOTICE).count(), 1);
      assert.equal(await page.locator(IR_NOTICE).getAttribute('role'), 'status');
      const noticeText = await page.locator(IR_NOTICE).innerText();
      assert.ok(noticeText.includes(c.reason), 'notice carries the code');
      assert.equal(await page.locator(STEP_TITLE).count(), 0, 'no step');
      assert.equal(await page.locator(DRAFT).count(), 0, 'no input field');
      assert.equal(await sentinelAbsent(opened, sentinel), true, 'input value is not shown or logged');
      await page.waitForTimeout(1500);
      assert.equal(await storageDigest(page), baseline, 'storage unchanged while refusing');
      await page.reload();
      await waitMounted(page);
      await page.waitForSelector(IR_NOTICE, { timeout: 60000 });
      assert.equal(await storageDigest(page), baseline, 'storage unchanged after pagehide');
      assert.equal(await sentinelAbsent(opened, sentinel), true, 'input value is not shown after the reload');

      // 陽性の対照: 本体を戻すと、保存済みの履歴で開く。
      body = good;
      await page.goto(`${origin}/`);
      await waitMounted(page);
      await page.waitForSelector(STEP_TITLE, { timeout: 30000 });
      assert.equal(await page.locator(IR_NOTICE).count(), 0);
      assert.ok((await page.locator(LOG_ITEMS).count()) >= logBefore, 'the saved log is not lost');
      await page.waitForTimeout(1500);
      assert.equal(await storageDigest(page), baseline, 'storage content is identical to the baseline after recovery');
      t.diagnostic(`case=${c.id} reason=${c.reason} notice_chars=${noticeText.length}`);
      assertQuiet(opened.rec, c.id);
      await opened.context.close();
    } finally {
      server.closeAllConnections();
      server.close();
    }
  });
}

// ---- (c) 起動後の loadIr ----

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  const LOAD_CASES = [
    { id: 'L-deep', reason: 'too_deep', make: (s) => `{"a":${deepText(65, s)}}` },
    { id: 'L-shape', reason: 'malformed', make: (s) => JSON.stringify({ [s]: s }) },
    { id: 'L-dupuid', reason: 'malformed', make: (s) => dupUid(s).toString() },
  ];
  for (const c of LOAD_CASES) {
    test(`起動後の IR ${c.id}: 読めない IR を送っても状態・保存先・画面が変わらない（CSP ${label}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const opened = await freshPage(srv);
        const { page } = opened;
        await saveOneStep(page);
        await page.reload();
        await waitMounted(page);
        await settle(page);
        const before = await engine(page);
        assert.equal(before.bootKind, 'restored');
        const url = page.url();
        const storage = await storageDigest(page);
        assert.equal(hasHistory(storage), true);
        const screen = await screenOf(page);

        const sentinel = `SAFE40-SENTINEL-${c.id}`;
        assert.equal(await page.evaluate((text) => window.__fandheTest.loadIr(text), c.make(sentinel)), true);
        await settle(page);
        const after = await engine(page);
        assert.equal(after.irRefusal, c.reason);
        assert.equal(after.irLoaded, true);
        assert.equal(JSON.stringify(after.stateWithoutLog), JSON.stringify(before.stateWithoutLog));
        assert.equal(after.logLen, before.logLen);
        assert.equal(after.installCount, before.installCount);
        assert.equal(page.url(), url);
        await page.waitForTimeout(1500);
        assert.equal(await storageDigest(page), storage, 'storage unchanged');
        assert.equal((await screenOf(page)).html, screen.html, 'screen unchanged');
        assert.equal(await sentinelAbsent(opened, sentinel), true, 'input value is not shown or logged');
        assertQuiet(opened.rec, c.id);
        await opened.context.close();
      }));
    });
  }
}

// ---- (d) 保存先からの起動 ----

const STORED_CASES = [
  { id: 'S-deep', make: (s) => deepText(65, s) },
  { id: 'S-malformed', make: (s, b) => JSON.stringify({ ...b, ops: s }) },
  { id: 'S-large', make: (s) => padded(s, MAX_HISTORY + 1).toString() },
];

for (const c of STORED_CASES) {
  test(`保存先からの起動 ${c.id}: 断られても保存先の値がバイト単位で変わらず、入力の値が出ない`, async (t) => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const opened = await freshPage(srv);
      const { page } = opened;
      await openStorage(page);
      const base = JSON.parse(await call(page, 'opsHistoryText', 100));
      const sentinel = `SAFE40-SENTINEL-${c.id}`;
      const text = c.make(sentinel, base);
      const written = JSON.parse(await call(page, 'storageWrite', PACK, START, text));
      t.diagnostic(`case=${c.id} bytes=${text.length} write_ok=${written.ok}`);
      if (written.ok !== true) {
        // 置けない大きさの値は限界として残し、置けた場合だけ確かめる（黙って落とさない）。docs/testing.md に記録する。
        assert.equal(c.id, 'S-large', 'only the oversized value may fail to be stored');
        await opened.context.close();
        return;
      }
      await page.waitForTimeout(1500);
      assert.equal(JSON.parse(await call(page, 'storageState', PACK, START)).lastAsyncError ?? null, null, 'no async storage error');
      await page.reload();
      await waitMounted(page);
      await settle(page);
      const e0 = await engine(page);
      assert.equal(e0.bootKind, 'not_opened');
      const storage = await storageDigest(page);
      await page.locator(DRAFT).fill(OPERATION);
      await page.locator(DRAFT).press('Enter');
      await settle(page);
      await page.waitForTimeout(1500);
      assert.equal(await storageDigest(page), storage, 'storage unchanged after an operation');
      await openStorage(page);
      assert.equal((await call(page, 'storageRead', PACK, START)) === text, true, 'the stored value is byte-identical');
      assert.equal(await sentinelAbsent(opened, sentinel), true, 'stored value is not shown or logged');
      t.diagnostic(`boot refused notices: ${await page.locator(ITEM).count()}`);
      assertQuiet(opened.rec, c.id);
      await opened.context.close();
    }));
  });
}
