// IME の模擬による試験文 30 文の行列と集計のブラウザ試験（TASK-276 2/3。JUDGE-14・JUDGE-16。BUILD-10）。
//
// 役割: ime-input.test.mjs は代表の 5 文だけを持つ。ここでは試験文 30 文（S-01〜S-30。ime-sentences.md の判定の規則）の
// 全部を、下書き 3 通り（なし・1 行・複数行）で流し、次の 3 つの件数を集計して 0 件であることを確かめる。
//   誤送信          変換中の Enter（isComposing=true / keyCode 229）で送信の記録が増えた数
//   確定後の送信漏れ 確定後の通常の Enter で、期待どおりの文字列がちょうど 1 件送信されなかった数
//   変換中の Esc    変換中の Esc で世代が変わる・停止になる・取り消しの入力がログに残る数（取り消しの 3 文）
// 変換中の判定を外す変異 M1（Enter を除外から外す）は、下書きありの確定 Enter と複数行の 2 行目の確定 Enter が落とす。
// 誤送信の assert は keydown の表明より先に置き、失敗の文言に印 IME-MIS-SEND と誤送信の件数が出る。
// 再現は scripts/test-ime-composing-control.sh（make test-ime-composing-control。#2123）。M3（Esc を外す）は、
// 上流の action_from_keydown が変換中の keydown を先に除くため、ブラウザでは生き残る（docs/testing.md 5.503）。
// M3 の検出は、Esc の後の件数の表明に付く印 IME-ESC-STOP（件数 1 以上）で判定する。S-C の Esc の前の表明や集計の失敗では印が出ない。
// S-A が途中で落ちた場合、集計は組の欠けで失敗する。誤送信の件数は S-A の各試験の文言で読む。
// 証拠の種類はシミュレーター（CDP による IME の模擬）で、実 IME でも人の評価でもない（実 IME は TASK-93 / BUILD-18。未実施）。
// 集計の数え方（misSendCount 等）は本番の行列と対照（C-1〜C-3）で同じ関数を使い、数え方が 0 より大きく数えられることを示す。
// 最後の `集計` は、行列の全ての組が積まれていること（途中の失敗で欠けても落ちる）と、3 つの件数が 0 であることを確かめる。
//
// 配信する設定（CSP あり・なし）: Enter 系 27 文と、取り消しの 3 文（送信件数と欄の値だけ）を、(CSP × 下書き) ごとに 1 ページで流す。
// 試験の設定（TEST_HOOKS=1）: 取り消しの 3 文で、世代・停止・ログの長さ（__fandheTest）を待機中と処理中で確かめる。
// 待ち: 固定の sleep は使わず、予約済みのタイマーの後ろに自分のタイマーを積む（settle）か、値の収束を待つ。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import {
  SIM_LABEL, loadSentences, openIme, assertFocused, imeCompose, imeCommit, imeCancel,
  imeEnterKey, imeEscKey, composeSentence, installKeyProbe, readKeyProbe, clearKeyProbe,
  MIS_SEND_MARK, ESC_STOP_MARK, settle, misSendCount, missedCount, stopCount, cancelLogCount,
  assertComposingKeydown, assertNoMisSend,
} from './ime.mjs';

const BASE = '/fandhe-ime-sentences/';
const INPUT = '[data-testid="draft-input"]';
const ITEMS = '[data-testid="log-list"] > li';
const RAW = '[data-testid="raw-trigger"]';
const DRAFTS = ['none', 'single', 'multi'];
const DRAFT_LABEL = { none: 'なし', single: '1 行', multi: '複数行' };

const SENTENCES = loadSentences();
assert.equal(SENTENCES.length, 30);
assert.equal(new Set(SENTENCES.map((s) => s.id)).size, 30, '試験文の ID が一意');
const CANCEL_SENTENCES = SENTENCES.filter((s) => s.kind === 'cancel');
assert.equal(SENTENCES.length - CANCEL_SENTENCES.length, 27);
assert.equal(CANCEL_SENTENCES.length, 3);

let browser;
// { id, kind, draft, csp, state, misSend, missed, escStop }
const results = [];

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

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await installKeyProbe(opened.context);
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  await opened.page.focus(INPUT);
  await assertFocused(opened.page, INPUT);
  return { ...opened, cdp: await openIme(opened.page) };
}

function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

const itemTexts = (page) => page.$$eval(ITEMS, (lis) => lis.map((li) => li.textContent));
const keydowns = async (page) => (await readKeyProbe(page)).filter((r) => r.type === 'keydown');
const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const waitValue = (page, v) => page.waitForFunction(([sel, x]) => document.querySelector(sel)?.value === x, [INPUT, v]);

// 下書きを入れる。複数行は 1 行目・Shift+Enter・2 行目（値に改行が入る）。
async function setDraft(page, kind) {
  const text = { none: '', single: '既にある下書き', multi: '一行目の下書き\n二行目の下書き' }[kind];
  if (kind === 'multi') {
    const [a, b] = text.split('\n');
    await page.fill(INPUT, a);
    await page.focus(INPUT);
    await page.keyboard.press('Shift+Enter');
    await page.keyboard.type(b);
  } else {
    await page.fill(INPUT, text);
  }
  await page.focus(INPUT);
  await settle(page);
  assert.equal(await page.inputValue(INPUT), text, `下書き ${kind}`);
  return text;
}

async function composingEnterDelta(page, cdp) {
  const n0 = (await itemTexts(page)).length;
  await clearKeyProbe(page);
  await imeEnterKey(cdp);
  await settle(page);
  const kd = await keydowns(page);
  return { kd, delta: (await itemTexts(page)).length - n0 };
}

// Enter 系の 1 文。誤送信（変換中の Enter）と送信漏れ（確定後の Enter）を数えて積む。
async function runEnterSentence(page, cdp, s, draftKind, csp) {
  const why = `${s.id} 下書き ${DRAFT_LABEL[draftKind]} CSP ${csp}`;
  const draft = await setDraft(page, draftKind);
  const expected = draft + s.text;
  let misSend = 0;
  if (s.lines) {
    for (let i = 0; i < s.lines.length; i += 1) {
      const [reading, text] = s.lines[i];
      await imeCompose(cdp, reading);
      await imeCompose(cdp, text);
      const r = await composingEnterDelta(page, cdp);
      assertNoMisSend(r.delta, `${why} 行 ${i + 1}`);
      assertComposingKeydown(r.kd, `${why} 行 ${i + 1}`);
      misSend += misSendCount(r.delta);
      await imeCommit(cdp, text);
      await settle(page);
      if (i < s.lines.length - 1) await page.keyboard.press('Shift+Enter');
    }
  } else {
    await composeSentence(cdp, s);
    const r = await composingEnterDelta(page, cdp);
    assertNoMisSend(r.delta, why);
    assertComposingKeydown(r.kd, why);
    misSend += misSendCount(r.delta);
    await imeCommit(cdp, s.text);
    await settle(page);
  }
  assert.equal(await page.inputValue(INPUT), expected, `確定後の欄 ${why} ${SIM_LABEL}`);
  const n0 = (await itemTexts(page)).length;
  await clearKeyProbe(page);
  await page.keyboard.press('Enter');
  await waitValue(page, '');
  const items = await itemTexts(page);
  const kd = await keydowns(page);
  assert.equal(kd[0].isComposing, false, `${why} ${SIM_LABEL}`);
  assert.equal(kd[0].keyCode, 13, `${why} ${SIM_LABEL}`);
  const missed = missedCount(items.length - n0, items[items.length - 1], expected);
  results.push({ id: s.id, kind: s.kind, draft: draftKind, csp, state: 'idle', misSend, missed, escStop: 0 });
  assert.equal(misSend, 0, `誤送信 ${why} ${SIM_LABEL}`);
  assert.equal(missed, 0, `送信漏れ ${why} ${SIM_LABEL}`);
}

async function makeBusy(page) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify({ input: { command: '/make-note first-note' } })]);
  await page.focus(INPUT);
}

// 取り消しの 1 文（配信する設定）。送信件数と欄の値だけを見る。
async function runCancelDelivery(page, cdp, s, draftKind, csp) {
  const why = `${s.id} 下書き ${DRAFT_LABEL[draftKind]} CSP ${csp}`;
  const draft = await setDraft(page, draftKind);
  const n0 = (await itemTexts(page)).length;
  await composeSentence(cdp, s);
  await clearKeyProbe(page);
  await imeEscKey(cdp);
  await settle(page);
  const delta = (await itemTexts(page)).length - n0;
  const misSend = misSendCount(delta);
  assertNoMisSend(delta, why);
  assertComposingKeydown(await keydowns(page), why);
  await imeCancel(cdp);
  await settle(page);
  assert.equal(await page.inputValue(INPUT), draft, `取り消しの後の欄 ${why} ${SIM_LABEL}`);
  results.push({ id: s.id, kind: s.kind, draft: draftKind, csp, state: 'idle', misSend, missed: 0, escStop: 0 });
  assert.equal(misSend, 0, `誤送信 ${why} ${SIM_LABEL}`);
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));
  for (const draftKind of DRAFTS) {
    test(`S-A 試験文 30 文 下書き ${DRAFT_LABEL[draftKind]} 変換中の Enter で誤送信 0 件、確定後の Enter で送信漏れ 0 件（CSP ${cspLabel}）`, async () => {
      await run(async (srv) => {
        const { page, context, cdp, rec } = await open(srv);
        for (const s of SENTENCES) {
          if (s.kind === 'cancel') await runCancelDelivery(page, cdp, s, draftKind, cspLabel);
          else await runEnterSentence(page, cdp, s, draftKind, cspLabel);
        }
        assertClean(rec);
        await context.close();
      });
    });
  }
}

// 取り消しの 3 文 × 待機中 / 処理中（試験の設定）。世代・停止・ログの長さを見る。
for (const draftKind of DRAFTS) {
  test(`S-B 取り消しの 3 文 待機中の変換中の Esc はログも世代も変えない 下書き ${DRAFT_LABEL[draftKind]}`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      const { page, context, cdp, rec } = await open(srv);
      for (const s of CANCEL_SENTENCES) {
        const why = `${s.id} 待機中 下書き ${DRAFT_LABEL[draftKind]}`;
        const draft = await setDraft(page, draftKind);
        await composeSentence(cdp, s);
        const e0 = await engine(page);
        const n0 = (await itemTexts(page)).length;
        await clearKeyProbe(page);
        await imeEscKey(cdp);
        await settle(page);
        assertComposingKeydown(await keydowns(page), why);
        const e1 = await engine(page);
        const misSend = misSendCount((await itemTexts(page)).length - n0);
        const escStop = stopCount(e0, e1) + cancelLogCount(e0.logLen, e1.logLen);
        await imeCancel(cdp);
        await settle(page);
        assert.equal(await page.inputValue(INPUT), draft, `${why} ${SIM_LABEL}`);
        results.push({ id: s.id, kind: s.kind, draft: draftKind, csp: 'test', state: 'waiting', misSend, missed: 0, escStop });
        assert.equal(misSend, 0, `誤送信 ${why} ${SIM_LABEL}`);
        assert.equal(escStop, 0, `${ESC_STOP_MARK} 停止 ${escStop} 件 ${why} ${SIM_LABEL}`);
      }
      assertClean(rec);
      await context.close();
    });
  });

  for (const s of CANCEL_SENTENCES) {
    test(`S-C ${s.id} 処理中の変換中の Esc は世代・停止・進行を変えない 下書き ${DRAFT_LABEL[draftKind]}`, async () => {
      await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
        const { page, context, cdp, rec } = await open(srv);
        const why = `${s.id} 処理中 下書き ${DRAFT_LABEL[draftKind]}`;
        const draft = await setDraft(page, draftKind);
        await makeBusy(page);
        await composeSentence(cdp, s);
        const e0 = await engine(page);
        assert.equal(e0.runStatus, 'busy', `${why} ${SIM_LABEL}`);
        await clearKeyProbe(page);
        await imeEscKey(cdp);
        await settle(page);
        assertComposingKeydown(await keydowns(page), why);
        const e1 = await engine(page);
        const escStop = stopCount(e0, e1) + (e1.ignoredReasons.length === e0.ignoredReasons.length ? 0 : 1);
        await imeCancel(cdp);
        results.push({ id: s.id, kind: s.kind, draft: draftKind, csp: 'test', state: 'busy', misSend: 0, missed: 0, escStop });
        assert.equal(escStop, 0, `${ESC_STOP_MARK} 停止 ${escStop} 件 ${why} ${SIM_LABEL}`);
        assert.equal(await page.inputValue(INPUT), draft, `${why} ${SIM_LABEL}`);
        assertClean(rec);
        await context.close();
      });
    });
  }
}

// 対照: 数え方が 0 より大きく数えられる（0 件が空振りでないこと）。
test('C-1 対照 合成をせずに確定した文字列へ通常の Enter を送ると、誤送信の数え方が 1 を数える', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    await page.fill(INPUT, '今日は天気がよいので散歩に行きます');
    const n0 = (await itemTexts(page)).length;
    await page.keyboard.press('Enter');
    await waitValue(page, '');
    assert.equal(misSendCount((await itemTexts(page)).length - n0), 1);
    assertClean(rec);
    await context.close();
  });
});

test('C-2 対照 変換していない Esc は処理中を停止し、停止の数え方が 1 を数える', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    await makeBusy(page);
    const e0 = await engine(page);
    assert.equal(e0.runStatus, 'busy');
    await page.keyboard.press('Escape');
    await settle(page);
    assert.equal(stopCount(e0, await engine(page)), 1);
    assertClean(rec);
    await context.close();
  });
});

test('C-3 対照 待機中で変換していない Esc は取り消しをログに残し、数え方が 1 を数える', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    const e0 = await engine(page);
    await page.keyboard.press('Escape');
    await settle(page);
    assert.equal(cancelLogCount(e0.logLen, (await engine(page)).logLen), 1);
    assertClean(rec);
    await context.close();
  });
});

// 全ての組が積まれ、3 つの件数が 0 であること。最後に置く（node:test はファイル内を上から順に流す）。
test('集計 試験文 30 文 × 下書き 3 通りで、誤送信 0 件・確定後の送信漏れ 0 件・変換中の Esc での停止 0 件', (t) => {
  const expectedKeys = [];
  for (const s of SENTENCES) {
    for (const d of DRAFTS) {
      for (const csp of ['あり', 'なし']) expectedKeys.push(`${s.id}|${d}|${csp}|idle`);
      if (s.kind === 'cancel') {
        expectedKeys.push(`${s.id}|${d}|test|waiting`);
        expectedKeys.push(`${s.id}|${d}|test|busy`);
      }
    }
  }
  const got = results.map((r) => `${r.id}|${r.draft}|${r.csp}|${r.state}`).sort();
  assert.deepEqual(got, expectedKeys.sort(), `行列の全ての組が積まれている ${SIM_LABEL}`);
  const sum = (k) => results.reduce((n, r) => n + r[k], 0);
  t.diagnostic(`組 ${results.length} 件 / 誤送信 ${sum('misSend')} 件 / 送信漏れ ${sum('missed')} 件 / 変換中の Esc での停止・取り消し ${sum('escStop')} 件 ${SIM_LABEL}`);
  assert.equal(sum('misSend'), 0, `誤送信 ${SIM_LABEL}`);
  assert.equal(sum('missed'), 0, `確定後の送信漏れ ${SIM_LABEL}`);
  assert.equal(sum('escStop'), 0, `変換中の Esc での停止 ${SIM_LABEL}`);
});
