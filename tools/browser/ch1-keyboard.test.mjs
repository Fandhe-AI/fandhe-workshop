// キーボードだけで CH-1 の 6 Step を完走し、マウスの click が 0 回であることの試験（TASK-374 1/2。A11Y-17。#978）。
//
// 役割: 5.507 の keyboard-only（20 Step の通し）は CH-0 から始まり、確認の Step で「受付の時点では未達成で、確認のキーで初めて達成になる」
// ことを判定していない。本試験は CH-1（開始点 s-1-1）だけを対象に、許すキー（Tab・Enter・Space・Shift+Enter・Esc と文字）に閉じたドライバで通し、
// 確認のある 4 Step（st-1-1・st-1-4・st-1-5・st-1-6）の受付 → 確認 → 達成の遷移を判定する。#979（2/2）が拒否の後の保持・停止・復帰を足す土台になる。
//
// 確かめること:
//   U1  試験用 IR の CH-1 の 6 Step の入力方式・確認の種類・宛先が、本ファイルの表（CH1）と一致する（ブラウザなし。操作の順序のずれの検出）。
//   K1  `?fixture=ch0-3-chapters&start=s-1-1` を CH-1 の先頭から st-1-6 の達成まで通す（CSP あり・なし）。ページ内のマウスのイベント 0・
//       ドライバ側の監視 0・許さないキー 0・raw-trigger の click 0。確認のキーは Enter（左の行・選択肢）と Space（左の行・選択肢）の両方を使う。
//   N1  確認のキーを押さないと達成にならない（時間が経っても受付のまま）。
//   N2  実クリックは、ドライバ側とページ内の 2 つの計数に出て、終わりの判定が失敗する。
//
// 確かめないこと: 拒否の後の下書きの保持・処理中の停止・チェックポイントの復帰・Tab の回数の判定（#979）。本物の教材（TASK-612〜620）。
// 幅 960px 未満の tablist（矢印キーが要る）。信頼されない click（detail 0）の計数。出力に入力した値は出さない。
// 操作の順序は ch0-3-journey.mjs の journey の CH-1 と同じだが、共有の部品を変えないため本ファイルに写して持つ。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages,
} from './harness.mjs';
import {
  engine, ledger, install, at, ports, waitCount, keyTo, rowSel, REJECTED, CLARIFIED, NEXT, T,
} from './ch0-3-journey.mjs';
import {
  ALLOWED_KEYS, installInputCounter, guardPage, keyOnlyDriver,
} from './keyboard-only.mjs';

const BASE = '/fandhe-ch1-keyboard/';
const QUERY = '?fixture=ch0-3-chapters&start=s-1-1';
const PLAIN = [0, 0, 1, 1];
const CHOICES2 = [2, 0, 1, 1];

// CH-1 の表。確認の宛先は point_at なら path、answer なら正答の id。
const CH1 = [
  { uid: 'main/st-1-1', input: 'command', confirm: 'point_at', target: 'cards/help-1' },
  { uid: 'main/st-1-2', input: 'command', confirm: 'none', target: null },
  { uid: 'main/st-1-3', input: 'text', confirm: 'none', target: null },
  { uid: 'main/st-1-4', input: 'command', confirm: 'answer', target: 'asked-back' },
  { uid: 'main/st-1-5', input: 'command', confirm: 'point_at', target: 'cards/sample' },
  { uid: 'main/st-1-6', input: 'choice', confirm: 'answer', target: 'ans-restored' },
];

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// IR のノードから確認の宛先を取り出す。形が違えば null。
function confirmTarget(step) {
  const c = step?.confirm;
  if (!c) return null;
  if (c.kind === 'point_at') return c.targets?.[0]?.path ?? null;
  if (c.kind === 'answer') return (c.options ?? c.choices ?? []).find((o) => o.correct === true)?.id ?? null;
  return null;
}

test('U1: 試験用 IR の CH-1 の 6 Step が本ファイルの表と一致する', () => {
  const ir = JSON.parse(fs.readFileSync(new URL('../../crates/web/tests/data/ch0_3/ir.json', import.meta.url), 'utf8'));
  const nodes = ir.nodes ?? ir.graph?.nodes ?? ir;
  const find = (uid) => (Array.isArray(nodes) ? nodes.find((n) => n.uid === uid || n.id === uid) : nodes[uid]);
  for (const row of CH1) {
    const n = find(row.uid);
    assert.ok(n, `${row.uid} が IR に無い`);
    const step = n.step ?? n;
    assert.equal(step.input?.kind, row.input, `${row.uid} の入力方式`);
    assert.equal(step.confirm?.kind ?? 'none', row.confirm, `${row.uid} の確認の種類`);
    if (row.target !== null) assert.equal(confirmTarget(step), row.target, `${row.uid} の確認の宛先`);
  }
  console.log(`U1: ${CH1.length} Step の表が IR と一致`);
});

// 試験の設定の画面を、計数つきの context で CH-1 の先頭から開く。
async function openCh1(csp, fn) {
  let L;
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 900 } });
    const counter = await installInputCounter(context);
    L = ledger();
    await page.goto(`${srv.url}${QUERY}`);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    await install(page);
    await fn({ page, rec, L, counter });
    await context.close();
  }));
  return L;
}

const achievedCount = async (page) => (await engine(page)).log.filter((r) => r.kind === 'achieved').length;
const statusOf = async (page, uid) => (await engine(page)).stateWithoutLog.run.slices.main.status[uid];

// 終わりの判定。台帳に項目名と合否だけを積む。Space は文字入力の空白も数えるので、押した回数はドライバ側が主。
async function finish({ page, rec, L, counter }, g, scene) {
  const c = await counter.read(page);
  L.check(scene, 'ページ内のマウスのイベントが 0', c.mouse === 0);
  L.check(scene, '許さないキーの keydown が 0', c.forbidden.length === 0);
  L.check(scene, 'ドライバ側の監視が 0', g.log.calls === 0);
  for (const k of ALLOWED_KEYS) L.check(scene, `ドライバ側で ${k} が 1 回以上`, g.keyboard.used[k] >= 1);
  L.check(scene, 'キーの集合の外が 0', Object.keys(c.keys).every((k) => ALLOWED_KEYS.includes(k)));
  const run = await page.evaluate(() => window.__run);
  L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
  const e = await engine(page);
  L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
  L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
  const status = e.stateWithoutLog.run.slices.main.status;
  L.check(scene, 'CH-1 の 6 Step がすべて達成', CH1.every((r) => status[r.uid] === 'achieved'));
  L.check(scene, '現在の Step が st-1-6', e.current === 'main/st-1-6');
  L.check(scene, '達成が 6 件', e.log.filter((r) => r.kind === 'achieved').length === 6);
  const counts = await collectSafety(page, rec);
  for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
  L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
  L.check(scene, 'ページ内エラーが 0 件', rec.pageErrors.length === 0 && !rec.crashed);
  L.check(scene, 'コンソールの CSP 警告が 0 件', cspConsoleMessages(rec).length === 0);
  L.check(scene, '配信元の外への通信が 0 件', rec.external.length === 0);
  return c;
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  test(`K1: CH-1 の 6 Step を Tab・Enter・Space・Shift+Enter・Esc と文字だけで達成まで通す（${mode}）`, { timeout: 300000 }, async () => {
    const L = await openCh1(csp, async (ctx) => {
      const { page } = ctx;
      const S = 'K1';
      await at(page, 'main/st-1-1', 'entered');
      ctx.L.check(S, '開始点が st-1-1 で CH-0 の状態が 0 件', Object.keys((await engine(page)).stateWithoutLog.run.slices.main.status).every((u) => !u.startsWith('main/st-0-')));
      ctx.L.check(S, '開始直後の口', JSON.stringify(await ports(page)) === JSON.stringify(PLAIN));
      const g = guardPage(page);
      const d = keyOnlyDriver(g.page);
      let rejected = 0;
      let clarified = 0;
      const reject = async (uid) => {
        rejected += 1;
        await waitCount(page, REJECTED, rejected);
        ctx.L.check(S, `${uid} 拒否で進まない`, (await engine(page)).current === uid);
      };
      const clarify = async (uid) => {
        clarified += 1;
        await waitCount(page, CLARIFIED, clarified);
        ctx.L.check(S, `${uid} 問い返しで進まない`, (await engine(page)).current === uid);
      };
      // 確認のある Step。受付の時点で未達成、確認のキーで達成が 1 増える。
      const confirmFlow = async (uid, expectedPorts, act, next) => {
        await at(page, uid, 'accepted');
        ctx.L.check(S, `${uid} 受付の時点で未達成`, (await statusOf(page, uid)) === 'accepted');
        ctx.L.check(S, `${uid} 確認待ちの口`, JSON.stringify(await ports(page)) === JSON.stringify(expectedPorts));
        const n0 = await achievedCount(page);
        await act();
        if (next) await at(page, next, 'entered');
        else await at(page, uid, 'achieved');
        ctx.L.check(S, `${uid} 確認のキーで達成`, (await statusOf(page, uid)) === 'achieved');
        ctx.L.check(S, `${uid} 達成が 1 件だけ増える`, (await achievedCount(page)) === n0 + 1);
      };
      // 確認のない Step。受付と同時に達成になり、次の Step に入る。
      const noConfirm = async (uid, next) => {
        await at(page, next, 'entered');
        ctx.L.check(S, `${uid} 確認なしで達成`, (await statusOf(page, uid)) === 'achieved');
      };

      // st-1-1: 拒否 → 受付 → 左の行を Enter で指す。
      await d.send('/hepl');
      await reject('main/st-1-1');
      await d.send('/help');
      await confirmFlow('main/st-1-1', PLAIN, () => d.point('cards/help-1'), 'main/st-1-2');
      // st-1-2: Tab 補完。
      await d.completeAndSend('/he', '/help');
      await noConfirm('main/st-1-2', 'main/st-1-3');
      // st-1-3: Shift+Enter の複数行。
      await d.sendMultiline('memo line 1', 'memo line 2');
      await noConfirm('main/st-1-3', 'main/st-1-4');
      // st-1-4: 問い返し → 順序違いの拒否 → Esc の取り消し → 選択肢を Enter で答える。
      await d.send('/new-idea');
      await clarify('main/st-1-4');
      await d.send('/advance-phase');
      await reject('main/st-1-4');
      await d.esc();
      await confirmFlow('main/st-1-4', CHOICES2, () => d.choose('asked-back', 'Enter'), 'main/st-1-5');
      // st-1-5: 処理中を待って受付 → 左の行を Space で指す。
      await d.send('/make-card sample');
      await confirmFlow('main/st-1-5', PLAIN, async () => {
        await keyTo(g.page, rowSel('cards/sample'));
        await g.page.keyboard.press('Space');
      }, 'main/st-1-6');
      // st-1-6: 選択肢 → 選択肢を Space で答える。最後の達成で完走。
      await at(page, 'main/st-1-6', 'entered');
      await d.choose('opt-right');
      await confirmFlow('main/st-1-6', CHOICES2, () => d.choose('ans-restored', 'Space'), null);
      await waitCount(page, NEXT, 1);

      const c = await finish(ctx, g, S);
      console.log(`K1 (${mode}): キー ${JSON.stringify(c.keys)}・Tab ${g.keyboard.used.Tab} 回（観察のみ）・文字 ${c.chars}・マウス ${c.mouse}・監視 ${g.log.calls}`);
    });
    console.log(`K1 (${mode}): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), []);
  });
}

test('N1: 確認のキーを押さないと、時間が経っても達成にならない', { timeout: 120000 }, async () => {
  await openCh1(true, async ({ page }) => {
    await at(page, 'main/st-1-1', 'entered');
    const g = guardPage(page);
    const d = keyOnlyDriver(g.page);
    await d.send('/help');
    await at(page, 'main/st-1-1', 'accepted');
    for (let i = 0; i < 5; i += 1) await g.page.keyboard.press('Tab');
    await page.waitForTimeout(500);
    assert.equal(await statusOf(page, 'main/st-1-1'), 'accepted');
    assert.equal((await engine(page)).current, 'main/st-1-1');
    assert.equal(await achievedCount(page), 0);
  });
});

test('N2: 実クリックは、ドライバ側とページ内の 2 つの計数に出て、終わりの判定が失敗する', { timeout: 120000 }, async () => {
  let failed = [];
  await openCh1(true, async (ctx) => {
    const { page } = ctx;
    await at(page, 'main/st-1-1', 'entered');
    const g = guardPage(page);
    const d = keyOnlyDriver(g.page);
    await d.send('/help');
    await at(page, 'main/st-1-1', 'accepted');
    await g.page.click(rowSel('cards/help-1'), { timeout: T });
    assert.ok(g.log.calls >= 1);
    assert.ok((await ctx.counter.read(page)).mouse >= 1);
    await finish(ctx, g, 'N2');
    failed = ctx.L.failed();
  });
  assert.ok(failed.includes('N2: ドライバ側の監視が 0'), failed.join(' / '));
  assert.ok(failed.includes('N2: ページ内のマウスのイベントが 0'), failed.join(' / '));
});
