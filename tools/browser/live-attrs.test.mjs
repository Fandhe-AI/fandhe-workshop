// ログが増える更新を 101 回の観察で挟んでも、通知欄・ライブの欄・領域のラベル・ビューの選択の属性が最初の時点と同じままで、
// ログの一覧がライブリージョンの外にあることの試験（A11Y-7。TASK-371。CSP あり・なし）。
//
// 根拠は PoC-1 の F-2（keyed list の親に後から付けた属性は更新のたびに消える）。このため属性は `view()` に直接書く方針で、
// その方針が実際の DOM で守られていることを配信する設定の画面で固定する。list-parent-attrs.test.mjs は「後から付けた属性が消える」こと、
// view-switch.test.mjs の V4 は「ビューの選択の属性が 101 回の再描画の後も同じ」ことを確かめる。本試験は両者を重ねて書かず、
// 通知欄・ラベル・ライブの欄を含む固定の ID の一覧と画面全体の aria 系の属性を、ログが増える更新の間じゅう見張る。
//
// 注入の口: input-field-xss.test.mjs と同じ。配信する設定の web の一時の写しに `fandhe-ir` の meta と `ir/main.json`
// （開始点 s-1-4 を足した ch0_3 の IR）を置き、`?start=s-1-4` で開いて本物の入力欄から送る。試験用の口は使わない。
// 送る入力は固定の無害な `/new-idea Bad_Name`（引数の形が合わず拒否される。Step は進まず、1 回の送信でエンジンのログが 2 行増える）。
//
// 観察点は 101 回。観察点ごとに 3 つの写しを最初の時点と比べ、差が 0 回であることを確かめる。
//   (1) 固定の ID の写し: 通知欄・ライブの欄・タブ・ビューの選択などの存在・件数・role・aria-*・data-selected・hidden。
//       無い要素は「無い」という状態として写しに残す（今画面に無い欄が後から入っても、判定が壊れずに「消えない」を数え始める）。
//   (2) 文書全体の写し: `#app` の中で role か aria-* を持つ要素すべての属性の集合（増えたログの行に aria・role が付かないこともここで検出）。
//   (3) AX の投影: CDP の Accessibility.getFullAXTree から、役割と live・atomic・relevant・busy の性質（status 以外は名前も）を木の順に並べたもの。
// あわせて観察点ごとに、log-list と part-log-list がライブリージョン（role=status・alert・log・aria-live が off 以外）の祖先を
// DOM でも AX でも持たないことを確かめる。
//
// 限界（2026-11-09 の HEAD で実測）: 画面には live-response・live-left（aria-live の専用の欄）・view-notice・右の領域の aria-label が
// まだ無い（TASK-370 の担当）。本試験はそれらを「無い」状態のまま差 0 として数え、無いことは assert しない。あれば aria-live=polite・
// aria-atomic=true であることだけを確かめる。読み上げそのものの証拠ではない（支援技術での確認は TASK-381 の人の確認）。幅は 1280px だけ。
//
// 空振りの防止: 描かれているはずの最小の要素（通知欄 3 つの role・成果物のラベル・tablist のラベル・2 つのログの一覧）の存在を assert し、
// 負の対照 N1（属性の欠落・書き換え・ライブの欄の追加を 3 つの写しが差として数える）・N2（log-list の祖先に aria-live を付けると
// DOM と AX の判定が「ライブの中」と答える）で検出器を確かめる。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1。配信側の確認用）。未設定・不在は skip にせず失敗にする。
// 書き込み先は mkdtemp の一時ディレクトリの固定のパス（ir/main.json）だけ。ページへは文字列・配列の引数だけを渡す。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { pageRequestProblems } from './delivery.mjs';
import { irFetchCheck } from './l1-l2-runner.mjs';
import { DRAFT } from './ch0-3-journey.mjs';
import { loadBaseIr, withStarts } from './e2-ir.mjs';

const BASE = '/fandhe-live-attrs/';
const PACK_ID = 'a11y7-live-attrs';
const START = 's-1-4';
const TITLE = '[data-testid="step-title"]';
const RECEIVED = '[data-testid="part-log-list"] > li[data-kind="received"]';
const ENGINE_ROWS = '[data-testid="part-log-list"] > li';
const SECRET = '[data-testid="secret-notice"]';
const CSP_META_RE = /<meta\b[^>]*\bhttp-equiv\s*=\s*["']?content-security-policy["']?[^>]*>/gi;
const T = 30000;
const POINTS = 101;
// 1 回の送信で増えるエンジンのログの行数（2026-11-09 の実測。拒否の行が受付の行に続く）。最後の観察点で PoC-12 の 218 行に届かせる。
const ROWS_PER_SEND = 2;
const FINAL_ROWS = 218;
// a11y.rs の ID の定数と同じ値（出所: crates/web の a11y の契約）。画面に無い ID も一覧に入れ、「無い」まま変わらないことを数える。
const IDS = [
  'live-response', 'live-left', 'view-notice', 'input-notice', 'log-notice', 'export-notice', 'secret-notice',
  'stale-notice', 'view-only-notice', 'save-notice', 'shell-notice', 'pane-tabs', 'pane-tab', 'view-select',
  'view-select-button', 'view-list', 'log-list', 'part-log-list',
];
const ATTRS = [
  'role', 'aria-live', 'aria-atomic', 'aria-relevant', 'aria-label', 'aria-labelledby', 'aria-controls', 'aria-selected',
  'data-selected', 'data-view', 'hidden',
];
const LIVE_LISTS = ['log-list', 'part-log-list'];

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

async function withDeliveryCopy(fn) {
  const ir = withStarts(loadBaseIr());
  ir.pack = { ...ir.pack, id: PACK_ID };
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-live-attrs-web-'));
  try {
    fs.cpSync(process.env.FANDHE_WEB_DIR, copy, { recursive: true });
    const index = path.join(copy, 'index.html');
    const html = fs.readFileSync(index, 'utf8');
    assert.equal(html.split('</head>').length - 1, 1, 'exactly one </head>');
    assert.ok(!/name\s*=\s*["']?fandhe-ir/i.test(html), 'delivery index has no fandhe-ir meta');
    assert.ok(!/name\s*=\s*["']?fandhe-test-hooks/i.test(html), 'delivery index has no test-hooks meta');
    assert.equal((html.match(CSP_META_RE) ?? []).length, 1, 'exactly one CSP meta');
    fs.writeFileSync(index, html.replace('</head>', '<meta name="fandhe-ir" content="1"></head>'));
    fs.mkdirSync(path.join(copy, 'ir'), { recursive: true });
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), JSON.stringify(ir), 'utf8');
    return await fn(copy);
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}

async function openStart(page, srv) {
  await page.goto(`${srv.url}?start=${START}`);
  await waitMounted(page);
  assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'no test hooks');
  await page.waitForSelector(TITLE, { timeout: T });
  return page.evaluate((sel) => document.querySelector(sel).textContent, TITLE);
}

// ---- 写し (1) 固定の ID。要素ごとに属性の値（無ければ null）を並べる。
const snapIds = (page) => page.evaluate(([ids, attrs]) => {
  const out = {};
  for (const id of ids) {
    const els = [...document.querySelectorAll(`[data-testid="${id}"]`)];
    out[id] = els.map((el) => Object.fromEntries(attrs.map((a) => [a, el.getAttribute(a)])));
  }
  return out;
}, [IDS, ATTRS]);

// ---- 写し (2) #app の中で role か aria-* を持つ要素すべて。(識別子か tag, 同じ識別子の中の順番) をキーにする。
const snapDoc = (page) => page.evaluate(() => {
  const out = {};
  const seen = {};
  for (const el of document.querySelectorAll('#app *')) {
    const attrs = el.getAttributeNames().filter((n) => n === 'role' || n.startsWith('aria-'));
    if (attrs.length === 0) continue;
    const base = el.dataset.testid || el.tagName.toLowerCase();
    seen[base] = (seen[base] ?? -1) + 1;
    out[`${base}#${seen[base]}`] = Object.fromEntries(attrs.sort().map((n) => [n, el.getAttribute(n)]));
  }
  return out;
});

// ---- 写し (3) AX の投影。役割か性質で選んだ節点を木の順に並べる。status の名前は中身の文字から変わりうるので含めない。
const AX_ROLES = new Set(['status', 'region', 'tablist', 'tab', 'tabpanel', 'tree', 'treeitem', 'list', 'alert', 'log']);
const AX_PROPS = ['live', 'atomic', 'relevant', 'busy'];
const axValue = (n, name) => n.properties?.find((p) => p.name === name)?.value?.value;
async function axNodes(cdp) {
  const { nodes } = await cdp.send('Accessibility.getFullAXTree');
  return nodes;
}
function projectAx(nodes) {
  return nodes
    .filter((n) => !n.ignored && (AX_ROLES.has(n.role?.value) || AX_PROPS.some((p) => axValue(n, p) !== undefined)))
    .map((n) => ({
      role: n.role?.value ?? null,
      props: Object.fromEntries(AX_PROPS.map((p) => [p, axValue(n, p) ?? null])),
      name: n.role?.value === 'status' ? null : (n.name?.value ?? null),
    }));
}
const snapAx = async (cdp) => projectAx(await axNodes(cdp));

// 差の数: 2 つの写しのキーごとの値が JSON として違う個数（片方にしか無いキーも 1 個）。
function diffCount(a, b) {
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  let n = 0;
  for (const k of keys) if (JSON.stringify(a[k]) !== JSON.stringify(b[k])) n += 1;
  return n;
}
const diffAx = (a, b) => (JSON.stringify(a) === JSON.stringify(b) ? 0 : Math.max(a.length, b.length, 1));

// ---- ライブリージョンの外の判定。
const domInsideLive = (page) => page.evaluate((ids) => {
  const sel = '[role=status],[role=alert],[role=log],[aria-live]:not([aria-live=off])';
  return Object.fromEntries(ids.map((id) => {
    const els = [...document.querySelectorAll(`[data-testid="${id}"]`)];
    return [id, { count: els.length, inside: els.filter((el) => el.closest(sel) !== null).length }];
  }));
}, LIVE_LISTS);

// 2 つのリストに対応する AX の節点とその祖先に、live（off 以外）か status・alert・log の役割があるか。見つからない（空で無視された）リストは null。
async function axInsideLive(cdp) {
  const nodes = await axNodes(cdp);
  const byId = new Map(nodes.map((n) => [n.nodeId, n]));
  const byBackend = new Map(nodes.filter((n) => n.backendDOMNodeId !== undefined).map((n) => [n.backendDOMNodeId, n]));
  const { root } = await cdp.send('DOM.getDocument', { depth: 0 });
  const out = {};
  for (const id of LIVE_LISTS) {
    const { nodeId } = await cdp.send('DOM.querySelector', { nodeId: root.nodeId, selector: `[data-testid="${id}"]` });
    if (!nodeId) { out[id] = null; continue; }
    const { node } = await cdp.send('DOM.describeNode', { nodeId });
    let cur = byBackend.get(node.backendNodeId);
    if (!cur) { out[id] = null; continue; }
    let inside = false;
    while (cur) {
      const live = axValue(cur, 'live');
      if ((live !== undefined && live !== 'off') || ['status', 'alert', 'log'].includes(cur.role?.value)) inside = true;
      cur = cur.parentId === undefined ? undefined : byId.get(cur.parentId);
    }
    out[id] = inside;
  }
  return out;
}

async function takeSnapshots(page, cdp) {
  return { ids: await snapIds(page), doc: await snapDoc(page), ax: await snapAx(cdp) };
}

// 入力欄に入れて Enter で送る。受付の行が 1 つ、エンジンのログが ROWS_PER_SEND 行増えるまで待つ。
async function sendOnce(page, before) {
  await page.fill(DRAFT, '/new-idea Bad_Name');
  await page.press(DRAFT, 'Enter');
  await page.waitForFunction(([recv, rows, r, e]) => document.querySelectorAll(recv).length === r
    && document.querySelectorAll(rows).length === e, [RECEIVED, ENGINE_ROWS, before.received + 1, before.rows + ROWS_PER_SEND], { timeout: T });
}
const counts = (page) => page.evaluate(([recv, rows, secret, title]) => ({
  received: document.querySelectorAll(recv).length,
  rows: document.querySelectorAll(rows).length,
  secret: (document.querySelector(secret)?.textContent ?? '').length,
  title: document.querySelector(title)?.textContent ?? null,
}), [RECEIVED, ENGINE_ROWS, SECRET, TITLE]);

async function assertQuiet(page, rec, label) {
  const c = await collectSafety(page, rec);
  const detail = `${label}: ${JSON.stringify({ counts: c, hits: rec.hits.length })}`;
  assert.deepEqual(nonZero(c), [], detail);
  assert.deepEqual(rec.hits, [], detail);
}

function assertNetwork(rec, srv, label) {
  const chk = irFetchCheck(rec, `${srv.url}ir/main.json`);
  assert.equal(chk.irCount, 1, `${label}: ir/main.json request count`);
  assert.ok(chk.irOk, `${label}: ir/main.json status`);
  const problems = pageRequestProblems({ ...rec, failed: chk.failed }, { origin: srv.origin, base: BASE });
  assert.deepEqual(problems, [], `${label}: ${JSON.stringify(problems)}`);
}

// 描かれているはずの最小の要素（空振りの防止）。
async function assertMinimum(page) {
  const m = await page.evaluate(() => {
    const role = (id) => [...document.querySelectorAll(`[data-testid="${id}"]`)].map((e) => e.getAttribute('role'));
    return {
      notices: ['input-notice', 'log-notice', 'shell-notice'].map(role),
      artifact: document.querySelectorAll('[aria-label="成果物"]').length,
      tablist: document.querySelector('[data-testid="pane-tabs"]')?.getAttribute('aria-label') ?? null,
      lists: ['log-list', 'part-log-list'].map((id) => document.querySelectorAll(`[data-testid="${id}"]`).length),
    };
  });
  assert.deepEqual(m.notices, [['status'], ['status'], ['status']], 'notices have role=status');
  assert.equal(m.artifact, 1, 'one region labelled 成果物');
  assert.equal(m.tablist, '表示する領域');
  assert.deepEqual(m.lists, [1, 1], 'both log lists are drawn');
}

// ライブの専用の欄は今の画面に無い。あるなら aria-live=polite・aria-atomic=true であること（個数は診断に残す）。
async function liveRegionReport(page) {
  return page.evaluate(() => ['live-response', 'live-left'].map((id) => {
    const els = [...document.querySelectorAll(`[data-testid="${id}"]`)];
    return { id, count: els.length, bad: els.filter((e) => e.getAttribute('aria-live') !== 'polite' || e.getAttribute('aria-atomic') !== 'true').length };
  }));
}

for (const csp of [true, false]) {
  test(`L1: ログが 1 行から ${FINAL_ROWS} 行まで増える ${POINTS} 回の観察で、属性が最初の時点と同じでログの一覧がライブの外（CSP ${csp ? 'あり' : 'なし'}）`, { timeout: 600000 }, async (t) => {
    await withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      const label = `csp=${csp ? 'on' : 'off'}`;
      try {
        const title0 = await openStart(page, srv);
        const cdp = await context.newCDPSession(page);
        await cdp.send('Accessibility.enable');
        await cdp.send('DOM.enable');
        await assertMinimum(page);
        // 最初の時点は、エンジンのログが 1 行以上になるまで 1 回送ってから取る（初期のログは 0 行）。
        let cur = await counts(page);
        assert.equal(cur.rows, 0, 'initial engine log is empty');
        await sendOnce(page, cur);
        cur = await counts(page);
        const base = await takeSnapshots(page, cdp);
        const rows0 = cur.rows;
        assert.ok(rows0 >= 1, 'first point has log rows');
        const diffs = { ids: 0, doc: 0, ax: 0 };
        const outside = { dom: 0, ax: 0, axUnseen: 0 };
        let prevRows = rows0;
        let increased = 0;
        let lastRows = rows0;
        for (let i = 1; i <= POINTS; i += 1) {
          if (i > 1) {
            // 最後の観察点までに 218 行に届くよう、最後の点の前にまとめて送る。
            const extra = i === POINTS ? Math.ceil((FINAL_ROWS - (cur.rows + ROWS_PER_SEND)) / ROWS_PER_SEND) : 0;
            for (let k = 0; k < 1 + Math.max(extra, 0); k += 1) {
              await sendOnce(page, cur);
              cur = await counts(page);
            }
            assert.equal(cur.title, title0, `${label} point ${i}: step did not move`);
            assert.equal(cur.secret, 0, `${label} point ${i}: no secret warning`);
            assert.ok(cur.rows >= prevRows, `${label} point ${i}: log rows never shrink`);
            if (cur.rows > rows0) increased += 1;
            prevRows = cur.rows;
          }
          lastRows = cur.rows;
          const snap = await takeSnapshots(page, cdp);
          diffs.ids += diffCount(base.ids, snap.ids) > 0 ? 1 : 0;
          diffs.doc += diffCount(base.doc, snap.doc) > 0 ? 1 : 0;
          diffs.ax += diffAx(base.ax, snap.ax) > 0 ? 1 : 0;
          const dom = await domInsideLive(page);
          for (const id of LIVE_LISTS) {
            assert.equal(dom[id].count, 1, `${label} point ${i}: ${id} drawn once`);
            if (dom[id].inside !== 0) outside.dom += 1;
          }
          const ax = await axInsideLive(cdp);
          for (const id of LIVE_LISTS) {
            if (ax[id] === true) outside.ax += 1;
            if (ax[id] === null) outside.axUnseen += 1;
          }
        }
        t.diagnostic(`${label}: points=${POINTS} rows ${rows0} -> ${lastRows}, diffs=${JSON.stringify(diffs)}, inside-live=${JSON.stringify(outside)}, live fields=${JSON.stringify(await liveRegionReport(page))}`);
        assert.ok(lastRows >= FINAL_ROWS, `${label}: reached ${FINAL_ROWS} rows (got ${lastRows})`);
        assert.ok(lastRows < 1000, `${label}: below the 1,000-item fold`);
        assert.ok(increased >= POINTS - 4, `${label}: rows grew at ${increased} points`);
        assert.deepEqual(diffs, { ids: 0, doc: 0, ax: 0 }, `${label}: attribute differences from the first point`);
        assert.deepEqual({ dom: outside.dom, ax: outside.ax }, { dom: 0, ax: 0 }, `${label}: log lists inside a live region`);
        assert.ok(outside.axUnseen <= 1, `${label}: AX node of a log list unseen at ${outside.axUnseen} points`);
        const live = await liveRegionReport(page);
        assert.deepEqual(live.filter((l) => l.bad !== 0), [], `${label}: live fields that exist are polite and atomic`);
        await assertMinimum(page);
        await assertQuiet(page, rec, label);
        assertNetwork(rec, srv, label);
      } finally {
        await context.close();
      }
    })));
  });
}

test('N1: 対照。属性の欠落・書き換え・ライブの欄の追加を、3 つの写しがそれぞれ差として数える', async () => {
  await withDeliveryCopy((copy) => withServer({ dir: copy, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await openStart(page, srv);
      const cdp = await context.newCDPSession(page);
      await cdp.send('Accessibility.enable');
      const before = await takeSnapshots(page, cdp);
      assert.equal(diffCount(before.ids, await snapIds(page)), 0, 'same page, same snapshot');
      await page.evaluate(() => {
        document.querySelector('[data-testid="input-notice"]').removeAttribute('role');
        document.querySelector('[aria-label="成果物"]').setAttribute('aria-label', 'x');
        const fake = document.createElement('div');
        fake.setAttribute('data-testid', 'live-response');
        fake.setAttribute('aria-live', 'polite');
        document.querySelector('#app').appendChild(fake);
      });
      const after = await takeSnapshots(page, cdp);
      assert.ok(diffCount(before.ids, after.ids) >= 2, `ids: ${diffCount(before.ids, after.ids)}`);
      assert.ok(diffCount(before.doc, after.doc) >= 3, `doc: ${diffCount(before.doc, after.doc)}`);
      assert.ok(diffAx(before.ax, after.ax) >= 1, 'ax');
    } finally {
      await context.close();
    }
  }));
});

test('N2: 対照。log-list の祖先に aria-live を付けると、DOM と AX の判定が「ライブの中」と答える', async () => {
  await withDeliveryCopy((copy) => withServer({ dir: copy, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await openStart(page, srv);
      const cdp = await context.newCDPSession(page);
      await cdp.send('Accessibility.enable');
      await cdp.send('DOM.enable');
      // 空の一覧は AX に出ないことがあるので、1 回送ってから当てる。
      await sendOnce(page, await counts(page));
      const clean = { dom: await domInsideLive(page), ax: await axInsideLive(cdp) };
      assert.equal(clean.dom['log-list'].inside, 0);
      assert.equal(clean.ax['log-list'], false);
      await page.evaluate(() => document.querySelector('[data-testid="log-list"]').parentElement.setAttribute('aria-live', 'polite'));
      const dom = await domInsideLive(page);
      const ax = await axInsideLive(cdp);
      assert.equal(dom['log-list'].inside, 1, 'DOM check sees the live ancestor');
      assert.equal(ax['log-list'], true, 'AX check sees the live ancestor');
    } finally {
      await context.close();
    }
  }));
});
