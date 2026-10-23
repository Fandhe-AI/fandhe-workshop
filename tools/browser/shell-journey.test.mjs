// 枠の通しの試験の骨格（CH-0〜CH-3 の 20 Step を幅 1280px と 360px で最後まで通す）（SHELL-1・SHELL-7・SHELL-8・SHELL-10。TASK-548 1/2）。
//
// 試験用の教材 `ch0-3`（`?fixture=ch0-3-chapters`）を、実クリックと実キーで最後まで進める。章の境目は「次の章へ進む」の 1 個で越える。
// 各 Step で確かめること（誘導と観察は shell-journey.mjs）:
//   - 左右の取り付け点に ART の部品（成果物のビュー）と TERM の部品（ログ・Step の題と do）が載り、逆側には載っていない
//   - Step の題・do が IR のとおり出る。ログが増える。effects を持つ Step では左の成果物が変わる（変化が無いまま状態だけ変わる場面は 0 件）
//   - 上部の位置が IR から数えた形で出て、章が変わると章の ID と K が変わる
//   - 360px では、操作のタブで題が、成果物のタブでビューが見える。1280px ではタブを切り替えない
// 最後に、20 Step の達成・エンジンの警告と無視が 0 件・ページ内のエラーと CSP 違反が 0 件・配信元の外への通信が 0 件・安全性の計数が 0 件を確かめ、
// 同じ CSP の設定どうしで 1280px と 360px の最終の世界状態と各 Step の状態が一致することを確かめる。
// 対照: 成果物のビューを hidden にすると見え方の判定が、位置の文字を書き換えると位置の判定が、それぞれ不合格になる（空振りの防止）。
// 解釈・結果の記録の様式は docs/testing.md 5.269。チェックポイント・再読み込み・書き出しは TASK-548 2/2（shell-journey.mjs の hooks.afterStep）。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { ledger } from './ch0-3-journey.mjs';
import {
  runShellJourney, recordLine, positionFailures, visibleViaTab, LEFT, ARTIFACT, POSITION,
} from './shell-journey.mjs';

const BASE = '/fandhe-shell-journey/';
const QUERY = '?fixture=ch0-3-chapters';
const WIDTHS = [1280, 360];
let browser;
// 幅をまたぐ比較用。csp ごとに幅の結果を保持する（node:test は同じファイルの試験を順に流す）。
const results = new Map();

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

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  for (const width of WIDTHS) {
    test(`幅 ${width}px で CH-0 から CH-3 の 20 Step を最後まで通せる（${mode}）`, { timeout: 300000 }, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width, height: 800 } });
        try {
          const L = ledger();
          await page.goto(`${srv.url}${QUERY}`);
          await waitMounted(page);
          assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
          const scene = `${width}px`;
          const result = await runShellJourney(page, { width, scene, L });
          const s = summarize(rec);
          L.check(scene, 'ページ内のエラーが 0 件', s.pageErrors === 0);
          L.check(scene, 'CSP 違反が 0 件', s.csp === 0);
          L.check(scene, '配信元の外への通信が 0 件', rec.external.length === 0);
          const counts = await collectSafety(page, rec);
          L.check(scene, '安全性の計数が全区分 0', nonZero(counts).length === 0);
          const st = result.stat;
          L.check(scene, '20 Step がすべて達成', result.achieved === 20 && result.current === 'main/st-3-2');
          L.check(scene, '章の移動が 3 回', st.crossings === 3);
          L.check(scene, '左の変化が全対象で合格', st.left[1] > 0 && st.left[0] === st.left[1]);
          L.check(scene, '右の変化が全対象で合格', st.right[1] === 19 && st.right[0] === st.right[1]);
          L.check(scene, '題と do が全 Step で合格', st.title[1] === 20 && st.title[0] === st.title[1]);
          L.check(scene, '位置が全 Step で合格', st.position[1] === 20 && st.position[0] === st.position[1]);
          console.log(recordLine({ width, csp, result, L, rec }));
          assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
          results.set(`${csp}/${width}`, { world: result.world, statuses: result.statuses });

          // 対照: 成果物のビューを hidden にすると見え方の判定が、位置の文字を書き換えると位置の判定が不合格になる。
          // style 属性を使わないので CSP ありでも流せる。
          await page.evaluate(([left, art]) => { document.querySelector(`${left} ${art}`).hidden = true; }, [LEFT, ARTIFACT]);
          assert.equal(await visibleViaTab(page, { clicks: 0 }, 'pane-tab-artifacts', `${LEFT} ${ARTIFACT}`), false, 'control: hidden artifact view must fail');
          await page.evaluate((sel) => { document.querySelector(sel).textContent = '書き換え'; }, POSITION);
          assert.notDeepEqual(await positionFailures(page, 'main/st-3-2'), [], 'control: rewritten position must fail');
        } finally {
          await context.close();
        }
      }));
    });
  }

  test(`1280px と 360px で最終の世界状態と各 Step の状態が一致する（${mode}）`, () => {
    const wide = results.get(`${csp}/1280`);
    const narrow = results.get(`${csp}/360`);
    assert.ok(wide && narrow, '両方の幅の結果がある');
    assert.equal(narrow.world, wide.world);
    assert.equal(narrow.statuses, wide.statuses);
  });
}
