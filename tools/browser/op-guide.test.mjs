// 操作プロファイルの説明（`op-guide`）への修飾キーの表示のブラウザ試験（TASK-392 1/2。A11Y-25・D-48・D-79）。
//
// 推定した OS（`openPage` の `userAgent`）と受講者が選ぶ欄（`os-select`）で、説明の中の修飾キーが Cmd／Ctrl に
// 切り替わることを、CSP あり・なしの両方で確かめる。修飾キーは記号ではなく文字で出す（D-79）。
//
// 確かめること:
//   - Windows・macOS・Linux の UA で、説明が 1 件・右のエリアの中・role／aria なし。Linux では Cmd・Ctrl を出さない
//   - 選ぶ欄で切り替えると、説明と貼り付けの案内の両方が新しい OS の文言になり、要素は同じノードのまま
//   - 選ぶ欄が、アクセシビリティツリー上の名前（お使いの OS）を持ち、送信ボタンから Tab 1 回で届く
//   - 入力欄なしの教材（`?fixture=no-input`）では説明が 0 件
//   - 安全性の計数（通信・違反・ダイアログ等）が 0 件
//
// 確かめないこと:
//   - 名前の読み上げを人が聞く確認・推定が外れたときに直せるかの人の確認（TASK-393・TASK-460）・Windows の実機（TASK-395）
//   - 選ぶ欄の値をキーボードで変える操作（Chromium の版で揺れうるため assert にしない）
//   - 4 つの操作が OS によらず同じ結果になる試験（2/2。#704）
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-op-guide/';
const GUIDE = '[data-testid="op-guide"]';
const HINT = '[data-testid="key-hint"]';
const SELECT = '[data-testid="os-select"]';
const SUBMIT = '[data-testid="submit-input"]';
const OPS = '操作: Enter で送信、Shift+Enter で改行、Tab で補完、Esc で処理中は停止・候補が開いていれば閉じる・それ以外は取り消し。';
const TEXT = {
  mac: `${OPS}コピーは Cmd+C、貼り付けは Cmd+V`,
  windows: `${OPS}コピーは Ctrl+C、貼り付けは Ctrl+V`,
  unknown: `${OPS}コピー・貼り付けのキーは、OS を選ぶと表示されます`,
};
const HINT_TEXT = { mac: '貼り付け: Cmd+V', windows: '貼り付け: Ctrl+V' };

const UA = {
  windows: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  mac: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  linux: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
};

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function run(csp, opts, body) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, userAgent: opts.ua });
    await page.goto(opts.query ? `${srv.url}${opts.query}` : srv.url);
    await waitMounted(page);
    await body(page);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
    await context.close();
  }));
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  for (const c of [
    { name: 'Windows', ua: UA.windows, text: TEXT.windows },
    { name: 'macOS', ua: UA.mac, text: TEXT.mac },
    { name: 'Linux（推定できない）', ua: UA.linux, text: TEXT.unknown },
  ]) {
    test(`${c.name} の UA では説明の修飾キーが合う（CSP ${label}）`, async () => {
      await run(csp, { ua: c.ua }, async (page) => {
        assert.equal(await page.locator(GUIDE).count(), 1);
        assert.equal(await page.locator(GUIDE).textContent(), c.text);
        const info = await page.locator(GUIDE).evaluate((el) => ({
          attrs: el.getAttributeNames().filter((n) => n === 'role' || n.startsWith('aria-')),
          inRight: el.closest('[data-testid="shell-right"]') !== null,
        }));
        assert.deepEqual(info.attrs, []);
        assert.equal(info.inRight, true);
        if (c.text === TEXT.unknown) {
          const t = await page.locator(GUIDE).textContent();
          assert.ok(!t.includes('Cmd') && !t.includes('Ctrl'), t);
        }
      });
    });
  }

  for (const sw of [
    { name: 'Windows の推定を mac に直す', ua: UA.windows, to: 'mac' },
    { name: 'Linux（推定できない）から windows を選ぶ', ua: UA.linux, to: 'windows' },
  ]) {
    test(`選ぶ欄で切り替えると説明が変わり要素は同じノードのまま: ${sw.name}（CSP ${label}）`, async () => {
      await run(csp, { ua: sw.ua }, async (page) => {
        await page.evaluate(([g, h, s, t]) => {
          window.__keep = {
            guide: document.querySelector(g),
            hint: document.querySelector(h),
            select: document.querySelector(s),
            step: document.querySelector(t)?.textContent ?? null,
          };
        }, [GUIDE, HINT, SELECT, '[data-testid="step-text"]']);
        await page.locator(SELECT).selectOption(sw.to);
        await page.waitForFunction(([g, t]) => document.querySelector(g)?.textContent === t, [GUIDE, TEXT[sw.to]]);
        assert.equal(await page.locator(HINT).textContent(), HINT_TEXT[sw.to]);
        const same = await page.evaluate(([g, h, s, t]) => ({
          guide: window.__keep.guide.isSameNode(document.querySelector(g)),
          hint: window.__keep.hint.isSameNode(document.querySelector(h)),
          select: window.__keep.select.isSameNode(document.querySelector(s)),
          step: window.__keep.step === (document.querySelector(t)?.textContent ?? null),
        }), [GUIDE, HINT, SELECT, '[data-testid="step-text"]']);
        assert.deepEqual(same, { guide: true, hint: true, select: true, step: true });
      });
    });
  }

  test(`選ぶ欄は名前を持ち Tab で届く（CSP ${label}）`, async () => {
    await run(csp, { ua: UA.windows }, async (page) => {
      const byRole = page.getByRole('combobox', { name: /お使いの OS/ });
      assert.equal(await byRole.count(), 1);
      const same = await byRole.evaluate((el) => el === document.querySelector('[data-testid="os-select"]'));
      assert.equal(same, true);
      await page.locator(SUBMIT).focus();
      await page.keyboard.press('Tab');
      const active = await page.evaluate(() => document.activeElement?.getAttribute('data-testid'));
      assert.equal(active, 'os-select');
    });
  });

  test(`入力欄なしの教材では説明が 0 件（CSP ${label}）`, async () => {
    await run(csp, { ua: UA.mac, query: '?fixture=no-input' }, async (page) => {
      assert.equal(await page.locator(GUIDE).count(), 0);
      assert.equal(await page.locator(SELECT).count(), 0);
    });
  });
}
