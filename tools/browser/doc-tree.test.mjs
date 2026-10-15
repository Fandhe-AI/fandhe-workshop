// 木の読み込みの検査と描画の深さの上限（SAFE-12。TASK-150 2/3）を、実際にビルドした画面で確かめる試験。
//
// - 関門（読み込みの経路）: 語彙の外の種別・未知の項目・木の版の違いが型・内容の段で、深い入れ子が深さの段で拒否される。
// - 描画の深さの上限: 関門は 50・60 段の木を描画まで通さない（JSON の深さ 64）ので、Rust 側で引用の入れ子を組み立てる
//   試験用の口（renderQuoteNest）で描画し、32 段で省略の文言になること、タブが落ちないことを確かめる。
// - コードの言語名（T-09）: class に安全な文字だけが入り、属性が注入されない。
// 1 件ごとに新しい context を使い、記録が件どうしで混ざらないようにする。試験データから実行コードを組み立てず、
// 固定の関数に引数で渡す。
//
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。
// 未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-doc-tree/';
const DATA = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../testdata/untrusted-input-safety');
const OMITTED = '（入れ子が深すぎるため省略）';
let browser;

// 引用の入れ子 n 段の木を文字列で作る（試験データの gen.kind == "quote_nest" と同じ形）。
function quoteNest(n) {
  return '{"version":1,"blocks":[' + '{"t":"quote","children":['.repeat(n) + ']}'.repeat(n) + ']}';
}

function loadCases() {
  const out = new Map();
  for (const rel of ['test-data-addendum/tree-cases.json', 'test-data-addendum-2/tree-cases-2.json']) {
    const v = JSON.parse(fs.readFileSync(path.join(DATA, rel), 'utf8'));
    for (const c of v.cases) {
      if (c.tree) out.set(c.id, JSON.stringify(c.tree));
      else {
        assert.equal(c.gen.kind, 'quote_nest', c.id);
        out.set(c.id, quoteNest(c.gen.n));
      }
    }
  }
  return out;
}

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

// ページを開き、fn(page) を実行する。そのあとページが応答することを確かめ、計数を添えて返す。
async function withPage(srv, fn) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.renderQuoteNest), 'function', 'test hooks are missing');
    const result = await fn(page);
    assert.equal(await page.evaluate(() => 1 + 1), 2, 'page must still respond');
    const counts = await collectSafety(page, rec);
    return { result, counts, rec };
  } finally {
    await context.close();
  }
}

// 描画した HTML 文字列を #app の外の容器へ入れ、要素の数などを返す（固定の関数。HTML は引数で渡す）。
function summarizeInPage(page, html) {
  return page.evaluate((h) => {
    const box = document.createElement('div');
    document.body.append(box);
    box.innerHTML = h;
    return {
      blockquotes: box.querySelectorAll('blockquote').length,
      omitted: (box.textContent.match(/（入れ子が深すぎるため省略）/g) ?? []).length,
      classes: [...box.querySelectorAll('[class]')].map((e) => e.getAttribute('class')),
      onAttrs: [...box.querySelectorAll('*')].flatMap((e) => [...e.attributes].map((a) => a.name)).filter((n) => n.startsWith('on')).length,
      codes: box.querySelectorAll('pre > code').length,
      text: box.textContent,
    };
  }, html);
}

const checkDoc = (page, json) => page.evaluate((t) => JSON.parse(window.__fandheTest.checkDoc(t)), json);

test('語彙の外の種別・未知の項目・木の版の違いは型・内容の段で拒否される', async () => {
  const cases = loadCases();
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    for (const id of ['T-05', 'T-06', 'T-07', 'T-10']) {
      const { result, counts } = await withPage(srv, (page) => checkDoc(page, cases.get(id)));
      assert.deepEqual(result, { ok: false, stage: 'type_and_content' }, id);
      assert.deepEqual(nonZero(counts), [], id);
    }
  });
});

test('深い入れ子（T-11/12/15/16）は深さの段で拒否され、タブは落ちない', async () => {
  const cases = loadCases();
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    for (const id of ['T-11', 'T-12', 'T-15', 'T-16']) {
      const { result, counts, rec } = await withPage(srv, (page) => checkDoc(page, cases.get(id)));
      assert.deepEqual(result, { ok: false, stage: 'depth' }, id);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ id, counts, rec }));
    }
  });
});

test('引用の入れ子 50・60 段は 32 段で省略になり、計数 0 でタブが落ちない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    for (const n of [50, 60]) {
      const { result, counts, rec } = await withPage(srv, async (page) => {
        const html = await page.evaluate((k) => window.__fandheTest.renderQuoteNest(k), n);
        return summarizeInPage(page, html);
      });
      const detail = JSON.stringify({ n, result, counts, rec });
      assert.equal(result.blockquotes, 32, detail);
      assert.equal(result.omitted, 1, detail);
      assert.deepEqual(nonZero(counts), [], detail);
    }
  });
});

test('コードの言語名: 属性を注入する入力で class に安全な文字だけが入る', async () => {
  const cases = loadCases();
  const rust = '{"version":1,"blocks":[{"t":"code","lang":"rust","text":"x"}]}';
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    for (const [id, json, wantClasses] of [['T-09', cases.get('T-09'), 0], ['対照 rust', rust, 1]]) {
      const { result, counts, rec } = await withPage(srv, async (page) => {
        const html = await page.evaluate((t) => window.__fandheTest.renderDoc(t), json);
        return summarizeInPage(page, html);
      });
      const detail = JSON.stringify({ id, result, counts, rec });
      assert.equal(result.codes, 1, detail);
      assert.equal(result.onAttrs, 0, detail);
      assert.equal(result.classes.length, wantClasses, detail);
      for (const c of result.classes) assert.match(c, /^language-[A-Za-z0-9_+.-]{1,32}$/, detail);
      assert.deepEqual(nonZero(counts), [], detail);
    }
  });
});

test('対照: 版 1 の無害な木は描画される（空振りでない）', async () => {
  const plain = '{"version":1,"blocks":[{"t":"paragraph","children":[{"t":"text","text":"hello"}]}]}';
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { result, counts } = await withPage(srv, (page) => page.evaluate((t) => window.__fandheTest.renderDoc(t), plain));
    assert.ok(result.includes('hello'), result);
    assert.ok(!result.includes(OMITTED), result);
    assert.deepEqual(nonZero(counts), []);
  });
});
