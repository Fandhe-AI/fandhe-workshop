// 変換中の文字列を下書きに入れないことの層 3 ブラウザ試験（TASK-328 3/3＝#968。SAVE-23 の R-6 を、SAVE-23 の場面と組にする）。
//
// 守る期待（SAVE-23。PoC-2 の R-6）: 変換中に再読み込みしても、変換中の文字列は下書き（欄・状態・ops・保存先）に入らず、
//   確定済みの文字列だけが 1 文字も変わらず戻る。変換中の input は上流の Runtime が延期し、compositionend の確定値だけが
//   ホストの record_draft_change に届く。リポジトリの中に変換中の SetDraft を止める門は無いため、この性質を検出するリポジトリ内の
//   変異は作らない（延期は上流の責務）。観察が空振りでないことは、確定させた読みが戻る対照（DI-5c・DI-6c）で示す。
//     DI-1 SAVE-23 の下書きの種類 5 通りに続けて変換し、末尾で再読み込み
//     DI-2 複数行の途中へキャレットを置いて変換し、再読み込み
//     DI-3 確定を 1 回行った後に、もう一度変換して再読み込み
//     DI-4 拒否された入力が欄に残った後の変換（全体が選択されたまま・末尾へ動かしてから）
//     DI-5 保存の遅延の前（pagehide だけが保存先へ届く道）の変換中の再読み込み。タブを閉じて開き直す変種も置く
//     DI-5c 負の対照（同じ手順で読みを確定させると pagehide の経路でも戻る）
//     DI-6 配信する設定（IR 同梱・試験用の口なし）で DI-1・DI-4・DI-5・DI-5c の要点を DOM と localStorage だけで確かめる
// ime-reload-flood の R-6（a）（b）・C-2 は、確定して落ち着いた後の基本形（試験の設定のみ）。ここはそれを複製せず、
// 下書きの種類・途中への変換・拒否の後・pagehide・配信する設定との組を足す。
// 証拠の種類はシミュレーター（CDP による IME の模擬）で、実 IME でも人の評価でもない（実 IME は TASK-93。未実施）。
// DI-5 の作り方は draft-reload の DR-5 と同じ（タイマーを止めて保存先へ届く道を pagehide だけにし、保存済みなら別の文で最大 3 回試し直す）。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_AUTHOR_DIR（IR 同梱・口なし）。未設定・不在は skip にせず失敗にする。
// 失敗の文には長さと真偽だけを出し、下書きの本文は出さない。固定の sleep は使わない。限界は docs/testing.md 5.520。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import {
  SIM_LABEL, openIme, assertFocused, imeCompose, imeCommit,
  installValueWriteProbe, readValueWriteProbe, clearValueWriteProbe,
} from './ime.mjs';

const BASE = '/fandhe-draft-reload-ime/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const REJECTED_LI = '[data-testid="part-log-list"] li[data-kind="rejected"]';
const READING = 'みかくていのよみ';
const READING2 = 'ほりゅうちゅうのよみ';
const REJECTED = '/unknown-x';
const MULTI = '\n  メモを書く\n\n  二行目  \n';
const T = 30000;

// SAVE-23 の下書きの種類（先頭が改行・混合・末尾が改行・空白だけ・全角空白）。
const KINDS = [
  ['先頭が改行', '\n先頭が改行'],
  ['複数行・空行・Tab の混合', '1 行目\n\n3 行目（空行の後）\t（Tab 文字）'],
  ['末尾が改行', '末尾が改行\n'],
  ['空白だけ', '   '],
  ['全角空白', '　全角空白'],
];

let browser;

function needDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
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
const escaped = (text) => JSON.stringify(text).slice(1, -1);

// 保留中のタイマーを全部消し、以後のタイマーも捨てる。保存先へ届く道を pagehide だけにする（DI-5）。以後は settle を使わない。
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

// 受講中のホストの書き出しをダウンロードして ops の draft の列を返す（試験の設定のみ。読み取りのみ）。
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

async function openTest(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await installValueWriteProbe(opened.context, INPUT);
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  await settle(opened.page);
  return { ...opened, cdp: await openIme(opened.page) };
}

async function openAuthor(srv) {
  const opened = await openPage(browser, { origin: srv.origin, viewport: { width: 1024, height: 768 } });
  await opened.page.goto(`${srv.url}?start=s-1-1`);
  await waitMounted(opened.page);
  assert.equal(await opened.page.evaluate(() => typeof window.__fandheTest), 'undefined', 'author build must have no test hooks');
  await settle(opened.page);
  return { ...opened, cdp: await openIme(opened.page) };
}

// 欄にフォーカスを置き、キャレットを pos（UTF-16 の位置）へ置いて読みを変換中にする。
async function composeAt(page, cdp, pos, reading) {
  await page.focus(INPUT);
  await page.evaluate(([sel, p]) => document.querySelector(sel).setSelectionRange(p, p), [INPUT, pos]);
  await assertFocused(page, INPUT);
  await imeCompose(cdp, reading);
}

async function reload(page) {
  await page.reload();
  await waitMounted(page);
}

// 再読み込みの後の確認。mount の直後と落ち着いた後の両方で、欄＝確定済み（試験の設定では状態の下書きも）で、読みはどこにも無い。
async function assertRestoredOnly(page, label, expected, { hooks }) {
  const f0 = await fieldOf(page);
  assert.ok(f0 === expected, `${label}（mount の直後）: 欄の値が違う（期待 ${cpLen(expected)} 文字・実際 ${cpLen(f0)} 文字）`);
  await settle(page);
  const f1 = await fieldOf(page);
  assert.ok(f1 === expected, `${label}: 欄の値が違う（期待 ${cpLen(expected)} 文字・実際 ${cpLen(f1)} 文字）`);
  assert.equal(f1.includes(READING) || f1.includes(READING2), false, `${label}: 欄に読みが入っていない ${SIM_LABEL}`);
  assert.equal((await storageHas(page, READING)) || (await storageHas(page, READING2)), false, `${label}: 保存先に読みが無い ${SIM_LABEL}`);
  if (hooks) {
    const draft = await call(page, 'draft');
    assert.ok(draft === expected, `${label}: 状態の下書きが違う（期待 ${cpLen(expected)} 文字・実際 ${cpLen(draft)} 文字）`);
    const ops = await draftOps(page);
    assert.equal(ops.some((d) => d.includes(READING) || d.includes(READING2)), false, `${label}: ops の draft に読みが無い ${SIM_LABEL}`);
  }
}

for (const csp of [true, false]) {
  const cspLabel = csp ? 'あり' : 'なし';
  const runTest = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));
  const runAuthor = (fn) => withCspMode({ dir: process.env.FANDHE_WEB_AUTHOR_DIR, csp }, (dir) => withServer({ dir, base: BASE }, fn));

  for (const [name, text] of KINDS) {
    test(`DI-1 ${name}: 続けて変換中に再読み込みしても、確定済みだけが 1 文字も変わらず戻る（CSP ${cspLabel}）`, async (t) => {
      await runTest(async (srv) => {
        const { page, context, cdp, rec } = await openTest(srv);
        await page.fill(INPUT, text);
        await settle(page);
        assert.equal(await storageHas(page, READING), false, '前提: 保存先に読みが無い');
        await clearValueWriteProbe(page);
        await composeAt(page, cdp, text.length, READING);
        await settle(page);
        assert.ok((await fieldOf(page)) === text + READING, `前提: 欄は確定済み＋変換中 ${SIM_LABEL}`);
        assert.ok((await call(page, 'draft')) === text, `変換中も状態の下書きは確定済みのまま ${SIM_LABEL}`);
        assert.equal((await readValueWriteProbe(page)).composing, 0, `変換中の欄への書き込みが 0 回 ${SIM_LABEL}`);
        await reload(page);
        t.diagnostic(`DI-1 bootKind=${(await state(page)).engine.bootKind}`);
        await assertRestoredOnly(page, name, text, { hooks: true });
        assertClean(rec);
        await context.close();
      });
    });
  }

  test(`DI-2 複数行の途中へキャレットを置いて変換し、再読み込みしても元の文だけが戻る（CSP ${cspLabel}）`, async () => {
    await runTest(async (srv) => {
      const { page, context, cdp, rec } = await openTest(srv);
      const text = '1 行目\n\n3 行目\t(Tab)';
      const pos = text.indexOf('\n') + 1;
      await page.fill(INPUT, text);
      await settle(page);
      await composeAt(page, cdp, pos, READING);
      await settle(page);
      assert.ok((await fieldOf(page)) === text.slice(0, pos) + READING + text.slice(pos), `前提: 途中に変換中の読みがある ${SIM_LABEL}`);
      await reload(page);
      await assertRestoredOnly(page, '途中への変換', text, { hooks: true });
      assertClean(rec);
      await context.close();
    });
  });

  test(`DI-3 確定を 1 回行った後にもう一度変換して再読み込みすると、確定した分までが戻る（CSP ${cspLabel}）`, async () => {
    await runTest(async (srv) => {
      const { page, context, cdp, rec } = await openTest(srv);
      const base = '\n下地\n二行目';
      await page.fill(INPUT, base);
      await settle(page);
      await composeAt(page, cdp, base.length, 'かくてい');
      await imeCommit(cdp, '確定');
      await page.waitForFunction((d) => window.__fandheTest.draft() === d, `${base}確定`, { timeout: T });
      await settle(page);
      await composeAt(page, cdp, `${base}確定`.length, READING);
      await settle(page);
      await reload(page);
      await assertRestoredOnly(page, '確定の後の変換', `${base}確定`, { hooks: true });
      assertClean(rec);
      await context.close();
    });
  });

  for (const body of [REJECTED, MULTI]) {
    for (const select of ['全体が選択されたまま', '末尾へ動かしてから']) {
      test(`DI-4 拒否（${body === REJECTED ? '未知のコマンド' : '複数行の自然文'}）が欄に残った後、${select}変換して再読み込み（CSP ${cspLabel}）`, async (t) => {
        await runTest(async (srv) => {
          const { page, context, cdp, rec } = await openTest(srv);
          await page.fill(INPUT, body);
          await page.focus(INPUT);
          await page.keyboard.press('Enter');
          await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.received.length === 1, null, { timeout: T });
          await settle(page);
          assert.ok((await fieldOf(page)) === body, '前提: 拒否の後も本文が欄に残る');
          if (select === '全体が選択されたまま') {
            await page.focus(INPUT);
            await assertFocused(page, INPUT);
            await imeCompose(cdp, READING);
          } else {
            await composeAt(page, cdp, body.length, READING);
          }
          await settle(page);
          t.diagnostic(`DI-4 変換中の欄の長さ=${cpLen(await fieldOf(page))}（本文 ${cpLen(body)}）`);
          assert.ok((await call(page, 'draft')) === body, `変換中も状態の下書きは本文のまま ${SIM_LABEL}`);
          await reload(page);
          await assertRestoredOnly(page, '拒否の後', body, { hooks: true });
          assert.equal((await state(page)).engine.received.length, 1, '拒否の受付も復元されている');
          assertClean(rec);
          await context.close();
        });
      });
    }
  }

  // pagehide の判別は 1 回ごとに別の文を使い、最大 3 回まで試し直す（既に保存済みでは判別できない）。
  // freezeTimers は setTimeout を無効にするため、その後は settle を呼ばない。
  const pagehideCase = async (open, label, { commit }) => {
    for (let i = 0; i < 3; i += 1) {
      const { page, context, cdp, rec } = await open();
      const text = `leave-${i}\n二行目\t(${i})`;
      await page.fill(INPUT, text);
      await freezeTimers(page);
      const expected = commit ? text + READING : text;
      const already = await storageHas(page, escaped(expected));
      if (already) {
        await context.close();
        continue;
      }
      await composeAt(page, cdp, text.length, READING);
      if (commit) {
        await imeCommit(cdp, READING);
        await page.waitForFunction(([sel, v]) => document.querySelector(sel).value === v, [INPUT, expected]);
      }
      await page.reload();
      await waitMounted(page);
      await settle(page);
      const field = await fieldOf(page);
      const readingInStore = await storageHas(page, READING);
      assertClean(rec);
      await context.close();
      return { field, expected, readingInStore, tries: i + 1 };
    }
    assert.fail(`${label}: 保存の前に再読み込みする場面を作れず判別できない（3 回とも保存済み）`);
    return null;
  };

  test(`DI-5 保存の遅延の前の変換中の再読み込み: pagehide でも確定済みだけが戻り、読みは保存先に入らない（CSP ${cspLabel}）`, async (t) => {
    await runTest(async (srv) => {
      const r = await pagehideCase(() => openTest(srv), 'DI-5', { commit: false });
      t.diagnostic(`DI-5 試した回数: ${r.tries}`);
      assert.ok(r.field === r.expected, `確定済みの文が戻る（期待 ${cpLen(r.expected)} 文字・実際 ${cpLen(r.field)} 文字）`);
      assert.equal(r.readingInStore, false, `保存先に読みが無い ${SIM_LABEL}`);
    });
  });

  test(`DI-5 タブを閉じて開き直す変種: 変換中に閉じても確定済みだけが戻る（CSP ${cspLabel}）`, async () => {
    await runTest(async (srv) => {
      const { page, context, cdp, rec } = await openTest(srv);
      const text = 'close-tab\n二行目\t(Tab)';
      await page.fill(INPUT, text);
      await freezeTimers(page);
      await composeAt(page, cdp, text.length, READING2);
      // 開き直すページは、ハーネスのポップアップの自動閉じの対象にしない。
      context.removeAllListeners('page');
      await page.close({ runBeforeUnload: true });
      const next = await context.newPage();
      await next.goto(srv.url);
      await waitMounted(next);
      await settle(next);
      assert.ok((await fieldOf(next)) === text, '閉じた後の欄は確定済みだけ');
      assert.equal(await storageHas(next, READING2), false, `保存先に読みが無い ${SIM_LABEL}`);
      assert.equal(summarize(rec).csp, 0);
      await context.close();
    });
  });

  test(`DI-5c 負の対照: 同じ手順で読みを確定させると pagehide の経路でも戻る（CSP ${cspLabel}）`, async (t) => {
    await runTest(async (srv) => {
      const r = await pagehideCase(() => openTest(srv), 'DI-5c', { commit: true });
      t.diagnostic(`DI-5c 試した回数: ${r.tries}`);
      assert.ok(r.field === r.expected, `確定した読みは戻る（期待 ${cpLen(r.expected)} 文字・実際 ${cpLen(r.field)} 文字）`);
      assert.equal(r.readingInStore, true, '確定した読みは保存先にある（観察が空振りでない）');
    });
  });

  // DI-6: 配信する設定（IR 同梱・口なし）。DOM と localStorage だけを観察する。
  test(`DI-6a 配信する設定: 先頭が改行・混合の複数行に続けて変換して再読み込みしても確定済みだけが戻る（CSP ${cspLabel}）`, async () => {
    await runAuthor(async (srv) => {
      const { page, context, cdp, rec } = await openAuthor(srv);
      for (const [name, text] of KINDS.slice(0, 2)) {
        await page.fill(INPUT, text);
        await settle(page);
        await composeAt(page, cdp, text.length, READING);
        await settle(page);
        assert.ok((await fieldOf(page)) === text + READING, `前提: 欄は確定済み＋変換中 ${SIM_LABEL}`);
        await reload(page);
        await assertRestoredOnly(page, name, text, { hooks: false });
      }
      assertClean(rec);
      await context.close();
    });
  });

  test(`DI-6b 配信する設定: 拒否が欄に残った後に変換して再読み込みしても本文だけが戻る（CSP ${cspLabel}）`, async () => {
    await runAuthor(async (srv) => {
      const { page, context, cdp, rec } = await openAuthor(srv);
      await page.fill(INPUT, REJECTED);
      await page.click(SUBMIT);
      await page.waitForSelector(REJECTED_LI, { timeout: T });
      await settle(page);
      await composeAt(page, cdp, REJECTED.length, READING);
      await settle(page);
      await reload(page);
      await assertRestoredOnly(page, '拒否の後', REJECTED, { hooks: false });
      await page.waitForSelector(REJECTED_LI, { timeout: T });
      assertClean(rec);
      await context.close();
    });
  });

  test(`DI-6c 配信する設定: 保存の遅延の前でも読みは入らず、確定させた対照では戻る（CSP ${cspLabel}）`, async (t) => {
    await runAuthor(async (srv) => {
      const a = await pagehideCase(() => openAuthor(srv), 'DI-6c', { commit: false });
      t.diagnostic(`DI-6c 試した回数: ${a.tries}`);
      assert.ok(a.field === a.expected, '確定済みの文が戻る');
      assert.equal(a.readingInStore, false, `保存先に読みが無い ${SIM_LABEL}`);
      const b = await pagehideCase(() => openAuthor(srv), 'DI-6c 対照', { commit: true });
      assert.ok(b.field === b.expected, '対照: 確定した読みは戻る');
      assert.equal(b.readingInStore, true, '対照: 確定した読みは保存先にある');
    });
  });
}
