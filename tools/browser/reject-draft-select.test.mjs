// 拒否の後の下書きが選択状態になり、続けて打った文字が連結されないことの試験（TASK-284 2/2。JUDGE-27・TERM-22。決定 D-67）。
//
// 拒否で残した下書きは全体を選択状態にし、全選択せずに打った文字で置き換わる（末尾に続けて入力されない）。
// 本番は host.rs の submit（selects_draft）と glue.rs の on_draft_select_request・sync_draft。
// Rust 側の対は crates/web/tests/reject_draft_select.rs。問い返しの後が空になることは clarify-draft.test.mjs（1/2）。
//
// 証拠の種類: 自動ブラウザ・シミュレーター（AI の代理と同じ操作の再現。人の評価ではない）。
// 操作は `page.keyboard` だけで、`page.fill`・`page.click`・全選択（Ctrl/Cmd+A）は使わない。
// 受け持ち（試験用の教材 `?fixture=ch0-3` の main/st-1-4。CSP あり・なし × 送信の経路 Enter・送信ボタン）:
//   - 問い返しの後に欄が空になり、続けて送った `/advance-phase` の拒否の後、欄の選択が下書きの全体になる
//   - 拒否の後に状態の下書きが残る（JUDGE-5 と両立する）
//   - 全選択せずに打った文字列で欄の値・状態の下書きが置き換わり、`/advance-phase` に連結されない
//   - 置き換えた入力を Enter で送ると、受け取った入力の本文が打った文字列ちょうどになる
//   - `raw-trigger` の click が 0 回、安全の計数が 0、エンジンの警告・無視が 0 件
//
// 確かめないこと: 送信ボタンの経路のフォーカスの位置（選択は欄に残るが、フォーカスはボタンのまま。D-67 に記述が無い）、
// 実 IME・Safari・Firefox での選択の挙動（Chromium だけ）、0 ミリ秒のタイマーより速い打鍵、人の確認。
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

const BASE = '/fandhe-reject-draft-select/';
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

// 欄の値が want で、選択が全体（先頭から末尾まで）になるのを待つ。固定の sleep は使わない。
const waitSelectedAll = (page, want) => page.waitForFunction(
  ([sel, v]) => {
    const el = document.querySelector(sel);
    return el?.value === v && el.selectionStart === 0 && el.selectionEnd === v.length;
  }, [DRAFT, want], { timeout: T },
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
    test(`拒否の後の下書きが選択状態で打った文字に置き換わる: ${how}（${mode}）`, async () => {
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

        // 拒否の後は下書きが残り、全体が選択状態になる（JUDGE-5・JUDGE-27）。選択は 0 ミリ秒のタイマーで付くので待つ。
        await waitSelectedAll(page, '/advance-phase');
        L.check(scene, '拒否の後に欄の値が残り全体が選択されている', true);
        L.check(scene, '拒否の後に状態の下書きが残る', (await stateDraft(page)) === '/advance-phase');

        // 全選択せずに打つ: 下書きが置き換わり、連結されない。
        const typed = '/new-idea';
        await keyTo(page, DRAFT);
        await page.keyboard.type(typed);
        await waitDomDraft(page, typed);
        L.check(scene, '打った文字列で欄の値が置き換わる', (await page.inputValue(DRAFT)) === typed);
        L.check(scene, '打った文字列で状態の下書きが置き換わる', (await stateDraft(page)) === typed);
        L.check(scene, '打った文字が下書きに連結されない', !(await page.inputValue(DRAFT)).includes('/advance-phase'));
        const received0 = (await engine(page)).received.length;
        await page.keyboard.press('Enter');
        await waitDomDraft(page, '');
        const e2 = await engine(page);
        L.check(scene, '送った入力が打った文字列ちょうど', e2.received.length === received0 + 1 && e2.received.at(-1).input.command === typed);

        const run = await page.evaluate(() => window.__run);
        L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
        L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
        L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
        const counts = await collectSafety(page, rec);
        for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
        L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);

        console.log(`reject-draft-select ${how} (${mode}): ${L.summary()} 項目`);
        assert.deepEqual(L.failed(), [], 'failed items');
        await context.close();
      }));
    });
  }
}
