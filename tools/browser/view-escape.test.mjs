// 未信頼の値の表示を確かめる試験の骨格と負の対照（ART-6。TASK-482 1/2）。
//
// 役割: 世界状態の値に HTML・スクリプト・javascript: のリンク・属性の閉じ忘れの形を入れた行（escape の行）を、部品に直接渡して
// 表示させ、値が文字として出て、危険な要素・属性と実行が 0 件であることを数える検査器（artifact-view-check.mjs の
// runEscapeCase）の枠が、CSP あり・なしの両方で動くことと、差を検出できること（負の対照）を示す。
// 代表の組み合わせはカードの (a)・(b) の前後（4 通り）。24 通りすべての合格と記録は TASK-482 2/2 の担当。
// 教材パック・IR からの注入は TASK-145 の担当で、ここでは扱わない。
//
// - E1: 代表の組み合わせが、CSP あり・なしの両方で違反 0・計数 0（期待の件数は escape の行から独立に数えた値）
// - E2: 行を escape に切り替えなければ、検査器が口の空振りを違反として検出する
// - N1: 同じ形の文字列は、不活性の文書で innerHTML に入れると危険な要素として 3 件数えられ、textContent なら 0
// - N2: 表示した文字を innerHTML に入れ直すと検査器が検出する。CSP なしでは実行が、CSP ありでは CSP 違反が数えられる
// - N3: 計数器の自己確認（alert・外部への fetch が数えられる）
// 呼び出し元は make test-browser（scripts/test-browser.sh）。docs/testing.md の 5.288。1 件ごとに新しい context を使う。
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
// 通信・実行の副作用は N2・N3 の意図した形だけで、外部の宛先は example.invalid だけを使う。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages,
} from './harness.mjs';
import { DANGER_SELECTOR, ESCAPE_HIT, installEscapeTrap, runEscapeCase } from './artifact-view-check.mjs';

const BASE = '/fandhe-view-escape/';
const ROOT = '#view-fixture-root';
let browser;

// docs/spec の PoC-8 の負の対照の文字列（harness/test.mjs）。img・iframe・javascript: の a を 1 件ずつ数える。
const POC8_CONTROL =
  '<img src=x onerror="window.__escape_hit=1"><iframe src="about:blank"></iframe><a href="javascript:void(0)">x</a>';

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

async function withEscapePage(csp, fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await installEscapeTrap(page);
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal(await page.evaluate(() => typeof window.__fandheTest?.viewFixtureMount), 'function', 'test hooks are missing');
      await page.evaluate(() => {
        const box = document.createElement('div');
        box.id = 'view-fixture-root';
        document.body.append(box);
      });
      const res = JSON.parse(await page.evaluate(() => window.__fandheTest.viewFixtureMount('view-fixture-root')));
      assert.deepEqual(res, { ok: true });
      await fn(page, rec);
    } finally {
      await context.close();
    }
  }));
}

function fixtureAdapter(page) {
  const dispatch = async (name, payload) => {
    const r = JSON.parse(await page.evaluate(([n, p]) => window.__fandheTest.viewFixtureDispatch(n, p), [name, payload]));
    return { ok: r.outcome === 'applied' };
  };
  return {
    // 差分の (a) は契約が拒否するため、対照の口へ送る。
    select: (kind, variant) => (kind === 'diff' && variant === 'a'
      ? dispatch('fixture_contrast', 'diff:a')
      : dispatch('fixture_select', `${kind}:${variant}`)),
    data: (payload) => dispatch('fixture_data', payload),
    phase: (name) => dispatch('fixture_phase', name),
    keyOf: (id) => `k${id}`,
  };
}

// 非同期に発火する記録（img の onerror・CSP 違反）が Node 側へ届くのを、上限つきで待つ。
async function waitFor(cond, ms = 4000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (cond()) return true;
    await new Promise((r) => setTimeout(r, 50));
  }
  return cond();
}

// --- E1: 代表の組み合わせ（CSP あり・なし x カードの (a)・(b)）---

for (const csp of [true, false]) {
  for (const variant of ['a', 'b']) {
    test(`E1: CSP ${csp ? 'あり' : 'なし'} の card_list (${variant}) で escape の行が文字として出て、違反も計数も 0`, async () => {
      await withEscapePage(csp, async (page, rec) => {
        const { violations, stats } = await runEscapeCase(page, fixtureAdapter(page), { kind: 'card_list', variant, rootSelector: ROOT });
        const detail = JSON.stringify({ violations, stats });
        assert.deepEqual(violations, [], detail);
        assert.deepEqual(stats.itemCounts, [3, 4, 3], detail);
        assert.deepEqual(nonZero(await collectSafety(page, rec)), [], detail);
        assert.deepEqual(cspConsoleMessages(rec), []);
        assert.deepEqual(rec.hits, []);
        assert.equal(await page.evaluate((n) => window[n], ESCAPE_HIT), undefined);
      });
    });
  }
}

test('E2 対: 行を escape に切り替えなければ、検査器が空振りを違反として検出する', async () => {
  await withEscapePage(true, async (page) => {
    const { violations } = await runEscapeCase(page, fixtureAdapter(page), {
      kind: 'card_list', variant: 'b', rootSelector: ROOT, skipData: true,
    });
    const detail = JSON.stringify(violations);
    assert.ok(violations.some((m) => m.includes('item count or keys differ from the escape rows')), detail);
    assert.ok(violations.some((m) => m.includes('a value is not shown as literal text')), detail);
  });
});

test('E2: 不正な payload（文書・大文字違い）は拒否される', async () => {
  await withEscapePage(true, async (page) => {
    const adapter = fixtureAdapter(page);
    for (const p of ['document:escape', 'cards:Escape', 'cards:escape ']) {
      assert.equal((await adapter.data(p)).ok, false, p);
    }
    assert.equal((await adapter.data('cards:escape')).ok, true);
  });
});

// --- N1: 同じ形の文字列を innerHTML に入れる（不活性の文書で数える）---

function countDangerInert([value, selector, hitName]) {
  const doc = document.implementation.createHTMLDocument('');
  const count = (assign) => {
    const box = doc.createElement('div');
    assign(box);
    return box.querySelectorAll(selector).length;
  };
  return {
    html: count((box) => { box.innerHTML = value; }),
    text: count((box) => { box.textContent = value; }),
    executed: window[hitName] !== undefined,
  };
}

test('N1: PoC-8 の対照の文字列は innerHTML でちょうど 3 件数えられ、textContent なら 0', async () => {
  await withEscapePage(true, async (page, rec) => {
    const r = await page.evaluate(countDangerInert, [POC8_CONTROL, DANGER_SELECTOR, ESCAPE_HIT]);
    assert.deepEqual(r, { html: 3, text: 0, executed: false }, JSON.stringify(r));
    assert.deepEqual(rec.external, []);
    assert.deepEqual(rec.hits, []);
  });
});

// --- N2: 表示した文字を innerHTML に入れ直す ---

// 後の状態で e1 の項目のラベルを、表示された文字そのもので innerHTML に入れ直す。値は引数で渡さず、表示済みの文字を使う。
function reinjectLabel(p) {
  return p.evaluate(() => {
    const el = document.querySelector('#view-fixture-root [data-key="ke1"] [data-testid="card-label"]');
    if (!el) throw new Error('label of e1 not found');
    el.innerHTML = el.textContent;
  });
}

for (const csp of [true, false]) {
  test(`N2: CSP ${csp ? 'あり' : 'なし'} で表示した文字を innerHTML に入れ直すと、検査器が検出し、${csp ? 'CSP 違反' : '実行'}が数えられる`, async () => {
    await withEscapePage(csp, async (page, rec) => {
      const { violations } = await runEscapeCase(page, fixtureAdapter(page), {
        kind: 'card_list', variant: 'b', rootSelector: ROOT, mutateAfter: reinjectLabel,
      });
      const detail = JSON.stringify(violations);
      assert.ok(violations.some((m) => m.includes('dangerous elements or attributes are present')), detail);
      assert.ok(violations.some((m) => m.includes('a value is not shown as literal text')), detail);
      if (csp) {
        assert.ok(await waitFor(() => rec.csp.length >= 1), 'no CSP violation was counted');
        assert.deepEqual(rec.hits, []);
      } else {
        // src=x は配信元への相対の要求で 404 になり、onerror が走る。
        assert.ok(await waitFor(() => rec.hits.includes('escape:3')), `no execution was counted: ${JSON.stringify(rec.hits)}`);
        assert.deepEqual(rec.csp, []);
      }
      assert.deepEqual(rec.external, []);
    });
  });
}

// --- N3: 計数器の自己確認 ---

for (const csp of [true, false]) {
  test(`N3: CSP ${csp ? 'あり' : 'なし'} の設定で alert と外部への fetch が数えられる`, async () => {
    await withEscapePage(csp, async (page, rec) => {
      await page.evaluate(() => { window.alert('x'); });
      await page.evaluate(() => fetch('https://example.invalid/').catch(() => {}));
      if (csp) assert.ok(await waitFor(() => rec.csp.length >= 1), 'no CSP violation was counted');
      else assert.ok(await waitFor(() => rec.external.length === 1), JSON.stringify(rec.external));
      assert.equal(rec.dialogs.length, 1);
      assert.equal(rec.external.length, csp ? 0 : 1);
    });
  });
}
