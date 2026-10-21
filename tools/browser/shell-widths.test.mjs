// 4 つの幅（1280px・960px・959px・360px）で CH-1 の 6 Step を通す試験（SHELL-1。TASK-517 2/2）。
//
// 解釈: CH-1 から始める口（直接リンク。TASK-109）がまだ無いため、試験用の教材（`?fixture=ch0-3`）を CH-0 から通し、
// CH-1 の最後（`main/st-1-6`）を終えて `main/st-2-1` に入るまで進める（`main/st-1-1`〜`st-1-6` が CH-1 の 6 Step）。
// 操作は実クリックと実キーだけで行う。960px 未満では、操作の前に要るタブ（指す・見るのは成果物、答える・入力する・
// 決めるのは操作）を、tablist が見えているときだけクリックして選ぶ。見えているかは幅ではなく計算された display で判定する。
//
// 確かめること:
//   - 台帳がすべて合格（各方式の口の数・確認待ちの口）で、エンジンの警告・無視が 0 件、ページ内のエラーが 0 件
//   - 959px・360px では、通しの途中でタブを 1 回以上切り替えた。1280px・960px では 0 回（tablist が見えない）
// 確かめないこと:
//   - 360px の横スクロールと 24px 未満の操作対象の件数（TASK-518・TASK-368 の担当）。件数だけを diagnostic に残す
//   - 試験の出力に、入力した値は出さない（名前・件数・合否だけ）
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted,
} from './harness.mjs';
import {
  ledger, mouseDriver, journey, engine,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-shell-widths/';
const QUERY = '?fixture=ch0-3';
const WIDTHS = [1280, 960, 959, 360];
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

// 操作の前に、要るタブを選ぶドライバ。tablist が見えていないとき（2 列）は何もしない。
function paneAware(page, d, counter) {
  const ensure = async (id) => {
    const visible = await page.evaluate(() => {
      const e = document.querySelector('[data-testid="pane-tabs"]');
      return !!e && getComputedStyle(e).display !== 'none';
    });
    if (!visible) return;
    const selected = await page.getAttribute(`#${id}`, 'aria-selected');
    if (selected === 'true') return;
    await page.click(`#${id}`);
    counter.clicks += 1;
  };
  const ops = (name) => async (...args) => {
    await ensure('pane-tab-ops');
    return d[name](...args);
  };
  return {
    point: async (...args) => {
      await ensure('pane-tab-artifacts');
      return d.point(...args);
    },
    choose: ops('choose'),
    decide: ops('decide'),
    revise: ops('revise'),
    send: ops('send'),
    sendMultiline: ops('sendMultiline'),
    completeAndSend: ops('completeAndSend'),
    esc: ops('esc'),
  };
}

for (const width of WIDTHS) {
  test(`幅 ${width}px で CH-1 の 6 Step を通せる`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width, height: 800 } });
      const L = ledger();
      const counter = { clicks: 0 };
      await page.goto(`${srv.url}${QUERY}`);
      await waitMounted(page);
      assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
      const scene = `${width}px`;
      await journey(page, paneAware(page, mouseDriver(page), counter), scene, L, 'main/st-2-1');
      const e = await engine(page);
      L.check(scene, 'main/st-2-1 に入った', e.current === 'main/st-2-1');
      L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
      L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
      const narrow = width < 960;
      L.check(scene, narrow ? 'タブを 1 回以上切り替えた' : 'タブの切り替えが 0 回', narrow ? counter.clicks >= 1 : counter.clicks === 0);
      const s = summarize(rec);
      L.check(scene, 'ページ内のエラーが 0 件', s.pageErrors === 0);
      L.check(scene, 'CSP 違反が 0 件', s.csp === 0);
      // 360px の欠けの計数は TASK-518・TASK-368 の担当。assert せず、件数だけを残す。
      const diag = await page.evaluate(() => ({
        overflowX: document.documentElement.scrollWidth - window.innerWidth,
        small: [...document.querySelectorAll('button, [role="tab"], a[href], input, select, textarea')]
          .filter((el) => {
            const r = el.getBoundingClientRect();
            return r.width > 0 && r.height > 0 && (r.width < 24 || r.height < 24);
          }).length,
      }));
      console.log(`diagnostic ${scene}: tab clicks=${counter.clicks} overflowX=${diag.overflowX} small-targets=${diag.small} ledger=${L.summary()}`);
      assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
      await context.close();
    });
  });
}
