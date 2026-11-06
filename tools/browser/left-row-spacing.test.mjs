// 左エリアのタスクツリーの行の余白とカードの補足の上の空きの試験（#2078。5.473）。
//
// 製品の画面（形つきの固定データ）と代役の部品（view_fixture）の矩形で、見え方を固定する。
// - RS1: 製品のタスクツリー（?fixture=artifact-redraw-shaped-task_tree-50）。幅 1280px・800px で、全行のラベル・補足・状態のバッジの間に空きがある
// - RS2: 製品のカード（?fixture=artifact-redraw-shaped-card_list-20）。幅 1280px・800px で、全行のバッジと補足の間に 1 行分の空きが無い
// - RS3: 代役のタスクツリー (b)（補足が空でない行）。全行の空きがある
// - RS4: 代役のカード (b)。全行で補足の上の空きが 1 行分より小さい
// - RS5: 規則を外した写しでは RS1〜RS4 と同じ測定が違反を数える（負の対照）。外さない写しでは 0 件（写しの手順の較正）
// 各場面で CSP の違反・ページ内エラー・配信元以外への通信が 0 件であることも確かめる。
// 閾値は font-size に対する相対値で、端末の差を受けない。選択子は閉じた testid と data-view の値だけ。
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
// 限界: 第 3 教材そのものを compile した画面では測っていない。基準端末では測っていない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-left-row-spacing/';
const ROOT = '#view-fixture-root';
const WIDTHS = [1280, 800];
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

// 固定の関数。scope 配下の kind の list の各行を測り、違反の文字列と測った行の数を返す。
function measure([scope, kind]) {
  const out = [];
  const list = document.querySelector(`${scope} [data-testid="view-list"][data-view="${kind}"]`);
  if (!list) return { out: ['no list'], rows: 0 };
  const rows = [...list.querySelectorAll(':scope > li')];
  for (const li of rows) {
    if (kind === 'task_tree') {
      const label = li.querySelector('[data-testid="task-label"]');
      const detail = li.querySelector('[data-testid="task-detail"]');
      const badge = li.querySelector('[data-testid="state-badge"]');
      if (!label || !badge) { out.push('missing label or badge'); continue; }
      const fs = parseFloat(getComputedStyle(label).fontSize);
      const l = label.getBoundingClientRect();
      const b = badge.getBoundingClientRect();
      if (l.width === 0 || b.width === 0) { out.push('zero width'); continue; }
      const seq = [l];
      if (detail) {
        const d = detail.getBoundingClientRect();
        if (d.width > 0) seq.push(d);
      }
      seq.push(b);
      for (let i = 1; i < seq.length; i += 1) {
        const prev = seq[i - 1];
        const cur = seq[i];
        const sameLine = cur.top < prev.bottom && prev.top < cur.bottom;
        if (sameLine && cur.left - prev.right < 0.25 * fs) out.push(`gap ${cur.left - prev.right}`);
      }
    } else {
      const badge = li.querySelector('[data-scope="marker"][data-part="root"]');
      const detail = li.querySelector('[data-testid="card-detail"]');
      if (!badge || !detail) { out.push('missing badge or detail'); continue; }
      const fs = parseFloat(getComputedStyle(detail).fontSize);
      const gap = detail.getBoundingClientRect().top - badge.getBoundingClientRect().bottom;
      const mt = parseFloat(getComputedStyle(detail).marginTop);
      if (gap > 0.5 * fs) out.push(`card gap ${gap}`);
      if (mt > 0.5 * fs) out.push(`card margin-top ${mt}`);
    }
  }
  return { out, rows: rows.length };
}

async function showLeft(page) {
  const visible = await page.evaluate(() => {
    const e = document.querySelector('[data-testid="pane-tabs"]');
    return !!e && getComputedStyle(e).display !== 'none';
  });
  if (visible) await page.click('#pane-tab-artifacts');
}

async function withProduct(dir, query, width, kind, fn) {
  await withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.setViewportSize({ width, height: 900 });
      await page.goto(`${srv.url}${query}`);
      await waitMounted(page);
      await showLeft(page);
      await page.locator(`#app [data-testid="view-select-button"][data-payload="${kind}"]`).click();
      await fn(page);
      assert.deepEqual(nonZero(await collectSafety(page, rec)), []);
    } finally {
      await context.close();
    }
  });
}

async function withFixtureRoot(dir, kind, fn) {
  await withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.setViewportSize({ width: 1280, height: 900 });
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal(await page.evaluate(() => typeof window.__fandheTest?.viewFixtureMount), 'function', 'test hooks are missing');
      await page.evaluate(() => {
        const box = document.createElement('div');
        box.id = 'view-fixture-root';
        document.body.append(box);
      });
      assert.deepEqual(JSON.parse(await page.evaluate(() => window.__fandheTest.viewFixtureMount('view-fixture-root'))), { ok: true });
      const r = JSON.parse(await page.evaluate(([n, p]) => window.__fandheTest.viewFixtureDispatch(n, p), ['fixture_select', `${kind}:b`]));
      assert.equal(r.outcome, 'applied');
      await fn(page);
      assert.deepEqual(nonZero(await collectSafety(page, rec)), []);
    } finally {
      await context.close();
    }
  });
}

const CASES = [
  ['task_tree', '?fixture=artifact-redraw-shaped-task_tree-50', 50],
  ['card_list', '?fixture=artifact-redraw-shaped-card_list-20', 20],
];

// 4 場面（製品 2 種 × 幅 2 つ）と代役 2 種を流し、違反の文字列を集める。
async function violationsOf(dir) {
  const found = [];
  for (const [kind, query, n] of CASES) {
    for (const w of WIDTHS) {
      await withProduct(dir, query, w, kind, async (page) => {
        const r = await page.evaluate(measure, ['#app', kind]);
        assert.equal(r.rows, n, `${kind} ${w}`);
        found.push(...r.out);
      });
    }
    await withFixtureRoot(dir, kind, async (page) => {
      const r = await page.evaluate(measure, [ROOT, kind]);
      assert.ok(r.rows > 0, `${kind} fixture rows`);
      found.push(...r.out);
    });
  }
  return found;
}

for (const [id, [kind, query, n]] of [['RS1', CASES[0]], ['RS2', CASES[1]]]) {
  test(`${id}: 製品の ${kind} の全行で空きの条件を満たす（幅 1280px・800px）`, async () => {
    for (const w of WIDTHS) {
      await withProduct(process.env.FANDHE_WEB_TEST_DIR, query, w, kind, async (page) => {
        const r = await page.evaluate(measure, ['#app', kind]);
        assert.equal(r.rows, n, `${kind} ${w}`);
        assert.deepEqual(r.out, [], `${kind} ${w}`);
      });
    }
  });
}

for (const [id, kind] of [['RS3', 'task_tree'], ['RS4', 'card_list']]) {
  test(`${id}: 代役の ${kind} (b) の全行で空きの条件を満たす`, async () => {
    await withFixtureRoot(process.env.FANDHE_WEB_TEST_DIR, kind, async (page) => {
      const r = await page.evaluate(measure, [ROOT, kind]);
      assert.ok(r.rows > 0);
      assert.deepEqual(r.out, []);
    });
  });
}

test('RS5: 規則を外した写しでは同じ測定が違反を数える。外さない写しでは 0 件', async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-row-spacing-'));
  try {
    const keep = path.join(tmp, 'keep');
    const bare = path.join(tmp, 'bare');
    fs.cpSync(process.env.FANDHE_WEB_TEST_DIR, keep, { recursive: true });
    fs.cpSync(process.env.FANDHE_WEB_TEST_DIR, bare, { recursive: true });
    assert.deepEqual(await violationsOf(keep), [], 'unchanged copy');
    const css = path.join(bare, 'app.css');
    let text = fs.readFileSync(css, 'utf8');
    const cut = [
      /\[data-view="task_tree"\] \[data-testid="task-detail"\],\n\[data-view="task_tree"\] \[data-testid="state-badge"\] \{[^}]*\}\n/g,
      /\[data-view="card_list"\] \[data-testid="card-detail"\] \{[^}]*\}\n/g,
    ];
    for (const re of cut) {
      assert.equal((text.match(re) ?? []).length, 1, String(re));
      text = text.replace(re, '');
    }
    fs.writeFileSync(css, text);
    const found = await violationsOf(bare);
    assert.ok(found.some((s) => s.startsWith('gap')), JSON.stringify(found));
    assert.ok(found.some((s) => s.startsWith('card')), JSON.stringify(found));
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});
