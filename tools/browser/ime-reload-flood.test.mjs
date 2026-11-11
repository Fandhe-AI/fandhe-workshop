// 変換中の再読み込み・ログの追加中の消失 0 件・変異 M2 の検出の層 3 ブラウザ試験（TASK-355 2/2。FEND-10）。
//
// 守る期待（FEND-10）:
//   消失 0 件     ログを 100 件以上足している間に、変換中の文字と下書きが消えた件数が 0 件（PoC-2 の F-1・F-2・F-1b・F-2b に当たる）
//   R-6           変換中に再読み込みすると、確定済みの文字列は戻り、変換中の文字列は戻らず、保存先にも入らない
//   M2 の検出     反映の変換中の除外（crates/web/src/glue.rs の sync_draft の先頭）を外すと、この試験（S-M2b）が失敗する
//   補完（S-M2）  変換中に届いた補完は適用されず、確定した文字が欄に残る（#2121）
//   B-1・B-2      compositionend が来ないまま欄が焦点を失っても、印と保留が focusout で解消される（#2121）
//
// PoC-2 と構成が違う点: PoC-2 は update のたびに欄へ反映したので、ログの追加そのものが M2 の引き金だった。この repo では
// 書き戻し（sync_draft）はホストの書き戻しの要求・変換中でない input・compositionend の段でしか起きず、ログを足すだけでは
// 起きない。そのため M2 の検出点は S-M2b（変換中に送信の書き戻しを要求させる場面。変換中の補完は #2121 で適用しなくなった）に置く。ログの追加の場面は、
// 再描画が欄の値・要素・フォーカスを壊さないことの確認である。
// 証拠の種類はシミュレーター（CDP による IME の模擬）で、実 IME でも人の評価でもない（実 IME は TASK-93 / BUILD-18。未実施）。
// 対照: scripts/test-ime-sync-control.sh が M2 を入れた写しでこの試験を流し、固定の印 IME-COMPOSING-LOST つきで失敗することを確かめる。
//
// 待ち: 固定の sleep は使わない（settle と waitForFunction）。入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）だけ。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import {
  SIM_LABEL, openIme, assertFocused, imeCompose, imeCommit, imeCancel,
  installValueWriteProbe, readValueWriteProbe, clearValueWriteProbe,
} from './ime.mjs';

const BASE = '/fandhe-ime-reload-flood/';
const INPUT = '[data-testid="draft-input"]';
const RAW = '[data-testid="raw-trigger"]';
const MARK = 'IME-COMPOSING-LOST';
const ACCEPTED = '/make-note first-note';
const EXISTING = '既にある下書き';
const READING = 'へんかん';
const FLOOD_ROUNDS = 50;
const T = 30000;

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
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

const settle = (page) => page.evaluate(() => new Promise((resolve) => {
  setTimeout(() => setTimeout(resolve, 0), 0);
}));
const call = (page, name, ...args) => page.evaluate(([n, a]) => window.__fandheTest[n](...a), [name, args]);
const draftOf = (page) => call(page, 'draft');
const countOf = (page) => call(page, 'draftSetCount');
const state = async (page) => JSON.parse(await call(page, 'stateJson'));
const engine = async (page) => (await state(page)).engine;

// 受講中のホストの書き出しを実際にダウンロードして ops の draft の列を返す（読み取りのみ）。
async function draftOps(page) {
  const prepared = JSON.parse(await call(page, 'exportPrepareLive'));
  if (prepared.gate === 'empty') return [];
  assert.equal(prepared.ok, true, `export prepared ${SIM_LABEL}`);
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    call(page, 'exportClick', prepared.url, prepared.name),
  ]);
  const history = JSON.parse(fs.readFileSync(await download.path(), 'utf8'));
  assert.ok(Array.isArray(history.ops), 'ops is an array');
  return history.ops.filter((o) => o && Object.hasOwn(o, 'draft')).map((o) => o.draft);
}

// localStorage の全値を連結した文字列（変換中の文字列が保存先に入っていないことの確認用）。
const storageDump = (page) => page.evaluate(() => {
  const out = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    out.push(`${k}=${localStorage.getItem(k)}`);
  }
  return out.join('\n');
});

async function openTest(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await installValueWriteProbe(opened.context, INPUT);
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  await opened.page.focus(INPUT);
  await assertFocused(opened.page, INPUT);
  return { ...opened, cdp: await openIme(opened.page) };
}

async function baseline(page) {
  await settle(page);
  await clearValueWriteProbe(page);
  return { draft: await draftOf(page), count: await countOf(page), drafts: await draftOps(page) };
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  for (const initial of ['', EXISTING]) {
    const initLabel = initial === '' ? '下書きなし' : '既にある下書きあり';
    test(`F-1・F-2 ログを 100 件以上足している間、変換中の文字と下書きが消えない（${initLabel}。CSP ${cspLabel}）`, async (t) => {
      await run(async (srv) => {
        const { page, context, cdp, rec } = await openTest(srv);
        if (initial !== '') {
          await page.fill(INPUT, initial);
          await page.focus(INPUT);
        }
        const base = await baseline(page);
        assert.equal(base.draft, initial, `基準の下書き ${SIM_LABEL}`);
        const logLen0 = (await engine(page)).logLen;
        const handle = await page.$(INPUT);
        await imeCompose(cdp, READING);
        await imeCompose(cdp, '変換');
        const expectedValue = base.draft + '変換';
        let checks = 0;
        let lost = 0;
        for (let i = 0; i < FLOOD_ROUNDS; i += 1) {
          // 1 回で 2 行増える。JS の click で送るため、フォーカスは動かない。
          await page.evaluate(([sel, json]) => {
            const el = document.querySelector(sel);
            el.setAttribute('data-payload', json);
            el.click();
          }, [RAW, JSON.stringify({ input: { command: '/make-note Bad_Name' } })]);
          await settle(page);
          checks += 1;
          const same = await page.evaluate(([sel, h]) => document.querySelector(sel) === h, [INPUT, handle]);
          const focused = await page.evaluate((sel) => document.activeElement === document.querySelector(sel), INPUT);
          const ok = (await page.inputValue(INPUT)) === expectedValue
            && (await draftOf(page)) === base.draft
            && (await countOf(page)) === base.count
            && (await readValueWriteProbe(page)).composing === 0
            && same
            && focused;
          if (!ok) lost += 1;
        }
        const grew = (await engine(page)).logLen - logLen0;
        t.diagnostic(`lost=${lost} log=${grew} checks=${checks}`);
        assert.equal(lost, 0, `${MARK}: ログの追加中に変換中の文字・下書き・フォーカスが消えた回数 ${SIM_LABEL}`);
        assert.ok(checks >= FLOOD_ROUNDS, `確かめた回数 ${SIM_LABEL}`);
        assert.ok(grew >= 100, `ログが 100 件以上増えた: ${grew} ${SIM_LABEL}`);
        assert.deepEqual(await draftOps(page), base.drafts, `ログの追加中に ops の draft の列が変わらない ${SIM_LABEL}`);
        // F-1b・F-2b: 確定すると、下書きは基準＋確定値になり、配送はちょうど 1 回増える。
        await imeCommit(cdp, '変換');
        await page.waitForFunction((x) => window.__fandheTest.draft() === x, expectedValue, { timeout: T });
        await settle(page);
        await settle(page);
        assert.equal((await countOf(page)) - base.count, 1, `確定値の配送はちょうど 1 回 ${SIM_LABEL}`);
        assert.equal(await page.inputValue(INPUT), expectedValue, `確定後の欄の値 ${SIM_LABEL}`);
        assertClean(rec);
        await context.close();
      });
    });
  }

  test(`S-M2 変換中に補完が届いても適用されず、確定した文字が欄に残る（CSP ${cspLabel}）`, async (t) => {
    await run(async (srv) => {
      const { page, context, cdp, rec } = await openTest(srv);
      await page.fill(INPUT, '/he');
      await page.focus(INPUT);
      await settle(page);
      await clearValueWriteProbe(page);
      const base = await countOf(page);
      const before = (await state(page)).completeCount;
      await imeCompose(cdp, READING);
      await call(page, 'requestComplete');
      await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).completeCount === n + 1, before, { timeout: T });
      await settle(page);
      await settle(page);
      // 変換中の補完は適用しない（#2121）。状態の下書きは変わらず、欄は変換中の値のまま、書き込みは 0 回。
      assert.equal(await draftOf(page), '/he', `変換中の補完は適用されない ${SIM_LABEL}`);
      assert.equal(await page.inputValue(INPUT), '/he' + READING, `${MARK}: 変換中に補完が届いても欄の値が保たれる ${SIM_LABEL}`);
      assert.equal((await readValueWriteProbe(page)).composing, 0, `${MARK}: 変換中に欄へ書かない ${SIM_LABEL}`);
      await imeCommit(cdp, '変換');
      await page.waitForFunction((x) => window.__fandheTest.draft() === x, '/he変換', { timeout: T });
      await settle(page);
      await settle(page);
      assert.equal(await page.inputValue(INPUT), '/he変換', `${MARK}: 確定した文字が欄に残る ${SIM_LABEL}`);
      assert.equal(await draftOf(page), '/he変換', `確定後の状態の下書き ${SIM_LABEL}`);
      assert.equal((await countOf(page)) - base, 1, `確定値の配送はちょうど 1 回 ${SIM_LABEL}`);
      assertClean(rec);
      await context.close();
    });
  });

  test(`S-M2b 変換中に送信の書き戻しが頼まれても、欄を書かず変換中の文字が消えない（M2 の検出点。CSP ${cspLabel}）`, async (t) => {
    await run(async (srv) => {
      const { page, context, cdp, rec } = await openTest(srv);
      await page.fill(INPUT, ACCEPTED);
      await page.focus(INPUT);
      await settle(page);
      await clearValueWriteProbe(page);
      const logLen0 = (await engine(page)).logLen;
      await imeCompose(cdp, READING);
      // JS の click で送る（フォーカスは動かない。#1510 の経路で、変換中に書き戻しが頼まれる）。
      await page.evaluate((sel) => document.querySelector(sel).click(), '[data-testid="submit-input"]');
      await settle(page);
      await settle(page);
      // 空振りでないこと: 送信で受付が増え、状態の下書きは空になった。
      assert.ok((await engine(page)).logLen > logLen0, `送信で受付が増えた（前提） ${SIM_LABEL}`);
      assert.equal(await draftOf(page), '', `送信で状態の下書きが空になった（前提） ${SIM_LABEL}`);
      assert.equal(await page.inputValue(INPUT), ACCEPTED + READING, `${MARK}: 変換中に書き戻しが頼まれても欄の値が保たれる ${SIM_LABEL}`);
      assert.equal((await readValueWriteProbe(page)).composing, 0, `${MARK}: 変換中に欄へ書かない ${SIM_LABEL}`);
      // 変換の終わりの後の値は、保留した書き戻しの優先（#1510）に依るため assert せず、実測を残す。
      await imeCancel(cdp);
      await settle(page);
      await settle(page);
      t.diagnostic(`S-M2b 取り消しの後の欄の値: ${JSON.stringify(await page.inputValue(INPUT))}`);
      assertClean(rec);
      await context.close();
    });
  });

  // 変換中の印が compositionend の来ないまま残る場面（#2121）。合成の compositionstart で印だけを立てる。
  const startMark = (page) => page.evaluate((sel) => {
    document.querySelector(sel).dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
  }, INPUT);

  test(`B-1n（負の対照）印が残ったまま焦点を動かさず打つと、Enter は送信されない（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await openTest(srv);
      await settle(page);
      const logLen0 = (await engine(page)).logLen;
      await startMark(page);
      await page.keyboard.type(ACCEPTED);
      await page.keyboard.press('Enter');
      await settle(page);
      assert.equal((await engine(page)).logLen, logLen0, `印が立っている間は送信されない（印の残る状態を作れている） ${SIM_LABEL}`);
      await context.close();
      void rec;
    });
  });

  test(`B-1 compositionend が来ないまま欄が焦点を失っても、印が残らず続けて打った文字が送信される（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await openTest(srv);
      await settle(page);
      const logLen0 = (await engine(page)).logLen;
      await startMark(page);
      await page.evaluate((sel) => document.querySelector(sel).blur(), INPUT);
      await page.focus(INPUT);
      await page.keyboard.type(ACCEPTED);
      await settle(page);
      assert.equal(await page.inputValue(INPUT), ACCEPTED, `欄の値 ${SIM_LABEL}`);
      assert.equal(await draftOf(page), ACCEPTED, `${MARK}: 状態の下書きが欄と一致する ${SIM_LABEL}`);
      await page.keyboard.press('Enter');
      await settle(page);
      await settle(page);
      assert.ok((await engine(page)).logLen > logLen0, `${MARK}: Enter で受付が増える ${SIM_LABEL}`);
      assert.equal(await draftOf(page), '', `送信の後に状態の下書きが空 ${SIM_LABEL}`);
      assert.equal(await page.inputValue(INPUT), '', `送信の後に欄も空 ${SIM_LABEL}`);
      assertClean(rec);
      await context.close();
    });
  });

  test(`B-2 保留した書き戻しは focusout で解消され、後の変換の確定した文字が消えない（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, cdp, rec } = await openTest(srv);
      await page.fill(INPUT, ACCEPTED);
      await page.focus(INPUT);
      await settle(page);
      // 同じタスクの中で送信と変換の開始を続け、書き戻しを保留にする（S-5(b) と同じ手順）。
      await page.evaluate(([sub, sel]) => {
        document.querySelector(sub).click();
        document.querySelector(sel).dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
      }, ['[data-testid="submit-input"]', INPUT]);
      await page.evaluate((sel) => document.querySelector(sel).blur(), INPUT);
      await settle(page);
      await settle(page);
      await page.focus(INPUT);
      await imeCompose(cdp, READING);
      await imeCommit(cdp, '確定');
      await page.waitForFunction((x) => window.__fandheTest.draft() === x, '確定', { timeout: T });
      await settle(page);
      await settle(page);
      assert.equal(await page.inputValue(INPUT), '確定', `確定した文字が欄に残る ${SIM_LABEL}`);
      assert.equal(await draftOf(page), '確定', `確定した文字が状態の下書きに残る ${SIM_LABEL}`);
      assertClean(rec);
      await context.close();
    });
  });

  test(`O-1（観察）変換中に欄の焦点を外したときの compositionend と focusout の順（CSP ${cspLabel}）`, async (t) => {
    await run(async (srv) => {
      const { page, context, cdp } = await openTest(srv);
      await page.evaluate((sel) => {
        window.__order = [];
        const el = document.querySelector(sel);
        el.addEventListener('compositionend', () => window.__order.push('compositionend'));
        el.addEventListener('focusout', () => window.__order.push('focusout'));
      }, INPUT);
      await imeCompose(cdp, READING);
      await page.evaluate((sel) => document.querySelector(sel).blur(), INPUT);
      await settle(page);
      t.diagnostic(`O-1 順: ${JSON.stringify(await page.evaluate(() => window.__order))}`);
      await context.close();
    });
  });

  test(`R-6 変換中に再読み込みすると、確定済みの文字列は戻り、変換中の文字列は戻らず保存先にも入らない（CSP ${cspLabel}）`, async (t) => {
    await run(async (srv) => {
      const { page, context, cdp, rec } = await openTest(srv);
      await imeCompose(cdp, 'かくてい');
      await imeCommit(cdp, '確定済み');
      await page.waitForFunction(() => window.__fandheTest.draft() === '確定済み', undefined, { timeout: T });
      await settle(page);
      await imeCompose(cdp, READING);
      await settle(page);
      assert.equal(await page.inputValue(INPUT), '確定済み' + READING, `再読み込みの前: 欄は確定済み＋変換中 ${SIM_LABEL}`);
      await page.reload();
      await waitMounted(page);
      await settle(page);
      const eng = await engine(page);
      t.diagnostic(`R-6(a) bootKind=${eng.bootKind}`);
      assert.equal(eng.bootKind, 'restored', `確定済みの下書きがあれば復元される ${SIM_LABEL}`);
      assert.equal(await page.inputValue(INPUT), '確定済み', `確定済みの文字列が戻り、変換中の文字列は戻らない ${SIM_LABEL}`);
      assert.equal(await draftOf(page), '確定済み', `状態の下書きも確定済みだけ ${SIM_LABEL}`);
      assert.ok(!(await draftOps(page)).some((d) => d.includes(READING)), `ops の draft に変換中の文字列が無い ${SIM_LABEL}`);
      assert.ok(!(await storageDump(page)).includes(READING), `保存先に変換中の文字列が無い ${SIM_LABEL}`);
      assertClean(rec);
      await context.close();
    });
  });

  test(`R-6 確定済みが無いとき、変換中の再読み込みの後は欄も下書きも空で、保存先に読みが無い（CSP ${cspLabel}）`, async (t) => {
    await run(async (srv) => {
      const { page, context, cdp, rec } = await openTest(srv);
      await imeCompose(cdp, READING);
      await settle(page);
      await page.reload();
      await waitMounted(page);
      await settle(page);
      t.diagnostic(`R-6(b) bootKind=${(await engine(page)).bootKind}`);
      assert.equal(await page.inputValue(INPUT), '', `欄は空 ${SIM_LABEL}`);
      assert.equal(await draftOf(page), '', `状態の下書きは空 ${SIM_LABEL}`);
      assert.ok(!(await storageDump(page)).includes(READING), `保存先に変換中の文字列が無い ${SIM_LABEL}`);
      assertClean(rec);
      await context.close();
    });
  });

  // 負の対照。読み方・数え方が空振りでないことを示す。
  test(`C-1 変換中に試験から欄へ書くと、書き込みの器が composing に数える（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, cdp, rec } = await openTest(srv);
      await clearValueWriteProbe(page);
      await imeCompose(cdp, READING);
      await page.evaluate((sel) => { document.querySelector(sel).value = 'x'; }, INPUT);
      assert.equal((await readValueWriteProbe(page)).composing, 1, `${SIM_LABEL}`);
      await imeCancel(cdp);
      assertClean(rec);
      await context.close();
    });
  });

  test(`C-2 変換せずに入れた読みは、再読み込みで保存先と欄に戻る（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await openTest(srv);
      await page.keyboard.insertText(READING);
      await page.waitForFunction((x) => window.__fandheTest.draft() === x, READING, { timeout: T });
      await settle(page);
      await page.reload();
      await waitMounted(page);
      await settle(page);
      const dump = await storageDump(page);
      const value = await page.inputValue(INPUT);
      assert.ok(dump.includes(READING) || value === READING, `読み方が空振りでない: 戻ったか保存された ${SIM_LABEL}`);
      assertClean(rec);
      await context.close();
    });
  });

  test(`C-3 変換していないときの補完は、欄を /help に書き戻す（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await openTest(srv);
      await page.fill(INPUT, '/he');
      await page.focus(INPUT);
      await settle(page);
      await clearValueWriteProbe(page);
      const before = (await state(page)).completeCount;
      await call(page, 'requestComplete');
      await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).completeCount === n + 1, before, { timeout: T });
      await page.waitForFunction((sel) => document.querySelector(sel).value === '/help', INPUT, { timeout: T });
      assert.ok((await readValueWriteProbe(page)).idle >= 1, `idle の書き込みが 1 回以上 ${SIM_LABEL}`);
      assertClean(rec);
      await context.close();
    });
  });
}
