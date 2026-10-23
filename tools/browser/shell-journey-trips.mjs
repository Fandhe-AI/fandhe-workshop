// 枠の通し（shell-journey.mjs の runShellJourney）の途中へ挟む操作と、その前後の比較の補助モジュール（TASK-548 2/2。
// SHELL-1・SHELL-7・SHELL-10・SHELL-13・SHELL-15）。
//
// 役割: shell-journey.test.mjs が `makeTripHooks` で作った `afterStep` を `runShellJourney` の hooks に渡す。
//   通しの誘導（ch0-3-journey.mjs の journey）は直線で途中から再開できず、拒否・問い返しの待ちはログの累積件数との完全一致で行う。
//   そのため挟む操作はすべて「拒否・問い返しを増やさず、終わったときに同じ Step・同じ状態へ戻る」ものに限る。
//   挟む操作は 4 区間: checkpoint（保存→復帰）・reload（再読み込み）・export-import（書き出し→別のオリジンでの読み込み）・
//   save-failure（localStorage の容量超過の表示）。区間の位置と比較の定義は docs/testing.md 5.270。
// 入力: すべて実際のボタン・ファイル入力・再読み込みで行う。試験用の口（raw-trigger）は使わない。観察だけ `__fandheTest.stateJson()` を読む。
// 出力: 区間ごとの [合格, 対象] と台帳 L への積み込みだけ。履歴・下書き・保存値の中身は出さない（真偽・件数・欄の名前・長さだけ）。
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {
  engine, at, LOG, ports,
} from './ch0-3-journey.mjs';
import {
  openPage, waitMounted, summarize, collectSafety, nonZero,
} from './harness.mjs';
import {
  expectedPosition, LEFT, TITLE, DO, POSITION,
} from './shell-journey.mjs';

export const SAVE = '[data-testid="checkpoint-save"]';
export const RESTORE = '[data-testid="checkpoint-restore"]';
const EXPORT_BTN = '[data-testid="shell-export"]';
const LINK = '[data-testid="export-download"]';
const FILE_INPUT = '[data-testid="shell-import"] input[type="file"]';
const NOTICE = '[data-testid="shell-notice"]';
const FAILED_ITEM = '[data-testid="shell-notice-item"][data-notice-kind="save-failed"]';
const NOTICE_TEXT = '[data-testid="shell-notice-text"]';
export const HIDE_IDB_FLAG = '__shellJourneyHideIdb';
const PAD_PREFIX = 'pad/';
const T = 30000;

export const SECTIONS = ['checkpoint', 'reload', 'export-import', 'save-failure'];
export const newTrips = () => Object.fromEntries(SECTIONS.map((s) => [s, [0, 0]]));

// IndexedDB を隠す初期化スクリプト。目印（sessionStorage）が立っているときだけ効くので、区間の外の保存先は変えない。
export async function addHideIdbInitScript(context) {
  await context.addInitScript((flag) => {
    try {
      if (window.sessionStorage.getItem(flag) === '1') {
        Object.defineProperty(window, 'indexedDB', { get: () => undefined, configurable: true });
      }
    } catch { /* sessionStorage が使えない場合は隠さない */ }
  }, HIDE_IDB_FLAG);
}

// ---- 観察 ----

// 画面の層。値のまま返し、比べるときは一致の真偽と欄の名前だけを使う。差分の印（mark）は表示の状態で揺れるので含めない。
const screenOf = (page) => page.evaluate(([left, title, dos, pos, log, restore]) => {
  const text = (sel) => [...document.querySelectorAll(sel)].map((e) => e.textContent);
  return {
    left: [...document.querySelector(left).querySelectorAll('[data-row]')]
      .map((e) => {
        // 差分の印（記号・文字）は直前の世代との差で決まる表示の状態で、復帰・読み込みで変わりうる（restore-cancel と同じ扱い）。
        const c = e.cloneNode(true);
        c.querySelectorAll('.mk-glyph, .mk-text').forEach((m) => m.remove());
        return `${e.getAttribute('data-row')}\u0000${c.textContent}`;
      }).join('\u0001'),
    position: text(pos),
    title: text(title),
    do: text(dos),
    list: [...document.querySelectorAll(restore)].map((b) => [
      b.getAttribute('data-checkpoint'), b.getAttribute('data-checkpoint-chapter'),
      b.getAttribute('data-checkpoint-m'), b.getAttribute('data-checkpoint-k'), b.textContent,
    ]),
    logKinds: [...document.querySelectorAll(`${log} li`)].map((li) => li.getAttribute('data-kind')),
  };
}, [LEFT, TITLE, DO, POSITION, LOG, RESTORE]);

export async function snapshotOf(page) {
  const e = await engine(page);
  return { e, screen: await screenOf(page), ports: await ports(page) };
}

const j = (v) => JSON.stringify(v);
const withoutGeneration = (e) => {
  const run = { ...e.stateWithoutLog.run };
  delete run.generation;
  return j({ ...e.stateWithoutLog, run });
};
// 差分の印（mark）を除いた JSON。印は直前の世代との差で決まる表示の状態なので、復帰の前後では比べない。
const noMark = (v) => JSON.stringify(v, (k, x) => (k === 'mark' ? undefined : x));
const generationOf = (e) => e.stateWithoutLog.run.generation;
const countKind = (log, kind) => log.filter((r) => r.kind === kind).length;

// 再読み込みの前後の比較。違った欄の名前の配列を返す（空なら一致）。
export function diffReload(a, b) {
  const d = [];
  if (a.e.current !== b.e.current) d.push('current');
  if (withoutGeneration(a.e) !== withoutGeneration(b.e)) d.push('state');
  if (j(a.e.artifacts) !== j(b.e.artifacts)) d.push('artifacts');
  if (j(a.e.review) !== j(b.e.review)) d.push('review');
  if (generationOf(b.e) !== generationOf(a.e) + 1) d.push('generation');
  const n = a.e.log.length;
  if (b.e.log.length !== n + 1 || b.e.log[n]?.kind !== 'reloaded' || j(b.e.log.slice(0, n)) !== j(a.e.log)) d.push('log');
  for (const k of ['left', 'position', 'title', 'do']) if (j(a.screen[k]) !== j(b.screen[k])) d.push(`screen.${k}`);
  if (j(a.screen.list) !== j(b.screen.list)) d.push('screen.list');
  if (j(a.ports) !== j(b.ports)) d.push('ports');
  const m = a.screen.logKinds.length;
  if (b.screen.logKinds.length !== m + 1 || j(b.screen.logKinds.slice(0, m)) !== j(a.screen.logKinds)) d.push('screen.log');
  return d;
}

// 復帰の前後の比較。`saved` は保存した時点の観察、`now` は復帰の後の観察、`listBefore` は復帰の直前の一覧。
export function diffRestore(saved, now, listBefore, genBefore, restoredBefore) {
  const d = [];
  if (saved.e.current !== now.e.current) d.push('current');
  if (j(saved.e.stateWithoutLog.run.world) !== j(now.e.stateWithoutLog.run.world)) d.push('world');
  if (j(saved.e.stateWithoutLog.run.slices) !== j(now.e.stateWithoutLog.run.slices)) d.push('slices');
  if (noMark(saved.e.artifacts) !== noMark(now.e.artifacts)) d.push('artifacts');
  for (const k of ['left', 'position', 'title', 'do']) if (j(saved.screen[k]) !== j(now.screen[k])) d.push(`screen.${k}`);
  if (j(saved.ports) !== j(now.ports)) d.push('ports');
  if (generationOf(now.e) <= genBefore) d.push('generation');
  if (countKind(now.e.log, 'restored') !== restoredBefore + 1) d.push('restored');
  if (j(now.screen.list) !== j(listBefore)) d.push('screen.list');
  return d;
}

// 書き出し→読み込みの比較。読み込みの結果（lastImport）・組み込みの回数（installCount）・起動の種類（bootKind。
// 再読み込みを経た側は restored、読み込んだ側は imported になる）は、受講の状態ではなく起動の経緯の記録なので除く。
export function diffImport(a, b) {
  const strip = (e) => {
    const {
      lastImport, installCount, bootKind, ...rest
    } = e;
    return rest;
  };
  const d = [];
  const ea = strip(a.e);
  const eb = strip(b.e);
  for (const k of new Set([...Object.keys(ea), ...Object.keys(eb)])) if (j(ea[k]) !== j(eb[k])) d.push(`engine.${k}`);
  for (const k of Object.keys(a.screen)) if (j(a.screen[k]) !== j(b.screen[k])) d.push(`screen.${k}`);
  if (j(a.ports) !== j(b.ports)) d.push('ports');
  return d;
}

// ---- 操作 ----

const waitList = (page, n) => page.waitForFunction(
  ([sel, k]) => document.querySelectorAll(sel).length === k, [RESTORE, n], { timeout: T },
);
const settle = (page) => page.waitForFunction(() => window.__fandheTest.nextTick() === 'null', null, { timeout: T });

export async function saveCheckpoint(page, n) {
  await page.click(SAVE);
  await waitList(page, n);
  await settle(page);
}

export async function restoreTo(page, id, uid) {
  await page.click(`${RESTORE}[data-checkpoint="${id}"]`);
  await at(page, uid, 'entered');
}

export async function reloadAndWait(page, uid, status) {
  await page.reload();
  await waitMounted(page);
  await at(page, uid, status);
}

let fileSeq = 0;
// 画面の操作だけで書き出し、一時ディレクトリへ保存したファイルの経路を返す。ファイル名は試験側で作る。
export async function exportByUi(page, tmpDir) {
  await page.locator(EXPORT_BTN).click();
  const link = page.locator(LINK);
  await link.waitFor({ state: 'visible', timeout: 10000 });
  const [download] = await Promise.all([page.waitForEvent('download'), link.click()]);
  fileSeq += 1;
  const file = path.join(tmpDir, `trip-${process.pid}-${fileSeq}.json`);
  await download.saveAs(file);
  return file;
}

const failedItems = (page) => page.locator(FAILED_ITEM).count();
const waitFailed = (page, n) => page.waitForFunction(
  ([sel, k]) => document.querySelectorAll(sel).length === k, [FAILED_ITEM, n], { timeout: T },
);

// 履歴の鍵ではない値（pad/ 以外）の長さと簡易 digest。値そのものは返さない。
const savedDigest = (page) => page.evaluate((prefix) => {
  let h = 0;
  let len = 0;
  for (const k of Object.keys(localStorage).filter((x) => !x.startsWith(prefix)).sort()) {
    const v = localStorage.getItem(k) ?? '';
    len += v.length;
    for (let i = 0; i < v.length; i += 1) h = (h * 31 + v.charCodeAt(i)) | 0;
  }
  return `${len}:${h}`;
}, PAD_PREFIX);

const fillStorage = (page) => page.evaluate((prefix) => {
  let n = 0;
  let size = 1 << 20;
  while (size >= 1) {
    try {
      localStorage.setItem(`${prefix}${n}`, 'x'.repeat(size));
      n += 1;
    } catch {
      size >>= 1;
    }
  }
  return n;
}, PAD_PREFIX);

const clearPad = (page) => page.evaluate((prefix) => {
  Object.keys(localStorage).filter((k) => k.startsWith(prefix)).forEach((k) => localStorage.removeItem(k));
}, PAD_PREFIX);

const setFlag = (page, on) => page.evaluate(([flag, v]) => {
  if (v) sessionStorage.setItem(flag, '1');
  else sessionStorage.removeItem(flag);
}, [HIDE_IDB_FLAG, on]);

const noticeVisible = (page) => page.evaluate(([area, item]) => {
  const e = document.querySelector(item) ?? document.querySelector(area);
  if (!e) return false;
  const r = e.getBoundingClientRect();
  const c = getComputedStyle(e);
  return r.width > 0 && r.height > 0 && c.display !== 'none' && c.visibility !== 'hidden';
}, [NOTICE, FAILED_ITEM]);

// ---- 挟む操作の束 ----

// `afterStep`（runShellJourney の hooks）・`finish`（通しの後）・`controls`（対照用の観察）・`trips`（区間ごとの [合格, 対象]）を返す。
// `openB` は別のオリジンの新しい context を開く関数（{ page, context, rec }）を返す。
export function makeTripHooks({ openB, tmpDir, scene, L, trips = newTrips() }) {
  const ck = (section, name, ok, diff = []) => {
    trips[section][1] += 1;
    if (ok) trips[section][0] += 1;
    L.check(scene, ok || diff.length === 0 ? `${section} ${name}` : `${section} ${name}: ${diff.join(',')}`, ok);
  };
  const controls = {};
  const mem = {};

  const reloadTrip = async (page, uid, status, label) => {
    const before = await snapshotOf(page);
    await reloadAndWait(page, uid, status);
    const after = await snapshotOf(page);
    const d = diffReload(before, after);
    ck('reload', `${label} 同じ Step・状態・画面で世代 +1`, d.length === 0, d);
    return { before, after };
  };

  const saveTrip = async (page, uid, n) => {
    const prevIds = (await screenOf(page)).list.map((x) => x[0]);
    await saveCheckpoint(page, n);
    const list = (await screenOf(page)).list;
    const fresh = list.filter((x) => !prevIds.includes(x[0]));
    const pos = expectedPosition(uid);
    ck('checkpoint', `${uid} を保存して一覧が ${n} 件`, list.length === n && fresh.length === 1);
    const item = fresh[0] ?? [];
    ck('checkpoint', `${uid} の項目の章・M・K が IR の値`,
      item[1] === pos.chapter && item[2] === String(pos.m) && item[3] === String(pos.k));
    return item[0];
  };

  const restoreTrip = async (page, id, uid, saved, label) => {
    const listBefore = (await screenOf(page)).list;
    const cur = await engine(page);
    const gen = generationOf(cur);
    const restoredBefore = countKind(cur.log, 'restored');
    await restoreTo(page, id, uid);
    await settle(page);
    const now = await snapshotOf(page);
    const d = diffRestore(saved, now, listBefore, gen, restoredBefore);
    ck('checkpoint', `${label} へ復帰して保存の時点と同じ`, d.length === 0, d);
    return now;
  };

  const afterStep = async (uid, status, { page }) => {
    const key = `${uid}/${status}`;
    if (key === 'main/st-0-2b/entered') {
      mem.k1 = await saveTrip(page, uid, 1);
    } else if (key === 'main/st-1-1/entered') {
      await reloadTrip(page, uid, 'entered', '章の境目の直後');
    } else if (key === 'main/st-1-4/entered') {
      mem.k2 = await saveTrip(page, uid, 2);
      mem.s2 = await snapshotOf(page);
    } else if (key === 'main/st-2-1/entered') {
      mem.sNow = await snapshotOf(page);
      mem.k3 = await saveTrip(page, uid, 3);
      const list = (await screenOf(page)).list;
      const back = await restoreTrip(page, mem.k2, 'main/st-1-4', mem.s2, '前の章の保存');
      const fwd = await restoreTrip(page, mem.k3, uid, mem.sNow, '現在の保存');
      controls.s2 = mem.s2;
      controls.sNow = mem.sNow;
      controls.back = back;
      const { after } = await reloadTrip(page, uid, 'entered', '復帰の後');
      ck('checkpoint', '復帰と再読み込みの後も一覧が 3 件で一致',
        j((await screenOf(page)).list) === j(list) && j(after.screen.list) === j(list) && fwd.screen.list.length === 3);
    } else if (key === 'main/st-2-2/entered') {
      await setFlag(page, true);
      await reloadTrip(page, uid, 'entered', '保存の失敗の区間の開始');
      mem.pad = await fillStorage(page);
      mem.digest0 = await savedDigest(page);
      ck('save-failure', '保存領域を満たした', mem.pad > 0);
    } else if (key === 'main/st-2-2/accepted') {
      await waitFailed(page, 1);
      const text = await page.evaluate(([i, t]) => document.querySelector(i)?.querySelector(t)?.textContent ?? '', [FAILED_ITEM, NOTICE_TEXT]);
      ck('save-failure', '失敗の通知が 1 件で QuotaExceededError の理由つき', (await failedItems(page)) === 1 && text.includes('QuotaExceededError'));
      ck('save-failure', '失敗の通知が見える', await noticeVisible(page));
      const e = await engine(page);
      ck('save-failure', '受講の状態は進んでいる', e.current === uid && e.stateWithoutLog.run.slices.main?.status?.[uid] === 'accepted');
      ck('save-failure', '失敗の間、保存値は区間の開始のまま', (await savedDigest(page)) === mem.digest0);
      await clearPad(page);
    } else if (key === 'main/st-2-3/entered') {
      await waitFailed(page, 0);
      ck('save-failure', '次の保存の成功で通知が消えた', (await failedItems(page)) === 0);
      ck('save-failure', '保存値が更新された', (await savedDigest(page)) !== mem.digest0);
      await setFlag(page, false);
      await reloadTrip(page, uid, 'entered', '保存の失敗の区間の終了');
    } else if (key === 'main/st-3-1/entered') {
      await exportImportTrip(page, uid);
    } else if (key === 'main/st-3-2/accepted') {
      await reloadTrip(page, uid, 'accepted', '確認待ち');
    }
  };

  async function exportImportTrip(page, uid) {
    const file = await exportByUi(page, tmpDir);
    const a = await snapshotOf(page);
    const { page: pb, context, rec } = await openB();
    try {
      const fresh = await snapshotOf(pb);
      ck('export-import', '読み込みの前の別のオリジンは書き出し側と違う', diffImport(a, fresh).length > 0);
      await pb.setInputFiles(FILE_INPUT, file);
      await pb.waitForFunction(() => {
        const e = JSON.parse(window.__fandheTest.stateJson()).engine;
        return e && e.lastImport && e.lastImport.verdict === 'open';
      }, null, { timeout: 15000 });
      await settle(pb);
      const b = await snapshotOf(pb);
      const d = diffImport(a, b);
      ck('export-import', '別のオリジンで読み込むと同じ Step・状態・画面', d.length === 0, d);
      const before = await snapshotOf(pb);
      await reloadAndWait(pb, uid, 'entered');
      const dr = diffReload(before, await snapshotOf(pb));
      ck('export-import', '読み込んだ側の再読み込みの後も同じ', dr.length === 0, dr);
      const s = summarize(rec);
      ck('export-import', '読み込んだ側の通信・エラー・CSP 違反・ダイアログ・ポップアップが 0 件',
        s.external === 0 && s.pageErrors === 0 && s.csp === 0 && s.dialogs === 0 && s.popups === 0);
      ck('export-import', '読み込んだ側の安全性の計数が全区分 0', nonZero(await collectSafety(pb, rec)).length === 0);
    } finally {
      await context.close();
      fs.rmSync(file, { force: true });
    }
  }

  // 通しの後。達成のまま再読み込みしても各 Step の状態が一致する。
  const finish = async (page, lastUid) => {
    await reloadTrip(page, lastUid, 'achieved', '通しの後');
  };

  return {
    afterStep, finish, controls, trips,
  };
}

// 4 区間の合格数＝対象数で、対象が 1 以上。
export const tripsOk = (trips) => SECTIONS.every((s) => trips[s][1] > 0 && trips[s][0] === trips[s][1]);

// 対照: 比較の関数が「違う」と言うこと、失敗の検出が区間の外で 0 件であること（空振りの防止）。
export async function runTripControls(page, controls) {
  assert.ok(controls.s2 && controls.sNow, 'control: snapshots exist');
  assert.notDeepEqual(diffReload(controls.s2, controls.sNow), [], 'control: different steps must differ for reload');
  assert.notDeepEqual(diffRestore(controls.s2, controls.sNow, controls.s2.screen.list, 0, 0), [], 'control: different steps must differ for restore');
  assert.notDeepEqual(diffImport(controls.s2, controls.sNow), [], 'control: different steps must differ for import');
  assert.equal(await failedItems(page), 0, 'control: no save-failed item outside the section');
}
