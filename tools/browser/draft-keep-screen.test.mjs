// 未送信の文章の保持を、右のログと拒否の表示のある本体の画面で確かめる試験（TASK-443。TERM-17・TERM-18）。
//
// 守る期待:
//   TERM-17 拒否の全分類で、送った本文が入力欄に残る。右のログには拒否の行（分類・理由・次の行動）が出る
//   TERM-18 送信していない下書き（改行・空行・Tab 文字・先頭や末尾の改行・空白だけ・貼り付けた複数行）が再読み込みで
//           同じに戻り、先頭が改行の下書きも戻る。変換中の文字列は下書きに入らず、確定済みの文字列だけが戻る
//     SK-1 拒否の 7 分類 × 送信の経路 2 通り（Enter・送信ボタン）。拒否の後と再読み込みの後で欄の値と拒否の行を見る
//     SK-2 拒否の行が出ている画面で、下書きの 6 種と先頭が改行の下書きを入れて再読み込み
//     SK-3 同じ画面で、貼り付けた複数行（試料 2 種）を再読み込み
//     SK-4 同じ画面で、確定済みの文に続けて変換中に再読み込み（シミュレーター）。SK-4c は読みを確定させた対照
// 既存の試験との違い: 保持（reject-keeps-draft。TASK-271）と復元（draft-reload・draft-reload-ime。TASK-328）は
//   試験用の最小の教材の画面で確かめる。ここは CH-0〜CH-3 の形の教材（`?fixture=ch0-3-links` の開始点 s-1-1・s-1-5・s-2-1）で、
//   拒否の行の理由と次の行動が IR の文面のまま出て、再読み込みの後も同じ行が残る画面と組にする。経路の網羅・pagehide・
//   配信する設定は既存の試験が持ち、ここでは複製しない。
// 証拠の種類: 自動ブラウザ。SK-4 は CDP による IME の模擬で、実 IME でも人の評価でもない（実 IME は TASK-93）。
// 失敗の文には長さと真偽だけを出し、下書きの本文は出さない。固定の sleep は使わない。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import { DRAFT, SUBMIT, LOG, REJECTED, T, waitCount } from './ch0-3-journey.mjs';
import { loadErrorInputs } from './error-inputs.mjs';
import { SIM_LABEL, openIme, assertFocused, imeCompose, imeCommit, settle } from './ime.mjs';

const BASE = '/fandhe-draft-keep-screen/';
const QUERY = (start) => `?fixture=ch0-3-links&start=${start}`;
const STATUS = '[data-testid="run-status"]';
const HERE = path.dirname(fileURLToPath(import.meta.url));
const SAMPLES = path.resolve(HERE, '../../testdata/input-field-ime/test-data/paste-samples');
const ir = JSON.parse(fs.readFileSync(path.resolve(HERE, '../../crates/web/tests/data/ch0_3/ir.json'), 'utf8'));
const MULTI = '\n  メモを書く\n\n  二行目  \n';
const READING = 'みかくていのよみ';

// 開始点ごとに、その分類の拒否になる本文。busy は先に受付になる本文（before）を送り、処理中に送る。
const CASES = [
  { category: 'unknown_command', start: 's-1-1', body: '/unknown-x' },
  { category: 'wrong_order', start: 's-1-1', body: '/advance-phase' },
  { category: 'not_in_this_step', start: 's-1-1', body: MULTI },
  { category: 'not_supported', start: 's-1-1', body: '/sync-cloud' },
  { category: 'missing_arg', start: 's-2-1', body: '/new-idea' },
  { category: 'bad_arg_shape', start: 's-2-1', body: '/new-idea Bad_Name' },
  { category: 'busy', start: 's-1-5', body: '/make-card second', before: '/make-card sample' },
];

// TERM-18 の下書きの種類（6 種と先頭が改行）。貼り付けた複数行は SK-3 で別に扱う。
const KINDS = [
  ['改行', '1 行目\n2 行目'],
  ['空行', '1 行目\n\n3 行目'],
  ['Tab 文字', '列 1\t列 2'],
  ['先頭が改行', '\n先頭が改行'],
  ['末尾が改行', '末尾が改行\n'],
  ['空白だけ', '   '],
];

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(
    dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(),
    'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1',
  );
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const cpLen = (s) => [...s].length;
const hookDraft = (page) => page.evaluate(() => window.__fandheTest.draft());

// 拒否の行の一覧を { category, reason, next } で読む（右のログの見える文字だけ）。
const rejectedRows = (page) => page.$$eval(REJECTED, (lis) => lis.map((li) => ({
  category: li.getAttribute('data-category'),
  reason: li.querySelector('[data-testid="line-reason"]')?.textContent ?? null,
  next: li.querySelector('[data-testid="line-next-action"]')?.textContent ?? null,
})));

function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(s);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
  assert.equal(s.external, 0, detail);
}

// 欄の値（と状態の下書き）が期待と同じ。本文は出さず、長さだけを失敗の文に出す。
async function assertField(page, label, expected, { hooks = true } = {}) {
  const field = await page.inputValue(DRAFT);
  assert.ok(field === expected, `${label}: 欄の値が違う（期待 ${cpLen(expected)} 文字・実際 ${cpLen(field)} 文字）`);
  if (!hooks) return;
  const draft = await hookDraft(page);
  assert.ok(draft === expected, `${label}: 状態の下書きが違う（期待 ${cpLen(expected)} 文字・実際 ${cpLen(draft)} 文字）`);
}

// 最後の拒否の行が、その分類の IR の文面（理由・次の行動）で出ている。
async function assertRejectedRow(page, label, category, count) {
  const rows = await rejectedRows(page);
  assert.equal(rows.length, count, `${label}: 拒否の行の数`);
  const want = ir.rejections[ir.defaults[category]];
  assert.deepEqual(rows.at(-1), { category, reason: want.message, next: want.next_action }, `${label}: 拒否の行の分類・理由・次の行動`);
}

async function open(srv, start) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: srv.origin });
  await opened.page.emulateMedia({ reducedMotion: 'no-preference' });
  await opened.page.goto(`${srv.url}${QUERY(start)}`);
  await waitMounted(opened.page);
  await settle(opened.page);
  return opened;
}

async function send(page, how) {
  if (how === 'click') await page.click(SUBMIT);
  else {
    await page.focus(DRAFT);
    await page.keyboard.press('Enter');
  }
}

// 再読み込みし、mount の直後（先頭の改行が落ちていないこと）と落ち着いた後の両方で欄を見る。
async function reloadAndCheck(page, label, expected) {
  await page.reload();
  await waitMounted(page);
  await assertField(page, `${label}（再読み込みの mount の直後）`, expected, { hooks: false });
  await settle(page);
  await assertField(page, `${label}（再読み込みの後）`, expected);
}

// 拒否の行が 1 件ある画面を作る（SK-2〜SK-4 の前段）。
async function withRejection(page) {
  await page.fill(DRAFT, '/unknown-x');
  await send(page, 'enter');
  await waitCount(page, REJECTED, 1);
  await assertRejectedRow(page, '前段', 'unknown_command', 1);
}

test('SK-1 の分類が誤入力の試験データの 7 分類と一致する（漏れ・余りが無い）', () => {
  const want = [...new Set(loadErrorInputs().map((c) => c.category))].sort();
  assert.deepEqual(CASES.map((c) => c.category).sort(), want);
});

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  for (const c of CASES) {
    for (const how of ['enter', 'click']) {
      test(`SK-1 ${c.category}（${how}）: 拒否の行が出て、欄の本文が拒否と再読み込みの後も変わらない（${mode}）`, async () => {
        await run(async (srv) => {
          const { page, context, rec } = await open(srv, c.start);
          if (c.before) {
            await page.fill(DRAFT, c.before);
            await send(page, 'enter');
            await page.waitForFunction((s) => document.querySelector(s)?.getAttribute('data-state') === 'busy', STATUS, { timeout: T });
          }
          await page.fill(DRAFT, c.body);
          await send(page, how);
          await waitCount(page, REJECTED, 1);
          await settle(page);
          await assertRejectedRow(page, c.category, c.category, 1);
          await assertField(page, `${c.category}（拒否の後）`, c.body);
          if (c.before) {
            // 処理が終わった後も、進行の描画・受付済みへの遷移で欄が書き換わらない。
            await page.waitForFunction((s) => document.querySelector(s)?.getAttribute('data-state') !== 'busy', STATUS, { timeout: T });
            await settle(page);
            await assertField(page, `${c.category}（処理の後）`, c.body);
          }
          await reloadAndCheck(page, c.category, c.body);
          await assertRejectedRow(page, `${c.category}（再読み込みの後）`, c.category, 1);
          assertClean(rec);
          await context.close();
        });
      }, { timeout: T * 2 });
    }
  }

  for (const [name, text] of KINDS) {
    test(`SK-2 ${name}: 拒否の行のある画面で、下書きが再読み込みで同じに戻る（${mode}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv, 's-1-1');
        await withRejection(page);
        await page.fill(DRAFT, text);
        await settle(page);
        await reloadAndCheck(page, name, text);
        await assertRejectedRow(page, `${name}（再読み込みの後）`, 'unknown_command', 1);
        assertClean(rec);
        await context.close();
      });
    }, { timeout: T * 2 });
  }

  for (const file of ['paste-3lines.txt', 'paste-crlf-tabs.txt']) {
    test(`SK-3 貼り付けた複数行（${file}）: 送信されず、再読み込みで同じに戻る（${mode}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv, 's-1-1');
        await withRejection(page);
        const raw = fs.readFileSync(path.join(SAMPLES, file), 'utf8');
        const text = raw.replace(/\r\n?/g, '\n');
        const logBefore = await page.locator(`${LOG} > li`).count();
        await page.fill(DRAFT, '');
        await page.evaluate((t) => navigator.clipboard.writeText(t), raw);
        await page.focus(DRAFT);
        await page.keyboard.press('ControlOrMeta+V');
        await page.waitForFunction(([s, n]) => document.querySelector(s).value.length >= n, [DRAFT, text.length], { timeout: T });
        await settle(page);
        await assertField(page, `${file}（貼り付けた後）`, text);
        assert.equal(await page.locator(`${LOG} > li`).count(), logBefore, '貼り付けだけでは右のログが増えない');
        await reloadAndCheck(page, file, text);
        await assertRejectedRow(page, `${file}（再読み込みの後）`, 'unknown_command', 1);
        assertClean(rec);
        await context.close();
      });
    }, { timeout: T * 2 });
  }

  for (const commit of [false, true]) {
    const id = commit ? 'SK-4c 対照: 読みを確定させてから' : 'SK-4 変換中に';
    test(`${id}再読み込みすると、${commit ? '確定した読みも戻る' : '確定済みの文だけが戻る'}（${SIM_LABEL}。${mode}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv, 's-1-1');
        const cdp = await openIme(page);
        await withRejection(page);
        const done = '\n確定済みの\tメモ';
        await page.fill(DRAFT, done);
        await settle(page);
        await page.focus(DRAFT);
        await page.evaluate(([s, p]) => document.querySelector(s).setSelectionRange(p, p), [DRAFT, done.length]);
        await assertFocused(page, DRAFT);
        await imeCompose(cdp, READING);
        await settle(page);
        assert.ok((await page.inputValue(DRAFT)) === done + READING, '前提: 欄は確定済み＋変換中の読み');
        assert.ok((await hookDraft(page)) === done, '変換中も状態の下書きは確定済みのまま');
        if (commit) {
          await imeCommit(cdp, READING);
          await settle(page);
        }
        const expected = commit ? done + READING : done;
        await reloadAndCheck(page, id, expected);
        await assertRejectedRow(page, `${id}（再読み込みの後）`, 'unknown_command', 1);
        assertClean(rec);
        await context.close();
      });
    }, { timeout: T * 2 });
  }
}
