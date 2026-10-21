// 幅 960px 未満のタブの切り替えの試験（SHELL-1・A11Y-5。TASK-517 2/2）。
//
// 配信する設定の web を幅 959px と 360px で開き、次を確かめる。
//   - tablist（pane-tabs）が見え、tab が 2 つ（24×24px 以上）あり、既定は「操作」で、見える領域は操作（shell-right）だけである
//   - 「成果物」のクリックで成果物（shell-left）だけが見え、aria-selected・tabindex が入れ替わる
//   - 矢印（左右。端は巡回）・Home・End で切り替わり、フォーカスも選んだタブへ移る（続けて Enter・Space を押しても選択は戻らない）。Home・End はページを動かさない
//     （keydown の defaultPrevented が真）。対照として Alt+ArrowRight では何も変わらない
//   - 境目: 959px で成果物を選んだ後に 960px へ広げると、両方が見えて tablist が隠れ、959px へ戻すと選んだ側だけが見える
// 試験の設定（FANDHE_WEB_TEST_DIR）では、tab にフォーカスがある Esc がちょうど 1 回届くこと（FEND-6）を確かめる。
// どちらも、CSP 違反・ページ内のエラー・コンソールのエラーが 0 件である。
//
// 確かめないこと: フォーカスの置き方の細部（A11Y-21・未決）、既定のタブと Step をまたぐ保持（SHELL-2・未決。仮置き）、
// 360px の横スクロールと 24px 未満の操作対象の件数（TASK-518・TASK-368 の担当）。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted,
} from './harness.mjs';

const BASE = '/fandhe-tabs/';
const TAB_OPS = '#pane-tab-ops';
const TAB_ART = '#pane-tab-artifacts';
let browser;

before(async () => {
  for (const v of ['FANDHE_WEB_DIR', 'FANDHE_WEB_TEST_DIR']) {
    const dir = process.env[v];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${v} must point to a built web directory`);
  }
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 見えている領域の名前（left・right）を返す。
async function visiblePanes(page) {
  const out = [];
  if (await page.locator('[data-testid="shell-left"]').isVisible()) out.push('left');
  if (await page.locator('[data-testid="shell-right"]').isVisible()) out.push('right');
  return out;
}

// tab の選択の状態（id・aria-selected・tabindex）を返す。
const tabState = (page) => page.evaluate(() => [...document.querySelectorAll('[data-testid="pane-tab"]')]
  .map((t) => ({ id: t.id, selected: t.getAttribute('aria-selected'), tabindex: t.getAttribute('tabindex') })));

const focusedId = (page) => page.evaluate(() => document.activeElement?.id ?? null);

async function expectSelected(page, id, panes, label) {
  const st = await tabState(page);
  assert.equal(st.length, 2, label);
  for (const t of st) {
    const on = t.id === id;
    assert.equal(t.selected, on ? 'true' : 'false', `${label}: aria-selected ${JSON.stringify(st)}`);
    assert.equal(t.tabindex, on ? '0' : '-1', `${label}: tabindex ${JSON.stringify(st)}`);
  }
  assert.deepEqual(await visiblePanes(page), panes, `${label}: visible panes`);
}

function noErrors(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

for (const width of [959, 360]) {
  test(`幅 ${width}px: tablist が見え、既定は操作だけが見える`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width, height: 800 } });
      await page.goto(srv.url);
      await waitMounted(page);
      const tabs = page.locator('[data-testid="pane-tabs"]');
      assert.equal(await tabs.isVisible(), true);
      for (const sel of [TAB_OPS, TAB_ART]) {
        const box = await page.locator(sel).boundingBox();
        assert.ok(box && box.width >= 24 && box.height >= 24, `${sel} size ${JSON.stringify(box)}`);
      }
      await expectSelected(page, 'pane-tab-ops', ['right'], '既定');
      noErrors(rec);
      await context.close();
    });
  });

  test(`幅 ${width}px: クリックで領域が切り替わる`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width, height: 800 } });
      await page.goto(srv.url);
      await waitMounted(page);
      await page.click(TAB_ART);
      await expectSelected(page, 'pane-tab-artifacts', ['left'], '成果物をクリック');
      await page.click(TAB_OPS);
      await expectSelected(page, 'pane-tab-ops', ['right'], '操作をクリック');
      noErrors(rec);
      await context.close();
    });
  });

  test(`幅 ${width}px: 矢印・Home・End で切り替わり、フォーカスも選んだタブへ移る。Alt つきでは変わらない`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width, height: 800 } });
      await page.goto(srv.url);
      await waitMounted(page);
      // Home・End の既定の動作（スクロール）が止められていることを、keydown の defaultPrevented で見る。
      await page.evaluate(() => {
        window.__prevented = [];
        document.addEventListener('keydown', (e) => window.__prevented.push([e.key, e.defaultPrevented]));
      });
      // roving tabindex: 選ばれた tab だけが Tab で着く。
      await page.focus(TAB_OPS);
      assert.equal(await focusedId(page), 'pane-tab-ops');
      const steps = [
        ['ArrowRight', 'pane-tab-artifacts', ['left']],
        ['ArrowRight', 'pane-tab-ops', ['right']],
        ['ArrowLeft', 'pane-tab-artifacts', ['left']],
        ['Home', 'pane-tab-ops', ['right']],
        ['End', 'pane-tab-artifacts', ['left']],
        ['ArrowRight', 'pane-tab-ops', ['right']],
      ];
      for (const [key, id, panes] of steps) {
        await page.keyboard.press(key);
        await expectSelected(page, id, panes, key);
        // フォーカスは選んだタブへ移る（glue が set_timeout(0) で移すので待つ）。
        await page.waitForFunction((want) => document.activeElement && document.activeElement.id === want, id, { timeout: 2000 });
      }
      // キーで選んだタブにフォーカスがあるので、Enter・Space を押しても元のタブの select-pane は発火せず、選択は戻らない。
      for (const activate of ['Enter', 'Space']) {
        await page.keyboard.press('ArrowRight');
        await page.waitForFunction(() => document.activeElement?.id === 'pane-tab-artifacts', null, { timeout: 2000 });
        await page.keyboard.press(activate);
        await expectSelected(page, 'pane-tab-artifacts', ['left'], activate);
        await page.keyboard.press('Home');
        await expectSelected(page, 'pane-tab-ops', ['right'], 'Home');
        await page.waitForFunction(() => document.activeElement?.id === 'pane-tab-ops', null, { timeout: 2000 });
      }
      const prevented = await page.evaluate(() => window.__prevented);
      for (const key of ['Home', 'End']) {
        const hit = prevented.filter(([k]) => k === key);
        assert.ok(hit.length > 0 && hit.every(([, p]) => p === true), `${key} must be default-prevented: ${JSON.stringify(prevented)}`);
      }
      // 対照: 修飾キーつきでは変わらない。
      await page.keyboard.press('Alt+ArrowRight');
      await expectSelected(page, 'pane-tab-ops', ['right'], 'Alt+ArrowRight');
      noErrors(rec);
      await context.close();
    });
  });

  test(`幅 ${width}px: tab にフォーカスがある Esc はちょうど 1 回届く`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width, height: 800 } });
      await page.goto(srv.url);
      await waitMounted(page);
      const esc = () => page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).escCount);
      await page.focus(TAB_OPS);
      const before = await esc();
      await page.keyboard.press('Escape');
      await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
      await new Promise((r) => setTimeout(r, 80));
      assert.equal((await esc()) - before, 1);
      // Esc は選択を変えない。
      await expectSelected(page, 'pane-tab-ops', ['right'], 'Esc の後');
      noErrors(rec);
      await context.close();
    });
  });
}

test('境目: 959px で選んだ側が、960px で両方が見え、959px へ戻すと選んだ側だけが見える', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 959, height: 800 } });
    await page.goto(srv.url);
    await waitMounted(page);
    await page.click(TAB_ART);
    await expectSelected(page, 'pane-tab-artifacts', ['left'], '959px');
    await page.setViewportSize({ width: 960, height: 800 });
    assert.deepEqual(await visiblePanes(page), ['left', 'right'], '960px');
    assert.equal(await page.locator('[data-testid="pane-tabs"]').isVisible(), false);
    await page.setViewportSize({ width: 959, height: 800 });
    await expectSelected(page, 'pane-tab-artifacts', ['left'], '959px へ戻す');
    noErrors(rec);
    await context.close();
  });
});
