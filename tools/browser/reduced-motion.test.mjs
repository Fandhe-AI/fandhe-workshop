// prefers-reduced-motion: reduce で段階の表示を省く試験（TASK-372 1/2。A11Y-11・EVT-19）。
//
// reduce の文脈で処理中の Step（5 段）を流し、右のログの段階の行（li[data-kind=progress]）と progress 要素が最初から最後まで
// 0 件であること、進行そのものは起きている（engine.ticks が 5 件）こと、結果の行と状態の語は通常どおり出ることを確かめる。
// 設定は読み込み時に 1 回読むため、emulateMedia は goto より前に呼ぶ。対照として no-preference では段階の行が 5 件出る。
// TASK-372 2/2: no-preference で処理中に中間の段階が見えること、両方の文脈でアニメーション・transition・@keyframes・
// 文字の逐次表示が 0 件であること（測り方は PoC-12 の測定の定義の手順 3）を確かめ、注入した動きを検出する負の対照を持つ。
// 文字の逐次表示は、ログの行・本文・カードの文字列が同じ要素の中で変わった回数で数える。状態の語（run-status・step-status。TERM-33）は
// 段階でも逐次表示でもないため数えない。エンジンに逐次表示の演出は無いので、0 件は演出が無いことの記録であり、基準を達成した証拠ではない。
// 待ちは固定の sleep ではなく、上限つきのポーリングで書く。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-reduced-motion/';
const RAW = '[data-testid="raw-trigger"]';
const STATUS = '[data-testid="run-status"]';
const PROGRESS_LI = '[data-testid="part-log-list"] li[data-kind="progress"]';
const INPUT = { input: { command: '/make-note first-note' } };
const TOTAL_TICKS = 5;

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

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;

const sample = (page) => page.evaluate(({ status, li }) => ({
  state: document.querySelector(status)?.getAttribute('data-state') ?? null,
  progressRows: document.querySelectorAll(li).length,
  progressEls: document.querySelectorAll('progress').length,
  received: document.querySelectorAll('[data-testid="part-log-list"] li[data-kind="received"]').length,
}), { status: STATUS, li: PROGRESS_LI });

async function waitUntil(what, read, pred, timeout = 10000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await read();
    if (await pred(last)) return last;
    await new Promise((r) => setTimeout(r, 20));
  }
  throw new Error(`timeout waiting for ${what}: ${JSON.stringify(last)}`);
}

const detail = (page) => page.evaluate(({ status, li }) => ({
  state: document.querySelector(status)?.getAttribute('data-state') ?? null,
  progressRows: document.querySelectorAll(li).length,
  indices: [...document.querySelectorAll(li)].map((e) => e.getAttribute('data-index')),
  totals: [...document.querySelectorAll(li)].map((e) => e.getAttribute('data-total')),
}), { status: STATUS, li: PROGRESS_LI });

// ページ内の計測。イベント（animation*・transition*）、段階の行の追加・削除、文字の逐次表示を数える。
// 逐次表示は、対象（ログの行・Step の本文・カード・負の対照の probe）の文字が書き換わった回数（characterData の更新と、textContent 等による文字ノードの置換・追加）。
// 行の追加（新しいノードの挿入）は数えない。最後の状態への更新も 1 回と数え、PoC-12 の定義より検出側に倒す。
const MOTION_WATCH = () => {
  const m = { events: 0, progressAdded: 0, progressRemoved: 0, seqText: 0 };
  window.__motion = m;
  for (const t of ['animationstart', 'animationiteration', 'transitionrun', 'transitionstart']) {
    window.addEventListener(t, () => { m.events += 1; }, true);
  }
  const TARGET = '[data-testid="part-log-list"] li, [data-testid="step-body"], [data-testid^="card"], [data-motion-probe]';
  const isProg = (n) => n.nodeType === 1 && n.matches('li[data-kind="progress"]');
  new MutationObserver((ms) => {
    for (const r of ms) {
      for (const n of r.addedNodes) if (isProg(n)) m.progressAdded += 1;
      for (const n of r.removedNodes) if (isProg(n)) m.progressRemoved += 1;
      if (r.type === 'childList' && r.target.closest?.(TARGET)) {
        // textContent の再代入など、文字ノードの置換・既存の文字ノードへの追加も逐次表示として数える。
        const txt = (list) => Array.from(list).filter((n) => n.nodeType === 3);
        const added = txt(r.addedNodes);
        const removed = txt(r.removedNodes);
        const joined = (l) => l.map((n) => n.data).join('');
        if (added.length > 0 && removed.length > 0 && joined(added) !== joined(removed)) m.seqText += 1;
        else if (added.length > 0 && removed.length === 0 && added.some((n) => n.previousSibling?.nodeType === 3 || n.nextSibling?.nodeType === 3)) m.seqText += 1;
      }
      if (r.type === 'characterData' && r.oldValue !== r.target.data && r.target.parentElement?.closest(TARGET)) m.seqText += 1;
    }
  }).observe(document.documentElement, { childList: true, subtree: true, characterData: true, characterDataOldValue: true });
};

// スタイルシートの静的な件数。入れ子の規則もたどる。読めないシートは例外になり失敗する（fail-closed）。
const staticMotion = (page) => page.evaluate(() => {
  let keyframes = 0;
  let decls = 0;
  const walk = (rules) => {
    for (const r of rules) {
      if (r.constructor.name.includes('Keyframes')) keyframes += 1;
      if (r.style) {
        for (let i = 0; i < r.style.length; i += 1) {
          const n = r.style[i].replace(/^-(webkit|moz)-/, '');
          if (n.startsWith('animation') || n.startsWith('transition')) decls += 1;
        }
      }
      if (r.cssRules) walk(r.cssRules);
    }
  };
  for (const sh of document.styleSheets) walk(sh.cssRules);
  return { keyframes, decls };
});

const dynamicMotion = (page) => page.evaluate(() => {
  let animated = 0;
  let transitioned = 0;
  for (const el of document.querySelectorAll('*')) {
    const cs = getComputedStyle(el);
    if (cs.animationName !== 'none') animated += 1;
    if (cs.transitionDuration.split(',').some((d) => d.trim() !== '0s')) transitioned += 1;
  }
  return { animations: document.getAnimations().length, animated, transitioned };
});

// ページ内で progress の行・要素が足されたことを数える。ポーリングの間に一瞬だけ現れて消える分も拾う。
const WATCH = () => {
  window.__rmAdded = 0;
  const isProgress = (n) => n.nodeType === 1
    && (n.matches('li[data-kind="progress"], progress') || n.querySelector('li[data-kind="progress"], progress'));
  new MutationObserver((ms) => {
    for (const m of ms) for (const n of m.addedNodes) if (isProgress(n)) window.__rmAdded += 1;
  }).observe(document.documentElement, { childList: true, subtree: true });
};

for (const csp of [true, false]) {
  const mode = csp ? 'CSP あり' : 'CSP なし';

  test(`reduce では処理中でも段階の行が 0 件で、結果と状態は通常どおり出る（${mode}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.emulateMedia({ reducedMotion: 'reduce' });
      await page.goto(srv.url);
      await waitMounted(page);
      await page.evaluate(WATCH);
      assert.equal((await engine(page)).reducedMotion, true);

      await sendRaw(page, INPUT);
      const samples = [];
      let sawBusy = false;
      await waitUntil('busy then idle', async () => {
        const s = await sample(page);
        samples.push(s);
        if (s.state === 'busy') sawBusy = true;
        return s;
      }, (s) => sawBusy && s.state === 'idle');
      assert.ok(sawBusy, '処理中を観察できなかった');
      assert.ok(samples.every((s) => s.progressRows === 0 && s.progressEls === 0), JSON.stringify(samples));
      assert.equal(await page.evaluate(() => window.__rmAdded), 0, '段階の行が一瞬でも足された');

      const eng = await engine(page);
      assert.equal(eng.ticks.filter(([g]) => g === 1).length, TOTAL_TICKS, '進行は起き、表示だけを省く');
      assert.equal(eng.runStatus, 'idle');
      assert.equal(eng.log.filter((r) => r.line === 'progress').length, 0);
      assert.ok(eng.log.some((r) => r.line === 'received'), '受付の行が出ている');
      assert.ok((await sample(page)).received >= 1, '結果の行が DOM に出ている');

      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    }));
  });

  test(`no-preference では処理中に中間の段階が見え、段階の行は 1 回ずつ足される（${mode}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await page.emulateMedia({ reducedMotion: 'no-preference' });
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal(await page.evaluate(() => matchMedia('(prefers-reduced-motion: reduce)').matches), false);
      assert.equal((await engine(page)).reducedMotion, false);
      await page.evaluate(MOTION_WATCH);
      await sendRaw(page, INPUT);
      const samples = [];
      let sawBusy = false;
      await waitUntil('busy then idle', async () => {
        const s = await detail(page);
        samples.push(s);
        if (s.state === 'busy') sawBusy = true;
        return s;
      }, (s) => sawBusy && s.state === 'idle');
      assert.ok(samples.some((s) => s.state === 'busy' && s.progressRows > 0 && s.progressRows < TOTAL_TICKS),
        `処理中の中間の段階を観察できなかった ${JSON.stringify(samples)}`);
      for (let i = 1; i < samples.length; i += 1) {
        assert.ok(samples[i].progressRows >= samples[i - 1].progressRows, '段階の行が減った');
      }
      const last = samples[samples.length - 1];
      assert.equal(last.progressRows, TOTAL_TICKS);
      assert.deepEqual(last.indices, ['0', '1', '2', '3', '4']);
      assert.ok(last.totals.every((t) => t === String(TOTAL_TICKS)), JSON.stringify(last.totals));
      const m = await page.evaluate(() => ({ added: window.__motion.progressAdded, removed: window.__motion.progressRemoved,
        seq: window.__motion.seqText, events: window.__motion.events }));
      assert.equal(m.added, TOTAL_TICKS, '段階の行は 1 回ずつ足される');
      assert.equal(m.removed, 0, '段階の行は置き換えられない');
      assert.equal(m.seq, 0, '文字の逐次表示');
      assert.equal(m.events, 0, 'アニメーション・transition のイベント');
      assert.equal((await engine(page)).ticks.filter(([g]) => g === 1).length, TOTAL_TICKS);
      const safety = await collectSafety(page, rec);
      assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
      await context.close();
    }));
  });

  for (const motion of ['reduce', 'no-preference']) {
    test(`${motion} でアニメーション・transition・@keyframes・文字の逐次表示が 0 件である（${mode}）`, { timeout: 120000 }, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        await page.emulateMedia({ reducedMotion: motion });
        await page.goto(srv.url);
        await waitMounted(page);
        assert.equal(await page.evaluate(() => matchMedia('(prefers-reduced-motion: reduce)').matches), motion === 'reduce');
        assert.equal((await engine(page)).reducedMotion, motion === 'reduce');
        await page.evaluate(MOTION_WATCH);
        assert.deepEqual(await staticMotion(page), { keyframes: 0, decls: 0 });
        await sendRaw(page, INPUT);
        let sawBusy = false;
        let maxAnims = 0;
        let styled = { animated: 0, transitioned: 0 };
        await waitUntil('busy then idle', async () => {
          const d = await dynamicMotion(page);
          maxAnims = Math.max(maxAnims, d.animations);
          styled = { animated: Math.max(styled.animated, d.animated), transitioned: Math.max(styled.transitioned, d.transitioned) };
          const s = await sample(page);
          if (s.state === 'busy') sawBusy = true;
          return s;
        }, (s) => sawBusy && s.state === 'idle');
        assert.ok(sawBusy, '処理中を観察できなかった');
        assert.equal(maxAnims, 0, 'getAnimations の最大');
        assert.deepEqual(styled, { animated: 0, transitioned: 0 });
        assert.deepEqual(await staticMotion(page), { keyframes: 0, decls: 0 });
        const m = await page.evaluate(() => ({ seq: window.__motion.seqText, events: window.__motion.events }));
        assert.deepEqual(m, { seq: 0, events: 0 });
        const safety = await collectSafety(page, rec);
        assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
        await context.close();
      }));
    });
  }
}

test('負の対照: 注入した @keyframes・transition・WAAPI・文字の書き換えを検出する（CSP なし）', { timeout: 120000 }, async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: false }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await page.goto(srv.url);
    await waitMounted(page);
    await page.evaluate(MOTION_WATCH);
    await page.addStyleTag({ content: '@keyframes fw-probe { from { opacity: 0 } to { opacity: 1 } }'
      + ' .fw-anim { animation: fw-probe 5s infinite } .fw-tr { transition: opacity 0.2s; opacity: 1 }' });
    await page.evaluate(() => {
      for (const c of ['fw-anim', 'fw-tr']) {
        const e = document.createElement('div');
        e.className = c;
        e.id = c;
        document.body.appendChild(e);
      }
      const t = document.createElement('div');
      t.id = 'fw-typed';
      t.setAttribute('data-motion-probe', '');
      t.appendChild(document.createTextNode('a'));
      document.body.appendChild(t);
    });
    const st = await staticMotion(page);
    assert.ok(st.keyframes >= 1 && st.decls >= 2, JSON.stringify(st));
    const dyn = await dynamicMotion(page);
    assert.ok(dyn.animations >= 1 && dyn.animated >= 1 && dyn.transitioned >= 1, JSON.stringify(dyn));
    await page.evaluate(() => { document.getElementById('fw-tr').style.opacity = '0.5'; });
    await waitUntil('transitionrun', () => page.evaluate(() => window.__motion.events), (n) => n >= 1);
    const before = (await dynamicMotion(page)).animations;
    await page.evaluate(() => document.getElementById('fw-tr').animate([{ opacity: 0 }, { opacity: 1 }], { duration: 5000 }));
    assert.ok((await dynamicMotion(page)).animations > before, 'WAAPI を検出できない');
    await page.evaluate(() => {
      const node = document.getElementById('fw-typed').firstChild;
      let n = 0;
      const id = setInterval(() => { node.data += 'b'; n += 1; if (n >= 3) clearInterval(id); }, 10);
    });
    await waitUntil('typed', () => page.evaluate(() => window.__motion.seqText), (n) => n >= 1);
    // textContent の再代入（文字ノードの置換。childList 経由）も検出できることの負の対照
    const seqBefore = await page.evaluate(() => window.__motion.seqText);
    await page.evaluate(() => {
      const e = document.createElement('div');
      e.id = 'fw-typed2';
      e.setAttribute('data-motion-probe', '');
      e.textContent = 'a';
      document.body.appendChild(e);
      let n = 0;
      const id = setInterval(() => { e.textContent += 'b'; n += 1; if (n >= 3) clearInterval(id); }, 10);
    });
    await waitUntil('typed-textContent', () => page.evaluate(() => window.__motion.seqText), (n) => n > seqBefore);
    await context.close();
  }));
});

test('配信設定でも reduce で開いてマウントでき、例外・CSP 違反が 0 件である', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    const safety = await collectSafety(page, rec);
    assert.deepEqual(nonZero(safety), [], JSON.stringify({ safety, rec }));
    await context.close();
  });
});
