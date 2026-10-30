// 上部のヒントを 1 段ずつ開く操作と表示の試験（SHELL-14・GUIDE-17。TASK-530 1/2）。
//
// 試験用の教材 `ch0-3`（`?fixture=ch0-3-chapters`）で、実際のボタンの click・キーだけで「ヒントを開く」を押し、開いた段が
// 上部の区画に出ることを確かめる（TASK-530 1/2）。上限を超える押下の断りと開いた回数の履歴への記録は場面 G・H（TASK-530 2/2。
// docs/testing.md 5.393）、通信の計数は egress-count.test.mjs の G4 の担当である。
//   場面 A  st-0-1（ヒント 1 つ）で押すと段が 1 つ出て、文面が IR の `text` と一致し、状態の文字が 1 / 1。ボタンの要素は同じノードのまま。
//           区画は shell-top の中・tabpanel の外
//   場面 B  キーボード（Tab でボタンへ移り Enter）で開け、押した後もフォーカスがボタンに残る
//   場面 C  Step を進めると前の Step の段が消え、ヒントの無い Step ではボタンも 0 件。ヒントのある次の Step では段が 0 件から始まる
//   場面 D  幅 360px・959px でも区画が見え、横スクロールが無い
//   場面 F  配信する設定（IR なし）では区画の項目が 0 件
//   場面 G  試験の設定で、出せる段（1 段）をすべて開いた後に押しても段が増えず、状態の使用回数が 1 のまま、操作の記録が押下ごとに増え、
//           書き出しの snapshot の使用回数と ops の open_hint の件数（押した回数。拒否を含む）が一致し、再読み込みの後も増えない
//   場面 H  配信する設定（3 段＋水準 C の 4 つ目の IR）で、1 回ずつ押すと 1・2・3 段、上限を超えて押しても 3 段のまま、4 つ目の文面が
//           画面のどこにも出ず、書き出しの snapshot が 3・ops の open_hint が 6 件、再読み込みの後も 3 段。試験用の口が無く、cookie と URL は変わらない
//   ops の open_hint は押した回数（拒否を含む）、snapshot の使用回数は開いた段数で、両者は一致しない。画面の段数は web が上限で丸めるため、
//   上限の判定は DOM ではなく状態（stateJson・書き出し）で行う。書き出しのダウンロードは安全性の計数に nonHttp として数えられるため、
//   場面 G は安全性の計数を取らない（通信の計数は egress-count の G3・G4 が持つ）。
// 試験の出力に、ヒントの文面そのものは出さない（一致の可否だけ）。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_WEB_DIR（配信する設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { ledger, mouseDriver, journey } from './ch0-3-journey.mjs';
import { withDeliveryCopy } from './ch4-9-journey.mjs';
import { exportByUi } from './shell-journey-trips.mjs';
import { hintLimitIr, LIMIT_UID, LIMIT_TEXTS, HIDDEN_TEXT } from './hint-limit-ir.mjs';

const BASE = '/fandhe-hint-open/';
const QUERY = '?fixture=ch0-3-chapters';
const OPEN = '[data-testid="hint-open"]';
const STAGE = '[data-testid="hint-stage"]';
const TEXT = '[data-testid="hint-text"]';
const STATUS = '[data-testid="hint-status"]';
const SECTION = '[data-testid="shell-hint"]';
let browser;
let tmpDir;

const IR = JSON.parse(fs.readFileSync(
  path.join(path.dirname(fileURLToPath(import.meta.url)), '../../crates/web/tests/data/ch0_3/ir.json'),
  'utf8',
));

// IR から独自に引いた、Step のヒントの文面（定義の順）。
const hintTexts = (uid) => IR.nodes.find((n) => n.uid === uid).step.hints.map((id) => IR.hints[id].text);

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-hint-open-'));
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

const stageTexts = (page) => page.evaluate(([s, t]) => [...document.querySelectorAll(`${s} ${t}`)].map((e) => e.textContent), [STAGE, TEXT]);

async function openScene(csp, viewport, query, fn, { dir = process.env.FANDHE_WEB_TEST_DIR, hooks = true } = {}) {
  await withCspMode({ dir, csp }, (d) => withServer({ dir: d, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport });
    const L = ledger();
    await page.goto(`${srv.url}${query}`);
    await waitMounted(page);
    await fn(page, L);
    const s = summarize(rec);
    L.check('共通', 'ページ内のエラーが 0 件', s.pageErrors === 0);
    L.check('共通', 'CSP 違反が 0 件', s.csp === 0);
    if (hooks) assert.deepEqual(nonZero(await collectSafety(page, rec)), [], JSON.stringify({ rec }));
    console.log(`ヒント (CSP ${csp ? 'あり' : 'なし'}, ${viewport.width}px): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  }));
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`ボタンの click で開いた段が出て、文面が IR と一致する（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, { width: 1280, height: 800 }, QUERY, async (page, L) => {
      const want = hintTexts('main/st-0-1');
      L.check('A', '開く前は段が 0 件・ボタンが 1 個', (await page.locator(STAGE).count()) === 0 && (await page.locator(OPEN).count()) === 1);
      const place = await page.evaluate(([b, s]) => {
        const e = document.querySelector(b);
        return {
          inTop: !!e.closest('[data-testid="shell-top"]') && !!e.closest(s),
          inPanel: !!e.closest('[role="tabpanel"]'),
        };
      }, [OPEN, SECTION]);
      L.check('A', 'ボタンが shell-top の shell-hint の中・tabpanel の外', place.inTop && !place.inPanel);
      await page.evaluate((b) => { window.__hintBtn = document.querySelector(b); }, OPEN);
      await page.click(OPEN);
      await page.waitForSelector(STAGE);
      L.check('A', '段が 1 つ・番号が 1', await page.evaluate((s) => {
        const e = document.querySelectorAll(s);
        return e.length === 1 && e[0].getAttribute('data-hint-stage') === '1';
      }, STAGE));
      L.check('A', '文面が IR の定義の順と一致', JSON.stringify(await stageTexts(page)) === JSON.stringify(want.slice(0, 1)));
      L.check('A', '状態の文字の数が 1 / 全体', await page.evaluate(([s, total]) => {
        const e = document.querySelector(s);
        return e?.getAttribute('data-hint-opened') === '1' && e.getAttribute('data-hint-total') === String(total);
      }, [STATUS, want.length]));
      L.check('A', 'ボタンの要素が同じノードのまま', await page.evaluate((b) => window.__hintBtn === document.querySelector(b) && window.__hintBtn.isConnected, OPEN));
    });
  });

  test(`キーボードだけで開け、押した後もフォーカスがボタンに残る（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, { width: 1280, height: 800 }, QUERY, async (page, L) => {
      let reached = false;
      for (let i = 0; i < 40 && !reached; i += 1) {
        await page.keyboard.press('Tab');
        reached = await page.evaluate((b) => document.activeElement === document.querySelector(b), OPEN);
      }
      L.check('B', 'Tab でボタンへ届く', reached);
      await page.keyboard.press('Enter');
      await page.waitForSelector(STAGE);
      L.check('B', 'Enter で段が 1 つ出る', (await page.locator(STAGE).count()) === 1);
      L.check('B', '押した後もフォーカスがボタンにある', await page.evaluate((b) => document.activeElement === document.querySelector(b), OPEN));
    });
  });

  test(`Step を進めると前の Step の段が消える（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, { width: 1280, height: 800 }, QUERY, async (page, L) => {
      await page.click(OPEN);
      await page.waitForSelector(STAGE);
      let sawNoButton = false;
      let sawFresh = false;
      const onStep = async (uid, status) => {
        if (status !== 'entered') return;
        if (uid === 'main/st-0-2a') {
          sawNoButton = (await page.locator(OPEN).count()) === 0 && (await page.locator(STAGE).count()) === 0;
        }
        if (uid === 'main/st-1-1') {
          sawFresh = (await page.locator(STAGE).count()) === 0 && (await page.locator(OPEN).count()) === 1;
        }
      };
      await journey(page, mouseDriver(page), 'C', L, 'main/st-1-1', { chaptered: true, onStep });
      L.check('C', 'ヒントの無い Step ではボタンも段も 0 件', sawNoButton);
      L.check('C', 'ヒントのある次の Step に入った時点で段が 0 件・ボタンが 1 個', sawFresh);
    });
  });
}

test('幅 360px と 959px で区画が見え、横スクロールが無い', { timeout: 300000 }, async () => {
  for (const width of [360, 959]) {
    await openScene(true, { width, height: 800 }, QUERY, async (page, L) => {
      await page.click(OPEN);
      await page.waitForSelector(STAGE);
      const m = await page.evaluate(([b, s]) => {
        const r = document.querySelector(b)?.getBoundingClientRect();
        const t = document.querySelector(s)?.getBoundingClientRect();
        return {
          visible: !!r && r.width > 0 && r.height > 0 && !!t && t.width > 0,
          scroll: document.documentElement.scrollWidth <= document.documentElement.clientWidth,
        };
      }, [OPEN, STAGE]);
      L.check(`D${width}`, 'ボタンと段が見える', m.visible);
      L.check(`D${width}`, '横スクロールが無い', m.scroll);
    });
  }
});

test('配信する設定（IR なし）ではヒントの項目が 0 件', { timeout: 300000 }, async () => {
  await openScene(true, { width: 1280, height: 800 }, '', async (page, L) => {
    L.check('F', '区画はあり、ボタンと段が 0 件',
      (await page.locator(SECTION).count()) === 1 && (await page.locator(`${OPEN}, ${STAGE}`).count()) === 0);
  }, { dir: process.env.FANDHE_WEB_DIR, hooks: false });
});

// 書き出した履歴から、使用回数（snapshot）と open_hint の操作の件数（ops。押した回数）を読む。値そのものは出さない。
const readExport = (file) => {
  const h = JSON.parse(fs.readFileSync(file, 'utf8'));
  return { hints: h.snapshot.hints_opened ?? {}, pressed: h.ops.filter((o) => JSON.stringify(o).includes('open_hint')).length };
};

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`G: 出せる段をすべて開いた後の押下は段を増やさず、使用回数は開いた段数のまま履歴に残る（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, { width: 1280, height: 800 }, QUERY, async (page, L) => {
      const engine = () => page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine);
      const opened = async () => (await engine()).stateWithoutLog.hints_opened?.[LIMIT_UID] ?? 0;
      await page.evaluate((b) => { window.__hintBtn = document.querySelector(b); }, OPEN);
      await page.click(OPEN);
      await page.waitForSelector(STAGE);
      L.check('G', '1 回目で使用回数が 1・段が 1 件', (await opened()) === 1 && (await page.locator(STAGE).count()) === 1);
      const logBefore = (await engine()).logLen;
      for (let i = 0; i < 3; i += 1) {
        const before = (await engine()).logLen;
        await page.click(OPEN);
        await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before, { timeout: 15000 });
      }
      L.check('G', '上限を超える 3 回の押下で使用回数が 1 のまま', (await opened()) === 1);
      L.check('G', '段が 1 件のまま・状態の文字が 1 / 1', await page.evaluate((s) => {
        const e = document.querySelector(s);
        return document.querySelectorAll('[data-testid="hint-stage"]').length === 1
          && e?.getAttribute('data-hint-opened') === '1' && e.getAttribute('data-hint-total') === '1';
      }, STATUS));
      L.check('G', 'ボタンの要素が同じノードのまま', await page.evaluate((b) => window.__hintBtn === document.querySelector(b), OPEN));
      L.check('G', '拒否の押下ごとに操作の記録が 1 件増えた', (await engine()).logLen === logBefore + 3);
      const file = await exportByUi(page, tmpDir);
      const ex = readExport(file);
      L.check('G', '書き出しの使用回数が 1・ops の open_hint が押した回数の 4 件', ex.hints[LIMIT_UID] === 1 && ex.pressed === 4);
      await page.reload();
      await waitMounted(page);
      await page.waitForFunction(() => window.__fandheTest?.stateJson && JSON.parse(window.__fandheTest.stateJson()).engine.installCount > 0, null, { timeout: 30000 });
      L.check('G', '再読み込みの後も段が 1 件', (await page.locator(STAGE).count()) === 1);
      const before = (await engine()).logLen;
      await page.click(OPEN);
      await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before, { timeout: 15000 });
      L.check('G', '再読み込みの後に押しても段・使用回数が増えない', (await page.locator(STAGE).count()) === 1 && (await opened()) === 1);
      fs.rmSync(file, { force: true });
    }, { hooks: false });
  });

  test(`H: 配信する設定で 3 段を開き、上限を超えても段が増えず、水準外の文面が出ない（${mode}）`, { timeout: 300000 }, async () => {
    await withDeliveryCopy(async (copy) => {
      fs.writeFileSync(path.join(copy, 'ir', 'main.json'), JSON.stringify(hintLimitIr()));
      await openScene(csp, { width: 1280, height: 800 }, '', async (page, L) => {
        L.check('H', '配信する設定に試験用の口が無い', (await page.evaluate(() => typeof window.__fandheTest)) === 'undefined');
        L.check('H', '開始の Step にヒントのボタンがある', (await page.locator(OPEN).count()) === 1);
        const url0 = await page.evaluate(() => location.href);
        const press = async (k) => {
          await page.click(OPEN);
          if (k <= 3) await page.waitForFunction((n) => document.querySelectorAll('[data-testid="hint-stage"]').length === n, k, { timeout: 15000 });
          else await page.waitForTimeout(300);
        };
        for (let k = 1; k <= 3; k += 1) {
          await press(k);
          L.check('H', `${k} 回目で段が ${k} 件・文面が定義の順と一致`, JSON.stringify(await stageTexts(page)) === JSON.stringify(LIMIT_TEXTS.slice(0, k)));
        }
        for (let k = 4; k <= 6; k += 1) await press(k);
        L.check('H', '上限を超える 3 回の押下の後も段が 3 件・文面が同じ', JSON.stringify(await stageTexts(page)) === JSON.stringify(LIMIT_TEXTS));
        L.check('H', '状態の文字が 3 / 3', await page.evaluate((s) => {
          const e = document.querySelector(s);
          return e?.getAttribute('data-hint-opened') === '3' && e.getAttribute('data-hint-total') === '3';
        }, STATUS));
        L.check('H', '水準 C の 4 つ目の文面が画面にも属性にも出ない', await page.evaluate((t) => {
          if (document.body.innerText.includes(t)) return false;
          return ![...document.querySelectorAll('*')].some((e) => [...e.attributes].some((a) => a.value.includes(t)));
        }, HIDDEN_TEXT));
        L.check('H', '文面のマークアップの形が文字として出て要素にならない', await page.evaluate((s) => !document.querySelector(`${s} b`), STAGE));
        const file = await exportByUi(page, tmpDir);
        const ex = readExport(file);
        L.check('H', '書き出しの使用回数が 3・ops の open_hint が押した回数の 6 件', ex.hints[LIMIT_UID] === 3 && ex.pressed === 6);
        fs.rmSync(file, { force: true });
        await page.reload();
        await waitMounted(page);
        await page.waitForFunction((n) => document.querySelectorAll('[data-testid="hint-stage"]').length === n, 3, { timeout: 30000 });
        L.check('H', '再読み込みの後も段が 3 件', (await page.locator(STAGE).count()) === 3);
        L.check('H', 'cookie が空で URL が変わっていない', await page.evaluate((u) => document.cookie === '' && location.href === u, url0));
      }, { dir: copy, hooks: false });
    });
  });
}
