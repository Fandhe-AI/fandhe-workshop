// 左の成果物ビュー共通の検査器（artifact-view-check.mjs。TASK-471 2/3）の自己確認。
//
// 検査器が「14 通りを流せる枠」として実際に動くことを、マウントする代役の部品（試験の設定の口 viewFixture*）で示す。
// - T1: 期待値を数える純関数が、JSON を目で数えた表と一致する（空の期待で通らない）
// - T2: 契約が許す 12 通りが、すべての検査を通る。外部リクエスト・ページのエラー・ダイアログも全区分 0
// - T3: 契約が許さない 2 通り（文書 (a)・差分 (a)）は、拒否され、状態も画面も変わらず、黙って退避しない
// - T4: マウントしない空の画面では、検査器が違反を出す（空振りで「通過」しない最小の確認）
// 負の対照 3 種（印を消す・部品を項目の外に置く・innerHTML）は artifact-view-controls.test.mjs（TASK-471 3/3）が持つ。
// 代役の部品は #app の外の別の root にマウントし、本体の画面とは状態を共有しない。
// 合格は検査器の枠が動くことの証拠で、各ビューの合格ではない（ビューごとの証拠は TASK-473 以降）。
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
import {
  CASES, DOC_EXPECTED, expectedFromRows, loadSampleRows, loadEscapeRows, runViewCase, runRejectedCase,
} from './artifact-view-check.mjs';

const BASE = '/fandhe-artifact-view-check/';
const ROOT = '#view-fixture-root';
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

// ページを開き、#app の外へ root を作って代役の部品をマウントする（mount: false なら作るだけで流さない）。
async function withFixturePage(fn, { mount = true } = {}) {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal(await page.evaluate(() => typeof window.__fandheTest?.viewFixtureMount), 'function', 'test hooks are missing');
      await page.evaluate(() => {
        const box = document.createElement('div');
        box.id = 'view-fixture-root';
        document.body.append(box);
      });
      if (mount) {
        const res = JSON.parse(await page.evaluate(() => window.__fandheTest.viewFixtureMount('view-fixture-root')));
        assert.deepEqual(res, { ok: true });
      }
      await fn(page, rec);
    } finally {
      await context.close();
    }
  });
}

// 代役の部品のアダプタ。action は Runtime::dispatch_action の口へ届け、applied だけを成功と数える。
function fixtureAdapter(page) {
  const dispatch = async (name, payload) => {
    const r = JSON.parse(await page.evaluate(([n, p]) => window.__fandheTest.viewFixtureDispatch(n, p), [name, payload]));
    return { ok: r.outcome === 'applied' };
  };
  return {
    select: (kind, variant) => dispatch('fixture_select', `${kind}:${variant}`),
    phase: (name) => dispatch('fixture_phase', name),
    state: () => page.evaluate(() => window.__fandheTest.viewFixtureState()),
    keyOf: (id) => `k${id}`,
  };
}

// JSON を目で数えた期待（前・後・追加・削除・変更・未変更）。
const SAMPLE_COUNTS = {
  file_tree: [10, 11, 1, 1, 2, 7],
  diff: [7, 9, 2, 1, 3, 3],
  task_tree: [5, 6, 1, 1, 3, 1],
  card_list: [3, 4, 1, 1, 2, 0],
  check_list: [5, 6, 1, 1, 3, 1],
  app_demo: [8, 9, 1, 0, 3, 5],
};

test('T1: 期待値の純関数は、JSON を目で数えた表と一致する', () => {
  for (const [kind, [b, a, add, rem, chg, same]] of Object.entries(SAMPLE_COUNTS)) {
    const e = expectedFromRows(loadSampleRows(kind));
    assert.deepEqual(e.counts, { before: b, after: a, add, remove: rem, change: chg }, kind);
    assert.equal(e.unchangedIds.length, same, kind);
    assert.equal(e.beforeIds.length + e.addedIds.length, e.afterIds.length, kind);
    assert.equal(new Set(e.afterIds).size, e.afterIds.length, `${kind}: ids must be unique`);
  }
  assert.equal(DOC_EXPECTED.unchanged + DOC_EXPECTED.remove, DOC_EXPECTED.before);
  assert.equal(DOC_EXPECTED.unchanged + DOC_EXPECTED.add + DOC_EXPECTED.remove, DOC_EXPECTED.after);
});

test('T1: 14 通りのうち、許されるのは 12 通りで、拒否は文書 (a)・差分 (a) の 2 通り', () => {
  assert.equal(CASES.length, 14);
  assert.equal(CASES.filter((c) => c.allowed).length, 12);
  assert.deepEqual(
    CASES.filter((c) => !c.allowed).map((c) => `${c.kind}/${c.variant}`),
    ['document/a', 'diff/a'],
  );
});

test('T1: escape の行のローダは view の名前だけ差し替えて、行を運ぶ', () => {
  const e = loadEscapeRows('card_list');
  assert.equal(e.view, 'card_list');
  assert.ok(e.rows.length > 0);
  assert.throws(() => loadEscapeRows('nope'));
});

for (const { kind, variant } of CASES.filter((c) => c.allowed)) {
  test(`T2: ${kind} の変種 (${variant}) は、前後 2 往復ですべての検査を通る`, async () => {
    await withFixturePage(async (page, rec) => {
      const { violations, stats } = await runViewCase(page, fixtureAdapter(page), { kind, variant, rootSelector: ROOT });
      const counts = await collectSafety(page, rec);
      const detail = JSON.stringify({ violations, stats, counts, rec });
      assert.deepEqual(violations, [], detail);
      // 空振りでないこと: 5 つの状態を見て、項目を実際に数えている。
      assert.equal(stats.states, 5, detail);
      const [b, a] = kind === 'document' ? [DOC_EXPECTED.before, DOC_EXPECTED.after] : SAMPLE_COUNTS[kind];
      assert.deepEqual(stats.itemCounts, [b, a, b, a, b], detail);
      assert.deepEqual(stats.preserved, [b, b, b, b], detail);
      assert.deepEqual(nonZero(counts), [], detail);
    });
  });
}

for (const { kind, variant } of CASES.filter((c) => !c.allowed)) {
  test(`T3: ${kind} の変種 (${variant}) は拒否され、状態も画面も変わらず、黙って退避しない`, async () => {
    await withFixturePage(async (page, rec) => {
      const { violations } = await runRejectedCase(page, fixtureAdapter(page), { kind, variant, rootSelector: ROOT });
      const counts = await collectSafety(page, rec);
      const detail = JSON.stringify({ violations, counts, rec });
      assert.deepEqual(violations, [], detail);
      assert.deepEqual(nonZero(counts), [], detail);
    });
  });
}

test('T3: 許される組を拒否の検査へ渡すと、取り違えとして違反になる', async () => {
  await withFixturePage(async (page) => {
    const { violations } = await runRejectedCase(page, fixtureAdapter(page), { kind: 'card_list', variant: 'a', rootSelector: ROOT });
    assert.equal(violations.length, 1, JSON.stringify(violations));
  });
});

test('T3: ラベルだけ書き換えて拒否する実装は、画面の変化として検出される', async () => {
  await withFixturePage(async (page) => {
    const base = fixtureAdapter(page);
    const adapter = {
      ...base,
      select: async (kind, variant) => {
        const r = await base.select(kind, variant);
        if (kind === 'document') {
          await page.evaluate((sel) => {
            const el = [...document.querySelector(sel).querySelectorAll('*')].find((e) => e.children.length === 0 && e.textContent.trim() !== '');
            el.textContent = `${el.textContent}!`;
          }, ROOT);
          return { ok: false };
        }
        return r;
      },
    };
    const { violations } = await runRejectedCase(page, adapter, { kind: 'document', variant: 'a', rootSelector: ROOT });
    assert.ok(violations.some((m) => m.includes('state or screen changed')), JSON.stringify(violations));
  });
});

test('T4: マウントしない空の画面では、検査器は違反を出す（空振りで通らない）', async () => {
  await withFixturePage(async (page) => {
    // select・phase は成功を返すが、画面には何も描かれていない。
    const adapter = { select: async () => ({ ok: true }), phase: async () => ({ ok: true }), keyOf: (id) => `k${id}` };
    const { violations, stats } = await runViewCase(page, adapter, { kind: 'file_tree', variant: 'b', rootSelector: ROOT });
    assert.ok(violations.length > 0, 'an empty screen must not pass');
    assert.ok(violations.some((m) => m.includes('expected exactly 7 view lists')), JSON.stringify(violations));
    assert.equal(stats.states, 5);
  }, { mount: false });
});

test('T4: 変種を無視して常に (b) を描く実装は、(a) のケースで印の宿主の位置の違反になる', async () => {
  await withFixturePage(async (page) => {
    const base = fixtureAdapter(page);
    const adapter = { ...base, select: (kind, variant) => base.select(kind, 'b') };
    const { violations } = await runViewCase(page, adapter, { kind: 'card_list', variant: 'a', rootSelector: ROOT });
    assert.ok(violations.some((m) => m.includes('misplaced for the variant')), JSON.stringify(violations));
  });
});

test('T4: key の期待が合わないアダプタでは、検査器は key の違反を出す', async () => {
  await withFixturePage(async (page) => {
    const adapter = { ...fixtureAdapter(page), keyOf: (id) => `x${id}` };
    const { violations } = await runViewCase(page, adapter, { kind: 'file_tree', variant: 'b', rootSelector: ROOT });
    assert.ok(violations.some((m) => m.includes('item keys differ from the expected keys')), JSON.stringify(violations));
  });
});

test('T4: 存在しない root でも、検査器は違反を出す', async () => {
  await withFixturePage(async (page) => {
    const adapter = fixtureAdapter(page);
    const { violations } = await runViewCase(page, adapter, { kind: 'file_tree', variant: 'b', rootSelector: '#no-such-root' });
    assert.ok(violations.some((m) => m.includes('root element not found')), JSON.stringify(violations));
  });
});
