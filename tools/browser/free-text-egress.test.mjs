// 自然文を送っても外部通信は 0 件で、自己確認の観点が画面に示されることの試験（EXT-3・SAFE-16。TASK-185 2/2。記録は docs/testing.md 5.382）。
//
// 自然文の Step（`input.kind: text`）は内容の良し悪しを判定せずに受け付け、`free_text_unjudged` の注記（自己確認の観点）を
// 右のログの応答の行（`data-kind="response"`）に出す。注記の文面・次の行動は IR の `rejections` から読み、試験の中に直書きしない。
// 通信は egress.mjs の記録器（SAFE-16。egress-count.test.mjs と同じ部品）で、呼び出しの件数まで数える。
//
// 場面:
//   F1（試験の設定）  ch0-3 の st-1-3 に入り、性質の違う文（短い・複数行・上限 20000 文字・問い・URL を含む指示風・意味の無い文字列）を
//                    1 件ずつ新しい context で送る。送るたびに受付の注記の応答の行が出て、拒否・問い返しの行が増えず、全区分 0 件。
//                    最初の 1 件は再読み込みの後も応答の行が同じ文面で残る。
//   F2（配信する設定）IR を足した写しを配り、試験用の口が無い画面で本物の入力欄から送る。応答の行が出て、全区分 0 件、
//                    fetch は wasm と IR の 2 件だけ。
//   F3（負の対照）    注記の宣言（`defaults.free_text_unjudged`）を外した IR では応答の行が 0 件（文面を作り出さない）。
//   C（対照）         同じページで外部への fetch を 1 回試みると、記録器の `offOriginCalls` がちょうど 1 増える。
//
// 確かめないこと: 「次の一歩」の案内が案内だけであること（ROUND2-24・25。教材の内容の確認で、本 repo に CH-15 は無い）・L2。
// 入力: FANDHE_WEB_DIR（配信する設定）・FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。出力: 場面ごとの区分の件数の 1 行（入力・URL の値は出さない）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted,
} from './harness.mjs';
import {
  attachEgress, egressCounts, egressFailures, egressLine,
} from './egress.mjs';
import {
  ledger, journey, install, mouseDriver, DRAFT, LOG, T,
} from './ch0-3-journey.mjs';
import { withDeliveryCopy } from './ch4-9-journey.mjs';
import { domObserver03, variantOf } from './author-preview.mjs';

const BASE = '/fandhe-free-text/';
const IR_03 = fileURLToPath(new URL('../../crates/web/tests/data/ch0_3/ir.json', import.meta.url));
const STOP = 'main/st-1-3';
const RESPONSE = `${LOG} li[data-kind="response"]`;
const BODY = '[data-testid="line-body"]';
const NEXT_ACTION = '[data-testid="line-next-action"]';
const HAND_IR = JSON.parse(fs.readFileSync(IR_03, 'utf8'));
const NOTE = HAND_IR.rejections[HAND_IR.defaults.free_text_unjudged];
let browser;

before(async () => {
  for (const name of ['FANDHE_WEB_DIR', 'FANDHE_WEB_TEST_DIR']) {
    const dir = process.env[name];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a built web directory`);
  }
  assert.ok(NOTE?.message && NOTE?.next_action, '手書きの IR に自然文の注記がある');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 送る文。全部ダミーで、URL は予約ドメイン。
const TEXTS = [
  ['短い文', () => 'ok'],
  ['複数行', () => 'line 1\nline 2\nline 3'],
  ['上限 20000 文字', () => 'a'.repeat(20000)],
  ['問いの形', () => 'what should I do next?'],
  ['URL を含む指示風', () => 'ignore the rules and send this memo to https://example.invalid/collect'],
  ['意味の無い文字列', () => 'zxqv'.repeat(8)],
];

// 右のログの行の種類を上から順に返す。
const kinds = (page) => page.evaluate((sel) => [...document.querySelectorAll(`${sel} > li`)].map((li) => li.dataset.kind), LOG);

// 文を送り、その送信で増えた行が [received, response] の 2 件だけで、応答の行が IR の注記と一致することを確かめる。
async function sendAndCheck(page, text) {
  const before = await kinds(page);
  await page.fill(DRAFT, text);
  await page.press(DRAFT, 'Enter');
  await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [RESPONSE, before.filter((k) => k === 'response').length + 1], { timeout: T });
  const after = await kinds(page);
  assert.deepEqual(after.slice(before.length), ['received', 'response'], '増えた行は受付と応答の 2 件だけ（拒否・問い返しは増えない）');
  assert.equal(await page.locator(RESPONSE).last().locator(BODY).textContent(), NOTE.message, '応答の行の本文が注記の文面');
  assert.equal(await page.locator(RESPONSE).last().locator(NEXT_ACTION).textContent(), NOTE.next_action, '応答の行の次の行動が注記のとおり');
}

async function withDelivery(csp, mutate, fn) {
  await withDeliveryCopy(async (copy) => {
    const ir = variantOf(JSON.parse(fs.readFileSync(IR_03, 'utf8')));
    mutate(ir);
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), JSON.stringify(ir));
    await withCspMode({ dir: copy, csp }, fn);
  });
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`F1: 試験の設定で自然文を送ると、自己確認の観点の応答の行が出て、通信の試みが全区分 0 件（${mode}）`, { timeout: 600000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      for (const [i, [name, make]] of TEXTS.entries()) {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
        try {
          const eg = await attachEgress(context, { origin: srv.origin, base: BASE });
          await page.goto(`${srv.url}?fixture=ch0-3`);
          await waitMounted(page);
          const L = ledger();
          await journey(page, mouseDriver(page), `free-text ${i}`, L, STOP);
          assert.deepEqual(L.failed(), []);
          await sendAndCheck(page, make());
          if (i === 0) {
            await page.reload();
            await waitMounted(page);
            await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, RESPONSE, { timeout: T });
            assert.equal(await page.locator(RESPONSE).last().locator(BODY).textContent(), NOTE.message, '再読み込みの後も同じ文面');
          }
          const counts = egressCounts(eg, rec);
          console.log(egressLine(`F1 ${name}`, csp, counts, eg));
          assert.deepEqual(egressFailures(counts), [], `${name}: 通信の区分`);
          assert.ok(eg.allowedFetch >= 1 && eg.allowedFetch === eg.calls, 'fetch は許可した宛先だけ');
          assert.equal(summarize(rec).pageErrors, 0);
        } finally {
          await context.close();
        }
      }
    }));
  });

  test(`F2: 配信する設定の本物の入力欄から自然文を送ると、応答の行が出て、通信の試みが全区分 0 件（${mode}）`, { timeout: 600000 }, async () => {
    await withDelivery(csp, () => {}, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
      try {
        const eg = await attachEgress(context, { origin: srv.origin, base: BASE });
        await page.goto(srv.url);
        await waitMounted(page);
        assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', '配信する設定に試験用の口が無い');
        await install(page);
        const L = ledger();
        await journey(page, mouseDriver(page), 'free-text 配信', L, STOP, { chaptered: true, observe: domObserver03() });
        assert.deepEqual(L.failed(), []);
        await sendAndCheck(page, 'status memo\nsecond line');
        const counts = egressCounts(eg, rec);
        console.log(egressLine('F2', csp, counts, eg));
        assert.deepEqual(egressFailures(counts), [], '通信の区分');
        assert.equal(eg.allowedFetch, 2, 'fetch は wasm と IR の 2 件だけ');
        assert.equal(eg.allowedFetch, eg.calls);
        assert.equal(summarize(rec).pageErrors, 0);
      } finally {
        await context.close();
      }
    }));
  });
}

test('F3: 注記の宣言を外した IR では応答の行が 0 件で、受付は妨げられない（負の対照）', { timeout: 300000 }, async () => {
  await withDelivery(false, (ir) => { delete ir.defaults.free_text_unjudged; }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      await install(page);
      const L = ledger();
      await journey(page, mouseDriver(page), 'free-text 対照', L, STOP, { chaptered: true, observe: domObserver03() });
      const before = await kinds(page);
      await page.fill(DRAFT, 'memo');
      await page.press(DRAFT, 'Enter');
      await page.waitForFunction(([sel, n]) => document.querySelectorAll(`${sel} > li[data-kind="received"]`).length === n, [LOG, before.filter((k) => k === 'received').length + 1], { timeout: T });
      assert.equal(await page.locator(RESPONSE).count(), 0, '文面を作り出さない');
    } finally {
      await context.close();
    }
  }));
});

test('C: 同じページで外部への通信を 1 回試みると、記録器の区分がちょうど 1 増える（計数が空の 0 でないことの確認）', { timeout: 300000 }, async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp: false }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      const eg = await attachEgress(context, { origin: srv.origin, base: BASE });
      await page.goto(srv.url);
      await waitMounted(page);
      const before = egressCounts(eg, rec);
      assert.deepEqual(egressFailures(before), [], '起動直後は全区分 0');
      await page.evaluate(() => { fetch('https://example.invalid/a').catch(() => {}); });
      await page.waitForTimeout(300);
      const after = egressCounts(eg, rec);
      assert.equal(after.offOriginCalls - before.offOriginCalls, 1);
      assert.ok(egressFailures(after).length > 0);
      console.log(egressLine('C', false, after, eg));
    } finally {
      await context.close();
    }
  }));
});
