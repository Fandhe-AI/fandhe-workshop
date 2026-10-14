// 実際にビルドした配信設定の画面へ対照を挿入し、安全性の計数が SAFE-8 の期待と一致することを確かめる（TASK-143 2/2）。
//
// 対照: 生の img（onerror）・外部の img・インラインの script。計数の道具（harness.mjs。TASK-143 1/2）が実画面で
// 数えられることを示し、後続の TASK-144 以降の「0 件」が数えられない状態での 0 件でないことの裏づけにする。
// 期待値の出典: docs/spec/04-behavior/engine-safety.md の SAFE-8 と、PoC-7 の記録
// （docs/spec/03-poc/untrusted-input-safety/logs/xss-run.txt の 13〜14 行目）。
// 値が合わないときは期待値を書き換えず、道具・除外規則・待機の側を調べる。
// danger の 3 は、scanDom の既定 entryScripts（./app.js）が画面自身の module script を除外することに依存する（既定のまま呼ぶ）。
//
// 入力: 環境変数 FANDHE_WEB_DIR（scripts/test-browser.sh が用意する配信設定の web）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, SAFETY_KEYS, HIT_FN,
} from './harness.mjs';

const BASE = '/fandhe-smoke/';
const EXTERNAL_URL = 'https://example.invalid/PROBE';
const COMMON = { dialogs: 0, popups: 0, crashed: 0, pageErrors: 0, danger: 3, onAttrs: 1, styleAttrs: 0, nonHttp: 0, extHttp: 1, scanError: 0 };
const EXPECTED = {
  off: { ...COMMON, hits: 2, external: 1, csp: 0 },
  on: { ...COMMON, hits: 0, external: 0, csp: 3 },
};
// hits + external + csp の合計。挿入後の非同期の発火を待つ目安。
const TOTAL = { off: 3, on: 3 };

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to a built web directory');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

test('期待値が計数の全キーを過不足なく持つ', () => {
  for (const mode of ['on', 'off']) {
    assert.deepEqual(Object.keys(EXPECTED[mode]).sort(), [...SAFETY_KEYS].sort());
  }
});

for (const csp of [true, false]) {
  const mode = csp ? 'on' : 'off';
  test(`対照が CSP ${csp ? 'あり' : 'なし'}で SAFE-8 の期待どおりに数えられる`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.goto(srv.url);
      await waitMounted(page);

      // 固定の関数を渡し、記録用の関数名は引数で渡す（試験データから実行コードを組み立てない）。
      // page.evaluate は CSP の制約を受けないため挿入は行われ、挿入された要素の実行・読み込みだけが CSP で止まる。
      await page.evaluate((hitFn) => {
        document.body.insertAdjacentHTML(
          'beforeend',
          `<img src="x" onerror="${hitFn}('PROBE')"><img src="https://example.invalid/PROBE">`,
        );
        const s = document.createElement('script');
        s.textContent = `${hitFn}('PROBE-inline')`;
        document.body.append(s);
      }, HIT_FN);

      // img の onerror は 404 の往復後に発火するため、期待の合計に達するまで有限時間待つ。超過しても例外にせず集計へ進む。
      const deadline = Date.now() + 5000;
      while (rec.hits.length + rec.external.length + rec.csp.length < TOTAL[mode] && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }

      const counts = await collectSafety(page, rec);
      const detail = JSON.stringify({ counts, rec });
      assert.deepEqual(counts, EXPECTED[mode], detail);

      if (csp) {
        assert.deepEqual(rec.csp.map((v) => v.violatedDirective).sort(), ['img-src', 'script-src-attr', 'script-src-elem'], detail);
      } else {
        assert.deepEqual([...rec.hits].sort(), ['PROBE', 'PROBE-inline'], detail);
        assert.ok(rec.external[0].startsWith(EXTERNAL_URL), detail);
      }
      assert.ok((await page.locator('#app').count()) > 0, 'app must stay rendered');
      await context.close();
    }));
  });
}
