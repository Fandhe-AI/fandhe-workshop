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
import {
  RAW, engine, ports, draftValue, ledger, mouseDriver, keyboardDriver, journey, install,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-ch0-3-run/';
const QUERY = '?fixture=ch0-3';
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

// 計数: 実キーと実マウスの別を分けるため、キーボードの区間は mouse が 0、通しは mouse が 1 以上であることも確かめる。
async function finish(page, rec, scene, L, { expectMouse }) {
  const run = await page.evaluate(() => window.__run);
  L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
  L.check(scene, expectMouse ? '実マウスの操作が通っている' : '信頼されたマウスのイベントが 0 件', expectMouse ? run.mouse > 0 : run.mouse === 0);
  const e = await engine(page);
  L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
  L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
  // 投影の行（e.log）には Event::Skipped が出ない。省略は Step ごとの見直し項目（e.review）の skipped に現れる。
  L.check(scene, '省略した Step が 0 件', e.review.filter((r) => r.skipped).length === 0);
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
