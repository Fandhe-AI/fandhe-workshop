// 修飾キーの案内（Cmd／Ctrl）の切り替えが右のエリア（shell-right）で働くことと、4 つの操作の結果が OS によらず同じことの
// ブラウザ試験（TASK-456。TERM-38・JUDGE-28・D-48）。機構そのもの（推定・選ぶ欄・復号）は TASK-285 の
// `modifier-key-hint.test.mjs` と native の試験が持つ。ここは枠（TASK-517 の 2 列とタブ）の中での結線だけを固定する。
//
// 確かめること（CSP あり・なしの両方。1 件ごとに新しい context を開く。User-Agent は `openPage` の `userAgent` で与える）:
//   (A) 幅 1280px: 案内（key-hint）と選ぶ欄（os-select）が shell-right の中にあり shell-left・shell-top の中に無い。
//       UA ごとの文言が合い、選ぶ欄で切り替えると文言が変わる。shell-right・案内・選ぶ欄は同じノードのまま、
//       Step の本文（step-text）の文字は切り替えの前後で変わらない（OS の表示が教材の文面を書き換えない）
//   (B) 幅 360px のタブ: 既定の「操作」で案内が見え、「成果物」では隠れ（件数は 1 のまま）、戻すと同じノード・同じ文言で見える。
//       「操作」で選び直した値と文言は、タブを往復しても保たれる（推定に戻らない）
//   (C) 幅 1280px: Enter 送信・Shift+Enter 改行・Tab 補完・Esc 停止の 4 操作の観察が、macOS・Windows・Linux の UA と
//       「macOS の UA から選ぶ欄で windows に切り替えた後」の 4 条件で互いに一致し、条件ごとの期待値にも合う
//
// 確かめないこと:
//   - 推定が外れたときに受講者が選ぶ欄で直せるかの人の確認（TASK-393・TASK-460）と、Windows の実機・人の目での
//     確認（TASK-20・TASK-286・TASK-395）。ここは User-Agent の差し替えによる模擬で、どちらも未実施
//   - ヒントの画面（SHELL-14。未描画）・UA-CH による推定・操作プロファイルの説明（TASK-392）
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-modifier-key-right-pane/';
const HINT = '[data-testid="key-hint"]';
const SELECT = '[data-testid="os-select"]';
const RIGHT = '[data-testid="shell-right"]';
const STEP_TEXT = '[data-testid="step-text"]';
const DRAFT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const STATUS = '[data-testid="run-status"]';
const LIST = '[data-testid="candidate-list"]';
const BTNS = '[data-testid="candidate-btn"]';
const LOG = '[data-testid="part-log-list"]';
const TAB_OPS = '#pane-tab-ops';
const TAB_ART = '#pane-tab-artifacts';
const PROMPT = 'OS を選ぶと、貼り付けのキーが表示されます';

const UA = {
  windows: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  mac: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  linux: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
};
const TEXT = { windows: '貼り付け: Ctrl+V', mac: '貼り付け: Cmd+V', linux: PROMPT };
const VALUE = { windows: 'windows', mac: 'mac', linux: '' };
// 選び直す先（推定と違う OS）。
const SWITCH = { windows: 'mac', mac: 'windows', linux: 'windows' };

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function waitUntil(what, read, pred, timeout = 8000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await read();
    if (await pred(last)) return last;
    await new Promise((r) => setTimeout(r, 20));
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

async function open(srv, os, width) {
  const opened = await openPage(browser, { origin: srv.origin, userAgent: UA[os], viewport: { width, height: 900 } });
  await opened.page.emulateMedia({ reducedMotion: 'no-preference' });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

async function finish(page, rec, context) {
  const counts = await collectSafety(page, rec);
  assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  await context.close();
}

const hintText = (page) => page.locator(HINT).textContent();
const waitHint = (page, text) => page.waitForFunction(([h, t]) => document.querySelector(h)?.textContent === t, [HINT, text]);

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  // (A)
  for (const os of ['windows', 'mac', 'linux']) {
    test(`(A) ${os} の UA: 案内と選ぶ欄は右のエリアの中にあり、切り替えても同じノードで本文は変わらない（CSP ${label}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv, os, 1280);
        const place = await page.evaluate(([h, s, r]) => {
          const at = (sel) => {
            const els = document.querySelectorAll(sel);
            const e = els[0];
            return {
              count: els.length,
              inRight: !!e?.closest(r),
              inLeft: !!e?.closest('[data-testid="shell-left"]'),
              inTop: !!e?.closest('[data-testid="shell-top"]'),
            };
          };
          return { hint: at(h), select: at(s) };
        }, [HINT, SELECT, RIGHT]);
        assert.deepEqual(place, {
          hint: { count: 1, inRight: true, inLeft: false, inTop: false },
          select: { count: 1, inRight: true, inLeft: false, inTop: false },
        });
        assert.equal(await page.locator(HINT).isVisible(), true);
        assert.equal(await page.locator(SELECT).isVisible(), true);
        assert.equal(await hintText(page), TEXT[os]);
        assert.equal(await page.locator(SELECT).inputValue(), VALUE[os]);
        if (os === 'linux') {
          const t = await hintText(page);
          assert.ok(!t.includes('Cmd') && !t.includes('Ctrl'), t);
        }
        const body0 = await page.locator(STEP_TEXT).textContent();
        await page.evaluate(([h, s, r]) => {
          window.__keep = { right: document.querySelector(r), hint: document.querySelector(h), select: document.querySelector(s) };
        }, [HINT, SELECT, RIGHT]);
        const to = SWITCH[os];
        await page.locator(SELECT).selectOption(to);
        await waitHint(page, TEXT[to]);
        assert.equal(await page.locator(SELECT).inputValue(), to);
        const after0 = await page.evaluate(([h, s, r]) => ({
          right: window.__keep.right.isSameNode(document.querySelector(r)),
          hint: window.__keep.hint.isSameNode(document.querySelector(h)),
          select: window.__keep.select.isSameNode(document.querySelector(s)),
          hintInRight: !!document.querySelector(h)?.closest(r),
          selectInRight: !!document.querySelector(s)?.closest(r),
        }), [HINT, SELECT, RIGHT]);
        assert.deepEqual(after0, { right: true, hint: true, select: true, hintInRight: true, selectInRight: true });
        assert.equal(await page.locator(STEP_TEXT).textContent(), body0, 'OS の表示が Step の本文を書き換えた');
        await finish(page, rec, context);
      });
    });
  }

  // (B)
  for (const os of ['mac', 'windows']) {
    test(`(B) ${os} の UA・幅 360px: タブの切り替えで案内は隠れて戻り、選び直した値はタブの往復で保たれる（CSP ${label}）`, async () => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv, os, 360);
        assert.equal(await page.locator(HINT).isVisible(), true, '既定の「操作」で案内が見える');
        assert.equal(await hintText(page), TEXT[os]);
        await page.evaluate(([h, s]) => {
          window.__keep = { hint: document.querySelector(h), select: document.querySelector(s) };
        }, [HINT, SELECT]);
        await page.click(TAB_ART);
        assert.equal(await page.locator(HINT).count(), 1);
        assert.equal(await page.locator(SELECT).count(), 1);
        assert.equal(await page.locator(HINT).isVisible(), false);
        assert.equal(await page.locator(SELECT).isVisible(), false);
        await page.click(TAB_OPS);
        assert.equal(await page.locator(HINT).isVisible(), true);
        assert.equal(await hintText(page), TEXT[os]);
        const to = SWITCH[os];
        await page.locator(SELECT).selectOption(to);
        await waitHint(page, TEXT[to]);
        await page.click(TAB_ART);
        await page.click(TAB_OPS);
        assert.equal(await page.locator(SELECT).inputValue(), to);
        assert.equal(await hintText(page), TEXT[to], 'タブの往復で推定に戻った');
        const same = await page.evaluate(([h, s]) => ({
          hint: window.__keep.hint.isSameNode(document.querySelector(h)),
          select: window.__keep.select.isSameNode(document.querySelector(s)),
        }), [HINT, SELECT]);
        assert.deepEqual(same, { hint: true, select: true });
        await finish(page, rec, context);
      });
    });
  }

  // (C)
  test(`(C) 4 つの操作の観察は OS の推定・選び直しによらず同じ（CSP ${label}）`, async () => {
    await run(async (srv) => {
      const conditions = [
        { name: 'mac', os: 'mac' },
        { name: 'windows', os: 'windows' },
        { name: 'linux', os: 'linux' },
        { name: 'mac→windows', os: 'mac', pick: 'windows' },
      ];
      const results = [];
      for (const c of conditions) {
        const { page, context, rec } = await open(srv, c.os, 1280);
        if (c.pick) {
          await page.locator(SELECT).selectOption(c.pick);
          await waitHint(page, TEXT[c.pick]);
        } else {
          assert.equal(await hintText(page), TEXT[c.os], `${c.name}: UA の差し替えが効いていない`);
        }
        const engine = async () => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
        const rows = () => page.locator(`${LOG} li`).evaluateAll((els) => els.map((e) => e.getAttribute('data-kind')));
        const snap = async () => {
          const e = await engine();
          return {
            current: e.current,
            generation: e.generation,
            runStatus: e.runStatus,
            kinds: await rows(),
            draft: await page.inputValue(DRAFT),
            listHidden: await page.locator(LIST).evaluate((el) => el.hasAttribute('hidden')),
            status: await page.locator(STATUS).getAttribute('data-state'),
          };
        };
        const obs = {};
        const base = await snap();
        // Enter 送信（試験の教材では最初の自由入力は拒否になり、行が増える。下書きの扱いは拒否の規則に従う）
        await page.focus(DRAFT);
        await page.keyboard.type('hello');
        await page.keyboard.press('Enter');
        await waitUntil('submit', snap, (s) => s.kinds.length > base.kinds.length);
        obs.enter = await snap();
        // Shift+Enter
        await page.keyboard.type('a');
        await page.keyboard.press('Shift+Enter');
        await page.keyboard.type('b');
        obs.shift = await snap();
        assert.equal(obs.shift.draft, 'a\nb');
        assert.deepEqual(obs.shift.kinds, obs.enter.kinds, `${c.name}: Shift+Enter で行が増えた`);
        await page.fill(DRAFT, '');
        // Tab 補完
        await page.fill(DRAFT, '/he');
        await waitUntil('candidates', () => page.locator(BTNS).allTextContents(), (l) => l.length === 1 && l[0] === '/help');
        await page.focus(DRAFT);
        await page.keyboard.press('Tab');
        await waitUntil('completed', () => page.inputValue(DRAFT), (v) => v === '/help');
        obs.tab = { draft: await page.inputValue(DRAFT), kinds: await rows() };
        assert.equal(obs.tab.draft, '/help');
        // Esc 停止
        await page.fill(DRAFT, '/make-note first-note');
        const before0 = await engine();
        await page.click(SUBMIT);
        await waitUntil('busy', () => page.locator(STATUS).getAttribute('data-state'), (s) => s === 'busy');
        await page.focus(DRAFT);
        await page.keyboard.press('Escape');
        await waitUntil('stopped', () => page.locator(STATUS).getAttribute('data-state'), (s) => s === 'stopped');
        const e = await engine();
        assert.equal(e.runStatus, 'stopped');
        assert.equal(e.generation, before0.generation + 1);
        const kinds = await rows();
        assert.equal(kinds.filter((k) => k === 'stopped').length, 1);
        obs.esc = {
          runStatus: e.runStatus,
          generationDelta: e.generation - before0.generation,
          stopped: kinds.filter((k) => k === 'stopped').length,
          word: await page.locator(STATUS).textContent(),
        };
        results.push({ name: c.name, obs });
        await finish(page, rec, context);
      }
      for (const r of results.slice(1)) {
        assert.deepEqual(r.obs, results[0].obs, `${r.name} の観察が ${results[0].name} と違う`);
      }
    });
  });
}
