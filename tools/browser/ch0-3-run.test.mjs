// CH-0〜CH-3 の形の 20 Step を 1 つのページで通す試験と、CH-1 のキーボードだけの完走の試験（TASK-450 3/3。TERM-23）。
//
// 解釈: 教材 repo（CH-0〜CH-3 の本物）がまだ無く、core にも章の完了・遷移先の入口の写し・規則の条件（`when`）が無い。
// そのため本試験は、CH-0〜CH-3 の形（章ごとの Step 数・入力方式・確認の種類・拒否・問い返し・補完・処理中）を写した
// 試験用の教材（`crates/web/tests/data/ch0_3/ir.json`。`pack.id` は `ch0-3-run`）を、章をまたぐ無条件の `next` だけで
// 線形につないで通す。試作の 73 項目は目標にしない。教材 repo での流し直しは未実施（`docs/testing.md` の 5.117）。
// 試験の設定（TEST_HOOKS=1）の画面を `?fixture=ch0-3` で開くと、起動の前にこの教材が選ばれる。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい画面を開く）:
//   - 通し（実クリックと実キーだけ。`raw-trigger`・`load-fixture:` は 0 回）: 20 Step を最後の `main/st-3-2` の達成まで進む。
//     5 方式（コマンド・複数行の自然文・選択肢・画面操作・承認）と 4 種の確認（指す・答える・承認する・なし）を 1 回以上ずつ操作する。
//     各方式に入った直後と確認待ちで、出ている口（選択肢・承認・入力欄・送信）の数が表のとおりである。
//     拒否・問い返し・Tab の補完・Shift+Enter の改行・Esc の取り消し・処理中の完了待ちを通る
//   - キーボードだけ: `waitMounted` の後は `page.keyboard` だけで、先頭（`main/st-0-1`）から `main/st-2-1` に入るまで進む。
//     CH-1 の 6 Step がこの区間に入る。信頼されたマウスのイベント（pointerdown・mousedown・click）は 0 件
//   - どちらも、ページのエラー・配信元以外への通信・ダイアログ・危険な要素・CSP 違反が 0 件、エンジンの警告・無視が 0 件
//
// 確かめないこと:
//   - 処理中の Esc による停止。先頭以外の Step では入口の写しが無く、停止が警告で無視されるため通しに入れない（5.117 の限界）
//   - 試作の CH-0〜CH-3 の 73 項目。章の完了・次の章へ進む表示・チェックポイントの復帰の口は、製品の側にまだ無い
//   - ボタンにフォーカスがあるときの Esc（`main` に届かない。TASK-450 2/3 の対象外の事項）
//   - 試験の出力に、入力した値や下書きの中身は出さない（名前・件数・合否だけ）
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-ch0-3-run/';
const QUERY = '?fixture=ch0-3';
const DRAFT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const CHOICE = '[data-testid="choice-btn"]';
const APPROVAL = '[data-testid="approval-btn"]';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const REJECTED = `${LOG} li[data-kind="rejected"]`;
const CLARIFIED = `${LOG} li[data-kind="clarified"]`;
const T = 30000;
const TAB_LIMIT = 300;

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

const engine = (page) => page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine);

// 現在の Step が `uid` で、状態が `status` で、処理中でないのを待つ。`entered` は状態の欠落も含める。
const at = (page, uid, status) => page.waitForFunction(([u, st]) => {
  const e = JSON.parse(window.__fandheTest.stateJson()).engine;
  return e.current === u && (e.stateWithoutLog.run.slices.main?.status?.[u] ?? 'entered') === st
    && window.__fandheTest.nextTick() === 'null';
}, [uid, status], { timeout: T });

const rowSel = (path) => `[data-testid="part-artifact-view"] [data-row="${path}"]`;
const choiceSel = (id) => `${CHOICE}[data-choice="${id}"]`;
const decideSel = (d) => `${APPROVAL}[data-decision="${d}"]:not([data-revise-target])`;
const reviseSel = (t) => `${APPROVAL}[data-revise-target="${t}"]`;

// 4 つの口の数を (選択肢, 承認, 入力欄, 送信) の順で返す。
const ports = (page) => page.evaluate(([c, a, i, s]) => [c, a, i, s].map((sel) => document.querySelectorAll(sel).length),
  [CHOICE, APPROVAL, DRAFT, SUBMIT]);

const waitCount = (page, sel, n) => page.waitForFunction(([s, k]) => document.querySelectorAll(s).length === k, [sel, n], { timeout: T });
const draftValue = (page) => page.inputValue(DRAFT);

// 項目の台帳。途中の合否は台帳に積み、最後に全項目の合格を一度に確かめる。入力した値は載せない。
function ledger() {
  const items = [];
  return {
    items,
    check(scene, name, ok) {
      items.push({ scene, name, ok: ok === true });
    },
    summary() {
      return `${items.filter((i) => i.ok).length}/${items.length}`;
    },
    failed() {
      return items.filter((i) => !i.ok).map((i) => `${i.scene}: ${i.name}`);
    },
  };
}

// 実クリックの操作。`page.click` と入力欄への `page.fill`・`page.press`・`page.keyboard` だけを使う。
function mouseDriver(page) {
  return {
    choose: (id) => page.click(choiceSel(id), { timeout: T }),
    point: (path) => page.click(rowSel(path), { timeout: T }),
    decide: (d) => page.click(decideSel(d), { timeout: T }),
    revise: (t) => page.click(reviseSel(t), { timeout: T }),
    async send(text) {
      await page.fill(DRAFT, text);
      await page.press(DRAFT, 'Enter');
    },
    async sendMultiline(first, second) {
      await page.fill(DRAFT, first);
      await page.press(DRAFT, 'Shift+Enter');
      await page.keyboard.type(second);
      await page.press(DRAFT, 'Enter');
    },
    async completeAndSend(prefix, completed) {
      await page.fill(DRAFT, prefix);
      await page.press(DRAFT, 'Tab');
      await page.waitForFunction(([sel, v]) => document.querySelector(sel)?.value === v, [DRAFT, completed], { timeout: T });
      await page.press(DRAFT, 'Enter');
    },
    esc: () => page.press(DRAFT, 'Escape'),
  };
}

async function activeMatches(page, sel) {
  return page.evaluate((s) => document.activeElement?.matches(s) ?? false, sel);
}

// key を押して sel に着くまで進める。押した回数には上限があり、回数そのものは固定しない。
async function keyTo(page, sel, key = 'Tab') {
  for (let n = 0; n <= TAB_LIMIT; n += 1) {
    if (await activeMatches(page, sel)) return n;
    if (n === TAB_LIMIT) break;
    await page.keyboard.press(key);
  }
  assert.fail(`${key} で ${TAB_LIMIT} 回押しても ${sel} に着かない`);
  return -1;
}

// 入力欄にフォーカスを移し、残っている下書き（拒否の後は残る）を全選択して、次の入力で置き換わるようにする。
async function replaceDraft(page) {
  await keyTo(page, DRAFT);
  await page.keyboard.press('ControlOrMeta+A');
}

// `page.keyboard` だけの操作。`page.click`・`page.fill`・`page.focus` は使わない。
function keyboardDriver(page) {
  const activate = async (sel, key = 'Enter') => {
    await keyTo(page, sel);
    await page.keyboard.press(key);
  };
  return {
    choose: (id, key) => activate(choiceSel(id), key),
    point: (path) => activate(rowSel(path)),
    decide: (d) => activate(decideSel(d)),
    revise: (t) => activate(reviseSel(t)),
    async send(text) {
      await replaceDraft(page);
      await page.keyboard.type(text);
      await page.keyboard.press('Enter');
    },
    async sendMultiline(first, second) {
      await replaceDraft(page);
      await page.keyboard.type(first);
      await page.keyboard.press('Shift+Enter');
      await page.keyboard.type(second);
      await page.keyboard.press('Enter');
    },
    async completeAndSend(prefix, completed) {
      await replaceDraft(page);
      await page.keyboard.type(prefix);
      await page.keyboard.press('Tab');
      await page.waitForFunction(([sel, v]) => document.querySelector(sel)?.value === v, [DRAFT, completed], { timeout: T });
      await page.keyboard.press('Enter');
    },
    async esc() {
      await keyTo(page, DRAFT);
      await page.keyboard.press('Escape');
    },
  };
}

// 通し。`d` の操作で教材の 20 Step を上から進める。`stopAt` の Step に入った時点で返す（指定なしは最後の達成まで）。
async function journey(page, d, scene, L, stopAt = null) {
  let rejected = 0;
  let clarified = 0;
  const enter = async (uid, expected) => {
    await at(page, uid, 'entered');
    L.check(scene, `${uid} 入力待ちの口`, JSON.stringify(await ports(page)) === JSON.stringify(expected));
  };
  const wait = async (uid, expected) => {
    await at(page, uid, 'accepted');
    L.check(scene, `${uid} 確認待ちの口`, JSON.stringify(await ports(page)) === JSON.stringify(expected));
  };
  const reject = async (uid) => {
    rejected += 1;
    await waitCount(page, REJECTED, rejected);
    L.check(scene, `${uid} 拒否で進まない`, (await engine(page)).current === uid);
  };
  const clarify = async (uid) => {
    clarified += 1;
    await waitCount(page, CLARIFIED, clarified);
    L.check(scene, `${uid} 問い返しで進まない`, (await engine(page)).current === uid);
  };
  const plain = [0, 0, 1, 1];

  // CH-0: 選択肢（3 件）と、答える確認 3 つ。
  await enter('main/st-0-1', [3, 0, 1, 1]);
  await d.choose('opt-left');
  await reject('main/st-0-1');
  await d.choose('opt-top');
  await reject('main/st-0-1');
  await d.choose('opt-right');
  await wait('main/st-0-1', plain);
  await d.point('cards/notice');
  for (const [uid, next, read, wrong, right] of [
    ['main/st-0-2a', 'main/st-0-2b', 'opt-read-a', 'ans-a-ng', 'ans-a-ok'],
    ['main/st-0-2b', 'main/st-0-2c', 'opt-read-b', null, 'ans-b-ok'],
    ['main/st-0-2c', 'main/st-1-1', 'opt-read-c', null, 'ans-c-ok'],
  ]) {
    await enter(uid, [1, 0, 1, 1]);
    await d.choose(read);
    await wait(uid, [2, 0, 1, 1]);
    if (wrong !== null) {
      await d.choose(wrong);
      await reject(uid);
    }
    await d.choose(right, uid === 'main/st-0-2b' ? 'Space' : 'Enter');
    await at(page, next, 'entered');
  }

  // CH-1: コマンド・補完・複数行・問い返しと取り消し・処理中・選択肢。
  await enter('main/st-1-1', plain);
  await d.send('/hepl');
  await reject('main/st-1-1');
  await d.send('/help');
  await wait('main/st-1-1', plain);
  await d.point('cards/help-1');
  await enter('main/st-1-2', plain);
  await d.completeAndSend('/he', '/help');
  await enter('main/st-1-3', plain);
  await d.sendMultiline('memo line 1', 'memo line 2');
  await enter('main/st-1-4', plain);
  await d.send('/new-idea');
  await clarify('main/st-1-4');
  await d.send('/advance-phase');
  await reject('main/st-1-4');
  await d.esc();
  await wait('main/st-1-4', [2, 0, 1, 1]);
  await d.choose('asked-back');
  await enter('main/st-1-5', plain);
  await d.send('/make-card sample');
  await wait('main/st-1-5', plain);
  await d.point('cards/sample');
  await enter('main/st-1-6', [2, 0, 1, 1]);
  await d.choose('opt-right');
  await wait('main/st-1-6', [2, 0, 1, 1]);
  await d.choose('ans-restored');
  await at(page, 'main/st-2-1', 'entered');
  if (stopAt === 'main/st-2-1') return;

  // CH-2: コマンド・表示名の選択・承認・自然文・処理中・対象つきの直す・画面操作。
  await enter('main/st-2-1', plain);
  await d.send('/new-idea ideas');
  await wait('main/st-2-1', plain);
  await d.point('docs/readme');
  await enter('main/st-2-2', [1, 0, 1, 1]);
  await d.choose('opt-view');
  await wait('main/st-2-2', [2, 0, 1, 1]);
  await d.choose('ans-readme');
  await enter('main/st-2-3', [0, 3, 1, 1]);
  await d.decide('approve');
  await wait('main/st-2-3', plain);
  await d.point('docs/plan');
  await enter('main/st-2-4', plain);
  await d.sendMultiline('status line 1', 'status line 2');
  await enter('main/st-2-5', plain);
  await d.send('/run-check');
  await wait('main/st-2-5', plain);
  await d.point('checks/result');
  await enter('main/st-2-6', [0, 4, 1, 1]);
  await d.decide('revise');
  await reject('main/st-2-6');
  await d.decide('approve');
  await reject('main/st-2-6');
  await d.revise('t-expect');
  await wait('main/st-2-6', [0, 3, 1, 1]);
  await d.decide('approve');
  await enter('main/st-2-7', plain);
  await d.point('tasks/t-2');
  await reject('main/st-2-7');
  await d.point('tasks/t-1');
  await wait('main/st-2-7', [2, 0, 1, 1]);
  await d.choose('ans-t2');
  await reject('main/st-2-7');
  await d.choose('ans-t1');
  await enter('main/st-2-8', [1, 0, 1, 1]);
  await d.choose('opt-go');

  // CH-3: 選択肢の誤答と指す確認、最後の承認。
  await enter('main/st-3-1', [2, 0, 1, 1]);
  await d.choose('opt-place-ng');
  await reject('main/st-3-1');
  await d.choose('opt-place-ok');
  await wait('main/st-3-1', plain);
  await d.point('work/place');
  await enter('main/st-3-2', [0, 3, 1, 1]);
  await d.decide('approve');
  await wait('main/st-3-2', [0, 3, 1, 1]);
  await d.decide('approve');
  await at(page, 'main/st-3-2', 'achieved');
}

// 信頼されたマウスのイベントと、`raw-trigger` の click を数える（capture）。
async function install(page) {
  await page.evaluate((raw) => {
    window.__run = { mouse: 0, raw: 0 };
    for (const t of ['pointerdown', 'mousedown']) {
      window.addEventListener(t, (e) => {
        if (e.isTrusted) window.__run.mouse += 1;
      }, true);
    }
    // Enter・Space によるボタンの click も信頼されたイベントになる。マウスの click は detail が 1 以上、キーでは 0 である。
    window.addEventListener('click', (e) => {
      if (e.isTrusted && e.detail > 0) window.__run.mouse += 1;
    }, true);
    window.addEventListener('click', (e) => {
      if (e.target instanceof Element && e.target.closest(raw)) window.__run.raw += 1;
    }, true);
  }, RAW);
}

// 計数: 実キーと実マウスの別を分けるため、キーボードの区間は mouse が 0、通しは mouse が 1 以上であることも確かめる。
async function finish(page, rec, scene, L, { expectMouse }) {
  const run = await page.evaluate(() => window.__run);
  L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
  L.check(scene, expectMouse ? '実マウスの操作が通っている' : '信頼されたマウスのイベントが 0 件', expectMouse ? run.mouse > 0 : run.mouse === 0);
  const e = await engine(page);
  L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
  L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
  L.check(scene, '省略による達成が 0 件', e.log.filter((r) => r.kind === 'skipped').length === 0);
  const counts = await collectSafety(page, rec);
  for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
  L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
  return e;
}

async function scenario(csp, name, fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    const L = ledger();
    await page.goto(`${srv.url}${QUERY}`);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    await install(page);
    await fn(page, rec, L);
    console.log(`${name} (CSP ${csp ? 'あり' : 'なし'}): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  }));
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`20 Step を実クリックと実キーだけで最後まで通す（${mode}）`, async () => {
    await scenario(csp, 'through', async (page, rec, L) => {
      const scene = '通し';
      await journey(page, mouseDriver(page), scene, L);
      const e = await finish(page, rec, scene, L, { expectMouse: true });
      const status = e.stateWithoutLog.run.slices.main.status;
      L.check(scene, '最後の Step が達成', e.current === 'main/st-3-2' && status['main/st-3-2'] === 'achieved');
      L.check(scene, '20 Step がすべて達成', Object.keys(status).length === 20 && Object.values(status).every((s) => s === 'achieved'));
      L.check(scene, '達成が 20 件', e.log.filter((r) => r.kind === 'achieved').length === 20);
      L.check(scene, '配信元の外への通信が 0 件', rec.external.length === 0);
    });
  });

  test(`CH-1 の 6 Step を含む区間をキーボードだけで完走する（${mode}）`, async () => {
    await scenario(csp, 'keyboard', async (page, rec, L) => {
      const scene = 'キーボード';
      await journey(page, keyboardDriver(page), scene, L, 'main/st-2-1');
      const e = await finish(page, rec, scene, L, { expectMouse: false });
      const status = e.stateWithoutLog.run.slices.main.status;
      L.check(scene, 'CH-1 の 6 Step がすべて達成', [1, 2, 3, 4, 5, 6].every((n) => status[`main/st-1-${n}`] === 'achieved'));
      L.check(scene, 'main/st-2-1 に入った', e.current === 'main/st-2-1');
      L.check(scene, '入力欄の下書きが空', (await draftValue(page)) === '');
      L.check(scene, '口の数が入力待ちのとおり', JSON.stringify(await ports(page)) === JSON.stringify([0, 0, 1, 1]));
    });
  });
}
