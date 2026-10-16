// 履歴のファイルからの読み込み（TASK-330 2/2、SAVE-16・SHELL-15 の自動ブラウザの部分）の試験。
//
// 入力: FANDHE_WEB_TEST_DIR（試験の設定。`exportPrepareLoadable`・`importInstall`・`importText` を持つ）。未設定・不在は skip にせず失敗にする。
// 構成: 配信を 2 つ起動する（ポートが違うため別のオリジン）。オリジン A で履歴を書き出してダウンロードし、別の context のオリジン B で
// そのファイルを `setInputFiles` で選ぶ。HTTP のサーバ・ブラウザの context は A と B で共有しない。
// 確かめること: (1) B が A と同じ状態になり、続きの操作でも A と同じ結果になる、(2) 読み込みで外部への要求・CSP 違反・ページのエラーが 0 件、
// (3) URL（location・履歴・ハッシュ）に状態が載らない、(4) 保存領域はオリジンごとに分かれ、持ち運びはファイルだけ、
// (5) 書き換えたファイル・JSON でないファイルは開かず、状態を変えない。
// 開ける包み: 受講中の `ops` の記録がホストに無い間は、契約の試料の `ops` を適用し直したものを使う（受講中の記録ではない）。
// 限界: 実際の GitHub Pages の間と、macOS のファイル画面での確認（SAVE-30）は人が行う。Chromium だけで確かめる。
// 試験の出力・失敗の文に、履歴の中身を出さない（受講者のデータのため。真偽・件数・固定のコードだけを比べる）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize } from './harness.mjs';

const BASE = '/fandhe-import/';
const RAW = '[data-testid="raw-trigger"]';
const FILE_INPUT = '[data-testid="history-file"]';
// 読み込んだ後の続きの操作。いまの Step の次へ移る（イベントが増え、世代と現在の Step が変わる）。
const NEXT_OP = { control: { go_to: 'main/st-2' } };
let browser;
let tmpDir;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a built web directory');
  browser = await launchBrowser({ channel: channelFromEnv() });
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-history-import-'));
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);
const engine = async (page) => JSON.parse(await call(page, 'stateJson')).engine;
const urlState = (page) => page.evaluate(() => ({ href: location.href, hash: location.hash, length: history.length, state: history.state }));

// 読み込みの結果（lastImport）以外の書き出し。履歴の中身を出さないよう、比べるのは文字列の一致の真偽だけにする。
const comparable = (e) => {
  const { lastImport, ...rest } = e;
  return JSON.stringify(rest);
};

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

async function waitImport(page, verdict) {
  await page.waitForFunction(
    (v) => {
      const e = JSON.parse(window.__fandheTest.stateJson()).engine;
      return e && e.lastImport && e.lastImport.verdict === v;
    },
    verdict,
    { timeout: 10000 },
  );
}

// 進行が止まる（予約済みの宛先が無い）まで待つ。止まる前に比べると、古い進行の到着で揺れる。
async function settle(page) {
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null', null, { timeout: 15000 });
}

async function writeStorage(page, pack, start, value) {
  await call(page, 'storageOpen');
  await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [pack, start], { timeout: 10000 });
  return JSON.parse(await call(page, 'storageWrite', pack, start, value));
}

async function openStorage(page, pack, start) {
  await call(page, 'storageOpen');
  await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [pack, start], { timeout: 10000 });
}

// 2 つのオリジンの配信を起動して fn に渡す。
async function withTwoOrigins(fn) {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withServer({ dir, base: BASE }, (a) => withServer({ dir, base: BASE }, (b) => fn(a, b)));
}

async function openOrigin(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

// オリジン A で開ける包みを取り込んで基準の状態を作り、書き出したファイルの保存先（一時ディレクトリ内）を返す。
async function exportFromA(a) {
  const prepared = JSON.parse(await call(a.page, 'exportPrepareLoadable'));
  assert.equal(prepared.ok, true, 'loadable export must be prepared');
  const [download] = await Promise.all([a.page.waitForEvent('download'), call(a.page, 'exportClick', prepared.url, prepared.name)]);
  const file = path.join(tmpDir, `${Date.now()}-${Math.random().toString(16).slice(2)}.json`);
  await download.saveAs(file);
  assert.equal(fs.statSync(file).size, prepared.bytes, 'downloaded length matches the prepared length');
  return file;
}

function assertQuiet(rec, label) {
  const s = summarize(rec);
  assert.equal(s.external, 0, `${label}: no external request`);
  assert.equal(s.csp, 0, `${label}: no CSP violation`);
  assert.equal(s.pageErrors, 0, `${label}: no page error`);
  assert.equal(s.dialogs, 0, `${label}: no dialog`);
  assert.equal(s.popups, 0, `${label}: no popup`);
}

test('別のオリジンで同じ状態になり、続きを操作できる。保存領域は共有されず、URL にも状態が載らない', async () => {
  await withTwoOrigins(async (srvA, srvB) => {
    assert.notEqual(srvA.origin, srvB.origin, 'two distinct origins');
    const A = await openOrigin(srvA);
    const B = await openOrigin(srvB);

    // A: 古い進行の到着で比較が揺れないよう、予約が無いことを確かめてから基準を作る。
    assert.equal(await call(A.page, 'nextTick'), 'null');
    const text = await call(A.page, 'loadableHistoryText');
    assert.ok(text.length > 0, 'loadable history exists');
    assert.equal(await call(A.page, 'importText', text), true);
    await waitImport(A.page, 'open');
    await settle(A.page);
    const baseline = await engine(A.page);
    assert.ok(baseline.review.length > 0, 'baseline has a review');
    assert.ok(baseline.artifacts.length > 0, 'baseline has artifacts');
    assert.ok(Object.keys(baseline.stateWithoutLog.checkpoints).length > 0, 'baseline has checkpoints');
    assert.ok(baseline.logLen > 0, 'baseline has a log');

    // 保存領域はオリジンごとに分かれる。A に書いた値は B から見えない（持ち運びはファイルだけ）。
    const written = await writeStorage(A.page, 'pack-a', 'start-a', 'marker');
    assert.equal(written.ok, true, 'storage write in A');
    assert.equal(await call(A.page, 'storageRead', 'pack-a', 'start-a'), 'marker', 'A reads its own value');
    await openStorage(B.page, 'pack-a', 'start-a');
    assert.equal(await call(B.page, 'storageRead', 'pack-a', 'start-a'), undefined, 'B does not see the storage of A');

    // A でファイルに書き出し、B で選ぶ。
    const file = await exportFromA(A);
    const urlBefore = await urlState(B.page);
    const initialB = await engine(B.page);
    assert.notEqual(comparable(initialB), comparable(baseline), 'B starts from a different state');
    await call(B.page, 'importInstall');
    await B.page.setInputFiles(FILE_INPUT, file);
    await waitImport(B.page, 'open');
    await settle(B.page);

    // 同じ Step・世界状態・復習・ログ・チェックポイントの一覧になる。
    const loaded = await engine(B.page);
    assert.equal(loaded.lastImport.reason, null);
    assert.equal(loaded.current, baseline.current, 'same step');
    assert.equal(loaded.generation, baseline.generation, 'same generation');
    assert.equal(loaded.runStatus, baseline.runStatus);
    assert.equal(loaded.logLen, baseline.logLen, 'same log length');
    assert.equal(JSON.stringify(loaded.stateWithoutLog) === JSON.stringify(baseline.stateWithoutLog), true, 'same state (world, checkpoints, drafts)');
    assert.equal(JSON.stringify(loaded.log) === JSON.stringify(baseline.log), true, 'same log');
    assert.equal(JSON.stringify(loaded.review) === JSON.stringify(baseline.review), true, 'same review');
    assert.equal(JSON.stringify(loaded.artifacts) === JSON.stringify(baseline.artifacts), true, 'same artifacts');
    assert.equal(comparable(loaded) === comparable(baseline), true, 'same engine snapshot');
    assert.equal(loaded.worldMatchesState, true);
    assert.equal(loaded.incrementalMatches, true, 'derived views match a fresh derivation');

    // 続きの操作で A と B が同じ結果になる。
    await sendRaw(A.page, NEXT_OP);
    await sendRaw(B.page, NEXT_OP);
    await settle(A.page);
    await settle(B.page);
    const nextA = await engine(A.page);
    const nextB = await engine(B.page);
    assert.ok(nextB.logLen > loaded.logLen, 'the log grew after the next operation');
    assert.notEqual(nextB.current, loaded.current, 'the step moved');
    assert.equal(comparable(nextB) === comparable(nextA), true, 'A and B agree after the next operation');
    assert.equal(nextB.incrementalMatches, true);

    // URL に状態が載らない（読み込みの前後で location・履歴が同じ）。
    const urlAfter = await urlState(B.page);
    assert.deepEqual(urlAfter, urlBefore, 'location and history are unchanged by the import');
    assert.equal(urlAfter.hash, '');

    // 読み込みは保存先へ書かない（保存の結線は TASK-319）。B の保存領域は空のまま。
    assert.equal(await call(B.page, 'storageRead', 'pack-a', 'start-a'), undefined);

    // 外部への通信（相手のオリジンへのものを含む）・CSP 違反・ページのエラーが 0 件。
    assertQuiet(A.rec, 'A');
    assertQuiet(B.rec, 'B');
    await A.context.close();
    await B.context.close();
  });
});

test('書き換えたファイル・JSON でないファイルは開かず、状態を変えない', async () => {
  await withTwoOrigins(async (srvA, srvB) => {
    const A = await openOrigin(srvA);
    const B = await openOrigin(srvB);
    const file = await exportFromA(A);
    const tampered = path.join(tmpDir, 'tampered.json');
    const value = JSON.parse(fs.readFileSync(file, 'utf8'));
    value.snapshot.next_checkpoint = 9999;
    fs.writeFileSync(tampered, JSON.stringify(value));
    const notJson = path.join(tmpDir, 'not-json.json');
    fs.writeFileSync(notJson, 'this is not json');

    await settle(B.page);
    const untouched = comparable(await engine(B.page));
    await call(B.page, 'importInstall');

    await B.page.setInputFiles(FILE_INPUT, tampered);
    await waitImport(B.page, 'refused');
    const first = await engine(B.page);
    assert.equal(first.lastImport.reason, 'replay_mismatch');
    assert.equal(comparable(first) === untouched, true, 'state is unchanged by a tampered file');

    await B.page.setInputFiles(FILE_INPUT, notJson);
    await B.page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.lastImport.reason === 'not_json', null, { timeout: 10000 });
    const second = await engine(B.page);
    assert.equal(second.lastImport.verdict, 'refused');
    assert.equal(comparable(second) === untouched, true, 'state is unchanged by a non-JSON file');

    // 開けないファイルのあとでも、開けるファイルは開ける。
    await B.page.setInputFiles(FILE_INPUT, file);
    await waitImport(B.page, 'open');
    assert.equal((await engine(B.page)).incrementalMatches, true);

    assertQuiet(B.rec, 'B');
    await A.context.close();
    await B.context.close();
  });
});
