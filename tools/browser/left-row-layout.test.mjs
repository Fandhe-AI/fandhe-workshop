// 左エリアの文書の行の語の途中の折り返しと、カードの先頭の空の行の試験（#2087。5.476）。
//
// 実教材の通し（choice-journey の設定ファイル。文書は review-request-derived、カードは task-breakdown-cards）を配信する設定の画面で流し、
// 左の領域を測った全時点（runJourney の probe）で行の見え方を固定する。教材の語はこの試験に書かない（設定ファイルにある）。
// - LR1: 文書（幅 1280px・960px）。題・状態が 1 行以下、補足が題と同じ行に並ぶ、行の横のはみ出し 0
// - LR2: 文書（幅 800px）。題・状態・補足がどれも 1 行以下で同じ行に並ぶ（見え方を変えていない）
// - LR3: 文書（幅 360px）。題・状態が 1 行以下、はみ出し 0
// - LR4: 文書の属性を長い文字（空白の無い英字・日本語の繰り返し）に置き換えた IR。幅 1280px・800px ではみ出し 0
// - LR5: カード（幅 1280px・800px）。黒丸なし、印の無い行は上の空きが小さく印の 2 つの span が出ない、印のある行は記号と文字が見える。両方の行を測る
// - LR6: 負の対照。写しの app.css から足した規則を 1 回ずつ取り除くと、同じ測定が違反を数える
// 各場面で完走・安全性の計数 0・配信元以外への通信 0・ページ内エラー 0 を確かめる。閾値は font-size に対する相対値。出力は件数と合否だけ。
// 限界: 基準端末では測っていない。人の目の確認の代わりにはならない。
// 入力: FANDHE_PACK_BIN・FANDHE_WEB_AUTHOR_DIR。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer,
} from './harness.mjs';
import { stepsOf, prepareDelivery, runJourney } from './choice-journey.mjs';
import { loadConfig, buildIr } from './choice-journey-config.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const cfgPath = (name) => path.join(HERE, 'choice-journey', `${name}.json`);
const BASE = '/fandhe-left-row-layout/';

let browser;
let bin;
let webDir;

before(async () => {
  bin = packBinFromEnv();
  webDir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(webDir && fs.statSync(webDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a delivery web directory');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 固定のページ内の関数。左の領域の文書・カードの各行を測る。教材の文字列から選択子は組み立てない。
function probeInPage() {
  const lines = (el) => {
    if (!el) return 0;
    const r = document.createRange();
    r.selectNodeContents(el);
    return new Set([...r.getClientRects()].filter((x) => x.width > 0).map((x) => Math.round(x.top))).size;
  };
  const docs = [];
  for (const li of document.querySelectorAll('[data-view="document"] > li[data-kind="item"]')) {
    const b = li.getBoundingClientRect();
    if (b.width === 0) continue;
    const t = li.querySelector('.dr-title');
    const s = li.querySelector('.dr-state');
    const d = li.querySelector('.dr-detail');
    const tb = t.getBoundingClientRect();
    const db = d.getBoundingClientRect();
    docs.push({
      t: lines(t),
      s: lines(s),
      d: lines(d),
      detailEmpty: (d.textContent ?? '').trim() === '',
      sameRow: db.top < tb.bottom - 1,
      overflow: li.scrollWidth > li.clientWidth + 1,
    });
  }
  const cards = [];
  for (const li of document.querySelectorAll('[data-view="card_list"] > li[data-kind="card"]')) {
    const b = li.getBoundingClientRect();
    if (b.width === 0) continue;
    const a = li.querySelector('article');
    const g = li.querySelector('.mk-glyph');
    const x = li.querySelector('.mk-text');
    const fs = parseFloat(getComputedStyle(li).fontSize);
    const marked = (li.getAttribute('data-mark') ?? '') !== '';
    cards.push({
      marked,
      bullet: getComputedStyle(li).listStyleType !== 'none',
      gap: (a.getBoundingClientRect().top - b.top) / fs,
      markShown: [g, x].every((e) => e && getComputedStyle(e).display !== 'none' && e.getBoundingClientRect().width > 0),
      markHidden: [g, x].every((e) => !e || getComputedStyle(e).display === 'none'),
    });
  }
  return {
    docs,
    cards,
    pageOverflow: document.documentElement.scrollWidth > document.documentElement.clientWidth + 1,
  };
}

// 設定の通しを流し、probe の結果を全時点ぶん集めて返す。irEdit は IR の JSON（文字列）を書き換える関数、cssEdit は配信物の app.css の書き換え。
async function journey(config, { width, irEdit = (x) => x, cssEdit = null }) {
  const irText = irEdit(buildIr({ bin, config }));
  const ir = JSON.parse(irText);
  const info = stepsOf(ir);
  const copy = prepareDelivery({ webDir, irText });
  try {
    if (cssEdit) {
      const f = path.join(copy.dir, 'app.css');
      fs.writeFileSync(f, cssEdit(fs.readFileSync(f, 'utf8')));
    }
    let out;
    await withServer({ dir: copy.dir, base: BASE }, async (srv) => {
      out = await runJourney({
        browser, srv, ir, sequence: config.sequence, expect: config.sequence.map((s) => s.expect), initial: config.initial,
        titles: info.titles, finals: info.finals, viewport: { width, height: 800 }, probe: (page) => page.evaluate(probeInPage),
      });
    });
    assert.equal(out.completed, true, 'journey must complete');
    assert.deepEqual(out.safety, []);
    assert.equal(out.external, 0);
    assert.equal(out.pageErrors, 0);
    const probes = [...out.left.ops.map((o) => o.probe), out.left.endProbe].filter(Boolean);
    return probes;
  } finally {
    copy.cleanup();
  }
}

const rowsOf = (probes, k) => probes.flatMap((p) => p[k]);

// 文書の違反の数。wide は補足が題と同じ行に並ぶことを求める場面、single は補足も 1 行を求める場面。
function docViolations(probes, { sameRow, single }) {
  let n = 0;
  for (const p of probes) {
    if (p.pageOverflow) n += 1;
    for (const r of p.docs) {
      if (r.t > 1 || r.s > 1 || r.overflow) n += 1;
      if (sameRow && !r.detailEmpty && !r.sameRow) n += 1;
      if (single && r.d > 1) n += 1;
    }
  }
  return n;
}

function cardViolations(probes) {
  let n = 0;
  for (const r of rowsOf(probes, 'cards')) {
    if (r.bullet) n += 1;
    if (r.marked ? !r.markShown : (!r.markHidden || r.gap > 0.25)) n += 1;
  }
  return n;
}

const docConfig = () => loadConfig(cfgPath('review-request-derived'));
const cardConfig = () => loadConfig(cfgPath('task-breakdown-cards'));

test('LR1 文書: 幅 1280px・960px で題・状態が語の途中で折れず、補足は同じ行の列', async () => {
  for (const width of [1280, 960]) {
    const probes = await journey(docConfig(), { width });
    assert.ok(rowsOf(probes, 'docs').length > 0, 'measured some rows');
    assert.ok(rowsOf(probes, 'docs').some((r) => !r.detailEmpty), 'measured rows with detail');
    assert.equal(docViolations(probes, { sameRow: true, single: false }), 0, `width ${width}`);
  }
});

test('LR2 文書: 幅 800px で題・状態・補足がどれも 1 行で同じ行に並ぶ', async () => {
  const probes = await journey(docConfig(), { width: 800 });
  assert.ok(rowsOf(probes, 'docs').length > 0);
  assert.equal(docViolations(probes, { sameRow: true, single: true }), 0);
});

test('LR3 文書: 幅 360px で題・状態が 1 行以下ではみ出さない', async () => {
  const probes = await journey(docConfig(), { width: 360 });
  assert.ok(rowsOf(probes, 'docs').length > 0);
  assert.equal(docViolations(probes, { sameRow: false, single: false }), 0);
});

// 属性の題・状態を長い文字へ置き換える（attrs の中だけ。Step の題は観察に使うので触らない）。
function longAttrs(irText) {
  const long = { title: 'あ'.repeat(120), state: `${'x'.repeat(200)}` };
  const walk = (v) => {
    if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === 'object') {
      for (const [k, x] of Object.entries(v)) {
        if (k === 'attrs' && x && typeof x === 'object') {
          if ('title' in x) x.title = long.title;
          if ('state' in x) x.state = long.state;
        } else walk(x);
      }
    }
  };
  const ir = JSON.parse(irText);
  walk(ir);
  return JSON.stringify(ir);
}

test('LR4 文書: 長い題・状態でも行・ページが横にはみ出さない', async () => {
  for (const width of [1280, 800]) {
    const probes = await journey(docConfig(), { width, irEdit: longAttrs });
    assert.ok(rowsOf(probes, 'docs').length > 0);
    let n = 0;
    for (const p of probes) {
      if (p.pageOverflow) n += 1;
      for (const r of p.docs) if (r.overflow) n += 1;
    }
    assert.equal(n, 0, `width ${width}`);
  }
});

test('LR5 カード: 黒丸が無く、印の無い行は空の印の行が出ず、印のある行は印が見える', async () => {
  for (const width of [1280, 800]) {
    const probes = await journey(cardConfig(), { width });
    const cards = rowsOf(probes, 'cards');
    assert.ok(cards.some((r) => r.marked), 'measured marked rows');
    assert.ok(cards.some((r) => !r.marked), 'measured unmarked rows');
    assert.equal(cardViolations(probes), 0, `width ${width}`);
  }
});

// 取り除く規則（各 1 回だけ一致すること）。
const STRIPS = {
  docWrap: [/\[data-view="document"\] > li\[data-kind="item"\] \{ flex-wrap: wrap; \}\n/g, 'doc'],
  docDetail: [/\.dr-detail \{ flex: 1 1 8rem; min-width: 0; overflow-wrap: anywhere; \}\n/g, 'doc'],
  cardBullet: [/\[data-view="card_list"\] > li\[data-kind="card"\] \{ list-style: none; \}\n/g, 'card'],
  cardMark: [/\[data-view="card_list"\] > li\[data-kind="card"\]\[data-mark=""\] > \.mk-glyph,\n\[data-view="card_list"\] > li\[data-kind="card"\]\[data-mark=""\] > \.mk-text \{ display: none; \}\n/g, 'card'],
};

for (const [name, [re, kind]] of Object.entries(STRIPS)) {
  test(`LR6 負の対照: 規則 ${name} を取り除くと違反を数える`, async () => {
    const cssEdit = (css) => {
      assert.equal((css.match(re) ?? []).length, 1, 'rule must match exactly once');
      return css.replace(re, '');
    };
    if (kind === 'doc') {
      // 折り返しの規則は、1 行に入らない長い題・状態のときに効く（実教材の行は幅 1280px なら入る）。
      const probes = await journey(docConfig(), { width: 1280, cssEdit, irEdit: longAttrs });
      assert.ok(docViolations(probes, { sameRow: true, single: false }) > 0);
    } else {
      const probes = await journey(cardConfig(), { width: 1280, cssEdit });
      assert.ok(cardViolations(probes) > 0);
    }
  });
}
