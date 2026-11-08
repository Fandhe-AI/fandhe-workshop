// 変換中の入力の延期と確定値が 1 回だけ届く経路の層 3 ブラウザ試験（TASK-355 1/2。FEND-10）。
//
// 守る期待（FEND-10）:
//   状態に 0 件   変換中（compositionstart から compositionend まで）の文字列は、状態の下書きにも履歴の ops にも入らない
//   1 回だけ      確定（Input.insertText）ごとに、上流からの下書きの配送がちょうど 1 回届く。変換中の配送は 0 回
//   欄を書かない  変換中に、プログラムから <textarea> の value へ書いた回数が 0 回
// 配送の回数は試験の設定だけの口 draftSetCount（Rust の EngineHost が SetDraft を数える）で読む。ops は同じ値の重ねての配送を
// 記録しないため、「1 回だけ」は ops だけでは見分けられない。履歴の ops は exportPrepareLive から実際にダウンロードして読む
// （保存の遅延に依らず、固定の sleep が要らない）。
// 欄への書き込みは ime.mjs の書き込みの器（value の setter の包み）で数える。
//
// 証拠の種類はシミュレーター（CDP による IME の模擬）で、実 IME でも人の評価でもない（実 IME は TASK-93 / BUILD-18。未実施）。
// 空振りでないことは、負の対照 C-1〜C-3 と ime.test.mjs の書き込みの器の自己確認で示す。
//
// 変換中の再読み込み（R-6）・ログの追加中の消失 0 件・変異 M2 の検出は ime-reload-flood.test.mjs（TASK-355 2/2）。
//
// 待ち: 固定の sleep は使わない。予約済みの set_timeout(0) の後ろに積んで順序で待つ（settle）か、値の収束を waitForFunction で待つ。
// Chrome では確定の input が compositionend より前に出る（F-6）ため、状態の標本は imeCompose の各段の後と imeCommit の前に取る。
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
  installValueWriteProbe, readValueWriteProbe, clearValueWriteProbe,
} from './ime.mjs';
import { withDeliveryCopy } from './ch4-9-journey.mjs';
import { loadBaseIr, withStarts } from './e2-ir.mjs';
import { START } from './e4-history.mjs';

const BASE = '/fandhe-ime-defer/';
const INPUT = '[data-testid="draft-input"]';
const RAW = '[data-testid="raw-trigger"]';
const RECEIVED = '[data-testid="part-log-list"] li[data-kind="received"]';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
const PACK_ID = 'ime-defer';
const REPRESENTATIVE = ['S-01', 'S-11', 'S-16', 'S-21', 'S-29'];
const EXISTING = '既にある下書き';
const T = 30000;

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

function assertClean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify(rec);
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

// 予約済みの set_timeout(0)（書き戻し・配送）の後ろに自分のタイマーを積み、2 段通ってから返る。
const settle = (page) => page.evaluate(() => new Promise((resolve) => {
  setTimeout(() => setTimeout(resolve, 0), 0);
}));
const call = (page, name, ...args) => page.evaluate(([n, a]) => window.__fandheTest[n](...a), [name, args]);
const draftOf = (page) => call(page, 'draft');
const countOf = (page) => call(page, 'draftSetCount');
const engine = async (page) => JSON.parse(await call(page, 'stateJson')).engine;

// 受講中のホストの書き出しを実際にダウンロードして、履歴の ops を返す（読み取りのみ。状態は変わらない）。
async function liveOps(page) {
  const prepared = JSON.parse(await call(page, 'exportPrepareLive'));
  // 操作がまだ 1 件も無い間は書き出せない（gate が empty）。ops は空と読む。
  if (prepared.gate === 'empty') return [];
  assert.equal(prepared.ok, true, `export prepared ${SIM_LABEL}`);
  assert.match(prepared.url, /^blob:/);
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    call(page, 'exportClick', prepared.url, prepared.name),
  ]);
  const filePath = await download.path();
  assert.ok(filePath, 'download saved');
  const history = JSON.parse(fs.readFileSync(filePath, 'utf8'));
  assert.ok(Array.isArray(history.ops), 'ops is an array');
  return history.ops;
}
const draftOps = (ops) => ops.filter((o) => o && Object.hasOwn(o, 'draft')).map((o) => o.draft);

async function openTest(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await installValueWriteProbe(opened.context, INPUT);
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  await opened.page.focus(INPUT);
  await assertFocused(opened.page, INPUT);
  return { ...opened, cdp: await openIme(opened.page) };
}

// 変換する前の基準（状態の下書き・配送の回数・ops の draft の列）を取る。
async function baseline(page) {
  await settle(page);
  await clearValueWriteProbe(page);
  return { draft: await draftOf(page), count: await countOf(page), drafts: draftOps(await liveOps(page)) };
}

// 変換中の標本。状態の下書き・配送の回数が基準のまま、欄の値が基準の下書きと変換中の文字列をつないだもので、欄を書いていない。
async function assertDeferred(page, base, composition, why) {
  await settle(page);
  assert.equal(await draftOf(page), base.draft, `変換中は状態の下書きが変わらない: ${why} ${SIM_LABEL}`);
  assert.equal(await countOf(page), base.count, `変換中の配送は 0 回: ${why} ${SIM_LABEL}`);
  assert.equal(await page.inputValue(INPUT), base.draft + composition, `欄の値は下書き＋変換中の文字列: ${why} ${SIM_LABEL}`);
  assert.equal((await readValueWriteProbe(page)).composing, 0, `変換中に欄へ書かない: ${why} ${SIM_LABEL}`);
}

// 確定の後。状態の下書きが期待の値になり、配送がちょうど 1 回増える。2 回 settle して遅れて届く 2 回目も取りこぼさない。
async function assertCommitted(page, base, expectedDraft, why) {
  await page.waitForFunction((x) => window.__fandheTest.draft() === x, expectedDraft, { timeout: T });
  await settle(page);
  await settle(page);
  assert.equal((await countOf(page)) - base.count, 1, `確定値の配送はちょうど 1 回: ${why} ${SIM_LABEL}`);
  assert.equal(await page.inputValue(INPUT), expectedDraft, `${why} ${SIM_LABEL}`);
}

const sentences = loadSentences().filter((s) => REPRESENTATIVE.includes(s.id));
assert.equal(sentences.length, REPRESENTATIVE.length);

for (const initial of ['', EXISTING]) {
  const initLabel = initial === '' ? '下書きなし' : '既にある下書きあり';
  for (const sentence of sentences) {
    test(`D-1・D-2 ${sentence.id}（${sentence.kind}）変換中は状態・履歴・欄が動かず、確定で配送ちょうど 1 回（${initLabel}）`, async () => {
      await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
        const { page, context, cdp, rec } = await openTest(srv);
        if (initial !== '') {
          await page.fill(INPUT, initial);
          await page.focus(INPUT);
        }
        const rows = sentence.lines
          ? sentence.lines.map(([reading, text]) => ({ comps: [reading, text], text }))
          : [{ comps: sentence.comps, text: sentence.text }];
        let expected = initial;
        for (let i = 0; i < rows.length; i += 1) {
          const { comps, text } = rows[i];
          if (i > 0) {
            await page.keyboard.press('Shift+Enter');
            expected += '\n';
          }
          const base = await baseline(page);
          assert.equal(base.draft, expected, `基準の下書き ${sentence.id} 行 ${i + 1} ${SIM_LABEL}`);
          for (const comp of comps) {
            await imeCompose(cdp, comp);
            await assertDeferred(page, base, comp, `${sentence.id} 行 ${i + 1} 「${comp}」`);
          }
          // 変換中の ops は、変換の前と同じ draft の列のまま（変換中の文字列が履歴に入らない）。
          assert.deepEqual(draftOps(await liveOps(page)), base.drafts, `変換中の ops の draft の列が変わらない: ${sentence.id} 行 ${i + 1} ${SIM_LABEL}`);
          await imeCommit(cdp, text);
          expected += text;
          await assertCommitted(page, base, expected, `${sentence.id} 行 ${i + 1}`);
          // ops の draft は書き出しや保存の時点の下書きを記録する（途中の値は畳まれる）ため、列の長さでなく中身で判定する。
          // 確定の後の列の末尾は確定値で、入っているのは変換の前から有った値・変換の前の下書き・確定値だけである
          // （変換中の文字列は、確定値と同じ文字列になる場合を除いて入らない。部分文字列の判定は使わない）。
          const after = draftOps(await liveOps(page));
          assert.equal(after[after.length - 1], expected, `確定後の ops の draft の末尾: ${sentence.id} 行 ${i + 1} ${SIM_LABEL}`);
          const allowed = new Set([...base.drafts, base.draft, expected]);
          assert.deepEqual(after.filter((d) => !allowed.has(d)), [], `ops の draft に想定外の値: ${sentence.id} 行 ${i + 1} ${SIM_LABEL}`);
        }
        assertClean(rec);
        await context.close();
      });
    });
  }
}

test('D-3 処理中（ログが増える間）に変換しても、状態・配送・欄・履歴は変換中に動かず、確定で配送ちょうど 1 回', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, cdp, rec } = await openTest(srv);
    const base = await baseline(page);
    // 処理中にする（進行は実タイマーの 300ms ごと）。入力欄の送信は使わず、見えない raw-trigger で届ける（ime-input の I-5 と同じ）。
    await page.evaluate(([sel, json]) => {
      const el = document.querySelector(sel);
      el.setAttribute('data-payload', json);
      el.click();
    }, [RAW, JSON.stringify({ input: { command: '/make-note first-note' } })]);
    await page.focus(INPUT);
    await imeCompose(cdp, 'へんかん');
    await imeCompose(cdp, '変換');
    const logLen0 = (await engine(page)).logLen;
    let grew = false;
    const deadline = Date.now() + 10000;
    for (let i = 0; ; i += 1) {
      await assertDeferred(page, base, '変換', `処理中の標本 ${i}`);
      if ((await engine(page)).logLen > logLen0) grew = true;
      if ((await call(page, 'nextTick')) === 'null' && grew) break;
      assert.ok(Date.now() < deadline, 'timeout waiting for ticks');
      await page.waitForTimeout(50);
    }
    assert.equal(grew, true, `ログが増える間に確かめられた ${SIM_LABEL}`);
    assert.deepEqual(draftOps(await liveOps(page)), base.drafts, `処理中の ops の draft の列が変わらない ${SIM_LABEL}`);
    await imeCommit(cdp, '変換');
    await assertCommitted(page, base, base.draft + '変換', 'D-3');
    assertClean(rec);
    await context.close();
  });
});

test('D-4 取り消しでは、状態の下書きと履歴の draft の列が変換の前のまま、欄を書かない（配送の回数は実測を記録する）', async (t) => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, cdp, rec } = await openTest(srv);
    await page.fill(INPUT, EXISTING);
    await page.focus(INPUT);
    const base = await baseline(page);
    await imeCompose(cdp, 'ほげ');
    await assertDeferred(page, base, 'ほげ', 'D-4');
    await imeCancel(cdp);
    await settle(page);
    await settle(page);
    assert.equal(await draftOf(page), base.draft, `取り消しで状態の下書きが変わらない ${SIM_LABEL}`);
    assert.equal(await page.inputValue(INPUT), base.draft, `取り消しで欄が変換の前に戻る ${SIM_LABEL}`);
    assert.deepEqual(draftOps(await liveOps(page)), base.drafts, `取り消しで ops の draft の列が変わらない ${SIM_LABEL}`);
    assert.equal((await readValueWriteProbe(page)).composing, 0, `取り消しの変換中に欄へ書かない ${SIM_LABEL}`);
    const delivered = (await countOf(page)) - base.count;
    // 上流が取り消しの compositionend でも配送するかは固定しない。上限だけを守り、実測を残す。
    assert.ok(delivered <= 1, `取り消しの配送は高々 1 回: 実測 ${delivered} ${SIM_LABEL}`);
    t.diagnostic(`D-4 取り消しの配送の実測: ${delivered} 回`);
    assertClean(rec);
    await context.close();
  });
});

// 負の対照。数え方と読み方が空振りでないことを示す。
test('C-1 変換せずに打った文字は、配送の計数と ops の draft の列に現れる', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTest(srv);
    const base = await baseline(page);
    await page.keyboard.type('abc');
    await page.waitForFunction(() => window.__fandheTest.draft() === 'abc', undefined, { timeout: T });
    await settle(page);
    assert.ok((await countOf(page)) - base.count >= 3, `3 打鍵で配送が 3 回以上増える ${SIM_LABEL}`);
    assert.notDeepEqual(draftOps(await liveOps(page)), base.drafts, `ops の draft の列が増える ${SIM_LABEL}`);
    assertClean(rec);
    await context.close();
  });
});

test('C-2 変換中に試験から欄へ書くと、書き込みの器が composing に数える', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, cdp, rec } = await openTest(srv);
    await clearValueWriteProbe(page);
    await imeCompose(cdp, 'へんかん');
    await page.evaluate((sel) => { document.querySelector(sel).value = 'x'; }, INPUT);
    assert.equal((await readValueWriteProbe(page)).composing, 1, `${SIM_LABEL}`);
    await imeCancel(cdp);
    assertClean(rec);
    await context.close();
  });
});

test('C-3 変換せずに入れた読みの文字列は、ops の draft の列を期待と違うものにする', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openTest(srv);
    const base = await baseline(page);
    await page.keyboard.insertText('へんかん');
    await page.waitForFunction(() => window.__fandheTest.draft() === 'へんかん', undefined, { timeout: T });
    await settle(page);
    const drafts = draftOps(await liveOps(page));
    assert.ok(drafts.includes('へんかん'), `読みの文字列が列に現れる ${SIM_LABEL}`);
    assert.notDeepEqual(drafts, base.drafts, `D-2 と同じ比べ方で期待の列と違うと判定される ${SIM_LABEL}`);
    assertClean(rec);
    await context.close();
  });
});

// 配信する設定。試験の口なしで、画面と保存先だけで見る。
function packIr() {
  const ir = withStarts(loadBaseIr());
  ir.pack = { ...ir.pack, id: PACK_ID };
  return ir;
}

const historyEntries = (page) => page.evaluate(([prefix]) => {
  const out = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    if (!k.startsWith(prefix)) continue;
    try {
      const h = JSON.parse(localStorage.getItem(k) ?? '');
      if (h.format === 'fandhe-workshop-history') out.push(h);
    } catch { /* 履歴でない値は数えない */ }
  }
  return out;
}, [HISTORY_PREFIX]);

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  for (const sentence of sentences.filter((s) => !s.lines)) {
    test(`D-5 ${sentence.id}（${sentence.kind}）配信設定で、変換中は欄を書かず、確定後の Enter で受付が 1 件・履歴の draft と submit が確定値（CSP ${cspLabel}）`, async () => {
      await withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, async (dir) => {
        fs.writeFileSync(path.join(dir, 'ir', 'main.json'), JSON.stringify(packIr()), 'utf8');
        await withServer({ dir, base: BASE }, async (srv) => {
          const { page, context, rec } = await openPage(browser, { origin: srv.origin });
          await installValueWriteProbe(context, INPUT);
          await page.goto(`${srv.url}?start=${encodeURIComponent(START)}`);
          await waitMounted(page);
          assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'no test hooks');
          await page.waitForSelector('[data-testid="step-title"]', { timeout: T });
          await page.focus(INPUT);
          const cdp = await openIme(page);
          await clearValueWriteProbe(page);
          for (const comp of sentence.comps) {
            await imeCompose(cdp, comp);
            await settle(page);
            assert.equal(await page.inputValue(INPUT), comp, `変換中の欄の値 ${SIM_LABEL}`);
            assert.equal((await readValueWriteProbe(page)).composing, 0, `変換中に欄へ書かない ${SIM_LABEL}`);
          }
          await imeCommit(cdp, sentence.text);
          await page.waitForFunction(([sel, v]) => document.querySelector(sel)?.value === v, [INPUT, sentence.text], { timeout: T });
          await settle(page);
          await page.keyboard.press('Enter');
          await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, RECEIVED, { timeout: T });
          // この Step は自由な入力を拒否する。拒否では下書きが残る（TERM-17）ため、欄は確定値のまま。
          assert.equal(await page.inputValue(INPUT), sentence.text, `${SIM_LABEL}`);
          // 自動保存が着地するまで待つ（上限 15 秒）。値の比較だけを行う。
          let ops = [];
          for (let i = 0; i < 60; i += 1) {
            const entries = await historyEntries(page);
            ops = entries.length === 1 ? entries[0].ops : [];
            if (ops.some((o) => o && o.submit === sentence.text)) break;
            await page.waitForTimeout(250);
          }
          assert.deepEqual(ops.filter((o) => o && Object.hasOwn(o, 'submit')).map((o) => o.submit), [sentence.text], `submit の列 ${SIM_LABEL}`);
          // draft の列に入るのは確定値だけで、変換中の文字列（読み）は入らない。
          const drafts = draftOps(ops);
          assert.ok(drafts.length >= 1, `draft の列に確定値が入る ${SIM_LABEL}`);
          assert.ok(drafts.every((d) => d === sentence.text), `draft の列は確定値だけ: ${sentence.id} ${SIM_LABEL}`);
          assertClean(rec);
          await context.close();
        });
      }));
    });
  }
}
