// 旧版が残っていないときに表示のみへ落ちることのブラウザでの確認（VER-9・VER-10。TASK-178 2/2）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。履歴 15 件と開く側の教材は version-histories と同じ
// （FANDHE_VERSION_HISTORIES_DIR。表は testdata/version-histories/expectations.json を直接読む）。
//
// エンジンは版つき URL を知らず、別の版を探さない（SAFE-33・D-68。docs/known-gaps.md の VER-9）。最新版だけの配信では、
// 15 件のうち表示だけになるのがちょうど 5 件で、その履歴は操作が何も変えない。本試験はそれを CSP あり・なしで固定する。
//
// 確かめること:
//   - 表の表示だけの行の ID の集合が {V-ii-1, V-iii-1, V-iii-2, V-iv-4, V-v-3} と一致する
//   - 5 件のそれぞれ: 結果が view_only で理由が表と一致し、通知欄に理由のコードが 1 件出る。実行環境の差し替えは 0 回
//   - 5 件のそれぞれ: 打鍵・Enter・送信・Esc・raw の save・raw の stop が状態・ログ・世代・入力欄・保存領域・URL を変えない
//   - 5 件のそれぞれ: ほかの版を探す要求（配信元の `.json`・`ir/` の下・旧版らしいパスへの取得）が読み込みの前後で 0 件増える
//   - 外部への要求・CSP 違反・ページのエラー・dialog・popup が 0 件
//   - 陽性の対照: V-0（開く履歴）では同じ操作が状態を変える（比較が変化を検出できる）
// 既存の view-only（通知・起動時の復元）・view-only-storage（保存先のバイト一致）・version-histories（15 件の判定）との分担:
// 本試験は 5 件の ID と全理由コードでの操作なしと「別の版を探さない」を担う。
// 限界: 起動時の復元は、再読み込みで教材の上書き（loadIr）が消えるため view-only が担う content_major だけをブラウザで確かめる。
// 4 種の理由コードの起動時の経路は core（boot_without_a_versioned_url_matches_the_import_judgment）と web
// （crates/web/tests/view_only_fallback.rs）が担う。V-iv-4・V-v-3 は版の欄を書き換えた作り物である。
// 失敗の文には行 ID・件数・真偽・固定のコードだけを出す（履歴の中身を出さない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-view-only-fallback/';
const FILE_INPUT = '[data-testid="history-file"]';
const RAW = '[data-testid="raw-trigger"]';
const NOTICE = '[data-testid="view-only-notice"]';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const OPERATION = '/make-note second-note';
const EXPECTED_IDS = ['V-ii-1', 'V-iii-1', 'V-iii-2', 'V-iv-4', 'V-v-3'];
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

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);
const engine = async (page) => JSON.parse(await call(page, 'stateJson')).engine;
const urlState = (page) => page.evaluate(() => ({ href: location.href, length: history.length }));
const fingerprint = (e) => JSON.stringify({ s: e.stateWithoutLog, l: e.log, g: e.generation, c: e.current });

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

// 保存領域の指紋（値は要約にして返し、中身は失敗の文に出ない）。
async function storageFingerprint(page) {
  return page.evaluate(() => {
    const digest = (text) => {
      let h = 0x811c9dc5;
      for (let i = 0; i < text.length; i += 1) {
        h ^= text.charCodeAt(i);
        h = Math.imul(h, 0x01000193) >>> 0;
      }
      return `${text.length}:${h.toString(16)}`;
    };
    return Object.keys(localStorage).sort().map((k) => [k, digest(String(localStorage.getItem(k)))]);
  });
}

// 6 種の操作を送り、状態・ログ・世代・入力欄・保存領域・URL が変わったかを返す（変わったとき false の「不変」）。
async function runOperations(page) {
  const before = {
    engine: fingerprint(await engine(page)), storage: JSON.stringify(await storageFingerprint(page)), url: JSON.stringify(await urlState(page)),
  };
  const steps = [
    async () => { await page.locator(INPUT).fill(OPERATION); },
    async () => { await page.locator(INPUT).press('Enter'); },
    async () => {
      await page.locator(INPUT).fill(OPERATION);
      await page.locator(SUBMIT).click({ force: true });
    },
    async () => { await page.locator(INPUT).press('Escape'); },
    async () => { await sendRaw(page, { control: 'save' }); },
    async () => { await sendRaw(page, { control: 'stop' }); },
  ];
  let inputsEmpty = true;
  for (const step of steps) {
    await step();
    await settle(page);
    if ((await page.locator(INPUT).inputValue()) !== '') inputsEmpty = false;
  }
  await settle(page);
  const same = fingerprint(await engine(page)) === before.engine
    && JSON.stringify(await storageFingerprint(page)) === before.storage
    && JSON.stringify(await urlState(page)) === before.url;
  return { unchanged: same, inputsEmpty };
}

// 配信元への要求のうち、ほかの版を探すものに当たる URL かどうか（`.json`・`ir/` の下・`/v<数字>/` の形）。
const looksLikeVersionLookup = (url) => {
  const p = new URL(url).pathname;
  return p.endsWith('.json') || p.includes('/ir/') || /\/v\d+(\.\d+)*\//.test(p);
};

async function openRow(srv, row) {
  const opened = await openPage(browser, { origin: srv.origin });
  const { page } = opened;
  await page.goto(srv.url);
  await waitMounted(page);
  await settle(page);
  const irText = fs.readFileSync(path.join(historiesDir, 'packs', `${row.opening.pack}.ir.json`), 'utf8');
  const n0 = (await engine(page)).installCount;
  assert.equal(await call(page, 'loadIr', irText), true, `${row.id}: load-ir sent`);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
  assert.equal(await call(page, 'openingOverride', JSON.stringify({ engine: row.opening.engine, vocab_max: row.opening.vocab_max })), true, `${row.id}: override sent`);
  await settle(page);
  assert.equal(await page.locator(NOTICE).textContent(), '', `${row.id}: no notice before the import`);

  const requests = [];
  page.on('request', (r) => requests.push(r.url()));
  const installBefore = (await engine(page)).installCount;
  await call(page, 'importInstall');
  await page.setInputFiles(FILE_INPUT, path.join(historiesDir, `${row.id}.history.json`));
  await page.waitForFunction(() => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    return e && e.lastImport != null;
  }, null, { timeout: 15000 });
  await settle(page);
  const e = await engine(page);
  const ops = await runOperations(page);
  const out = {
    verdict: e.lastImport.verdict,
    reason: e.lastImport.reason ?? null,
    viewOnly: e.viewOnly ?? null,
    installDelta: e.installCount - installBefore,
    noticeCount: await page.locator(NOTICE).count(),
    noticeText: (await page.locator(NOTICE).textContent()) ?? '',
    noticeHasCode: false,
    lookups: requests.filter(looksLikeVersionLookup).length,
    ...ops,
  };
  if (out.noticeCount === 1) {
    out.noticeHasCode = typeof e.viewOnly === 'string' && (await page.locator(NOTICE).textContent()).includes(e.viewOnly);
  }
  assertQuiet(opened.rec, row.id);
  await opened.context.close();
  return out;
}

test('表の形: 版つき URL が無いと表示だけになるのは 5 件で、ID が固定のとおり', () => {
  const ids = table.rows.filter((r) => r.expect.result === 'view_only').map((r) => r.id);
  assert.deepEqual(ids, EXPECTED_IDS);
  assert.equal(table.rows.length, 15);
});

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`表示だけの 5 件は通知が出て操作が何も変えず、ほかの版を探さない（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      for (const id of EXPECTED_IDS) {
        const row = table.rows.find((r) => r.id === id);
        const r = await openRow(srv, row);
        assert.equal(r.verdict, 'view_only', `${id}: view only`);
        assert.equal(r.reason, row.expect.code, `${id}: reason matches the table`);
        assert.equal(r.viewOnly, r.reason, `${id}: viewOnly is the import reason`);
        assert.equal(r.installDelta, 0, `${id}: no environment was built`);
        assert.equal(r.noticeCount, 1, `${id}: one notice`);
        assert.equal(r.noticeHasCode, true, `${id}: notice carries the reason code`);
        assert.equal(r.unchanged, true, `${id}: operations changed nothing`);
        assert.equal(r.inputsEmpty, true, `${id}: input stays empty`);
        assert.equal(r.lookups, 0, `${id}: no other version was looked up`);
      }
    }));
  });

  test(`対照: 開く履歴（V-0）では同じ操作が状態を変える（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const row = table.rows.find((r) => r.id === 'V-0');
      const r = await openRow(srv, row);
      assert.equal(r.verdict, 'open', 'V-0: opens');
      assert.equal(r.noticeText, '', 'V-0: no view-only notice text');
      assert.equal(r.unchanged, false, 'V-0: the same operations change state, so the comparison can detect change');
    }));
  });
}
