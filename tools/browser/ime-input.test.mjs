// 入力欄と IME の層 3 ブラウザ試験（TASK-86 1/2。BUILD-10。誤送信 0 件・送信漏れ 0 件）。
//
// CDP による IME の模擬（ime.mjs）で、変換中の Enter・Tab・差分の再描画・確定後の Enter を流し、次を確かめる。
//   誤送信 0 件    変換中（isComposing=true / keyCode 229）の Enter では送信されず、既定の動作も止めない
//   送信漏れ 0 件  確定した後の通常の Enter では、確定した文字列がちょうど 1 件送信される
//   下書きと焦点   変換中の差分の再描画で、欄の値・要素・フォーカスが保たれる。送信後も焦点は欄に残る
// 証拠の種類はシミュレーター（CDP の模擬）で、実 IME でも人の評価でもない（実 IME は TASK-93 / BUILD-18）。
// 合成が起きていなかったための空振りの 0 件でないことは、ime.test.mjs とここの対照（I-8）で示す。
//
// 範囲: 試験文は kind ごとに 1 文の代表 5 文（S-01・S-11・S-16・S-21・S-29）。30 文の全行列と変換中の Esc
// （cancel の 3 文・K-12・K-13）は TASK-276（#945・#946）の担当。Enter・Shift+Enter・Tab・貼り付けの非 IME の場面は
// enter-submit・tab-complete・paste の各試験が確かめ済みのため、ここでは IME と組み合わさる場面だけを持つ。
// 変換中の文字列が確定前に状態へ入らないこと（PoC-2 F-3）は、現行の挙動を固定する根拠が無いため assert しない。
//
// 待ち: 確定の後の欄の書き戻しは compositionend → set_timeout(0) → sync_draft の遅延を通る。固定の sleep は使わず、
// 予約済みのタイマーの後ろに自分のタイマーを積んで（settle）順序で待つか、値の収束を waitForFunction で待つ。
// 右のログの一覧（part-log-list）は TASK-517 で整う予定のため、ここでは兄弟の試験と同じ selector を使う。
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
  imeEnterKey, imeTabKey, composeSentence, installKeyProbe, readKeyProbe, clearKeyProbe,
} from './ime.mjs';

const BASE = '/fandhe-ime-input/';
const INPUT = '[data-testid="draft-input"]';
const ITEMS = '[data-testid="log-list"] > li';
const RAW = '[data-testid="raw-trigger"]';
const RECEIVED = '[data-testid="part-log-list"] li[data-kind="received"]';
const REPRESENTATIVE = ['S-01', 'S-11', 'S-16', 'S-21', 'S-29'];

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

// 予約済みの set_timeout(0)（書き戻し・配送）の後ろに自分のタイマーを積み、2 段通ってから返る。
const settle = (page) => page.evaluate(() => new Promise((resolve) => {
  setTimeout(() => setTimeout(resolve, 0), 0);
}));
const itemTexts = (page) => page.$$eval(ITEMS, (lis) => lis.map((li) => li.textContent));
const keydowns = async (page) => (await readKeyProbe(page)).filter((r) => r.type === 'keydown');
const draftOf = (page) => page.evaluate(() => window.__fandheTest.draft());
const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson()));
const activeId = (page) => page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? null);
const waitValue = (page, v) => page.waitForFunction(([sel, x]) => document.querySelector(sel)?.value === x, [INPUT, v]);

// 変換中の Enter を送り、送信されず、既定の動作も止められなかったことを確かめる。
async function assertComposingEnterIgnored(page, cdp, expectedCount, why) {
  await clearKeyProbe(page);
  await imeEnterKey(cdp);
  await settle(page);
  const kd = await keydowns(page);
  assert.equal(kd.length, 1, `${why} ${SIM_LABEL}`);
  assert.equal(kd[0].isComposing, true, `${why} ${SIM_LABEL}`);
  assert.equal(kd[0].keyCode, 229, `${why} ${SIM_LABEL}`);
  assert.equal(kd[0].defaultPrevented, false, `${why} ${SIM_LABEL}`);
  assert.equal((await itemTexts(page)).length, expectedCount, `誤送信 ${why} ${SIM_LABEL}`);
}

test('I-9 配信設定の画面に試験の口（__fandheTest）が無い', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await open(srv);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined');
    await context.close();
  });
});

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));
  const sentences = loadSentences().filter((s) => REPRESENTATIVE.includes(s.id));
  assert.equal(sentences.length, REPRESENTATIVE.length);

  for (const sentence of sentences) {
    test(`I-1 ${sentence.id}（${sentence.kind}）変換中の Enter で誤送信 0 件、確定後の Enter で送信漏れ 0 件（CSP ${cspLabel}）`, async () => {
      await run(async (srv) => {
        const { page, context, cdp, rec } = await open(srv);
        if (sentence.lines) {
          // 行ごとに変換して確定し、行の間は Shift+Enter（送信しない）。
          for (let i = 0; i < sentence.lines.length; i += 1) {
            const [reading, text] = sentence.lines[i];
            await imeCompose(cdp, reading);
            await imeCompose(cdp, text);
            await assertComposingEnterIgnored(page, cdp, 0, `${sentence.id} 行 ${i + 1}`);
            await imeCommit(cdp, text);
            await settle(page);
            if (i < sentence.lines.length - 1) await page.keyboard.press('Shift+Enter');
          }
        } else {
          await composeSentence(cdp, sentence);
          await assertComposingEnterIgnored(page, cdp, 0, sentence.id);
          await imeCommit(cdp, sentence.text);
          await settle(page);
        }
        assert.equal(await page.inputValue(INPUT), sentence.text, `${sentence.id} ${SIM_LABEL}`);
        await clearKeyProbe(page);
        await page.keyboard.press('Enter');
        await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, ITEMS);
        const kd = await keydowns(page);
        assert.equal(kd[0].isComposing, false, SIM_LABEL);
        assert.equal(kd[0].keyCode, 13, SIM_LABEL);
        assert.deepEqual(await itemTexts(page), [sentence.text], `送信漏れ ${sentence.id} ${SIM_LABEL}`);
        await waitValue(page, '');
        assertClean(rec);
        await context.close();
      });
    });
  }

  test(`I-2 下書きの後ろへ変換しても、変換中の Enter は送信せず、確定後の Enter で 1 件になる（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, cdp, rec } = await open(srv);
      await page.fill(INPUT, '既にある下書き');
      await page.focus(INPUT);
      await settle(page);
      await imeCompose(cdp, 'へんかん');
      await imeCompose(cdp, '変換');
      await assertComposingEnterIgnored(page, cdp, 0, 'I-2');
      await imeCommit(cdp, '変換');
      await settle(page);
      await page.keyboard.press('Enter');
      await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, ITEMS);
      assert.deepEqual(await itemTexts(page), ['既にある下書き変換'], SIM_LABEL);
      assertClean(rec);
      await context.close();
    });
  });

  test(`I-3 確定の後に届く keyCode 229 の Enter（Safari の順）では送信せず、次の通常の Enter で 1 件になる（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, cdp, rec } = await open(srv);
      await imeCompose(cdp, 'かんじ');
      await imeCompose(cdp, '漢字');
      await imeCommit(cdp, '漢字');
      await settle(page);
      await clearKeyProbe(page);
      await imeEnterKey(cdp);
      await settle(page);
      const kd = await keydowns(page);
      assert.equal(kd.length, 1, SIM_LABEL);
      assert.equal(kd[0].keyCode, 229, SIM_LABEL);
      assert.equal(kd[0].isComposing, false, `確定の後の 229 の Enter は isComposing=false で届く。${SIM_LABEL}`);
      assert.deepEqual(await itemTexts(page), [], `誤送信 ${SIM_LABEL}`);
      await page.keyboard.press('Enter');
      await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, ITEMS);
      assert.deepEqual(await itemTexts(page), ['漢字'], SIM_LABEL);
      assertClean(rec);
      await context.close();
    });
  });

  test(`I-6 送信の後の再描画でも、フォーカスは入力欄に残り、欄は空になる（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, 'hello');
      await page.keyboard.press('Enter');
      await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, ITEMS);
      await waitValue(page, '');
      assert.equal(await activeId(page), 'draft-input');
      assertClean(rec);
      await context.close();
    });
  });

  test(`I-8 対照 合成をせずに確定した文字列へ通常の Enter を送ると 1 件を数える（数え方が 0 より大きく数えられる。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, '今日は天気がよいので散歩に行きます');
      await page.keyboard.press('Enter');
      await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, ITEMS);
      assert.equal((await itemTexts(page)).length, 1);
      assertClean(rec);
      await context.close();
    });
  });
}

test('I-4 変換中の Tab（keyCode 229）は補完にならず、既定の動作も止めない（K-07）', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, cdp, rec } = await open(srv);
    await page.fill(INPUT, '/he');
    await page.focus(INPUT);
    await settle(page);
    const before = (await engine(page)).completeCount;
    await imeCompose(cdp, 'へ');
    await clearKeyProbe(page);
    await imeTabKey(cdp);
    await settle(page);
    const kd = await keydowns(page);
    assert.equal(kd.length, 1, SIM_LABEL);
    assert.equal(kd[0].isComposing, true, SIM_LABEL);
    assert.equal(kd[0].defaultPrevented, false, SIM_LABEL);
    assert.equal((await engine(page)).completeCount, before, SIM_LABEL);
    assert.ok(!(await page.inputValue(INPUT)).includes('/help'), SIM_LABEL);
    await imeCancel(cdp);
    assertClean(rec);
    await context.close();
  });
});

test('I-7 丸ごとの作り直しの後、欄の値は下書きと一致する（フォーカスは測定用の口のため assert しない）', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await open(srv);
    await page.fill(INPUT, 'abc下書き');
    await settle(page);
    assert.equal(await page.evaluate(() => window.__fandheTest.rerender()), true);
    await settle(page);
    assert.equal(await page.inputValue(INPUT), await draftOf(page));
    assertClean(rec);
    await context.close();
  });
});

for (const draft of ['', '先にある下書き']) {
  test(`I-5 処理中の差分の再描画でも、変換中の文字・欄の要素・フォーカスが保たれる（下書き ${draft === '' ? 'なし' : 'あり'}）`, async () => {
    await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
      const { page, context, cdp, rec } = await open(srv);
      if (draft !== '') {
        await page.fill(INPUT, draft);
        await page.focus(INPUT);
        await settle(page);
      }
      const receivedBefore = (await page.$$(RECEIVED)).length;
      // 処理中にする（進行は実タイマーの 300ms ごと）。入力欄の送信は使わず、見えない raw-trigger で届ける。
      await page.evaluate(([sel, json]) => {
        const el = document.querySelector(sel);
        el.setAttribute('data-payload', json);
        el.click();
      }, [RAW, JSON.stringify({ input: { command: '/make-note first-note' } })]);
      await page.focus(INPUT);
      await imeCompose(cdp, 'へんかん');
      await page.evaluate((sel) => { window.__imeEl = document.querySelector(sel); }, INPUT);
      const logLen0 = (await engine(page)).engine.logLen;
      const expected = `${draft}へんかん`;
      let grew = false;
      const deadline = Date.now() + 10000;
      for (;;) {
        const probe = await page.evaluate((sel) => ({
          same: window.__imeEl === document.querySelector(sel),
          active: document.activeElement === document.querySelector(sel),
          value: document.querySelector(sel).value,
          next: window.__fandheTest.nextTick(),
        }), INPUT);
        assert.equal(probe.same, true, `欄の要素が作り直された ${SIM_LABEL}`);
        assert.equal(probe.active, true, `フォーカスが外れた ${SIM_LABEL}`);
        assert.equal(probe.value, expected, `変換中の文字が消えた ${SIM_LABEL}`);
        if ((await engine(page)).engine.logLen > logLen0) grew = true;
        if (probe.next === 'null' && grew) break;
        assert.ok(Date.now() < deadline, 'timeout waiting for ticks');
        await page.waitForTimeout(50);
      }
      assert.equal(grew, true, 'ログが増える間に確かめられた');
      await imeCommit(cdp, '変換');
      await waitValue(page, `${draft}変換`);
      await page.waitForFunction((v) => window.__fandheTest.draft() === v, `${draft}変換`);
      await page.keyboard.press('Enter');
      await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [RECEIVED, receivedBefore + 2]);
      assertClean(rec);
      await context.close();
    });
  });
}
