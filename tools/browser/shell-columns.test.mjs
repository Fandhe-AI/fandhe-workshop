// 画面の枠の 2 列の骨格の試験（SHELL-1。TASK-517 1/2）。
//
// 配信する設定の web をサブパスで配り、幅 1280px と 960px で成果物（shell-left）と操作（shell-right）が横に並んで同時に見え、
// tablist（pane-tabs）が隠れることを確かめる。対照として 959px では tablist が見え、2 つの領域が横に並ばない。
// 960px 未満で片方だけを表示する挙動と切り替えは TASK-517 2/2 の担当で、ここでは確かめない。
//
// 入力: 環境変数 FANDHE_WEB_DIR（tools/build-web.sh が出力した配信設定のディレクトリ）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted,
} from './harness.mjs';

const BASE = '/fandhe-columns/';
let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to a built web directory');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 枠の配置を 1 回で測る。要素が無ければ null を返し、呼び出し側が失敗にする。
async function measure(page) {
  return page.evaluate(() => {
    const box = (id) => {
      const e = document.querySelector(`[data-testid="${id}"]`);
      if (!e) return null;
      const r = e.getBoundingClientRect();
      const cs = getComputedStyle(e);
      return { left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: r.width, height: r.height, display: cs.display, visibility: cs.visibility };
    };
    return {
      left: box('shell-left'),
      right: box('shell-right'),
      tabs: box('pane-tabs'),
      scrollWidth: document.documentElement.scrollWidth,
      innerWidth: window.innerWidth,
    };
  });
}

for (const width of [1280, 960]) {
  test(`幅 ${width}px で成果物と操作が横に並び、tablist が隠れる`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width, height: 800 } });
      await page.goto(srv.url);
      await waitMounted(page);
      const m = await measure(page);
      assert.ok(m.left && m.right && m.tabs, JSON.stringify(m));
      assert.ok(m.left.width > 0 && m.left.height > 0, 'shell-left must be visible');
      assert.ok(m.right.width > 0 && m.right.height > 0, 'shell-right must be visible');
      assert.ok(m.left.right <= m.right.left + 0.5, `left must be left of right: ${JSON.stringify(m)}`);
      assert.ok(Math.abs(m.left.top - m.right.top) <= 1, `tops must align: ${JSON.stringify(m)}`);
      assert.equal(m.tabs.display, 'none');
      assert.ok(m.scrollWidth <= m.innerWidth, `no horizontal scroll: ${JSON.stringify(m)}`);
      const s = summarize(rec);
      const detail = JSON.stringify(rec);
      assert.equal(s.csp, 0, detail);
      assert.equal(s.pageErrors, 0, detail);
      assert.equal(s.consoleErrors, 0, detail);
      await context.close();
    });
  });
}

test('対照: 幅 959px では tablist が見え、2 つの領域は横に並ばない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: 959, height: 800 } });
    await page.goto(srv.url);
    await waitMounted(page);
    const m = await measure(page);
    assert.ok(m.left && m.right && m.tabs, JSON.stringify(m));
    assert.notEqual(m.tabs.display, 'none');
    assert.ok(m.tabs.width > 0 && m.tabs.height > 0, 'pane-tabs must be visible');
    assert.ok(m.right.top >= m.left.bottom - 0.5, `panes must be stacked: ${JSON.stringify(m)}`);
    await context.close();
  });
});
