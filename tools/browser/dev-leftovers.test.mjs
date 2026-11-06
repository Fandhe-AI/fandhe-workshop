// 開発用の残り物（「Hello」の文字と受け付けた入力の数）が配信する設定の画面で見えないことの試験（SHELL-35。#2081）。
//
// 入力: FANDHE_WEB_AUTHOR_DIR（配信する設定。CH-0〜CH-3 の IR 同梱）・FANDHE_WEB_DIR（配信する設定。IR なし）。
// 未設定・不在は skip にせず失敗にする。ビルドは scripts/test-browser.sh が行う。
//
// 確かめること:
//   A 20 Step の通し（CSP あり・なし）の入った直後と確認待ちの全時点で、「Hello」が 0 件、受け付けた入力の数の欄 3 つが
//     1 件ずつあり、見えず、アクセシビリティツリーにも出ず、textContent は数字のまま。通しの後に値が 0 より大きい。
//   B IR の無い配信する設定を開いた直後も「Hello」が 0 件。
//   C 負の対照。欄の hidden を外すと見えない・読み上げの判定が不合格になり、「Hello」の p を差し込むと Hello の判定が不合格になる。
// 出力は項目名・件数・合否だけで、DOM の文字は出さない。記録は docs/testing.md 5.475。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { ledger, mouseDriver, journey, install } from './ch0-3-journey.mjs';
import { domObserver03 } from './author-preview.mjs';

const BASE = '/fandhe-dev-leftovers/';
const CONSUMED = ['part-log-consumed', 'part-artifact-consumed', 'part-review-consumed'];
let browser;
const dirs = {};

function requireDir(name) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a built web directory`);
  return dir;
}

before(async () => {
  dirs.web = requireDir('FANDHE_WEB_DIR');
  dirs.author = requireDir('FANDHE_WEB_AUTHOR_DIR');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 「Hello」の判定。返すのは失敗した項目の名前の配列（空なら合格）。
async function probeHello(page) {
  const failed = [];
  const dom = await page.evaluate(() => ({
    message: document.querySelectorAll('[data-testid="message"]').length,
    text: document.body.innerText.includes('Hello'),
  }));
  if (dom.message !== 0) failed.push('message の要素が 0 件');
  if (dom.text) failed.push('画面に Hello の文字が無い');
  return failed;
}

// 受け付けた入力の数の欄の判定。返すのは失敗した項目の名前の配列（空なら合格）。
async function probeConsumed(page) {
  const failed = [];
  for (const id of CONSUMED) {
    const sel = `[data-testid="${id}"]`;
    const dom = await page.evaluate((s) => {
      const all = document.querySelectorAll(s);
      if (all.length !== 1) return { count: all.length };
      const e = all[0];
      const r = e.getBoundingClientRect();
      return {
        count: 1,
        hiddenAncestor: !!e.closest('[hidden]'),
        display: getComputedStyle(e).display,
        empty: r.width === 0 && r.height === 0,
        numeric: /^\d+$/.test(e.textContent),
      };
    }, sel);
    if (dom.count !== 1) { failed.push(`${id}: 1 件`); continue; }
    if (!dom.hiddenAncestor) failed.push(`${id}: hidden の中`);
    if (dom.display !== 'none') failed.push(`${id}: display none`);
    if (!dom.empty) failed.push(`${id}: 大きさ 0`);
    if (!dom.numeric) failed.push(`${id}: 数字のまま`);
    if (!(await page.locator(sel).isHidden())) failed.push(`${id}: 見えない`);
    // 読み上げに出ないこと。親の部品の ariaSnapshot に欄の数字の行が無いことでは判定が曖昧なため、欄そのものの snapshot を使う。
    const tree = await page.locator(sel).ariaSnapshot({ timeout: 1000 }).catch(() => '');
    if (tree.trim() !== '') failed.push(`${id}: 読み上げに出ない`);
  }
  return failed;
}

async function probe(page) {
  return [...(await probeHello(page)), ...(await probeConsumed(page))];
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  test(`A: 20 Step の全時点で「Hello」と受け付けた入力の数が見えない（${mode}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: dirs.author, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        const L = ledger();
        await page.goto(srv.url);
        await waitMounted(page);
        await install(page);
        const points = [];
        const onStep = async (uid, status) => {
          points.push({ uid, status, failed: await probe(page) });
        };
        await journey(page, mouseDriver(page), '配信', L, null, { chaptered: true, observe: domObserver03(), onStep });
        assert.deepEqual(L.failed(), [], '台帳');
        assert.ok(points.length > 0, '時点の数が 0（空振り）');
        assert.ok(points.some((p) => p.status === 'accepted'), '確認待ちの時点がある');
        const bad = points.filter((p) => p.failed.length).map((p) => `${p.uid} ${p.status}: ${p.failed.join(' / ')}`);
        assert.deepEqual(bad, [], '見える・読み上げに出る時点');
        const values = await page.evaluate((ids) => ids.map((id) => Number(document.querySelector(`[data-testid="${id}"]`).textContent)), CONSUMED);
        assert.ok(values.every((v) => v > 0), '通しの後に値が 0 より大きい');
        assert.deepEqual(nonZero(await collectSafety(page, rec)), [], '安全の計数');
        assert.equal(rec.pageErrors.length, 0, 'ページ内エラー');
      } finally {
        await context.close();
      }
    }));
  });
}

test('B: IR の無い配信する設定の画面にも「Hello」が無い', { timeout: 60000 }, async () => {
  await withServer({ dir: dirs.web, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      assert.deepEqual(await probeHello(page), []);
    } finally {
      await context.close();
    }
  });
});

test('C: 負の対照。hidden を外す・Hello を差し込むと探針が不合格になる', { timeout: 120000 }, async () => {
  await withServer({ dir: dirs.author, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      await install(page);
      assert.deepEqual(await probe(page), [], '対照の前は合格');
      await page.evaluate((ids) => {
        for (const id of ids) document.querySelector(`[data-testid="${id}"]`).removeAttribute('hidden');
      }, CONSUMED);
      const unhidden = await probeConsumed(page);
      for (const id of CONSUMED) {
        assert.ok(unhidden.some((f) => f.startsWith(`${id}: 見えない`)), `${id}: 見えない判定が不合格`);
        assert.ok(unhidden.some((f) => f.startsWith(`${id}: 読み上げに出ない`)), `${id}: 読み上げの判定が不合格`);
      }
      await page.evaluate(() => {
        const p = document.createElement('p');
        p.setAttribute('data-testid', 'message');
        p.textContent = 'Hello';
        document.querySelector('[data-testid="hello-root"]').prepend(p);
      });
      assert.equal((await probeHello(page)).length, 2, 'Hello の 2 判定が不合格');
    } finally {
      await context.close();
    }
  });
});
