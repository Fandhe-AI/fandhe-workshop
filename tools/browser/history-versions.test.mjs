// 保存・復元・書き出し・版の層 3 ブラウザテスト（TASK-86 2/2。BUILD-10。PoC-6 の S-10・S-11・S-13〜S-15 と S-3・S-4 の一部）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。画面の保存・復帰・読み込みの UI はまだ無いため、
// 試験の設定（TEST_HOOKS=1）の口（`__fandheTest` の `exportPrepareLoadable`・`importInstall`・`stateJson` と、
// `raw-trigger` へ書く `Op` の JSON）から操作する。判定の正は core の `history_open`・`history_load`
// （理由の順序は contract の `load_reason`）で、ここは web を通した結果（`stateJson().engine.lastImport`）だけを確かめる。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
// 確かめること:
//   P-1 往復（別オリジン A の書き出しを B が読み込み、続きの操作も同じ結果。CSP あり・なし）
//   P-2 読み込んだ後の保存と復帰（S-3・S-4 の、再読み込みを除いた縦断）
//   P-3 版の違い（S-13〜S-15 相当。変換・表示だけ・開かない。拒否・表示だけでは状態が変わらない）
//   P-4 壊れた履歴の拒否理由（S-11 相当。状態・保存領域・URL が変わらない）
//   P-5 書き出しの決定性（同じ包みを 2 回書き出すとバイト列が一致する）
//   P-6 配信設定には試験の口が無い
// 版の値は基準のファイルの `versions` から読み、試験に直書きしない。
// 覆わない場面と理由:
//   S-1〜S-9・S-12・S-19: 実際の再読み込みでの復元。web に起動時の復元の結線が無い（TASK-319 の範囲）
//   S-10 の「B の保存領域に履歴がある」: 読み込みは保存先へ書かない（TASK-319）。ここは「書かない」ことを確かめる
//   受講中の操作の書き出し: ホストが受講中の ops を記録しない。契約の試料の開ける包みが代わりを務める
//   S-14・S-15 の「表示だけで開き操作を受け付けない」画面: web は view_only で状態を変えず、表示だけの画面が無い
//     （試作との差）。ここは今の挙動（状態不変）を固定する
//   engine_major: 開く側のエンジンの版が 0.0.0 で、履歴の側でこれより小さい主版を作れない（大きくすると engine_newer に先に当たる）
//   S-16（保存の遅延）・S-17（容量超過の表示）・S-18（IndexedDB の 20MB）: それぞれ別タスク・表示経路なし・記録だけの場面
// 試験の出力・失敗の文に、履歴の中身を出さない（受講者のデータのため。真偽・件数・固定のコードだけを比べる）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-history-versions/';
const RAW = '[data-testid="raw-trigger"]';
const FILE_INPUT = '[data-testid="history-file"]';
const NEXT_OP = { control: { go_to: 'main/st-2' } };
const SAVE = { control: 'save' };
const restoreOp = (id) => ({ control: { restore: id } });
// contract の limits と同じ値。変わったら P-4 の B-6〜B-8 が失敗して気づく。
const MAX_HISTORY_BYTES = 10_485_760;
const MAX_JSON_DEPTH = 64;
const MAX_HISTORY_OPS = 20_000;

let browser;
let tmpDir;

before(async () => {
  for (const name of ['FANDHE_WEB_DIR', 'FANDHE_WEB_TEST_DIR']) {
    const dir = process.env[name];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a built web directory`);
  }
  browser = await launchBrowser({ channel: channelFromEnv() });
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-history-versions-'));
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);
const engine = async (page) => JSON.parse(await call(page, 'stateJson')).engine;
const urlState = (page) => page.evaluate(() => ({ href: location.href, hash: location.hash, length: history.length, state: history.state }));
// 読み込みの結果（lastImport）以外の書き出し。比べるのは文字列の一致の真偽だけにする。
const comparable = (e) => {
  const { lastImport, ...rest } = e;
  return JSON.stringify(rest);
};
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const kindCount = (e, kind) => e.log.filter((r) => r.kind === kind).length;

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

// 判定（verdict と reason の両方）が揃うまで待つ。ページは判定ごとに開き直すので、前の結果で抜けることはない。
async function waitImport(page, verdict, reason) {
  try {
    await waitImportRaw(page, verdict, reason);
  } catch (err) {
    // 失敗の文には固定の判定コードだけを添える（履歴の中身は出さない）。
    const last = (await engine(page)).lastImport;
    throw new Error(`import verdict mismatch: want ${verdict}/${reason}, got ${last?.verdict}/${last?.reason ?? null}`, { cause: err });
  }
}

async function waitImportRaw(page, verdict, reason) {
  await page.waitForFunction(
    ([v, r]) => {
      const e = JSON.parse(window.__fandheTest.stateJson()).engine;
      return e && e.lastImport && e.lastImport.verdict === v && (e.lastImport.reason ?? null) === r;
    },
    [verdict, reason],
    { timeout: 15000 },
  );
}

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

async function openOrigin(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

async function openSettled(srv) {
  const opened = await openOrigin(srv);
  await settle(opened.page);
  return opened;
}

// 保存領域の鍵の一覧（localStorage の鍵と、IndexedDB の history ストアの鍵の数。DB が無ければ null）。
async function storageFingerprint(page) {
  return page.evaluate(async () => {
    const local = Object.keys(localStorage).sort();
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
            const c = db.transaction('history').objectStore('history').count();
            c.onsuccess = () => { db.close(); resolve(c.result); };
            c.onerror = () => { db.close(); resolve('error'); };
          };
        });
      }
    }
    return { local, idb };
  });
}

async function withTwoOrigins(csp, fn) {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withCspMode({ dir, csp }, (dirA) => withCspMode({ dir, csp }, (dirB) =>
    withServer({ dir: dirA, base: BASE }, (a) => withServer({ dir: dirB, base: BASE }, (b) => fn(a, b)))));
}

let fileSeq = 0;
function tmpFile(name) {
  fileSeq += 1;
  return path.join(tmpDir, `${fileSeq}-${name}`);
}

// オリジン A の開ける包みを書き出して保存先を返す。
async function exportFromA(a) {
  const prepared = JSON.parse(await call(a.page, 'exportPrepareLoadable'));
  assert.equal(prepared.ok, true, 'loadable export must be prepared');
  const [download] = await Promise.all([a.page.waitForEvent('download'), call(a.page, 'exportClick', prepared.url, prepared.name)]);
  const file = tmpFile('base.json');
  await download.saveAs(file);
  assert.equal(fs.statSync(file).size, prepared.bytes, 'downloaded length matches the prepared length');
  return file;
}

// 基準のファイルを書き換えた写しを書く。
function variant(baseFile, name, mutate) {
  const value = JSON.parse(fs.readFileSync(baseFile, 'utf8'));
  mutate(value);
  const file = tmpFile(name);
  fs.writeFileSync(file, JSON.stringify(value));
  return file;
}

// 基準の状態を持つ A から基準のファイルを作る。
async function makeBaseline(srvA) {
  const A = await openSettled(srvA);
  const text = await call(A.page, 'loadableHistoryText');
  assert.ok(text.length > 0, 'loadable history exists');
  assert.equal(await call(A.page, 'importText', text), true);
  await waitImport(A.page, 'open', null);
  await settle(A.page);
  const baseline = await engine(A.page);
  const file = await exportFromA(A);
  return { A, baseline, file };
}

async function importFile(page, file) {
  await call(page, 'importInstall');
  await page.setInputFiles(FILE_INPUT, file);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  test(`P-1 往復: A の書き出しを B が読み込むと同じ状態になり、続きの操作も同じ結果になる（CSP ${label}）`, async () => {
    await withTwoOrigins(csp, async (srvA, srvB) => {
      assert.notEqual(srvA.origin, srvB.origin);
      const { A, baseline, file } = await makeBaseline(srvA);
      const B = await openSettled(srvB);
      const urlBefore = await urlState(B.page);
      await importFile(B.page, file);
      await waitImport(B.page, 'open', null);
      await settle(B.page);
      const loaded = await engine(B.page);
      assert.equal(loaded.current, baseline.current);
      assert.equal(loaded.generation, baseline.generation);
      assert.equal(same(loaded.stateWithoutLog, baseline.stateWithoutLog), true, 'same state');
      assert.equal(same(loaded.log, baseline.log), true, 'same log');
      assert.equal(same(loaded.review, baseline.review), true, 'same review');
      assert.equal(same(loaded.artifacts, baseline.artifacts), true, 'same artifacts');
      assert.equal(loaded.incrementalMatches, true);
      await sendRaw(A.page, NEXT_OP);
      await sendRaw(B.page, NEXT_OP);
      await settle(A.page);
      await settle(B.page);
      const nextA = await engine(A.page);
      const nextB = await engine(B.page);
      assert.notEqual(nextB.current, loaded.current, 'the step moved');
      assert.equal(comparable(nextA) === comparable(nextB), true, 'A and B agree after the next operation');
      assert.deepEqual(await urlState(B.page), urlBefore, 'url and history unchanged');
      assertQuiet(A.rec, 'A');
      assertQuiet(B.rec, 'B');
      await A.context.close();
      await B.context.close();
    });
  });
}

test('P-2 読み込んだ後に保存し、先へ進んでから復帰すると保存時の状態へ戻る', async () => {
  await withTwoOrigins(true, async (srvA, srvB) => {
    const { A, file } = await makeBaseline(srvA);
    const B = await openSettled(srvB);
    await importFile(B.page, file);
    await waitImport(B.page, 'open', null);
    await settle(B.page);
    const loaded = await engine(B.page);
    const id = loaded.stateWithoutLog.next_checkpoint;
    await sendRaw(B.page, SAVE);
    await settle(B.page);
    const saved = await engine(B.page);
    assert.equal(kindCount(saved, 'checkpoint_saved') - kindCount(loaded, 'checkpoint_saved'), 1, 'one checkpoint saved');
    assert.equal(Object.hasOwn(saved.stateWithoutLog.checkpoints, String(id)), true, 'the new checkpoint exists');
    await sendRaw(B.page, NEXT_OP);
    await settle(B.page);
    const moved = await engine(B.page);
    assert.notEqual(moved.current, saved.current, 'moved ahead');
    await sendRaw(B.page, restoreOp(id));
    await settle(B.page);
    const restored = await engine(B.page);
    assert.equal(kindCount(restored, 'restored') - kindCount(moved, 'restored'), 1, 'one restored entry');
    assert.ok(restored.generation > saved.generation, 'generation advanced');
    assert.equal(restored.current, saved.current, 'same step as saved');
    const noGen = (run) => ({ ...run, generation: undefined });
    assert.equal(same(noGen(restored.stateWithoutLog.run), noGen(saved.stateWithoutLog.run)), true, 'same run');
    assert.equal(same(restored.stateWithoutLog.entry, saved.stateWithoutLog.entry), true, 'same entry');
    assert.equal(same(restored.review, saved.review), true, 'same review');
    assert.equal(restored.incrementalMatches, true);
    assertQuiet(B.rec, 'B');
    assertQuiet(A.rec, 'A');
    await A.context.close();
    await B.context.close();
  });
});

// 版・壊れた履歴の試験で共通の流れ。判定ごとにページを開き直し、期待の判定と不変条件を確かめる。
async function expectImport(srvB, baseline, { name, file, verdict, reason, unchanged }) {
  const B = await openSettled(srvB);
  const before = comparable(await engine(B.page));
  const storeBefore = await storageFingerprint(B.page);
  const urlBefore = await urlState(B.page);
  await importFile(B.page, file);
  await waitImport(B.page, verdict, reason);
  await settle(B.page);
  const after = await engine(B.page);
  assert.equal(after.lastImport.verdict, verdict, `${name}: verdict`);
  assert.equal(after.lastImport.reason ?? null, reason, `${name}: reason`);
  if (unchanged) assert.equal(comparable(after) === before, true, `${name}: state unchanged`);
  else {
    assert.equal(comparable(after) === comparable(baseline), true, `${name}: same as baseline`);
    assert.equal(after.incrementalMatches, true, `${name}: derived views match`);
  }
  assert.equal(same(await storageFingerprint(B.page), storeBefore), true, `${name}: storage unchanged`);
  assert.deepEqual(await urlState(B.page), urlBefore, `${name}: url unchanged`);
  assertQuiet(B.rec, name);
  await B.context.close();
}

// 版の文字列（content・engine は 主.副.修）の主版または副版を 1 つ上げる。成分数は保つ。
const bump = (v, part) => {
  const parts = v.split('.').map(Number);
  parts[part === 'major' ? 0 : 1] += 1;
  return parts.join('.');
};

test('P-3 版の違い: 変換・表示だけ・開かないが期待どおりで、開かないときは状態が変わらない', async () => {
  await withTwoOrigins(true, async (srvA, srvB) => {
    const { A, baseline, file } = await makeBaseline(srvA);
    const versions = JSON.parse(fs.readFileSync(file, 'utf8')).versions;
    for (const k of ['content', 'content_hash', 'engine', 'ir_format', 'vocab']) assert.ok(versions[k] !== undefined, `versions.${k} exists`);
    const hash = (versions.content_hash.startsWith('0') ? '1' : '0') + versions.content_hash.slice(1);
    const cases = [
      { name: 'V-1 content の副版 +1', verdict: 'converted', reason: null, unchanged: false, mutate: (v) => { v.versions.content = bump(v.versions.content, 'minor'); } },
      { name: 'V-2 content_hash の違い', verdict: 'converted', reason: null, unchanged: false, mutate: (v) => { v.versions.content_hash = hash; } },
      { name: 'V-3 content の主版 +1', verdict: 'view_only', reason: 'content_major', unchanged: true, mutate: (v) => { v.versions.content = bump(v.versions.content, 'major'); } },
      { name: 'V-4 content_hash と snapshot の違い', verdict: 'view_only', reason: 'replay_diverged', unchanged: true, mutate: (v) => { v.versions.content_hash = hash; v.snapshot.next_checkpoint = 9999; } },
      { name: 'V-5 ir_format +1', verdict: 'refused', reason: 'format_newer', unchanged: true, mutate: (v) => { v.versions.ir_format += 1; } },
      { name: 'V-6 engine の副版 +1', verdict: 'refused', reason: 'engine_newer', unchanged: true, mutate: (v) => { v.versions.engine = bump(v.versions.engine, 'minor'); } },
      { name: 'V-7 vocab の主版 +99', verdict: 'refused', reason: 'vocab_newer', unchanged: true, mutate: (v) => { v.versions.vocab = `${Number(String(v.versions.vocab).split('.')[0]) + 99}.0`; } },
      { name: 'V-8 content が読めない書式', verdict: 'refused', reason: 'bad_version', unchanged: true, mutate: (v) => { v.versions.content = 'x.y'; } },
    ];
    if (versions.ir_format >= 1) {
      cases.push({ name: 'V-9 ir_format -1', verdict: 'view_only', reason: 'format_older', unchanged: true, mutate: (v) => { v.versions.ir_format -= 1; } });
    }
    for (const c of cases) {
      await expectImport(srvB, baseline, { ...c, file: variant(file, 'v.json', c.mutate) });
    }
    // 拒否の後でも、基準のファイルは開ける。
    await expectImport(srvB, baseline, { name: 'base', file, verdict: 'open', reason: null, unchanged: false });
    assertQuiet(A.rec, 'A');
    await A.context.close();
  });
});

test('P-4 壊れた履歴は開かず、状態・保存領域・URL を変えない', async () => {
  await withTwoOrigins(true, async (srvA, srvB) => {
    const { A, baseline, file } = await makeBaseline(srvA);
    const raw = fs.readFileSync(file, 'utf8');
    const truncated = tmpFile('truncated.json');
    fs.writeFileSync(truncated, raw.slice(0, Math.floor(raw.length / 2)));
    const tooLarge = tmpFile('too-large.json');
    fs.writeFileSync(tooLarge, ' '.repeat(MAX_HISTORY_BYTES + 1));
    const tooDeep = tmpFile('too-deep.json');
    fs.writeFileSync(tooDeep, '['.repeat(MAX_JSON_DEPTH + 1) + ']'.repeat(MAX_JSON_DEPTH + 1));
    const cases = [
      { name: 'B-1 末尾が切れている', file: truncated, reason: 'truncated' },
      { name: 'B-2 versions が無い', file: variant(file, 'b2.json', (v) => { delete v.versions; }), reason: 'missing_version' },
      { name: 'B-3 history_format の型違い', file: variant(file, 'b3.json', (v) => { v.history_format = 'x'; }), reason: 'malformed' },
      { name: 'B-4 別の教材', file: variant(file, 'b4.json', (v) => { v.pack_id = `${v.pack_id}-other`; }), reason: 'other_pack' },
      { name: 'B-5 history_format が新しい', file: variant(file, 'b5.json', (v) => { v.history_format = 999; }), reason: 'history_format_newer' },
      { name: 'B-6 大きさの上限超え', file: tooLarge, reason: 'too_large' },
      { name: 'B-7 入れ子の上限超え', file: tooDeep, reason: 'too_deep' },
      { name: 'B-8 ops の件数の上限超え', file: variant(file, 'b8.json', (v) => { v.ops = Array.from({ length: MAX_HISTORY_OPS + 1 }, () => v.ops[0]); }), reason: 'too_many_ops' },
    ];
    for (const c of cases) {
      await expectImport(srvB, baseline, { ...c, verdict: 'refused', unchanged: true });
    }
    await expectImport(srvB, baseline, { name: 'base', file, verdict: 'open', reason: null, unchanged: false });
    assertQuiet(A.rec, 'A');
    await A.context.close();
  });
});

test('P-5 同じ包みの書き出しはバイト列が一致する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const A = await openSettled(srv);
    const first = await exportFromA(A);
    const second = await exportFromA(A);
    assert.equal(fs.readFileSync(first).equals(fs.readFileSync(second)), true, 'export is deterministic');
    assertQuiet(A.rec, 'A');
    await A.context.close();
  });
});

test('P-6 配信設定には試験の口が無い', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openOrigin(srv);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    assert.equal(await page.locator(FILE_INPUT).count(), 0);
    assert.equal(await page.locator(RAW).count(), 0);
    assertQuiet(rec, 'dist');
    await context.close();
  });
});
