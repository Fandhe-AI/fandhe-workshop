// 本体の画面での変換中の確定の Enter・変換中の Esc の結果の試験（TASK-439。TERM-3・TERM-6。#989。BUILD-10）。
//
// 役割: ime-sentences.test.mjs（TASK-276 2/3）は試験文 30 文 × 下書き 3 通りを IR なしの画面（Hello の入力欄）で流し、
// 数えているのは Hello の送信済み一覧 `log-list` だった。本体の右のログ（`part-log-list`）・状態表示（`run-status`）・
// Step の状態（`step-status`）は見ていない。ここでは教材の IR を載せた本体の画面で同じ 30 文を流し、次を確かめる。
//   誤送信          変換中の Enter で右のログの行が増える数（受付の行・全行・世代・状態表示・Step の状態の変化）
//   確定後の送信漏れ 確定後の通常の Enter で、期待の文字列の受付の行がちょうど 1 件出て、欄が空になり、次の Step へ進まなかった数
//   変換中の Esc    変換中の Esc でログ・世代・状態表示が変わる数（待機中・処理中。処理中は進行の行だけが増えてよい）
// 数え方（misSendCount 等）と印（IME-MIS-SEND・IME-ESC-STOP）は ime.mjs の共有で、ime-sentences と同じ関数を使う。
// 数え方が 0 より大きく数えられることは対照（C-1〜C-3）で示す。判定・延期・送信の結線は TASK-276・TASK-355・TASK-438 が持つ。
//
// 画面: 試験の設定（TEST_HOOKS=1）の `?fixture=ch0-3`（本体の画面。`st-1-3` は自由な文を受け付ける text の Step で、
// 受け付けると `st-1-4` へ進む）。文ごとに `raw-trigger` の `go_to` で `st-1-3` へ戻す。処理中は `st-1-5` で `/make-card sample` を送る。
// 配信する設定（IR を同梱。試験の口なし。DOM だけで判定）: 開始点 `s-1-3`・`s-1-5` を足した IR を `ir/main.json` に置き、
// CSP あり・なしで、文ごとに新しい context で `?start=` を開く（保存先が新しいため戻す手段が要らない）。
//
// 証拠の種類はシミュレーター（CDP による IME の模擬）で、実 IME でも人の評価でもない。
// 確かめないこと: 実 IME（macOS の日本語入力・Chrome。TASK-93 / BUILD-18。未実施）・Firefox・Safari・人の確認。
// 出力に、入力値や下書きの中身は出さない（ID・件数・合否だけ）。欄の値は読むだけで、状態へ流さない（BUILD-6）。
// 待ち: 固定の sleep は使わず、予約済みのタイマーの後ろに自分のタイマーを積む（settle）か、上限つきで値の収束を待つ。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import {
  SIM_LABEL, loadSentences, openIme, assertFocused, imeCompose, imeCommit, imeCancel,
  imeEnterKey, imeEscKey, composeSentence, installKeyProbe, readKeyProbe, clearKeyProbe,
  ESC_STOP_MARK, settle, misSendCount, missedCount, stopCount, cancelLogCount,
  assertComposingKeydown, assertNoMisSend,
} from './ime.mjs';
import { withDeliveryCopy } from './ch4-9-journey.mjs';
import { loadBaseIr } from './e2-ir.mjs';

const BASE = '/fandhe-ime-screen/';
const QUERY = '?fixture=ch0-3';
const WAIT = 'main/st-1-3';
const BUSY = 'main/st-1-5';
const INPUT = '[data-testid="draft-input"]';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const T = 30000;
const DRAFTS = ['none', 'single', 'multi'];
const DRAFT_LABEL = { none: 'なし', single: '1 行', multi: '複数行' };
const COMMAND = '/make-card sample';

const SENTENCES = loadSentences();
assert.equal(SENTENCES.length, 30);
const CANCEL_SENTENCES = SENTENCES.filter((s) => s.kind === 'cancel');
const ENTER_SENTENCES = SENTENCES.filter((s) => s.kind !== 'cancel');
assert.equal(ENTER_SENTENCES.length, 27);
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

function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

// 画面の観察（DOM だけ）。右のログの行（種類と本文）・状態表示・Step の状態・Step の題・欄の値・Hello の一覧の行数を 1 回で読む。
const snap = (page) => page.evaluate(([log, input]) => {
  const rows = [...document.querySelectorAll(`${log} > li`)].map((li) => ({
    kind: li.getAttribute('data-kind') ?? '',
    body: (li.querySelector('[data-testid="line-body"]') ?? li).textContent ?? '',
  }));
  const run = document.querySelector('[data-testid="run-status"]');
  return {
    rows,
    run: { state: run?.getAttribute('data-state') ?? null, text: run?.textContent ?? null },
    step: document.querySelector('[data-testid="step-status"]')?.textContent ?? null,
    title: document.querySelector('[data-testid="step-title"]')?.textContent ?? null,
    field: document.querySelector(input)?.value ?? null,
    hello: document.querySelectorAll('[data-testid="log-list"] > li').length,
  };
}, [LOG, INPUT]);

const received = (s) => s.rows.filter((r) => r.kind === 'received');
const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const sendRaw = (page, op) => page.evaluate(([sel, json]) => {
  const el = document.querySelector(sel);
  el.setAttribute('data-payload', json);
  el.click();
}, [RAW, JSON.stringify(op)]);
const keydowns = async (page) => (await readKeyProbe(page)).filter((r) => r.type === 'keydown');
const waitField = (page, v) => page.waitForFunction(([sel, x]) => document.querySelector(sel)?.value === x, [INPUT, v], { timeout: T });

// 変換中のキーの前後で、ログ・状態表示・Step の状態が不変であること（待機中）。変化の件数を返す（0 が期待）。
function changedCount(a, b) {
  const kinds = (s) => JSON.stringify(s.rows);
  return (kinds(a) === kinds(b) ? 0 : 1) + (JSON.stringify(a.run) === JSON.stringify(b.run) ? 0 : 1)
    + (a.step === b.step ? 0 : 1) + (a.title === b.title ? 0 : 1) + (a.hello === b.hello ? 0 : 1);
}

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

// 変換中のキーを 1 回送り、前後の観察と keydown を返す。
async function composingKey(page, cdp, key) {
  const before0 = await snap(page);
  await clearKeyProbe(page);
  await (key === 'Enter' ? imeEnterKey(cdp) : imeEscKey(cdp));
  await settle(page);
  return { b: before0, a: await snap(page), kd: await keydowns(page) };
}

// 試験の設定で Step へ移る（go_to）。移ったことを前提として確かめる。
async function goTo(page, uid) {
  await sendRaw(page, { control: { go_to: uid } });
  await page.waitForFunction((u) => JSON.parse(window.__fandheTest.stateJson()).engine.current === u, uid, { timeout: T });
  await page.focus(INPUT);
  await settle(page);
}

// Enter 系の 1 文。hooks が真なら試験の設定（世代・状態の下書きも見る）、偽なら DOM だけ。
async function runEnter(page, cdp, s, draftKind, csp, hooks) {
  const why = `${s.id} 下書き ${DRAFT_LABEL[draftKind]} CSP ${csp} ${hooks ? '試験の設定' : '配信設定'}`;
  const draft = await setDraft(page, draftKind);
  const expected = draft + s.text;
  const e0 = hooks ? await engine(page) : null;
  const guard = async (ph) => {
    const r = await composingKey(page, cdp, 'Enter');
    assertNoMisSend(received(r.a).length - received(r.b).length + (r.a.rows.length - r.b.rows.length) + (r.a.hello - r.b.hello), `${why} ${ph}`);
    assert.equal(changedCount(r.b, r.a), 0, `変換中の Enter で画面が変わらない ${why} ${ph} ${SIM_LABEL}`);
    assertComposingKeydown(r.kd, `${why} ${ph}`);
    return misSendCount(r.a.rows.length - r.b.rows.length);
  };
  let misSend = 0;
  if (s.lines) {
    for (let i = 0; i < s.lines.length; i += 1) {
      const [reading, text] = s.lines[i];
      await imeCompose(cdp, reading);
      await imeCompose(cdp, text);
      misSend += await guard(`行 ${i + 1}`);
      await imeCommit(cdp, text);
      await settle(page);
      if (i < s.lines.length - 1) await page.keyboard.press('Shift+Enter');
    }
  } else {
    await composeSentence(cdp, s);
    misSend += await guard('');
    await imeCommit(cdp, s.text);
    await settle(page);
  }
  if (hooks) {
    const e1 = await engine(page);
    assert.equal(e1.generation, e0.generation, `変換中の Enter で世代が変わらない ${why} ${SIM_LABEL}`);
  }
  assert.equal(await page.inputValue(INPUT), expected, `確定後の欄 ${why} ${SIM_LABEL}`);
  const s0 = await snap(page);
  await clearKeyProbe(page);
  await page.keyboard.press('Enter');
  await waitField(page, '');
  await page.waitForFunction(([log, n]) => document.querySelectorAll(`${log} > li[data-kind="received"]`).length === n, [LOG, received(s0).length + 1], { timeout: T }).catch(() => {});
  await settle(page);
  const s1 = await snap(page);
  const kd = await keydowns(page);
  assert.equal(kd[0].isComposing, false, `${why} ${SIM_LABEL}`);
  assert.equal(kd[0].keyCode, 13, `${why} ${SIM_LABEL}`);
  const rs = received(s1);
  const missedRow = missedCount(rs.length - received(s0).length, rs.at(-1)?.body.replaceAll('\n', ''), expected.replaceAll('\n', ''));
  const missedStep = s1.title === s0.title ? 1 : 0;
  const missedDraft = s1.field === '' && (!hooks || (await page.evaluate(() => window.__fandheTest.draft())) === '') ? 0 : 1;
  const missed = missedRow + missedStep + missedDraft;
  results.push({ id: s.id, kind: s.kind, draft: draftKind, csp, state: hooks ? 'idle' : 'dom', misSend, missed, escStop: 0 });
  assert.equal(misSend, 0, `誤送信 ${why} ${SIM_LABEL}`);
  assert.equal(missedRow, 0, `送信漏れ（受付の行が 1 件・本文が期待どおり）${why} ${SIM_LABEL}`);
  assert.equal(missedStep, 0, `送信漏れ（次の Step へ進む）${why} ${SIM_LABEL}`);
  assert.equal(missedDraft, 0, `送信漏れ（欄と状態の下書きが空）${why} ${SIM_LABEL}`);
}

// 取り消しの 1 文（待機中）。ログ・世代・状態表示が変わらない。
async function runCancelIdle(page, cdp, s, draftKind, csp, hooks) {
  const why = `${s.id} 待機中 下書き ${DRAFT_LABEL[draftKind]} CSP ${csp} ${hooks ? '試験の設定' : '配信設定'}`;
  const draft = await setDraft(page, draftKind);
  await composeSentence(cdp, s);
  const e0 = hooks ? await engine(page) : null;
  const r = await composingKey(page, cdp, 'Escape');
  assertComposingKeydown(r.kd, why);
  const e1 = hooks ? await engine(page) : null;
  const escStop = (hooks ? stopCount(e0, e1) + cancelLogCount(e0.logLen, e1.logLen) : 0)
    + cancelLogCount(r.b.rows.length, r.a.rows.length) + changedCount(r.b, r.a);
  const misSend = misSendCount(received(r.a).length - received(r.b).length);
  await imeCancel(cdp);
  await settle(page);
  assert.equal(await page.inputValue(INPUT), draft, `取り消しの後の欄 ${why} ${SIM_LABEL}`);
  results.push({ id: s.id, kind: s.kind, draft: draftKind, csp, state: hooks ? 'waiting' : 'dom-waiting', misSend, missed: 0, escStop });
  assert.equal(misSend, 0, `誤送信 ${why} ${SIM_LABEL}`);
  assert.equal(escStop, 0, `${ESC_STOP_MARK} 停止 ${escStop} 件 ${why} ${SIM_LABEL}`);
}

// 処理中の変換中の Esc。進行の行だけが増えてよく、世代・停止・ほかの行は変わらない。完了まで待って停止していないことを示す。
async function checkBusy(page, cdp, s, draftKind, csp, hooks, draft) {
  const why = `${s.id} 処理中 下書き ${DRAFT_LABEL[draftKind]} CSP ${csp} ${hooks ? '試験の設定' : '配信設定'}`;
  await composeSentence(cdp, s);
  const e0 = hooks ? await engine(page) : null;
  if (hooks) assert.equal(e0.runStatus, 'busy', `${why} ${SIM_LABEL}`);
  const r0 = await snap(page);
  assert.equal(r0.run.state, 'busy', `処理中であること ${why} ${SIM_LABEL}`);
  const r = await composingKey(page, cdp, 'Escape');
  assertComposingKeydown(r.kd, why);
  const e1 = hooks ? await engine(page) : null;
  const strip = (x) => x.rows.filter((row) => row.kind !== 'progress');
  const stop = r.a.run.state === 'stopped' ? 1 : 0;
  const rowsChanged = JSON.stringify(strip(r.b)) === JSON.stringify(strip(r.a)) ? 0 : 1;
  const escStop = (hooks ? stopCount(e0, e1) : 0) + stop + rowsChanged;
  await imeCancel(cdp);
  assert.equal(escStop, 0, `${ESC_STOP_MARK} 停止 ${escStop} 件 ${why} ${SIM_LABEL}`);
  assert.equal(await page.inputValue(INPUT), draft, `${why} ${SIM_LABEL}`);
  // 自然な完了まで待つ。停止していれば進行の行は 4 件にならず、受付済みにもならない。
  await page.waitForFunction((log) => {
    const run = document.querySelector('[data-testid="run-status"]');
    return run?.getAttribute('data-state') === 'idle'
      && document.querySelectorAll(`${log} > li[data-kind="progress"]`).length === 4;
  }, LOG, { timeout: T });
  const done = await snap(page);
  assert.equal(done.run.state, 'idle', `${why} ${SIM_LABEL}`);
  assert.ok((done.step ?? '').includes('受付済み'), `処理が自然に完了して受付済み（確認待ち）になる ${why} ${SIM_LABEL}`);
  return escStop;
}

async function openHooks(srv, query = QUERY) {
  const opened = await openPage(browser, { origin: srv.origin });
  await installKeyProbe(opened.context);
  await opened.page.goto(`${srv.url}${query}`);
  await waitMounted(opened.page);
  assert.equal(await opened.page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
  await opened.page.focus(INPUT);
  await assertFocused(opened.page, INPUT);
  return { ...opened, cdp: await openIme(opened.page) };
}

// P-1・P-2: 試験の設定（本体の画面）で、(CSP × 下書き) ごとに 1 ページで 30 文を流す。
for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  for (const draftKind of DRAFTS) {
    test(`P-1・P-2 本体の画面 試験文 30 文 下書き ${DRAFT_LABEL[draftKind]} 変換中の Enter でログが増えず、確定後の Enter で受付が 1 件、変換中の Esc でログ・世代が変わらない（CSP ${cspLabel}）`, async () => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, cdp, rec } = await openHooks(srv);
        for (const s of SENTENCES) {
          await goTo(page, WAIT);
          if (s.kind === 'cancel') await runCancelIdle(page, cdp, s, draftKind, cspLabel, true);
          else await runEnter(page, cdp, s, draftKind, cspLabel, true);
        }
        assertClean(rec);
        await context.close();
      }));
    });
  }
}

// P-3: 処理中の変換中の Esc（1 組ごとに新しいページ）。
for (const draftKind of DRAFTS) {
  for (const s of CANCEL_SENTENCES) {
    test(`P-3 ${s.id} 処理中の変換中の Esc は世代・停止・ほかの行を変えず、進行は自然に完了する 下書き ${DRAFT_LABEL[draftKind]}`, async () => {
      await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
        const { page, context, cdp, rec } = await openHooks(srv);
        await goTo(page, BUSY);
        const draft = await setDraft(page, draftKind);
        await sendRaw(page, { input: { command: COMMAND } });
        await page.focus(INPUT);
        const escStop = await checkBusy(page, cdp, s, draftKind, 'test', true, draft);
        results.push({ id: s.id, kind: s.kind, draft: draftKind, csp: 'test', state: 'busy', misSend: 0, missed: 0, escStop });
        assertClean(rec);
        await context.close();
      });
    });
  }
}

// 配信する設定（試験の口なし。DOM だけ）。IR に開始点 s-1-3・s-1-5 を足して同梱する。
function deliveryIr() {
  const ir = loadBaseIr();
  for (const [id, node] of [['s-1-3', WAIT], ['s-1-5', BUSY]]) {
    assert.ok(ir.nodes.some((n) => n.uid === node), `node ${node}`);
    if (!ir.starts.some((x) => x.id === id)) ir.starts.push({ id, node, effects: [] });
  }
  return ir;
}

async function withDelivery(csp, fn) {
  return withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, (dir) => {
    fs.writeFileSync(path.join(dir, 'ir', 'main.json'), JSON.stringify(deliveryIr()), 'utf8');
    return withServer({ dir, base: BASE }, fn);
  }));
}

async function openDelivery(srv, start) {
  const opened = await openPage(browser, { origin: srv.origin });
  await installKeyProbe(opened.context);
  await opened.page.goto(`${srv.url}?start=${encodeURIComponent(start)}`);
  await waitMounted(opened.page);
  assert.equal(await opened.page.evaluate(() => typeof window.__fandheTest), 'undefined', 'no test hooks');
  await opened.page.waitForSelector('[data-testid="step-title"]', { timeout: T });
  await opened.page.focus(INPUT);
  await assertFocused(opened.page, INPUT);
  return { ...opened, cdp: await openIme(opened.page) };
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  test(`D-1 配信設定 試験文 30 文（文ごとに新しい context）の変換中の Enter・Esc と確定後の Enter が DOM の観察だけで期待どおり（CSP ${cspLabel}）`, async () => {
    await withDelivery(csp, async (srv) => {
      for (const s of SENTENCES) {
        const { page, context, cdp, rec } = await openDelivery(srv, 's-1-3');
        if (s.kind === 'cancel') await runCancelIdle(page, cdp, s, 'none', cspLabel, false);
        else await runEnter(page, cdp, s, 'none', cspLabel, false);
        assertClean(rec);
        await context.close();
      }
    });
  });

  test(`D-2 配信設定 取り消しの 3 文 処理中の変換中の Esc は状態表示とほかの行を変えず進行が完了する（CSP ${cspLabel}）`, async () => {
    await withDelivery(csp, async (srv) => {
      for (const s of CANCEL_SENTENCES) {
        const { page, context, cdp, rec } = await openDelivery(srv, 's-1-5');
        await page.fill(INPUT, COMMAND);
        await page.focus(INPUT);
        await page.keyboard.press('Enter');
        await page.waitForFunction((sel) => document.querySelector(sel)?.getAttribute('data-state') === 'busy',
          '[data-testid="run-status"]', { timeout: T });
        await page.focus(INPUT);
        const escStop = await checkBusy(page, cdp, s, 'none', cspLabel, false, '');
        results.push({ id: s.id, kind: s.kind, draft: 'none', csp: cspLabel, state: 'dom-busy', misSend: 0, missed: 0, escStop });
        assertClean(rec);
        await context.close();
      }
    });
  });
}

// 対照: 数え方が 0 より大きく数えられる（0 件が空振りでないこと）。
test('C-1 対照 合成をせずに確定した文字列へ通常の Enter を送ると、右のログの受付の行が 1 増え、誤送信の数え方が 1 を数える', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openHooks(srv);
    await goTo(page, WAIT);
    await page.fill(INPUT, '今日は天気がよいので散歩に行きます');
    const a = await snap(page);
    await page.keyboard.press('Enter');
    await waitField(page, '');
    await settle(page);
    const b = await snap(page);
    assert.equal(misSendCount(received(b).length - received(a).length), 1);
    assertClean(rec);
    await context.close();
  });
});

test('C-2 対照 変換していない Esc は処理中を停止し、停止の数え方が 1 を数え、状態表示が stopped になる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openHooks(srv);
    await goTo(page, BUSY);
    await sendRaw(page, { input: { command: COMMAND } });
    await page.focus(INPUT);
    const e0 = await engine(page);
    assert.equal(e0.runStatus, 'busy');
    await page.keyboard.press('Escape');
    await settle(page);
    assert.equal(stopCount(e0, await engine(page)), 1);
    assert.equal((await snap(page)).run.state, 'stopped');
    assertClean(rec);
    await context.close();
  });
});

test('C-3 対照 待機中で変換していない Esc は取り消しをログに残し、数え方が 1 を数える', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openHooks(srv);
    await goTo(page, WAIT);
    const e0 = await engine(page);
    const a = await snap(page);
    await page.keyboard.press('Escape');
    await settle(page);
    const b = await snap(page);
    assert.equal(cancelLogCount(e0.logLen, (await engine(page)).logLen), 1);
    assert.equal(cancelLogCount(a.rows.length, b.rows.length), 1);
    assertClean(rec);
    await context.close();
  });
});

// 全ての組が積まれ、3 つの件数が 0 であること。最後に置く（node:test はファイル内を上から順に流す）。
test('集計 本体の画面で試験文 30 文を流し、誤送信 0 件・確定後の送信漏れ 0 件・変換中の Esc での停止・取り消しの記録 0 件', (t) => {
  const expectedKeys = [];
  for (const s of SENTENCES) {
    for (const d of DRAFTS) {
      for (const csp of ['あり', 'なし']) {
        expectedKeys.push(`${s.id}|${d}|${csp}|${s.kind === 'cancel' ? 'waiting' : 'idle'}`);
      }
      if (s.kind === 'cancel') expectedKeys.push(`${s.id}|${d}|test|busy`);
    }
    for (const csp of ['あり', 'なし']) {
      expectedKeys.push(`${s.id}|none|${csp}|${s.kind === 'cancel' ? 'dom-waiting' : 'dom'}`);
      if (s.kind === 'cancel') expectedKeys.push(`${s.id}|none|${csp}|dom-busy`);
    }
  }
  const got = results.map((r) => `${r.id}|${r.draft}|${r.csp}|${r.state}`).sort();
  assert.deepEqual(got, expectedKeys.sort(), `行列の全ての組が積まれている ${SIM_LABEL}`);
  const sum = (k) => results.reduce((n, r) => n + r[k], 0);
  t.diagnostic(`組 ${results.length} 件 / 誤送信 ${sum('misSend')} 件 / 送信漏れ ${sum('missed')} 件 / 変換中の Esc での停止・取り消しの記録 ${sum('escStop')} 件 ${SIM_LABEL}`);
  assert.equal(sum('misSend'), 0, `誤送信 ${SIM_LABEL}`);
  assert.equal(sum('missed'), 0, `確定後の送信漏れ ${SIM_LABEL}`);
  assert.equal(sum('escStop'), 0, `変換中の Esc での停止 ${SIM_LABEL}`);
});
