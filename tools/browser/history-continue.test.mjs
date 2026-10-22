// 受講の途中の履歴を別のオリジンで読み込み、続きを操作できることの試験（TASK-531 2/2。SHELL-15 の自動ブラウザの部分）。
//
// 役割: history-ops（1/2）が足場として通した画面の書き出し・読み込みを、実際の受講の記録（ホストが OpsRecorder で記録した ops）に対して使い、
//   (1) 別のオリジン（名前の違うサブパス・ルート配信）の別の context で同じ Step・世界状態・復習・ログ・チェックポイント・画面になること、
//   (2) 読み込んだ側で続きを操作すると書き出した側と同じ結果になること、(3) 同じ入力列の書き出しがバイト単位で一致することを確かめる。
// 入力: FANDHE_WEB_TEST_DIR（試験の設定。`?fixture=ch0-3`）。未設定・不在は skip にせず失敗にする。
// 方針: 受講は journey（実クリック・実入力）で行い、書き出しは「履歴を書き出す」ボタン → `Download`、読み込みは input[type=file] への
//   `setInputFiles` だけで通す。試験用の口の使用は、チェックポイントの保存（`raw-trigger` の `{control:'save'}`）の 1 種だけである
//   （チェックポイントの操作の画面は TASK-529 で、まだ無いため）。
// 確かめないこと: L2 の配布物（TASK-114）、macOS のファイル画面・公開サイト（TASK-342）、配信する設定（教材を持たない）、
//   チェックポイントの一覧の画面（TASK-529 2/2・TASK-548）。Chromium だけ。
// 試験の出力・失敗の文に、履歴の中身を出さない（真偽・件数・長さ・op の種類名だけを出す）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize } from './harness.mjs';
import { journey, mouseDriver, ledger, engine, at, RAW, LOG } from './ch0-3-journey.mjs';

const BASE_A = '/fandhe-continue-a/';
const BASE_B = '/another-repo-name/';
const BASE_ROOT = '/';
const URL_QUERY = '?fixture=ch0-3';
const EXPORT_BTN = '[data-testid="shell-export"]';
const LINK = '[data-testid="export-download"]';
const FILE_INPUT = '[data-testid="shell-import"] input[type="file"]';
const NOTICE_ITEM = '[data-testid="shell-notice-item"]';
const STOP = 'main/st-2-1';
let browser;
let tmpDir;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a built web directory');
  browser = await launchBrowser({ channel: channelFromEnv() });
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-history-continue-'));
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

const urlState = (page) => page.evaluate(() => ({ href: location.href, hash: location.hash, length: history.length, state: history.state }));
// 読み込みの結果（lastImport）と、読み込みで 1 増える試験の計数（installCount。状態ではなく、エンジンの組み込みの回数の観察）を除いた書き出し。
// 比べるのは文字列の一致の真偽だけにする。
const NOT_STATE = new Set(['lastImport', 'installCount']);
const comparable = (e) => {
  const { lastImport, installCount, ...rest } = e;
  return JSON.stringify(rest);
};

async function openOrigin(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url + URL_QUERY);
  await waitMounted(opened.page);
  return opened;
}

async function waitImport(page, verdict) {
  await page.waitForFunction(
    (v) => {
      const e = JSON.parse(window.__fandheTest.stateJson()).engine;
      return e && e.lastImport && e.lastImport.verdict === v;
    },
    verdict,
    { timeout: 15000 },
  );
}

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

// 画面の操作だけで書き出し、保存したファイルの経路・名前・中身を返す。
async function exportByUi(page) {
  await page.locator(EXPORT_BTN).click();
  const link = page.locator(LINK);
  await link.waitFor({ state: 'visible', timeout: 10000 });
  const [download] = await Promise.all([page.waitForEvent('download'), link.click()]);
  const file = path.join(tmpDir, `${Date.now()}-${Math.random().toString(16).slice(2)}.json`);
  await download.saveAs(file);
  return { file, name: download.suggestedFilename(), bytes: fs.readFileSync(file) };
}

// チェックポイントの保存（唯一の試験用の口の使用）。保存は世代を変えない。
async function saveCheckpoint(page) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify({ control: 'save' })]);
  await settle(page);
}

// CH-0・CH-1 を通って CH-2 の先頭の入力待ちまで受講する。2 か所でチェックポイントを保存する。
async function attend(page, scene) {
  const L = ledger();
  const saved = new Set();
  const onStep = async (uid, status) => {
    if (status === 'entered' && (uid === 'main/st-0-2b' || uid === 'main/st-1-4') && !saved.has(uid)) {
      saved.add(uid);
      await saveCheckpoint(page);
    }
  };
  await journey(page, mouseDriver(page), scene, L, STOP, { onStep });
  await settle(page);
  assert.deepEqual(L.failed(), [], `${scene}: journey ledger has no failure`);
  assert.equal(saved.size, 2, `${scene}: two checkpoints were saved`);
}

// 画面の層。値のまま返し、呼び出し側が一致の真偽だけを比べる。
const screenOf = (page) => page.evaluate((log) => {
  const text = (id) => document.querySelector(`[data-testid="${id}"]`)?.textContent ?? null;
  const count = (sel) => document.querySelectorAll(sel).length;
  return {
    title: text('shell-title'),
    position: text('shell-position'),
    logItems: [...document.querySelectorAll(`${log} li`)].map((li) => [li.getAttribute('data-step'), li.getAttribute('data-kind')]),
    rows: [...document.querySelectorAll('[data-testid="part-artifact-view"] [data-row]')].map((r) => [r.getAttribute('data-row'), r.textContent]),
    stepStatus: text('step-status'),
    runState: document.querySelector('[data-testid="run-status"]')?.getAttribute('data-state') ?? null,
    ports: [
      count('[data-testid="choice-btn"]'), count('[data-testid="approval-btn"]'),
      count('[data-testid="draft-input"]'), count('[data-testid="submit-input"]'),
    ],
  };
}, LOG);

// 状態の層で比べる欄。履歴の中身は出さず、一致の真偽だけを assert する。
function assertSameEngine(a, b, label) {
  for (const key of ['current', 'generation', 'runStatus', 'logLen']) {
    assert.equal(b[key], a[key], `${label}: same ${key}`);
  }
  assert.equal(JSON.stringify(b.log) === JSON.stringify(a.log), true, `${label}: same log`);
  assert.equal(JSON.stringify(b.stateWithoutLog) === JSON.stringify(a.stateWithoutLog), true, `${label}: same state`);
  assert.equal(JSON.stringify(b.review) === JSON.stringify(a.review), true, `${label}: same review`);
  assert.equal(JSON.stringify(b.artifacts) === JSON.stringify(a.artifacts), true, `${label}: same artifacts`);
  const differing = Object.keys({ ...a, ...b }).filter((k) => !NOT_STATE.has(k) && JSON.stringify(a[k]) !== JSON.stringify(b[k]));
  assert.deepEqual(differing, [], `${label}: engine fields that differ`);
  assert.equal(comparable(b) === comparable(a), true, `${label}: same engine snapshot`);
}

// バイト列の不一致の診断。中身は出さず、長さ・ops の件数・最初に違う op の位置と種類名だけを出す。
function diagnose(x, y) {
  try {
    const ox = JSON.parse(x.toString('utf8')).ops ?? [];
    const oy = JSON.parse(y.toString('utf8')).ops ?? [];
    let i = 0;
    while (i < Math.min(ox.length, oy.length) && JSON.stringify(ox[i]) === JSON.stringify(oy[i])) i += 1;
    const kind = (o) => (o ? Object.keys(o)[0] : 'none');
    return `bytes ${x.length} vs ${y.length}, ops ${ox.length} vs ${oy.length}, first diff at ${i} (${kind(ox[i])} vs ${kind(oy[i])})`;
  } catch {
    return `bytes ${x.length} vs ${y.length}`;
  }
}

function assertBytes(x, y, label) {
  assert.equal(Buffer.compare(x, y) === 0, true, `${label}: byte-identical (${diagnose(x, y)})`);
}

async function importAndOpen(page, file) {
  await page.setInputFiles(FILE_INPUT, file);
  await waitImport(page, 'open');
  await settle(page);
}

// 続きの操作。CH-2 の先頭の 4 手を、各ページに同じ順で行う。各手の後に、全ページが同じ Step になるのを待つ。
async function continueBoth(pages) {
  const drivers = pages.map((p) => mouseDriver(p));
  const each = (f) => Promise.all(drivers.map((d, i) => f(d, pages[i])));
  await each((d) => d.send('/new-idea ideas'));
  await each((_, p) => at(p, 'main/st-2-1', 'accepted'));
  await each((d) => d.point('docs/readme'));
  await each((_, p) => at(p, 'main/st-2-2', 'entered'));
  await each((d) => d.choose('opt-view'));
  await each((_, p) => at(p, 'main/st-2-2', 'accepted'));
  await each((d) => d.choose('ans-readme'));
  await each((_, p) => at(p, 'main/st-2-3', 'entered'));
}

test('受講の途中の履歴が別のオリジン（名前の違うサブパス）で同じ状態になり、続きを操作できる。書き出しはバイト単位で一致する', async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withServer({ dir, base: BASE_A }, (srvA) =>
    withServer({ dir, base: BASE_B }, async (srvB) => {
      assert.notEqual(srvA.origin, srvB.origin, 'two distinct origins');
      const A = await openOrigin(srvA);
      const B = await openOrigin(srvB);

      await attend(A.page, 'A');
      const baseline = await engine(A.page);
      const screenA = await screenOf(A.page);
      // 比較が空振りしないための前提。
      assert.ok(baseline.logLen > 0, 'the log is not empty');
      assert.ok(baseline.review.length > 0, 'review is not empty');
      assert.ok(baseline.artifacts.length > 0, 'artifacts are not empty');
      assert.equal(Object.keys(baseline.stateWithoutLog.checkpoints).length, 2, 'two checkpoints');

      const urlA = await urlState(A.page);
      const exportA = await exportByUi(A.page);
      assert.match(exportA.name, /^fandhe-workshop-history(-[a-z0-9-]+)?\.json$/, 'file name has the fixed shape');
      assert.deepEqual(await urlState(A.page), urlA, 'export does not change the URL');

      // B は別の状態から始まる。
      const initialB = await engine(B.page);
      assert.notEqual(comparable(initialB), comparable(baseline), 'B starts from a different state');
      const urlB = await urlState(B.page);
      await importAndOpen(B.page, exportA.file);

      const loaded = await engine(B.page);
      assert.equal(loaded.lastImport.reason, null, 'no refusal reason');
      assertSameEngine(baseline, loaded, 'loaded');
      assert.equal(loaded.worldMatchesState, true);
      assert.equal(loaded.incrementalMatches, true, 'derived views match a fresh derivation');
      assert.deepEqual(
        Object.keys(loaded.stateWithoutLog.checkpoints).sort(),
        Object.keys(baseline.stateWithoutLog.checkpoints).sort(),
        'same checkpoint list',
      );
      assert.equal(await B.page.locator(NOTICE_ITEM).count(), 0, 'no notice for a loadable file');
      // 画面の層（書き出しのリンクの blob: はオリジンごとに違うため含めない）。
      const screenB = await screenOf(B.page);
      assert.equal(JSON.stringify(screenB) === JSON.stringify(screenA), true, 'same screen');
      assert.ok(screenA.logItems.length > 0, 'the log list is not empty');

      // 往復: 読み込んだ履歴を書き出し直してもバイト列が変わらない。
      const exportB0 = await exportByUi(B.page);
      assertBytes(exportA.bytes, exportB0.bytes, 'round trip');

      // 続きの操作。
      await continueBoth([A.page, B.page]);
      const afterA = await engine(A.page);
      const afterB = await engine(B.page);
      assert.ok(afterB.logLen > loaded.logLen, 'the log grew');
      assert.notEqual(afterB.current, loaded.current, 'the step moved');
      assertSameEngine(afterA, afterB, 'after continuing');
      assert.equal(afterB.incrementalMatches, true);
      assert.equal(JSON.stringify(await screenOf(B.page)) === JSON.stringify(await screenOf(A.page)), true, 'same screen after continuing');

      const exportA1 = await exportByUi(A.page);
      const exportB1 = await exportByUi(B.page);
      assertBytes(exportA1.bytes, exportB1.bytes, 'after continuing');
      assert.equal(Buffer.compare(exportA1.bytes, exportA.bytes) !== 0, true, 'negative control: the export changed after continuing');

      const urlAfterB = await urlState(B.page);
      assert.equal(urlAfterB.hash, '', 'no state in the URL');
      assert.equal(urlAfterB.href, urlB.href, 'B URL unchanged');
      assertQuiet(A.rec, 'origin A');
      assertQuiet(B.rec, 'origin B');
      await A.context.close();
      await B.context.close();
    }),
  );
});

test('同じ入力列の書き出しが、名前の違うサブパスとルート配信でバイト単位で一致する', async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withServer({ dir, base: BASE_A }, (srvA) =>
    withServer({ dir, base: BASE_ROOT }, async (srvR) => {
      assert.notEqual(srvA.origin, srvR.origin, 'two distinct origins');
      const A = await openOrigin(srvA);
      const R = await openOrigin(srvR);
      await attend(A.page, 'A');
      await attend(R.page, 'R');
      const exportA = await exportByUi(A.page);
      const exportR = await exportByUi(R.page);
      assert.equal(exportR.name, exportA.name, 'same suggested file name');
      assertBytes(exportA.bytes, exportR.bytes, 'subpath vs root');
      assertQuiet(A.rec, 'origin A');
      assertQuiet(R.rec, 'origin R');
      await A.context.close();
      await R.context.close();
    }),
  );
});

test('サブパスで書き出した履歴をルート配信で読み込んでも同じ状態になる', async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withServer({ dir, base: BASE_A }, (srvA) =>
    withServer({ dir, base: BASE_ROOT }, async (srvR) => {
      const A = await openOrigin(srvA);
      const R = await openOrigin(srvR);
      await attend(A.page, 'A');
      const baseline = await engine(A.page);
      const exportA = await exportByUi(A.page);
      await importAndOpen(R.page, exportA.file);
      assertSameEngine(baseline, await engine(R.page), 'root loaded');
      const exportR = await exportByUi(R.page);
      assertBytes(exportA.bytes, exportR.bytes, 'subpath to root round trip');
      assertQuiet(A.rec, 'origin A');
      assertQuiet(R.rec, 'origin R');
      await A.context.close();
      await R.context.close();
    }),
  );
});
