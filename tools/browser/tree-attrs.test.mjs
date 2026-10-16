// ツリー（ファイルツリー・タスクツリー）の変種 (b) の木の属性と、全項目が開いた表示（TASK-472。ART-9・ART-13）を、
// 実際にビルドした画面で確かめる試験。
//
// 試験用の口（renderTree）が返す HTML 文字列を #app の外の容器へ入れ、次を数える。
// - 親が role=tree で 1 個、全項目が role=treeitem で、期待の数と一致する
// - 全 treeitem が aria-level・aria-posinset・aria-setsize を持ち、値が JS 側に固定で書いた表と一致する
// - 全項目が見える（checkVisibility・高さ・hidden の祖先・aria-expanded="false" がいずれも 0 件）
// - 対照: 変種 (a) には role・aria-level 等が付かず、項目は同じ数だけ見える
// 読み上げは対象外（A11Y-14・TASK-381 の人の確認で扱う）。期待値は Rust 側の試料から導かず、ここに固定で書く。
// 1 件ごとに新しい context を使い、記録が件どうしで混ざらないようにする。
//
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。
// 未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-tree-attrs/';
let browser;

// 後の状態で出る行の [level, posinset, setsize] と、aria-expanded="true" を持つ行の添字。
const EXPECTED = {
  file_tree: {
    positions: [[1, 1, 3], [2, 1, 2], [2, 2, 2], [3, 1, 1], [1, 2, 3], [1, 3, 3]],
    expanded: [0, 2],
  },
  task_tree: {
    positions: [[1, 1, 2], [2, 1, 2], [2, 2, 2], [1, 2, 2], [2, 1, 1]],
    expanded: [0, 3],
  },
};

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

// ページを開き、描画した HTML を #app の外の容器へ入れて集計する。固定の関数に引数で渡す。
async function inspect(srv, kind, variant) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.renderTree), 'function', 'test hooks are missing');
    const result = await page.evaluate(([k, v]) => {
      const html = window.__fandheTest.renderTree(k, v);
      const box = document.createElement('div');
      box.setAttribute('data-test-box', '');
      document.body.append(box);
      box.innerHTML = html;
      const items = [...box.querySelectorAll('li')];
      const num = (e, n) => Number(e.getAttribute(n));
      return {
        htmlLength: html.length,
        trees: box.querySelectorAll('[role="tree"]').length,
        treeitems: box.querySelectorAll('[role="treeitem"]').length,
        items: items.length,
        levelAttrs: box.querySelectorAll('[aria-level]').length,
        positions: items.map((e) => [num(e, 'aria-level'), num(e, 'aria-posinset'), num(e, 'aria-setsize')]),
        expandedTrue: items.flatMap((e, i) => (e.getAttribute('aria-expanded') === 'true' ? [i] : [])),
        expandedFalse: box.querySelectorAll('[aria-expanded="false"]').length,
        rootIsTree: box.firstElementChild?.getAttribute('role') === 'tree',
        visible: items.filter((e) => e.checkVisibility() && e.getBoundingClientRect().height > 0 && !e.closest('[hidden]')).length,
        hiddenAttrs: box.querySelectorAll('[hidden]').length,
        onAttrs: [...box.querySelectorAll('*')].flatMap((e) => [...e.attributes].map((a) => a.name)).filter((n) => n.startsWith('on')).length,
      };
    }, [kind, variant]);
    assert.equal(await page.evaluate(() => 1 + 1), 2, 'page must still respond');
    const counts = await collectSafety(page, rec);
    return { result, counts, rec };
  } finally {
    await context.close();
  }
}

for (const kind of ['file_tree', 'task_tree']) {
  test(`${kind} の変種 (b): role=tree 1 個・全項目が treeitem・3 属性が期待どおり・全項目が見える`, async () => {
    const want = EXPECTED[kind];
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      const { result, counts, rec } = await inspect(srv, kind, 'b');
      const detail = JSON.stringify({ kind, result, counts, rec });
      assert.ok(result.htmlLength > 0, `renderTree returned an empty string: ${detail}`);
      assert.equal(result.trees, 1, detail);
      assert.equal(result.rootIsTree, true, detail);
      assert.equal(result.items, want.positions.length, detail);
      assert.equal(result.treeitems, want.positions.length, detail);
      assert.equal(result.levelAttrs, want.positions.length, detail);
      assert.deepEqual(result.positions, want.positions, detail);
      assert.deepEqual(result.expandedTrue, want.expanded, detail);
      assert.equal(result.expandedFalse, 0, detail);
      assert.equal(result.visible, want.positions.length, detail);
      assert.equal(result.hiddenAttrs, 0, detail);
      assert.equal(result.onAttrs, 0, detail);
      assert.deepEqual(nonZero(counts), [], detail);
    });
  });

  test(`対照: ${kind} の変種 (a) には role・aria-level が付かず、項目は同じ数だけ見える`, async () => {
    const want = EXPECTED[kind];
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      const { result, counts, rec } = await inspect(srv, kind, 'a');
      const detail = JSON.stringify({ kind, result, counts, rec });
      assert.equal(result.trees, 0, detail);
      assert.equal(result.treeitems, 0, detail);
      assert.equal(result.levelAttrs, 0, detail);
      assert.equal(result.items, want.positions.length, detail);
      assert.equal(result.visible, want.positions.length, detail);
      assert.deepEqual(nonZero(counts), [], detail);
    });
  });
}

test('範囲外の種類・変種は空文字列を返し、別の形に退避しない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      const out = await page.evaluate(() => [
        window.__fandheTest.renderTree('document', 'b'),
        window.__fandheTest.renderTree('file_tree', 'c'),
        window.__fandheTest.renderTree('', ''),
      ]);
      assert.deepEqual(out, ['', '', '']);
    } finally {
      await context.close();
    }
  });
});
