// ライブの欄 `live-response` の更新回数を、応答ごとに DOM の観察で数える試験（A11Y-6。TASK-370 1/2。#971）。
//
// 役割: 新しい応答が右のログに出たら、読み上げ用の欄 `live-response` を、その応答の行を含めて「応答ごとにちょうど 1 回」更新し、
// 利用者自身の入力の写し（`received`）と処理の途中経過（`progress`）だけが増えた更新では触らない、という契約（`a11y::counts_for_live_response`）を
// 実際の画面で固定する。`workshop-root` に MutationObserver を 1 つ付け、コールバック 1 回（= 1 回の `update` の DOM の書き込み）を「更新の束」として、
// 束ごとに `live-response` の記録の数・束の後の欄の文字・足されたログの行の種類と部品の文字を記録して判定する。
//
// 確かめること:
//   R1  試験用 IR の CH-1（`?fixture=ch0-3-chapters&start=s-1-1`）を実クリックで通し、全部の束で
//       - 数える種類（received・progress 以外）の行が足された束は `live-response` の記録がちょうど 1 件で、束の後の文字が「応答 」で始まり
//         足された数える行の部品の文字をすべて含む
//       - received・progress だけの束と行が足されない束は `live-response` の記録が 0 件
//       - 応答の総数と `live-response` の記録の総数が一致する
//       を満たす。2 つの欄（`live-response`・`live-left`）の属性（aria-live=polite・aria-atomic=true・role なし・子は文字だけ（空の間は 0 個）・親は
//       workshop-root・tabpanel の外）は、マウントの直後・束ごと・終わりに判定する。アクセシビリティツリー（CDP）では、拒否・問い返し・停止・終わりの各時点で
//       2 つの欄が live=polite・atomic=true の性質を持ち、`live-response` の部分木の文字が DOM と一致する。CSP あり・なし。
//   N1  同じタスクの中で `live-response` を 2 回書くと、観察器が 2 件と数える（判定が失敗する）。
//   N2  合成した束の列（2 回・0 回・received だけの束での更新）は判定が不合格にする。
//   N3  写した属性から `aria-atomic` を外すと、属性の判定が失敗する。
//
// 確かめないこと: 読み上げそのもの（VoiceOver での読まれ方は TASK-381）。`live-left` の文（#972・A11Y-8）。幅は 1280px だけ。
// 応答の数は固定しない（試験用の教材の CH-1 は 6 Step で、本物の教材は別の repo にある）。注記の無い受付（Accepted(None)）は契約どおり応答に数えない。
// 出力に入力した値と欄の文は出さず、件数と種類だけを出す。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages,
} from './harness.mjs';
import {
  engine, ledger, at, ports, waitCount, mouseDriver, REJECTED, CLARIFIED, T, LOG,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-live-response/';
const QUERY = '?fixture=ch0-3-chapters&start=s-1-1';
const PLAIN = [0, 0, 1, 1];
const CHOICES2 = [2, 0, 1, 1];
const STAGES = 4;
const STATUS = '[data-testid="run-status"]';
const PROGRESS = `${LOG} li[data-kind="progress"]`;
const STOPPED = `${LOG} li[data-kind="stopped"]`;
const EXCLUDED = ['received', 'progress'];
const REGIONS = ['live-response', 'live-left'];

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// ---- ページ内の観察器。束ごとの記録を window.__lr に積む。
const OBSERVER = ({ regions, excluded }) => {
  const root = document.querySelector('[data-testid="workshop-root"]');
  const state = { batches: [] };
  window.__lr = state;
  const attrsOk = () => regions.every((id) => {
    const els = document.querySelectorAll(`[data-testid="${id}"]`);
    if (els.length !== 1) return false;
    const el = els[0];
    return el.getAttribute('aria-live') === 'polite' && el.getAttribute('aria-atomic') === 'true'
      && !el.hasAttribute('role') && el.childNodes.length <= 1 && [...el.childNodes].every((c) => c.nodeType === 3)
      && el.parentElement === root && !el.closest('[role="tabpanel"]');
  });
  const within = (id, node) => {
    const el = document.querySelector(`[data-testid="${id}"]`);
    return !!el && (el === node || el.contains(node));
  };
  const obs = new MutationObserver((records) => {
    const added = [];
    let live = 0;
    let left = 0;
    for (const r of records) {
      if (within('live-response', r.target)) live += 1;
      if (within('live-left', r.target)) left += 1;
      if (r.target instanceof Element && r.target.matches('[data-testid="part-log-list"]')) {
        for (const n of r.addedNodes) {
          if (n instanceof Element && n.matches('li[data-kind]')) {
            added.push({
              kind: n.getAttribute('data-kind'),
              parts: [...n.querySelectorAll('[data-testid^="line-"]')].map((p) => p.textContent ?? ''),
            });
          }
        }
      }
    }
    const el = document.querySelector('[data-testid="live-response"]');
    state.batches.push({ live, left, after: el?.textContent ?? null, added, attrsOk: attrsOk(), counting: added.filter((a) => !excluded.includes(a.kind)).length });
  });
  obs.observe(root, { subtree: true, childList: true, characterData: true });
  state.attrsOk = attrsOk;
  state.disconnect = () => obs.disconnect();
  return attrsOk();
};

// 束の列の判定。問題の説明の配列を返す（空なら合格）。入力した値は載せない。
export function judgeBatches(batches) {
  const problems = [];
  batches.forEach((b, i) => {
    const counting = b.added.filter((a) => !EXCLUDED.includes(a.kind));
    if (counting.length > 0) {
      if (b.live !== 1) problems.push(`束 ${i}: 応答の行が足されたのに live-response の更新が ${b.live} 回`);
      if (!(typeof b.after === 'string' && b.after.startsWith('応答 '))) problems.push(`束 ${i}: 束の後の文字が「応答 」で始まらない`);
      for (const a of counting) {
        for (const p of a.parts) {
          if (!(b.after ?? '').includes(p)) problems.push(`束 ${i}: ${a.kind} の部品の文字が欄に含まれない`);
        }
      }
    } else if (b.live !== 0) {
      problems.push(`束 ${i}: 応答の行が無いのに live-response の更新が ${b.live} 回（足された行 ${b.added.map((a) => a.kind).join(',') || 'なし'}）`);
    }
    if (!b.attrsOk) problems.push(`束 ${i}: 欄の属性の判定が不合格`);
  });
  const responses = batches.filter((b) => b.added.some((a) => !EXCLUDED.includes(a.kind))).length;
  const liveTotal = batches.reduce((s, b) => s + b.live, 0);
  if (responses !== liveTotal) problems.push(`応答の総数 ${responses} と更新の総数 ${liveTotal} が違う`);
  return problems;
}

// 属性の写しの判定。写しは OBSERVER の attrsOk と同じ条件を、要素ごとの値で持つ。
const snapAttrs = (page) => page.evaluate((regions) => {
  const root = document.querySelector('[data-testid="workshop-root"]');
  return regions.map((id) => {
    const els = document.querySelectorAll(`[data-testid="${id}"]`);
    const el = els[0];
    return {
      id,
      count: els.length,
      live: el?.getAttribute('aria-live') ?? null,
      atomic: el?.getAttribute('aria-atomic') ?? null,
      role: el?.getAttribute('role') ?? null,
      textChild: !!el && el.childNodes.length <= 1 && [...el.childNodes].every((c) => c.nodeType === 3),
      rootChild: !!el && el.parentElement === root,
      inTabpanel: !!el?.closest('[role="tabpanel"]'),
    };
  });
}, REGIONS);

export function attrProblems(snap) {
  const out = [];
  for (const s of snap) {
    if (s.count !== 1) out.push(`${s.id} が ${s.count} 個`);
    if (s.live !== 'polite') out.push(`${s.id} の aria-live が polite でない`);
    if (s.atomic !== 'true') out.push(`${s.id} の aria-atomic が true でない`);
    if (s.role !== null) out.push(`${s.id} に role がある`);
    if (!s.textChild) out.push(`${s.id} の子が文字だけでない`);
    if (!s.rootChild) out.push(`${s.id} の親が workshop-root でない`);
    if (s.inTabpanel) out.push(`${s.id} が tabpanel の中にある`);
  }
  return out;
}

// アクセシビリティツリー（CDP）から、欄のノードの性質と部分木の文字を求める。
async function axOf(cdp, page) {
  const { root } = await cdp.send('DOM.getDocument', { depth: -1 });
  const ids = {};
  for (const id of REGIONS) {
    const { nodeId } = await cdp.send('DOM.querySelector', { nodeId: root.nodeId, selector: `[data-testid="${id}"]` });
    const { node } = await cdp.send('DOM.describeNode', { nodeId });
    ids[id] = node.backendNodeId;
  }
  const { nodes } = await cdp.send('Accessibility.getFullAXTree');
  const byId = new Map(nodes.map((n) => [n.nodeId, n]));
  const prop = (n, name) => n.properties?.find((p) => p.name === name)?.value?.value;
  const textOf = (n) => {
    if (n.role?.value === 'StaticText' || n.role?.value === 'InlineTextBox') return n.name?.value ?? '';
    return (n.childIds ?? []).map((c) => textOf(byId.get(c))).join('');
  };
  const out = {};
  for (const id of REGIONS) {
    const n = nodes.find((x) => x.backendDOMNodeId === ids[id]);
    out[id] = n ? { live: prop(n, 'live'), atomic: prop(n, 'atomic'), text: textOf(n) } : null;
  }
  return out;
}

async function openCh1(csp, fn) {
  let L;
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 900 } });
    L = ledger();
    await page.goto(`${srv.url}${QUERY}`);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    const cdp = await context.newCDPSession(page);
    await cdp.send('DOM.enable');
    await cdp.send('Accessibility.enable');
    await fn({ page, rec, L, cdp });
    await context.close();
  }));
  return L;
}

const statusOf = async (page, uid) => (await engine(page)).stateWithoutLog.run.slices.main.status[uid];
const batches = (page) => page.evaluate(() => window.__lr.batches);

// 束が落ち着くまで（観察器の記録が 2 回続けて同じ件数）待つ。
async function settle(page) {
  let last = -1;
  for (let i = 0; i < 20; i += 1) {
    const n = (await batches(page)).length;
    if (n === last) return;
    last = n;
    await page.waitForTimeout(60);
  }
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  test(`R1: CH-1 を通し、応答の束ごとに live-response がちょうど 1 回更新される（${mode}）`, { timeout: 300000 }, async () => {
    const L = await openCh1(csp, async ({ page, rec, L: led, cdp }) => {
      const S = 'R1';
      await at(page, 'main/st-1-1', 'entered');
      const mounted = await page.evaluate(OBSERVER, { regions: REGIONS, excluded: EXCLUDED });
      led.check(S, 'マウントの直後に 2 つの欄の属性が正しい', mounted === true);
      led.check(S, 'マウントの直後の属性の写しが合格', attrProblems(await snapAttrs(page)).length === 0);
      const d = mouseDriver(page);
      const axChecks = [];
      const checkAx = async (name) => {
        await settle(page);
        const ax = await axOf(cdp, page);
        const dom = await page.evaluate(() => document.querySelector('[data-testid="live-response"]').textContent);
        const ok = REGIONS.every((id) => ax[id]?.live === 'polite' && ax[id]?.atomic === true) && ax['live-response']?.text === dom;
        led.check(S, `AX ツリー（${name}）で 2 つの欄が polite・atomic で文字が DOM と一致`, ok);
        axChecks.push(name);
      };
      let rejected = 0;
      let clarified = 0;
      const reject = async (uid) => {
        rejected += 1;
        await waitCount(page, REJECTED, rejected);
        led.check(S, `${uid} 拒否で進まない`, (await engine(page)).current === uid);
      };
      const clarify = async (uid) => {
        clarified += 1;
        await waitCount(page, CLARIFIED, clarified);
        led.check(S, `${uid} 問い返しで進まない`, (await engine(page)).current === uid);
      };
      const confirmFlow = async (uid, expectedPorts, act, next) => {
        await at(page, uid, 'accepted');
        led.check(S, `${uid} 確認待ちの口`, JSON.stringify(await ports(page)) === JSON.stringify(expectedPorts));
        await act();
        if (next) await at(page, next, 'entered');
        else await at(page, uid, 'achieved');
        led.check(S, `${uid} 確認で達成`, (await statusOf(page, uid)) === 'achieved');
      };

      // st-1-1: 拒否（応答）→ 受付（received だけ）→ 左の行を指す。
      await d.send('/hepl');
      await reject('main/st-1-1');
      await checkAx('拒否');
      await d.send('/help');
      await confirmFlow('main/st-1-1', PLAIN, () => d.point('cards/help-1'), 'main/st-1-2');
      await d.completeAndSend('/he', '/help');
      await at(page, 'main/st-1-3', 'entered');
      await d.sendMultiline('memo line 1', 'memo line 2');
      await at(page, 'main/st-1-4', 'entered');
      // st-1-4: 問い返し → 順序違いの拒否 → Esc → 選択肢。
      await d.send('/new-idea');
      await clarify('main/st-1-4');
      await checkAx('問い返し');
      await d.send('/advance-phase');
      await reject('main/st-1-4');
      await d.esc();
      await confirmFlow('main/st-1-4', CHOICES2, () => d.choose('asked-back'), 'main/st-1-5');
      // st-1-5: 処理中（progress だけの束が続く）に Esc で停止 → 送り直し。
      await at(page, 'main/st-1-5', 'entered');
      await d.send('/make-card sample');
      await page.waitForFunction(([st, pr, n]) => document.querySelector(st)?.getAttribute('data-state') === 'busy'
        && document.querySelectorAll(pr).length >= 1 && document.querySelectorAll(pr).length < n, [STATUS, PROGRESS, STAGES], { timeout: T });
      await d.esc();
      await page.waitForFunction((s) => document.querySelector(s)?.getAttribute('data-state') === 'stopped', STATUS, { timeout: T });
      led.check(S, '停止の行が 1 件', (await page.locator(STOPPED).count()) === 1);
      await checkAx('停止');
      await d.send('/make-card sample');
      await confirmFlow('main/st-1-5', PLAIN, () => d.point('cards/sample'), 'main/st-1-6');
      // st-1-6: 選択肢。
      await at(page, 'main/st-1-6', 'entered');
      await d.choose('opt-right');
      await confirmFlow('main/st-1-6', CHOICES2, () => d.choose('ans-restored'), null);
      await settle(page);
      await checkAx('終わり');

      const bs = await batches(page);
      const problems = judgeBatches(bs);
      for (const p of problems) led.check(S, p, false);
      led.check(S, '束の判定が合格', problems.length === 0);
      led.check(S, '終わりの属性の写しが合格', attrProblems(await snapAttrs(page)).length === 0);

      // 空振りの防止。種類ごとの束の数。
      const kindsOf = (b) => b.added.map((a) => a.kind);
      const responses = bs.filter((b) => b.counting > 0);
      const has = (k) => responses.some((b) => kindsOf(b).includes(k));
      led.check(S, '応答が 3 回以上', responses.length >= 3);
      led.check(S, 'rejected の応答が 1 回以上', has('rejected'));
      led.check(S, 'clarified の応答が 1 回以上', has('clarified'));
      led.check(S, 'stopped の応答が 1 回以上', has('stopped'));
      const receivedOnly = bs.filter((b) => b.added.length > 0 && b.added.every((a) => a.kind === 'received'));
      const progressOnly = bs.filter((b) => b.added.length > 0 && b.added.every((a) => a.kind === 'progress'));
      led.check(S, 'received だけの束が 1 回以上で、どれも更新 0 回', receivedOnly.length >= 1 && receivedOnly.every((b) => b.live === 0));
      led.check(S, 'progress だけの束が 1 回以上で、どれも更新 0 回', progressOnly.length >= 1 && progressOnly.every((b) => b.live === 0));
      led.check(S, 'live-left は文を持たず更新 0 回（#972 まで）', bs.every((b) => b.left === 0));

      const counts = await collectSafety(page, rec);
      for (const k of nonZero(counts)) led.check(S, `安全の計数 ${k} が 0`, false);
      led.check(S, 'ページ内エラーが 0 件', rec.pageErrors.length === 0 && !rec.crashed);
      led.check(S, 'コンソールの CSP 警告が 0 件', cspConsoleMessages(rec).length === 0);
      led.check(S, '配信元の外への通信が 0 件', rec.external.length === 0);

      // 実測の記録（入力・欄の文は出さない。件数と種類だけ）。
      const perKind = {};
      for (const b of responses) for (const k of new Set(kindsOf(b))) perKind[k] = (perKind[k] ?? 0) + 1;
      console.log(`R1 (${mode}): 束 ${bs.length}・応答 ${responses.length}（種類別 ${JSON.stringify(perKind)}）・received だけ ${receivedOnly.length}・progress だけ ${progressOnly.length}・live-response の更新 ${bs.reduce((s, b) => s + b.live, 0)}・AX 確認 ${axChecks.length} 回`);
    });
    console.log(`R1 (${mode}): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), []);
  });
}

// ---- 負の対照 ----
const synth = (added, live, after = '応答 1: x', attrsOk = true) => ({ live, left: 0, after, added, attrsOk, counting: added.length });

test('N1: 同じタスクで live-response を 2 回書くと、観察器が 2 件と数える', { timeout: 120000 }, async () => {
  let two;
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 900 } });
    await page.goto(`${srv.url}${QUERY}`);
    await waitMounted(page);
    await page.evaluate(OBSERVER, { regions: REGIONS, excluded: EXCLUDED });
    await page.evaluate(() => {
      const el = document.querySelector('[data-testid="live-response"]');
      el.textContent = '応答 9: a';
      el.textContent = '応答 10: b';
    });
    await settle(page);
    two = await batches(page);
    await context.close();
  }));
  assert.equal(two.length, 1, '1 つの束');
  assert.equal(two[0].live, 2, '2 回の書き込みは 2 件');
  assert.ok(judgeBatches([{ ...two[0], added: [{ kind: 'rejected', parts: ['b'] }], counting: 1 }]).length > 0, '2 回は不合格');
});

test('N2: 合成した束の列（2 回・0 回・received だけの束での更新）は不合格になる', () => {
  const rejected = [{ kind: 'rejected', parts: ['理由'] }];
  const received = [{ kind: 'received', parts: ['/x'] }];
  assert.deepEqual(judgeBatches([synth(rejected, 1, '応答 1: 理由')]), [], '基準は合格');
  assert.ok(judgeBatches([synth(rejected, 2, '応答 1: 理由')]).length > 0, '2 回');
  assert.ok(judgeBatches([synth(rejected, 0, '応答 1: 理由')]).length > 0, '0 回');
  assert.ok(judgeBatches([synth(received, 1, '応答 1: ')]).length > 0, 'received だけで更新');
  assert.ok(judgeBatches([synth([{ kind: 'progress', parts: ['s'] }], 1)]).length > 0, 'progress だけで更新');
  assert.ok(judgeBatches([synth(rejected, 1, '応答 1: 別の文')]).length > 0, '行の部品の文字が欄に無い');
  assert.deepEqual(judgeBatches([synth(received, 0, '')]), [], 'received だけで更新 0 回は合格');
});

test('N3: 写した属性から aria-atomic を外すと、属性の判定が失敗する', () => {
  const ok = REGIONS.map((id) => ({ id, count: 1, live: 'polite', atomic: 'true', role: null, textChild: true, rootChild: true, inTabpanel: false }));
  assert.deepEqual(attrProblems(ok), []);
  const bad = ok.map((s, i) => (i === 0 ? { ...s, atomic: null } : s));
  assert.ok(attrProblems(bad).some((p) => p.includes('aria-atomic')));
  assert.ok(attrProblems(ok.map((s) => ({ ...s, role: 'status' }))).length > 0, 'role が付くと失敗');
  assert.ok(attrProblems(ok.map((s) => ({ ...s, inTabpanel: true }))).length > 0, 'tabpanel の中は失敗');
});
