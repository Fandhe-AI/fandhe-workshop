// 上部のチェックポイントの一覧と、再読み込み・書き出し・読み込みをまたぐ一致の試験（SHELL-13。TASK-529 2/2）。
//
// 試験用の教材 `ch0-3`（`?fixture=ch0-3-chapters`）で、実際の保存ボタンの click だけで 3 件のチェックポイントを作る（`raw-trigger` は使わない）。
//   場面 S  章をまたぐ 3 件（うち 1 件は復帰の後の保存）の各項目に、保存した時点の章・M・K が文字と属性で出る。期待値は IR から独自に数える
//   場面 R  再読み込みを 2 回しても、一覧（ID・属性・文字・順）が前と完全に一致する
//   場面 E  画面の操作で書き出し、名前の違うサブパス（別のオリジン）の別の context で読み込むと、一覧が書き出し前と一致する。読み込んだ側の再読み込みの後も一致する
//   場面 F  読み込んだ側で復帰ボタンを押すと、現在の Step・上部の位置・左の行が、書き出した側で同じ復帰をした結果と一致する
//   場面 N  負の対照: 保存をもう 1 回押すと一覧が 1 件増え、比べる関数が「同じ」と言わない
//   場面 W  幅 360px で 3 件を表示しても横スクロールが無い
// 一覧はページ 0（500 件以下）の範囲で比べる。ページ送りの位置は表示の状態で、再読み込みで 0 に戻る（docs/testing.md 5.267 の限界）。
// 試験の出力・失敗の文には、一致の真偽と件数だけを出し、履歴の中身は出さない。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  ledger, mouseDriver, journey, engine,
} from './ch0-3-journey.mjs';

const BASE_A = '/fandhe-checkpoint-list/';
const BASE_B = '/another-repo-name/';
const QUERY = '?fixture=ch0-3-chapters';
const SAVE = '[data-testid="checkpoint-save"]';
const RESTORE = '[data-testid="checkpoint-restore"]';
const POSITION = '[data-testid="shell-position"]';
const ROWS = '[data-testid="part-artifact-view"] [data-row]';
const EXPORT_BTN = '[data-testid="shell-export"]';
const LINK = '[data-testid="export-download"]';
const FILE_INPUT = '[data-testid="shell-import"] input[type="file"]';
let browser;
let tmpDir;

const IR = JSON.parse(fs.readFileSync(
  path.join(path.dirname(fileURLToPath(import.meta.url)), '../../crates/web/tests/data/ch0_3/ir.json'),
  'utf8',
));

// IR から独自に数えた（章の ID, M, K）。
function expectedPlace(uid) {
  const node = IR.nodes.find((n) => n.uid === uid);
  const list = IR.nodes.filter((n) => n.scope === 'main' && n.unit === node.unit);
  return [node.unit, String(list.findIndex((n) => n.uid === uid) + 1), String(list.length)];
}

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-checkpoint-list-'));
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

// 一覧を DOM の順に読む。各項目は [ID, 章, M, K, 文字]。
const listOf = (page) => page.evaluate((sel) => [...document.querySelectorAll(sel)].map((b) => [
  b.getAttribute('data-checkpoint'), b.getAttribute('data-checkpoint-chapter'),
  b.getAttribute('data-checkpoint-m'), b.getAttribute('data-checkpoint-k'), b.textContent,
]), RESTORE);

const sameList = (a, b) => JSON.stringify(a) === JSON.stringify(b);

// 左の行の列と上部の位置の文字。
const observe = (page) => page.evaluate(([rows, pos]) => ({
  rows: [...document.querySelectorAll(rows)].map((r) => [r.getAttribute('data-row'), r.textContent]),
  position: document.querySelector(pos)?.textContent ?? null,
}), [ROWS, POSITION]);

const waitList = (page, n) => page.waitForFunction(
  ([sel, k]) => document.querySelectorAll(sel).length === k, [RESTORE, n], { timeout: 15000 },
);

const waitCurrent = (page, uid) => page.waitForFunction(
  (u) => JSON.parse(window.__fandheTest.stateJson()).engine.current === u, uid, { timeout: 15000 },
);

// 手動の保存を押し、増えた 1 件の ID を返す。章の終わりの自動保存（SAVE-31）も一覧に入るため、件数は固定にできない。
async function clickSave(page) {
  const before = (await listOf(page)).map((e) => e[0]);
  await page.click(SAVE);
  await waitList(page, before.length + 1);
  const fresh = (await listOf(page)).map((e) => e[0]).filter((i) => !before.includes(i));
  if (fresh.length !== 1) throw new Error('手動の保存で増えた項目が 1 件でない');
  return fresh[0];
}

async function waitImport(page) {
  await page.waitForFunction(() => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    return e && e.lastImport && e.lastImport.verdict === 'open';
  }, null, { timeout: 15000 });
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null', null, { timeout: 15000 });
}

async function exportByUi(page) {
  await page.locator(EXPORT_BTN).click();
  const link = page.locator(LINK);
  await link.waitFor({ state: 'visible', timeout: 10000 });
  const [download] = await Promise.all([page.waitForEvent('download'), link.click()]);
  const file = path.join(tmpDir, `${Date.now()}-${Math.random().toString(16).slice(2)}.json`);
  await download.saveAs(file);
  return file;
}

// 章をまたぐ 2 件を実ボタンで保存し、st-1-4 から CH-0 の保存へ復帰して 3 件目を保存する。
async function makeList(page, L) {
  const saved = new Set();
  const ids = [];
  const onStep = async (uid, status) => {
    if (status === 'entered' && (uid === 'main/st-0-2b' || uid === 'main/st-1-4') && !saved.has(uid)) {
      saved.add(uid);
      ids.push(await clickSave(page));
    }
  };
  await journey(page, mouseDriver(page), 'S', L, 'main/st-1-4', { chaptered: true, onStep });
  await page.click(`${RESTORE}[data-checkpoint="${ids[0]}"]`);
  await waitCurrent(page, 'main/st-0-2b');
  ids.push(await clickSave(page));
  return ids;
}

async function scene(csp, fn) {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withCspMode({ dir, csp }, (d) => withServer({ dir: d, base: BASE_A }, (srvA) => withServer({ dir: d, base: BASE_B }, async (srvB) => {
    const L = ledger();
    const opened = [];
    const open = async (srv, viewport = { width: 1280, height: 800 }) => {
      const o = await openPage(browser, { origin: srv.origin, viewport });
      await o.page.goto(`${srv.url}${QUERY}`);
      await waitMounted(o.page);
      opened.push(o);
      return o;
    };
    await fn({ L, open, srvA, srvB });
    for (const o of opened) {
      const s = summarize(o.rec);
      L.check('共通', 'ページ内のエラーが 0 件', s.pageErrors === 0);
      L.check('共通', 'CSP 違反が 0 件', s.csp === 0);
      assert.deepEqual(nonZero(await collectSafety(o.page, o.rec)), [], 'safety counters are zero');
      await o.context.close();
    }
    console.log(`チェックポイントの一覧 (CSP ${csp ? 'あり' : 'なし'}): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), []);
  })));
}

for (const csp of [true, false]) {
  test(`一覧が再読み込みと書き出し→別のオリジンでの読み込みの前後で一致し、読み込んだ側で復帰できる（CSP ${csp ? 'あり' : 'なし'}）`, { timeout: 600000 }, async () => {
    await scene(csp, async ({ L, open, srvA, srvB }) => {
      const A = await open(srvA);
      const ids = await makeList(A.page, L);
      const base = await listOf(A.page);
      const total = base.length;
      const manual = ids.map((id) => base.find((e) => e[0] === id));

      // 場面 S（手動の 3 件に絞る。自動保存の項目は一覧に混ざる）
      L.check('S', '手動の 3 件が一覧にある', manual.length === 3 && manual.every(Boolean) && total >= 3);
      const uids = ['main/st-0-2b', 'main/st-1-4', 'main/st-0-2b'];
      L.check('S', '各項目の章・M・K が IR から数えた値', uids.every((u, i) => sameList(manual[i].slice(1, 4), expectedPlace(u))));
      L.check('S', '各項目の文字に位置が出る', base.every((e) => e[4].includes(`Step ${e[2]} / ${e[3]}`) && e[4].includes(e[1])));
      L.check('S', '章をまたぐ', new Set(manual.map((e) => e[1])).size === 2);

      // 場面 R
      for (let i = 1; i <= 2; i += 1) {
        await A.page.reload();
        await waitMounted(A.page);
        await waitList(A.page, total);
        L.check('R', `${i} 回目の再読み込みの後に一覧が一致`, sameList(await listOf(A.page), base));
      }

      // 場面 E
      const file = await exportByUi(A.page);
      const B = await open(srvB);
      await B.page.setInputFiles(FILE_INPUT, file);
      await waitImport(B.page);
      await waitList(B.page, total);
      L.check('E', '読み込んだ側の一覧が書き出し前と一致', sameList(await listOf(B.page), base));
      await B.page.reload();
      await waitMounted(B.page);
      await waitList(B.page, total);
      L.check('E', '読み込んだ側の再読み込みの後も一致', sameList(await listOf(B.page), base));

      // 場面 F: 同じ復帰を両側で行い、結果が一致する。先頭の項目は CH-0 の保存
      for (const p of [A.page, B.page]) {
        await p.click(`${RESTORE}[data-checkpoint="${manual[1][0]}"]`);
        await waitCurrent(p, 'main/st-1-4');
        await p.click(`${RESTORE}[data-checkpoint="${manual[0][0]}"]`);
        await waitCurrent(p, 'main/st-0-2b');
      }
      const oa = await observe(A.page);
      const ob = await observe(B.page);
      L.check('F', '現在の Step が保存の時点', (await engine(B.page)).current === 'main/st-0-2b');
      L.check('F', '上部の位置が保存の時点', ob.position === `${manual[0][1]}　Step ${manual[0][2]} / ${manual[0][3]}`);
      L.check('F', '左の行と位置が書き出した側と一致', sameList(oa, ob));
      L.check('F', '復帰しても一覧が変わらない', sameList(await listOf(B.page), base));

      // 場面 N
      await clickSave(B.page);
      const grown = await listOf(B.page);
      L.check('N', '保存で一覧が 1 件増える', grown.length === total + 1);
      L.check('N', '比べる関数が異なる一覧を同じと言わない', !sameList(grown, base) && sameList(grown, grown));
    });
  });
}

test('幅 360px で 3 件の一覧を表示しても横スクロールが無い', { timeout: 300000 }, async () => {
  await scene(true, async ({ L, open, srvA }) => {
    const A = await open(srvA);
    const ids = await makeList(A.page, L);
    await A.page.setViewportSize({ width: 360, height: 800 });
    const m = await A.page.evaluate(() => ({
      scroll: document.documentElement.scrollWidth <= document.documentElement.clientWidth,
    }));
    L.check('W', '手動の 3 件が一覧にある', (await listOf(A.page)).length >= ids.length);
    L.check('W', '横スクロールが無い', m.scroll);
  });
});
