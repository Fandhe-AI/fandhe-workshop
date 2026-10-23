// 修飾キー付き Enter と上限を超える貼り付けの ST-1.4 の画面の結果の試験（TASK-452 2/2。TERM-14・TERM-16・JUDGE-22・JUDGE-24・D-60・D-65）。
//
// 決定 D-60: Ctrl・Cmd（Meta）・Alt 付きの Enter は送信にも改行にもしない。上限を超える貼り付けは貼った分だけを
// `beforeinput` で切る（全体の末尾で切る処理は安全網として残す）。D-65: キャレットは残した貼り付けの直後に置く。
// 実装は TASK-282・TASK-283 が持つ。本試験は製品コードを変えず、教材の Step の画面（`?fixture=ch0-3` の ST-1.4）での結果を固定する。
// 既存の試験との差分: enter-submit・input-limit（J-1〜J-5）は最小の画面で値・案内・selectionStart を見る。
// 本試験は教材の画面で、ログ・世代・Step・問い返しや拒否の行・候補の一覧が変わらないことと、
// キャレットの位置の結果（次の Backspace・打鍵がどこに効くか）を見る。
//
// 証拠の種類: 自動ブラウザ。人の評価ではない。操作は page.keyboard だけ。ただし 10,000 文字目へのキャレットの配置だけは
// 矢印キー 10,000 回が遅いので setSelectionRange で置く（input-limit の pasteAt と同じ）。
// 出力・失敗の文面に下書きの中身は出さない（長さと合否だけ）。
//
// 確かめないこと: 上限ちょうどの途中での打鍵（観察 2026-10-23: 全体の末尾が切られ、キャレットが末尾へ移る。D-60 の対象外）、実キーボードの OS ごとの差（Windows。D-48）、実 IME、Safari・Firefox、他アプリからの貼り付け（TERM-20）、人の確認。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  DRAFT, CLARIFIED, REJECTED, T, engine, ledger, keyboardDriver, journey, install, keyTo, waitCount,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-modifier-enter-paste-st14/';
const STEP = 'main/st-1-4';
const LIST = '[data-testid="candidate-list"]';
const NOTICE_SEL = '[data-testid="input-notice"]';
const NOTICE = '入力は 20,000 文字までです。超えた分は受け付けていません。';
const MODS = ['Control', 'Alt', 'Meta', 'Shift+Control', 'Shift+Alt', 'Shift+Meta', 'Control+Alt'];
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

const count = (page, sel) => page.locator(sel).count();
const listHidden = (page) => page.evaluate((s) => document.querySelector(s)?.hasAttribute('hidden') ?? true, LIST);
const waitListHidden = (page, want) => page.waitForFunction(
  ([s, w]) => (document.querySelector(s)?.hasAttribute('hidden') ?? true) === w, [LIST, want], { timeout: T },
);
const waitDraft = (page, want) => page.waitForFunction(
  ([sel, v]) => document.querySelector(sel)?.value === v, [DRAFT, want], { timeout: T },
);
const raf2 = (page) => page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
const notice = async (page) => (await page.textContent(NOTICE_SEL)).trim();
const caret = (page) => page.evaluate((sel) => {
  const el = document.querySelector(sel);
  return [el.selectionStart, el.selectionEnd];
}, DRAFT);
// 変化の有無を比べる組。
const snap = async (page) => {
  const e = await engine(page);
  return {
    log: e.log.length, generation: e.stateWithoutLog.run.generation, current: e.current,
    cla: await count(page, CLARIFIED), rej: await count(page, REJECTED),
  };
};
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

async function toSt14(srv, mode) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: srv.origin });
  await page.goto(`${srv.url}?fixture=ch0-3`);
  await waitMounted(page);
  assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
  await install(page);
  const L = ledger();
  await journey(page, keyboardDriver(page), mode, L, STEP);
  assert.equal((await engine(page)).current, STEP);
  await waitDraft(page, '');
  return { page, context, rec, L };
}

async function finish(page, rec, L, scene, label) {
  const e = await engine(page);
  const run = await page.evaluate(() => window.__run);
  L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
  L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
  L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
  const counts = await collectSafety(page, rec);
  for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
  L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
  console.log(`${label} ${scene}: ${L.summary()} 項目`);
  assert.deepEqual(L.failed(), [], 'failed items');
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`修飾キー付き Enter は送信にも改行にも補完にもならない（${mode}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const scene = `修飾キー付き Enter（${mode}）`;
      const { page, context, rec, L } = await toSt14(srv, scene);

      // 修飾なしなら問い返しになる下書き。コマンドと完全一致なので候補は開かない。
      await keyTo(page, DRAFT);
      await page.keyboard.type('/new-idea');
      await waitListHidden(page, true);
      const base = await snap(page);
      for (const mod of MODS) {
        await page.keyboard.press(`${mod}+Enter`);
        await raf2(page);
        L.check(scene, `${mod}+Enter で欄の値が変わらない`, (await page.inputValue(DRAFT)) === '/new-idea');
        L.check(scene, `${mod}+Enter でログ・世代・Step・行が変わらない`, same(await snap(page), base));
        L.check(scene, `${mod}+Enter で候補が開かない`, await listHidden(page));
      }
      // 対照 1: Shift+Enter は改行になる（TERM-13）。観察が効いていることの確認。
      await page.keyboard.press('Shift+Enter');
      await raf2(page);
      L.check(scene, '対照: Shift+Enter は改行を入れる', (await page.inputValue(DRAFT)) === '/new-idea\n');
      await page.keyboard.press('Backspace');
      L.check(scene, '対照: 改行を戻せた', (await page.inputValue(DRAFT)) === '/new-idea');
      L.check(scene, '対照: 改行でログが増えない', same(await snap(page), base));
      // 対照 2: 修飾なしの Enter なら同じ下書きが送れる。
      await page.keyboard.press('Enter');
      await waitCount(page, CLARIFIED, base.cla + 1);
      await waitDraft(page, '');
      L.check(scene, '対照: 修飾なしの Enter は問い返しになる', (await count(page, CLARIFIED)) === base.cla + 1);

      // 候補が開いているとき。
      await page.keyboard.type('/ne');
      await waitListHidden(page, false);
      const open0 = await snap(page);
      for (const mod of MODS) {
        await page.keyboard.press(`${mod}+Enter`);
        await raf2(page);
        L.check(scene, `候補あり ${mod}+Enter で補完も改行もしない`, (await page.inputValue(DRAFT)) === '/ne');
        L.check(scene, `候補あり ${mod}+Enter でログ・世代・Step・行が変わらない`, same(await snap(page), open0));
      }
      // 観察（2026-10-23。Chromium）: 修飾キー付き Enter は一覧を閉じない。D-60 は一覧の開閉を決めていない。
      L.check(scene, '候補あり 修飾キー付き Enter の後も一覧が開いたまま（観察値）', !(await listHidden(page)));
      // 一覧の状態を壊していない確認: Esc の 1 回目で閉じる。
      await page.keyboard.press('Escape');
      await waitListHidden(page, true);
      L.check(scene, 'Esc の 1 回目で一覧が閉じる', await listHidden(page));
      await finish(page, rec, L, scene, 'modifier-enter-st14');
      await context.close();
    }));
  });

  test(`上限を超える途中への貼り付けは貼った分だけを切り、キャレットが直後に残る（${mode}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const scene = `貼り付けの切り捨て（${mode}）`;
      const { page, context, rec, L } = await toSt14(srv, scene);
      const A = 'a'.repeat(10000);
      const B = 'b'.repeat(9990);

      // 上限に近い下書き（19,990 文字）。
      await page.evaluate((t) => navigator.clipboard.writeText(t), A + B);
      await keyTo(page, DRAFT);
      await page.keyboard.press('ControlOrMeta+A');
      await page.keyboard.press('ControlOrMeta+V');
      await waitDraft(page, A + B);
      L.check(scene, '用意した下書きは 19,990 文字で案内なし', (await notice(page)) === '');
      const base = await snap(page);

      // 途中（10,000 文字目）へ 20 文字を貼る。入るのは 10 文字。
      await page.evaluate((t) => navigator.clipboard.writeText(t), 'x'.repeat(20));
      await page.evaluate(([sel, p]) => document.querySelector(sel).setSelectionRange(p, p), [DRAFT, 10000]);
      await page.keyboard.press('ControlOrMeta+V');
      await raf2(page);
      const v1 = await page.inputValue(DRAFT);
      L.check(scene, '貼った分だけが切られ後ろの b が 9,990 文字残る', v1 === A + 'x'.repeat(10) + B);
      L.check(scene, '長さがちょうど 20,000 文字', [...v1].length === 20000);
      L.check(scene, '上限の案内が出る', (await notice(page)) === NOTICE);
      L.check(scene, 'キャレットが残した貼り付けの直後', (await caret(page)).join() === '10010,10010');
      L.check(scene, '自動送信が無い', same(await snap(page), base));

      // 次の Backspace は貼った x に効く（末尾の b ではない）。
      await page.keyboard.press('Backspace');
      const v2 = await page.inputValue(DRAFT);
      L.check(scene, 'Backspace で消えたのは貼った x', v2 === A + 'x'.repeat(9) + B);
      L.check(scene, 'Backspace の後は案内が消える', (await notice(page)) === '');
      L.check(scene, 'Backspace の後のキャレットが 10,009', (await caret(page)).join() === '10009,10009');

      // 次の打鍵はキャレットの位置に入る。
      await page.keyboard.type('y');
      const v3 = await page.inputValue(DRAFT);
      L.check(scene, '打鍵はキャレットの位置に入る', v3 === A + 'x'.repeat(9) + 'y' + B);
      L.check(scene, '打鍵の後は上限ちょうどで案内なし', [...v3].length === 20000 && (await notice(page)) === '');
      L.check(scene, '最後までログ・世代・Step・行が変わらない', same(await snap(page), base));
      await finish(page, rec, L, scene, 'paste-st14');
      await context.close();
    }));
  });
}
