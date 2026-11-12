// 右のエリアの部品がそろった本体の画面での Tab・Shift+Tab の結果の試験（TASK-451。TERM-10・JUDGE-19・A11Y-16。PoC-2 の K-01〜K-08・K-15）。
//
// 役割: tab-exit.test.mjs・tab-trap.test.mjs（TASK-373）は開始点 `s-1-1`（送信・停止・補完の候補だけが在る画面）で固定した。
// ここでは同じ判定を、選択肢・承認のボタンなど右のエリアの部品が増えた画面で流し直す。あわせて、先送りされていた 3 件を回収する。
//   (1) 一覧が開いた欄からの順方向の Tab の一巡（補完が 1 回挟まる）  (2) K-07（変換中の Tab）を試験用の口の無い画面で  (3) 部品がそろった画面での再確認。
// 判定の実体は tab-focus.mjs の共有（exitProblems・reverseProblems・completionProblems・walkCycle）。観察は DOM と defaultPrevented だけで、
// __fandheTest は読まない（毎回 undefined であることを確かめる）。
//
// 画面: 配信する設定の写しに、開始点 `s-1-1`・`s-1-5`・`s-1-6`・`s-2-6` を足した IR を `ir/main.json` として置き、`?start=` で開く。CSP あり・なしの両方。幅 1024px。
//   W1 s-1-1（command の入力待ち） W2 s-1-6（choice の入力待ち。選択肢のボタン）
//   W3 s-1-6 で選択肢を押した確認待ち（answer の確認の選択肢のボタン） W4 s-2-6（approval の入力待ち。はい・いいえ・直す・対象つきの直す）
//   W5 s-2-6 で対象つきの「直す」を押した確認待ち（approve の確認の承認のボタン）
// 選択肢と承認のボタンは製品の表示条件（offers.rs）で同時には出ない。そのため全部品が 1 画面にそろう場面は作れず、場面を分けて確かめる。
//   S1 空欄  S2 一覧が閉じて文字あり（/zzz・/help・Esc で閉じた /he）  S3 一覧が開いて文字あり（/he）
//   S1-T・S2-T は欄から Tab で 1 つ次へ（preventDefault なし・値不変）、S3-T は 1 回目が補完（preventDefault・欄に残る・値 /help）で 2 回目が通常の移動。
//   S*-S は Shift+Tab で 1 つ前へ（補完せず・preventDefault なし）。B は欄の前を試験のページの中だけ tabindex=-1 にして欄を先頭にし、Shift+Tab で body へ出る。
//   B0 は配置を変えない画面の実際の最初の操作対象から Shift+Tab で body へ出る（欄の前に上部の操作対象がある差の記録）。
//   C1 欄から Tab・Shift+Tab の一巡（K-15）。C2 一覧が開いた欄で補完した後の Tab の一巡。K7 変換中の Tab（CDP の模擬）。
// 候補のボタン（candidate-btn）は tabindex=-1 でフォーカスの順に入らないことも確かめる。
// 対照 N1〜N5: 罠を試験のページの中だけに入れ、判定が検出することを確かめる（配信物には残さない）。
//
// 確かめないこと: 処理中の一巡（進行の間隔 300ms 固定で時間に依存する）・幅 959px 以下（shell-boundary）・Firefox・Safari・実 IME と人の確認（TASK-93・TASK-386）。
// K7 の証拠の種類はシミュレーター（CDP による IME の模擬）で、実 IME でも人の評価でもない。
// 出力に、入力値や教材の文面は出さない（ID・件数・位置だけ）。待ちは固定の sleep でなく、上限つきの waitForFunction で行う。
//
// 入力: FANDHE_WEB_DIR（配信する設定。IR なし）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import {
  OUT, listFocusables, installTabProbe, pressTracked, walkCycle, focusAt,
  exitProblems, reverseProblems, completionProblems,
} from './tab-focus.mjs';
import { SIM_LABEL, openIme, imeCompose, imeCancel, imeTabKey, installKeyProbe, readKeyProbe } from './ime.mjs';
import { withDeliveryCopy } from './ch4-9-journey.mjs';
import { loadBaseIr } from './e2-ir.mjs';

const BASE = '/fandhe-tab-parts/';
const INPUT = '[data-testid="draft-input"]';
const LIST = '[data-testid="candidate-list"]';
const LOG_ITEMS = '[data-testid="part-log-list"] > li';
const T = 30000;

// 場面。start は開始点の ID、node は IR の節点。require は一巡の列挙に在るべき testid。
const SCENES = {
  W1: { start: 's-1-1', node: 'main/st-1-1', require: ['draft-input', 'submit-input', 'stop-btn'] },
  W2: { start: 's-1-6', node: 'main/st-1-6', require: ['draft-input', 'submit-input', 'stop-btn', 'choice-btn'] },
  W3: { start: 's-1-6', node: 'main/st-1-6', require: ['draft-input', 'submit-input', 'stop-btn', 'choice-btn'], prep: 'choose' },
  W4: { start: 's-2-6', node: 'main/st-2-6', require: ['draft-input', 'submit-input', 'stop-btn', 'approval-btn'] },
  W5: { start: 's-2-6', node: 'main/st-2-6', require: ['draft-input', 'submit-input', 'stop-btn', 'approval-btn'], prep: 'revise' },
};

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_DIR must point to a web directory built without TEST_HOOKS');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

function deliveryIr() {
  const ir = loadBaseIr();
  for (const s of Object.values(SCENES)) {
    assert.ok(ir.nodes.some((n) => n.uid === s.node), `node ${s.node}`);
    if (!ir.starts.some((x) => x.id === s.start)) ir.starts.push({ id: s.start, node: s.node, effects: [] });
  }
  return ir;
}

function withDelivery(csp, fn) {
  return withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, (dir) => {
    fs.writeFileSync(path.join(dir, 'ir', 'main.json'), JSON.stringify(deliveryIr()), 'utf8');
    return withServer({ dir, base: BASE }, fn);
  }));
}

function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

const listHidden = (page) => page.evaluate((sel) => document.querySelector(sel).hidden, LIST);
const draftValue = (page) => page.inputValue(INPUT);
const logCount = (page) => page.evaluate((sel) => document.querySelectorAll(sel).length, LOG_ITEMS);
const count = (page, testid) => page.evaluate((id) => document.querySelectorAll(`[data-testid="${id}"]`).length, testid);

// 場面を開いて、部品が期待どおりそろうまで準備する。W3・W5 は画面の実クリックで確認待ちにする。
async function openScene(srv, name, { ime = false } = {}) {
  const sc = SCENES[name];
  const opened = await openPage(browser, { origin: srv.origin, viewport: { width: 1024, height: 768 } });
  const { page } = opened;
  if (ime) await installKeyProbe(opened.context);
  await page.goto(`${srv.url}?start=${encodeURIComponent(sc.start)}`);
  await waitMounted(page);
  assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'delivery build must have no test hooks');
  await page.waitForSelector('[data-testid="step-title"]', { timeout: T });
  if (sc.prep === 'choose') {
    await page.waitForSelector('[data-testid="choice-btn"]', { timeout: T });
    const id = await page.evaluate(() => document.querySelector('[data-testid="choice-btn"]').getAttribute('data-choice'));
    assert.ok(id, 'choice id');
    await page.click('[data-testid="choice-btn"]');
    await page.waitForFunction(() => [...document.querySelectorAll('[data-testid="choice-btn"]')].some((b) => /^ans-/.test(b.getAttribute('data-choice') ?? '')), null, { timeout: T });
  } else if (sc.prep === 'revise') {
    await page.waitForSelector('[data-testid="approval-btn"][data-decision="revise"][data-revise-target]', { timeout: T });
    await page.click('[data-testid="approval-btn"][data-decision="revise"][data-revise-target]');
    await page.waitForFunction(() => {
      const btns = [...document.querySelectorAll('[data-testid="approval-btn"]')];
      return btns.length > 0 && !btns.some((b) => b.hasAttribute('data-revise-target'));
    }, null, { timeout: T });
  }
  for (const id of sc.require) assert.ok((await count(page, id)) >= 1, `scene ${name}: ${id} must be present`);
  await installTabProbe(page);
  return opened;
}

// 値を入れて欄にフォーカスし、一覧の開閉を前提として照合する。列挙と欄の位置を返す。
async function arm(page, value, listOpen) {
  await page.fill(INPUT, value);
  await page.focus(INPUT);
  assert.equal(await listHidden(page), !listOpen, `candidate list ${listOpen ? 'should be open' : 'should be closed'} for ${value}`);
  const lst = await listFocusables(page);
  const idx = lst.ids.indexOf('draft-input');
  assert.ok(idx >= 1, `draft-input must be preceded by a focusable: ${lst.ids.join(',')}`);
  assert.ok(!lst.ids.includes('candidate-btn'), 'candidate-btn must not be in the focus order');
  return { ...lst, idx };
}

const STATES = [
  { id: 'S1 空欄', value: '', open: false },
  { id: 'S2a 候補なし（/zzz）', value: '/zzz', open: false },
  { id: 'S2b 完全一致（/help）', value: '/help', open: false },
  { id: 'S2c Esc で閉じた後（/he）', value: '/he', open: false, esc: true },
];

async function armState(page, st) {
  let a = await arm(page, st.value, Boolean(st.esc));
  if (st.esc) {
    await page.keyboard.press('Escape');
    assert.equal(await listHidden(page), true, 'Esc closes the list');
    await page.focus(INPUT);
    a = { ...a, ...(await listFocusables(page)) };
    a.idx = a.ids.indexOf('draft-input');
  }
  return a;
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';

  for (const name of Object.keys(SCENES)) {
    test(`${name} 一覧が閉じた各状態の欄で Tab は次へ・Shift+Tab は 1 つ前へ移る（K-01・K-05・K-06・K-08・K-04）（CSP ${cspLabel}）`, async () => {
      await withDelivery(csp, async (srv) => {
        for (const st of STATES) {
          for (const key of ['Tab', 'Shift+Tab']) {
            const { page, context, rec } = await openScene(srv, name);
            const a = await armState(page, st);
            const logs = await logCount(page);
            const r = await pressTracked(page, key);
            assert.equal(r.before, a.idx);
            const value = await draftValue(page);
            const problems = key === 'Tab'
              ? exitProblems({ ...r, value, expectedValue: st.value })
              : reverseProblems({ ...r, value, expectedValue: st.value, expectedAfter: a.idx - 1 });
            assert.deepEqual(problems, [], `${name} ${st.id} ${key}: ${JSON.stringify(r)}`);
            assert.equal(await logCount(page), logs, 'Tab must not add log rows');
            assertClean(rec);
            await context.close();
          }
        }
      });
    });

    test(`${name} 一覧が開いた欄で Tab は補完（preventDefault・欄に残る）・次の Tab は移動、Shift+Tab は補完せず 1 つ前（K-03・K-04）（CSP ${cspLabel}）`, async () => {
      await withDelivery(csp, async (srv) => {
        {
          const { page, context, rec } = await openScene(srv, name);
          const a = await arm(page, '/he', true);
          assert.ok((await count(page, 'candidate-btn')) >= 1, 'candidate buttons are shown while the list is open');
          const r = await pressTracked(page, 'Tab');
          await page.waitForFunction(([sel, v]) => document.querySelector(sel).value === v, [INPUT, '/help'], { timeout: T });
          assert.deepEqual(completionProblems({ ...r, value: await draftValue(page), expectedValue: '/help' }), [], JSON.stringify(r));
          assert.equal(r.after, a.idx);
          // 補完の後で一覧が閉じ、次の Tab は通常の移動になる。
          assert.equal(await listHidden(page), true, 'list closes after completion');
          await listFocusables(page);
          const r2 = await pressTracked(page, 'Tab');
          assert.deepEqual(exitProblems({ ...r2, value: await draftValue(page), expectedValue: '/help' }), [], JSON.stringify(r2));
          assertClean(rec);
          await context.close();
        }
        {
          const { page, context, rec } = await openScene(srv, name);
          const a = await arm(page, '/he', true);
          const logs = await logCount(page);
          const r = await pressTracked(page, 'Shift+Tab');
          assert.deepEqual(reverseProblems({ ...r, value: await draftValue(page), expectedValue: '/he', expectedAfter: a.idx - 1 }), [], JSON.stringify(r));
          assert.equal(await logCount(page), logs);
          assertClean(rec);
          await context.close();
        }
      });
    });

    test(`${name} 欄が最初の操作対象なら Shift+Tab は body へ出る（B）。配置を変えない画面の最初の操作対象からも出る（B0）（CSP ${cspLabel}）`, async () => {
      await withDelivery(csp, async (srv) => {
        for (const [value, open] of [['', false], ['/he', true]]) {
          const { page, context, rec } = await openScene(srv, name);
          const { idx } = await arm(page, value, open);
          await page.evaluate((i) => window.__tabFocusables.slice(0, i).forEach((e) => e.setAttribute('tabindex', '-1')), idx);
          await page.focus(INPUT);
          const lst = await listFocusables(page);
          assert.equal(lst.ids.indexOf('draft-input'), 0, `field must be first: ${lst.ids.join(',')}`);
          const r = await pressTracked(page, 'Shift+Tab');
          assert.deepEqual(reverseProblems({ ...r, value: await draftValue(page), expectedValue: value, expectedAfter: OUT }), [], JSON.stringify(r));
          assertClean(rec);
          await context.close();
        }
        {
          const { page, context, rec } = await openScene(srv, name);
          const lst = await listFocusables(page);
          const idx = lst.ids.indexOf('draft-input');
          console.log(`# B0 ${name} csp=${cspLabel} fieldIndex=${idx} before=${JSON.stringify(lst.ids.slice(0, idx))}`);
          assert.ok(idx >= 1, 'in the unmodified screen the field is not the first target');
          await focusAt(page, 0);
          const r = await pressTracked(page, 'Shift+Tab');
          assert.equal(r.before, 0);
          assert.equal(r.after, OUT);
          assert.deepEqual(r.prevented, [false]);
          assertClean(rec);
          await context.close();
        }
      });
    });

    test(`${name} 欄からの Tab・Shift+Tab の一巡に罠が無い。補完の後の一巡も（K-15）（CSP ${cspLabel}）`, async () => {
      await withDelivery(csp, async (srv) => {
        for (const key of ['Tab', 'Shift+Tab']) {
          const { page, context, rec } = await openScene(srv, name);
          const a = await arm(page, '', false);
          const r = await walkCycle(page, key, { require: SCENES[name].require });
          const detail = JSON.stringify(r);
          assert.equal(r.start, a.idx, detail);
          assert.deepEqual(r.problems, [], detail);
          assert.ok(r.prevented.length >= r.seq.length && r.prevented.every((p) => p === false), detail);
          assert.equal(await draftValue(page), '');
          console.log(`# C1 ${name} ${key} csp=${cspLabel} ids=${JSON.stringify(r.ids)}`);
          assertClean(rec);
          await context.close();
        }
        {
          // 1 回目の Tab は補完（罠ではない）。歩行の前に分けて確かめ、その後の一巡を判定する。
          const { page, context, rec } = await openScene(srv, name);
          await arm(page, '/he', true);
          const r = await pressTracked(page, 'Tab');
          await page.waitForFunction(([sel, v]) => document.querySelector(sel).value === v, [INPUT, '/help'], { timeout: T });
          assert.deepEqual(completionProblems({ ...r, value: await draftValue(page), expectedValue: '/help' }), []);
          assert.equal(await listHidden(page), true);
          const w = await walkCycle(page, 'Tab', { require: SCENES[name].require });
          const detail = JSON.stringify(w);
          assert.deepEqual(w.problems, [], detail);
          assert.ok(w.prevented.length >= w.seq.length && w.prevented.every((p) => p === false), detail);
          assert.equal(await draftValue(page), '/help');
          assertClean(rec);
          await context.close();
        }
      });
    });
  }

  test(`K7 変換中の Tab は補完にならず preventDefault もされない（K-07。${SIM_LABEL}）（CSP ${cspLabel}）`, async () => {
    await withDelivery(csp, async (srv) => {
      const { page, context, rec } = await openScene(srv, 'W1', { ime: true });
      await page.fill(INPUT, '/he');
      await page.focus(INPUT);
      assert.equal(await listHidden(page), false);
      const cdp = await openIme(page);
      await imeCompose(cdp, 'へ');
      await page.evaluate(() => { window.__fandheKeyProbe.length = 0; });
      await imeTabKey(cdp);
      const kd = (await readKeyProbe(page)).filter((r) => r.type === 'keydown');
      assert.equal(kd.length, 1, JSON.stringify(kd));
      assert.equal(kd[0].isComposing, true, JSON.stringify(kd));
      assert.equal(kd[0].defaultPrevented, false, JSON.stringify(kd));
      assert.ok(!(await draftValue(page)).includes('/help'), 'no completion while composing');
      await imeCancel(cdp);
      assertClean(rec);
      await context.close();
    });
  });
}

// 対照: 罠を試験のページの中だけに入れ、判定が検出することを確かめる（配信物には残さない。CSP あり）。
const withTrap = (fn) => withDelivery(true, fn);

test('N1 選択肢のボタンの Tab を捕まえる罠: 欄からの一巡が検出する（CSP あり）', async () => {
  await withTrap(async (srv) => {
    const { page, context } = await openScene(srv, 'W2');
    await page.evaluate(() => {
      document.addEventListener('keydown', (e) => {
        if (e.key === 'Tab' && !e.shiftKey && e.target.getAttribute('data-testid') === 'choice-btn') e.preventDefault();
      }, true);
    });
    await arm(page, '', false);
    const w = await walkCycle(page, 'Tab', { require: SCENES.W2.require });
    assert.ok(w.problems.length >= 1, JSON.stringify(w));
    await context.close();
  });
});

test('N2 承認のボタンの Tab を捕まえて自分へ戻す罠: 欄からの一巡が検出する（CSP あり）', async () => {
  await withTrap(async (srv) => {
    const { page, context } = await openScene(srv, 'W4');
    await page.evaluate(() => {
      document.addEventListener('keydown', (e) => {
        if (e.key === 'Tab' && !e.shiftKey && e.target.getAttribute('data-testid') === 'approval-btn') { e.preventDefault(); e.target.focus(); }
      }, true);
    });
    await arm(page, '', false);
    const w = await walkCycle(page, 'Tab', { require: SCENES.W4.require });
    assert.ok(w.problems.length >= 1, JSON.stringify(w));
    await context.close();
  });
});

test('N3 停止のボタンの Shift+Tab を捕まえる罠: 逆向きの一巡が検出する（CSP あり）', async () => {
  await withTrap(async (srv) => {
    const { page, context } = await openScene(srv, 'W1');
    await page.evaluate(() => {
      document.addEventListener('keydown', (e) => {
        if (e.key === 'Tab' && e.shiftKey && e.target.getAttribute('data-testid') === 'stop-btn') e.preventDefault();
      }, true);
    });
    await arm(page, '', false);
    const w = await walkCycle(page, 'Shift+Tab', { require: SCENES.W1.require });
    assert.ok(w.problems.length >= 1 || w.prevented.some((p) => p === true), JSON.stringify(w));
    await context.close();
  });
});

test('N4 欄の Tab の補完を打ち消す罠: 補完の判定が不合格にする（CSP あり）', async () => {
  await withTrap(async (srv) => {
    const { page, context } = await openScene(srv, 'W1');
    await arm(page, '/he', true);
    await page.evaluate(() => {
      document.querySelector('[data-testid="draft-input"]').addEventListener('keydown', (e) => {
        if (e.key === 'Tab' && !e.shiftKey) e.stopImmediatePropagation();
      }, true);
    });
    const r = await pressTracked(page, 'Tab');
    const problems = completionProblems({ ...r, value: await draftValue(page), expectedValue: '/help' });
    assert.ok(problems.length >= 1, JSON.stringify(r));
    await context.close();
  });
});

test('N5 欄が最初の操作対象で Shift+Tab を捕まえる罠: body へ出ないことを検出する（CSP あり）', async () => {
  await withTrap(async (srv) => {
    const { page, context } = await openScene(srv, 'W2');
    const { idx } = await arm(page, '', false);
    await page.evaluate((i) => window.__tabFocusables.slice(0, i).forEach((e) => e.setAttribute('tabindex', '-1')), idx);
    await page.focus(INPUT);
    await listFocusables(page);
    await page.evaluate(() => {
      document.querySelector('[data-testid="draft-input"]').addEventListener('keydown', (e) => {
        if (e.key === 'Tab' && e.shiftKey) { e.preventDefault(); e.currentTarget.focus(); }
      });
    });
    const r = await pressTracked(page, 'Shift+Tab');
    const problems = reverseProblems({ ...r, value: await draftValue(page), expectedValue: '', expectedAfter: OUT });
    assert.ok(problems.length >= 1, JSON.stringify(r));
    await context.close();
  });
});
