// L1・L2 の一致の試験（l1-l2-match.test.mjs。TASK-114 1/2）と固定の入力列の試験（l1-l2-fixed.test.mjs。TASK-114 2/2）が
// 共有する、配信先の起動・1 回の実行・木の照合の部品。
//
// 役割: 2 つの試験が「同じ配信先を同じやり方で起動し、同じ計数と合否で 1 回の実行を評価する」ための一か所の定義。
// 試験ではないため `.test.mjs` にしない（試験ファイルを import すると、試験が二重に登録されるため）。
// 呼び出し元は ctx = { browser, order, tmpDir, zip, siteDir } を渡す（試験の before で用意する）。
// 出力に履歴の本文・入力した値・DOM の文字を載せない。
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { withServer, openPage, waitMounted, summarize, cspConsoleMessages } from './harness.mjs';
import { ledger, journey, mouseDriver } from './ch0-3-journey.mjs';
import { runL2JourneyWith, domObserver, L2_STOP } from './l2-journey.mjs';
import { startL2, stopL2 } from './l2-server.mjs';
import { takeSnapshot, observeStep, sha256 } from './screen-snapshot.mjs';

export const BASES = { 'L1-a': '/fandhe-workshop/', 'L1-b': '/another-repo-name/' };
export const EXPORT_BTN = '[data-testid="shell-export"]';
export const LINK = '[data-testid="export-download"]';
export const IR_REL = 'ir/main.json';

/** ディレクトリの木を、相対パス（/ 区切り）から SHA-256 への Map にする。 */
export function treeHashes(dir) {
  const out = new Map();
  const walk = (d) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.isFile()) out.set(path.relative(dir, p).split(path.sep).join('/'), sha256(fs.readFileSync(p)));
    }
  };
  walk(dir);
  return out;
}

/** 2 つの木の差の件数（片方だけにあるファイルと、内容が違うファイル）。 */
export function treeDiff(a, b) {
  const ha = treeHashes(a);
  const hb = treeHashes(b);
  const names = new Set([...ha.keys(), ...hb.keys()]);
  let onlyOne = 0;
  let differ = 0;
  for (const n of names) {
    if (!ha.has(n) || !hb.has(n)) onlyOne += 1;
    else if (ha.get(n) !== hb.get(n)) differ += 1;
  }
  return { files: names.size, onlyOne, differ };
}

/** 配信する木を `tmpDir` の下へ写し、`ir/main.json` を `irBytes` に差し替えた木のパスを返す（元の木は変えない）。 */
export function overlaidSite(siteDir, tmpDir, irBytes) {
  const dest = fs.mkdtempSync(path.join(tmpDir, 'site-'));
  fs.cpSync(siteDir, dest, { recursive: true });
  fs.writeFileSync(path.join(dest, IR_REL), irBytes);
  return dest;
}

/** 画面の操作だけで履歴を書き出し、保存したファイルのバイト列を返す。 */
export async function exportByUi(page, tmpDir) {
  await page.locator(EXPORT_BTN).click();
  const link = page.locator(LINK);
  await link.waitFor({ state: 'visible', timeout: 10000 });
  const [download] = await Promise.all([page.waitForEvent('download'), link.click()]);
  const file = path.join(tmpDir, `${Date.now()}-${Math.random().toString(16).slice(2)}.json`);
  await download.saveAs(file);
  const bytes = fs.readFileSync(file);
  fs.rmSync(file, { force: true });
  return bytes;
}

/** 失敗・配信元以外の要求の診断用の名前。query と fragment は落とし、origin 内は path だけにする。 */
export function reqLabel(u) {
  try {
    const x = new URL(typeof u === 'string' ? u : u.url);
    return x.hostname === '127.0.0.1' ? x.pathname : `${x.origin}${x.pathname}`;
  } catch {
    return '(unparsable)';
  }
}

/**
 * `url` を新しい browser context で開いて入力列を流し、観察・snapshot・history と計数を返す。
 * `stopAt` を指定すると、その Step に入ったところで止める（対照用）。`keep` が真なら context を閉じず page を返す。
 * `drive(page, { L, onStep })` があれば、既定の誘導（journey）の代わりに呼ぶ（固定の列の再生・凍結の記録）。
 * `prepare({ page, context })` は最初の遷移の前に呼ぶ（時計・乱数をずらす試験用）。
 */
export async function runOnce(ctx, label, origin, url, { stopAt = L2_STOP, keep = false, drive = null, prepare = null } = {}) {
  const { browser, order, tmpDir } = ctx;
  const t0 = Date.now();
  const { page, context, rec } = await openPage(browser, { origin });
  try {
    if (prepare) await prepare({ page, context });
    await page.goto(url);
    await waitMounted(page);
    const L = ledger();
    const steps = [];
    const onStep = async (uid, status) => { steps.push(await observeStep(page, order, uid, status)); };
    if (drive) {
      await drive(page, { L, onStep });
    } else if (stopAt === L2_STOP) {
      await runL2JourneyWith(page, L, { order, scene: 'match', onStep });
    } else {
      await journey(page, mouseDriver(page), 'match', L, stopAt, { observe: domObserver(order), onStep });
    }
    const snapshot = await takeSnapshot(page);
    const history = await exportByUi(page, tmpDir);
    // 計数と problems は書き出し操作の後に評価する（書き出しで生じたエラー・CSP 違反・外部要求も合否に含める）。
    const counts = summarize(rec);
    const irUrl = `${origin}${new URL(url).pathname}ir/main.json`;
    // IR の取得の完了（requestfinished）は、中断の通知と前後して届くことがある。完了の通知を最大 2 秒待ってから数える。
    for (let i = 0; i < 40 && !rec.finished.includes(irUrl); i += 1) await new Promise((res) => setTimeout(res, 50));
    const irReqs = rec.requests.filter((r) => r.url === irUrl);
    // 要求は Request の同一性（id）で response・requestfinished・requestfailed を対応付ける。
    // 本文の取得まで完了した 200 の IR（別の要求）がある実行に限り、応答を受けた後に ERR_ABORTED になった同じ URL の要求
    // （完了後に中断された重複の要求）だけを、失敗にも要求の件数にも数えない。応答のない中断・query の違う要求・
    // 別の資源・別のエラー種別・完了した取得が無いときの中断は従来どおり数える（IR の 200 応答はちょうど 1 件であること）。
    const respondedIds = new Set(irReqs.map((r) => r.id));
    const isAbortedIr = (f) => f.url === irUrl && f.error === 'net::ERR_ABORTED' && respondedIds.has(f.id);
    const abortedIds = new Set(rec.failed.filter(isAbortedIr).map((f) => f.id));
    const completedOk = irReqs.filter((r) => r.status === 200 && rec.finishedIds.includes(r.id) && !abortedIds.has(r.id));
    const irDone = completedOk.length >= 1;
    const failed = rec.failed.filter((f) => !(irDone && isAbortedIr(f)));
    const counted = irDone ? irReqs.filter((r) => !abortedIds.has(r.id)) : irReqs;
    const irCount = counted.length;
    const problems = [];
    if (irCount !== 1 || !counted.some((r) => r.status === 200)) problems.push('ir-fetch');
    if (!rec.requests.every((r) => r.status >= 200 && r.status < 300)) problems.push('non-2xx');
    if (counts.pageErrors !== 0) problems.push('page-error');
    if (counts.csp !== 0 || cspConsoleMessages(rec).length !== 0) problems.push('csp');
    if (counts.consoleErrors !== 0 || rec.consoleWarnings.length !== 0) problems.push('console');
    if (counts.external !== 0 || failed.length !== 0) problems.push(`external(${counts.external}:${rec.external.map(reqLabel).join(',')})/failed(${failed.length}:${failed.map((f) => `${reqLabel(f.url)}=${f.error}`).join(',')})`);
    if (counts.dialogs !== 0 || counts.popups !== 0 || counts.crashed !== 0) problems.push('dialog-popup-crash');
    const parsed = JSON.parse(history.toString('utf8'));
    assert.ok(Array.isArray(parsed.ops) && parsed.ops.length >= 1, `${label}: history has ops`);
    const obsText = JSON.stringify(steps);
    const stepsText = JSON.stringify({ steps, ledger: L.items.map(({ name, ok }) => ({ name, ok })) });
    const r = {
      label,
      stepsText,
      obsText,
      snapshot,
      history: history.toString('utf8'),
      parsed,
      hashes: { steps: sha256(stepsText), obs: sha256(obsText), snapshot: sha256(snapshot), history: sha256(history) },
      failedItems: L.failed(),
      problems,
      requests: counts.requests,
      ms: Date.now() - t0,
    };
    return keep ? { ...r, page, context } : r;
  } finally {
    if (!keep) await context.close().catch(() => {});
  }
}

/**
 * 配信先 `target` を起動して fn(origin, url) を呼び、必ず片づける。
 * `overlay`（Buffer）があれば、配信する木の `ir/main.json` をそれに差し替えた写しを配る（元の木・zip は変えない）。
 */
export async function withTarget(ctx, target, fn, { overlay = null } = {}) {
  const { zip, siteDir, tmpDir } = ctx;
  if (target === 'L2') {
    const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l1-l2-run-'));
    let run = null;
    try {
      run = await startL2(zip, work, overlay ? { overlay: { [`site/${IR_REL}`]: overlay } } : {});
      return await fn(`http://127.0.0.1:${run.port}`, run.url);
    } finally {
      await stopL2(run);
      fs.rmSync(work, { recursive: true, force: true });
    }
  }
  const dir = overlay ? overlaidSite(siteDir, tmpDir, overlay) : siteDir;
  try {
    return await withServer({ dir, base: BASES[target] }, (srv) => fn(srv.origin, srv.url));
  } finally {
    if (overlay) fs.rmSync(dir, { recursive: true, force: true });
  }
}
