// 幅 959px と 960px の境目とタブのキーボード切り替えの試験（A11Y-5・SHELL-1。TASK-369）。
//
// PoC-12 の再試験の手順 2 の R-23 (a)〜(d) を自動ブラウザで確かめる。shell-columns・shell-tabs が確かめる範囲に、次を足す。
//   - tablist（aria-label「表示する領域」）と tab（「操作」「成果物」）のアクセシブルな名前・aria-controls／aria-labelledby の対応と、
//     アクセシビリティツリーの形（959px は tablist 1・tabpanel 1、960px は tablist 0・tabpanel 2）
//   - 境目の両方向（960px で開いて 959px へ縮める。959px で開いて広げる方向は shell-tabs が持つ）
//   - Tab だけで選択中の tab に着くこと（roving tabindex）と、960px では Tab が tab に着かないこと
//   - 中身のある画面（`?fixture=ch0-3`）でのキー切り替えがエンジンの状態・下書きを変えないことと、キーボードだけの CH-1 の通し
//   - 境目の判定（boundaryViolations）が空振りで通らないことの対照
// 確かめないこと: フォーカスの置き方の細部（A11Y-21・未決）、360px（TASK-518・TASK-368 の担当）、小数の幅（viewport は整数）。
// 試験の出力に、入力した値は出さない（名前・件数・真偽だけ）。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted,
} from './harness.mjs';
import {
  ledger, keyboardDriver, keyPaneAware, journey, engine, DRAFT,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-boundary/';
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

const open = async (srv, width, query = '') => {
  const o = await openPage(browser, { origin: srv.origin, viewport: { width, height: 800 } });
  await o.page.goto(`${srv.url}${query}`);
  await waitMounted(o.page);
  return o;
};

function noErrors(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

// AX ツリーの数え方はここ 1 か所に閉じる（書式に依存しすぎないため行の頭の役割名だけを見る）。
async function axCounts(page) {
  const snap = await page.locator('body').ariaSnapshot();
  const lines = snap.split('\n').map((l) => l.trim());
  return {
    tablist: lines.filter((l) => /^- tablist\b/.test(l) && l.includes('"表示する領域"')).length,
    tabpanel: lines.filter((l) => /^- tabpanel\b/.test(l)).length,
  };
}

const geom = (page) => page.evaluate(() => {
  const rect = (sel) => {
    const e = document.querySelector(sel);
    if (!e) return null;
    const cs = getComputedStyle(e);
    const r = e.getBoundingClientRect();
    return { shown: cs.display !== 'none' && cs.visibility !== 'hidden' && r.width > 0 && r.height > 0, l: r.left, r: r.right, t: r.top };
  };
  return {
    tabs: rect('[data-testid="pane-tabs"]'),
    left: rect('[data-testid="shell-left"]'),
    right: rect('[data-testid="shell-right"]'),
    overflowX: document.documentElement.scrollWidth - window.innerWidth,
  };
});

// 境目の判定。違反の名前の配列を返す（空が合格）。
async function boundaryViolations(page, width) {
  const v = [];
  const g = await geom(page);
  const roleVisible = await page.getByRole('tablist', { name: '表示する領域' }).isVisible();
  const ax = await axCounts(page);
  const narrow = width < 960;
  const tabsShown = !!g.tabs?.shown && roleVisible;
  if (narrow && !tabsShown) v.push('tablist-hidden-below-960');
  if (!narrow && (g.tabs?.shown || roleVisible)) v.push('tablist-visible-at-960');
  const shown = [g.left, g.right].filter((p) => p?.shown).length;
  if (narrow && shown !== 1) v.push('panes-not-one-below-960');
  if (!narrow && !(shown === 2 && g.left.r <= g.right.l + 0.5 && Math.abs(g.left.t - g.right.t) <= 1)) v.push('panes-not-side-by-side-at-960');
  if (ax.tablist !== (narrow ? 1 : 0) || ax.tabpanel !== (narrow ? 1 : 2)) v.push('ax-tabpanel-count');
  if (g.overflowX > 0) v.push('horizontal-scroll');
  return v;
}

const tabInfo = (page) => page.evaluate(() => [...document.querySelectorAll('[role="tab"]')].map((t) => {
  const panel = document.getElementById(t.getAttribute('aria-controls'));
  return {
    name: t.textContent.trim(),
    selected: t.getAttribute('aria-selected'),
    tabindex: t.getAttribute('tabindex'),
    panelRole: panel?.getAttribute('role') ?? null,
    back: panel?.getAttribute('aria-labelledby') === t.id,
  };
}));

const selectedName = async (page) => (await tabInfo(page)).filter((t) => t.selected === 'true').map((t) => t.name);
const panesShown = async (page) => {
  const g = await geom(page);
  return [g.left?.shown && 'left', g.right?.shown && 'right'].filter(Boolean);
};
const activeIsTab = (page) => page.evaluate(() => document.activeElement?.matches('[role="tab"]') ?? false);
const activeId = (page) => page.evaluate(() => document.activeElement?.id ?? null);
const waitActive = (page, id) => page.waitForFunction((w) => document.activeElement?.id === w, id, { timeout: 3000 });

// 読み込み直後から Tab で最初に着く tab を探す。着かなければ null（上限まで押す）。
async function firstTabByTab(page, limit = 40) {
  for (let n = 0; n < limit; n += 1) {
    await page.keyboard.press('Tab');
    if (await activeIsTab(page)) return activeId(page);
  }
  return null;
}

test('B1 959px の冷えた起動: tablist と tab の名前・対応・AX ツリー', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, 959);
    assert.equal(await page.getByRole('tablist', { name: '表示する領域' }).isVisible(), true);
    const tabs = await tabInfo(page);
    assert.deepEqual(tabs.map((t) => t.name), ['操作', '成果物']);
    assert.deepEqual(tabs.map((t) => t.selected), ['true', 'false']);
    assert.ok(tabs.every((t) => t.panelRole === 'tabpanel' && t.back), JSON.stringify(tabs));
    assert.deepEqual(await panesShown(page), ['right']);
    assert.deepEqual(await boundaryViolations(page, 959), []);
    noErrors(rec);
    await context.close();
  });
});

test('B2 960px の冷えた起動: tablist なし・2 領域が横に並ぶ', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, 960);
    assert.equal(await page.getByRole('tablist', { name: '表示する領域' }).isVisible(), false);
    assert.deepEqual(await panesShown(page), ['left', 'right']);
    assert.deepEqual(await boundaryViolations(page, 960), []);
    noErrors(rec);
    await context.close();
  });
});

test('B3 境目の逆方向: 960px で開き 959px へ縮め、キーで選び、広げて戻す', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, 960);
    await page.setViewportSize({ width: 959, height: 800 });
    assert.deepEqual(await boundaryViolations(page, 959), []);
    assert.deepEqual(await selectedName(page), ['操作']);
    assert.equal(await firstTabByTab(page), 'pane-tab-ops');
    await page.keyboard.press('End');
    await waitActive(page, 'pane-tab-artifacts');
    assert.deepEqual(await panesShown(page), ['left']);
    await page.setViewportSize({ width: 960, height: 800 });
    assert.deepEqual(await boundaryViolations(page, 960), []);
    await page.setViewportSize({ width: 959, height: 800 });
    assert.deepEqual(await boundaryViolations(page, 959), []);
    assert.deepEqual(await panesShown(page), ['left']);
    assert.deepEqual(await selectedName(page), ['成果物']);
    noErrors(rec);
    await context.close();
  });
});

test('B4 959px: Tab で選択中の tab に着き、矢印・Home・End で追従する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, 959);
    assert.equal((await tabInfo(page)).filter((t) => t.tabindex === '0').length, 1);
    assert.equal(await firstTabByTab(page), 'pane-tab-ops');
    const steps = [
      ['ArrowRight', 'pane-tab-artifacts', ['left']],
      ['ArrowLeft', 'pane-tab-ops', ['right']],
      ['End', 'pane-tab-artifacts', ['left']],
      ['Home', 'pane-tab-ops', ['right']],
    ];
    for (const [key, id, panes] of steps) {
      await page.keyboard.press(key);
      await waitActive(page, id);
      assert.deepEqual(await panesShown(page), panes, key);
      assert.equal((await tabInfo(page)).filter((t) => t.tabindex === '0').length, 1, key);
    }
    await page.keyboard.press('Tab');
    assert.equal(await activeIsTab(page), false, 'Tab は tablist の外へ出る');
    await page.keyboard.press('Shift+Tab');
    assert.equal(await activeId(page), 'pane-tab-ops');
    noErrors(rec);
    await context.close();
  });
});

test('B5 960px: Tab は tab に着かず、矢印・Home・End は何も変えない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, 960);
    assert.equal(await firstTabByTab(page, 60), null);
    for (const key of ['ArrowRight', 'Home', 'End']) {
      await page.keyboard.press(key);
      assert.deepEqual(await panesShown(page), ['left', 'right'], key);
      assert.deepEqual(await selectedName(page), ['操作'], key);
    }
    assert.deepEqual(await boundaryViolations(page, 960), []);
    noErrors(rec);
    await context.close();
  });
});

test('B6 中身のある画面: キー切り替えは下書きとエンジンの状態を変えない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, 959, '?fixture=ch0-3');
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    await page.locator(DRAFT).waitFor();
    const onDraft = () => page.evaluate((s) => document.activeElement?.matches(s) ?? false, DRAFT);
    for (let n = 0; n < 40 && !(await onDraft()); n += 1) await page.keyboard.press('Tab');
    assert.equal(await onDraft(), true, '入力欄に着く');
    await page.keyboard.type('abc');
    const view = async () => {
      const e = await engine(page);
      return { v: await page.inputValue(DRAFT), cur: e.current, n: e.log.length, s: JSON.stringify(e.stateWithoutLog) };
    };
    const before = await view();
    assert.equal(before.v, 'abc');
    for (let n = 0; n < 40 && !(await activeIsTab(page)); n += 1) await page.keyboard.press('Shift+Tab');
    assert.equal(await activeIsTab(page), true, 'tab に着く');
    await page.keyboard.press('ArrowRight');
    await waitActive(page, 'pane-tab-artifacts');
    assert.deepEqual(await panesShown(page), ['left']);
    await page.keyboard.press('ArrowLeft');
    await waitActive(page, 'pane-tab-ops');
    assert.deepEqual(await view(), before);
    noErrors(rec);
    await context.close();
  });
});

test('B7 959px: キーボードだけで CH-1 の 6 Step を通せる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv, 959, '?fixture=ch0-3');
    const L = ledger();
    const counter = { keys: 0 };
    await journey(page, keyPaneAware(page, keyboardDriver(page), counter), '959px-key', L, 'main/st-2-1');
    const e = await engine(page);
    L.check('959px-key', 'main/st-2-1 に入った', e.current === 'main/st-2-1');
    L.check('959px-key', 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
    L.check('959px-key', 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
    L.check('959px-key', 'キーでのタブの切り替えが 1 回以上', counter.keys >= 1);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    noErrors(rec);
    await context.close();
  });
});

test('B8 検出器の対照: 加工した画面では違反が出て、加工しなければ空', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const wide = await open(srv, 960);
    assert.deepEqual(await boundaryViolations(wide.page, 960), []);
    await wide.page.evaluate(() => document.querySelector('[data-testid="pane-tabs"]').style.setProperty('display', 'flex', 'important'));
    assert.ok((await boundaryViolations(wide.page, 960)).includes('tablist-visible-at-960'));
    await wide.context.close();
    const narrow = await open(srv, 959);
    assert.deepEqual(await boundaryViolations(narrow.page, 959), []);
    await narrow.page.evaluate(() => document.querySelector('[data-testid="pane-tabs"]').setAttribute('data-pane', 'both'));
    assert.ok((await boundaryViolations(narrow.page, 959)).includes('panes-not-one-below-960'));
    await narrow.context.close();
  });
});
