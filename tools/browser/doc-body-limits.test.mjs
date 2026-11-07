// 文書ビューの本文の表示の限界の観察の固定（ART-18。TASK-489 2/2。D-50・D-51）。CSP あり・なしの両方で流す。
//
// 目的: 載せる案（TASK-489 1/2。doc-body.test.mjs）の実装の後に残る限界を、現状の観察として固定する。限界の記録は
// docs/known-gaps.md の ART-18。ここで assert するのは「今こう見える」であり、望ましい姿ではない。仕様の「本文は IR に
// 載っていないため表示しません」は試作の文言で、製品には無い（理由の文は出さない）。その文言が 0 件であることも固定する。
// 観察（各項目に負の対照を付け、判定の空振りを防ぐ）:
//   L1 body を省略・hidden にした文書ビューでは、要素に fixture があっても本文の区画が 0 件で、経路の行は出る。理由の文は出ない
//   L2 body: shown でも、fixture を持たない要素の行には本文の区画が 0 件
//   L3 .md 以外（.txt）の練習データは解釈されず、区画の中は pre > code の 1 ブロックで、先頭の `# ` の行も h1 にならない
//   L4 文書以外のビューを選ぶと区画が 0 件になり、文書に戻すと戻る
//   L5 replace で本文が替わっても、区画の中にブロック単位の前後の印（del・ins・data-mark）が 0 件。替わった後の木だけを出す
//   L6 本文に長い行（pre の長い行・幅の広い表。架空の内容）があっても、行の横のはみ出しとページの横のはみ出しが幅 1280・960・800・360px で
//      0 件で、pre と表は区画の中で横にスクロールする。短い本文の幅と位置は規則の有無で変わらない（#2092）
//   L7 L6 の負の対照: 配信物の写しの app.css から本文の規則（min-width・overflow-x）を 1 つずつ外すと、同じ測定がはみ出しを数える
//   L8 横にスクロールする本文の pre には、キーボードだけで届く（Tab で止まり、左右の矢印キーで横に動く）。幅 1280・360px。
//      負の対照: 短い本文（はみ出さない）の pre には Tab で止まらない。overflow-x を外した写しでも止まらない（#2094）
// 試験用の教材は crates/web/tests/data/doc_body_limits_pack を一時ディレクトリへ写して実物の `fandhe-pack compile` で IR にする。
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

const HERE = path.dirname(fileURLToPath(import.meta.url));
const BASE = '/fandhe-doc-body-limits/';
const PACK = path.resolve(HERE, '../../crates/web/tests/data/doc_body_limits_pack');
const BODY = '[data-testid="artifact-doc-body"]';
const ROW_MD = 'li[data-path="notes/md"]';
const ROW_TXT = 'li[data-path="notes/txt"]';
const ROW_BARE = 'li[data-path="notes/bare"]';
const APPROVE = '[data-testid="approval-btn"][data-decision="approve"]';
const SELECT = '[data-testid="view-select-button"]';
const PROTOTYPE_REASON = ['本文はこの教材の定義（IR）に載っていないため表示しません', '本文は IR に載っていない'];
const SHOWN = '{kind: document, body: shown}';

let browser;
let irShown;
let irOmitted;
let irHidden;
let irLong;

// L6 用の長い行（架空の内容。空白が無く、折り返す位置が無い）。一時ディレクトリの写しにだけ書き、教材の原本は変えない。
const LONG_ASCII = 'x'.repeat(600);
const LONG_JA = 'あ'.repeat(600);
function writeLongLines(work) {
  fs.writeFileSync(path.join(work, 'fixtures/doc-txt.txt'), `# 【架空】長い行\n\n${LONG_ASCII}\n${LONG_JA}\n`, 'utf8');
  const md = path.join(work, 'fixtures/doc-md.md');
  const table = `| 列A | 列B |\n|---|---|\n| ${LONG_ASCII} | ${LONG_JA} |\n`;
  fs.appendFileSync(md, `\n\`\`\`\n${LONG_ASCII}\n\`\`\`\n\n${table}`, 'utf8');
}

function compile(bin, mutate, prepare) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-doc-body-limits-'));
  try {
    fs.cpSync(PACK, work, { recursive: true });
    // 選択のボタンを宣言した 2 種に絞る観察なので、`view_tabs: declared` を足す（既定の `all` は 7 種を出す。PACK-22・TASK-481 1/2）。
    const y = path.join(work, 'workshop.yaml');
    const base = fs.readFileSync(y, 'utf8');
    const declared = base.replace("vocab: '1.4'}", "vocab: '1.4', left_pane: {view_tabs: declared}}");
    assert.notEqual(declared, base, 'left_pane applied');
    fs.writeFileSync(y, declared, 'utf8');
    if (mutate) {
      const after = mutate(declared);
      assert.notEqual(after, declared, 'mutation applied');
      fs.writeFileSync(y, after, 'utf8');
    }
    if (prepare) prepare(work);
    const r = spawnSync(bin, ['compile', work], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    assert.equal(r.status, 0, `compile exit ${r.status}`);
    return r.stdout;
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  const bin = packBinFromEnv();
  irShown = compile(bin);
  assert.equal(JSON.parse(irShown).views['v-doc'].body, 'shown');
  irOmitted = compile(bin, (t) => t.replace(SHOWN, '{kind: document}'));
  assert.equal(JSON.parse(irOmitted).views['v-doc'].body, undefined);
  irHidden = compile(bin, (t) => t.replace(SHOWN, '{kind: document, body: hidden}'));
  assert.equal(JSON.parse(irHidden).views['v-doc'].body, 'hidden');
  irLong = compile(bin, null, writeLongLines);
  assert.equal(JSON.parse(irLong).views['v-doc'].body, 'shown');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function load(page, irText) {
  const n0 = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.installCount);
  assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), irText), true, 'load-ir sent');
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
}

async function approve(page) {
  await page.waitForSelector(APPROVE, { timeout: 10000 });
  await page.click(APPROVE);
}

// 承認の前の状態は行が 0 件。1 回目の承認で 3 行が出るまで待つ。
async function openAndPut(page, srv, irText) {
  await page.goto(srv.url);
  await waitMounted(page);
  await load(page, irText);
  await approve(page);
  await page.waitForSelector(ROW_BARE, { state: 'attached', timeout: 10000 });
  await page.waitForSelector(ROW_MD, { state: 'attached', timeout: 10000 });
  await page.waitForSelector(ROW_TXT, { state: 'attached', timeout: 10000 });
}

const appText = (page) => page.evaluate(() => document.querySelector('#app').textContent);

async function assertNoPrototypeReason(page) {
  const t = await appText(page);
  for (const s of PROTOTYPE_REASON) assert.equal(t.includes(s), false, '試作の理由の文は製品に無い');
}

async function safe(page, rec, csp) {
  const counts = await collectSafety(page, rec);
  assert.deepEqual(nonZero(counts), [], JSON.stringify({ csp, counts }));
  assert.equal(summarize(rec).pageErrors, 0);
}

async function withPage(csp, fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    try {
      await fn(page, srv, rec);
    } finally {
      await context.close();
    }
  }));
}

// 固定のページ内関数。行の横のはみ出し・ページの横のはみ出し・pre と表の位置を数える。教材の文字列から選択子は組み立てない。
function overflowProbe() {
  const rows = [...document.querySelectorAll('li[data-path="notes/md"], li[data-path="notes/txt"]')];
  const de = document.documentElement;
  let pre = 0;
  let preBeyond = 0;
  let preScroll = 0;
  let table = 0;
  let tableBeyond = 0;
  for (const li of rows) {
    const right = li.getBoundingClientRect().right + 1;
    for (const e of li.querySelectorAll('[data-testid="artifact-doc-body"] pre')) {
      pre += 1;
      if (e.getBoundingClientRect().right > right) preBeyond += 1;
      if (e.scrollWidth > e.clientWidth) preScroll += 1;
    }
    for (const e of li.querySelectorAll('[data-testid="artifact-doc-body"] table')) {
      table += 1;
      if (e.getBoundingClientRect().right > right) tableBeyond += 1;
    }
  }
  const body = document.querySelector('li[data-path="notes/md"] [data-testid="artifact-doc-body"]');
  const title = document.querySelector('li[data-path="notes/md"] .dr-title');
  return {
    rows: rows.length,
    liOverflow: rows.filter((li) => li.scrollWidth > li.clientWidth + 1).length,
    pageOverflow: de.scrollWidth > de.clientWidth + 1,
    pre, preBeyond, preScroll, table, tableBeyond,
    bodyWidth: body ? Math.round(body.getBoundingClientRect().width) : -1,
    bodyTop: body ? Math.round(body.getBoundingClientRect().top) : -1,
    bodyLeft: body ? Math.round(body.getBoundingClientRect().left) : -1,
    titleTop: title ? Math.round(title.getBoundingClientRect().top) : -1,
  };
}

// 配信物の写しの app.css を cssEdit で書き換え（無指定なら元のまま）、指定の幅で IR を開いて測る。幅が 960px 未満なら「成果物」のタブを選ぶ。
async function measureAt(irText, width, cssEdit) {
  const src = process.env.FANDHE_WEB_TEST_DIR;
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-doc-body-css-'));
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
        await openAndPut(page, srv, irText);
        if (width < 960) await page.locator('#pane-tab-artifacts').click();
        await page.waitForSelector(`${ROW_MD} ${BODY}`, { state: 'visible', timeout: 10000 });
        out = await page.evaluate(overflowProbe);
        await safe(page, rec, true);
      } finally {
        await context.close();
      }
    });
    return out;
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}

const WIDTHS = [1280, 960, 800, 360];
const overflowCount = (m) => m.liOverflow + (m.pageOverflow ? 1 : 0) + m.preBeyond + m.tableBeyond;

test('L6 長い行があっても行とページが横にはみ出さず、pre と表は区画の中でスクロールする', { timeout: 300000 }, async () => {
  for (const w of WIDTHS) {
    const m = await measureAt(irLong, w);
    assert.equal(m.rows, 2, `${w}px: 2 行を測った`);
    assert.ok(m.pre >= 2 && m.table >= 1, `${w}px: pre と表を測った（空振りの防止）`);
    assert.equal(overflowCount(m), 0, `${w}px: ${JSON.stringify(m)}`);
    assert.equal(m.preScroll, m.pre, `${w}px: 長い行の pre はすべて区画の中でスクロールする`);
  }
});

test('L6 短い本文の幅と位置は本文の規則の有無で変わらない', { timeout: 120000 }, async () => {
  const strip = (css) => css.replace(/\.artifact-doc-body \{ min-width: 0; \}\n/g, '').replace(/\.artifact-doc-body pre \{ overflow-x: auto; \}\n/g, '');
  for (const w of [1280, 800]) {
    const withRules = await measureAt(irShown, w);
    const without = await measureAt(irShown, w, strip);
    assert.ok(withRules.bodyWidth > 0, `${w}px: 本文を測った`);
    assert.equal(withRules.bodyWidth, without.bodyWidth, `${w}px: 幅`);
    assert.equal(withRules.bodyTop - withRules.titleTop, without.bodyTop - without.titleTop, `${w}px: 題からの縦の位置`);
    assert.equal(withRules.bodyLeft, without.bodyLeft, `${w}px: 横の位置`);
  }
});

const BODY_RULES = {
  minWidth: /\.artifact-doc-body \{ min-width: 0; \}\n/g,
  preScroll: /\.artifact-doc-body pre \{ overflow-x: auto; \}\n/g,
};
for (const [name, re] of Object.entries(BODY_RULES)) {
  test(`L7 負の対照: 規則 ${name} を外すとはみ出しを数える`, { timeout: 120000 }, async () => {
    const cssEdit = (css) => {
      assert.equal((css.match(re) ?? []).length, 1, 'rule must match exactly once');
      return css.replace(re, '');
    };
    const m = await measureAt(irLong, 1280, cssEdit);
    assert.ok(overflowCount(m) > 0, `${name}: ${JSON.stringify(m)}`);
  });
}

// L8 用の固定のページ内関数。いまのフォーカスの要素の種類・属する行・本文の区画の中か・スクロールの数値だけを返す。教材の文字列は返さない。
function focusProbe() {
  const a = document.activeElement;
  const li = a && a.closest ? a.closest('li[data-path]') : null;
  return {
    tag: a ? a.tagName : '',
    path: li ? li.getAttribute('data-path') : '',
    inBody: !!(a && a.closest && a.closest('[data-testid="artifact-doc-body"]')),
    left: a ? Math.round(a.scrollLeft) : -1,
    sw: a ? a.scrollWidth : -1,
    cw: a ? a.clientWidth : -1,
    outline: a ? getComputedStyle(a).outlineStyle : '',
  };
}

// Tab の回数。フォーカスは途中でブラウザの外（BODY）を通るので、止めずに画面の部品が一巡する以上（約 25 回）を押す。
const MAX_TABS = 45;

// 先頭から Tab を繰り返し、フォーカスが本文の区画の中の PRE に来た行ごとに、最初に届いた Tab の回数を記録する。
async function tabThrough(page) {
  await page.evaluate(() => { document.activeElement?.blur(); window.scrollTo(0, 0); });
  const reached = new Map();
  for (let i = 1; i <= MAX_TABS; i += 1) {
    await page.keyboard.press('Tab');
    const f = await page.evaluate(focusProbe);
    if (f.inBody && f.tag === 'PRE' && !reached.has(f.path)) {
      // Tab で止まったその pre のまま、矢印キーで右へ、続けて左へ動かす（動きはなめらかなスクロールなので値が変わるまで待つ）。
      await page.keyboard.press('ArrowRight');
      await page.waitForFunction(() => document.activeElement.scrollLeft > 0, null, { timeout: 5000 }).catch(() => {});
      const right = await page.evaluate(focusProbe);
      await page.keyboard.press('ArrowLeft');
      await page.waitForFunction(() => document.activeElement.scrollLeft === 0, null, { timeout: 5000 }).catch(() => {});
      const left = await page.evaluate(focusProbe);
      reached.set(f.path, { tabs: i, f, right: right.left, back: left.left, max: right.sw - right.cw });
    }
  }
  return reached;
}

// cssEdit を適用した配信物の写しで irText を開き、本文の pre への Tab の到達と、届いた pre での矢印キー・End の効きを測る。
async function keyboardAt(irText, width, cssEdit) {
  const src = process.env.FANDHE_WEB_TEST_DIR;
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-doc-body-kbd-'));
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
        await openAndPut(page, srv, irText);
        if (width < 960) await page.locator('#pane-tab-artifacts').click();
        await page.waitForSelector(`${ROW_MD} ${BODY}`, { state: 'visible', timeout: 10000 });
        const overflow = await page.evaluate(overflowProbe);
        const reached = await tabThrough(page);
        out = {
          overflow,
          reached: Object.fromEntries([...reached].map(([k, v]) => [k, v.tabs])),
          keys: Object.fromEntries([...reached].map(([k, v]) => [k, { right: v.right, back: v.back, max: v.max, outline: v.f.outline }])),
        };
        await safe(page, rec, true);
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
  test(`L8 横にスクロールする本文の pre に Tab で届き、左右の矢印キーで横に動く（${w}px）`, { timeout: 180000 }, async (t) => {
    const m = await keyboardAt(irLong, w);
    t.diagnostic(`browser=${browser.version()} channel=${channelFromEnv() ?? 'default'} width=${w} tabs=${JSON.stringify(m.reached)} keys=${JSON.stringify(m.keys)}`);
    assert.ok(m.overflow.preScroll >= 2, `${w}px: 横にスクロールする pre を測った（空振りの防止）`);
    for (const row of ['notes/txt', 'notes/md']) {
      assert.ok(m.reached[row] > 0, `${w}px: ${row} の pre に Tab で届く`);
      assert.ok(m.keys[row].max > 0 && m.keys[row].right > 0, `${w}px: ${row} 右矢印で横に動く`);
      assert.equal(m.keys[row].back, 0, `${w}px: ${row} 左矢印で元に戻る`);
    }
  });
}

test('L8 負の対照: はみ出さない短い本文の pre には Tab で止まらない', { timeout: 120000 }, async () => {
  const m = await keyboardAt(irShown, 1280);
  assert.ok(m.overflow.pre >= 1, '短い本文でも pre を測った（空振りの防止）');
  assert.equal(m.overflow.preScroll, 0, '短い本文の pre は横にはみ出さない');
  assert.deepEqual(m.reached, {}, '本文の pre に Tab で止まらない');
});

test('L8 負の対照: overflow-x の規則を外すと長い行の pre にも Tab で止まらない', { timeout: 120000 }, async () => {
  const re = BODY_RULES.preScroll;
  const cssEdit = (css) => {
    assert.equal((css.match(re) ?? []).length, 1, 'rule must match exactly once');
    return css.replace(re, '');
  };
  const m = await keyboardAt(irLong, 1280, cssEdit);
  assert.ok(overflowCount(m.overflow) > 0, `規則を外すとはみ出しを数える: ${JSON.stringify(m.overflow)}`);
  assert.deepEqual(m.reached, {}, '本文の pre に Tab で止まらない');
});

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`L1 body を省略・hidden にすると本文の区画が 0 件で理由の文も出ない（${mode}）`, { timeout: 120000 }, async () => {
    for (const [name, ir] of [['省略', irOmitted], ['hidden', irHidden]]) {
      await withPage(csp, async (page, srv, rec) => {
        await openAndPut(page, srv, ir);
        assert.equal(await page.locator(BODY).count(), 0, `${name}: 区画は 0 件`);
        assert.equal(await page.locator(`${ROW_MD}`).count(), 1, `${name}: 経路の行は出る`);
        await assertNoPrototypeReason(page);
        await safe(page, rec, csp);
      });
    }
    // 負の対照: body: shown では同じ操作で区画が出る（判定が常に 0 件を返す空振りの防止）。
    await withPage(csp, async (page, srv) => {
      await openAndPut(page, srv, irShown);
      await page.waitForSelector(`${ROW_MD} ${BODY}`, { timeout: 10000 });
    });
  });

  test(`L2 fixture を持たない要素の行には本文の区画が 0 件（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page, srv, rec) => {
      await openAndPut(page, srv, irShown);
      await page.waitForSelector(`${ROW_MD} ${BODY}`, { timeout: 10000 });
      assert.equal(await page.locator(`${ROW_BARE} ${BODY}`).count(), 0, 'fixture の無い行は区画 0 件');
      assert.equal(await page.locator(`${ROW_MD} ${BODY}`).count(), 1, '対照: fixture のある行は 1 件');
      await assertNoPrototypeReason(page);
      await safe(page, rec, csp);
      // 負の対照: 区画を注入すると 0 件の判定は不合格になる。
      await page.evaluate((s) => {
        const d = document.createElement('div');
        d.setAttribute('data-testid', 'artifact-doc-body');
        document.querySelector(s).appendChild(d);
      }, ROW_BARE);
      assert.equal(await page.locator(`${ROW_BARE} ${BODY}`).count(), 1, 'control: injected body must be counted');
    });
  });

  test(`L3 .txt は解釈されず pre > code の 1 ブロックで出る（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page, srv, rec) => {
      await openAndPut(page, srv, irShown);
      await page.waitForSelector(`${ROW_TXT} ${BODY}`, { timeout: 10000 });
      const shape = await page.evaluate(([s, b]) => {
        const body = document.querySelector(`${s} ${b}`);
        return {
          blocks: body.children.length,
          pre: body.querySelectorAll(':scope > pre > code').length,
          headings: body.querySelectorAll('h1,h2,h3,h4,h5,h6').length,
          text: body.querySelector('pre > code')?.textContent ?? '',
        };
      }, [ROW_TXT, BODY]);
      assert.equal(shape.blocks, 1, '区画の直下は 1 ブロック');
      assert.equal(shape.pre, 1, 'pre > code の 1 つ');
      assert.equal(shape.headings, 0, '見出しにならない');
      assert.equal(shape.text.startsWith('# '), true, '先頭の記号の行がそのまま文字で出る');
      // 対照: .md の行は見出しの木で出る（判定が常に見出し 0 件を返す空振りの防止）。
      assert.equal(await page.locator(`${ROW_MD} ${BODY} h1`).count(), 1);
      await safe(page, rec, csp);
      // 負の対照: 区画の中へ h1 を注入すると見出し 0 件の判定は不合格になる。
      await page.evaluate(([s, b]) => {
        document.querySelector(`${s} ${b}`).appendChild(document.createElement('h1'));
      }, [ROW_TXT, BODY]);
      assert.equal(await page.locator(`${ROW_TXT} ${BODY} h1`).count(), 1, 'control: injected h1 must be counted');
    });
  });

  test(`L4 文書以外のビューでは区画が 0 件になり、戻すと戻る（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page, srv, rec) => {
      await openAndPut(page, srv, irShown);
      await page.waitForSelector(`${ROW_MD} ${BODY}`, { timeout: 10000 });
      const payloads = await page.evaluate((b) => [...document.querySelectorAll(b)].map((x) => x.dataset.payload), SELECT);
      assert.deepEqual(payloads, ['document', 'file_tree'], '選択のボタンは宣言した 2 種');
      const total = () => page.locator(BODY).count();
      assert.equal(await total(), 2, 'md と txt の 2 行に区画');

      await page.locator(`${SELECT}[data-payload="file_tree"]`).click();
      await page.waitForFunction((b) => document.querySelector(`${b}[data-payload="file_tree"]`).dataset.selected === 'true', SELECT);
      assert.equal(await total(), 0, 'ファイルツリーでは区画 0 件');
      await assertNoPrototypeReason(page);

      await page.locator(`${SELECT}[data-payload="document"]`).click();
      await page.waitForSelector(`${ROW_MD} ${BODY}`, { timeout: 10000 });
      assert.equal(await total(), 2, '文書に戻すと 2 件');
      assert.equal(await page.locator(`${ROW_MD} ${BODY} h1`).count(), 1);
      await safe(page, rec, csp);
      // 負の対照: 区画を注入すると 0 件の判定は不合格になる。
      await page.locator(`${SELECT}[data-payload="file_tree"]`).click();
      await page.waitForFunction((b) => document.querySelector(`${b}[data-payload="file_tree"]`).dataset.selected === 'true', SELECT);
      await page.evaluate(() => {
        const d = document.createElement('div');
        d.setAttribute('data-testid', 'artifact-doc-body');
        document.querySelector('[data-testid="part-artifact-view"]').appendChild(d);
      });
      assert.equal(await total(), 1, 'control: injected body must be counted');
    });
  });

  test(`L5 replace で本文が替わっても前後の印は出ず、替わった後の木だけが出る（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page, srv, rec) => {
      await openAndPut(page, srv, irShown);
      await page.waitForSelector(`${ROW_MD} ${BODY} h1`, { timeout: 10000 });
      const first = await page.locator(`${ROW_MD} ${BODY} h1`).textContent();
      await approve(page);
      await page.waitForFunction(([s, f]) => {
        const h = document.querySelector(`${s} [data-testid="artifact-doc-body"] h1`);
        return h && h.textContent !== f;
      }, [ROW_MD, first], { timeout: 10000 });
      const marks = () => page.evaluate(([s, b]) => document.querySelectorAll(`${s} ${b} del, ${s} ${b} ins, ${s} ${b} [data-mark]`).length, [ROW_MD, BODY]);
      assert.equal(await page.locator(`${ROW_MD} ${BODY}`).count(), 1, '区画は 1 件のまま');
      assert.equal(await page.locator(`${ROW_MD} ${BODY} h1`).count(), 1, '替わった後の見出しだけ');
      assert.equal(await page.locator(`${ROW_MD} ${BODY}`).evaluate((e, f) => e.textContent.includes(f), first), false, '替わる前の見出しの文字は残らない');
      assert.equal(await marks(), 0, '前後の印は 0 件');
      await safe(page, rec, csp);
      // 負の対照: 区画の中へ data-mark 付きの要素を注入すると 0 件の判定は不合格になる。
      await page.evaluate(([s, b]) => {
        const d = document.createElement('span');
        d.setAttribute('data-mark', 'changed');
        document.querySelector(`${s} ${b}`).appendChild(d);
      }, [ROW_MD, BODY]);
      assert.equal(await marks(), 1, 'control: injected mark must be counted');
    });
  });
}
