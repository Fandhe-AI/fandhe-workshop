// L2 の完走試験が使う DOM だけの観察（l2-journey.mjs）の較正の試験（TASK-112 2/2。DLV-13）。
//
// 目的: 配信する設定には `__fandheTest` が無く、L2 の完走試験は DOM の目印だけで「いまどの Step か」を判断する。
// 目印が実際の状態とずれると、完走試験は空振りで通るか誤って落ちる。そこで試験の設定（TEST_HOOKS=1）の画面を
// `?fixture=ch0-3` で開き、同じ誘導（CH-0〜CH-1。`main/st-2-1` に入るまで）を DOM の観察で進めながら、各 Step の観察ごとに
// `stateJson` の現在の Step・状態と一致することを確かめる。わざとずらした期待（1 Step 先）は DOM が示さないことも確かめる対照を持つ。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。入力した値は出力しない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { launchBrowser, channelFromEnv, withServer, openPage, waitMounted } from './harness.mjs';
import { engine, ledger } from './ch0-3-journey.mjs';
import { domObserver, inspectDom, runL2JourneyWith, stepOrder } from './l2-journey.mjs';

const IR = new URL('../../crates/web/tests/data/ch0_3/ir.json', import.meta.url);
const ORDER = stepOrder(fs.readFileSync(IR, 'utf8'));
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

async function open(srv) {
  const { page, context } = await openPage(browser, { origin: srv.origin });
  await page.goto(`${srv.url}?fixture=ch0-3`);
  await waitMounted(page);
  assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
  return { page, context };
}

test('DOM の観察は各 Step で stateJson の現在の Step・状態と一致する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: '/fandhe-l2-calibrate/' }, async (srv) => {
    const { page, context } = await open(srv);
    const L = ledger();
    const dom = domObserver(ORDER);
    let observed = 0;
    // DOM の観察で待ち、待ち終えた時点の stateJson と突き合わせる観察。
    const observe = {
      async at(pg, uid, status) {
        await dom.at(pg, uid, status);
        const e = await engine(pg);
        const st = e.stateWithoutLog.run.slices.main?.status?.[uid] ?? 'entered';
        observed += 1;
        L.check('較正', `${uid} ${status} が stateJson と一致`, e.current === uid && st === status);
      },
      async stillAt(pg, uid) {
        const dv = await dom.stillAt(pg, uid);
        const e = await engine(pg);
        L.check('較正', `${uid} の拒否後の観察が stateJson と一致`, dv === (e.current === uid));
        return dv;
      },
    };
    await runL2JourneyWith(page, L, { order: ORDER, observe });
    assert.ok(observed >= 14, `観察の回数が少なすぎる: ${observed}`);
    console.log(`DOM の観察の較正: ${L.summary()} 項目（観察 ${observed} 回）`);
    assert.deepEqual(L.failed(), []);
    await context.close();
  });
});

test('対照: 1 Step 先の期待は DOM が示さず、正しい期待だけを示す', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: '/fandhe-l2-calibrate-ctl/' }, async (srv) => {
    const { page, context } = await open(srv);
    const probe = async (uid, status) => (await page.evaluate(inspectDom, [ORDER, uid, status])).ok;
    assert.equal(await probe('main/st-0-1', 'entered'), true);
    assert.equal(await probe('main/st-0-2a', 'entered'), false);
    assert.equal(await probe('main/st-0-1', 'accepted'), false);
    await context.close();
  });
});
