// 早送りが入でも、確認・承認・差し戻し・復帰・拒否の場面が省かれず、受講者の入力があるまで先へ進まないことの画面の試験
// （SHELL-27。TASK-542 1/2。#1047。docs/testing.md 5.534）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。試験の設定（TEST_HOOKS=1）の `?fixture=ch0-3-chapters` を幅 1280px で開き、
// 早送りの切り替え（`fast-forward-toggle`）を実クリックで入にしてから、ch0-3-journey.mjs の journey で CH-0〜CH-3 の 20 Step を実クリックで通す。
//   N1  journey の台帳（入力待ち・確認待ちの口、拒否・問い返しで進まない、承認・差し戻し・対象つきの直す、章の境目）がすべて合格
//   N2  確認待ちになった全時点で、700ms（進行の間隔 300ms の 2 回分より長い）待っても確認待ちのまま・処理中でない（自動で達成・次の Step にならない）
//   N3  処理中の段を持つ Step（st-1-5・st-2-5）で、進行がすべて適用されてから確認待ちになる（段が省かれない）
//   R   入のまま、st-1-5 に入った時点でチェックポイントを保存し、st-2-1 で復帰すると st-1-5 の入力待ちに戻り、700ms 待っても進まない。
//       送り直すと進行がすべて適用されて確認待ちになり、700ms 待っても達成にならず、指す確認で達成になる
//   共通 早送りの状態の文字が最後まで「早送り: 入」。エンジンの警告・無視が 0 件。ページ内のエラー・CSP 違反が 0 件
// 停止（Esc）: 早送りの入では処理中の段が間を置かずに適用されるため、止める場面は `normal_speed` の Step だけにある（SHELL-26）。
//   その画面の確認は fast-forward-toggle の G・H（5.528）が持ち、ここでは複製しない。
// 入・切で進行の数・ops・左の最終的な変化が一致することは TASK-542 2/2（#1048）の担当。
// 出力に入力した値は出さない。入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted,
} from './harness.mjs';
import {
  ledger, mouseDriver, journey, engine, at, rowSel, DRAFT, T,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-fast-forward-journey/';
const QUERY = '?fixture=ch0-3-chapters';
const TOGGLE = '[data-testid="fast-forward-toggle"]';
const STATUS = '[data-testid="fast-forward-status"]';
const SAVE = '[data-testid="checkpoint-save"]';
const RESTORE = '[data-testid="checkpoint-restore"]';
const RUN = '[data-testid="run-status"]';
const HOLD_MS = 700;
// 処理中の段を持つ Step と段の数（試験用の教材 `crates/web/tests/data/ch0_3/ir.json` の `processing`）。
const PROCESSING = { 'main/st-1-5': 4, 'main/st-2-5': 2 };
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

const statusOf = async (page, uid) => (await engine(page)).stateWithoutLog.run.slices.main?.status?.[uid] ?? 'entered';
const runState = (page) => page.evaluate((s) => document.querySelector(s)?.getAttribute('data-state') ?? null, RUN);
const ffState = (page) => page.evaluate((s) => document.querySelector(s)?.getAttribute('data-fast-forward') ?? null, STATUS);
// 適用された進行の記録の数（engine の ticks は記録の中の Progress の (世代, 番号) の列）。
const tickCount = async (page) => (await engine(page)).ticks.length;

// 待っても同じ Step・同じ状態で、処理中でないこと。
async function holds(page, uid, status) {
  await page.waitForTimeout(HOLD_MS);
  const e = await engine(page);
  return e.current === uid && (await statusOf(page, uid)) === status && (await runState(page)) !== 'busy';
}

async function openOn(srv) {
  const opened = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
  const { page } = opened;
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.goto(`${srv.url}${QUERY}`);
  await waitMounted(page);
  await page.click(TOGGLE);
  await page.waitForFunction((s) => document.querySelector(s)?.getAttribute('data-fast-forward') === 'on', STATUS, { timeout: T });
  return opened;
}

function finish(L, scene, e, rec) {
  L.check(scene, 'エンジンの警告・無視が 0 件', e.log.filter((r) => r.kind === 'warning' || r.kind === 'ignored').length === 0);
  const s = summarize(rec);
  L.check(scene, 'ページ内のエラー・CSP 違反が 0 件', s.pageErrors === 0 && s.csp === 0);
}

test('早送りが入でも 20 Step の確認・承認・差し戻し・拒否の場面が省かれず、入力まで進まない（N1〜N3）', { timeout: 600000 }, async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openOn(srv);
    const L = ledger();
    let waits = 0;
    let enteredTicks = 0;
    const onStep = async (uid, status) => {
      if (status === 'entered') {
        enteredTicks = await tickCount(page);
        return;
      }
      waits += 1;
      if (PROCESSING[uid]) {
        L.check('N3', `${uid} 進行が ${PROCESSING[uid]} 件適用されてから確認待ち`, (await tickCount(page)) - enteredTicks === PROCESSING[uid]);
      }
      L.check('N2', `${uid} 確認待ちのまま進まない`, await holds(page, uid, 'accepted'));
    };
    await journey(page, mouseDriver(page), 'N1', L, null, { chaptered: true, onStep });
    L.check('N2', '確認待ちの時点を 1 つ以上測った', waits >= 1);
    L.check('end', '早送りが最後まで入', (await ffState(page)) === 'on');
    finish(L, 'end', await engine(page), rec);
    console.log(`diagnostic fast-forward on: ledger=${L.summary()} accepted-points=${waits}`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  });
});

test('早送りが入でも、チェックポイントへの復帰の後は入力まで進まず、確認を省かない（R）', { timeout: 600000 }, async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openOn(srv);
    const L = ledger();
    // 保存した 1 件の ID。章の終わりの自動保存（TASK-339）も一覧に並ぶため、保存の前後の差で引く。
    let saved = null;
    const listed = () => page.evaluate((sel) => [...document.querySelectorAll(sel)].map((b) => b.getAttribute('data-checkpoint')), RESTORE);
    const onStep = async (uid, status) => {
      if (uid === 'main/st-1-5' && status === 'entered' && saved === null) {
        const ids = await listed();
        await page.click(SAVE);
        await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n + 1, [RESTORE, ids.length], { timeout: T });
        saved = (await listed()).find((id) => !ids.includes(id)) ?? null;
      }
    };
    await journey(page, mouseDriver(page), 'R', L, 'main/st-2-1', { chaptered: true, onStep });
    L.check('R', 'st-1-5 で保存した', saved !== null);
    await page.click(`${RESTORE}[data-checkpoint="${saved}"]`, { timeout: T });
    await at(page, 'main/st-1-5', 'entered');
    L.check('R', '復帰の後は st-1-5 の入力待ちのまま進まない', await holds(page, 'main/st-1-5', 'entered'));
    const before = await tickCount(page);
    await page.fill(DRAFT, '/make-card sample');
    await page.press(DRAFT, 'Enter');
    await at(page, 'main/st-1-5', 'accepted');
    L.check('R', '送り直すと進行が段の数だけ増えてから確認待ち', (await tickCount(page)) - before === PROCESSING['main/st-1-5']);
    L.check('R', '確認待ちのまま達成にならない', await holds(page, 'main/st-1-5', 'accepted'));
    await page.click(rowSel('cards/sample'), { timeout: T });
    await at(page, 'main/st-1-6', 'entered');
    L.check('R', '指す確認で st-1-5 が達成', (await statusOf(page, 'main/st-1-5')) === 'achieved');
    L.check('R', '早送りが最後まで入', (await ffState(page)) === 'on');
    finish(L, 'R', await engine(page), rec);
    console.log(`diagnostic fast-forward restore: ledger=${L.summary()}`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  });
});
