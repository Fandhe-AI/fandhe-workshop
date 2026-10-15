// 受講者の文字列を非解釈で表示する（SAFE-11。TASK-150 1/3）の試験。
//
// 入力欄から送信した文字列は、Markdown でも HTML でもなく、書いたとおりの文字で右の一覧に出る。
// 見出し・リンク・画像・コードブロックなどの記法が要素にならず、実行・外部通信・CSP 違反・危険な要素が 0 件であることを、
// 配信設定の画面で CSP あり・なしの両方について確かめる（CSP なしでも 0 件なら、止められたのではなく要素にならない）。
// 履歴の保存と復元は画面が未実装のため、native の試験（crates/web/tests/learner_text.rs）が代える。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1。再描画の経路）。どちらも未設定・不在は
// skip にせず失敗にする。入力は試験の中で組み立て、fixture のファイルは置かない。URL は example.invalid だけを使う。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, HIT_FN,
} from './harness.mjs';

const BASE = '/fandhe-learner-text/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const LIST = '[data-testid="log-list"]';
const ITEMS = `${LIST} > li`;
// 改行は LF だけにする（textarea は CRLF を LF に正規化するため）。
const CASES = [
  '# 見出し',
  '## 小見出し',
  '[リンク](https://example.invalid/SAFE-11-link)',
  '![画像](https://example.invalid/SAFE-11-img.png)',
  '<https://example.invalid/SAFE-11-auto>',
  `\`\`\`\n${HIT_FN}('SAFE-11-code')\n\`\`\``,
  '`inline`',
  '**強調** *斜体*',
  '> 引用',
  '- 一覧\n- 二',
  '| a | b |\n|---|---|\n| 1 | 2 |',
  '---',
  '<b>太字</b>',
  `<img src="https://example.invalid/SAFE-11-html" onerror="${HIT_FN}('SAFE-11-html')">`,
  '&lt;b&gt;',
  `# 混在\n[x](https://example.invalid/SAFE-11-mix) ![y](https://example.invalid/SAFE-11-mix.png)\n<script>${HIT_FN}('SAFE-11-script')</script>`,
];
// 一覧の中にあってはならない要素。
const FORBIDDEN = 'a, img, h1, h2, h3, h4, h5, h6, pre, code, strong, em, b, i, blockquote, ul, table, hr, script, iframe';

let browser;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 項目ごとの本文・子ノードの数と種類・禁止要素の件数を、Node 側へ返す。
async function inspect(page) {
  return page.$$eval(ITEMS, (lis, forbidden) => lis.map((li) => ({
    text: li.textContent,
    kinds: [...li.childNodes].map((n) => n.nodeType),
    forbidden: li.querySelectorAll(forbidden).length,
  })), FORBIDDEN);
}

function assertLiteral(items, detail) {
  assert.equal(items.length, CASES.length, detail);
  items.forEach((it, i) => {
    assert.equal(it.text, CASES[i], `${detail} #${i}`);
    assert.deepEqual(it.kinds, [3], `${detail} #${i}`); // 文字ノード 1 つだけ
    assert.equal(it.forbidden, 0, `${detail} #${i}`);
  });
  return items;
}

async function submitAll(page) {
  const handle = await page.$(INPUT);
  for (let i = 0; i < CASES.length; i += 1) {
    await page.fill(INPUT, CASES[i]);
    await page.click(SUBMIT);
    await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [ITEMS, i + 1]);
    // 一覧は差分で更新される。root の作り直しが起きれば入力欄の要素が入れ替わる。
    assert.equal(await page.evaluate(([sel, prev]) => document.querySelector(sel) === prev, [INPUT, handle]), true, `input replaced at #${i}`);
    // 送信後も入力欄は空にしない（空にするのは送信の結線のタスク）。
  }
  await handle.dispose();
}

for (const csp of [true, false]) {
  test(`受講者の文字列は文字のまま表示され、実行・通信・危険な要素が 0 件（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.goto(srv.url);
      await waitMounted(page);
      await submitAll(page);
      const detail = JSON.stringify(rec);
      assertLiteral(await inspect(page), detail);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
      assert.deepEqual(rec.hits, [], detail);
      await context.close();
    }));
  });
}

test('再描画（HTML の文字列からの作り直し）の後も文字のまま表示される', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    await submitAll(page);
    assert.equal(await page.evaluate(() => window.__fandheTest.rerender()), true);
    const detail = JSON.stringify(rec);
    assertLiteral(await inspect(page), detail);
    const counts = await collectSafety(page, rec);
    assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
    await context.close();
  });
});

test('対照: 数え方は、解釈された要素があれば 0 より大きく数える', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    // 切り離した要素に当てる（img に src は付けず、配信元以外へ取りに行かない）。
    const n = await page.evaluate((forbidden) => {
      const li = document.createElement('li');
      li.innerHTML = '<a href="#x">l</a><h1>h</h1><img>';
      return li.querySelectorAll(forbidden).length;
    }, FORBIDDEN);
    assert.equal(n, 3);
    await context.close();
  });
});
