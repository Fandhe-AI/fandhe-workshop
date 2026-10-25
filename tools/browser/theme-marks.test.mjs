// 4 つの外観（standard・mono・high_contrast・reading）のどれでも、左のビューの変化の印（記号 + - ~ と文字 【追加】【削除】【変更】）が
// 残ることの試験（PACK-21。TASK-481 2/2）。CSP あり・なしの両方で流す。
//
// 製品の画面は `data-theme` を付けず、外観ごとの CSS も無い（docs/operation-profile.md の 2.2）。そのため主の検査は、試験が作った
// `<main data-theme="X">` の中へ代役の部品（TEST_HOOKS=1 の view_fixture）をマウントして行う（代役の検査）。属性は試験が作った要素に付けるので、
// 描画の同期で消えない。実物の app.css が読み込まれた状態で見るので、外観ごとの CSS が足されて印を隠せば検出する。
//   A1 外観 4 値 × 許される 12 通り × 前後で、印の宿主の記号・文字が表（MARK_TABLE）のとおりで、実際に見えている
//      （checkVisibility・display・visibility・opacity・font-size・色の alpha・寸法）。前後で 3 種の印が 1 回以上数えられる（空振りの防止）
//   A2 外観の違いで DOM が変わらない（4 値の innerHTML が同じ）。属性は前後・種類の切り替えの後も残る
//   C1 負の対照。`[data-theme="mono"]` の子孫に印を隠す規則（5 種）を足すと mono だけで期待の違反が出て、他の外観と空の規則では 0 件
//   P1 製品の画面。外観つきの教材を実物の `fandhe-pack compile` で IR にして読み込み、承認の後の左の成果物の印が表のとおりで見えている。
//      印の並びは 4 値で同じ。`#app` の `data-theme` の有無は diagnostic に残すだけで assert しない（枠の結線で落ちないため）。あれば値は教材と一致する
//   P2 CSP 違反・ページ内エラー・配信元以外への通信が 0 件
// 対照の規則は構成した stylesheet（adoptedStyleSheets）で足す。配信物・CSP は変えない。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_PACK_BIN（前提バイナリ）。未設定・不在は失敗にする。
// 限界: 製品の画面に `data-theme` が付かない間、A・C は代役の検査である。`reading` の文字の大きさ・行間は確かめない（枠の担当）。
//   製品の枠の根は `div[data-testid="workshop-root"]` で `main` ではない。どの要素に付けるかは枠の担当が決める（この試験は子孫の選択子で足す）。
// AI による代理の確認であり、人の評価には数えない。
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
import { CASES, MARK_TABLE } from './artifact-view-check.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const BASE = '/fandhe-theme-marks/';
const PACK = path.resolve(HERE, '../../crates/web/tests/data/doc_body_limits_pack');
const PACK_LINE = "vocab: '1.4'}";
const APPROVE = '[data-testid="approval-btn"][data-decision="approve"]';
// crates/contract/src/appearance.rs の Theme の綴りの写し。
const THEMES = ['standard', 'mono', 'high_contrast', 'reading'];
const ROOT = '#view-fixture-root';
const ALLOWED = CASES.filter((c) => c.allowed);

// 対照の規則（固定の表）。expect は pageViolations の固定の名前と対応する。
const NEGATIVE = [
  { rule: '.mk-glyph { display: none }', expect: 'display' },
  { rule: '.mk-text { visibility: hidden }', expect: 'visibility' },
  { rule: '.mk-glyph { color: transparent }', expect: 'color-alpha' },
  { rule: '.mk-text { font-size: 0 }', expect: 'font-size' },
  { rule: '.mk-glyph { opacity: 0 }', expect: 'opacity' },
];

let browser;
const irs = new Map();

function compile(bin, theme) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-theme-marks-'));
  try {
    fs.cpSync(PACK, work, { recursive: true });
    const y = path.join(work, 'workshop.yaml');
    const text = fs.readFileSync(y, 'utf8');
    const next = text.replace(PACK_LINE, `vocab: '1.4', appearance: {theme: ${theme}}}`);
    assert.notEqual(next, text, 'mutation applied');
    fs.writeFileSync(y, next, 'utf8');
    const r = spawnSync(bin, ['compile', work], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    assert.equal(r.status, 0, `compile exit ${r.status}`);
    assert.equal(JSON.parse(r.stdout).pack.appearance?.theme, theme, 'IR carries the theme');
    return r.stdout;
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  const bin = packBinFromEnv();
  for (const t of THEMES) irs.set(t, compile(bin, t));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// ページ内で使う固定の検査。選択子は引数で渡し、文字列から組み立てない。違反は固定の英語の名前の配列で返す。
// 対象は data-mark が空でない宿主のうち、hidden の祖先を持たないものの記号・文字。
function pageViolations([scope, table]) {
  const out = { marks: [], violations: [] };
  const hosts = [...document.querySelectorAll(`${scope} [data-mark]`)]
    .filter((h) => h.getAttribute('data-mark') !== '' && !h.closest('[hidden]'));
  const alpha = (c) => {
    const m = /^rgba?\(([^)]*)\)$/.exec(c);
    if (m) {
      const p = m[1].split(/[,/\s]+/).filter(Boolean);
      return p.length >= 4 ? Number(p[3]) : 1;
    }
    const k = /\/\s*([\d.]+)\s*\)$/.exec(c);
    return k ? Number(k[1]) : 1;
  };
  for (const h of hosts) {
    const mark = h.getAttribute('data-mark');
    out.marks.push(mark);
    const glyphs = h.querySelectorAll('[data-testid="mark-glyph"]');
    const texts = h.querySelectorAll('[data-testid="mark-text"]');
    if (glyphs.length !== 1 || texts.length !== 1) { out.violations.push('structure'); continue; }
    const want = table[mark];
    if (!want) { out.violations.push('unknown-mark'); continue; }
    if (glyphs[0].textContent !== want[0]) out.violations.push('glyph-text');
    if (texts[0].textContent !== want[1]) out.violations.push('mark-text');
    for (const el of [glyphs[0], texts[0]]) {
      const cs = getComputedStyle(el);
      const r = el.getBoundingClientRect();
      if (!el.checkVisibility({ opacityProperty: true, visibilityProperty: true, contentVisibilityAuto: true })) out.violations.push('check-visibility');
      if (cs.display === 'none') out.violations.push('display');
      if (cs.visibility !== 'visible') out.violations.push('visibility');
      if (!(Number(cs.opacity) > 0)) out.violations.push('opacity');
      if (!(parseFloat(cs.fontSize) > 0)) out.violations.push('font-size');
      if (!(alpha(cs.color) > 0)) out.violations.push('color-alpha');
      if (!(r.width > 0 && r.height > 0)) out.violations.push('size');
    }
  }
  out.violations = [...new Set(out.violations)];
  return out;
}

async function dispatch(page, name, payload) {
  const r = JSON.parse(await page.evaluate(([n, p]) => window.__fandheTest.viewFixtureDispatch(n, p), [name, payload]));
  assert.equal(r.outcome, 'applied', `${name} ${payload}`);
}

// 試験が作った main[data-theme] の中へ代役の部品をマウントして fn に渡す。
async function withThemedFixture(csp, theme, fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal(await page.evaluate(() => typeof window.__fandheTest?.viewFixtureMount), 'function', 'test hooks are missing');
      await page.evaluate((t) => {
        const main = document.createElement('main');
        main.setAttribute('data-theme', t);
        const box = document.createElement('div');
        box.id = 'view-fixture-root';
        main.append(box);
        document.body.append(main);
      }, theme);
      assert.deepEqual(JSON.parse(await page.evaluate(() => window.__fandheTest.viewFixtureMount('view-fixture-root'))), { ok: true });
      await fn(page, rec);
    } finally {
      await context.close();
    }
  }));
}

const inspect = (page, scope) => page.evaluate(pageViolations, [scope, MARK_TABLE]);
const themeAttr = (page) => page.evaluate(() => document.querySelector('main[data-theme]')?.getAttribute('data-theme') ?? null);

// 12 通り × 前後を流し、違反と印の種類と DOM の写しを返す。
async function sweep(page, theme) {
  const violations = [];
  const seen = new Set();
  const dom = [];
  for (const { kind, variant } of ALLOWED) {
    await dispatch(page, 'fixture_select', `${kind}:${variant}`);
    for (const phase of ['before', 'after']) {
      await dispatch(page, 'fixture_phase', phase);
      const o = await inspect(page, ROOT);
      for (const v of o.violations) violations.push(`${kind}/${variant}/${phase}: ${v}`);
      if (phase === 'after') for (const m of o.marks) seen.add(m);
      dom.push(await page.evaluate((s) => document.querySelector(s).innerHTML, ROOT));
      assert.equal(await themeAttr(page), theme, `${kind}/${variant}/${phase}: theme attribute stays`);
    }
  }
  return { violations, seen: [...seen].sort(), dom };
}

async function addRule(page, text) {
  await page.evaluate((t) => {
    const s = new CSSStyleSheet();
    s.replaceSync(t);
    document.adoptedStyleSheets = [...document.adoptedStyleSheets, s];
  }, text);
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`A1・A2 4 つの外観の 12 通り × 前後で印の記号・文字が残り見えている。DOM は外観で変わらない（${mode}）`, { timeout: 300000 }, async () => {
    let baseline;
    for (const theme of THEMES) {
      await withThemedFixture(csp, theme, async (page, rec) => {
        const r = await sweep(page, theme);
        assert.deepEqual(r.violations, [], `${theme}: 印の違反`);
        assert.deepEqual(r.seen, ['add', 'change', 'remove'], `${theme}: 3 種の印が出る`);
        baseline ??= r.dom;
        assert.deepEqual(r.dom, baseline, `${theme}: DOM が standard と同じ`);
        assert.deepEqual(nonZero(await collectSafety(page, rec)), [], `${theme}: ${mode}`);
      });
    }
  });

  test(`C1 負の対照。mono の子孫に印を隠す規則を足すと mono だけで期待の違反が出る（${mode}）`, { timeout: 300000 }, async () => {
    for (const { rule, expect } of NEGATIVE) {
      await withThemedFixture(csp, 'mono', async (page) => {
        await dispatch(page, 'fixture_select', 'card_list:b');
        await dispatch(page, 'fixture_phase', 'after');
        assert.deepEqual((await inspect(page, ROOT)).violations, [], 'precondition');
        await addRule(page, `[data-theme="mono"] ${rule}`);
        const v = (await inspect(page, ROOT)).violations;
        assert.ok(v.includes(expect), `${rule}: ${expect} を検出する: ${v}`);
      });
      await withThemedFixture(csp, 'standard', async (page) => {
        await dispatch(page, 'fixture_select', 'card_list:b');
        await dispatch(page, 'fixture_phase', 'after');
        await addRule(page, `[data-theme="mono"] ${rule}`);
        assert.deepEqual((await inspect(page, ROOT)).violations, [], `${rule}: standard では 0 件`);
      });
    }
    await withThemedFixture(csp, 'mono', async (page) => {
      await dispatch(page, 'fixture_select', 'card_list:b');
      await dispatch(page, 'fixture_phase', 'after');
      await addRule(page, '');
      assert.deepEqual((await inspect(page, ROOT)).violations, [], '空の規則では 0 件');
    });
  });

  test(`P1・P2 製品の画面で外観つきの教材の左の成果物に印が残る（${mode}）`, { timeout: 300000 }, async () => {
    let baseline;
    for (const theme of THEMES) {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
        try {
          await page.goto(srv.url);
          await waitMounted(page);
          const n0 = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.installCount);
          assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), irs.get(theme)), true, 'load-ir sent');
          await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
          await page.waitForSelector(APPROVE, { timeout: 10000 });
          await page.click(APPROVE);
          await page.waitForSelector('#app li[data-path="notes/md"]', { timeout: 10000 });
          const o = await inspect(page, '#app');
          assert.deepEqual(o.violations, [], `${theme}: 印の違反`);
          assert.ok(o.marks.length >= 1, `${theme}: 印が 1 件以上出る`);
          baseline ??= JSON.stringify(o.marks);
          assert.equal(JSON.stringify(o.marks), baseline, `${theme}: 印の並びが外観で変わらない`);
          // 枠が data-theme を結線したかは diagnostic に残すだけ。付いていれば値は教材と一致する。
          const attrs = await page.evaluate(() => [...document.querySelectorAll('#app [data-theme]')].map((e) => e.getAttribute('data-theme')));
          console.log(`# diagnostic: ${theme}: #app 内の data-theme は ${attrs.length} 件 ${JSON.stringify(attrs)}`);
          for (const a of attrs) assert.equal(a, theme, 'data-theme matches the pack');
          const counts = await collectSafety(page, rec);
          assert.deepEqual(nonZero(counts), [], JSON.stringify({ csp, counts }));
          assert.equal(summarize(rec).pageErrors, 0);
        } finally {
          await context.close();
        }
      }));
    }
  });
}
