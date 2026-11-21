// 幅 360px で CH-1 の 6 Step を開始点 s-1-1 から模範解答で通し、操作の欠けを数える試験（SHELL-3。TASK-518 2/2。#1019）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。試験の設定（TEST_HOOKS=1）の `?fixture=ch0-3-chapters&start=s-1-1` を
// 幅 360px・高さ 900px で開き、ch0-3-journey.mjs の journey（from: 'main/st-1-1'）を実クリックで流し、CH-1 の最後（st-1-6）の達成の後に
// 「次の章へ進む」を押して main/st-2-1 に入るまで進める。タブは paneAware が要るときだけ選ぶ。
// 数え方は PoC-12 の手順 2（`docs/spec/03-poc/screen-reader-narrow/test-data/measurement-definitions.md`）に合わせる:
//   (1) 模範解答の操作の対象（選択肢・指す行・入力欄と送信ボタン・「次の章へ進む」）は、操作の直前に、見えない・押せない・はみ出すを数える
//       （見えない = 描画されていないか、スクロールで入れた後も表示領域と重ならない。押せない = 中心の elementFromPoint が自身か子孫でない。
//       はみ出す = 左端が 0 より左か、右端が 360px を 0.5px 超える）
//   (2) 各 Step に入った時点と確認待ちになった時点で、描画されている操作可能な要素の全部について、はみ出す・押せない（覆われている）を数える
//   (3) 同じ時点で、ページの scrollWidth が 360 以下（横スクロール。1 つの時点につき 1 件）
//   (4) 同じ時点で、24×24 CSS ピクセル未満の操作対象（a11y::TARGET_SIZE_SELECTOR と同じ集合）が 0 件
//   欠けの 1 件は（Step, 要素, 種類）の組を重複なしで数える。流れの確認は journey の台帳（入力待ち・確認待ちの口、拒否・問い返しで
//   進まないこと、章の境目）と、開始点・終わりの確かめを合わせたもの。
// 負の対照（CSSOM の el.style だけで変える）: 幅 500px の要素と、送信ボタンを覆う要素を入れると、(3) と (1) の押せないが検出する。
// 範囲外: 4 幅の観察（TASK-368）、縦の長さ・読みやすさ（人の評価。TASK-387）、Tab の出入り（tab-exit・shell-boundary）。
// 出力には testid・件数・合否だけを出す（入力した値は出さない）。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, summarize, waitMounted,
} from './harness.mjs';
import {
  ledger, mouseDriver, journey, paneAware, engine, choiceSel, rowSel, decideSel, reviseSel, DRAFT, SUBMIT, NEXT,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-shell-narrow-ch1/';
const QUERY = '?fixture=ch0-3-chapters&start=s-1-1';
const WIDTH = 360;
const HEIGHT = 900;
// a11y::TARGET_SIZE_SELECTOR の写し（正は crates/web/src/a11y.rs）。
const TARGETS = 'button, select, input:not([type="hidden"]), [data-action]';
// PoC-12 の (2) の「操作可能な要素」。
const OPERABLE = 'button, textarea, select, input:not([type="hidden"]), a[href], [role="tab"], [tabindex]:not([tabindex^="-"])';
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

// (1) 操作の対象 1 つの観察。スクロールで表示領域に入れてから測る。
async function targetFaults(page, sel) {
  const loc = page.locator(sel).first();
  if ((await loc.count()) === 0 || !(await loc.isVisible())) return ['見えない'];
  await loc.scrollIntoViewIfNeeded();
  return loc.evaluate((node, [w, h]) => {
    const b = node.getBoundingClientRect();
    const out = [];
    if (b.width === 0 || b.height === 0 || b.bottom <= 0 || b.top >= h || b.right <= 0 || b.left >= w) out.push('見えない');
    if (b.left < -0.5 || b.right > w + 0.5) out.push('はみ出す');
    const top = document.elementFromPoint(b.left + b.width / 2, b.top + b.height / 2);
    if (!(top !== null && (top === node || node.contains(top)))) out.push('押せない');
    return out;
  }, [WIDTH, HEIGHT]);
}

// (2)〜(4) その時点の画面全体の観察。要素は testid（無ければタグ名）で表す。
const screenFaults = (page) => page.evaluate(([operable, targets, w]) => {
  const id = (el) => `${el.tagName.toLowerCase()}[${el.getAttribute('data-testid') ?? (el.id || '-')}]`;
  const shown = (el) => {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden';
  };
  const faults = [];
  for (const el of document.querySelectorAll(operable)) {
    if (!shown(el)) continue;
    const r = el.getBoundingClientRect();
    if (r.left < -0.5 || r.right > w + 0.5) faults.push(`${id(el)} はみ出す`);
    // 覆われているかは、表示領域の中にある要素だけで判定する（外にある要素は elementFromPoint が引けない）。
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    if (cx >= 0 && cx < window.innerWidth && cy >= 0 && cy < window.innerHeight) {
      const top = document.elementFromPoint(cx, cy);
      if (!(top !== null && (top === el || el.contains(top) || top.contains(el)))) faults.push(`${id(el)} 押せない`);
    }
  }
  const small = [...document.querySelectorAll(targets)]
    .filter((el) => shown(el) && (el.getBoundingClientRect().width < 24 || el.getBoundingClientRect().height < 24))
    .map(id);
  const de = document.documentElement;
  return { faults, small, scrollWidth: de.scrollWidth };
}, [OPERABLE, TARGETS, WIDTH]);

// 操作の直前に対象を測る包み。`point` は成果物のタブ、それ以外は操作のタブを paneAware が選んだ後に測る。
function measuringDriver(page, inner, record) {
  const before = (sels, fn) => async (...args) => {
    for (const sel of sels(...args)) {
      for (const kind of await targetFaults(page, sel)) record(sel, kind);
    }
    return fn(...args);
  };
  const ensured = paneAware(page, {
    ...inner,
    choose: before((id) => [choiceSel(id)], inner.choose),
    point: before((path) => [rowSel(path)], inner.point),
    decide: before((d) => [decideSel(d)], inner.decide),
    revise: before((t) => [reviseSel(t)], inner.revise),
    send: before(() => [DRAFT, SUBMIT], inner.send),
    sendMultiline: before(() => [DRAFT, SUBMIT], inner.sendMultiline),
    completeAndSend: before(() => [DRAFT, SUBMIT], inner.completeAndSend),
    esc: before(() => [DRAFT], inner.esc),
    nextChapter: before(() => [NEXT], inner.nextChapter),
  }, { clicks: 0 });
  return ensured;
}

// 1 回の通し。`tamper` はページを開いた直後に呼ぶ（負の対照の口）。欠けの組と台帳を返す。
async function run(srv, tamper = null) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: WIDTH, height: HEIGHT } });
  const L = ledger();
  const faults = new Set();
  const overflow = new Set();
  const small = new Set();
  let current = 'main/st-1-1';
  let points = 0;
  await page.goto(`${srv.url}${QUERY}`);
  await waitMounted(page);
  if (tamper) await tamper(page);
  L.check('open', '開始点 s-1-1 の CH-1 の先頭で開く', (await engine(page)).current === 'main/st-1-1');
  const onStep = async (uid, status) => {
    current = uid;
    points += 1;
    const o = await screenFaults(page);
    for (const f of o.faults) faults.add(`${uid} ${f}`);
    if (o.scrollWidth > WIDTH) overflow.add(`${uid}/${status}`);
    for (const s of o.small) small.add(`${uid} ${s}`);
  };
  const record = (sel, kind) => faults.add(`${current} ${sel} ${kind}`);
  const d = measuringDriver(page, mouseDriver(page), record);
  await journey(page, d, '360px', L, 'main/st-2-1', { chaptered: true, onStep, from: 'main/st-1-1' });
  const e = await engine(page);
  L.check('end', 'main/st-2-1 に入った', e.current === 'main/st-2-1');
  L.check('end', 'CH-1 の 6 Step がすべて達成', ['1', '2', '3', '4', '5', '6']
    .every((n) => e.stateWithoutLog.run.slices.main?.status?.[`main/st-1-${n}`] === 'achieved'));
  L.check('end', 'エンジンの警告・無視が 0 件', e.log.filter((r) => r.kind === 'warning' || r.kind === 'ignored').length === 0);
  const s = summarize(rec);
  L.check('end', 'ページ内のエラー・CSP 違反が 0 件', s.pageErrors === 0 && s.csp === 0);
  await context.close();
  return { L, faults: [...faults], overflow: [...overflow], small: [...small], points };
}

test('幅 360px で CH-1 を s-1-1 から模範解答で通すと、操作の欠けが 0 件で流れの確認がすべて通る', { timeout: 300000 }, async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const r = await run(srv);
    console.log(`diagnostic 360px CH-1: flow=${r.L.summary()} points=${r.points} faults=${r.faults.length} overflow=${r.overflow.length} small=${r.small.length}`);
    assert.deepEqual(r.L.failed(), [], 'flow checks');
    // 入力待ち 6 時点（st-1-1〜st-1-6）と確認待ち 4 時点（st-1-1・st-1-4・st-1-5・st-1-6）。
    assert.equal(r.points, 10, 'measured 10 screen states');
    assert.deepEqual(r.faults, [], 'operation faults (step, element, kind)');
    assert.deepEqual(r.overflow, [], 'horizontal scroll (scrollWidth > 360)');
    assert.deepEqual(r.small, [], 'targets smaller than 24x24');
  });
});

// 負の対照: 数え方が素通りでないこと。幅 500px の要素は横スクロールを、送信ボタンを覆う要素は押せないを出す。
test('対照: 幅 500px の要素と、送信ボタンを覆う要素を検出する', { timeout: 300000 }, async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const r = await run(srv, async (page) => {
      await page.evaluate((sel) => {
        const wide = document.createElement('div');
        wide.style.width = '500px';
        wide.style.height = '1px';
        document.body.append(wide);
        // 覆いは送信ボタンの位置に追従させず、ボタンの上に固定で重ねる（通しの間、ボタンの近くに居続ける）。
        const btn = document.querySelector(sel);
        const cover = document.createElement('div');
        cover.style.position = 'absolute';
        cover.style.zIndex = '10';
        btn.parentElement.style.position = 'relative';
        cover.style.inset = '0';
        btn.parentElement.append(cover);
      }, SUBMIT);
    });
    assert.ok(r.overflow.length >= 1, 'wide element is detected as horizontal scroll');
    assert.ok(r.faults.some((f) => f.includes('submit-input') && f.endsWith('押せない')), 'covered submit button is detected');
  });
});
