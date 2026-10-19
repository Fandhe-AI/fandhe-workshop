// 1 回の操作で起きる内部の遷移の上限の縦断試験（TASK-160 1/3。SAFE-23）。
//
// 作成者向けの検査を通さずに配られた巡回する IR を画面に読み込むと、試作のエンジンは再帰が止まらずスタックが溢れ、
// 受講者のタブが落ちた（PoC-7 手順 7）。ここでは、巡回する IR 3 本が 1,000 回に達した時点で止まり、警告が 1 件だけ残り、
// 止まった後の入力でもタブが落ちず、ページ内のエラーが 0 件であることを確かめる。
//
// 試験の設定（TEST_HOOKS=1）の画面で、見えない `raw-trigger` へ固定の接頭辞 `load-fixture:<名前>` を書いて click し、
// 試験用の教材へ差し替える（配信物には入らない）。続けて `Op` の JSON を同じ経路で送る。開始（`Control::Begin`）は
// エンジンが未実装（TASK-217）なので、`enter` の巡回は `go_to` で巡回の入口へ入って代わりにする。
//   cycle-enter-self・cycle-enter-2 : `go_to` で巡回の入口へ。`module_entered` が 1,000 件で止まる
//   cycle-exit-2                    : 自然文 1 回で達成 → `exit_to` の巡回。`module_exited` が 1,000 件で止まる
//   cycle-d07                       : 巡回していても 1 入力あたりの遷移が少数。警告 0 件の対照
//   chain-1000 / chain-1001         : 巡回しない `use` の長い連鎖。着地が 1,000 回なら通り、1,001 回で止まる
// 警告は、エンジンの書き出しの `warnings`（固定文）から、上限で止めた警告だけを数える。巡回する IR や `go_to` で飛んだ
// 教材では、効果の適用の警告が別に出ることがあるため、ログの種類 `warning` の件数では数えない。
// 所要時間は assert せず diagnostic に残す。必須チェックなので、時間で結果を変えない。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-transition-limit/';
const RAW = '[data-testid="raw-trigger"]';
const LIMIT = 1000;

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

// 文字列としてだけ渡し、コードの文字列を組み立てない。
async function sendRaw(page, payload) {
  await page.evaluate(([sel, p]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', p);
    el.click();
  }, [RAW, payload]);
}

const engineOf = (page) => page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine);
const kinds = (e, kind) => e.log.filter((r) => r.kind === kind).length;
// 遷移の上限で止めた警告だけを数える（効果の適用などの他の警告は数えない）。
const limitWarnings = (e) => e.warnings.filter((w) => w.message.includes('per-input limit')).length;

// 教材を差し替え、`current` が開始の Step になるのを待つ。
async function load(page, name, start) {
  await sendRaw(page, `load-fixture:${name}`);
  await page.waitForFunction((s) => JSON.parse(window.__fandheTest.stateJson()).engine.current === s, start);
}

// 操作を送り、ログが伸びるまで待つ（同期の処理だが、描画の更新を挟む）。
async function act(page, op) {
  const before = (await engineOf(page)).logLen;
  await sendRaw(page, JSON.stringify(op));
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before);
  return engineOf(page);
}

// 止まった後の入力でタブが落ちず、ログも伸びない（現在の Step が無いので無視される）。
async function assertIgnoredAfterHalt(page) {
  const e = await engineOf(page);
  assert.equal(e.current, null, '止めたら現在の Step が空');
  await sendRaw(page, JSON.stringify({ input: { text: 'go' } }));
  await page.waitForTimeout(100);
  const after = await engineOf(page);
  assert.equal(after.logLen, e.logLen, '止めた後の入力はログを増やさない');
  assert.equal(after.current, null);
}

async function run(name, body) {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const t0 = Date.now();
    await body(page);
    const ms = Date.now() - t0;
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
    assert.equal(rec.crashed, false, 'タブが落ちていない');
    assert.deepEqual(rec.pageErrors, [], 'ページ内のエラーが 0 件');
    await context.close();
    return ms;
  });
}

for (const [name, start, op, kind] of [
  ['cycle-enter-self', 'main/st-1', { control: { go_to: 'main/u-self' } }, 'module_entered'],
  ['cycle-enter-2', 'main/st-1', { control: { go_to: 'main/u-a' } }, 'module_entered'],
  ['cycle-exit-2', 'main/f1', { input: { text: 'go' } }, 'module_exited'],
]) {
  test(`${name}: 1,000 回で止まり警告が 1 件だけ残る`, async (t) => {
    await run(name, async (page) => {
      await load(page, name, start);
      const t0 = Date.now();
      const e = await act(page, op);
      t.diagnostic(`${name} elapsed_ms=${Date.now() - t0} logLen=${e.logLen}`);
      assert.equal(limitWarnings(e), 1, JSON.stringify(e.warnings));
      assert.equal(kinds(e, kind), LIMIT);
      await assertIgnoredAfterHalt(page);
    });
  });
}

test('cycle-d07: 1 入力あたりの遷移が少数なら止まらない', async (t) => {
  await run('cycle-d07', async (page) => {
    const start = 'impl-run@round1/merge-gate@gate/st-7-4';
    await load(page, 'cycle-d07', start);
    let e = await act(page, { input: { ui: { view: 'v-pr-cards', path: 'prs/pr-1-a#merge' } } });
    e = await act(page, { input: { ui: { view: 'v-issue-tree', path: 'issues/child-1' } } });
    t.diagnostic(`cycle-d07 logLen=${e.logLen} current=${e.current} judged=${JSON.stringify(e.judgedCategories)}`);
    assert.ok(e.current, '現在の Step が残っている');
    assert.equal(limitWarnings(e), 0, JSON.stringify(e.warnings));
    assert.ok(kinds(e, 'module_entered') >= 1 && kinds(e, 'module_entered') < 10, JSON.stringify(e.log.map((r) => r.kind)));
  });
});

for (const [name, halts] of [['chain-1000', false], ['chain-1001', true]]) {
  test(`${name}: 着地 ${halts ? '1,001' : '1,000'} 回の連鎖は${halts ? '止まる' : '通る'}`, async (t) => {
    await run(name, async (page) => {
      await load(page, name, 'main/st-1');
      await act(page, { control: { go_to: 'main/st-2' } });
      const t0 = Date.now();
      const e = await act(page, { input: { choice: 'opt-memo' } });
      t.diagnostic(`${name} elapsed_ms=${Date.now() - t0} logLen=${e.logLen}`);
      assert.equal(limitWarnings(e), halts ? 1 : 0, JSON.stringify(e.warnings));
      if (halts) {
        assert.equal(e.current, null);
        await assertIgnoredAfterHalt(page);
      } else {
        assert.equal(e.current, 'main/st-3', '最後の Step に入る');
      }
    });
  });
}
