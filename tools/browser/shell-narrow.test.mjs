// 幅 360px で枠の部品が欠けないことの試験（SHELL-3。TASK-518 1/2）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。試験の設定（TEST_HOOKS=1）の `?fixture=ch0-3-chapters` を
// 幅 360px で開き、実クリックだけで枠の部品が出る場面を作って測る。章をまたぐ誘導は ch0-3-journey.mjs の journey（chaptered）。
// 場面: N1 開いた直後（操作・成果物の両タブ）／N2 チェックポイントを保存した一覧とヒント／N3 書き出しの後／
//       N4 削除の確認の通知（確定しない）／N5 閉じるボタンのある案内の通知（Safari の UA）／N6 章の終わりの「次の章へ進む」。
// 各場面で確かめること:
//   (a) ページの scrollWidth が clientWidth 以下で、360 以下（横スクロールなし）
//   (b) 見えている操作対象（a11y::TARGET_SIZE_SELECTOR と同じ集合）がすべて 24×24 CSS ピクセル以上
//   (c) その場面の枠の部品が見え、左右が [0, 360] に収まる
//   (d) 部品の中心を elementFromPoint で引くと、その部品自身かその子孫が返る（押せる）
//   (e) ページ内のエラー・CSP 違反が 0 件
// 負の対照（CSSOM の el.style だけで変える。CSP の下でも動く）: 高さ 20px のボタン・幅 500px の要素・部品を覆う要素を
// 入れると、(b)・(a)・(d) がそれぞれ検出する。
// 範囲外: CH-1 の通しの 28/28（TASK-518 2/2）、4 幅の観察（TASK-368）、縦の長さ・読みやすさ（TASK-387）。
// 出力には testid・タグ名・寸法・件数・合否だけを出す（入力値・履歴の中身・ファイル名は出さない）。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted,
} from './harness.mjs';
import {
  ledger, mouseDriver, journey, paneAware, NEXT,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-shell-narrow/';
const QUERY = '?fixture=ch0-3-chapters';
const WIDTH = 360;
const SAVE = '[data-testid="checkpoint-save"]';
const HINT = '[data-testid="hint-open"]';
const EXPORT_BTN = '[data-testid="shell-export"]';
const DELETE = '[data-testid="shell-delete"]';
const FILE_LABEL = '.sh-import-label';
const FILE_INPUT = '.sh-import-label input[type="file"]';
const FRAME = [SAVE, EXPORT_BTN, DELETE, FILE_LABEL, FILE_INPUT, '[data-testid="pane-tab"]'];
// a11y::TARGET_SIZE_SELECTOR の写し（Rust の定数は import できない。正は crates/web/src/a11y.rs）。
const TARGETS = 'button, select, input:not([type="hidden"]), [data-action]';
const UA_SAFARI = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15';
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

const label = (el) => `${el.tag}[${el.id}] ${el.w}x${el.h}`;

// (a)(b) ページ全体の観察。値は件数と testid だけ。
const observe = (page) => page.evaluate((targets) => {
  const small = [...document.querySelectorAll(targets)].flatMap((el) => {
    const r = el.getBoundingClientRect();
    if (!(r.width > 0 && r.height > 0)) return [];
    if (r.width >= 24 && r.height >= 24) return [];
    return [{ tag: el.tagName.toLowerCase(), id: el.getAttribute('data-testid') ?? '-', w: Math.round(r.width), h: Math.round(r.height) }];
  });
  const de = document.documentElement;
  return { scrollWidth: de.scrollWidth, clientWidth: de.clientWidth, small };
}, TARGETS);

// (c)(d) 部品ごとの観察。見えている要素すべてについて、収まりと「中心を引くと自身か子孫」を数える。
async function partState(page, sel) {
  const loc = page.locator(sel);
  const n = await loc.count();
  const out = { sel, visible: 0, inside: 0, hit: 0 };
  for (let i = 0; i < n; i += 1) {
    const el = loc.nth(i);
    if (!(await el.isVisible())) continue;
    await el.scrollIntoViewIfNeeded();
    const r = await el.evaluate((node) => {
      const b = node.getBoundingClientRect();
      const top = document.elementFromPoint(b.left + b.width / 2, b.top + b.height / 2);
      return { l: b.left, r: b.right, hit: top !== null && (top === node || node.contains(top)) };
    });
    out.visible += 1;
    if (r.l >= 0 && r.r <= WIDTH + 0.5) out.inside += 1;
    if (r.hit) out.hit += 1;
  }
  return out;
}

// 1 場面を測って台帳へ積む。parts は「見えているはずの部品」の selector。
async function measure(page, scene, parts, L) {
  const o = await observe(page);
  L.check(scene, '(a) 横スクロールなし', o.scrollWidth <= o.clientWidth && o.scrollWidth <= WIDTH);
  L.check(scene, `(b) 24px 未満の操作対象が 0 件${o.small.length ? `（${o.small.map(label).join(', ')}）` : ''}`, o.small.length === 0);
  for (const sel of parts) {
    const p = await partState(page, sel);
    L.check(scene, `(c) ${sel} が見え 360px に収まる`, p.visible >= 1 && p.inside === p.visible);
    L.check(scene, `(d) ${sel} が押せる`, p.visible >= 1 && p.hit === p.visible);
  }
  return o;
}

// 両タブで測る。tablist が見えているので、選んでから測る。
async function measureBothPanes(page, scene, parts, L) {
  for (const pane of ['ops', 'artifacts']) {
    await page.click(`#pane-tab-${pane}`);
    await measure(page, `${scene}/${pane}`, parts, L);
  }
}

const quiet = (L, scene, rec) => {
  const s = summarize(rec);
  L.check(scene, '(e) ページ内のエラーが 0 件', s.pageErrors === 0);
  L.check(scene, '(e) CSP 違反が 0 件', s.csp === 0);
};

test('幅 360px で枠の部品が欠けない（N1〜N4・N6）', { timeout: 300000 }, async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: WIDTH, height: 800 } });
    const L = ledger();
    await page.goto(`${srv.url}${QUERY}`);
    await waitMounted(page);
    await measureBothPanes(page, 'N1 開いた直後', FRAME, L);

    // N2: 2 件保存して一覧を出し、ヒントを開く。N6: 章の終わりで「次の章へ進む」を測ってから押す。
    let saves = 0;
    let hints = 0;
    const onStep = async (uid, status) => {
      if (status !== 'entered') return;
      if (saves < 2) {
        await page.click(SAVE);
        saves += 1;
      }
      if (hints < 2 && (await page.locator(HINT).count()) === 1 && (await page.locator(HINT).isEnabled())) {
        await page.click(HINT);
        hints += 1;
        // ヒントの段は Step を移ると消えるので、開いた直後に測る（上部は両タブで共通）。
        if (hints === 1) await measure(page, 'N2 ヒント', [...FRAME, HINT], L);
      }
    };
    const counter = { clicks: 0 };
    const base = paneAware(page, mouseDriver(page), counter);
    const driver = {
      ...base,
      nextChapter: async () => {
        await page.click('#pane-tab-ops');
        await measure(page, 'N6 章の終わり', [...FRAME, NEXT], L);
        await base.nextChapter();
      },
    };
    // stopAt が効くのは st-1-3・st-1-4・st-2-1・st-2-6 だけ。st-2-1 なら CH-0→CH-1・CH-1→CH-2 の 2 つの章の境目（N6）を測って止まる。
    await journey(page, driver, 'N', L, 'main/st-2-1', { chaptered: true, onStep });
    L.check('N2', '保存が 2 件', saves === 2);
    await measureBothPanes(page, 'N2 一覧', [...FRAME, '[data-testid="checkpoint-restore"]'], L);
    L.check('N2', 'ヒントを 1 回以上開いた', hints >= 1);

    // N3: 書き出しの後。リンクか状態の文が出る。
    await page.click(EXPORT_BTN);
    await page.locator('[data-testid="export-download"], [data-testid="export-status"]').first().waitFor({ state: 'visible', timeout: 15000 });
    await measureBothPanes(page, 'N3 書き出しの後', [...FRAME, '[data-testid="export-download"]'], L);

    // N4: 削除の確認（確定しない）。
    await page.click(DELETE);
    await page.locator('[data-testid="shell-notice-item"][data-notice-kind="delete-confirm"]').waitFor({ state: 'visible', timeout: 15000 });
    await measureBothPanes(page, 'N4 削除の確認', [...FRAME, '[data-testid="shell-delete-confirm"]', '[data-testid="shell-delete-cancel"]'], L);

    quiet(L, 'run', rec);
    console.log(`diagnostic N1-N4,N6: tab clicks=${counter.clicks} saves=${saves} hints=${hints} ledger=${L.summary()}`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  });
});

test('幅 360px で閉じるボタンのある通知が欠けない（N5）', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: WIDTH, height: 800 }, userAgent: UA_SAFARI });
    const L = ledger();
    await page.goto(`${srv.url}${QUERY}`);
    await waitMounted(page);
    const item = '[data-testid="shell-notice-item"][data-notice-kind="browser-notice"]';
    L.check('N5', '案内が 1 件', (await page.locator(item).count()) === 1);
    await measureBothPanes(page, 'N5 案内の通知', [...FRAME, item, '[data-testid="shell-dismiss"]'], L);
    quiet(L, 'N5', rec);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  });
});

// 負の対照: 測り方が素通りでないことを、ページの中だけの変更（CSSOM）で確かめる。
test('対照: 高さ 20px のボタン・幅 500px の要素・覆いを検出する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: WIDTH, height: 800 } });
    await page.goto(`${srv.url}${QUERY}`);
    await waitMounted(page);
    const clean = await observe(page);
    assert.equal(clean.small.length, 0, 'baseline has no small target');
    assert.equal((await partState(page, DELETE)).hit, 1, 'baseline: delete is hittable');

    await page.locator(DELETE).evaluate((el) => {
      el.style.minHeight = '0';
      el.style.height = '20px';
    });
    const c1 = await observe(page);
    assert.ok(c1.small.some((s) => s.id === 'shell-delete'), 'C1: short button is detected');
    await page.locator(DELETE).evaluate((el) => {
      el.style.height = '';
      el.style.minHeight = '';
    });

    await page.evaluate(() => {
      const d = document.createElement('div');
      d.style.width = '500px';
      d.style.height = '4px';
      document.body.appendChild(d);
    });
    const c2 = await observe(page);
    assert.ok(c2.scrollWidth > WIDTH, 'C2: wide element is detected');

    await page.locator(DELETE).evaluate((el) => {
      const b = el.getBoundingClientRect();
      const cover = document.createElement('div');
      cover.style.position = 'fixed';
      cover.style.left = `${b.left}px`;
      cover.style.top = `${b.top}px`;
      cover.style.width = `${b.width}px`;
      cover.style.height = `${b.height}px`;
      cover.style.zIndex = '9999';
      document.body.appendChild(cover);
    });
    assert.equal((await partState(page, DELETE)).hit, 0, 'C3: covered button is detected');
    await context.close();
  });
});
