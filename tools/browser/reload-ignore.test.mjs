// 再読み込みの後に届く古いタイマーの縦断試験（TASK-221 2/2。EVT-6・EVT-8・EVT-12。SAVE-4 の類比）。
//
// 試験の設定（TEST_HOOKS=1）の画面で、実タイマーの進行（表示側の 300ms）を流しながら、再読み込みの前に予約された
// 古い宛先のタイマーが、再読み込みの後に届いても状態を変えず、無視が 1 件だけ増えることを確かめる。
//   R-1 実タイマー: 進行 2 段の瞬間に再読み込みし、予約済みの古いタイマーが後から届く。世代 2 の宛先で完走する
//   R-2 控えた古い宛先: 再読み込みの直後に、再読み込み前の宛先を同じ同期区間で送る（S-5 の類比）
//   R-3 再読み込みの後の停止: 入口の状態へ戻り、古いタイマーは状態を変えない（S-6 の類比）
//
// 注意: これは実際の `page.reload()` ではなく、ページ内の模擬の再読み込みである。web には保存した状態の復元の結線が
// まだ無く（TASK-319。#340〜#343）、`page.reload()` では教材の開始状態から作り直されるだけで、再読み込みの前の
// 世代を引き継げない。そこで見えない `raw-trigger` に固定の合図（`{"reload":{}}`）を書いて click し、メモリ上の
// `Session` の世代だけを上げる（試験の設定だけ。stop-move-ignore の停止の注入と同じ方式）。再読み込みの前に
// 予約された実際のタイマーは取り消されずに残り、再読み込みの後に届く。この場面が確かめたい対象である。
// 実際の `page.reload()` を使う場面（browser-scenes.md の S-5・S-6 ほか）は TASK-319 3/3（#343）の後に行う。
// 進行の間隔は 300ms である（EVT-18）。待つ条件は時間ではなく件数で決める。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-reload-ignore/';
const RAW = '[data-testid="raw-trigger"]';
const COMMAND = { input: { command: '/make-note first-note' } };
// ホストの固定の合図（crates/web/src/host.rs の RAW_RELOAD と同じ文字列）。
const RELOAD_PAYLOAD = '{"reload":{}}';
const STOP = { control: 'stop' };
const TOTAL_TICKS = 5;
const ST1 = 'main/st-1';

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

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const nextTick = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.nextTick()));

// 進行がちょうど n 段になった瞬間に、同じ同期区間で ops を順に click し、click ごとの後の書き出し（afters）を返す。
// ops の要素は `{ op }`（その Op を送る）、`{ reload: true }`（再読み込みの合図を送る）、
// `{ useNextTick: true }`（click の前に控えた次の進行の宛先を `Op::Tick` として送る）。
// n 段を飛び越えた場合・上限時間を超えた場合は、ページの中でエラーにして失敗にする。
async function actWhenTicks(page, n, ops, timeout = 8000) {
  const res = await page.evaluate(([sel, count, list, limit, reloadPayload]) => new Promise((resolve) => {
    const el = document.querySelector(sel);
    const snap = () => JSON.parse(window.__fandheTest.stateJson()).engine;
    const t0 = performance.now();
    const poll = () => {
      const e = snap();
      const len = e.ticks.length;
      if (len === count) {
        const addr = JSON.parse(window.__fandheTest.nextTick());
        const afters = [];
        for (const item of list) {
          let payload;
          if (item.reload) payload = reloadPayload;
          else payload = JSON.stringify(item.useNextTick ? { tick: addr } : item.op);
          el.setAttribute('data-payload', payload);
          el.click();
          afters.push(snap());
        }
        resolve({ before: e, addr, afters });
        return;
      }
      if (len > count) {
        resolve({ error: `ticks skipped past ${count}: ${JSON.stringify(e)}` });
        return;
      }
      if (performance.now() - t0 > limit) {
        resolve({ error: `timeout waiting for ${count} ticks: ${JSON.stringify(e)}` });
        return;
      }
      setTimeout(poll, 2);
    };
    poll();
  }), [RAW, n, ops, timeout, RELOAD_PAYLOAD]);
  assert.ok(!res.error, res.error);
  return res;
}

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

const kindCount = (e, kind) => e.log.filter((r) => r.kind === kind).length;
const ticksOf = (e, g) => e.ticks.filter(([gen]) => gen === g).map(([, i]) => i);
const ignoredCount = (e) => e.ignoredReasons.length;
const stateOf = (e) => e.stateWithoutLog;
// 世代は再読み込みで上がる。世代以外の一致を見るため、実行状態の世代だけを外す。
const withoutGeneration = (e) => {
  const s = structuredClone(stateOf(e));
  delete s.run.generation;
  return s;
};
const paths = (e) => e.artifacts.map((a) => a.path);

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

test('R-1 再読み込みの前に予約された実タイマーは、後から届いても無視 1 件で、世代 2 の宛先で完走する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    await sendRaw(page, COMMAND);
    const { before, afters: [reloaded] } = await actWhenTicks(page, 2, [{ reload: true }]);
    assert.equal(reloaded.generation, 2);
    assert.equal(reloaded.current, ST1);
    // `Reloaded` は行にしないイベントのため、log の行ではなく logLen の増分（1 件）で数える。
    assert.equal(reloaded.logLen - before.logLen, 1);
    assert.deepEqual(withoutGeneration(reloaded), withoutGeneration(before), '再読み込みで世代以外が変わった');
    assert.deepEqual(await nextTick(page), { generation: 2, step: ST1, index: 2 });
    assert.equal(ignoredCount(reloaded), 0);

    // 件数で待つ: 古いタイマーの無視がちょうど 1 件になり、世代 2 の進行が 2・3・4 と揃う。
    const done = await waitUntil(
      page,
      'stale tick ignored once and generation 2 completes',
      (e) => ticksOf(e, 2).length >= TOTAL_TICKS - 2 && ignoredCount(e) >= 1 && stateOf(e).run.busy === null,
    );
    assert.equal(ignoredCount(done), 1);
    const reason = done.ignoredReasons[0];
    assert.equal(reason.reason, 'stale');
    assert.equal(reason.generation, 2);
    assert.deepEqual(ticksOf(done, 1), [0, 1]);
    assert.deepEqual(ticksOf(done, 2), [2, 3, 4]);
    assert.deepEqual(paths(done), ['notes/draft']);
    assert.equal(await nextTick(page), null);

    // 完走の後に、古いタイマーの二重の届きが無い（積み増されていない）。
    await page.waitForTimeout(700);
    const later = await engine(page);
    assert.equal(ignoredCount(later), 1);
    assert.deepEqual(stateOf(later), stateOf(done));
    assertConsistent(later);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('R-2 再読み込みの直後に再読み込み前の宛先を送ると、無視 1 件で状態の JSON は一致する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    await sendRaw(page, COMMAND);
    const { addr, afters: [a, b] } = await actWhenTicks(page, 2, [{ reload: true }, { useNextTick: true }]);
    assert.deepEqual(addr, { generation: 1, step: ST1, index: 2 });
    assert.equal(a.generation, 2);
    assert.deepEqual(stateOf(b), stateOf(a));
    assert.equal(b.logLen - a.logLen, 1);
    assert.equal(b.log.at(-1).kind, 'ignored');
    assert.equal(ignoredCount(b) - ignoredCount(a), 1);
    const last = b.ignoredReasons.at(-1);
    assert.deepEqual(
      { generation: last.generation, step: last.step, reason: last.reason },
      { generation: 2, step: ST1, reason: 'stale' },
    );
    assert.deepEqual(b.ticks, a.ticks);

    // 自然に届く予約済みのタイマーも無視され（合計 2 件）、世代 2 の宛先で完走する。
    const done = await waitUntil(
      page,
      'generation 2 completes',
      (e) => ticksOf(e, 2).length >= TOTAL_TICKS - 2 && stateOf(e).run.busy === null,
    );
    assert.equal(ignoredCount(done), 2);
    assert.deepEqual(ticksOf(done, 1), [0, 1]);
    assert.deepEqual(ticksOf(done, 2), [2, 3, 4]);
    assert.deepEqual(paths(done), ['notes/draft']);
    assertConsistent(done);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('R-3 再読み込みの後の停止で入口の状態へ戻り、古いタイマーは状態を変えず、再び入力できる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTestPage(srv);
    const initial = await engine(page);
    await sendRaw(page, COMMAND);
    const { before, afters: [reloaded, stopped] } = await actWhenTicks(page, 2, [{ reload: true }, { op: STOP }]);
    assert.equal(reloaded.logLen - before.logLen, 1);
    assert.equal(stopped.generation, 3);
    assert.equal(stopped.current, ST1);
    assert.equal(stateOf(stopped).run.busy, null);
    assert.equal(kindCount(stopped, 'stopped'), 1);
    assert.deepEqual(stateOf(stopped).run.world, stateOf(initial).run.world, '入口の状態へ戻っていない');
    assert.deepEqual(paths(stopped), []);
    assert.equal(await nextTick(page), null);

    // 予約済みのタイマーは 2 本ある（再読み込みの前の世代 1 と、再読み込みで予約された世代 2）。取り消されずに
    // 後から届いても、どちらも無視 1 件ずつ（合計 2 件）で、状態は変わらない。
    const later = await waitUntil(page, 'both stale ticks ignored', (e) => ignoredCount(e) >= 2);
    await page.waitForTimeout(700);
    const settled = await engine(page);
    assert.equal(ignoredCount(settled), 2);
    assert.deepEqual(settled.ignoredReasons.map((r) => r.reason), ['stale', 'stale']);
    assert.deepEqual(stateOf(settled), stateOf(stopped));
    assert.deepEqual(stateOf(later), stateOf(stopped));
    assert.deepEqual(ticksOf(later, 1), [0, 1]);

    // 停止の後でも再び入力できる。
    await sendRaw(page, COMMAND);
    assert.deepEqual(await nextTick(page), { generation: 3, step: ST1, index: 0 });
    const done = await waitUntil(page, 'all ticks of generation 3', (e) => ticksOf(e, 3).length >= TOTAL_TICKS);
    assert.deepEqual(ticksOf(done, 3), [0, 1, 2, 3, 4]);
    assert.deepEqual(paths(done), ['notes/draft']);
    assertConsistent(done);
    await assertSafe(page, rec);
    await context.close();
  });
});

test('配信設定には試験の口も、再読み込みを届ける隠し要素も無い', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    assert.equal(await page.locator(RAW).count(), 0);
    await context.close();
  });
});
