// 旧いエンジンが新しい教材（IR）を理由つきで断り、保存済みの履歴が書き換わらないことの試験（VER-13。TASK-181 2/2）。
//
// 旧いエンジンの実物（過去の版のビルド）は repo に無い。ブラウザで動く今のエンジンを、より新しい教材に対する旧いエンジンと
// みなす。断られる IR は、既定の教材（pack `min-3step`）と同じ `pack.id` の次の 2 本で、同じ教材の新しい版が旧いエンジンに
// 届いた場面を表す（`?fixture=` の完全一致で選ぶ。試験の設定だけ）。
//   (a) `?fixture=future-ir-format`: 版が 3 で未知の欄を持つ。理由コードは `format_newer`。
//   (b) `?fixture=future-ir-field`: 版は 1 のまま未知の欄を持つ（混在の危険な場面）。理由コードは `malformed`。
// 実際の画面で次を確かめる。
//   (T1・T2) 先に既定の画面で 1 手を送って保存させる。断られる画面へ移ると、通知（`ir-notice`・`role=status`）が固定の文言と
//       コードだけで出て、教材（入力欄・ログ）は出ない。断った画面の間も、再読み込み（pagehide）を挟んでも、保存先
//       （localStorage）の全体が 1 文字も変わらない。既定の画面へ戻ると保存済みの履歴が開ける。
//   (T3) 対照: 既定の画面には通知が無く、`irRefusal` は null。
//   (T4) 起動後の `load-ir:` で読めない IR を送っても、状態・ログ・差し替えの回数・URL・保存先が変わらず、写しの `irRefusal` だけが立つ。
// 試験の出力・失敗の文に、保存した値を出さない（履歴は受講者のデータのため。一致かどうかだけを比べる）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-ir-refusal/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const NOTICE = '[data-testid="ir-notice"]';
const LOG_ITEMS = '[data-testid="part-log-list"] li';
const ACCEPTED = '/make-note first-note';
const SENTINEL = 'FUTURE-SENTINEL-V3-X';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const MIN3 = path.join(ROOT, 'testdata/engine-event-consistency/test-data/min-3step.json');

const CASES = [
  { name: 'future-ir-format', code: 'format_newer' },
  { name: 'future-ir-field', code: 'malformed' },
];

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 既定の教材から、旧いエンジンが読めない IR を作る（`test_fixture::future_ir_bytes` と同じ規則）。
function futureIr() {
  const ir = JSON.parse(fs.readFileSync(MIN3, 'utf8'));
  ir.format_version = 3;
  ir.pack.ir_format = 3;
  ir.future_v3_x = SENTINEL;
  return JSON.stringify(ir);
}

async function open(srv, query = '') {
  const opened = await openPage(browser, { origin: srv.origin });
  opened.consoleTexts = [];
  opened.page.on('console', (m) => opened.consoleTexts.push(m.text()));
  await opened.page.goto(srv.url + query);
  await waitMounted(opened.page);
  return opened;
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;

// 保存先（localStorage）の全体。鍵で並べた文字列にして、一致かどうかだけを比べる。
const storageDump = (page) => page.evaluate(() => JSON.stringify(
  Object.keys(localStorage).sort().map((k) => [k, localStorage.getItem(k)]),
));

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
  assert.equal(s.dialogs, 0, detail);
}

// 既定の画面で 1 手を送り、処理が終わってから再読み込みして保存させる。
async function saveOneStep(page) {
  await page.fill(INPUT, ACCEPTED);
  await page.click(SUBMIT);
  await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT);
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null');
  await page.reload();
  await waitMounted(page);
  assert.equal((await engine(page)).bootKind, 'restored');
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  for (const { name, code } of CASES) {
    test(`${name}: 理由つきで断られ、保存済みの履歴が書き換わらない（CSP ${label}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        // 同じ文脈（同じ保存先）のまま、既定の画面 → 断られる画面 → 既定の画面と移る。
        const { page, context, rec, consoleTexts } = await open(srv);
        await saveOneStep(page);
        const savedEng = await engine(page);
        await page.goto(`${srv.url}?fixture=${name}`);
        await waitMounted(page);
        const baseline = await storageDump(page);
        assert.notEqual(baseline, '[]', 'a saved history exists before the refusal (non-vacuous)');

        assert.equal(await page.locator(NOTICE).count(), 1);
        assert.equal(await page.locator(NOTICE).innerText(), `教材を読み込めませんでした。（${code}）`);
        assert.equal(await page.locator(NOTICE).getAttribute('role'), 'status');
        assert.equal(await page.locator(INPUT).count(), 0, 'no input field');
        assert.equal(await page.locator(LOG_ITEMS).count(), 0, 'no log rows');
        const eng = await engine(page);
        assert.equal(eng.irLoaded, false);
        assert.equal(eng.irRefusal, code);
        const html = await page.evaluate(() => document.documentElement.outerHTML);
        assert.equal(html.includes(SENTINEL), false, 'unknown field value is not shown');
        assert.equal(consoleTexts.some((t) => t.includes(SENTINEL)), false, 'unknown field value is not logged');

        assert.equal(await storageDump(page), baseline, 'storage is unchanged while refusing');
        await page.reload();
        await waitMounted(page);
        assert.equal(await page.locator(NOTICE).count(), 1);
        assert.equal(await storageDump(page), baseline, 'storage is unchanged after pagehide');

        // 陽性の対照: 断ったあとも、保存済みの履歴は既定の画面でまだ開ける。
        await page.goto(srv.url);
        await waitMounted(page);
        const again = await engine(page);
        assert.equal(again.bootKind, 'restored');
        assert.equal(again.irLoaded, true);
        assert.equal(again.generation >= savedEng.generation, true);
        assert.equal(again.logLen >= savedEng.logLen, true);
        assert.equal(await page.locator(NOTICE).count(), 0);
        clean(rec);
        await context.close();
      }));
    });
  }

  test(`対照: 読める教材には通知が無い（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      assert.equal(await page.locator(NOTICE).count(), 0);
      assert.equal(await page.locator(INPUT).count(), 1);
      const eng = await engine(page);
      assert.equal(eng.irLoaded, true);
      assert.equal(eng.irRefusal, null);
      clean(rec);
      await context.close();
    }));
  });

  test(`起動後に読めない IR を送っても、状態は変わらず断りだけが記録される（CSP ${label}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await open(srv);
      await saveOneStep(page);
      const before = await engine(page);
      const url = page.url();
      const stored = await storageDump(page);
      const sent = await page.evaluate((text) => window.__fandheTest.loadIr(text), futureIr());
      assert.equal(sent, true);
      const after = await engine(page);
      assert.equal(after.irRefusal, 'format_newer');
      assert.equal(after.irLoaded, true);
      assert.equal(JSON.stringify(after.stateWithoutLog), JSON.stringify(before.stateWithoutLog));
      assert.equal(after.logLen, before.logLen);
      assert.equal(after.installCount, before.installCount);
      assert.equal(page.url(), url);
      assert.equal(await storageDump(page), stored);
      assert.equal(await page.locator(NOTICE).count(), 0, 'running screen keeps its structure');
      clean(rec);
      await context.close();
    }));
  });
}
