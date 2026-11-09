// 書き出しに未送信の下書きが入ることの層 3 ブラウザ試験（TASK-329。SAVE-18・D-52）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の試験の設定）。未設定・不在は skip にせず失敗にする。CSP あり・なしで流す。
// 方針: 画面の操作だけで書き出す（入力欄へ入力 → 「履歴を書き出す」→ リンクの Download）。観察は engine の口
//   `exportIncludesUnsentDraft`（真偽だけ。配信する設定には無い）と、保存したファイルの JSON の構造を比べる。
// 確かめること:
//   D-1 送信しない下書きがあると口が真で、書き出したファイルの `ops` の最後の文字列の操作が同じ値の `draft`、
//       `snapshot.drafts` にも同じ値があり、ファイルから求めた真偽と口が一致する。
//   D-2 下書きを変えると出ていたリンクが外れ、もう一度書き出すと新しい値が入る。
//   D-3（負の対照）送信した後は口が偽で、ファイルの `snapshot.drafts` に空でない値が無い。
//   D-4（負の対照）欄を空にした後は口が偽。
//   各場面で外部への要求・CSP 違反・ページのエラーが 0 件。URL に状態が載らない。
// 限界: Chromium だけ。実 IME ではない。案内の画面は TASK-537（SHELL-22）が持つ。消した下書きの残り（`docs/known-gaps.md`）は確かめない。
// 下書きの値は平文の試験値（秘密情報の形に当たらない）。失敗の文・診断には値を出さず、長さと真偽だけを出す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize } from './harness.mjs';

const BASE = '/fandhe-export-draft/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const EXPORT_BTN = '[data-testid="shell-export"]';
const LINK = '[data-testid="export-download"]';
const DRAFT = 'memo draft text';
const DRAFT2 = 'second memo text';
const ACCEPTED = '/make-note first-note';
const T = 30000;

let browser;
let tmpDir;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-export-draft-'));
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);
const engine = async (page) => JSON.parse(await call(page, 'stateJson')).engine;
const urlState = (page) => page.evaluate(() => ({ href: location.href, hash: location.hash, length: history.length }));
const settle = (page) => page.evaluate(() => new Promise((resolve) => { setTimeout(() => setTimeout(resolve, 0), 0); }));

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  await settle(opened.page);
  return opened;
}

// 口が期待の真偽になるまで待つ（描画の次のタスクで揃う）。
async function waitFlag(page, expected, label) {
  await page.waitForFunction(
    (want) => JSON.parse(window.__fandheTest.stateJson()).engine.exportIncludesUnsentDraft === want,
    expected,
    { timeout: T },
  ).catch(() => assert.fail(`${label}: exportIncludesUnsentDraft did not become ${expected}`));
}

// 画面の操作だけで書き出し、ファイルの JSON を返す。
async function exportByUi(page) {
  await page.locator(EXPORT_BTN).click();
  const link = page.locator(LINK);
  await link.waitFor({ state: 'visible', timeout: 10000 });
  const [download] = await Promise.all([page.waitForEvent('download'), link.click()]);
  const file = path.join(tmpDir, `${Date.now()}-${Math.random().toString(16).slice(2)}.json`);
  await download.saveAs(file);
  const parsed = JSON.parse(fs.readFileSync(file, 'utf8'));
  fs.rmSync(file, { force: true });
  return parsed;
}

// 書き出したファイルの判定（試験の側で JSON から求める。実装の関数は使わない）。
function lastStringOp(ops) {
  for (let i = ops.length - 1; i >= 0; i -= 1) {
    const op = ops[i];
    if (op && typeof op === 'object' && 'draft' in op) return { kind: 'draft', value: op.draft };
    if (op && typeof op === 'object' && 'submit' in op) return { kind: 'submit', value: op.submit };
  }
  return null;
}
function fileVerdict(file) {
  const drafts = Object.values(file.snapshot.drafts);
  if (drafts.some((v) => v.length > 0)) return true;
  const last = lastStringOp(file.ops);
  return last !== null && last.kind === 'draft' && last.value.length > 0;
}

function assertQuiet(rec, label) {
  const s = summarize(rec);
  assert.equal(s.external, 0, `${label}: no external request`);
  assert.equal(s.csp, 0, `${label}: no CSP violation`);
  assert.equal(s.pageErrors, 0, `${label}: no page error`);
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  test(`D-1 送信しない下書きが書き出しのファイルに入り、口と一致する（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await waitFlag(page, false, 'D-1 初期');
      await page.fill(INPUT, DRAFT);
      await waitFlag(page, true, 'D-1');
      const urlBefore = await urlState(page);
      const file = await exportByUi(page);
      const last = lastStringOp(file.ops);
      assert.ok(last !== null && last.kind === 'draft', 'last string op is a draft');
      assert.equal(last.value === DRAFT, true, 'last draft op equals the typed value');
      const held = Object.values(file.snapshot.drafts).filter((v) => v === DRAFT).length;
      assert.equal(held, 1, 'snapshot.drafts holds the value once');
      assert.equal(fileVerdict(file), (await engine(page)).exportIncludesUnsentDraft, 'file verdict equals the port');
      assert.deepEqual(await urlState(page), urlBefore, 'URL does not carry the state');
      assertQuiet(rec, 'D-1');
      await context.close();
    });
  });

  test(`D-2 下書きを変えるとリンクが外れ、書き出し直すと新しい値が入る（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, DRAFT);
      await waitFlag(page, true, 'D-2 first');
      await exportByUi(page);
      await page.fill(INPUT, DRAFT2);
      await page.locator(LINK).waitFor({ state: 'detached', timeout: 10000 });
      await waitFlag(page, true, 'D-2 second');
      const file = await exportByUi(page);
      const last = lastStringOp(file.ops);
      assert.equal(last !== null && last.value === DRAFT2, true, 'second export holds the new value');
      assert.equal(Object.values(file.snapshot.drafts).includes(DRAFT2), true, 'snapshot holds the new value');
      assertQuiet(rec, 'D-2');
      await context.close();
    });
  });

  test(`D-3 負の対照: 送信した後は口が偽で、ファイルに空でない下書きが無い（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, ACCEPTED);
      await waitFlag(page, true, 'D-3 before');
      await page.click(SUBMIT);
      await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT, { timeout: T });
      await waitFlag(page, false, 'D-3 after');
      const file = await exportByUi(page);
      assert.equal(Object.values(file.snapshot.drafts).filter((v) => v.length > 0).length, 0, 'no non-empty draft in the file');
      assert.equal(fileVerdict(file), false, 'file verdict is false');
      assertQuiet(rec, 'D-3');
      await context.close();
    });
  });

  test(`D-4 負の対照: 欄を空にした後は口が偽（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, DRAFT);
      await waitFlag(page, true, 'D-4 before');
      await page.fill(INPUT, '');
      await waitFlag(page, false, 'D-4 after');
      assertQuiet(rec, 'D-4');
      await context.close();
    });
  });
}
