// 確認の前の達成と連打の縦断試験（TASK-222 1/3。B-1・B-2a〜B-2c。EVT-3・EVT-7・EVT-16）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、実タイマーの進行（表示側の 300ms）を流しながら、状態の不変条件を確かめる。
//   B-1  確認を省くと明示していない Step は、5 段の進行が終わっても「達成」にならない。確認して初めて達成になる
//   B-2a 処理中の Step への連打（20 回）は、受付が 1 回で、残り 19 回は処理中の拒否（`busy`）になる
//   B-2b 選択の連打（20 回）は、受付が 1 回で、st-2 は達成して st-3 に進み、状態が壊れない
//   B-2c 受付の後も残るボタンへの連打（20 回）は、すべて規則による拒否（`not_in_this_step`）で、状態は変わらない
//
// 入力は、画面の入力の口がまだエンジンに結線されていないため、見えない `raw-trigger` に `Op` の JSON を書いて click する
// 経路で送る（tick-dispatch・three-views と同じ）。進行（tick）だけは手で送らず、実タイマーに任せる。
// 進行の間隔は 300ms である。spec の TASK-222 は 60ms と書くが、D-47・EVT-18 が後の決定でビルド時の定数 300ms にした。
// 間隔を変える口は足さない（tick_interval.rs が禁じる）。5 段の進行は約 1.5 秒かかる。
// B-2b・B-2c の元の PoC は、受付で選択肢のボタンが DOM から消え、届く click が 1 回になることまで見ていた。
// 選択肢・承認のボタンが画面にまだ無いため、本試験は core に届く入力の不変条件だけを見る。ボタンを使う版は、
// ボタンを足すタスクの範囲とする。
// 連打は 1 回の `page.evaluate` の中で 50ms 間隔に送る（Playwright の往復を挟むと B-2a の時間の窓を越えるため）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-confirm-burst/';
const RAW = '[data-testid="raw-trigger"]';
const COMMAND = { input: { command: '/make-note first-note' } };
const POINT = { input: { ui: { view: 'v-files', path: 'notes/draft' } } };
const CHOICE = { input: { choice: 'opt-memo' } };
const APPROVE = { input: { approval: { decision: 'approve', target: null } } };
const REJECT = { input: { approval: { decision: 'reject', target: null } } };
const BURST = 20;
const GAP_MS = 50;
const TOTAL_TICKS = 5;

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

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

// 同じ入力を、1 回の evaluate の中で間隔を置いて n 回送る。かかった時間（ms）を返す。
async function sendBurst(page, op, n = BURST, gap = GAP_MS) {
  return page.evaluate(async ([sel, json, count, gapMs]) => {
    const el = document.querySelector(sel);
    const t0 = performance.now();
    for (let i = 0; i < count; i += 1) {
      el.setAttribute('data-payload', json);
      el.click();
      await new Promise((r) => setTimeout(r, gapMs));
    }
    return performance.now() - t0;
  }, [RAW, JSON.stringify(op), n, gap]);
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const nextTick = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.nextTick()));

// 条件が真になるまで待つ。上限を超えたら、最後の状態を添えて失敗にする。
async function waitUntil(page, what, pred, timeout = 8000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await engine(page);
    if (pred(last)) return last;
    await page.waitForTimeout(25);
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

const kindCount = (e, kind, step) => e.log.filter((r) => r.kind === kind && (step === undefined || r.step === step)).length;
const categoryCount = (e, category) => e.judgedCategories.filter((j) => j.category === category).length;
const ticksDone = (e) => e.ticks.length >= TOTAL_TICKS;

async function openTestPage(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

async function assertSafe(page, rec) {
  const counts = await collectSafety(page, rec);
  assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
}

function assertConsistent(e) {
  assert.equal(e.incrementalMatches, true);
  assert.equal(e.worldMatchesState, true);
}

test('B-1 確認の前は達成にならず、確認して初めて達成になる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    const interval = await page.evaluate(() => window.__fandheTest.tickIntervalMs());
    await sendRaw(page, COMMAND);
    assert.deepEqual(await nextTick(page), { generation: 1, step: 'main/st-1', index: 0 });
    await waitUntil(page, 'all ticks', ticksDone);
    assert.equal(await nextTick(page), null, '5 段の後は予約が無い');
    // 進行の後に、さらに間隔の 2 倍だけ待っても達成にならない（確認の前は達成にならない）。
    await page.waitForTimeout(interval * 2);
    const settled = await engine(page);
    assert.equal(settled.current, 'main/st-1');
    assert.equal(kindCount(settled, 'accepted', 'main/st-1'), 1);
    assert.equal(kindCount(settled, 'achieved', 'main/st-1'), 0, '確認の前に達成が出ている');
    assert.deepEqual(settled.review.map((i) => [i.step, i.achieved]), [['main/st-1', false]]);

    // 確認（指す操作）で初めて達成になり、次の Step に進む。
    await sendRaw(page, POINT);
    const confirmed = await engine(page);
    assert.equal(kindCount(confirmed, 'achieved', 'main/st-1'), 1);
    assert.equal(confirmed.current, 'main/st-2');
    assert.deepEqual(confirmed.review.map((i) => [i.step, i.achieved]), [['main/st-1', true]]);

    // st-3 は、承認の 1 回目では達成にならず、確認の承認で初めて達成になる。
    await sendRaw(page, CHOICE);
    assert.equal((await engine(page)).current, 'main/st-3');
    await sendRaw(page, APPROVE);
    const first = await engine(page);
    assert.equal(kindCount(first, 'accepted', 'main/st-3'), 1);
    assert.equal(kindCount(first, 'achieved', 'main/st-3'), 0, '確認の承認の前に達成が出ている');
    await sendRaw(page, APPROVE);
    const last = await engine(page);
    assert.equal(kindCount(last, 'achieved', 'main/st-3'), 1);
    assert.equal(kindCount(last, 'achieved'), 3);
    assertConsistent(last);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('B-2a 処理中の連打は、受付が 1 回で、残りは busy になる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    // 処理中の窓に収まったかは、実時間の上限ではなく、下のログの順序（受付が最後の進行より前）で判定する。
    // 負荷で setTimeout が遅れても偶発的に失敗しないようにするため。
    const elapsed = await sendBurst(page, COMMAND);
    assert.ok(elapsed >= 900, `burst took ${elapsed}ms`);
    const done = await waitUntil(page, 'all ticks', ticksDone);
    const lastProgress = Math.max(...done.log.filter((r) => r.kind === 'progress').map((r) => r.event));
    const received = done.log.filter((r) => r.kind === 'input_received');
    assert.equal(received.length, BURST);
    assert.ok(received.every((r) => r.event < lastProgress), '連打が処理中の窓の外に出ている');
    assert.equal(kindCount(done, 'accepted'), 1);
    assert.equal(categoryCount(done, 'busy'), BURST - 1);
    assert.equal(done.judgedCategories.length, BURST - 1, 'busy 以外の拒否が混ざっている');
    assert.deepEqual(done.artifacts.map((a) => a.path), ['notes/draft']);
    assert.equal(done.current, 'main/st-1');
    assert.equal(kindCount(done, 'achieved'), 0);
    assert.deepEqual(done.ticks.map(([g, i]) => [g, i]), [[1, 0], [1, 1], [1, 2], [1, 3], [1, 4]]);
    assertConsistent(done);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('B-2b 選択の連打は、受付が 1 回で、状態が壊れない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    await sendRaw(page, { control: { go_to: 'main/st-2' } });
    const moved = await engine(page);
    assert.equal(moved.current, 'main/st-2');
    assert.equal(moved.generation, 2);
    await sendBurst(page, CHOICE);
    const e = await engine(page);
    assert.equal(kindCount(e, 'accepted'), 1);
    assert.equal(kindCount(e, 'accepted', 'main/st-2'), 1);
    assert.equal(kindCount(e, 'achieved', 'main/st-2'), 1);
    assert.equal(e.current, 'main/st-3');
    assert.equal(e.generation, 2);
    // ボタンが DOM に無いため、残りの 19 回は遷移後の st-3 に届き、規則で拒否される。
    assert.equal(kindCount(e, 'input_received'), BURST);
    assert.equal(categoryCount(e, 'not_in_this_step'), BURST - 1);
    assert.equal(e.judgedCategories.length, BURST - 1);
    assertConsistent(e);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('B-2c 残り続けるボタンへの連打は、すべて規則で拒否され、状態は変わらない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    await sendRaw(page, { control: { go_to: 'main/st-3' } });
    const before = await engine(page);
    assert.equal(before.current, 'main/st-3');
    assert.equal(before.generation, 2);
    const liBefore = await page.locator('[data-testid="part-log"] li').count();
    await sendBurst(page, REJECT);
    const e = await engine(page);
    assert.equal(categoryCount(e, 'not_in_this_step') - categoryCount(before, 'not_in_this_step'), BURST);
    assert.equal(e.judgedCategories.length - before.judgedCategories.length, BURST);
    assert.equal(e.logLen - before.logLen, BURST * 2, '1 回につき入力と判定の 2 件');
    assert.equal(e.generation, 2);
    assert.equal(e.current, 'main/st-3');
    assert.equal(kindCount(e, 'accepted') - kindCount(before, 'accepted'), 0);
    assert.equal(kindCount(e, 'achieved') - kindCount(before, 'achieved'), 0);
    // DOM の行数は、書き出しの増分と一致する。
    // 行になるのは入力の記録だけ。拒否の行は TASK-437 の結線まで出ない（TASK-434 1/2）。
    assert.equal((await page.locator('[data-testid="part-log"] li').count()) - liBefore, BURST);
    assertConsistent(e);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('配信設定には試験の口も、入力を届ける隠し要素も無い', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    assert.equal(await page.locator(RAW).count(), 0);
    await context.close();
  });
});
