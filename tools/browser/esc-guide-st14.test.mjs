// 候補が開いているときの Esc の ST-1.4 の場面と案内の画面の結果の試験（TASK-452 1/2。TERM-9・JUDGE-18・D-60）。
//
// 決定 D-60: 候補の一覧が開いているときの Esc は、1 回目で一覧を閉じるだけ、2 回目で取り消す。この順序は案内文で教える。
// 現行の仮置きのまま採るため、エンジンの変更は無い。本試験は、その観察を固定する。
// 既存の通し（ch0-3-journey）は `/advance-phase`（コマンドと完全一致で候補が開かない）で Esc を押すので 1 回で済む。
// 本試験は、問い返しの後に `/ne` と打って候補が開いた状態で Esc を押す、取り消しが受け付けられる場面を確かめる。
// esc-outcomes（K-09）との差分: そちらは取り消しが拒否になる Step。こちらは ST-1.4 で、取り消しが受け付けられる。
//
// 証拠の種類: 自動ブラウザ。人の評価ではない。
// 場面: `?fixture=ch0-3`（案内はエンジンが足さず、IR の文面だけが出る）と、
// `?fixture=ch0-3-esc-guide`（案内文を順序を教える文面に差し替えた変種。その案内が出る）。各 CSP あり・なし。
// 操作は page.keyboard だけ。出力に下書きの中身は出さない（件数と合否だけ）。
//
// 確かめないこと: 本物の ST-1.4 の案内文（教材 repo の持ち分）、実 IME、Safari・Firefox、人の確認。
// 変種の文面は crates/web/src/test_fixture.rs の CH03_ESC_GUIDE_HOW_TO_ANSWER と同じ文字列を持つ（変えるときは両方を直す）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  DRAFT, CLARIFIED, REJECTED, T, engine, ledger, keyboardDriver, journey, install, keyTo, waitCount, at,
  ports, rowSel, choiceSel,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-esc-guide-st14/';
const STEP = 'main/st-1-4';
const LIST = '[data-testid="candidate-list"]';
const ESC_GUIDE = '名前を付けてもう一度送ります。取り消すときは Esc を押します。候補の一覧が開いているときは、1 回目の Esc で一覧が閉じ、2 回目で取り消します。';
let browser;

const ir = JSON.parse(fs.readFileSync(new URL('../../crates/web/tests/data/ch0_3/ir.json', import.meta.url), 'utf8'));
const clarify = ir.clarifications['cl-new-idea-name'];

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
const stepStatus = (page) => page.evaluate(
  (u) => JSON.parse(window.__fandheTest.stateJson()).engine.stateWithoutLog.run.slices.main?.status?.[u] ?? 'entered', STEP,
);
// 問い返しの行の部品。子が文字ノード 1 つであること（A03）も返す。
const clarifiedParts = (page) => page.evaluate((s) => {
  const li = [...document.querySelectorAll(s)].at(-1);
  const part = (t) => {
    const el = li?.querySelector(`[data-testid="${t}"]`);
    return el ? { text: el.textContent, plain: el.childNodes.length === 1 && el.childNodes[0].nodeType === 3 } : null;
  };
  return { question: part('line-question'), next: part('line-next-action') };
}, CLARIFIED);

const SCENES = [
  { query: '?fixture=ch0-3', label: '基準の教材', guide: clarify.how_to_answer },
  { query: '?fixture=ch0-3-esc-guide', label: '順序を教える案内の変種', guide: ESC_GUIDE },
];

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  for (const sc of SCENES) {
    test(`候補が開いているときの Esc は 1 回目で閉じ 2 回目で取り消す: ${sc.label}（${mode}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const scene = `${sc.label}（${mode}）`;
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        await page.goto(`${srv.url}${sc.query}`);
        await waitMounted(page);
        assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
        await install(page);
        const L = ledger();
        await journey(page, keyboardDriver(page), scene, L, STEP);
        assert.equal((await engine(page)).current, STEP);
        await waitDraft(page, '');

        // 問い返しまで。案内は IR の文面がそのまま出る。
        const cla0 = await count(page, CLARIFIED);
        await keyTo(page, DRAFT);
        await page.keyboard.type('/new-idea');
        await page.keyboard.press('Enter');
        await waitCount(page, CLARIFIED, cla0 + 1);
        await waitDraft(page, '');
        const parts = await clarifiedParts(page);
        L.check(scene, '問いの文面が教材の文面', parts.question?.text === clarify.message);
        L.check(scene, '案内が期待した文面ちょうど', parts.next?.text === sc.guide);
        L.check(scene, '問い・案内が文字ノード 1 つ', parts.question?.plain === true && parts.next?.plain === true);

        // 候補を開く。案内は読める位置に残る。
        await page.keyboard.type('/ne');
        await waitListHidden(page, false);
        const btnText = await page.locator(`${LIST} button`).allTextContents();
        L.check(scene, '候補が 1 件で /new-idea を含む', btnText.length === 1 && btnText[0].includes('/new-idea'));
        L.check(scene, '候補が開いても案内の行が残る', (await count(page, CLARIFIED)) === cla0 + 1);

        const before = await engine(page);
        const rej0 = await count(page, REJECTED);
        // 1 回目の Esc: 一覧を閉じるだけ。
        await page.keyboard.press('Escape');
        await waitListHidden(page, true);
        const mid = await engine(page);
        L.check(scene, '1 回目で一覧が閉じた', await listHidden(page));
        L.check(scene, '1 回目でログ・世代・Step が変わらない',
          mid.logLen === before.logLen && mid.generation === before.generation && mid.current === before.current);
        L.check(scene, '1 回目で拒否の行が増えない', (await count(page, REJECTED)) === rej0);
        L.check(scene, '1 回目で欄の値が残る', (await page.inputValue(DRAFT)) === '/ne');
        L.check(scene, '1 回目で Step が受付済みでない', (await stepStatus(page)) !== 'accepted');

        // 2 回目の Esc: 取り消しが受け付けられる。
        await page.keyboard.press('Escape');
        await at(page, STEP, 'accepted');
        const after2 = await engine(page);
        L.check(scene, '2 回目で確認待ちの口になる', (await ports(page)).join() === '2,0,1,1');
        L.check(scene, '2 回目で拒否の行が増えない', (await count(page, REJECTED)) === rej0);
        L.check(scene, '2 回目で左に cards/idea が出る', (await count(page, rowSel('cards/idea'))) === 1);
        L.check(scene, '2 回目の後も Step は同じ', after2.current === STEP);
        // 手元の観察（2026-10-23。Chromium）で固定した値: 受付で logLen は 5 増え、最後の種類は accepted、欄の値は残る。
        L.check(scene, '2 回目の後のログの増分が観察値 5', after2.logLen - mid.logLen === 5);
        L.check(scene, '2 回目の最後の判定が受付', after2.log.at(-1)?.kind === 'accepted');
        L.check(scene, '2 回目の後も欄の値が残る', (await page.inputValue(DRAFT)) === '/ne');

        // 2 回目の Esc が Step を正しく先へ進められる状態にしたことの確認。
        await keyTo(page, choiceSel('asked-back'));
        await page.keyboard.press('Enter');
        await at(page, 'main/st-1-5', 'entered');
        L.check(scene, '続けて main/st-1-5 へ進める', (await engine(page)).current === 'main/st-1-5');

        const e = await engine(page);
        const run = await page.evaluate(() => window.__run);
        L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
        L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
        L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
        const counts = await collectSafety(page, rec);
        for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
        L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
        console.log(`esc-guide-st14 ${scene}: ${L.summary()} 項目`);
        assert.deepEqual(L.failed(), [], 'failed items');
        await context.close();
      }));
    });
  }
}
