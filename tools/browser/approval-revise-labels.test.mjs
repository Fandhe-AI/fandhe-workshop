// 承認の「直す」の候補と表示名の画面の試験（TERM-28・INTRO-29・JUDGE-11。TASK-448 1/2）。
//
// 誤った候補を含む「直す」のボタンが、ID でなく表示名で出ることを、CSP あり・なしの両方で確かめる（1 回ごとに新しい画面）。
// 期待値（候補の ID・表示名・数・順）はこのファイルの定数で独自に持ち、画面の `data-*` を読み返して合格にしない。
//
// 場面:
//   - 承認の部品: `load-fixture:approval-candidates`（候補 3 件。t-gamma は規則に無い誤った候補）
//   - ST-2.6: `?fixture=ch0-3-revise&start=s-2-6`（候補 3 件。t-expect が正しい対象、他の 2 件は誤った候補）
//   - 対照: `load-fixture:approval`（候補の宣言なし）では、文字が ID になる（決定の前の現状の観察。数え方の検出力の確認）
// 各場面で確かめること:
//   - 対象つきのボタンが宣言順に候補の数だけ出る（規則の対象数より多い）。文字は `直す（対象: 表示名）` で、
//     見える文字・aria-label・title に候補の ID が出る件数が 0
//   - 対象つきボタンの属性名の集合が全候補で同じで、どれが正しい対象かを示す属性が無い
//   - 誤った候補を実クリックすると拒否の行が 1 件増え、Step は変わらず、ボタンが残る。正しい対象を押すと確認の段の 3 件になる
//   - 危険な要素・実行・CSP 違反・ダイアログが 0 件
// 操作のログの行に ID が出る件数は観察値として記録するだけで assert しない（log_lines は本件の範囲外）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-approval-revise-labels/';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const BTN = '[data-testid="approval-btn"]';
const TARGETED = `${BTN}[data-revise-target]`;

const SCENES = {
  part: {
    candidates: [
      { id: 't-alpha', label: '冒頭の見出し' },
      { id: 't-gamma', label: '中ほどの図表' },
      { id: 't-beta', label: '末尾の注記' },
    ],
    correct: 't-alpha',
    wrong: 't-gamma',
    step: 'main/st-1',
  },
  st26: {
    candidates: [
      { id: 't-other-a', label: '冒頭の見出し' },
      { id: 't-expect', label: '期待する結果' },
      { id: 't-other-b', label: '末尾の注記' },
    ],
    correct: 't-expect',
    wrong: 't-other-a',
    step: 'main/st-2-6',
  },
};

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
    const rejected = document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length;
    const buttons = [...document.querySelectorAll(btn)].map((b) => ({
      text: b.textContent,
      target: b.dataset.reviseTarget ?? null,
      payload: b.dataset.payload,
      attrNames: b.getAttributeNames().sort(),
      aria: b.getAttribute('aria-label'),
      title: b.getAttribute('title'),
    }));
    return { engine, rejected, buttons, logText: document.querySelector(log)?.textContent ?? '' };
  }, [LOG, BTN]);
}

// 見える文字・aria-label・title に ID が出る件数。
function idHits(buttons, ids) {
  let n = 0;
  for (const b of buttons) {
    for (const id of ids) {
      for (const s of [b.text, b.aria, b.title]) if (s && s.includes(id)) n += 1;
    }
  }
  return n;
}

async function checkScene(page, scene) {
  const { candidates, correct, wrong, step } = scene;
  await page.waitForSelector(TARGETED, { timeout: 30000 });
  let s = await snapshot(page);
  assert.equal(s.engine.current, step);
  assert.equal(s.buttons.length, 3 + candidates.length, JSON.stringify(s.buttons));
  const targeted = s.buttons.slice(3);
  assert.deepEqual(targeted.map((b) => b.target), candidates.map((c) => c.id), '宣言順');
  assert.deepEqual(targeted.map((b) => b.payload), candidates.map((c) => c.id));
  assert.deepEqual(targeted.map((b) => b.text), candidates.map((c) => `直す（対象: ${c.label}）`));
  assert.equal(idHits(s.buttons, candidates.map((c) => c.id)), 0, 'ID が見える文字・aria-label・title に出ない');
  const names = new Set(targeted.map((b) => b.attrNames.join(',')));
  assert.equal(names.size, 1, `対象つきボタンの属性名の集合が全候補で同じ: ${[...names]}`);
  const rejectedBefore = s.rejected;

  // 誤った候補: 実クリックで拒否になり、Step は変わらずボタンが残る。
  const wrongLabel = candidates.find((c) => c.id === wrong).label;
  await page.getByRole('button', { name: `直す（対象: ${wrongLabel}）` }).click();
  await page.waitForFunction(([log, n]) => document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length > n, [LOG, rejectedBefore]);
  s = await snapshot(page);
  assert.equal(s.rejected, rejectedBefore + 1);
  assert.equal(s.engine.current, step);
  assert.equal(s.buttons.length, 3 + candidates.length, '拒否の後もボタンが残る');
  console.log(`REVISE-LABELS step=${step} log-id-hits=${s.logText.split(wrong).length - 1}`);

  // 正しい対象: 受付になり、確認の段の 3 件に切り替わる。
  const okLabel = candidates.find((c) => c.id === correct).label;
  await page.getByRole('button', { name: `直す（対象: ${okLabel}）` }).click();
  await page.waitForFunction((btn) => document.querySelectorAll(btn).length === 3
    && !document.querySelector(`${btn}[data-revise-target]`), BTN);
  s = await snapshot(page);
  assert.equal(s.rejected, rejectedBefore + 1, '正しい対象で拒否の行は増えない');
  assert.deepEqual(s.buttons.map((b) => b.text), ['はい', 'いいえ', '直す']);
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
  test(`承認の部品: 誤った候補を含む直すが表示名で出る（CSP ${mode}）`, async () => {
    await withPage(csp, '', async (page) => {
      await sendRaw(page, 'load-fixture:approval-candidates');
      await checkScene(page, SCENES.part);
    });
  });

  test(`ST-2.6: 誤った候補を含む直すが表示名で出る（CSP ${mode}）`, async () => {
    await withPage(csp, '?fixture=ch0-3-revise&start=s-2-6', (page) => checkScene(page, SCENES.st26));
  });

  test(`対照: 候補の宣言が無い Step では文字が ID になる（CSP ${mode}）`, async () => {
    await withPage(csp, '', async (page) => {
      await sendRaw(page, 'load-fixture:approval');
      await page.waitForSelector(TARGETED, { timeout: 30000 });
      const s = await snapshot(page);
      const ids = ['t-alpha', 't-beta'];
      assert.deepEqual(s.buttons.slice(3).map((b) => b.text), ids.map((t) => `直す（対象: ${t}）`));
      assert.equal(idHits(s.buttons, ids), ids.length, '数え方は ID の表示を検出する');
    });
  });
}
