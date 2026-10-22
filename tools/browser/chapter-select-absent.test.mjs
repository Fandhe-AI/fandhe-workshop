// 章の飛ばし・選択を受講者の画面に出さないことの試験（TASK-528。SHELL-11・D-73）。
//
// D-73（2026-10-22）は「章を飛ばす・選ぶ操作を受講者の画面に出さない」と決めた。章をまたぐ操作は SHELL-10 の「次の章へ進む」の 1 つだけで、
// 特定の章から始める手段は直接リンク `?start=`（DLV-9。direct-link-start・direct-link-reload が証拠）である。製品のコードは変えず、観察を固定する。
//
// 確かめること（CSP あり・なしの両方。1 回ごとに新しい画面を開く）:
//   (a) `?fixture=ch0-3-chapters` の通し（実クリック）: 全 Step で章を選ぶ部品・開始点へのリンク・開始点の ID を持つ要素・「移動」ボタン・次の章へ進むが 0 件。
//       章の境目（各章の完了の直後）だけは、次の章へ進むがちょうど 1 個で、開始点の ID を持つ要素が直後の章の先頭 1 件だけ（飛ばさない）。最後の章の後は 0 件
//   (b) `?fixture=ch0-3-links`（開始点 10 件。章の最後は final でない）: 初期と途中の各 Step で、検出器のすべてが 0 件
//   (c) 検出器の対照: 画面の DOM へ選ぶ部品・開始点の ID を持つ要素・開始点へのリンクを足すと、検出器が 0 でなくなる
//   安全の計数・配信元の外への通信・エンジンの警告と無視が 0 件
//
// 確かめないこと: 配信する設定での章の境目（配信する設定は実行時に IR を取得しない）。試験の出力に入力値・題・開始点の ID は出さない。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  NEXT, engine, ledger, mouseDriver, journey, install, waitCount,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-chapter-select-absent/';
const CHAPTERS = '?fixture=ch0-3-chapters';
const LINKS = '?fixture=ch0-3-links';
// 開始点の ID の表。`src/test_fixture.rs` の `CH03_CHAPTER_POINTS`・`CH03_LINK_POINTS` と揃えて保つ。
const CHAPTER_IDS = ['s-0-1', 's-1-1', 's-2-1', 's-3-1'];
const LINK_IDS = ['s-0-1', 's-0-2b', 's-1-1', 's-1-4', 's-1-5', 's-1-6', 's-2-1', 's-2-3', 's-3-1', 's-3-2'];
const AFTER_CHAPTER = ['s-1-1', 's-2-1', 's-3-1'];
let browser;

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

async function scenario(csp, name, query, fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    const L = ledger();
    await page.goto(`${srv.url}${query}`);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    await install(page);
    await fn(page, rec, L);
    console.log(`${name} (CSP ${csp ? 'あり' : 'なし'}): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  }));
}

async function safety(page, rec, scene, L) {
  const e = await engine(page);
  L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
  L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
  const counts = await collectSafety(page, rec);
  for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
  L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
  L.check(scene, '配信元の外への通信が 0 件', rec.external.length === 0);
}

// 検出器。章を選ぶ・飛ばす部品らしいものを 1 か所で数える。OS を選ぶ欄（TERM-38）は章の選択ではないので除く。
// `payloadStarts` は `data-payload` の値のうち開始点の ID に一致するもの（重複を含む）。
function chapterSelectProbe(page, ids) {
  return page.evaluate(([next, startIds]) => {
    const os = document.querySelector('[data-testid="os-select"]');
    const inOs = (e) => os !== null && (e === os || os.contains(e));
    const roles = '[role="listbox"],[role="menu"],[role="menuitem"],[role="option"],[role="combobox"]';
    const selectLike = [...document.querySelectorAll(`select,option,${roles}`)].filter((e) => !inOs(e)).length;
    return {
      selectLike,
      startLinks: document.querySelectorAll('a[href*="start="]').length,
      next: document.querySelectorAll(next).length,
      moves: [...document.querySelectorAll('button')].filter((b) => (b.textContent ?? '').startsWith('移動')).length,
      payloadStarts: [...document.querySelectorAll('[data-payload]')]
        .map((e) => e.getAttribute('data-payload')).filter((p) => startIds.includes(p)),
    };
  }, [NEXT, ids]);
}

// 全時点の不変条件。`offered` は提示中の開始点（無ければ null）。
async function checkAbsent(page, ids, scene, L, label, offered = null) {
  const p = await chapterSelectProbe(page, ids);
  L.check(scene, `${label}: 章を選ぶ部品が 0 件`, p.selectLike === 0);
  L.check(scene, `${label}: 開始点へのリンクが 0 件`, p.startLinks === 0);
  L.check(scene, `${label}: 移動ボタンが 0 件`, p.moves === 0);
  L.check(scene, `${label}: 次の章へ進むが ${offered === null ? 0 : 1} 個`, p.next === (offered === null ? 0 : 1));
  const ok = offered === null ? p.payloadStarts.length === 0
    : p.payloadStarts.length === 1 && p.payloadStarts[0] === offered;
  L.check(scene, `${label}: 開始点の ID を持つ要素が ${offered === null ? 0 : '提示中の 1'} 件`, ok);
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`章の通しの全時点で選ぶ部品が 0 件で、境目では直後の章の 1 件だけが出る（${mode}）`, { timeout: 300000 }, async () => {
    await scenario(csp, 'chapters', CHAPTERS, async (page, rec, L) => {
      const scene = '章の通し';
      await checkAbsent(page, CHAPTER_IDS, scene, L, '初期');
      const d = mouseDriver(page);
      let crossed = 0;
      // 章の境目（押す直前）で、提示中の 1 件だけであることを確かめる。journey 側は変えない。
      const driver = {
        ...d,
        nextChapter: async () => {
          await checkAbsent(page, CHAPTER_IDS, scene, L, `章 ${crossed} の完了の直後`, AFTER_CHAPTER[crossed]);
          crossed += 1;
          return d.nextChapter();
        },
      };
      let steps = 0;
      await journey(page, driver, scene, L, null, {
        chaptered: true,
        onStep: async (uid, kind) => {
          steps += 1;
          await checkAbsent(page, CHAPTER_IDS, scene, L, `Step ${steps}（${kind}）`);
        },
      });
      L.check(scene, '章の境目が 3 回', crossed === 3);
      L.check(scene, 'Step の確認が 20 件以上', steps >= 20);
      await waitCount(page, NEXT, 0);
      await checkAbsent(page, CHAPTER_IDS, scene, L, '最後の章の後');
      await safety(page, rec, scene, L);
    });
  });

  test(`開始点が 10 件の教材でも一覧が出ない（${mode}）`, { timeout: 300000 }, async () => {
    await scenario(csp, 'links', LINKS, async (page, rec, L) => {
      const scene = '開始点 10 件';
      await checkAbsent(page, LINK_IDS, scene, L, '初期');
      let steps = 0;
      await journey(page, mouseDriver(page), scene, L, 'main/st-1-4', {
        onStep: async (uid, kind) => {
          steps += 1;
          await checkAbsent(page, LINK_IDS, scene, L, `Step ${steps}（${kind}）`);
        },
      });
      L.check(scene, 'Step の確認が 1 件以上', steps >= 1);
      await safety(page, rec, scene, L);
    });
  });

  test(`検出器の対照: 足した部品を検出器が数える（${mode}）`, { timeout: 300000 }, async () => {
    await scenario(csp, 'control', CHAPTERS, async (page, rec, L) => {
      const scene = '検出器の対照';
      await checkAbsent(page, CHAPTER_IDS, scene, L, '足す前');
      await page.evaluate(() => {
        const host = document.body;
        const sel = document.createElement('select');
        sel.append(document.createElement('option'), document.createElement('option'));
        const a = document.createElement('a');
        a.setAttribute('href', '?start=s-2-1');
        const b = document.createElement('button');
        b.setAttribute('data-payload', 's-2-1');
        host.append(sel, a, b);
      });
      const p = await chapterSelectProbe(page, CHAPTER_IDS);
      L.check(scene, '選ぶ部品を数える', p.selectLike === 3);
      L.check(scene, '開始点へのリンクを数える', p.startLinks === 1);
      L.check(scene, '開始点の ID を持つ要素を数える', p.payloadStarts.length === 1);
    });
  });
}
