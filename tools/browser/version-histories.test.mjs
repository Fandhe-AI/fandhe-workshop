// 版の違う履歴 15 件のブラウザでの確認（VER-2。TASK-175 2/2。#449）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。履歴 15 件と開く側の教材は、native の試験
// （`crates/core/tests/version_histories.rs`）と同じ作り方で `emit_histories_for_browser`（`#[ignore]`）が書き出し、
// その場所を FANDHE_VERSION_HISTORIES_DIR で受け取る。期待の表は `testdata/version-histories/expectations.json` を直接読む
// （表を 2 か所に持たない）。判定の正は core の `history_open`・`history_load` で、ここは web を通した結果
// （`stateJson().engine.lastImport`・`installCount`・`lastImportStateJson()`）だけを確かめる。
//
// 開く側の条件は試験の設定（TEST_HOOKS=1）の口から作る。教材は `loadIr`、エンジンの版と語彙の上限は `openingOverride`。
// 配信する設定の判定の経路は変えない（上書きは配信物に存在しない）。
//
// 確かめること（15 件のそれぞれ）:
//   - 結果の種類・理由コード・違いの軸が表と合う（食い違いを「期待より慎重な側」と「それ以外」に分けて数え、どちらも 0 件）
//   - 開く 7 件（変換なし 1・変換して開く 6）: 開いた直後の状態が履歴ファイルの `snapshot` と完全に一致する
//   - 開かない 8 件（表示だけ 5・開かない 3）: 実行環境を差し替えた回数が 0 で、状態・保存領域・URL が変わらない
//   - 開かない 8 件の理由が空でない。開く 7 件の理由は `null`
//   - 外部への要求・CSP 違反・ページのエラー・dialog・popup が 0 件
// 比較器の自己確認として、表の期待を 1 つ書き換えると食い違いが出ることも確かめる。
// 失敗の文には行 ID・種類・コード・軸名・真偽だけを出す。履歴の中身・状態の JSON は出さない（受講者のデータのため）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual } from 'node:util';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-version-histories/';
const FILE_INPUT = '[data-testid="history-file"]';
const TABLE_PATH = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'testdata', 'version-histories', 'expectations.json');

// native の試験（version_histories.rs）の VER-2 の定数と同じ値。表だけの書き換えを検出する。
const IDS = ['V-0', 'V-i-1', 'V-i-2', 'V-ii-1', 'V-ii-2', 'V-iii-1', 'V-iii-2', 'V-iv-1', 'V-iv-2', 'V-iv-3', 'V-iv-4', 'V-v-1', 'V-v-2', 'V-v-3', 'V-ref'];
const RESULT_COUNTS = { open: 1, converted: 6, view_only: 5, refused: 3 };
// 慎重さの順（開く 2 種は同順）。大きいほど慎重。
const CAUTION = { open: 0, converted: 0, view_only: 1, refused: 2 };

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
const urlState = (page) => page.evaluate(() => ({ href: location.href, hash: location.hash, length: history.length, state: history.state }));
// 読み込みの結果（lastImport）と、表示だけのモード（viewOnly・書き出しの2欄。TASK-176 1/2）以外の書き出し。
// 比べるのは文字列の一致の真偽だけにする。表示だけの3欄は下で明示して確かめる。
const comparable = (e) => {
  const { lastImport, viewOnly, exportGate, exportBytes, ...rest } = e;
  return JSON.stringify(rest);
};

// 進行が止まる（予約済みの宛先が無い）まで待つ。止まる前に比べると、古い進行の到着で揺れる。
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

// 保存領域の指紋（localStorage の鍵と値、IndexedDB の history ストアの鍵と値）。
// 値は要約（FNV-1a）にして返し、中身は失敗メッセージに出ない。DB が無ければ idb は null。
async function storageFingerprint(page) {
  return page.evaluate(async () => {
    const digest = (text) => {
      let h = 0x811c9dc5;
      for (let i = 0; i < text.length; i += 1) {
        h ^= text.charCodeAt(i);
        h = Math.imul(h, 0x01000193) >>> 0;
      }
      return `${text.length}:${h.toString(16)}`;
    };
    const local = Object.keys(localStorage).sort().map((k) => [k, digest(String(localStorage.getItem(k)))]);
    let idb = null;
    if (indexedDB.databases) {
      const dbs = await indexedDB.databases();
      if (dbs.some((d) => d.name === 'fandhe-workshop')) {
        idb = await new Promise((resolve) => {
          const req = indexedDB.open('fandhe-workshop');
          req.onerror = () => resolve('error');
          req.onsuccess = () => {
            const db = req.result;
            if (!db.objectStoreNames.contains('history')) { db.close(); resolve('no-store'); return; }
            const store = db.transaction('history').objectStore('history');
            const keysReq = store.getAllKeys();
            keysReq.onerror = () => { db.close(); resolve('error'); };
            keysReq.onsuccess = () => {
              const valuesReq = store.getAll();
              valuesReq.onerror = () => { db.close(); resolve('error'); };
              valuesReq.onsuccess = () => {
                db.close();
                resolve(keysReq.result.map((k, i) => [String(k), digest(JSON.stringify(valuesReq.result[i]) ?? '')]));
              };
            };
          };
        });
      }
    }
    return { local, idb };
  });
}

// 期待と結果の比較。食い違いは行 ID と固定の名前だけで書き、慎重な側とそれ以外に分ける。
function compareRow(row, got) {
  const want = row.expect;
  const gotReason = got.reason ?? null;
  const gotAxes = [...(got.axes ?? [])].sort();
  const wantAxes = [...want.axes].sort();
  if (got.verdict === want.result && gotReason === want.code && isDeepStrictEqual(gotAxes, wantAxes)) return null;
  const known = Object.hasOwn(CAUTION, got.verdict);
  const kind = known && CAUTION[got.verdict] > CAUTION[want.result] ? 'cautious' : 'other';
  const message = `${row.id}: expected ${want.result}/${want.code}/${wantAxes.join(',')}, got ${got.verdict}/${gotReason}/${gotAxes.join(',')}`;
  return { kind, message };
}

// 1 行を新しいページで開く。結果と、確認した真偽を返す（履歴の中身は返さない）。
async function openRow(srv, row) {
  const opened = await openPage(browser, { origin: srv.origin });
  const { page } = opened;
  await page.goto(srv.url);
  await waitMounted(page);
  await settle(page);

  // 開く側の教材と版の上書きを作る。上書きは null も含めて毎回送る（前の行の値を残さない）。
  const irText = fs.readFileSync(path.join(historiesDir, 'packs', `${row.opening.pack}.ir.json`), 'utf8');
  const installBeforeLoad = (await engine(page)).installCount;
  assert.equal(await call(page, 'loadIr', irText), true, `${row.id}: load-ir sent`);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, installBeforeLoad, { timeout: 15000 });
  assert.equal(await call(page, 'openingOverride', JSON.stringify({ engine: row.opening.engine, vocab_max: row.opening.vocab_max })), true, `${row.id}: override sent`);
  await settle(page);

  // 既存の保存データを置いた状態にする（拒否した履歴が上書きしても検知できるようにする）。
  await page.evaluate(() => localStorage.setItem('fandhe-test-sentinel', 'before'));
  const before = await engine(page);
  const stateBefore = comparable(before);
  const installBefore = before.installCount;
  const storeBefore = await storageFingerprint(page);
  const urlBefore = await urlState(page);

  const file = path.join(historiesDir, `${row.id}.history.json`);
  await call(page, 'importInstall');
  await page.setInputFiles(FILE_INPUT, file);
  await page.waitForFunction(() => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    return e && e.lastImport != null;
  }, null, { timeout: 15000 });

  const after = await engine(page);
  const got = after.lastImport;
  const out = { got, installDelta: after.installCount - installBefore };
  const isOpen = got.verdict === 'open' || got.verdict === 'converted';
  if (isOpen) {
    // 開いた直後の写しと履歴ファイルの snapshot の完全な一致（進行のタイマーが状態を進めても揺れない控え）。
    const snapshot = JSON.parse(fs.readFileSync(file, 'utf8')).snapshot;
    const restored = JSON.parse(await call(page, 'lastImportStateJson'));
    out.snapshotMatches = isDeepStrictEqual(restored, snapshot);
  } else {
    await settle(page);
    const e2 = await engine(page);
    const unchanged = comparable(e2) === stateBefore
      && e2.viewOnly === (got.verdict === 'view_only' ? got.reason : null)
      && isDeepStrictEqual(await storageFingerprint(page), storeBefore)
      && isDeepStrictEqual(await urlState(page), urlBefore)
      && (await call(page, 'lastImportStateJson')) === 'null';
    out.unchanged = unchanged;
  }
  assertQuiet(opened.rec, row.id);
  await opened.context.close();
  return out;
}

test('表の形: 15 件・ID の列・結果ごとの件数が VER-2 と一致する', () => {
  assert.equal(table.table_version, 1);
  assert.deepEqual(table.rows.map((r) => r.id), IDS);
  for (const [result, n] of Object.entries(RESULT_COUNTS)) {
    assert.equal(table.rows.filter((r) => r.expect.result === result).length, n, result);
  }
  for (const id of IDS) {
    assert.ok(fs.statSync(path.join(historiesDir, `${id}.history.json`), { throwIfNoEntry: false })?.isFile(), `${id}: history file exists`);
  }
});

test('比較器の自己確認: 期待を 1 つ書き換えると食い違いが出て、慎重さで分類される', () => {
  const row = structuredClone(table.rows[0]);
  assert.equal(row.id, 'V-0');
  const open = { verdict: 'open', reason: null, axes: [] };
  assert.equal(compareRow(row, open), null);
  // 開いた結果に対して、変換して開く期待に書き換える。
  const changed = { ...row, expect: { result: 'converted', code: null, axes: ['content'] } };
  assert.equal(compareRow(changed, open)?.kind, 'other');
  // 開く期待が表示だけを返された場合は慎重な側。
  assert.equal(compareRow(row, { verdict: 'view_only', reason: 'content_major', axes: [] })?.kind, 'cautious');
  // 軸だけの違いも拾う。
  const axisOnly = { ...table.rows.find((r) => r.expect.result === 'converted') };
  assert.ok(compareRow(axisOnly, { verdict: 'converted', reason: null, axes: [] }), 'a missing axis must mismatch');
});

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  test(`15 件を開く: 種類・理由・軸が表と合い、開く 7 件は写しが一致し、開かない 8 件は差し替えが 0 回（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const cautious = [];
      const other = [];
      let snapshotMatches = 0;
      let replaced = 0;
      let emptyReasons = 0;
      let openCount = 0;
      let closedCount = 0;
      let unchangedClosed = 0;
      for (const row of table.rows) {
        const r = await openRow(srv, row);
        const mismatch = compareRow(row, r.got);
        if (mismatch) (mismatch.kind === 'cautious' ? cautious : other).push(mismatch.message);
        const isOpen = r.got.verdict === 'open' || r.got.verdict === 'converted';
        if (isOpen) {
          openCount += 1;
          if (r.installDelta === 1 && r.snapshotMatches === true && r.got.reason == null) snapshotMatches += 1;
        } else {
          closedCount += 1;
          replaced += r.installDelta;
          if (!(typeof r.got.reason === 'string' && r.got.reason.length > 0)) emptyReasons += 1;
          if (r.unchanged === true) unchangedClosed += 1;
        }
      }
      assert.deepEqual(cautious, [], 'mismatches on the cautious side');
      assert.deepEqual(other, [], 'other mismatches');
      assert.equal(openCount, 7, 'seven histories open');
      assert.equal(snapshotMatches, 7, 'all seven restored states equal the saved snapshots');
      assert.equal(closedCount, 8, 'eight histories stay closed');
      assert.equal(replaced, 0, 'no session is built for a closed history');
      assert.equal(unchangedClosed, 8, 'state, storage and url are unchanged for a closed history');
      assert.equal(emptyReasons, 0, 'no closed history has an empty reason');
    }));
  });
}
