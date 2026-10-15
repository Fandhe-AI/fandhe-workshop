// 本文リンクの許可スキーム（SAFE-9。TASK-149 1/2）を、実際にビルドした画面で確かめる試験。
//
// 変換器を通さずに手作りした木 16 件（T-01〜T-16。試験データは spec の写し）を、読み込みの関門に通し、
// 描画した HTML を #app の外の容器へ innerHTML で入れる（本体のマウントと同じ「render の文字列 → innerHTML」の経路）。
// そのうえで、安全性の計数（実行 0・通信 0・危険な要素 0）と、許可外のリンクが a にならないことを確かめる。
// 1 件ごとに新しい context を使い、記録が件どうしで混ざらないようにする。
//
// 主張の範囲: T-10（木の版）・T-11/12/15/16（深さ）・T-09（言語名の class）は TASK-150 の担当なので、
// 計数 0 と「許可外の a[href] が無い」ことだけを主張し、止まる段は記録するだけで固定しない。
// 画像（T-03・T-04）は 2/2（#143）が置き換える既定のうえで計数 0 だけを主張する。
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

const BASE = '/fandhe-doc-links/';
const DATA = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../testdata/untrusted-input-safety');
// 許可外のリンクが、a にならず文字だけが残るべき件。
const LINK_CASES = ['T-01', 'T-02', 'T-13', 'T-14'];
let browser;

// 引用の入れ子 n 段の木を文字列で作る（試験データの gen.kind == "quote_nest" と同じ形）。
function quoteNest(n) {
  return '{"version":1,"blocks":[' + '{"t":"quote","children":['.repeat(n) + ']}'.repeat(n) + ']}';
}

function loadCases() {
  const out = [];
  for (const rel of ['test-data-addendum/tree-cases.json', 'test-data-addendum-2/tree-cases-2.json']) {
    const v = JSON.parse(fs.readFileSync(path.join(DATA, rel), 'utf8'));
    for (const c of v.cases) {
      if (c.tree) out.push({ id: c.id, json: JSON.stringify(c.tree) });
      else {
        assert.equal(c.gen.kind, 'quote_nest', c.id);
        out.push({ id: c.id, json: quoteNest(c.gen.n) });
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

// 木を関門に通して描画し、#app の外の容器へ入れ、計数と容器の中身を返す。
async function probe(srv, json) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.renderDoc), 'function', 'test hooks are missing');
    const gate = JSON.parse(await page.evaluate((t) => window.__fandheTest.checkDoc(t), json));
    // 固定の関数を渡し、HTML は引数で渡す（試験データから実行コードを組み立てない）。
    const info = await page.evaluate((t) => {
      const html = window.__fandheTest.renderDoc(t);
      const box = document.createElement('div');
      box.setAttribute('data-testid', 'doc-probe');
      document.body.append(box);
      box.innerHTML = html;
      return {
        htmlLength: html.length,
        anchors: [...box.querySelectorAll('a[href]')].map((a) => a.getAttribute('href')),
        text: box.textContent,
      };
    }, json);
    const counts = await collectSafety(page, rec);
    return { gate, info, counts, rec };
  } finally {
    await context.close();
  }
}

test('手作りの木 16 件で、実行 0・通信 0・危険な要素 0 になり、許可外の a[href] が出ない', async () => {
  const cases = loadCases();
  assert.equal(cases.length, 16);
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    for (const { id, json } of cases) {
      const { gate, info, counts, rec } = await probe(srv, json);
      const detail = JSON.stringify({ id, gate, info, counts, rec });
      assert.deepEqual(nonZero(counts), [], detail);
      for (const href of info.anchors) {
        assert.match(href, /^(https?:\/\/|[^:]*$)/i, detail);
        assert.ok(!href.startsWith('//') && !href.startsWith('\\'), detail);
      }
      if (LINK_CASES.includes(id)) {
        assert.equal(gate.ok, true, detail);
        assert.deepEqual(info.anchors, [], detail);
        assert.ok(info.text.includes('x'), detail);
      }
      if (['T-05', 'T-06', 'T-07'].includes(id)) {
        assert.deepEqual(gate, { ok: false, stage: 'type_and_content' }, detail);
      }
    }
  });
});

test('対照: 無害な木と相対リンクの木が、空振りでなく描画される', async () => {
  const plain = '{"version":1,"blocks":[{"t":"paragraph","children":[{"t":"text","text":"hello"}]}]}';
  const rel = '{"version":1,"blocks":[{"t":"paragraph","children":[{"t":"link","href":"./guide.md#a","children":[{"t":"text","text":"x"}]}]}]}';
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const a = await probe(srv, plain);
    assert.deepEqual(nonZero(a.counts), [], JSON.stringify(a));
    assert.ok(a.info.text.includes('hello'), JSON.stringify(a));
    const b = await probe(srv, rel);
    assert.deepEqual(nonZero(b.counts), [], JSON.stringify(b));
    assert.deepEqual(b.info.anchors, ['./guide.md#a'], JSON.stringify(b));
  });
});
