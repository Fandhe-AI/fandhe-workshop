// 誤答の後に前後の比較を自動では見せない現状の観察の固定（ART-19。TASK-488。D-69・TASK-509 の決定）。CSP あり・なしの両方で流す。
//
// 解釈: TASK-509（2026-10-31）は D-69 を追認し、(1) 前後比較は自動で開かない、(2) 拒否文で前後の切り替え（ART-2）を案内する、
// (3) 語彙には足さない（PACK-36）、を決めた。本タスクは「足さない」側なので、実装は増やさず現状の観察だけを固定する。
// ここで assert するのは「今こう見える」であり、望ましい姿ではない。ART-19 の見直しの判断は人の承認点で、この試験は決めない。
// 製品の画面には前後の切り替え（ART-2）の口が無い（`Phase` は試験の設定の代役の部品 `view_fixture` にだけ結線されている。
// docs/views.md・docs/known-gaps.md PACK-36）。ART-12（ビューの種類の選択）と ART-2 は別物で、ボタンを押せたことを ART-2 の証拠にしない。
// ART-2 が製品に結線されると O3 が落ちる。落ちたときは観察と known-gaps の記述を見直す目印にする。
//
// 観察（各項目に負の対照を付け、判定の空振りを防ぐ）:
//   O1 誤答の拒否の行が 1 件出て、理由と次の行動が IR の `message`・`next_action`（前後の切り替えの案内）と一致し見える。
//      対照: 案内の文面を変えた変種の IR では、元の文面との一致が不合格になる
//   O2 拒否の前後で、ビューの選択・7 本の一覧の hidden・左の行と印が一致する（比較のビューは自動では開かない）。
//      対照 A: 選択のボタンを実際に押すと選択が変わる。対照 B: 差分を選んで戻しても拒否の行の件数は変わらない
//   O3 前後を切り替える操作要素が #app の中に 0 件。対照: 同じ属性の要素を注入すると 1 件になる
//   O4 誤答の前後で、受講者が左で見られる手がかりは行と変化の印だけで、2 行とも同じ印（add）。変えた箇所を印で見分けられない
//   O5 拒否の後も選択肢は残り、正答で達成になり次の Step へ進む。拒否の行は増えない。対照: 正答の前は達成が 0 件
// 安全: 各場面で collectSafety の計数が 0・ページ内エラー 0。失敗の文面に本文の中身は出さない（件数・名前・合否だけ）。
//
// 確かめないこと: 配信する設定（`loadIr` は試験の設定にだけある）、新しい章の画面（教材が揃った後に TASK-643・TASK-713 で流す）、
// 人の評価（0 件。D-34）。
//
// 教材: crates/pack/tests/data/wrong-answer-compare-pack を一時ディレクトリへ写して実物の `fandhe-pack compile` で IR にする。
// その教材の on_enter の put は画面の左に行を出さない（開始の Step で警告が 1 件出る。原因は未調査）ため、写しの上で put を
// 規則の effects へ移して行を出す。書き込むパスは一時ディレクトリの固定のパスだけで、教材の値からパスを作らない。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_PACK_BIN。未設定・不在は skip にせず失敗にする。
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
import { CHOICE, REJECTED, engine } from './ch0-3-journey.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PACK = path.resolve(HERE, '../../crates/pack/tests/data/wrong-answer-compare-pack');
const BASE = '/fandhe-wrong-answer-compare/';
const VIEW = '[data-testid="part-artifact-view"]';
const SELECT = '[data-testid="view-select-button"]';
const PHASE_CONTROLS = '#app [data-action="fixture_phase"], #app [data-payload="before"], #app [data-payload="after"], #app [data-phase]';
const ON_ENTER = '    on_enter: [{put: {path: f/alpha, attrs: {k: v}}}, {put: {path: f/beta, attrs: {k: v}}}]\n';
const PUTS = '{put: {path: f/alpha, attrs: {k: v}}}, {put: {path: f/beta, attrs: {k: v}}}, ';
const NEXT_ACTION_VARIANT = '別の案内の文面にした変種。';

let browser;
let irText;
let irVariant;
let ir;

function compile(bin, variant) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-wrong-answer-compare-'));
  try {
    fs.cpSync(PACK, work, { recursive: true });
    const ch = path.join(work, 'chapters/c1/chapter.yaml');
    const base = fs.readFileSync(ch, 'utf8');
    const moved = base.replace(ON_ENTER, '').replace('effects: [{set_attr', `effects: [${PUTS}{set_attr`);
    assert.notEqual(moved, base, 'puts moved');
    fs.writeFileSync(ch, moved, 'utf8');
    if (variant) {
      const y = path.join(work, 'workshop.yaml');
      const wb = fs.readFileSync(y, 'utf8');
      const wv = wb.replace(/next_action: [^}]*\}/, `next_action: ${NEXT_ACTION_VARIANT}}`);
      assert.notEqual(wv, wb, 'variant applied');
      fs.writeFileSync(y, wv, 'utf8');
    }
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
  irText = compile(bin, false);
  irVariant = compile(bin, true);
  ir = JSON.parse(irText);
  assert.equal(JSON.parse(irVariant).rejections['rj-unchanged'].next_action, NEXT_ACTION_VARIANT);
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const uidOf = (id) => ir.nodes.find((n) => n.step?.id === id).uid;
const REJECTION = () => ir.rejections['rj-unchanged'];

async function load(page, text) {
  const n0 = await page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine.installCount);
  assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), text), true, 'load-ir sent');
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
}

const choose = (page, id) => page.click(`${CHOICE}[data-choice="${id}"]`);

// 画面の観察。属性と件数だけを返し、本文は返さない。
const snap = (page) => page.evaluate(([view, sel, rejected]) => ({
  selected: [...document.querySelectorAll(sel)].map((b) => `${b.dataset.payload}:${b.dataset.selected}`),
  hidden: [...document.querySelectorAll(`${view} > ul[data-view]`)].map((u) => `${u.dataset.view}:${u.hidden}`),
  rows: [...document.querySelectorAll(`${view} li[data-row]`)].map((l) => `${l.dataset.row}:${l.dataset.mark ?? ''}`),
  rejected: document.querySelectorAll(rejected).length,
}), [VIEW, SELECT, REJECTED]);

// 開始 → 変更 → pick の入力 → 確認待ち。誤答の直前の画面で止める。
async function toConfirm(page, srv, text) {
  await page.goto(srv.url);
  await waitMounted(page);
  await load(page, text);
  await page.waitForSelector(CHOICE, { timeout: 15000 });
  await choose(page, 'opt-edit');
  await page.waitForFunction((u) => JSON.parse(window.__fandheTest.stateJson()).engine.current === u, uidOf('pick'), { timeout: 15000 });
  await choose(page, 'pick-alpha');
  await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.stepStatus === 'accepted', null, { timeout: 15000 });
}

async function wrong(page) {
  await choose(page, 'pick-alpha');
  await page.waitForSelector(REJECTED, { timeout: 15000 });
}

async function withPage(csp, fn, text = irText) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    try {
      await toConfirm(page, srv, text);
      await fn(page, rec);
      const counts = await collectSafety(page, rec);
      assert.deepEqual(nonZero(counts), [], JSON.stringify({ csp, counts }));
      assert.equal(summarize(rec).pageErrors, 0);
    } finally {
      await context.close();
    }
  }));
}

const lineTexts = (page) => page.evaluate((rej) => {
  const li = document.querySelector(rej);
  return {
    reason: li.querySelector('[data-testid="line-reason"]')?.textContent ?? null,
    next: li.querySelector('[data-testid="line-next-action"]')?.textContent ?? null,
  };
}, REJECTED);

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`O1 誤答の拒否の行が前後の切り替えの案内を IR のとおりに出す（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page) => {
      assert.equal(await page.locator(REJECTED).count(), 0, '誤答の前は拒否の行が 0 件');
      await wrong(page);
      assert.equal(await page.locator(REJECTED).count(), 1);
      const t = await lineTexts(page);
      assert.equal(t.reason, REJECTION().message, '理由が IR と一致');
      assert.equal(t.next, REJECTION().next_action, '次の行動（案内）が IR と一致');
      assert.equal(await page.locator(`${REJECTED} [data-testid="line-next-action"]`).isVisible(), true, '案内が見える');
    });
    // 負の対照: 案内を変えた変種では、元の文面との一致が不合格になる。
    await withPage(csp, async (page) => {
      await wrong(page);
      const t = await lineTexts(page);
      assert.notEqual(t.next, REJECTION().next_action, '変種は元の案内と一致しない');
      assert.equal(t.next, NEXT_ACTION_VARIANT, '変種の案内が出る');
    }, irVariant);
  });

  test(`O2 拒否の後も比較のビューは自動では開かず、画面の選択・行・印が変わらない（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page) => {
      const before = await snap(page);
      assert.equal(before.rejected, 0);
      assert.ok(before.selected.includes('file_tree:true'), '開始のビューはファイル');
      await wrong(page);
      const after = await snap(page);
      assert.equal(after.rejected, 1);
      assert.deepEqual({ ...after, rejected: 0 }, { ...before, rejected: 0 }, '選択・hidden・行・印が拒否の前後で一致');
      assert.ok(after.hidden.includes('diff:true'), '差分の一覧は隠れたまま');
      // 対照 A: 実際に押せば選択が変わる（上の「変わらない」が何も見ていないわけではない）。
      await page.click(`${SELECT}[data-payload="diff"]`);
      await page.waitForFunction((s) => document.querySelector(`${s}[data-payload="diff"]`).dataset.selected === 'true', SELECT);
      const switched = await snap(page);
      assert.notDeepEqual(switched.selected, after.selected, '押すと選択が変わる');
      assert.ok(switched.hidden.includes('diff:false'), '押すと差分の一覧が開く');
      // 対照 B: 戻しても拒否の行は増減しない。
      await page.click(`${SELECT}[data-payload="file_tree"]`);
      await page.waitForFunction((s) => document.querySelector(`${s}[data-payload="file_tree"]`).dataset.selected === 'true', SELECT);
      assert.equal((await snap(page)).rejected, 1, '切り替えても拒否の行は 1 件のまま');
    });
  });

  test(`O3 製品の画面に前後の切り替え（ART-2）の操作要素が 0 件（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page) => {
      assert.equal(await page.locator(PHASE_CONTROLS).count(), 0, '誤答の前');
      await wrong(page);
      assert.equal(await page.locator(PHASE_CONTROLS).count(), 0, '誤答の後');
      // 負の対照: 同じ属性の要素を属性だけで注入すると検出される。
      await page.evaluate(() => {
        const b = document.createElement('button');
        b.setAttribute('data-action', 'fixture_phase');
        b.setAttribute('data-payload', 'before');
        document.querySelector('#app').appendChild(b);
      });
      assert.equal(await page.locator(PHASE_CONTROLS).count(), 1, '注入は検出される');
    });
  });

  test(`O4 左で見られる手がかりは行と同じ印だけで、変えた箇所を印で見分けられない（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page) => {
      const before = await snap(page);
      await wrong(page);
      const after = await snap(page);
      // 印は review（現在の状態と基準の差）から出る。Entered・Moved が消す pending は変更の併合用の保留で、印とは別物。
      // 実測（CI で CSP あり・なしとも合格）で誤答の前後とも 2 行が add のまま残る。
      assert.deepEqual(before.rows, ['f/alpha:add', 'f/beta:add']);
      assert.deepEqual(after.rows, before.rows, '誤答の前後で行と印が同じ');
      // 対照: 印の判定が空でないこと（行が 0 件なら上の一致は何も見ていない）。
      assert.equal(after.rows.length, 2);
    });
  });

  test(`O5 拒否の後も選び直せて、正答で達成になり次の Step へ進む（${mode}）`, { timeout: 120000 }, async () => {
    await withPage(csp, async (page) => {
      const pick = uidOf('pick');
      const achieved = async () => (await engine(page)).log.filter((r) => r.kind === 'achieved' && r.step === pick).length;
      assert.equal(await achieved(), 0, '対照: 正答の前は達成が 0 件');
      await wrong(page);
      assert.equal(await page.locator(CHOICE).count(), 2, '拒否の後も選択肢が 2 件残る');
      assert.equal(await achieved(), 0, '誤答では達成にならない');
      await choose(page, 'pick-beta');
      await page.waitForFunction((u) => JSON.parse(window.__fandheTest.stateJson()).engine.current !== u, pick, { timeout: 15000 });
      assert.equal(await achieved(), 1, '正答で達成が 1 件');
      assert.equal((await engine(page)).current, uidOf('end'));
      assert.equal(await page.locator(REJECTED).count(), 1, '拒否の行は正答で増えない');
    });
  });
}
