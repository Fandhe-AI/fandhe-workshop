// 「模擬」の常時表示の試験（SHELL-6・D-72。TASK-523）。
//
// 画面の枠の上部（shell-top）の「模擬」（shell-mock）が、どの場面でも 1 個だけ見えており、アクセシビリティツリーにも
// 出ることを確かめる。場面: CH-0 から CH-3 の 20 Step の通しと章の移動（CSP あり・なし）、幅 360px・959px・1280px の
// タブの切り替え、通知が出ている間、再読み込みの後、配信する設定（FANDHE_WEB_DIR）。
// 解釈: spec の検証方法は「全 22 Step」だが、試験用の教材 `ch0-3` は 20 Step（本物の CH-0〜CH-3 は TASK-642）。
// 対照: 「模擬」を display: none にすると、可視の判定とアクセシビリティツリーの判定の両方が不合格になる。
// 試験の出力に、入力した値は出さない。入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_WEB_DIR（配信する設定）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted,
} from './harness.mjs';
import {
  ledger, mouseDriver, journey,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-shell-mock/';
const QUERY = '?fixture=ch0-3-chapters';
const MOCK = '[data-testid="shell-mock"]';
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

// 「模擬」の判定。返すのは失敗した項目の名前の配列（空なら合格）。
async function probe(page) {
  const failed = [];
  const dom = await page.evaluate((sel) => {
    const all = document.querySelectorAll(sel);
    if (all.length !== 1) return { count: all.length };
    const e = all[0];
    const r = e.getBoundingClientRect();
    const cs = getComputedStyle(e);
    return {
      count: 1,
      text: e.textContent,
      visible: r.width > 0 && r.height > 0 && cs.display !== 'none' && cs.visibility !== 'hidden',
      inPanel: !!e.closest('[role="tabpanel"]'),
      inTop: !!e.closest('[data-testid="shell-top"]'),
    };
  }, MOCK);
  if (dom.count !== 1) failed.push('個数が 1');
  else {
    if (dom.text !== '模擬') failed.push('文字が「模擬」');
    if (!dom.visible) failed.push('見えている');
    if (dom.inPanel) failed.push('tabpanel の外');
    if (!dom.inTop) failed.push('shell-top の中');
    const tree = await page.locator(MOCK).ariaSnapshot().catch(() => '');
    if (!tree.includes('模擬')) failed.push('アクセシビリティツリーに出る');
  }
  return failed;
}

function check(L, scene, name, failed) {
  L.check(scene, `${name}: 「模擬」の表示`, failed.length === 0);
  if (failed.length) console.log(`失敗 ${scene} ${name}: ${failed.join(' / ')}`);
}

async function openScene(csp, viewport, fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport });
    const L = ledger();
    await page.goto(`${srv.url}${QUERY}`);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    await fn(page, L, srv);
    const s = summarize(rec);
    L.check('共通', 'ページ内のエラーが 0 件', s.pageErrors === 0);
    L.check('共通', 'CSP 違反が 0 件', s.csp === 0);
    console.log(`模擬 (CSP ${csp ? 'あり' : 'なし'}, ${viewport.width}px): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  }));
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`CH-0 から CH-3 の全 Step と章の移動で「模擬」が残る（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, { width: 1280, height: 800 }, async (page, L) => {
      const seen = new Set();
      check(L, '開いた直後', '初期', await probe(page));
      const onStep = async (uid, status) => {
        seen.add(uid);
        check(L, `${uid}`, status, await probe(page));
      };
      await journey(page, mouseDriver(page), '通し', L, null, { chaptered: true, onStep });
      L.check('通し', '20 Step を訪れた', seen.size === 20);
      check(L, '通しの後', '終了', await probe(page));
    });
  });
}

for (const width of [360, 959, 1280]) {
  test(`幅 ${width}px でタブを切り替えても「模擬」が残る`, async () => {
    await openScene(true, { width, height: 800 }, async (page, L) => {
      const scene = `${width}px`;
      check(L, scene, '初期', await probe(page));
      const tabs = page.locator('[data-testid="pane-tabs"]');
      if (await tabs.isVisible()) {
        await page.click('#pane-tab-artifacts');
        check(L, scene, '成果物のタブ', await probe(page));
        await page.click('#pane-tab-ops');
        check(L, scene, '操作のタブ', await probe(page));
        for (const key of ['ArrowRight', 'Home', 'End']) {
          await page.focus('[data-testid="pane-tab"][aria-selected="true"]');
          await page.keyboard.press(key);
          check(L, scene, key, await probe(page));
        }
      } else {
        L.check(scene, '1280px では tablist が見えない', width >= 960);
      }
    });
  });
}

test('通知が出ている間と閉じた後も「模擬」が残る', async () => {
  await openScene(true, { width: 360, height: 800 }, async (page, L) => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'shell-mock-'));
    try {
      const file = path.join(tmp, 'broken.history.json');
      fs.writeFileSync(file, '{"broken":');
      await page.evaluate(() => window.__fandheTest.importInstall());
      await page.setInputFiles('[data-testid="history-file"]', file);
      await page.waitForSelector('[data-testid="shell-notice-item"]');
      check(L, '通知', '表示中', await probe(page));
      const dismiss = page.locator('[data-testid="shell-dismiss"]');
      if (await dismiss.count()) {
        await dismiss.first().click();
        check(L, '通知', '閉じた後', await probe(page));
      }
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });
});

test('再読み込みの後も「模擬」が残る', async () => {
  await openScene(true, { width: 1280, height: 800 }, async (page, L) => {
    await journey(page, mouseDriver(page), '途中', L, 'main/st-1-4', { chaptered: true });
    await page.reload();
    await waitMounted(page);
    check(L, '再読み込み', '後', await probe(page));
  });
});

test('配信する設定でも「模擬」が出る', async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to a delivery build');
  await withServer({ dir, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await page.waitForSelector(MOCK);
    const failed = await probe(page);
    assert.deepEqual(failed, []);
    await context.close();
  });
});

test('対照: display を none にすると判定が不合格になる', async () => {
  await openScene(true, { width: 1280, height: 800 }, async (page, L) => {
    check(L, '対照', '付ける前', await probe(page));
    await page.evaluate((sel) => document.querySelector(sel).style.setProperty('display', 'none'), MOCK);
    const failed = await probe(page);
    console.log(`対照の不合格の項目: ${failed.join(' / ')}`);
    L.check('対照', '見えている が不合格になる', failed.includes('見えている'));
    L.check('対照', 'アクセシビリティツリーに出る が不合格になる', failed.includes('アクセシビリティツリーに出る'));
  });
});
