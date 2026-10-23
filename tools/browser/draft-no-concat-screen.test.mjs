// 問い返し・拒否の後の下書きが連結されないことの画面の結果の試験（TASK-444。JUDGE-27・TERM-22。決定 D-67）。
//
// 試作（PoC-13 の AI 代理の準備）では、ST-1.4 の問い返しの後に `/advance-phase` と打つと `/new-idea/advance-phase` に
// 連結され、順序違い（wrong_order）でなく未知のコマンド（unknown_command）で拒否された。エンジンの対処は TASK-284
// （clarify-draft・reject-draft-select）が受け持つ。本試験は、その結果を画面（右のログの行と入力欄の値）だけで判定する。
//
// 証拠の種類: 自動ブラウザ・シミュレーター（AI の代理と同じ操作の再現。人の評価ではない）。
// 既存の 2 試験との差分:
//   - 判定は DOM だけ（received の本文・rejected の data-category と理由・次の行動・clarified の問い）。状態のフックは
//     ST-1.4 までの通し（journey）と安全の計数の付随の確認にだけ使う
//   - 経路 proxy は、AI 代理の道具（spec の ai-proxy/tools/proxyd.mjs）の type / key Enter を写す。type は
//     locator.focus() の後に setSelectionRange(len, len) でキャレットを末尾へ置き、keyboard.type する
//   - 経路 enter / button は page.keyboard だけ（全選択なし）
// 経路 proxy は、他の試験の自己規則（focus・evaluate を使わない）から外れる。代理の道具に忠実であることが目的だからである。
// setSelectionRange はキャレットの位置を書くだけで、DOM の値を状態へ流す読み取りではない。
//
// 対照: 代理の道具の type は拒否の後にキャレットを末尾へ動かし、エンジンの全選択を崩す。このため proxy の経路で
// 拒否の後に打つと今も連結される。これは道具の操作の性質で、エンジンの規則の不足ではない。連結が画面で見分けられること
// （判定が 0 件のまま通らないこと）の確認として、連結を期待して固定する。
//
// 確かめないこと: 実 IME・Safari・Firefox での選択の挙動（Chromium だけ）、0 ミリ秒のタイマーより速い打鍵、
// 送信ボタンの経路のフォーカスの位置、人の確認。
// 出力に、入力した値や下書きの中身は出さない（名前・件数・合否だけ）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  DRAFT, SUBMIT, LOG, CLARIFIED, REJECTED, T, engine, ledger, keyboardDriver, journey, install, keyTo, waitCount,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-draft-no-concat-screen/';
const QUERY = '?fixture=ch0-3';
const STEP = 'main/st-1-4';
const RECEIVED = `${LOG} li[data-kind="received"]`;
let browser;

const ir = JSON.parse(fs.readFileSync(new URL('../../crates/web/tests/data/ch0_3/ir.json', import.meta.url), 'utf8'));
const wrongOrder = ir.rejections[ir.defaults.wrong_order];
const unknownCmd = ir.rejections[ir.defaults.unknown_command];
const clarify = ir.clarifications['cl-new-idea-name'];

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

const waitDomDraft = (page, want) => page.waitForFunction(
  ([sel, v]) => document.querySelector(sel)?.value === v, [DRAFT, want], { timeout: T },
);
const waitSelectedAll = (page, want) => page.waitForFunction(
  ([sel, v]) => {
    const el = document.querySelector(sel);
    return el?.value === v && el.selectionStart === 0 && el.selectionEnd === v.length;
  }, [DRAFT, want], { timeout: T },
);
const count = (page, sel) => page.locator(sel).count();
const lastText = (page, sel) => page.evaluate((s) => {
  const li = [...document.querySelectorAll(s)].at(-1);
  return li ? li.textContent : null;
}, sel);
const lastBody = (page) => page.evaluate((s) => {
  const li = [...document.querySelectorAll(s)].at(-1);
  return li?.querySelector('[data-testid="line-body"]')?.textContent ?? null;
}, RECEIVED);
const lastPart = (page, testid) => page.evaluate(([s, t]) => {
  const li = [...document.querySelectorAll(s)].at(-1);
  return li?.querySelector(`[data-testid="${t}"]`)?.textContent ?? null;
}, [REJECTED, testid]);
const lastCategory = (page) => page.evaluate(
  (s) => [...document.querySelectorAll(s)].at(-1)?.getAttribute('data-category') ?? null, REJECTED,
);

// 代理の道具の focusInput(): フォーカスしてキャレットを末尾へ置く。
async function proxyFocus(page) {
  await page.locator(DRAFT).focus();
  await page.evaluate((sel) => {
    const el = document.querySelector(sel);
    el.setSelectionRange(el.value.length, el.value.length);
  }, DRAFT);
}

// 経路ごとに打って送る。全選択はしない。
async function typeAndSend(page, text, how) {
  if (how === 'proxy') {
    await proxyFocus(page);
    await page.keyboard.type(text);
    await proxyFocus(page);
    await page.keyboard.press('Enter');
    return;
  }
  await keyTo(page, DRAFT);
  await page.keyboard.type(text);
  if (how === 'button') await keyTo(page, SUBMIT);
  await page.keyboard.press('Enter');
}

// 問い返しを送り、画面の結果を台帳に積む。
async function clarifyScene(page, L, scene, how) {
  const rec0 = await count(page, RECEIVED);
  const rej0 = await count(page, REJECTED);
  const cla0 = await count(page, CLARIFIED);
  await typeAndSend(page, '/new-idea', how);
  await waitCount(page, CLARIFIED, cla0 + 1);
  await waitDomDraft(page, '');
  L.check(scene, '問い返しの受け取りの行が 1 件増えた', (await count(page, RECEIVED)) === rec0 + 1);
  L.check(scene, '受け取りの本文が送った入力ちょうど', (await lastBody(page)) === '/new-idea');
  const q = await lastText(page, CLARIFIED);
  L.check(scene, '問い返しの文面が教材の文面', q.includes(clarify.message) && q.includes(clarify.how_to_answer));
  L.check(scene, '問い返しで拒否の行が増えない', (await count(page, REJECTED)) === rej0);
  L.check(scene, '問い返しの後に欄が空', (await page.inputValue(DRAFT)) === '');
  return { rec0: rec0 + 1, rej0 };
}

// 問い返しの後に続けた入力が順序違いの拒否になることを台帳に積む。
async function afterClarifyScene(page, L, scene, how, base) {
  await typeAndSend(page, '/advance-phase', how);
  await waitCount(page, REJECTED, base.rej0 + 1);
  L.check(scene, '続けた入力の受け取りの行が 1 件増えた', (await count(page, RECEIVED)) === base.rec0 + 1);
  L.check(scene, '受け取りの本文が連結されず入力ちょうど', (await lastBody(page)) === '/advance-phase');
  L.check(scene, '拒否の行がちょうど 1 件増えた', (await count(page, REJECTED)) === base.rej0 + 1);
  L.check(scene, '拒否の分類が wrong_order', (await lastCategory(page)) === 'wrong_order');
  L.check(scene, '拒否の理由が順序違いの文面', (await lastPart(page, 'line-reason')) === wrongOrder.message);
  L.check(scene, '拒否の次の行動が順序違いの文面', (await lastPart(page, 'line-next-action')) === wrongOrder.next_action);
  const rejTxt = await lastText(page, REJECTED);
  L.check(scene, '未知のコマンドの分類・文面が出ない',
    (await lastCategory(page)) !== 'unknown_command' && !rejTxt.includes(unknownCmd.message));
  L.check(scene, 'Step が進まない', (await engine(page)).current === STEP);
}

async function open(srv, scene) {
  const o = await openPage(browser, { origin: srv.origin });
  await o.page.goto(`${srv.url}${QUERY}`);
  await waitMounted(o.page);
  assert.equal(await o.page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
  await install(o.page);
  const L = ledger();
  await journey(o.page, keyboardDriver(o.page), scene, L, STEP);
  assert.equal((await engine(o.page)).current, STEP);
  // 直前の複数行の送信の後、欄が空になるのは 0 ミリ秒のタイマーの書き戻しで非同期。待たずに打つと前の下書きに続くため、空を待つ。
  await waitDomDraft(o.page, '');
  return { ...o, L };
}

async function finish(page, rec, L, scene, label) {
  const e = await engine(page);
  const run = await page.evaluate(() => window.__run);
  L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
  L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
  L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
  const counts = await collectSafety(page, rec);
  for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
  L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
  console.log(`draft-no-concat-screen ${label}: ${L.summary()} 項目`);
  assert.deepEqual(L.failed(), [], 'failed items');
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  for (const how of ['proxy', 'enter', 'button']) {
    test(`問い返し・拒否の後の下書きが画面で連結されない: ${how}（${mode}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const scene = `問い返しの後（${how}）`;
        const { page, context, rec, L } = await open(srv, scene);
        const base = await clarifyScene(page, L, scene, how);
        await afterClarifyScene(page, L, scene, how, base);
        // TASK-443 と両立: 拒否の後に欄の値が残る。書き戻しの 0 ミリ秒のタイマーを待つ。
        await waitDomDraft(page, '/advance-phase');
        L.check(scene, '拒否の後に欄の値が残る', (await page.inputValue(DRAFT)) === '/advance-phase');

        const scene2 = `拒否の後（${how}）`;
        const rec1 = await count(page, RECEIVED);
        const rej1 = await count(page, REJECTED);
        const cla1 = await count(page, CLARIFIED);
        if (how === 'proxy') {
          // 対照: 代理の道具はキャレットを末尾へ動かすので、拒否の後に打つと連結される（道具の性質）。
          await typeAndSend(page, '/new-idea', how);
          await waitCount(page, REJECTED, rej1 + 1);
          L.check(scene2, '対照: 受け取りの本文が連結されて見分けられる', (await lastBody(page)) === '/advance-phase/new-idea');
          L.check(scene2, '対照: 連結された入力は問い返しにならない', (await count(page, CLARIFIED)) === cla1);
        } else {
          await waitSelectedAll(page, '/advance-phase');
          L.check(scene2, '拒否の後に欄の全体が選択されている', true);
          await typeAndSend(page, '/new-idea', how);
          await waitCount(page, CLARIFIED, cla1 + 1);
          await waitDomDraft(page, '');
          L.check(scene2, '受け取りの行が 1 件増えた', (await count(page, RECEIVED)) === rec1 + 1);
          L.check(scene2, '受け取りの本文が連結されず入力ちょうど', (await lastBody(page)) === '/new-idea');
          L.check(scene2, '拒否の行が増えない', (await count(page, REJECTED)) === rej1);
          L.check(scene2, '問い返しの後に欄が空', (await page.inputValue(DRAFT)) === '');
        }
        await finish(page, rec, L, scene, `${how} (${mode})`);
        await context.close();
      }));
    });
  }
}
