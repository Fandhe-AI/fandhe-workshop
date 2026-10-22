// 「次の章へ進む」で CH-0 から CH-3 まで章をまたいで通す試験（TASK-526 2/2。SHELL-10・INTRO-3）。
//
// 試験の設定（TEST_HOOKS=1）の画面を `?fixture=ch0-3-chapters`（`src/test_fixture.rs` の `ch03_chapters_ir_bytes`）で開く。
// 20 Step の通しの教材の各章の最後（st-0-2c・st-1-6・st-2-8）を `final` にし、開始点を章の先頭 4 件にした変種で、
// 章の完了が起きるので各章の後に「次の章へ進む」が出る。誘導は `ch0-3-journey.mjs` の `journey(..., { chaptered: true })`。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい画面を開く）:
//   - 通し（実クリック）: CH-0 → CH-1 → CH-2 → CH-3 を、各章の完了後に出る 1 個のボタンだけで越えて最後の達成まで進む。
//     各章の完了後にちょうど 1 個、「移動」ボタンは 0 件、押すと次の章の先頭へ入り世代が進む。最後の章の後と各章の途中では 0 件
//   - キーボードだけ: CH-0 から CH-1 の途中（`main/st-1-4`）まで `page.keyboard` だけで境目を越える。信頼されたマウスのイベントは 0 件
//   - 再読み込み: 章を越えて `main/st-1-4` まで進んだ後に再読み込みしても、同じ Step に戻りボタンは 0 件（移動は ops から再現する）
//   - 安全の計数・配信元の外への通信・エンジンの警告と無視が 0 件
//
// 確かめないこと:
//   - `Control::Begin` による移動と開始点の effects の適用（core 未実装。TASK-217。今は `Control::GoTo`。`docs/testing.md` 5.227）
//   - 2 周目の開始（TASK-254）・範囲外の章の画面での確認（TASK-527）
//   - 試験の出力に、入力した値や下書きの中身は出さない（名前・件数・合否だけ）
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  NEXT, engine, ledger, mouseDriver, keyboardDriver, journey, install,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-next-chapter/';
const QUERY = '?fixture=ch0-3-chapters';
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

async function finish(page, rec, scene, L, { expectMouse }) {
  const run = await page.evaluate(() => window.__run);
  L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
  L.check(scene, expectMouse ? '実マウスの操作が通っている' : '信頼されたマウスのイベントが 0 件', expectMouse ? run.mouse > 0 : run.mouse === 0);
  const e = await engine(page);
  L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
  L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
  L.check(scene, '省略した Step が 0 件', e.review.filter((r) => r.skipped).length === 0);
  const counts = await collectSafety(page, rec);
  for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
  L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
  L.check(scene, '配信元の外への通信が 0 件', rec.external.length === 0);
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

  test(`次の章へ進むだけで CH-0 から CH-3 まで通す（${mode}）`, { timeout: 300000 }, async () => {
    await scenario(csp, 'through', async (page, rec, L) => {
      const scene = '通し';
      let midNonZero = 0;
      // 各 Step に入った・確認待ちになった直後にボタンが 0 件であること（章の最後の達成は onStep の対象外）。
      const onStep = async () => {
        if ((await page.locator(NEXT).count()) !== 0) midNonZero += 1;
      };
      await journey(page, mouseDriver(page), scene, L, null, { chaptered: true, onStep });
      L.check(scene, '各章の途中の Step でボタンが 0 件', midNonZero === 0);
      const e = await finish(page, rec, scene, L, { expectMouse: true });
      const status = e.stateWithoutLog.run.slices.main.status;
      L.check(scene, '最後の Step が達成', e.current === 'main/st-3-2' && status['main/st-3-2'] === 'achieved');
      L.check(scene, '20 Step がすべて達成', Object.keys(status).length === 20 && Object.values(status).every((s) => s === 'achieved'));
      L.check(scene, '最後の章の後にボタンが 0 件', (await page.locator(NEXT).count()) === 0);
    });
  });

  test(`CH-0 から CH-1 の途中までをキーボードだけで通す（${mode}）`, { timeout: 300000 }, async () => {
    await scenario(csp, 'keyboard', async (page, rec, L) => {
      const scene = 'キーボード';
      await journey(page, keyboardDriver(page), scene, L, 'main/st-1-4', { chaptered: true });
      const e = await finish(page, rec, scene, L, { expectMouse: false });
      const status = e.stateWithoutLog.run.slices.main.status;
      L.check(scene, 'main/st-1-4 に入った', e.current === 'main/st-1-4');
      L.check(scene, 'CH-0 の 4 Step が達成', ['0-1', '0-2a', '0-2b', '0-2c'].every((n) => status[`main/st-${n}`] === 'achieved'));
    });
  });

  test(`章を越えた後に再読み込みしても同じ Step に戻る（${mode}）`, { timeout: 300000 }, async () => {
    await scenario(csp, 'reload', async (page, rec, L) => {
      const scene = '再読み込み';
      await journey(page, mouseDriver(page), scene, L, 'main/st-1-4', { chaptered: true });
      await page.reload();
      await waitMounted(page);
      const e = await engine(page);
      L.check(scene, '同じ Step に戻る', e.current === 'main/st-1-4');
      L.check(scene, 'CH-0 の達成が残る', e.stateWithoutLog.run.slices.main?.status?.['main/st-0-2c'] === 'achieved');
      L.check(scene, 'ボタンが 0 件', (await page.locator(NEXT).count()) === 0);
    });
  });
}
