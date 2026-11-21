// 幅 360px で CH-1 の 6 Step を開始点 s-1-1 から模範解答で通し、操作の欠けを数える試験（SHELL-3。TASK-518 2/2。#1019）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。試験の設定（TEST_HOOKS=1）の `?fixture=ch0-3-chapters&start=s-1-1` を
// 幅 360px・高さ 900px で開き、ch0-3-journey.mjs の journey（from: 'main/st-1-1'）を実クリックで流し、CH-1 の最後（st-1-6）の達成の後に
// 「次の章へ進む」を押して main/st-2-1 に入るまで進める。タブは paneAware が要るときだけ選ぶ。
// 数え方は PoC-12 の手順 2（`docs/spec/03-poc/screen-reader-narrow/test-data/measurement-definitions.md`）に合わせる:
//   (1) 模範解答の操作の対象（選択肢・指す行・入力欄と送信ボタン・「次の章へ進む」）は、操作の直前に、見えない・押せない・はみ出すを数える
//       （見えない = 描画されていないか、自身か祖先が透明か、スクロールで入れた後も表示領域と重ならない。押せない = 中心の elementFromPoint が自身か子孫でない。
//       はみ出す = 左端が 0 より左か、右端が 360px を 0.5px 超える）
//   (2) 各 Step に入った時点と確認待ちになった時点で、操作・成果物の両方のタブを開き、描画されている操作可能な要素の全部について、
//       はみ出す・押せない（覆われている）を数える（(3)(4) も両方のタブで測る）
//   (3) 同じ時点で、ページの scrollWidth が 360 以下（横スクロール。1 つの時点につき 1 件）
//   (4) 同じ時点で、24×24 CSS ピクセル未満の操作対象（a11y::TARGET_SIZE_SELECTOR と同じ集合）が 0 件
//   欠けの 1 件は（Step, 要素, 種類）の組を重複なしで数える。流れの確認は journey の台帳（入力待ち・確認待ちの口、拒否・問い返しで
//   進まないこと、章の境目）と、開始点・終わりの確かめを合わせたもの。
// 送信は入力を済ませた後に送信ボタンを測ってから実クリックで押す（Enter は glue がボタンの click() を呼び、ポインターで押せなくても通るため）。
// 負の対照（CSSOM の el.style だけで変える）: 幅 500px の要素は (3) を、送信ボタンの覆いは (1)・(2) の押せないを、
// `pointer-events: none` のボタン（中心で祖先が返る）は (2) の押せないを、透明にしたボタンは (1) の見えないを、高さ 20px のボタンは (4) を出す。
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
  ledger, mouseDriver, journey, paneAware, engine, choiceSel, rowSel, decideSel, reviseSel, DRAFT, SUBMIT, NEXT, T,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-shell-narrow-ch1/';
const QUERY = '?fixture=ch0-3-chapters&start=s-1-1';
const WIDTH = 360;
const HEIGHT = 900;
// a11y::TARGET_SIZE_SELECTOR の写し（正は crates/web/src/a11y.rs）。
const TARGETS = 'button, select, input:not([type="hidden"]), [data-action]';
// 幅 959px 以下の tablist の 2 つのタブ（DOM の id）。
const PANES = ['pane-tab-ops', 'pane-tab-artifacts'];
// PoC-12 の (2) の「操作可能な要素」。
// 読み込みのファイル選択を開く label（`.sh-import-label`）も含める（shell-narrow と同じく、ファイル入力とは別に測る）。
const OPERABLE = 'button, textarea, select, input:not([type="hidden"]), a[href], [role="tab"], [tabindex]:not([tabindex^="-"]), .sh-import-label';
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

// (1) 操作の対象 1 つの観察。スクロールで表示領域に入れてから測る。識別子と欠けの種類の列を返す。
async function targetFaults(page, sel) {
  const loc = page.locator(sel).first();
  if ((await loc.count()) === 0 || !(await loc.isVisible())) return { id: sel, kinds: ['見えない'] };
  await loc.scrollIntoViewIfNeeded();
  return loc.evaluate((node, [w, h]) => {
    // 要素の識別子。testid に、選択肢・行・承認の決定・直す対象の値を足す（同じ testid の要素を区別する）。2 つの観察で同じ形にする。
    const id = (el) => {
      const extra = ['data-choice', 'data-row', 'data-decision', 'data-revise-target']
        .map((a) => el.getAttribute(a)).filter((v) => v !== null);
      return `${el.tagName.toLowerCase()}[${el.getAttribute('data-testid') ?? '-'}${el.id ? `#${el.id}` : ''}${extra.length ? `:${extra.join(':')}` : ''}]`;
    };
    const b = node.getBoundingClientRect();
    const kinds = [];
    // 自身か祖先が透明（opacity 0）なら、寸法があり押せても見えない。
    let transparent = false;
    for (let n = node; n instanceof Element; n = n.parentElement) {
      if (Number(getComputedStyle(n).opacity) === 0) transparent = true;
    }
    if (transparent || b.width === 0 || b.height === 0 || b.bottom <= 0 || b.top >= h || b.right <= 0 || b.left >= w) kinds.push('見えない');
    if (b.left < 0 || b.right > w + 0.5) kinds.push('はみ出す');
    const top = document.elementFromPoint(b.left + b.width / 2, b.top + b.height / 2);
    if (!(top !== null && (top === node || node.contains(top)))) kinds.push('押せない');
    return { id: id(node), kinds };
  }, [WIDTH, HEIGHT]);
}

// (2)〜(4) その時点の画面全体の観察。要素の識別子は (1) と同じ形。
const screenFaults = (page) => page.evaluate(([operable, targets, w]) => {
  // 要素の識別子。testid に、選択肢・行・承認の決定・直す対象の値を足す（同じ testid の要素を区別する）。2 つの観察で同じ形にする。
  const id = (el) => {
    const extra = ['data-choice', 'data-row', 'data-decision', 'data-revise-target']
      .map((a) => el.getAttribute(a)).filter((v) => v !== null);
    return `${el.tagName.toLowerCase()}[${el.getAttribute('data-testid') ?? '-'}${el.id ? `#${el.id}` : ''}${extra.length ? `:${extra.join(':')}` : ''}]`;
  };
  const shown = (el) => {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden';
  };
  const faults = [];
  // 縦に画面の外の要素は、スクロールで表示領域に入れてから測る（最後に元のスクロール位置へ戻す）。
  const sx = window.scrollX;
  const sy = window.scrollY;
  // 自身か祖先が透明（opacity 0）なら、寸法があっても見えない。
  const transparent = (el) => {
    for (let n = el; n instanceof Element; n = n.parentElement) {
      if (Number(getComputedStyle(n).opacity) === 0) return true;
    }
    return false;
  };
  for (const el of document.querySelectorAll(operable)) {
    if (!shown(el)) continue;
    if (transparent(el)) faults.push({ id: id(el), kind: '見えない' });
    // 無効の要素は押せない（CH-1 の通しの時点で無効にするボタンは無い。2026-11-22 の観察）。
    if (el.disabled === true || el.getAttribute('aria-disabled') === 'true') faults.push({ id: id(el), kind: '押せない' });
    let r = el.getBoundingClientRect();
    if (r.left < 0 || r.right > w + 0.5) faults.push({ id: id(el), kind: 'はみ出す' });
    if (r.bottom <= 0 || r.top >= window.innerHeight) {
      el.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'instant' });
      r = el.getBoundingClientRect();
    }
    // 覆われているかは、表示領域と重なる部分の中心で判定する（一部だけ見えている要素も測る。外の部分は elementFromPoint が引けない）。
    const vl = Math.max(r.left, 0);
    const vr = Math.min(r.right, window.innerWidth);
    const vt = Math.max(r.top, 0);
    const vb = Math.min(r.bottom, window.innerHeight);
    const cx = (vl + vr) / 2;
    const cy = (vt + vb) / 2;
    if (!(vr > vl && vb > vt)) {
      // スクロールで入れても表示領域と重ならない（届かない）要素は見えない。
      faults.push({ id: id(el), kind: '見えない' });
    } else {
      const top = document.elementFromPoint(cx, cy);
      // 祖先が返る（`pointer-events: none` など）のも押せないに数える。
      if (!(top !== null && (top === el || el.contains(top)))) faults.push({ id: id(el), kind: '押せない' });
    }
    // 次の要素の測定に前の要素のスクロール（縦・横）を持ち越さない。
    window.scrollTo(sx, sy);
  }
  window.scrollTo(sx, sy);
  const small = [...document.querySelectorAll(targets)]
    .filter((el) => shown(el) && (el.getBoundingClientRect().width < 24 || el.getBoundingClientRect().height < 24))
    .map(id);
  const de = document.documentElement;
  return { faults, small, scrollWidth: de.scrollWidth };
}, [OPERABLE, TARGETS, WIDTH]);

// 操作の直前に対象を測る包み。`point` は成果物のタブ、それ以外は操作のタブを paneAware が選んだ後に測る。
// 送信は、入力（改行・補完を含む）を済ませた後に送信ボタンを測ってから実クリックで押す（Enter は glue がボタンの click() を
// 呼ぶため、ポインターで押せないボタンでも通ってしまう）。
// `beforeNext` は「次の章へ進む」を押す直前に呼ぶ口（章の終わりの画面を走査する。ボタンの大きさは (4) で測る）。
function measuringDriver(page, inner, record, beforeNext) {
  const measure = async (sel) => {
    const { id, kinds } = await targetFaults(page, sel);
    for (const kind of kinds) record(id, kind);
  };
  const before = (sels, fn) => async (...args) => {
    for (const sel of sels(...args)) await measure(sel);
    return fn(...args);
  };
  const clickSubmit = async () => {
    await measure(SUBMIT);
    await page.click(SUBMIT, { timeout: T });
  };
  return paneAware(page, {
    ...inner,
    choose: before((id) => [choiceSel(id)], inner.choose),
    point: before((path) => [rowSel(path)], inner.point),
    decide: before((d) => [decideSel(d)], inner.decide),
    revise: before((t) => [reviseSel(t)], inner.revise),
    async send(text) {
      await measure(DRAFT);
      await page.fill(DRAFT, text);
      await clickSubmit();
    },
    async sendMultiline(first, second) {
      await measure(DRAFT);
      await page.fill(DRAFT, first);
      await page.press(DRAFT, 'Shift+Enter');
      await page.keyboard.type(second);
      await clickSubmit();
    },
    async completeAndSend(prefix, completed) {
      await measure(DRAFT);
      await page.fill(DRAFT, prefix);
      await page.press(DRAFT, 'Tab');
      await page.waitForFunction(([sel, v]) => document.querySelector(sel)?.value === v, [DRAFT, completed], { timeout: T });
      await clickSubmit();
    },
    esc: before(() => [DRAFT], inner.esc),
    // 章の終わりの画面を走査（タブの選び直しでスクロールが変わりうる）してから、押す直前のボタンを測る。
    nextChapter: async () => {
      await beforeNext();
      await measure(NEXT);
      return inner.nextChapter();
    },
  }, { clicks: 0 });
}

// 1 回の通し。欠けの組と台帳を返す。
async function run(srv) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: WIDTH, height: HEIGHT } });
  const L = ledger();
  // 欠けの組（Step, 要素, 種類）。
  const faults = new Set();
  const add = (key) => faults.add(key);
  const overflow = new Set();
  const small = new Set();
  let current = 'main/st-1-1';
  let points = 0;
  await page.goto(`${srv.url}${QUERY}`);
  await waitMounted(page);
  L.check('open', '開始点 s-1-1 の CH-1 の先頭で開く', (await engine(page)).current === 'main/st-1-1');
  // タブを押し、そのタブが選ばれて対応するパネル（aria-controls）が表示されるのを待つ（押しただけでは切り替わった保証がない）。
  const selectPane = async (pane) => {
    await page.click(`#${pane}`, { timeout: T });
    await page.waitForFunction((id) => {
      const tab = document.getElementById(id);
      const panel = document.getElementById(tab?.getAttribute('aria-controls') ?? '');
      if (!(tab?.getAttribute('aria-selected') === 'true' && panel && !panel.hidden)) return false;
      const cs = getComputedStyle(panel);
      return cs.display !== 'none' && cs.visibility === 'visible' && Number(cs.opacity) > 0 && panel.getBoundingClientRect().height > 0;
    }, pane, { timeout: T });
  };
  const scan = async (uid, status, pane) => {
    const o = await screenFaults(page);
    for (const f of o.faults) add(`${uid} ${f.id} ${f.kind}`);
    if (o.scrollWidth > WIDTH) overflow.add(`${uid}/${status}/${pane}`);
    for (const s of o.small) small.add(`${uid} ${s}`);
  };
  // 各時点で、操作・成果物の両方のタブを開いて走査し、元のタブに戻す（見えている側だけでは片方の欠けを見逃す）。
  const onStep = async (uid, status) => {
    current = uid;
    points += 1;
    const selected = await page.getAttribute('[data-testid="pane-tab"][aria-selected="true"]', 'id');
    for (const pane of PANES) {
      await selectPane(pane);
      await scan(uid, status, pane);
    }
    if (selected) await selectPane(selected);
  };
  const record = (id, kind) => add(`${current} ${id} ${kind}`);
  // 章の終わり（st-1-6 の達成の後。「次の章へ進む」が出ている）の画面も、操作のタブで (2)〜(4) を走査する。
  const beforeNext = async () => {
    await selectPane('pane-tab-ops');
    await scan(current, 'chapter-end', 'pane-tab-ops');
    points += 1;
  };
  const d = measuringDriver(page, mouseDriver(page), record, beforeNext);
  // 操作が失敗して通しが止まっても、それまでに数えた欠けと止まった Step を出してから失敗させる。
  try {
    await journey(page, d, '360px', L, 'main/st-2-1', { chaptered: true, onStep, from: 'main/st-1-1' });
  } catch (err) {
    await context.close();
    assert.fail(`journey stopped at ${current}: ${err.message.split('\n')[0]}; faults=${JSON.stringify([...faults])}`
      + ` overflow=${JSON.stringify([...overflow])} small=${JSON.stringify([...small])}`);
  }
  const e = await engine(page);
  L.check('end', 'main/st-2-1 に入った', e.current === 'main/st-2-1');
  L.check('end', 'CH-1 の 6 Step がすべて達成', ['1', '2', '3', '4', '5', '6']
    .every((n) => e.stateWithoutLog.run.slices.main?.status?.[`main/st-1-${n}`] === 'achieved'));
  L.check('end', 'エンジンの警告・無視が 0 件', e.log.filter((r) => r.kind === 'warning' || r.kind === 'ignored').length === 0);
  const s = summarize(rec);
  L.check('end', 'ページ内のエラー・CSP 違反が 0 件', s.pageErrors === 0 && s.csp === 0);
  await context.close();
  return {
    L, faults: [...faults], overflow: [...overflow], small: [...small], points,
  };
}

test('幅 360px で CH-1 を s-1-1 から模範解答で通すと、操作の欠けが 0 件で流れの確認がすべて通る', { timeout: 300000 }, async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const r = await run(srv);
    console.log(`diagnostic 360px CH-1: flow=${r.L.summary()} points=${r.points} faults=${r.faults.length} overflow=${r.overflow.length} small=${r.small.length}`);
    assert.deepEqual(r.L.failed(), [], 'flow checks');
    // journey の CH-1 の 18 項目と、開始点・終わりの 5 項目（docs/testing.md 5.532）。
    assert.equal(r.L.items.length, 23, 'flow checks count');
    // 入力待ち 6 時点（st-1-1〜st-1-6）と確認待ち 4 時点（st-1-1・st-1-4・st-1-5・st-1-6）と章の終わり 1 時点。
    assert.equal(r.points, 11, 'measured 11 screen states');
    assert.deepEqual(r.faults, [], 'operation faults (step, element, kind)');
    assert.deepEqual(r.overflow, [], 'horizontal scroll (scrollWidth > 360)');
    assert.deepEqual(r.small, [], 'targets smaller than 24x24');
  });
});

// 負の対照: 数え方が素通りでないこと。開いた直後の画面に CSSOM（el.style）だけで手を入れ、(1)・(2)・(3) を別々に確かめる。
// 送信ボタンを覆うと実クリックで通しが進まないため、通しは流さず、測る関数を直接呼ぶ。
test('対照: 幅 500px の要素・送信ボタンの覆い・pointer-events の無いボタンを検出する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: WIDTH, height: HEIGHT } });
    await page.goto(`${srv.url}${QUERY}`);
    await waitMounted(page);
    const clean = await screenFaults(page);
    assert.deepEqual(clean.faults, [], 'baseline screen has no fault');
    assert.ok(clean.scrollWidth <= WIDTH, 'baseline has no horizontal scroll');
    assert.deepEqual((await targetFaults(page, SUBMIT)).kinds, [], 'baseline submit button is fine');

    await page.evaluate((sel) => {
      const wide = document.createElement('div');
      wide.style.width = '500px';
      wide.style.height = '1px';
      document.body.append(wide);
      const btn = document.querySelector(sel);
      btn.parentElement.style.position = 'relative';
      const cover = document.createElement('div');
      cover.style.position = 'absolute';
      cover.style.zIndex = '10';
      cover.style.inset = '0';
      btn.parentElement.append(cover);
    }, SUBMIT);
    const covered = (faults) => faults.some((f) => f.id.includes('[submit-input]') && f.kind === '押せない');
    const t = await targetFaults(page, SUBMIT);
    assert.ok(t.id.includes('[submit-input]') && t.kinds.includes('押せない'), '(1) covered submit button');
    const o = await screenFaults(page);
    assert.ok(covered(o.faults), '(2) covered submit button on the screen scan');
    assert.ok(o.scrollWidth > WIDTH, '(3) wide element makes horizontal scroll');

    // 高さ 20px のボタンは (4) の 24px 未満に数える。
    await page.evaluate(() => {
      const small = document.createElement('button');
      small.type = 'button';
      small.setAttribute('data-testid', 'small-probe');
      small.textContent = 'x';
      small.style.cssText = 'min-height: 0; height: 20px; width: 40px;';
      document.body.prepend(small);
    });
    assert.ok((await screenFaults(page)).small.some((id) => id.includes('[small-probe]')), '(4) a 20px-high button is a small target');

    // 無効にした書き出しのボタンは、画面全体の走査 (2) で押せないに数える。
    const disabled = await openPage(browser, { origin: srv.origin, viewport: { width: WIDTH, height: HEIGHT } });
    await disabled.page.goto(`${srv.url}${QUERY}`);
    await waitMounted(disabled.page);
    await disabled.page.evaluate(() => { document.querySelector('[data-testid="shell-export"]').disabled = true; });
    assert.ok((await screenFaults(disabled.page)).faults.some((f) => f.id.includes('[shell-export]') && f.kind === '押せない'), '(2) disabled export button');
    await disabled.context.close();

    // 透明な送信ボタンは (1) で見えないに数え、画面全体の走査 (2) でも見えないに数える。
    const faded = await openPage(browser, { origin: srv.origin, viewport: { width: WIDTH, height: HEIGHT } });
    await faded.page.goto(`${srv.url}${QUERY}`);
    await waitMounted(faded.page);
    await faded.page.evaluate((sel) => { document.querySelector(sel).parentElement.style.opacity = '0'; }, SUBMIT);
    assert.ok((await targetFaults(faded.page, SUBMIT)).kinds.includes('見えない'), '(1) transparent submit button');
    assert.ok((await screenFaults(faded.page)).faults.some((f) => f.id.includes('[submit-input]') && f.kind === '見えない'), '(2) transparent submit button');
    await faded.context.close();

    // 祖先が返る形（pointer-events: none）も (2) で押せないに数える。
    const reload = await openPage(browser, { origin: srv.origin, viewport: { width: WIDTH, height: HEIGHT } });
    await reload.page.goto(`${srv.url}${QUERY}`);
    await waitMounted(reload.page);
    await reload.page.evaluate((sel) => { document.querySelector(sel).style.pointerEvents = 'none'; }, SUBMIT);
    assert.ok(covered((await screenFaults(reload.page)).faults), '(2) submit button without pointer events');
    await reload.context.close();
    await context.close();
  });
});
