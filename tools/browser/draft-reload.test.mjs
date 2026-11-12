// 再読み込みでの下書きの復元の層 3 ブラウザ試験（TASK-328 2/3＝#967。SAVE-23）。
//
// 守る期待（SAVE-23。PoC-2 の R-1〜R-5）: 入力欄の未送信の下書きは、再読み込みの後に 1 文字も変わらず欄と状態へ戻る。
//   対象は、改行・空行・Tab を含むもの、先頭や末尾が改行のもの、空白だけのもの、貼り付けた複数行、拒否された入力。
//   保存の遅延（畳み込み）より前の再読み込みでも、pagehide の同期の書き込みで戻る。
//     DR-1 文字の種類の表（落ち着いた後・入力の直後）   DR-2 貼り付けた複数行
//     DR-3 拒否の後の再読み込み（R-1＋R-2）             DR-4 送信の後に打った下書きだけが戻る
//     DR-5 保存の遅延の前の再読み込み（pagehide）        DR-5c 負の対照（pagehide を止めると戻らない）
//     DR-6 配信する設定（IR 同梱・試験用の口なし）で DR-1・DR-3・DR-5 の要点を DOM と localStorage だけで確かめる
// draft-sync（FEND-11）との違い: draft-sync は「欄の値＝状態の下書き」の観点で 4 場面を並べる。ここは SAVE-23 の列挙した
// 下書きの種類と、拒否・貼り付け・送信後・pagehide の組を、再読み込みの前後で比べる。
// 範囲外: 変換中の再読み込み（R-6）は ime-reload-flood（基本の形）と draft-reload-ime（SAVE-23 の場面との組。5.520。#968）が受け持つ。
//   実 IME・人の確認は TASK-93。
// DR-5 の作り方: 保留中のタイマーを全部消して以後のタイマーも捨ててから reload し、保存先へ届く道を pagehide だけにする。
//   その直前に保存先へ下書きが入っていれば判別できないので別の文で最大 3 回試し直し、3 回とも判別できなければ失敗にする。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_AUTHOR_DIR（IR 同梱・口なし）。未設定・不在は skip にせず失敗にする。
// 失敗の文には長さと真偽だけを出し、下書きの本文は出さない。固定の sleep は使わない。限界は docs/testing.md 5.519。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-draft-reload/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const LOG_ITEMS = '[data-testid="log-list"] > li';
const PACK = 'min-3step';
const START = 's-1';
const REJECTED = '/unknown-x';
const ACCEPTED = '/make-note first-note';
const T = 30000;
const SAMPLES = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../testdata/input-field-ime/test-data/paste-samples',
);
const normalize = (s) => s.replace(/\r\n?/g, '\n');

// DR-1 の文字の種類の表（PoC-2 の R-2〜R-4 と同じ種類）。
const KINDS = [
  ['単一行', 'hello world'],
  ['複数行・空行・Tab の混合', '1 行目\n\n3 行目（空行の後）\t（Tab 文字）'],
  ['先頭が改行', '\n先頭が改行'],
  ['先頭が二つの改行', '\n\n二つ'],
  ['末尾が改行', '末尾が改行\n'],
  ['空白だけ', '   '],
  ['Tab だけ', '\t'],
  ['全角空白', '　全角空白'],
];
const MULTI = '\n  メモを書く\n\n  二行目  \n';

let browser;

function needDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  needDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  needDir('FANDHE_WEB_AUTHOR_DIR', 'a web directory built with IR_FILE and without TEST_HOOKS');
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
  assert.equal(s.external, 0, detail);
}

const cpLen = (s) => [...s].length;
const settle = (page) => page.evaluate(() => new Promise((resolve) => {
  setTimeout(() => setTimeout(resolve, 0), 0);
}));
const call = (page, name, ...args) => page.evaluate(([n, a]) => window.__fandheTest[n](...a), [name, args]);
const state = async (page) => JSON.parse(await call(page, 'stateJson'));
const fieldOf = (page) => page.inputValue(INPUT);

// 欄の値（と、試験の設定では状態の下書き）が期待と同じ。本文は出さず、長さと真偽だけを失敗の文に出す。
async function assertRestored(page, label, expected, { hooks }) {
  const field = await fieldOf(page);
  assert.ok(field === expected, `${label}: 欄の値が違う（期待 ${cpLen(expected)} 文字・実際 ${cpLen(field)} 文字）`);
  if (hooks) {
    const draft = await call(page, 'draft');
    assert.ok(draft === expected, `${label}: 状態の下書きが違う（期待 ${cpLen(expected)} 文字・実際 ${cpLen(draft)} 文字）`);
  }
}

async function openTest(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  await settle(opened.page);
  return opened;
}

async function openAuthor(srv, init) {
  const opened = await openPage(browser, { origin: srv.origin, viewport: { width: 1024, height: 768 } });
  if (init) await opened.context.addInitScript(init);
  await opened.page.goto(`${srv.url}?start=s-1-1`);
  await waitMounted(opened.page);
  assert.equal(await opened.page.evaluate(() => typeof window.__fandheTest), 'undefined', 'author build must have no test hooks');
  await settle(opened.page);
  return opened;
}

async function reload(page) {
  await page.reload();
  await waitMounted(page);
}

async function send(page, how) {
  if (how === 'click') await page.click(SUBMIT);
  else {
    await page.focus(INPUT);
    await page.keyboard.press('Enter');
  }
}

// 保留中のタイマーを全部消し、以後のタイマーも捨てる。保存先へ届く道を pagehide だけにする（DR-5）。
const freezeTimers = (page) => page.evaluate(() => {
  const top = setTimeout(() => {}, 0);
  for (let i = 0; i <= top; i += 1) clearTimeout(i);
  window.setTimeout = () => 0;
});

// localStorage のどれかの値に needle が入っているか（真偽だけ返す）。
const storageHas = (page, needle) => page.evaluate((n) => {
  for (let i = 0; i < localStorage.length; i += 1) {
    const v = localStorage.getItem(localStorage.key(i));
    if (typeof v === 'string' && v.includes(n)) return true;
  }
  return false;
}, needle);

const escaped = (text) => JSON.stringify(text).slice(1, -1);

// pagehide だけで保存される場面を作って reload し、(再読み込みの前に既に保存されていたか) を返す。
async function typeFreezeReload(page, text, { clickAway = false } = {}) {
  await page.fill(INPUT, text);
  if (clickAway) await page.evaluate(() => document.activeElement?.blur());
  await freezeTimers(page);
  const alreadySaved = await storageHas(page, escaped(text));
  await page.reload();
  await waitMounted(page);
  return alreadySaved;
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const runTest = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));
  const runAuthor = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_AUTHOR_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  for (const [name, text] of KINDS) {
    for (const immediate of [false, true]) {
      test(`DR-1 ${name}: 再読み込みで 1 文字も変わらず戻る（${immediate ? '入力の直後' : '落ち着いた後'}。CSP ${cspLabel}）`, async () => {
        await runTest(async (srv) => {
          const { page, context, rec } = await openTest(srv);
          await page.fill(INPUT, text);
          if (!immediate) await settle(page);
          await reload(page);
          // settle の前に見る（mount の直後の欄。先頭の改行が落ちていないこと）。
          await assertRestored(page, `${name}（mount の直後）`, text, { hooks: false });
          await settle(page);
          await assertRestored(page, name, text, { hooks: true });
          assertClean(rec);
          await context.close();
        });
      });
    }
  }

  for (const file of ['paste-3lines.txt', 'paste-crlf-tabs.txt']) {
    test(`DR-2 貼り付けた複数行（${file}）: 再読み込みで戻り、貼り付けだけでは送信されない（CSP ${cspLabel}）`, async () => {
      await runTest(async (srv) => {
        const { page, context, rec } = await openTest(srv);
        const text = normalize(fs.readFileSync(path.join(SAMPLES, file), 'utf8'));
        const before = await page.$$eval(LOG_ITEMS, (l) => l.length);
        await page.evaluate((t) => navigator.clipboard.writeText(t), fs.readFileSync(path.join(SAMPLES, file), 'utf8'));
        await page.focus(INPUT);
        await page.keyboard.press('ControlOrMeta+V');
        await page.waitForFunction(([sel, n]) => document.querySelector(sel).value.length >= n, [INPUT, Math.min(text.length, 1)]);
        await settle(page);
        await assertRestored(page, `${file}（貼り付けた後）`, text, { hooks: true });
        assert.equal(await page.$$eval(LOG_ITEMS, (l) => l.length), before, '貼り付けだけでは送信が起きない');
        await reload(page);
        await settle(page);
        await assertRestored(page, `${file}（再読み込みの後）`, text, { hooks: true });
        assertClean(rec);
        await context.close();
      });
    });
  }

  for (const [label, body] of [['未知のコマンド', REJECTED], ['複数行の自然文', MULTI]]) {
    for (const how of ['enter', 'click']) {
      test(`DR-3 拒否の後の再読み込み（${label}。${how}。CSP ${cspLabel}）`, async () => {
        await runTest(async (srv) => {
          const { page, context, rec } = await openTest(srv);
          await page.fill(INPUT, body);
          await send(page, how);
          await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.received.length === 1, null, { timeout: T });
          await settle(page);
          await assertRestored(page, `${label}（拒否の後）`, body, { hooks: true });
          await reload(page);
          await settle(page);
          await assertRestored(page, `${label}（再読み込みの後）`, body, { hooks: true });
          assert.equal((await state(page)).engine.received.length, 1, '拒否の受付も復元されている');
          assertClean(rec);
          await context.close();
        });
      });
    }
  }

  test(`DR-4 送信の後に打った下書き: 戻るのは後から打った文だけ（CSP ${cspLabel}）`, async () => {
    await runTest(async (srv) => {
      const { page, context, rec } = await openTest(srv);
      await page.fill(INPUT, ACCEPTED);
      await send(page, 'click');
      await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT, { timeout: T });
      const later = '次の\n複数行\t下書き';
      await page.fill(INPUT, later);
      await settle(page);
      await reload(page);
      await settle(page);
      await assertRestored(page, '送信の後の下書き', later, { hooks: true });
      assertClean(rec);
      await context.close();
    });
  });

  // pagehide の判別は 1 回ごとに別の文を使い、最大 3 回まで試し直す（既に保存済みでは判別できない）。
  const pagehideCase = async (srv) => {
    let tries = 0;
    for (let i = 0; i < 3; i += 1) {
      const { page, context, rec } = await openTest(srv);
      const text = `leave-${i}\n二行目\t(${i})`;
      const key = await call(page, 'storageKey', PACK, START);
      assert.ok(typeof key === 'string' && key.length > 0, '保存先の鍵が引ける');
      tries += 1;
      const already = await typeFreezeReload(page, text);
      if (already) {
        await context.close();
        continue;
      }
      await settle(page);
      const field = await fieldOf(page);
      const restored = field === text && (await call(page, 'draft')) === text;
      assertClean(rec);
      await context.close();
      return { restored, tries };
    }
    assert.fail('保存の前に再読み込みする場面を作れず、pagehide の判別ができない（3 回とも保存済み）');
    return { restored: false, tries };
  };

  test(`DR-5 保存の遅延の前の再読み込み: pagehide の書き込みで戻る（CSP ${cspLabel}）`, async (t) => {
    await runTest(async (srv) => {
      const { restored, tries } = await pagehideCase(srv);
      t.diagnostic(`DR-5 試した回数: ${tries}`);
      assert.equal(restored, true, '再読み込みの後に欄と状態の下書きが戻る');
    });
  });

  test(`DR-5c 負の対照: pagehide を止めると戻らない（DR-5 が pagehide によることの根拠。CSP ${cspLabel}）`, async (t) => {
    await runTest(async (srv) => {
      const blockInit = () => {
        const stop = (e) => e.stopImmediatePropagation();
        window.addEventListener('pagehide', stop, true);
        window.addEventListener('visibilitychange', stop, true);
        document.addEventListener('visibilitychange', stop, true);
      };
      let tries = 0;
      for (let i = 0; i < 3; i += 1) {
        const opened = await openPage(browser, { origin: srv.origin });
        await opened.context.addInitScript(blockInit);
        await opened.page.goto(srv.url);
        await waitMounted(opened.page);
        await settle(opened.page);
        const text = `blocked-${i}\n二行目`;
        tries += 1;
        const already = await typeFreezeReload(opened.page, text);
        if (already) {
          await opened.context.close();
          continue;
        }
        await settle(opened.page);
        t.diagnostic(`DR-5c 試した回数: ${tries}`);
        assert.ok((await fieldOf(opened.page)) !== text, 'pagehide を止めた対照では欄に戻らない');
        assert.ok((await call(opened.page, 'draft')) !== text, 'pagehide を止めた対照では状態の下書きにも戻らない');
        await opened.context.close();
        return;
      }
      assert.fail('対照の場面を作れなかった（3 回とも保存済み）');
    });
  });

  test(`DR-5 タブを閉じて開き直す変種: 閉じる直前の下書きが戻る（CSP ${cspLabel}）`, async () => {
    await runTest(async (srv) => {
      const { page, context, rec } = await openTest(srv);
      const text = 'close-tab\n二行目\t(Tab)';
      await page.fill(INPUT, text);
      await freezeTimers(page);
      // 開き直すページは、ハーネスのポップアップの自動閉じの対象にしない。
      context.removeAllListeners('page');
      await page.close({ runBeforeUnload: true });
      const next = await context.newPage();
      await next.goto(srv.url);
      await waitMounted(next);
      await settle(next);
      await assertRestored(next, 'タブを閉じた後', text, { hooks: true });
      assert.equal(summarize(rec).csp, 0);
      await context.close();
    });
  });

  // DR-6: 配信する設定（IR 同梱・口なし）。DOM と localStorage だけを観察する。
  for (const immediate of [false, true]) {
    test(`DR-6a 配信する設定: 先頭の改行・混合の複数行・空白だけが戻る（${immediate ? '入力の直後' : '落ち着いた後'}。CSP ${cspLabel}）`, async () => {
      await runAuthor(async (srv) => {
        const { page, context, rec } = await openAuthor(srv);
        for (const [name, text] of KINDS.filter(([n]) => ['先頭が改行', '複数行・空行・Tab の混合', '空白だけ'].includes(n))) {
          await page.fill(INPUT, text);
          if (!immediate) await settle(page);
          await reload(page);
          await assertRestored(page, `${name}（mount の直後）`, text, { hooks: false });
          await settle(page);
          await assertRestored(page, name, text, { hooks: false });
        }
        assertClean(rec);
        await context.close();
      });
    });
  }

  test(`DR-6b 配信する設定: 拒否の後の再読み込みで本文が戻る（CSP ${cspLabel}）`, async () => {
    await runAuthor(async (srv) => {
      const { page, context, rec } = await openAuthor(srv);
      for (const body of [REJECTED, MULTI]) {
        await page.fill(INPUT, body);
        await send(page, 'click');
        await page.waitForSelector('[data-testid="part-log-list"] li[data-kind="rejected"]', { timeout: T });
        await settle(page);
        await assertRestored(page, '拒否の後', body, { hooks: false });
        await reload(page);
        await settle(page);
        await assertRestored(page, '再読み込みの後', body, { hooks: false });
        await page.waitForSelector('[data-testid="part-log-list"] li[data-kind="rejected"]', { timeout: T });
      }
      assertClean(rec);
      await context.close();
    });
  });

  test(`DR-6c 配信する設定: 保存の遅延の前の再読み込みでも pagehide で戻り、止めた対照では戻らない（CSP ${cspLabel}）`, async (t) => {
    await runAuthor(async (srv) => {
      let tries = 0;
      let proved = false;
      for (let i = 0; i < 3 && !proved; i += 1) {
        const { page, context, rec } = await openAuthor(srv);
        const text = `author-leave-${i}\n二行目\t(${i})`;
        tries += 1;
        const already = await typeFreezeReload(page, text);
        if (!already) {
          await settle(page);
          await assertRestored(page, 'pagehide の後', text, { hooks: false });
          proved = true;
        }
        assertClean(rec);
        await context.close();
      }
      t.diagnostic(`DR-6c 試した回数: ${tries}`);
      assert.ok(proved, '保存の前に再読み込みする場面を作れなかった（3 回とも保存済み）');

      const blockInit = () => {
        const stop = (e) => e.stopImmediatePropagation();
        window.addEventListener('pagehide', stop, true);
        window.addEventListener('visibilitychange', stop, true);
        document.addEventListener('visibilitychange', stop, true);
      };
      let controlled = false;
      for (let i = 0; i < 3 && !controlled; i += 1) {
        const { page, context } = await openAuthor(srv, blockInit);
        const text = `author-blocked-${i}\n二行目`;
        const already = await typeFreezeReload(page, text);
        if (!already) {
          await settle(page);
          assert.ok((await fieldOf(page)) !== text, 'pagehide を止めた対照では欄に戻らない');
          controlled = true;
        }
        await context.close();
      }
      assert.ok(controlled, '対照の場面を作れなかった');
    });
  });
}
