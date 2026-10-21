// 画面を開いて操作したときの通信・CSP の違反・コンソールの CSP 警告・ページ内エラーの計数（SAFE-13。TASK-151 2/2）。
//
// 場面ごとに、CSP あり → なしの順で同じ操作を流し、各回で次を確かめる。
//   - 配信元以外への通信 0・CSP の違反（securitypolicyviolation）0・コンソールの CSP 関連の出力 0・ページ内エラー 0・
//     失敗した要求 0（favicon を除く）・安全の計数（scanDom を含む）がすべて 0。Step ごとに走査する場面では各 Step で
//   - 場面の終わりに、CSP ありとなしで配信元への要求の種類（相対パス・種類・状態の組。favicon を除く）が同じであること。
//     CSP が黙って何かを止めていれば、ありの回だけ要求が減る（または失敗が増える）ことで差に現れる
//
// 場面:
//   D1（配信する設定）  最小の画面。配信する設定は実行時に教材を読み込まず（`EngineHost::for_mount` は session なし）、
//                        誘導の完走ができないため、起動と入力欄への入力・送信までを確かめる。`window.__fandheTest` が無いことも確かめる
//   T1（試験の設定）    最小教材（min-3step）の 3 Step を、実クリックと実入力で達成まで進める
//   T2（試験の設定）    CH-1 の 6 Step を含む区間（st-0-1 から st-2-1）をキーボードだけで進める。Step の到達ごとに走査
//   T3（試験の設定）    掲示板教材の代わりに、CH-2 の形を写した試験用の教材（ch0-3 の 20 Step・reload-points の 8 Step）の
//                        全 Step を「移動」（`go_to`）で順に開く。各 Step で走査
//   T4（試験の設定）    表示部品 7 種 × 変種 (a)・(b) の全 14 通りを実際の画面に mount して走査する。部品が増えれば
//                        artifact-view-check の CASES に入り、自動でこの検査の対象になる
//
// 確かめないこと:
//   - 本物の掲示板教材での移動（D-11。教材 repo の公開後）と配信する設定での教材の誘導の完走（実行時の教材の読み込みが未実装）。
//     限界は docs/testing.md 5.180 と docs/known-gaps.md に記録する
//   - favicon の取得。ヘッドレスでは揺れるため要求の種類の比較から外し、inline-free.test.mjs が直接の取得と静的検査で確かめる
//   - 移動の巡回で出るエンジンの警告・無視（入口の写しを無効にしうる）。件数だけ診断の行に残し、assert しない
//   - 入力した値や下書きの中身は出さない（名前・件数・合否だけ）
//
// 入力: FANDHE_WEB_DIR（D1）・FANDHE_WEB_TEST_DIR（TEST_HOOKS=1。T1〜T4）。未設定・不在は skip にせず失敗にする。
// 呼び出し元は scripts/test-browser.sh（make test-browser。CI の browser-test ジョブ）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
  cspConsoleMessages, requestKinds,
} from './harness.mjs';
import {
  DRAFT, RAW, T, at, engine, rowSel, choiceSel, decideSel, ledger, keyboardDriver, journey, install,
} from './ch0-3-journey.mjs';
import { CASES } from './artifact-view-check.mjs';

const BASE = '/fandhe-csp-runs/';
let browser;

before(async () => {
  for (const env of ['FANDHE_WEB_DIR', 'FANDHE_WEB_TEST_DIR']) {
    const dir = process.env[env];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${env} must point to a built web directory`);
  }
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 0 件の主張。どの区分が 0 でないかを名前で出し、入力値は出さない。favicon は揺れるため失敗の数から除く。
async function assertClean(page, rec, label, scanOpts = {}) {
  const counts = await collectSafety(page, rec, scanOpts);
  const failed = rec.failed.filter((f) => !/\/icon\.svg(\?|$)/.test(f.url));
  const extra = {
    cspConsole: cspConsoleMessages(rec).length,
    failed: failed.length,
    badStatus: rec.requests.filter((r) => r.status >= 400 && !/\/icon\.svg(\?|$)/.test(r.url)).length,
  };
  const bad = [...nonZero(counts), ...Object.keys(extra).filter((k) => extra[k] !== 0)];
  assert.deepEqual(bad, [], `${label}: ${JSON.stringify({ counts, extra })}`);
}

const midScan = { settleMs: 0, provoke: false };

// 1 つの場面を CSP あり → なしで流し、要求の種類を比べる。fn は (page, rec, srv) を受け取り、各所で assertClean を呼ぶ。
async function runScene({ envName, query = '', fn }) {
  const kindsByMode = [];
  for (const csp of [true, false]) {
    await withCspMode({ dir: process.env[envName], csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        await page.goto(`${srv.url}${query}`);
        await waitMounted(page);
        await fn(page, rec, srv);
        await assertClean(page, rec, `CSP ${csp ? 'あり' : 'なし'} の終わり`);
        kindsByMode.push(requestKinds(rec, { origin: srv.origin, base: BASE }));
      } finally {
        await context.close();
      }
    }));
  }
  assert.ok(kindsByMode[0].length > 0, '配信元への要求が記録されていない');
  assert.deepEqual(kindsByMode[0], kindsByMode[1], `CSP の有無で要求の種類が違う: ${JSON.stringify(kindsByMode)}`);
}

const hooksOk = async (page) => assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');

test('D1: 配信する設定の最小の画面は、起動と入力欄への入力・送信で通信・違反・警告・エラーが 0 件で、試験用の口が無い', async () => {
  await runScene({
    envName: 'FANDHE_WEB_DIR',
    fn: async (page, rec) => {
      assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', '配信する設定に試験用の口がある');
      await assertClean(page, rec, '起動直後', midScan);
      const input = page.locator(DRAFT);
      const n = await input.count();
      console.log(`D1: 入力欄 ${n} 件`);
      assert.equal(n, 1, '配信する設定の入力欄がちょうど 1 件ではない（入力と送信を省略しない）');
      await input.fill('x');
      await input.press('Enter');
      await page.waitForTimeout(300);
    },
  });
});

test('T1: 最小教材の 3 Step を実クリックと実入力で達成まで進めても、通信・違反・警告・エラーが 0 件', async () => {
  await runScene({
    envName: 'FANDHE_WEB_TEST_DIR',
    fn: async (page, rec) => {
      await hooksOk(page);
      await at(page, 'main/st-1', 'entered');
      await assertClean(page, rec, 'st-1 入力待ち', midScan);
      await page.fill(DRAFT, '/make-note first-note');
      await page.press(DRAFT, 'Enter');
      await at(page, 'main/st-1', 'accepted');
      await assertClean(page, rec, 'st-1 確認待ち', midScan);
      await page.click(rowSel('notes/draft'), { timeout: T });
      await at(page, 'main/st-2', 'entered');
      await assertClean(page, rec, 'st-2 入力待ち', midScan);
      await page.click(choiceSel('opt-memo'), { timeout: T });
      await at(page, 'main/st-3', 'entered');
      await assertClean(page, rec, 'st-3 入力待ち', midScan);
      await page.click(decideSel('approve'), { timeout: T });
      await at(page, 'main/st-3', 'accepted');
      await page.click(decideSel('approve'), { timeout: T });
      await at(page, 'main/st-3', 'achieved');
      const e = await engine(page);
      assert.equal(e.log.filter((r) => r.kind === 'achieved').length, 3, '3 Step がすべて達成');
    },
  });
});

test('T2: CH-1 の 6 Step を含む区間をキーボードだけで進めても、Step ごとの走査を含めて 0 件', async () => {
  await runScene({
    envName: 'FANDHE_WEB_TEST_DIR',
    query: '?fixture=ch0-3',
    fn: async (page, rec) => {
      await hooksOk(page);
      await install(page);
      const L = ledger();
      await journey(page, keyboardDriver(page), 'csp-runs', L, 'main/st-2-1', {
        onStep: (uid, phase) => assertClean(page, rec, `${uid} ${phase}`, midScan),
      });
      assert.deepEqual(L.failed(), []);
      const status = (await engine(page)).stateWithoutLog.run.slices.main.status;
      assert.ok([1, 2, 3, 4, 5, 6].every((n) => status[`main/st-1-${n}`] === 'achieved'), 'CH-1 の 6 Step が達成');
    },
  });
});

// 移動の巡回。掲示板教材（repo に無い。D-11）の代わりに、CH-2 の形を写した試験用の教材の全 Step を開く。
function uidsOf(name) {
  const ir = JSON.parse(fs.readFileSync(new URL(`../../crates/web/tests/data/${name}/ir.json`, import.meta.url), 'utf8'));
  return ir.nodes.map((n) => n.uid);
}

// 文字列としてだけ渡す（コードの文字列を組み立てない）。
async function goTo(page, uid) {
  await page.evaluate(([sel, p]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', p);
    el.click();
  }, [RAW, JSON.stringify({ control: { go_to: uid } })]);
  await page.waitForFunction((u) => JSON.parse(window.__fandheTest.stateJson()).engine.current === u && window.__fandheTest.nextTick() === 'null', uid, { timeout: T });
}

for (const [label, query, ir, expected] of [
  ['ch0-3 の 20 Step', '?fixture=ch0-3', 'ch0_3', 20],
  ['reload-points の 8 Step（CH-2 と同じ形）', '?fixture=reload-points', 'reload_points_ch2', 8],
]) {
  test(`T3: ${label}を「移動」で順に開いても、各 Step で通信・違反・警告・エラーが 0 件`, async () => {
    const uids = uidsOf(ir);
    assert.equal(uids.length, expected);
    await runScene({
      envName: 'FANDHE_WEB_TEST_DIR',
      query,
      fn: async (page, rec) => {
        await hooksOk(page);
        for (const uid of uids) {
          await goTo(page, uid);
          assert.equal(await page.locator('[data-testid="part-artifact-view"]').count(), 1, `${uid}: 左の成果物の部品が描かれる`);
          await assertClean(page, rec, `移動 ${uid}`, midScan);
        }
        const e = await engine(page);
        console.log(`T3 ${ir}: 移動 ${uids.length} 回、エンジンの警告 ${e.warnings.length} 件（assert しない）`);
        assert.equal(e.current, uids[uids.length - 1]);
      },
    });
  });
}

test('T4: 表示部品 7 種 x 変種 (a)・(b) の全ケースを mount しても、通信・違反・警告・エラーが 0 件', async () => {
  assert.equal(CASES.length, 14);
  await runScene({
    envName: 'FANDHE_WEB_TEST_DIR',
    fn: async (page, rec) => {
      await hooksOk(page);
      await page.evaluate(() => {
        const box = document.createElement('div');
        box.id = 'view-fixture-root';
        document.body.append(box);
      });
      const mounted = JSON.parse(await page.evaluate(() => window.__fandheTest.viewFixtureMount('view-fixture-root')));
      assert.deepEqual(mounted, { ok: true });
      const dispatch = async (name, payload) => {
        const r = JSON.parse(await page.evaluate(([n, p]) => window.__fandheTest.viewFixtureDispatch(n, p), [name, payload]));
        return r.outcome === 'applied';
      };
      for (const { kind, variant, allowed } of CASES) {
        const tag = `${kind}:${variant}`;
        // 契約が許さない 2 通り（文書 (a)・差分 (a)）は、対照の口からだけ描ける（製品の fixture_select は拒否する）。
        if (allowed) assert.equal(await dispatch('fixture_select', tag), true, `${tag}: 選べる`);
        else assert.equal(await dispatch('fixture_contrast', tag), true, `${tag}: 対照の口で描ける`);
        for (const phase of ['before', 'after']) {
          assert.equal(await dispatch('fixture_phase', phase), true, `${tag} ${phase}`);
          await assertClean(page, rec, `${tag} ${phase}`, midScan);
        }
      }
    },
  });
});
