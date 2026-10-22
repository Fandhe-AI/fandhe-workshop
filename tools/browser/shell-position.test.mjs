// 上部の教材名・章の ID・Step の位置の表示の試験（SHELL-8・D-72。TASK-524）。
//
// 試験用の教材 `ch0-3`（`?fixture=ch0-3-chapters`）の 20 Step を通し、各 Step で教材名（shell-title）と位置（shell-position）が
// 決めた形「<章の ID>　Step M / K」で出ること、章の中で M が 1 ずつ増え、章が変わると章の ID と K が変わることを確かめる。
// 期待値は、試験の中で教材の IR（crates/web/tests/data/ch0_3/ir.json）を読んで独自に数える（画面の data-position-* を
// 読み返して合格にしない）。再読み込みの後と、書き出し→別の文脈での読み込みの後にも、復元した Step の位置が出る。
// 解釈: 章の ID は教材の ID の綴り（ch-1）のまま出す（spec の例の CH-1 と大小が違う。docs/known-gaps.md）。
// 対照: 位置を display: none にすると判定が不合格になる。試験の出力に、入力した値は出さない。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_WEB_DIR（配信する設定）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted,
} from './harness.mjs';
import {
  ledger, mouseDriver, journey,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-shell-position/';
const QUERY = '?fixture=ch0-3-chapters';
const TITLE = '[data-testid="shell-title"]';
const POSITION = '[data-testid="shell-position"]';
const EXPORT_BTN = '[data-testid="shell-export"]';
const LINK = '[data-testid="export-download"]';
// `?fixture=ch0-3-chapters` の変種の教材名（crates/web/src/test_fixture.rs が pack.title だけ書き換える）。
const TITLE_TEXT = '章の移動の通しの試験データ';
let browser;
let tmpDir;

const IR = JSON.parse(fs.readFileSync(
  path.join(path.dirname(fileURLToPath(import.meta.url)), '../../crates/web/tests/data/ch0_3/ir.json'),
  'utf8',
));

// IR から独自に数えた期待値。
function expected(uid) {
  const node = IR.nodes.find((n) => n.uid === uid);
  const list = IR.nodes.filter((n) => n.scope === 'main' && n.unit === node.unit);
  return { chapter: node.unit, m: list.findIndex((n) => n.uid === uid) + 1, k: list.length };
}

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(
    dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(),
    'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1',
  );
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'shell-position-'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

// 教材名と位置の判定。uid が null のときは文字の形だけを確かめず、出ていることだけを見る。返すのは失敗した項目。
async function probe(page, uid) {
  const failed = [];
  const dom = await page.evaluate(([t, p]) => {
    const one = (sel) => {
      const all = document.querySelectorAll(sel);
      if (all.length !== 1) return { count: all.length };
      const e = all[0];
      const r = e.getBoundingClientRect();
      const cs = getComputedStyle(e);
      return {
        count: 1,
        text: e.textContent,
        m: e.getAttribute('data-position-m'),
        k: e.getAttribute('data-position-k'),
        visible: r.width > 0 && r.height > 0 && cs.display !== 'none' && cs.visibility !== 'hidden',
        inPanel: !!e.closest('[role="tabpanel"]'),
        inTop: !!e.closest('[data-testid="shell-top"]'),
      };
    };
    return { title: one(t), pos: one(p) };
  }, [TITLE, POSITION]);
  if (dom.title.count !== 1) failed.push('教材名が 1 個');
  else {
    if (dom.title.text !== TITLE_TEXT) failed.push('教材名の文字');
    if (!dom.title.visible) failed.push('教材名が見える');
    if (dom.title.inPanel || !dom.title.inTop) failed.push('教材名が shell-top の中・tabpanel の外');
  }
  if (dom.pos.count !== 1) failed.push('位置が 1 個');
  else {
    const e = expected(uid);
    if (dom.pos.text !== `${e.chapter}　Step ${e.m} / ${e.k}`) failed.push('位置の文字');
    if (dom.pos.m !== String(e.m) || dom.pos.k !== String(e.k)) failed.push('位置の属性');
    if (!dom.pos.visible) failed.push('位置が見える');
    if (dom.pos.inPanel || !dom.pos.inTop) failed.push('位置が shell-top の中・tabpanel の外');
    if (dom.pos.text.includes(uid) || /st-\d/i.test(dom.pos.text)) failed.push('Step の ID を出さない');
  }
  return failed;
}

function check(L, scene, name, failed) {
  L.check(scene, `${name}: 教材名・位置`, failed.length === 0);
  if (failed.length) console.log(`失敗 ${scene} ${name}: ${failed.join(' / ')}`);
}

async function currentUid(page) {
  return page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.current);
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
    console.log(`位置 (CSP ${csp ? 'あり' : 'なし'}, ${viewport.width}px): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  }));
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`全 Step で教材名・章の ID・位置が出て、CH-1 で M が 1 ずつ増える（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, { width: 1280, height: 800 }, async (page, L) => {
      const seen = [];
      const ch1 = [];
      const onStep = async (uid, status) => {
        const cur = await currentUid(page);
        check(L, uid, status, await probe(page, cur));
        if (status === 'entered') {
          seen.push(cur);
          if (expected(cur).chapter === 'ch-1') ch1.push(Number(await page.getAttribute(POSITION, 'data-position-m')));
        }
      };
      await journey(page, mouseDriver(page), '通し', L, null, { chaptered: true, onStep });
      L.check('通し', '20 Step を訪れた', new Set(seen).size === 20);
      L.check('通し', 'CH-1 の M が 1 から 6 まで 1 ずつ増える', JSON.stringify(ch1) === '[1,2,3,4,5,6]');
      const chapters = new Set(seen.map((u) => `${expected(u).chapter}/${expected(u).k}`));
      L.check('通し', '章が変わると章の ID と K が変わる', chapters.size === 4);
    });
  });
}

test('再読み込みの後と、書き出し→別の文脈での読み込みの後に位置が出る', { timeout: 300000 }, async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withServer({ dir, base: BASE }, async (srv) => {
    const A = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    const L = ledger();
    await A.page.goto(`${srv.url}${QUERY}`);
    await waitMounted(A.page);
    await journey(A.page, mouseDriver(A.page), '途中', L, 'main/st-1-4', { chaptered: true });
    await A.page.reload();
    await waitMounted(A.page);
    check(L, '再読み込み', '後', await probe(A.page, 'main/st-1-4'));
    assert.equal(await A.page.textContent(POSITION), 'ch-1　Step 4 / 6');

    await A.page.locator(EXPORT_BTN).click();
    const link = A.page.locator(LINK);
    await link.waitFor({ state: 'visible', timeout: 10000 });
    const [download] = await Promise.all([A.page.waitForEvent('download'), link.click()]);
    const file = path.join(tmpDir, 'out.history.json');
    await download.saveAs(file);

    const B = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    await B.page.goto(`${srv.url}${QUERY}`);
    await waitMounted(B.page);
    await B.page.evaluate(() => window.__fandheTest.importInstall());
    await B.page.setInputFiles('[data-testid="history-file"]', file);
    await B.page.waitForFunction(
      (sel) => document.querySelector(sel)?.textContent === 'ch-1　Step 4 / 6',
      POSITION,
      { timeout: 10000 },
    );
    check(L, '読み込み', '後', await probe(B.page, 'main/st-1-4'));
    L.check('共通', 'ページ内のエラーが 0 件', summarize(A.rec).pageErrors === 0 && summarize(B.rec).pageErrors === 0);
    assert.deepEqual(L.failed(), []);
    await A.context.close();
    await B.context.close();
  });
});

for (const width of [360, 959]) {
  test(`幅 ${width}px でも教材名・位置が見えて横スクロールが無い`, async () => {
    await openScene(true, { width, height: 800 }, async (page, L) => {
      check(L, `${width}px`, '初期', await probe(page, await currentUid(page)));
      const overflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
      L.check(`${width}px`, '横スクロールが無い', !overflow);
    });
  });
}

test('配信する設定では教材が無いとき位置を出さず、枠は崩れない', async () => {
  // 試験の組み立て（scripts/test-browser.sh）は IR を同梱せずに配信する設定を作る。教材が無いので教材名・位置は出ない（fail-closed）。
  // 教材ありの教材名・位置は試験の設定の各場面で確かめる（本物の教材での確認は教材 repo の担当）。
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to a delivery build');
  await withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await page.waitForSelector('[data-testid="shell-mock"]');
    assert.equal(await page.locator('[data-testid="shell-top"]').count(), 1);
    assert.equal(await page.locator(POSITION).count(), 0);
    assert.equal(await page.locator(`${POSITION}[data-position-m="0"]`).count(), 0);
    assert.equal(summarize(rec).pageErrors, 0);
    await context.close();
  });
});

test('対照: 位置を display: none にすると判定が不合格になる', async () => {
  await openScene(true, { width: 1280, height: 800 }, async (page, L) => {
    const uid = await currentUid(page);
    check(L, '対照', '付ける前', await probe(page, uid));
    await page.evaluate((sel) => document.querySelector(sel).style.setProperty('display', 'none'), POSITION);
    const failed = await probe(page, uid);
    console.log(`対照の不合格の項目: ${failed.join(' / ')}`);
    L.check('対照', '位置が見える が不合格になる', failed.includes('位置が見える'));
  });
});
