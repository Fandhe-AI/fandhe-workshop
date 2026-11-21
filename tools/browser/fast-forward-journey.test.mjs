// 早送りが入でも、確認・承認・差し戻し・復帰・拒否の場面が省かれず、受講者の入力があるまで先へ進まないことの画面の試験
// （SHELL-27。TASK-542 1/2。#1047。docs/testing.md 5.534）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。試験の設定（TEST_HOOKS=1）の `?fixture=ch0-3-chapters` を幅 1280px で開き、
// 早送りの切り替え（`fast-forward-toggle`）を実クリックで入にしてから、ch0-3-journey.mjs の journey で CH-0〜CH-3 の 20 Step を実クリックで通す。
//   N1  journey の台帳（入力待ち・確認待ちの口、拒否・問い返しで進まない、承認・差し戻し・対象つきの直す、章の境目）がすべて合格
//   N2  確認待ちになった全 16 時点（Step の集合を照合）で、700ms（進行の間隔 300ms の 2 回分より長い）待っても確認待ちのまま・待機中
//       （自動で達成・次の Step にならない）。入力待ちに入った全 20 時点でも、最初の入力（承認・差し戻しを含む）の前に同じく確かめる。
//       拒否・問い返しの直後と、章の終わりで「次の章へ進む」を押す前も、700ms 待って同じ Step・同じ状態・待機中で、測った 14 時点の Step と状態が期待の列と一致する
//   N3  処理中の段を持つ Step（st-1-5・st-2-5）で、進行がすべて適用されてから確認待ちになり（段が省かれない）、その処理で「早送り中」が出た
//       （早送りが実際に効いた。数 ms しか出ないため MutationObserver で記録する）
//   R   入のまま、st-1-5 に入った時点でチェックポイントを保存し、st-2-1 で復帰すると st-1-5 の入力待ちに戻り、700ms 待っても進まない。
//       送り直すと「早送り中」が出て進行がすべて適用されて確認待ちになり、700ms 待っても達成にならず、指す確認で達成になる
//   共通 早送りの状態の文字が、入力待ち・確認待ちの各時点と最後で「早送り: 入」。失敗の文には記録の件数だけを出す（記録の本文は出さない）。エンジンの警告・無視が 0 件。ページ内のエラー・CSP 違反が 0 件
// 停止（Esc）: 早送りの入でも、進行は最大 4 件の区切りごとに event loop へ戻るため、区切りの間の停止は受け付ける（5.526。native の
//   fast_forward_host.rs が通常の速さと同じ結果を確かめる）。ただし数 ms で終わるため、受講者が止める場面は `normal_speed` の Step に置く（SHELL-26）。
//   その画面の確認は fast-forward-toggle の G・H（5.528）が持つ。この試験は停止を扱わない。
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
const ACTIVE = '[data-testid="fast-forward-active"]';
// journey が拒否・問い返しを起こす 11 時点と、その時の Step の状態（試験用の教材の CH-0〜CH-3）。
// 章の境目の 3 時点（`:chapter-end`）も同じ列に並ぶ。
const REJECT_POINTS = [
  'main/st-0-1:entered', 'main/st-0-1:entered', 'main/st-0-2a:accepted', 'main/st-0-2c:chapter-end', 'main/st-1-1:entered',
  'main/st-1-4:entered', 'main/st-1-4:entered', 'main/st-1-6:chapter-end', 'main/st-2-6:entered', 'main/st-2-6:entered',
  'main/st-2-7:entered', 'main/st-2-7:accepted', 'main/st-2-8:chapter-end', 'main/st-3-1:entered',
];
const LOG_ROWS = '[data-testid="part-log-list"] li[data-kind="rejected"], [data-testid="part-log-list"] li[data-kind="clarified"]';
// journey が確認待ちにする 16 時点（試験用の教材の CH-0〜CH-3）。
const ACCEPTED = [
  'main/st-0-1', 'main/st-0-2a', 'main/st-0-2b', 'main/st-0-2c', 'main/st-1-1', 'main/st-1-4', 'main/st-1-5', 'main/st-1-6',
  'main/st-2-1', 'main/st-2-2', 'main/st-2-3', 'main/st-2-5', 'main/st-2-6', 'main/st-2-7', 'main/st-3-1', 'main/st-3-2',
];
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

// 待っても同じ Step・同じ状態で、待機中（状態表示が `idle`。表示が消えたときも偽）であること。
async function holds(page, uid, status) {
  await page.waitForTimeout(HOLD_MS);
  const e = await engine(page);
  return e.current === uid && (await statusOf(page, uid)) === status && (await runState(page)) === 'idle';
}

// 「早送り中」の出現の記録を始める（数 ms しか出ないため DOM の変化ごとに見る）。`seenActive` で読む。
const watchActive = (page) => page.evaluate((a) => {
  window.__ffJourneySeen = false;
  if (!window.__ffJourneyObserver) {
    window.__ffJourneyObserver = new MutationObserver(() => {
      if (document.querySelector(a)) window.__ffJourneySeen = true;
    });
    window.__ffJourneyObserver.observe(document.body, { childList: true, subtree: true, characterData: true });
  }
}, ACTIVE);
const seenActive = (page) => page.evaluate(() => window.__ffJourneySeen === true);
const logRows = (page) => page.evaluate((s) => document.querySelectorAll(s).length, LOG_ROWS);

// 拒否・問い返しの直後の静止を確かめる包み。各操作の前に、前の操作で拒否・問い返しの行が増えていれば、その時点の Step と状態が
// 700ms 後も同じで待機中であることを台帳に積む（journey の reject・clarify は行が出た直後に 1 回だけ見るため）。
// 測った時点は `held` に（Step:状態）で積み、呼び出し側が期待の列（REJECT_POINTS）と照合する。
function holdingDriver(page, inner, L, held) {
  let rows = 0;
  const wrap = (fn) => async (...args) => {
    const now = await logRows(page);
    if (now > rows) {
      const uid = (await engine(page)).current;
      const status = await statusOf(page, uid);
      held.push(`${uid}:${status}`);
      L.check('N2', `${uid} 拒否・問い返しの後も進まない（${now}）`, await holds(page, uid, status));
    }
    const result = await fn(...args);
    rows = now;
    return result;
  };
  const wrapped = Object.fromEntries(Object.entries(inner).map(([k, f]) => [k, wrap(f)]));
  // 章の終わり（最後の Step が達成で「次の章へ進む」が出ている）でも、押す前に 700ms 待って同じ Step の達成のまま・待機中であることを確かめる。
  const next = wrapped.nextChapter;
  wrapped.nextChapter = async (...args) => {
    const uid = (await engine(page)).current;
    held.push(`${uid}:chapter-end`);
    L.check('N2', `${uid} 章の終わりで「次の章へ進む」を押すまで進まない`, await holds(page, uid, 'achieved'));
    return next(...args);
  };
  return wrapped;
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
    const waits = [];
    const entered = [];
    const held = [];
    let enteredTicks = 0;
    const onStep = async (uid, status) => {
      // 承認・差し戻し・拒否などの場面を通っても早送りが入のまま（各時点で確かめる）。
      L.check('N1', `${uid} ${status} で早送りが入`, (await ffState(page)) === 'on');
      if (status === 'entered') {
        // 最初の入力（承認・差し戻しを含む）を送る前に、入力待ちのまま進まないことを確かめる。
        entered.push(uid);
        L.check('N2', `${uid} 入力待ちのまま進まない`, await holds(page, uid, 'entered'));
        enteredTicks = await tickCount(page);
        await watchActive(page);
        return;
      }
      waits.push(uid);
      if (PROCESSING[uid]) {
        L.check('N3', `${uid} 進行が ${PROCESSING[uid]} 件適用されてから確認待ち`, (await tickCount(page)) - enteredTicks === PROCESSING[uid]);
        L.check('N3', `${uid} の処理で「早送り中」が出た`, await seenActive(page));
      }
      L.check('N2', `${uid} 確認待ちのまま進まない`, await holds(page, uid, 'accepted'));
    };
    await journey(page, holdingDriver(page, mouseDriver(page), L, held), 'N1', L, null, { chaptered: true, onStep });
    L.check('N2', '確認待ちの 16 時点をすべて測った', JSON.stringify(waits) === JSON.stringify(ACCEPTED));
    L.check('N2', '入力待ちの 20 時点をすべて測った', entered.length === 20);
    L.check('N2', `拒否・問い返しの後の 11 時点と章の境目の 3 時点を期待の Step と状態で測った（${held.join(',')}）`, JSON.stringify(held) === JSON.stringify(REJECT_POINTS));
    L.check('end', '早送りが最後まで入', (await ffState(page)) === 'on');
    finish(L, 'end', await engine(page), rec);
    console.log(`diagnostic fast-forward on: ledger=${L.summary()} accepted-points=${waits.length}`);
    assert.deepEqual(L.failed(), [], JSON.stringify(summarize(rec)));
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
      L.check('R', `${uid} ${status} で早送りが入`, (await ffState(page)) === 'on');
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
    await watchActive(page);
    await page.fill(DRAFT, '/make-card sample');
    await page.press(DRAFT, 'Enter');
    await at(page, 'main/st-1-5', 'accepted');
    L.check('R', '送り直すと進行が段の数だけ増えてから確認待ち', (await tickCount(page)) - before === PROCESSING['main/st-1-5']);
    L.check('R', '送り直した処理で「早送り中」が出た', await seenActive(page));
    L.check('R', '確認待ちのまま達成にならない', await holds(page, 'main/st-1-5', 'accepted'));
    await page.click(rowSel('cards/sample'), { timeout: T });
    await at(page, 'main/st-1-6', 'entered');
    L.check('R', '指す確認で st-1-5 が達成', (await statusOf(page, 'main/st-1-5')) === 'achieved');
    L.check('R', '早送りが最後まで入', (await ffState(page)) === 'on');
    finish(L, 'R', await engine(page), rec);
    console.log(`diagnostic fast-forward restore: ledger=${L.summary()}`);
    assert.deepEqual(L.failed(), [], JSON.stringify(summarize(rec)));
    await context.close();
  });
});
