// Step の題・「いまやること」・本文の表示の試験（SHELL-7。TASK-525 1/2）。CSP あり・なしの両方で流す。
//
// (1) 試験用の教材 `ch0-3`（`?fixture=ch0-3-chapters`）の 20 Step を通し、各 Step の DOM の題（step-title）と do（step-do）が
//     教材の IR（crates/web/tests/data/ch0_3/ir.json。試験の中で読む）と一致すること、章の最初の Step でも出ること、
//     Step を進めても親（step-text）が同じ要素のまま残ること（要素の同一性。FEND-3）を確かめる。
// (2) 最小教材 min-3step-pack を一時ディレクトリへ写して st-1.md を構造のある Markdown に置き換え、実物の `fandhe-pack compile`
//     で IR 版 2 を作って `loadIr` で入れ、見出し・一覧・表が構造を保って出ること、外部の画像が img にならず代替の文字になること、
//     実行・通信・CSP 違反が全区分 0 であること、幅 360px で横スクロールが無いことを確かめる。
// (3) 長い行の pre・列の多い表を Step の本文に置き、横にスクロールする pre・表に Tab で届き左右の矢印キーで横に動くこと、
//     ページと section.sh-step が横にはみ出さないことを幅 1280・360px で確かめる（S1。#2096。5.480）。負の対照: 短い本文では止まらない・
//     app.css の overflow-x の規則を外した写しでは止まらない。
// (4) 届いた pre・表のアクセシビリティツリー上の役割・名前を CDP で読んで「今こう見える」を固定する（S2。名前は付けていない。
//     付けるには spec の決定と a11y 契約の版上げが要る。docs/known-gaps.md）。対照: 固定の aria-label を付けると名前が読める。
// 対照: step-body を display: none にすると判定が不合格になる（空振りの防止）。
// 案内レベル B・C の確かめは native の試験（crates/web/tests/step_body_host.rs）が持つ（開始値の決定は TASK-294）。
// 書き込むパスは一時ディレクトリの固定のパスだけで、教材の値からパスを作らない。失敗の文面に本文の中身は出さない。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_PACK_BIN（前提バイナリ）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, summarize,
} from './harness.mjs';
import { ledger, mouseDriver, journey } from './ch0-3-journey.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const BASE = '/fandhe-step-body/';
const QUERY = '?fixture=ch0-3-chapters';
const PACK = path.resolve(HERE, '../../testdata/engine-event-consistency/test-data/min-3step-pack');
const IR = JSON.parse(fs.readFileSync(path.join(HERE, '../../crates/web/tests/data/ch0_3/ir.json'), 'utf8'));
const MARKDOWN = [
  '# 大見出し',
  '',
  '## 小見出し',
  '',
  '- 項目 1',
  '- 項目 2',
  '',
  '1. 手順 1',
  '2. 手順 2',
  '',
  '| 列 A | 列 B |',
  '| --- | --- |',
  '| a1 | b1 |',
  '| a2 | b2 |',
  '',
  '![外部の画像](https://example.invalid/x.png)',
  '',
].join('\n');

const LONG_ASCII = 'x'.repeat(600);
const LONG_JA = 'あ'.repeat(600);
const WIDE_COLS = 80;
const wideTable = () => [
  `| ${Array.from({ length: WIDE_COLS }, (_, i) => `c${i + 1}`).join(' | ')} |`,
  `|${' --- |'.repeat(WIDE_COLS)}`,
  `| ${Array.from({ length: WIDE_COLS }, (_, i) => `v${i + 1}`).join(' | ')} |`,
].join('\n');
const LONG_MARKDOWN = ['# 長い行', '', '```', LONG_ASCII, LONG_JA, '```', '', wideTable(), ''].join('\n');
const SHORT_MARKDOWN = ['# 短い行', '', '```', 'abc', '```', '', '| 列 A | 列 B |', '| --- | --- |', '| a1 | b1 |', ''].join('\n');

let browser;
let irText;
let irLong;
let irShort;

function compileMarkdown(bin, markdown) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-step-body-'));
  try {
    fs.cpSync(PACK, work, { recursive: true });
    fs.writeFileSync(path.join(work, 'chapters', 'ch-min', 'st-1.md'), markdown, 'utf8');
    const r = spawnSync(bin, ['compile', work], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    assert.equal(r.status, 0, `compile exit ${r.status}`);
    assert.equal(JSON.parse(r.stdout).format_version, 2);
    return r.stdout;
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  const bin = packBinFromEnv();
  irText = compileMarkdown(bin, MARKDOWN);
  irLong = compileMarkdown(bin, LONG_MARKDOWN);
  irShort = compileMarkdown(bin, SHORT_MARKDOWN);
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const visible = (sel) => `(() => { const e = document.querySelector('${sel}'); if (!e) return false; const r = e.getBoundingClientRect(); const c = getComputedStyle(e); return r.width > 0 && r.height > 0 && c.display !== 'none' && c.visibility !== 'hidden'; })()`;

async function stepTexts(page) {
  return page.evaluate(() => ({
    titles: [...document.querySelectorAll('[data-testid="step-title"]')].map((e) => e.textContent),
    dos: [...document.querySelectorAll('[data-testid="step-do"]')].map((e) => e.textContent),
    parents: document.querySelectorAll('[data-testid="step-text"]').length,
  }));
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`20 Step の題と do が IR のとおり出て、親の要素が同じまま残る（${mode}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
      const L = ledger();
      await page.goto(`${srv.url}${QUERY}`);
      await waitMounted(page);
      const parent = await page.evaluateHandle(() => document.querySelector('[data-testid="step-text"]'));
      let seen = 0;
      const onStep = async (uid, status) => {
        if (status !== 'entered') return;
        const cur = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.current);
        const node = IR.nodes.find((n) => n.uid === cur);
        const t = await stepTexts(page);
        seen += 1;
        L.check(uid, '親が 1 個', t.parents === 1);
        L.check(uid, '題が IR のとおり', t.titles.length === 1 && t.titles[0] === node.step.title);
        L.check(uid, 'do が IR のとおり', t.dos.length === 1 && t.dos[0] === `いまやること${node.step.do}`);
        L.check(uid, '題が見える', await page.evaluate(visible('[data-testid="step-title"]')));
      };
      await journey(page, mouseDriver(page), '通し', L, null, { chaptered: true, onStep });
      L.check('通し', '20 Step を訪れた', seen === 20);
      L.check('通し', '親の要素が同じまま', await parent.evaluate((e) => e.isConnected && e === document.querySelector('[data-testid="step-text"]')));
      L.check('共通', 'ページ内のエラーが 0 件', summarize(rec).pageErrors === 0);
      assert.deepEqual(L.failed(), []);
      await context.close();
    }));
  });

  test(`最小教材の本文が見出し・一覧・表の構造を保って出て、実行・通信が 0 件（${mode}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      for (const width of [1280, 360]) {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width, height: 800 } });
        try {
          await page.goto(srv.url);
          await waitMounted(page);
          const n0 = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.installCount);
          assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), irText), true, 'load-ir sent');
          await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
          await page.waitForSelector('[data-testid="step-body"]', { timeout: 10000 });
          if (width === 360) await page.locator('[role="tab"]').first().click();
          const c = await page.evaluate(() => {
            const b = document.querySelector('[data-testid="step-body"]');
            const n = (s) => b.querySelectorAll(s).length;
            return {
              h1: n('h1'), h2: n('h2'), ul: n('ul'), ol: n('ol'), li: n('li'), th: n('th'), td: n('td'), img: n('img'),
              level: b.getAttribute('data-body-level'),
              hasAlt: b.textContent.includes('外部の画像'),
              overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
            };
          });
          assert.deepEqual(
            { ...c, overflow: undefined },
            { h1: 1, h2: 1, ul: 1, ol: 1, li: 4, th: 2, td: 4, img: 0, level: 'A', hasAlt: true, overflow: undefined },
            `width ${width}`,
          );
          assert.equal(c.overflow, false, `width ${width}: horizontal scroll`);
          assert.equal(await page.evaluate(visible('[data-testid="step-body"]')), true, `width ${width}: visible`);
          const counts = await collectSafety(page, rec);
          assert.deepEqual(nonZero(counts), [], JSON.stringify({ width, csp, counts }));

          // 対照: 本文を hidden にすると「見える」の判定は不合格になる（style 属性を使わないので CSP ありでも流せる）。
          await page.evaluate(() => { document.querySelector('[data-testid="step-body"]').hidden = true; });
          assert.equal(await page.evaluate(visible('[data-testid="step-body"]')), false, 'control: hidden body must fail the visibility check');
        } finally {
          await context.close();
        }
      }
    }));
  });
}

// S1・S2 用の固定のページ内関数。教材の文字列は返さない。
function stepOverflowProbe() {
  const b = document.querySelector('[data-testid="step-body"]');
  const sec = b.closest('section.sh-step') ?? b;
  const sr = sec.getBoundingClientRect();
  const m = (sel) => [...b.querySelectorAll(sel)].map((e) => ({ scroll: e.scrollWidth > e.clientWidth, out: e.getBoundingClientRect().right > sr.right + 1 }));
  const pre = m('pre');
  const table = m('table');
  return {
    pre: pre.length, preScroll: pre.filter((x) => x.scroll).length, preOut: pre.filter((x) => x.out).length,
    table: table.length, tableScroll: table.filter((x) => x.scroll).length, tableOut: table.filter((x) => x.out).length,
    page: document.documentElement.scrollWidth > document.documentElement.clientWidth + 1,
  };
}
function stepFocusProbe() {
  const a = document.activeElement;
  return {
    tag: a ? a.tagName : '',
    inBody: !!(a && a.closest && a.closest('[data-testid="step-body"]')),
    left: a ? Math.round(a.scrollLeft) : -1,
    sw: a ? a.scrollWidth : -1,
    cw: a ? a.clientWidth : -1,
    outline: a ? getComputedStyle(a).outlineStyle : '',
  };
}

// Tab の回数。右エリアは左エリアの後ろにあるので、画面の部品が一巡する以上を押す。
const STEP_MAX_TABS = 70;

// フォーカス中の要素の AX の役割・名前。名前は本文を出さないよう、空か・長さ・要素の文字と同じかだけを返す。
async function readAx(page) {
  const cdp = await page.context().newCDPSession(page);
  try {
    await cdp.send('Accessibility.enable');
    const { result } = await cdp.send('Runtime.evaluate', { expression: 'document.activeElement' });
    const { node } = await cdp.send('DOM.describeNode', { objectId: result.objectId });
    const { nodes } = await cdp.send('Accessibility.getPartialAXTree', { backendNodeId: node.backendNodeId, fetchRelatives: false });
    const n = nodes[0] ?? {};
    const name = String(n.name?.value ?? '');
    const text = await page.evaluate(() => document.activeElement.textContent ?? '');
    return { role: String(n.role?.value ?? ''), ignored: !!n.ignored, nameEmpty: name === '', nameLen: name.length, nameIsText: name !== '' && name === text.trim() };
  } finally {
    await cdp.detach().catch(() => {});
  }
}

// 先頭から Tab を繰り返し、本文の中の PRE・TABLE に初めて届いた回数と、そこでの矢印キーの効きを種類ごとに記録する。
// ax が真なら、届いた時点で AX の役割・名前も読む。
async function stepTabThrough(page, ax) {
  await page.evaluate(() => { document.activeElement?.blur(); window.scrollTo(0, 0); });
  const reached = new Map();
  for (let i = 1; i <= STEP_MAX_TABS; i += 1) {
    await page.keyboard.press('Tab');
    const f = await page.evaluate(stepFocusProbe);
    if (!f.inBody || !['PRE', 'TABLE'].includes(f.tag) || reached.has(f.tag)) continue;
    const rec = { tabs: i, outline: f.outline };
    if (ax) rec.ax = await readAx(page);
    await page.keyboard.press('ArrowRight');
    await page.waitForFunction(() => document.activeElement.scrollLeft > 0, null, { timeout: 5000 }).catch(() => {});
    const right = await page.evaluate(stepFocusProbe);
    await page.keyboard.press('ArrowLeft');
    await page.waitForFunction(() => document.activeElement.scrollLeft === 0, null, { timeout: 5000 }).catch(() => {});
    const back = await page.evaluate(stepFocusProbe);
    Object.assign(rec, { right: right.left, back: back.left, max: right.sw - right.cw });
    reached.set(f.tag, rec);
  }
  return reached;
}

const SCROLL_RULES = {
  pre: /\.sh-step pre \{ overflow-x: auto; \}\n/g,
  table: /(\.sh-step table \{ display: block; max-width: 100%; )overflow-x: auto; /g,
};

// cssEdit を適用した配信物の写しで ir を開き、Step の本文の pre・表のはみ出しと Tab の到達を測る。
async function stepKeyboardAt(ir, width, cssEdit, ax) {
  const src = process.env.FANDHE_WEB_TEST_DIR;
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-step-body-kbd-'));
  try {
    fs.cpSync(src, copy, { recursive: true });
    if (cssEdit) {
      const f = path.join(copy, 'app.css');
      fs.writeFileSync(f, cssEdit(fs.readFileSync(f, 'utf8')));
    }
    let out;
    await withServer({ dir: copy, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width, height: 800 } });
      try {
        await page.goto(srv.url);
        await waitMounted(page);
        const n0 = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.installCount);
        assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), ir), true, 'load-ir sent');
        await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
        await page.waitForSelector('[data-testid="step-body"]', { timeout: 10000 });
        if (width < 960) await page.locator('#pane-tab-ops').click();
        const overflow = await page.evaluate(stepOverflowProbe);
        const reached = await stepTabThrough(page, ax);
        out = { overflow, reached: Object.fromEntries(reached) };
        if (ax) {
          // 対照: 固定の aria-label を付けると同じ読み方で名前が読める（判定の空振りの防止）。付けた属性はその場で外す。
          await page.locator('[data-testid="step-body"] pre').focus();
          await page.evaluate(() => { document.activeElement.setAttribute('aria-label', '対照'); });
          out.control = await readAx(page);
          await page.evaluate(() => { document.activeElement.removeAttribute('aria-label'); });
        }
        const counts = await collectSafety(page, rec);
        assert.deepEqual(nonZero(counts), [], JSON.stringify({ width, counts }));
      } finally {
        await context.close();
      }
    });
    return out;
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}

for (const w of [1280, 360]) {
  test(`S1 Step の本文の横にスクロールする pre と表に Tab で届き、左右の矢印キーで横に動く（${w}px）`, { timeout: 180000 }, async (t) => {
    const m = await stepKeyboardAt(irLong, w);
    t.diagnostic(`browser=${browser.version()} channel=${channelFromEnv() ?? 'default'} width=${w} reached=${JSON.stringify(m.reached)} overflow=${JSON.stringify(m.overflow)}`);
    assert.ok(m.overflow.preScroll >= 1 && m.overflow.tableScroll >= 1, `${w}px: 横にスクロールする pre・表を測った（空振りの防止）`);
    assert.equal(m.overflow.page, false, `${w}px: ページが横にはみ出さない`);
    assert.equal(m.overflow.preOut + m.overflow.tableOut, 0, `${w}px: pre・表が section を越えない`);
    for (const tag of ['PRE', 'TABLE']) {
      const r = m.reached[tag];
      assert.ok(r && r.tabs > 0, `${w}px: ${tag} に Tab で届く`);
      assert.ok(r.max > 0 && r.right > 0, `${w}px: ${tag} 右矢印で横に動く`);
      assert.equal(r.back, 0, `${w}px: ${tag} 左矢印で元に戻る`);
    }
  });
}

test('S1 負の対照: はみ出さない短い本文の pre と表には Tab で止まらない', { timeout: 120000 }, async () => {
  const m = await stepKeyboardAt(irShort, 1280);
  assert.ok(m.overflow.pre >= 1 && m.overflow.table >= 1, '短い本文でも pre・表を測った（空振りの防止）');
  assert.equal(m.overflow.preScroll + m.overflow.tableScroll, 0, '短い本文の pre・表は横にはみ出さない');
  assert.deepEqual(m.reached, {}, '本文の pre・表に Tab で止まらない');
});

for (const [tag, re] of Object.entries(SCROLL_RULES)) {
  test(`S1 負の対照: ${tag} の overflow-x の規則を外すと長い行の ${tag} に Tab で止まらない`, { timeout: 120000 }, async () => {
    const cssEdit = (css) => {
      assert.equal((css.match(re) ?? []).length, 1, 'rule must match exactly once');
      return css.replace(re, tag === 'pre' ? '' : '$1');
    };
    const m = await stepKeyboardAt(irLong, 1280, cssEdit);
    assert.equal(m.reached[tag.toUpperCase()], undefined, `${tag} に Tab で止まらない`);
  });
}

test('S2 Step の本文のスクロールする pre と表の AX の役割・名前を観察する（1280px）', { timeout: 180000 }, async (t) => {
  const m = await stepKeyboardAt(irLong, 1280, null, true);
  t.diagnostic(`browser=${browser.version()} channel=${channelFromEnv() ?? 'default'} ax=${JSON.stringify(Object.fromEntries(Object.entries(m.reached).map(([k, v]) => [k, v.ax])))} control=${JSON.stringify(m.control)}`);
  const pre = m.reached.PRE?.ax;
  const table = m.reached.TABLE?.ax;
  assert.ok(pre && table, 'PRE・TABLE に届いて AX を読んだ');
  // 観察（Chromium 153。5.480）: pre は役割 generic で、名前は付けていないのに中身の文字から計算される（長い行なら長い名前になる）。表は役割 table で名前なし。
  assert.deepEqual([pre.role, pre.ignored, pre.nameEmpty], ['generic', false, false], 'PRE の役割と名前');
  assert.deepEqual([table.role, table.ignored, table.nameEmpty], ['table', false, true], 'TABLE の役割と名前');
  assert.equal(m.control.role, 'generic');
  assert.equal(m.control.nameLen, '対照'.length, '対照: 固定の aria-label を付けると名前が読める');
});
