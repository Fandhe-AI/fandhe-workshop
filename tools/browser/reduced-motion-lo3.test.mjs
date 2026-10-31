// reduce の文脈で、教材の文面による補いの経路と補う場面が no-preference と同じに働くことの試験
// （TASK-378 2/2。A11Y-12・A11Y-13・INTRO-20。決定は D-82）。CSP あり・なしの両方で流す。
//
// D-82 は、エンジンが reduce のために処理中の停止（LO-3）を体験させる仕組みも、教材内の動きの設定も足さず、LO-3 は他の場面と
// 教材の文面で補うと決めた（動きの設定が無いことは native の試験 a11y12_*。IR の側は reduced_motion_contract.rs）。
// ここでは、補いに使う経路（Step の題・do・本文）と、補う場面が reduce でも変わらないことを実際の画面で確かめる。確かめること:
//   G. 最小教材の st-1 の本文に中立の印の段落を足して compile し、reduce と no-preference で開く。題・do・本文の文字が両文脈で
//      一致し、印の段落が本文に 1 件あって、アクセシビリティツリーにも出る。処理中（busy）の全標本でも変わらず、本文の要素は
//      置き換わらない。通信・違反・実行は 0 件
//   H. 対照と余計な案内の不在: 印を入れない写しでは印が 0 件と数えられる。reduce の画面に reduce だけの部品・ボタンが無い
//      （段階の行を除いた data-testid の集合とボタンの数が no-preference と一致する）
//   J1. チェックポイントの保存 → 先へ進む → 復帰（ST-1.6 の場面）が reduce でも no-preference と同じ結果になる
//   J2. 差し戻し → 再承認（ST-2.3 に当たる場面。`ch0-3-reapprove` の st-2-6）が reduce でも同じ結果になる
// 確かめないこと: 補いの文面の採否・形（INTRO-20 の案。TASK-607）、再実行による再開の場面（ST-7.3・ST-13.2）、早送り（SHELL-29）、
// 実機の OS 設定や支援技術での確認（自動ブラウザの prefers-reduced-motion は模擬）。コミットする教材の文面は変えない。
// 期待値は試験の定数で独自に持つ。書き込むパスは一時ディレクトリの固定のパスだけで、失敗の文面に本文の中身は出さない。
// 待ちは固定の sleep ではなく、上限つきのポーリングで書く。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_WEB_DIR・FANDHE_PACK_BIN。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  ledger, mouseDriver, journey, engine,
} from './ch0-3-journey.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const BASE = '/fandhe-reduced-motion-lo3/';
const PACK = path.resolve(HERE, '../../testdata/engine-event-consistency/test-data/min-3step-pack');
const MARK = '試験用の補いの段落の印';
const BODY_MD = ['# 補いの試験', '', MARK, ''].join('\n');
const COMMAND = '/make-note first-note';
const MEDIA = ['reduce', 'no-preference'];

const STATUS = '[data-testid="run-status"]';
const DRAFT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const BODY = '[data-testid="step-body"]';
const LOG = '[data-testid="part-log-list"]';
const PROGRESS_LI = `${LOG} li[data-kind="progress"]`;
const SAVE = '[data-testid="checkpoint-save"]';
const RESTORE = '[data-testid="checkpoint-restore"]';
const POSITION = '[data-testid="shell-position"]';
const ROWS = '[data-testid="part-artifact-view"] [data-row]';
const BTN = '[data-testid="approval-btn"]';
const THREE = ['はい', 'いいえ', '直す'];

let browser;
let irWithMark;
let irPlain;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
}

function compileWithBody(bin, bodyMd) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-reduced-motion-lo3-'));
  try {
    fs.cpSync(PACK, work, { recursive: true });
    if (bodyMd !== null) fs.writeFileSync(path.join(work, 'chapters', 'ch-min', 'st-1.md'), bodyMd, 'utf8');
    const r = spawnSync(bin, ['compile', work], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    assert.equal(r.status, 0, `compile exit ${r.status}`);
    assert.equal(JSON.parse(r.stdout).format_version, 2);
    return r.stdout;
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  const bin = packBinFromEnv();
  irWithMark = compileWithBody(bin, BODY_MD);
  irPlain = compileWithBody(bin, null);
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function waitUntil(what, read, pred, timeout = 10000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await read();
    if (await pred(last)) return last;
    await new Promise((r) => setTimeout(r, 20));
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

// 文脈（reduce / no-preference）を指定して開く。文脈の取り違えを防ぐため、開いた直後に engine.reducedMotion を確かめる。
async function openIn(srv, media, query = '') {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.emulateMedia({ reducedMotion: media });
  await opened.page.goto(`${srv.url}${query}`);
  await waitMounted(opened.page);
  assert.equal((await engine(opened.page)).reducedMotion, media === 'reduce', `context ${media}`);
  return opened;
}

async function loadIr(page, text) {
  const n0 = (await engine(page)).installCount;
  assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), text), true, 'load-ir sent');
  await waitUntil('install', () => engine(page), (e) => e.installCount > n0);
  await page.waitForSelector(BODY, { timeout: 10000 });
}

// 1 時点の標本。文面（題・do・本文）・状態の語・画面の部品の集合・ボタンの数・段階の行の件数・印の件数。
const sample = (page) => page.evaluate(({ status, body, li, mark }) => {
  const text = (id) => [...document.querySelectorAll(`[data-testid="${id}"]`)].map((e) => e.textContent);
  const b = document.querySelector(body);
  return {
    state: document.querySelector(status)?.getAttribute('data-state') ?? null,
    title: text('step-title'),
    do: text('step-do'),
    body: b ? b.textContent : null,
    markParagraphs: b ? [...b.querySelectorAll('p')].filter((p) => p.textContent.includes(mark)).length : 0,
    progressRows: document.querySelectorAll(li).length,
    testids: [...new Set([...document.querySelectorAll('[data-testid]')].map((e) => e.getAttribute('data-testid')))],
    buttons: document.querySelectorAll('button').length,
  };
}, {
  status: STATUS, body: BODY, li: PROGRESS_LI, mark: MARK,
});

// 送信して busy → idle まで標本を採る。処理中を観察できなければ失敗にする。
async function runToIdle(page) {
  await page.fill(DRAFT, COMMAND);
  await page.click(SUBMIT);
  const samples = [];
  let sawBusy = false;
  await waitUntil('busy then idle', async () => {
    const s = await sample(page);
    samples.push(s);
    if (s.state === 'busy') sawBusy = true;
    return s;
  }, (s) => sawBusy && s.state === 'idle');
  assert.ok(sawBusy, '処理中を観察できなかった');
  return samples;
}

// 段階の行を除いた比較用の部品の集合。
const parts = (s) => [...s.testids].filter((t) => t !== 'line-stage').sort();

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`G・H: 補う文面の経路が reduce でも同じに出て、reduce だけの部品が無い（${mode}）`, { timeout: 180000 }, async () => {
    const seen = {};
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      for (const media of MEDIA) {
        const { page, context, rec } = await openIn(srv, media);
        try {
          await loadIr(page, irWithMark);
          const handle = await page.evaluateHandle((sel) => document.querySelector(sel), BODY);
          const first = await sample(page);
          const tree = await page.locator(BODY).ariaSnapshot();
          assert.ok(tree.includes(MARK), `${media}: アクセシビリティツリーに印の段落が出る`);
          const samples = await runToIdle(page);
          const all = [first, ...samples];
          assert.ok(all.every((s) => s.markParagraphs === 1), `${media}: 印の段落が全時点で 1 件`);
          assert.ok(all.every((s) => s.body === first.body && JSON.stringify(s.title) === JSON.stringify(first.title)
            && JSON.stringify(s.do) === JSON.stringify(first.do)), `${media}: 文面が全時点で変わらない`);
          assert.equal(await handle.evaluate((e, sel) => e.isConnected && e === document.querySelector(sel), BODY), true,
            `${media}: 本文の要素が置き換わらない`);
          const progress = samples.reduce((n, s) => n + s.progressRows, 0);
          if (media === 'reduce') assert.equal(progress, 0, 'reduce は段階の行が 0 件');
          else assert.ok(progress > 0, '対照: no-preference は段階の行が出る');
          const counts = await collectSafety(page, rec);
          assert.deepEqual(nonZero(counts), [], JSON.stringify({ media, csp, counts }));
          seen[media] = { first, tree, parts: parts(samples[samples.length - 1]), buttons: samples[samples.length - 1].buttons };

          // 対照: 印を入れない写しでは、同じ判定が印の段落を 0 件と数える（判定の空振りの防止）。
          await loadIr(page, irPlain);
          assert.equal((await sample(page)).markParagraphs, 0, `${media}: 対照で印が 0 件`);
          assert.equal((await page.locator(BODY).ariaSnapshot()).includes(MARK), false, `${media}: 対照で印がツリーに出ない`);
        } finally {
          await context.close();
        }
      }
    }));
    const [r, n] = [seen.reduce, seen['no-preference']];
    assert.equal(r.first.body, n.first.body, '本文の文字が一致');
    assert.deepEqual(r.first.title, n.first.title);
    assert.deepEqual(r.first.do, n.first.do);
    assert.equal(r.tree, n.tree, 'アクセシビリティツリーの本文が一致');
    assert.deepEqual(r.parts, n.parts, 'reduce だけの部品が無い');
    assert.equal(r.buttons, n.buttons, 'ボタンの数が一致');
  });

  test(`J1: チェックポイントの保存 → 復帰が reduce でも同じ結果になる（${mode}）`, { timeout: 300000 }, async () => {
    const results = {};
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      for (const media of MEDIA) {
        const { page, context, rec } = await openIn(srv, media, '?fixture=ch0-3-chapters');
        try {
          const L = ledger();
          const observe = () => page.evaluate(([rows, pos]) => ({
            rows: [...document.querySelectorAll(rows)].map((r) => [r.getAttribute('data-row'), r.textContent]),
            position: document.querySelector(pos)?.textContent ?? null,
          }), [ROWS, POSITION]);
          let saved = null;
          let cpId = null;
          const onStep = async (uid, status) => {
            if (uid !== 'main/st-1-1' || status !== 'entered') return;
            // 章の終わりの自動保存（SAVE-31）が先に ID を使うため、増えた 1 件の ID を使う。
            const ids = () => page.evaluate((sel) => [...document.querySelectorAll(sel)].map((b) => b.getAttribute('data-checkpoint')), RESTORE);
            const before = await ids();
            await page.click(SAVE);
            await page.waitForFunction(([sel, k]) => document.querySelectorAll(sel).length === k + 1, [RESTORE, before.length]);
            cpId = (await ids()).find((i) => !before.includes(i));
            saved = await observe();
          };
          await journey(page, mouseDriver(page), media, L, 'main/st-1-4', { chaptered: true, onStep });
          assert.ok(saved, '保存の時点の観察が取れていない');
          assert.notEqual((await engine(page)).current, 'main/st-1-1', '対照: 進めた後は保存の時点と違う');
          await page.click(`${RESTORE}[data-checkpoint="${cpId}"]`);
          await waitUntil('restore', () => engine(page), (e) => e.current === 'main/st-1-1');
          const e = await engine(page);
          const back = await observe();
          assert.deepEqual(back, saved, `${media}: 復帰の後に位置と成果物が保存の時点`);
          assert.deepEqual(L.failed(), []);
          assert.deepEqual(nonZero(await collectSafety(page, rec)), [], JSON.stringify({ media, csp }));
          results[media] = { current: e.current, back, state: e.stateWithoutLog };
        } finally {
          await context.close();
        }
      }
    }));
    assert.deepEqual(results.reduce, results['no-preference'], 'reduce と no-preference で復帰の結果が一致');
  });

  test(`J2: 差し戻し → 再承認が reduce でも同じ結果になる（${mode}）`, { timeout: 300000 }, async () => {
    const results = {};
    const rejected = (page) => page.locator(`${LOG} > li[data-kind="rejected"]`).count();
    const waitStep = (page, s) => page.waitForFunction(
      (x) => JSON.parse(window.__fandheTest.stateJson()).engine.current === x, s, { timeout: 30000 },
    );
    const waitRejected = (page, n) => page.waitForFunction(
      ([log, k]) => document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length === k, [LOG, n],
    );
    const click = (page, name) => page.getByRole('button', { name, exact: true }).click();
    const labels = (page) => page.evaluate((b) => [...document.querySelectorAll(b)].map((x) => x.textContent), BTN);
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      for (const media of MEDIA) {
        const { page, context, rec } = await openIn(srv, media, '?fixture=ch0-3-reapprove&start=s-2-6');
        try {
          await page.waitForSelector(`${BTN}[data-revise-target]`, { timeout: 30000 });
          assert.equal((await engine(page)).current, 'main/st-2-6');
          const base = await rejected(page);
          await click(page, '直す（対象: 冒頭の見出し）');
          await waitRejected(page, base + 1);
          assert.equal((await engine(page)).current, 'main/st-2-6', '誤った候補で Step は変わらない');
          await click(page, '直す（対象: 期待する結果）');
          await page.waitForFunction(
            (b) => document.querySelectorAll(b).length === 3 && !document.querySelector(`${b}[data-revise-target]`), BTN,
          );
          assert.deepEqual(await labels(page), THREE);
          await click(page, 'はい');
          await waitStep(page, 'main/st-2-6b');
          await page.waitForSelector(BTN, { timeout: 30000 });
          assert.deepEqual(await labels(page), THREE);
          const r0 = await rejected(page);
          await click(page, '直す');
          await waitRejected(page, r0 + 1);
          await click(page, 'いいえ');
          await waitRejected(page, r0 + 2);
          assert.equal((await engine(page)).current, 'main/st-2-6b', '拒否で Step は変わらない');
          await click(page, 'はい');
          await waitStep(page, 'main/st-2-7');
          assert.deepEqual(nonZero(await collectSafety(page, rec)), [], JSON.stringify({ media, csp }));
          results[media] = { rejected: await rejected(page), current: (await engine(page)).current, buttons: await labels(page) };
        } finally {
          await context.close();
        }
      }
    }));
    assert.deepEqual(results.reduce, results['no-preference'], 'reduce と no-preference で再承認の結果が一致');
    assert.equal(results.reduce.current, 'main/st-2-7');
  });
}
