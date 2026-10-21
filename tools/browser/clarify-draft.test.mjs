// 問い返しの後に続けて打った入力が連結されないことの試験（TASK-284 1/2。JUDGE-27・TERM-22。決定 D-67）。
//
// 試作（PoC-13 の AI 代理の準備）の観察: ST-1.4 で引数なしの `/new-idea` の問い返しを受けた後、全選択せずに
// `/advance-phase` と打つと、残った下書きにつながって `/new-idea/advance-phase` になり、期待した順序違い
// （wrong_order）ではなく unknown_command で拒否された。問い返しの後は入力欄を空にする（本番は host.rs の submit。
// Rust 側の対は crates/web/tests/clarify_draft.rs）。
//
// 証拠の種類: 自動ブラウザ・シミュレーター（AI の代理と同じ操作の再現。人の評価ではない）。
// 操作は `page.keyboard` だけで、`page.fill`・`page.click`・全選択（Ctrl/Cmd+A）は使わない。
// 受け持ち（試験用の教材 `?fixture=ch0-3` の main/st-1-4。CSP あり・なし × 送信の経路 Enter・送信ボタン）:
//   - 問い返しの後に、欄の値（DOM）と状態の下書きがどちらも空になり、拒否の行が増えず、Step が進まない
//   - 続けて打った `/advance-phase` の分類が wrong_order で、unknown_command でない
//   - 拒否の後は、欄の値と状態の下書きが `/advance-phase` のまま残る（JUDGE-5 と両立する）
//   - `raw-trigger` の click が 0 回、安全の計数が 0、エンジンの警告・無視が 0 件
//
// 確かめないこと: 拒否の後に下書きを選択状態にする処理（TASK-284 2/2）、フォーカスの位置、実 IME、人の確認。
// 出力に、入力した値や下書きの中身は出さない（名前・件数・合否だけ）。欄の値は読むだけで、状態へ流さない（BUILD-6）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  DRAFT, SUBMIT, CLARIFIED, REJECTED, T, engine, ledger, keyboardDriver, journey, install, keyTo, waitCount,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-clarify-draft/';
const QUERY = '?fixture=ch0-3';
const STEP = 'main/st-1-4';
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

const stateDraft = (page) => page.evaluate(() => window.__fandheTest.draft());
const waitDomDraft = (page, want) => page.waitForFunction(
  ([sel, v]) => document.querySelector(sel)?.value === v, [DRAFT, want], { timeout: T },
);

// 欄に打ち、経路に応じて送る。全選択はしない。enter は欄で Enter、button は送信ボタンへ Tab で移って Enter。
async function typeAndSend(page, text, how) {
  await keyTo(page, DRAFT);
  await page.keyboard.type(text);
  if (how === 'button') await keyTo(page, SUBMIT);
  await page.keyboard.press('Enter');
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  for (const how of ['enter', 'button']) {
    test(`問い返しの後に続けて打った入力が順序違いの拒否になる: ${how}（${mode}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        const L = ledger();
        const scene = `問い返しの後（${how}）`;
        await page.goto(`${srv.url}${QUERY}`);
        await waitMounted(page);
        assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
        await install(page);
        await journey(page, keyboardDriver(page), scene, L, STEP);
        assert.equal((await engine(page)).current, STEP);
        const judged0 = (await engine(page)).judgedCategories.length;
        // 通しの前段（st-0-1〜st-1-3）で拒否の行が既にあるため、拒否の行は増分で数える。
        const rejected0 = await page.locator(REJECTED).count();

        // 問い返し: 欄が空になる。
        await typeAndSend(page, '/new-idea', how);
        await waitCount(page, CLARIFIED, 1);
        await waitDomDraft(page, '');
        L.check(scene, '問い返しの後に欄の値が空', (await page.inputValue(DRAFT)) === '');
        L.check(scene, '問い返しの後に状態の下書きが空', (await stateDraft(page)) === '');
        L.check(scene, '問い返しで拒否の行が増えない', (await page.locator(REJECTED).count()) === rejected0);
        L.check(scene, '問い返しで進まない', (await engine(page)).current === STEP);

        // 全選択せずに続けて打つ: 順序違いの拒否になり、未知のコマンドにならない。
        await typeAndSend(page, '/advance-phase', how);
        await waitCount(page, REJECTED, rejected0 + 1);
        const e = await engine(page);
        const cats = e.judgedCategories;
        L.check(scene, '拒否の分類が 1 件だけ増えた（問い返しは数えない）', cats.length === judged0 + 1);
        const last = cats.at(-1)?.category;
        L.check(scene, '続けて打った入力が wrong_order', last === 'wrong_order');
        L.check(scene, '続けて打った入力が unknown_command でない', last !== 'unknown_command');
        L.check(scene, '拒否で進まない', e.current === STEP);

        // 拒否の後は下書きが残る（JUDGE-5）。書き戻しの 0 ミリ秒のタイマーが走り終わるのを待つ。
        await page.evaluate(() => new Promise((resolve) => { setTimeout(resolve, 0); }));
        L.check(scene, '拒否の後に欄の値が残る', (await page.inputValue(DRAFT)) === '/advance-phase');
        L.check(scene, '拒否の後に状態の下書きが残る', (await stateDraft(page)) === '/advance-phase');

        const run = await page.evaluate(() => window.__run);
        L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
        L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
        L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
        const counts = await collectSafety(page, rec);
        for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
        L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);

        console.log(`clarify-draft ${how} (${mode}): ${L.summary()} 項目`);
        assert.deepEqual(L.failed(), [], 'failed items');
        await context.close();
      }));
    });
  }
}
