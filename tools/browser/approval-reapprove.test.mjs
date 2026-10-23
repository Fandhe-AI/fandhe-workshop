// 差し戻しの後の再承認の画面の試験（TERM-28・TERM-29・JUDGE-11・PACK-32。TASK-448 2/2）。
//
// 決定 D-69: 1 Step の `approval` に再承認の語は足さず、差し戻しの Step と再承認の Step を分けて `next` でつなぐ。
// その形で、差し戻し → 再承認が画面で進むことを、CSP あり・なしの両方で確かめる（1 回ごとに新しい画面）。
// 期待値（節点の名前・ボタンの文字・件数）はこのファイルの定数で独自に持ち、画面の `data-*` を読み返して合格にしない。
//
// 場面:
//   - 承認の部品: `load-fixture:approval-reapprove`（st-1 が候補 3 件の差し戻し、st-2 が再承認、st-3 が最後）
//   - ST-2.6: `?fixture=ch0-3-reapprove&start=s-2-6`（st-2-6 の確認の段を経て、再承認の st-2-6b、st-2-7 へ）
// 各場面で確かめること:
//   - 誤った候補を実クリックすると拒否の行が 1 件増え、Step は変わらない。正しい対象で受付になる
//   - 再承認の Step では、ボタンが「はい」「いいえ」「直す」の 3 件で、対象つきが 0 件
//   - 「直す」「いいえ」の実クリックはそれぞれ拒否の行が 1 件増え、Step は変わらずボタンが残る
//   - 「はい」の実クリックで次の Step へ進む
//   - 危険な要素・実行・CSP 違反・ダイアログが 0 件
// ST-2.6 の場面では、再承認の Step にいる時点の再読み込みの後も同じ Step・同じボタンに戻ることも確かめる。
// 確かめないこと: ST-4.1・ST-6.1 の再承認（TASK-672）。操作のログの行に ID が出る件数は観察値として記録するだけで assert しない。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-approval-reapprove/';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const BTN = '[data-testid="approval-btn"]';
const TARGETED = `${BTN}[data-revise-target]`;
const THREE = ['はい', 'いいえ', '直す'];

let browser;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function sendRaw(page, payload) {
  await page.evaluate(([sel, p]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', p);
    el.click();
  }, [RAW, payload]);
}

function snapshot(page) {
  return page.evaluate(([log, btn]) => {
    const engine = JSON.parse(window.__fandheTest.stateJson()).engine;
    return {
      engine,
      rejected: document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length,
      buttons: [...document.querySelectorAll(btn)].map((b) => ({
        text: b.textContent,
        target: b.dataset.reviseTarget ?? null,
      })),
      logText: document.querySelector(log)?.textContent ?? '',
    };
  }, [LOG, BTN]);
}

// 段の判別は engine.current で行い、ボタンの件数だけで待たない。
async function waitStep(page, step) {
  await page.waitForFunction(
    (s) => JSON.parse(window.__fandheTest.stateJson()).engine.current === s, step, { timeout: 30000 },
  );
}

async function waitRejected(page, n) {
  await page.waitForFunction(
    ([log, k]) => document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length === k, [LOG, n],
  );
}

async function clickBtn(page, name) {
  await page.getByRole('button', { name, exact: true }).click();
}

// 誤った候補 → 正しい対象。正しい対象を押した後の段は呼び出し側が確かめる。
async function reviseStep(page, scene) {
  const { step, wrong, wrongLabel, correctLabel } = scene;
  await page.waitForSelector(TARGETED, { timeout: 30000 });
  let s = await snapshot(page);
  assert.equal(s.engine.current, step);
  const base = s.rejected;
  await clickBtn(page, `直す（対象: ${wrongLabel}）`);
  await waitRejected(page, base + 1);
  s = await snapshot(page);
  assert.equal(s.engine.current, step, '誤った候補で Step は変わらない');
  assert.equal(s.buttons.length, 6, '拒否の後もボタンが残る');
  console.log(`REAPPROVE step=${step} log-id-hits=${s.logText.split(wrong).length - 1}`);
  await clickBtn(page, `直す（対象: ${correctLabel}）`);
  return base + 1;
}

// 再承認の Step: 3 件・対象つき 0 件 → 直す・いいえは拒否 → はい。
async function reapproveStep(page, step, next, rejectedBefore) {
  await waitStep(page, step);
  await page.waitForSelector(BTN, { timeout: 30000 });
  let s = await snapshot(page);
  assert.deepEqual(s.buttons.map((b) => b.text), THREE);
  assert.equal(s.buttons.filter((b) => b.target !== null).length, 0, '再承認の Step は対象つきを出さない');
  assert.equal(s.rejected, rejectedBefore);
  await clickBtn(page, '直す');
  await waitRejected(page, rejectedBefore + 1);
  await clickBtn(page, 'いいえ');
  await waitRejected(page, rejectedBefore + 2);
  s = await snapshot(page);
  assert.equal(s.engine.current, step, '拒否で Step は変わらない');
  assert.deepEqual(s.buttons.map((b) => b.text), THREE, '拒否の後もボタンが残る');
  await clickBtn(page, 'はい');
  await waitStep(page, next);
  return rejectedBefore + 2;
}

async function withPage(csp, query, body) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(`${srv.url}${query}`);
    await waitMounted(page);
    await body(page);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
    await context.close();
  }));
}

for (const csp of [true, false]) {
  const mode = csp ? 'あり' : 'なし';
  test(`承認の部品: 差し戻しの後に再承認の Step で はい が進み 直す・いいえ は拒否（CSP ${mode}）`, async () => {
    await withPage(csp, '', async (page) => {
      await sendRaw(page, 'load-fixture:approval-reapprove');
      const rej = await reviseStep(page, {
        step: 'main/st-1', wrong: 't-gamma', wrongLabel: '中ほどの図表', correctLabel: '冒頭の見出し',
      });
      const after2 = await reapproveStep(page, 'main/st-2', 'main/st-3', rej);
      const s = await snapshot(page);
      assert.equal(s.rejected, after2);
      assert.deepEqual(s.buttons.map((b) => b.text), THREE, '最後の Step は入力待ちの 3 件');
      assert.ok(s.logText.includes('承認の操作: はい'), s.logText);
    });
  });

  test(`ST-2.6: 確認の段を経て再承認の Step へ進み 再読み込みの後も戻る（CSP ${mode}）`, async () => {
    await withPage(csp, '?fixture=ch0-3-reapprove&start=s-2-6', async (page) => {
      const rej = await reviseStep(page, {
        step: 'main/st-2-6', wrong: 't-other-a', wrongLabel: '冒頭の見出し', correctLabel: '期待する結果',
      });
      // 受付の後は st-2-6 のまま確認の段（対象つき 0 件の 3 件）。
      await page.waitForFunction(
        (btn) => document.querySelectorAll(btn).length === 3 && !document.querySelector(`${btn}[data-revise-target]`), BTN,
      );
      let s = await snapshot(page);
      assert.equal(s.engine.current, 'main/st-2-6');
      assert.deepEqual(s.buttons.map((b) => b.text), THREE);
      await clickBtn(page, 'はい');
      await waitStep(page, 'main/st-2-6b');

      // 再承認の Step にいる時点で再読み込みしても、同じ Step・同じボタンに戻る。
      await page.reload();
      await waitMounted(page);
      await waitStep(page, 'main/st-2-6b');
      await page.waitForSelector(BTN, { timeout: 30000 });
      s = await snapshot(page);
      assert.deepEqual(s.buttons.map((b) => b.text), THREE);
      assert.equal(s.buttons.filter((b) => b.target !== null).length, 0);

      await reapproveStep(page, 'main/st-2-6b', 'main/st-2-7', s.rejected);
      s = await snapshot(page);
      assert.equal(s.buttons.length, 0, 'st-2-7 は承認のボタンを出さない');
      assert.ok(rej >= 1);
    });
  });
}
