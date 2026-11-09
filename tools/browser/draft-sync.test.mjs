// 更新の後に状態の下書きを <textarea> へ反映することの層 3 ブラウザ試験（TASK-356 1/2。FEND-11）。
//
// 守る期待（FEND-11）:
//   上流（fandhe-frontend-wasm-client）は value の現在値を <input> のときだけ同期し <textarea> は同期しない（PoC-2 の F-1）。
//   そのため glue が、欄と状態の下書きがずれる 4 場面で欄へ書き戻す。この試験は 4 場面を「欄の値＝状態の下書き」の
//   1 つの観点で並べて固定する（PoC-2 の R-1〜R-5 に当たる）。変換中に書かないことも確かめる。
//     S-1  送信後の空化            S-2  拒否の後の保持と全選択・表示だけの間の巻き戻し
//     S-3  上限の切り捨て          S-4  再読み込みでの復元
//     S-5  変換中は書かない
//     S-4n 先頭が改行の下書きが mount の直後に欄へ戻る（TASK-356 2/2＝#964）  C-4  HTML の読み込みが先頭の改行を捨てる対照
// 時機: この repo の書き戻しは set_timeout(0)（更新の次のタスク）で、spec 文言のマイクロタスクではない（docs/known-gaps.md の FEND-11）。
// 待ち: 固定の sleep は使わない（settle と waitForFunction）。入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。
// 失敗の文には長さと真偽だけを出し、下書きの本文は出さない。実 IME ではない（TASK-93。未実施）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import {
  SIM_LABEL, openIme, imeCompose, imeCommit,
  installValueWriteProbe, readValueWriteProbe, clearValueWriteProbe,
} from './ime.mjs';

const BASE = '/fandhe-draft-sync/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const PACK = 'min-3step';
const START = 's-1';
const MAX = 20000;
const T = 30000;
const REJECTED = '/unknown-x';
const ACCEPTED = '/make-note first-note';

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

const cpLen = (s) => [...s].length;
const settle = (page) => page.evaluate(() => new Promise((resolve) => {
  setTimeout(() => setTimeout(resolve, 0), 0);
}));
const call = (page, name, ...args) => page.evaluate(([n, a]) => window.__fandheTest[n](...a), [name, args]);
const draftOf = (page) => call(page, 'draft');
const state = async (page) => JSON.parse(await call(page, 'stateJson'));
const fieldOf = (page) => page.inputValue(INPUT);

// 欄の値と状態の下書きの一致（本文は出さず、長さと真偽だけ）。
async function synced(page) {
  const [field, draft] = [await fieldOf(page), await draftOf(page)];
  return { same: field === draft, fieldLen: cpLen(field), draftLen: cpLen(draft), field, draft };
}

// 書き戻しの後に欄の値と状態の下書きが一致するまで待つ（一致しなければ時間切れで失敗する）。
async function waitSynced(page, label, timeout = 5000) {
  try {
    await page.waitForFunction(
      (sel) => document.querySelector(sel).value === window.__fandheTest.draft(), INPUT, { timeout },
    );
  } catch {
    const s = await synced(page);
    assert.fail(`${label}: 欄と状態の下書きが一致しない（欄 ${s.fieldLen} 文字・下書き ${s.draftLen} 文字）`);
  }
  await settle(page);
  const s = await synced(page);
  assert.ok(s.same, `${label}: 一致の後に再びずれた（欄 ${s.fieldLen} 文字・下書き ${s.draftLen} 文字）`);
  return s;
}

async function open(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: srv.origin });
  await installValueWriteProbe(opened.context, INPUT);
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  await settle(opened.page);
  return opened;
}

async function setValueWithInput(page, text) {
  await page.evaluate(([sel, t]) => {
    const el = document.querySelector(sel);
    el.value = t;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, [INPUT, text]);
}

async function send(page, how) {
  if (how === 'click') await page.click(SUBMIT);
  else {
    await page.focus(INPUT);
    await page.keyboard.press('Enter');
  }
}

// 開ける包みの content の主版だけを上げた履歴（結果は view_only の content_major）。
async function majorBumpedText(page) {
  const current = JSON.parse(await call(page, 'loadableHistoryText'));
  const parts = current.versions.content.split('.').map(Number);
  parts[0] += 1;
  return JSON.stringify({ ...current, versions: { ...current.versions, content: parts.join('.') } });
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const run = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  for (const how of ['click', 'enter']) {
    test(`S-1 送信後の空化: 欄の値＝状態の下書き＝空（${how}。CSP ${cspLabel}）`, async (t) => {
      await run(async (srv) => {
        const { page, context, rec } = await open(srv);
        await page.fill(INPUT, ACCEPTED);
        assert.equal((await synced(page)).same, true, '送信の前: 欄＝下書き');
        await send(page, how);
        await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT, { timeout: T });
        const s = await waitSynced(page, 'S-1');
        assert.equal(s.fieldLen, 0, '送信の後の欄は空');
        assert.equal(s.draftLen, 0, '送信の後の状態の下書きは空');
        // 時機の観察（assert しない）: 同じタスクの中では欄は送信前の本文のまま。
        await page.fill(INPUT, ACCEPTED);
        await page.focus(INPUT);
        const sameTask = await page.evaluate(([sel, sub]) => {
          document.querySelector(sub).click();
          return document.querySelector(sel).value.length;
        }, [INPUT, SUBMIT]);
        t.diagnostic(`S-1 同じタスクの中の欄の長さ（観察）: ${sameTask}`);
        await waitSynced(page, 'S-1 の観察の後');
        assertClean(rec);
        await context.close();
      });
    });
  }

  test(`S-2a 拒否の後: 欄の値＝状態の下書き＝送った本文で、全体が選択される（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      for (const how of ['click', 'enter']) {
        const { page, context, rec } = await open(srv);
        await page.fill(INPUT, REJECTED);
        await send(page, how);
        await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.received.length === 1, null, { timeout: T });
        const s = await waitSynced(page, `S-2a ${how}`);
        assert.equal(s.field, REJECTED, `${how}: 拒否の後も本文が残る`);
        const sel = await page.evaluate((q) => {
          const el = document.querySelector(q);
          return [el.selectionStart, el.selectionEnd];
        }, INPUT);
        assert.deepEqual(sel, [0, REJECTED.length], `${how}: 全体が選択されている`);
        assertClean(rec);
        await context.close();
      }
    });
  });

  test(`S-2b 表示だけの履歴で欄へ打っても、欄の値が状態の下書きへ戻り、状態は変わらない（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const text = await majorBumpedText(page);
      await call(page, 'storageOpen');
      await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [PACK, START], { timeout: 10000 });
      assert.equal(JSON.parse(await call(page, 'storageWrite', PACK, START, text)).ok, true, '履歴を置いた');
      await page.reload();
      await waitMounted(page);
      await settle(page);
      assert.equal((await state(page)).engine.viewOnly, 'content_major', '表示だけで開いた');
      const base = { draft: await draftOf(page), count: await call(page, 'draftSetCount'), eng: JSON.stringify((await state(page)).engine) };
      await page.focus(INPUT);
      await page.keyboard.type('abc');
      const s = await waitSynced(page, 'S-2b');
      assert.equal(s.draft, base.draft, '状態の下書きは変わらない');
      assert.equal(await call(page, 'draftSetCount'), base.count, '下書きの配送は増えない');
      assert.equal(JSON.stringify((await state(page)).engine), base.eng, '状態は変わらない');
      assertClean(rec);
      await context.close();
    });
  });

  test(`S-3a 上限の切り捨て（末尾への超過）: 欄の値＝状態の下書きで 20,000 コードポイント（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      for (const [label, text] of [
        ['BMP', 'あ'.repeat(MAX + 1)],
        ['サロゲートペア', 'あ'.repeat(MAX - 1) + '😀😀'],
      ]) {
        await setValueWithInput(page, text);
        const s = await waitSynced(page, `S-3a ${label}`);
        assert.equal(s.fieldLen, MAX, `${label}: 欄は 20,000 コードポイント`);
        assert.equal(s.field.isWellFormed(), true, `${label}: ペアを割らない`);
        assert.equal((await state(page)).inputOverLimit, true, `${label}: 超過の印`);
      }
      assertClean(rec);
      await context.close();
    });
  });

  test(`S-3b 上限の切り捨て（途中への超過の貼り付け）: 欄の値＝状態の下書き（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await setValueWithInput(page, 'あ'.repeat(MAX - 10));
      await waitSynced(page, 'S-3b の準備');
      await page.evaluate((t) => navigator.clipboard.writeText(t), 'い'.repeat(100));
      await page.evaluate((sel) => {
        const el = document.querySelector(sel);
        el.focus();
        el.setSelectionRange(5, 5);
      }, INPUT);
      await page.keyboard.press('ControlOrMeta+V');
      const s = await waitSynced(page, 'S-3b');
      assert.equal(s.fieldLen, MAX, '貼った後も 20,000 コードポイント');
      assertClean(rec);
      await context.close();
    });
  });

  const reloadCases = [
    ['単一行', 'hello world'],
    ['複数行と空行', 'a\n\nb\nc'],
    ['Tab 文字', 'a\tb\t\tc'],
    ['末尾の改行', 'tail\n'],
    ['空白だけ', '   '],
  ];
  for (const [name, text] of reloadCases) {
    for (const immediate of [false, true]) {
      test(`S-4 再読み込みでの復元（${name}。${immediate ? '入力の直後' : '落ち着いた後'}。CSP ${cspLabel}）`, async () => {
        await run(async (srv) => {
          const { page, context, rec } = await open(srv);
          await page.fill(INPUT, text);
          if (!immediate) await settle(page);
          await page.reload();
          await waitMounted(page);
          await settle(page);
          const s = await waitSynced(page, `S-4 ${name}`);
          assert.equal(s.draft, text, `${name}: 状態の下書きが戻る`);
          assert.equal(s.field, text, `${name}: 欄の値が戻る`);
          assertClean(rec);
          await context.close();
        });
      });
    }
  }

  test(`S-4n 先頭が改行の下書き: mount の直後に欄の値＝状態の下書き（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      for (const text of ['\n先頭が改行', '\n\n二つ']) {
        await page.fill(INPUT, text);
        await settle(page);
        await page.reload();
        await waitMounted(page);
        // settle・waitSynced の前に見る（後続のイベントによる同期と区別する）。
        const field = await fieldOf(page);
        assert.equal(field === text, true, `mount の直後に欄の値が下書きと一致（欄 ${cpLen(field)} 文字・期待 ${cpLen(text)} 文字）`);
        assert.equal(await draftOf(page), text, '状態の下書きが戻る');
        assert.ok((await readValueWriteProbe(page)).idle >= 1, 'mount の時点で欄へ書いている');
        await settle(page);
        const s = await waitSynced(page, 'S-4n');
        assert.equal(s.same, true);
      }
      assertClean(rec);
      await context.close();
    });
  });

  test(`S-4n 値が同じなら mount の直後に書かない（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, 'hello');
      await settle(page);
      await page.reload();
      await waitMounted(page);
      await settle(page);
      assert.equal((await readValueWriteProbe(page)).idle, 0, '一致していれば書き込み 0 回');
      assert.equal(await fieldOf(page), 'hello');
      assertClean(rec);
      await context.close();
    });
  });

  test(`C-4 負の対照: HTML の読み込みは textarea 直後の先頭の改行を捨てる（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const v = await page.evaluate(() => {
        const d = document.createElement('div');
        d.innerHTML = '<textarea>\nx</textarea>';
        return d.querySelector('textarea').value;
      });
      assert.equal(v, 'x', '先頭の改行が捨てられる（S-4n が修正の有無を見分ける根拠）');
      assertClean(rec);
      await context.close();
    });
  });

  test(`S-5 変換中は欄へ書かず、確定の後に欄の値＝状態の下書き（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const cdp = await openIme(page);
      await page.fill(INPUT, '/he');
      await page.focus(INPUT);
      await settle(page);
      await clearValueWriteProbe(page);
      // (a) 変換中に補完（書き戻しの要求）が届く。
      const before = (await state(page)).completeCount;
      await imeCompose(cdp, 'へんかん');
      await call(page, 'requestComplete');
      await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).completeCount === n + 1, before, { timeout: T });
      await settle(page);
      assert.equal((await readValueWriteProbe(page)).composing, 0, `(a) 変換中の書き込み 0 回 ${SIM_LABEL}`);
      await imeCommit(cdp, '確定');
      await waitSynced(page, 'S-5(a) 確定の後', T);
      // (b) 同じタスクの中で送信と変換の開始を続ける。
      await page.fill(INPUT, ACCEPTED);
      await page.focus(INPUT);
      await clearValueWriteProbe(page);
      await page.evaluate(([sub, sel]) => {
        document.querySelector(sub).click();
        document.querySelector(sel).dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
      }, [SUBMIT, INPUT]);
      await settle(page);
      await settle(page);
      assert.equal((await readValueWriteProbe(page)).composing, 0, `(b) 変換中の書き込み 0 回 ${SIM_LABEL}`);
      await page.evaluate((sel) => {
        document.querySelector(sel).dispatchEvent(new CompositionEvent('compositionend', { bubbles: true, data: '' }));
      }, INPUT);
      await waitSynced(page, 'S-5(b) 変換の終わりの後');
      assertClean(rec);
      await context.close();
    });
  });

  // 負の対照。
  test(`C-1 変換中に試験から欄へ書くと、書き込みの器が 1 回と数える（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      const cdp = await openIme(page);
      await page.focus(INPUT);
      await clearValueWriteProbe(page);
      await imeCompose(cdp, 'へんかん');
      await page.evaluate((sel) => { document.querySelector(sel).value = 'x'; }, INPUT);
      assert.equal((await readValueWriteProbe(page)).composing, 1, SIM_LABEL);
      assertClean(rec);
      await context.close();
    });
  });

  test(`C-2 変換していないときの補完は欄を書き戻す（書き込み 1 回以上。CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.fill(INPUT, '/he');
      await page.focus(INPUT);
      await settle(page);
      await clearValueWriteProbe(page);
      await call(page, 'requestComplete');
      await page.waitForFunction(() => window.__fandheTest.draft() === '/help', null, { timeout: T });
      await waitSynced(page, 'C-2');
      const probe = await readValueWriteProbe(page);
      assert.ok(probe.idle >= 1, `書き戻しが空振りでない（idle=${probe.idle}）`);
      assertClean(rec);
      await context.close();
    });
  });

  test(`C-3 input を出さずに欄だけを書き換えると、判定器が不一致を検出する（CSP ${cspLabel}）`, async () => {
    await run(async (srv) => {
      const { page, context, rec } = await open(srv);
      await page.evaluate((sel) => { document.querySelector(sel).value = 'ずれ'; }, INPUT);
      await settle(page);
      assert.equal((await synced(page)).same, false, '不一致を検出する');
      await assert.rejects(() => waitSynced(page, 'C-3', 500), /一致しない/);
      assertClean(rec);
      await context.close();
    });
  });
}
