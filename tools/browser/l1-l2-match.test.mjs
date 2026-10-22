// L1（手元のサブパス）と L2（展開して ./start.sh）の 3 配信先 × 3 回、計 9 回の実行で結果が一致することの試験
// （TASK-114 1/2。DLV-15。`make test-l2-run` から呼ばれる。`make test-browser` には入れない）。
//
// 目的: 配信の経路（サーバの実装・base のパス・オリジン）が、教材の挙動と書き出しに影響しないことを固定する。
// 3 配信先（D-58 が案 (i) を採らないと決めたため、PoC-10 の「案 (i)・案 (ii)」ではなく repo にある経路で互いに違うものにする）:
//   L1-a  tools/serve.mjs（gzip つき）・base /fandhe-workshop/・回ごとに新しいポート
//   L1-b  同上・別名のサブパス /another-repo-name/（DLV-2 の前例）・回ごとに新しいポート
//   L2    zip を展開した ./start.sh（miniserve。PATH=/usr/bin:/bin の模擬環境）・base /・回ごとに新しいポート
// 同じ入力列（CH-0〜CH-1 の誘導。l2-journey.mjs を共有）を各配信先で 3 回流し、次の 3 つが 9 回とも 1 種類であることを求める。
//   (a) 操作ごとの観察の列（Step の ID・状態・語・ログの行数と最後の行・入力の口の個数）と、誘導の台帳（項目名と合否）
//   (b) 最後の画面の状態 `snapshot.json`（screen-snapshot.mjs。オリジン依存の値は除く）
//   (c) 画面の操作（「履歴を書き出す」→ リンク）で書き出した `history.json` のバイト列
// 前提: 差が配信の差であってビルドの差でないことを、zip を展開した site/ と配信する木の SHA-256 の照合で先に確かめる。
// 対照（空振りの防止）: 途中で止めた回はハッシュが違う／直列化は文字の書き換えを検出し blob の href の違いは除く／照合は 1 バイトの違いを検出する。
// 限界: 教材は試験データ（ch0_3）で本物の CH-0〜CH-1 の 50 操作ではない。題は画面に描かれていない（shell-title は未結線）ため
// Step の ID で代える。拒否・問い返しの直後は観察を積まず、台帳の「進まない」の合否で比べる。Chromium・Linux の miniserve だけ。
// 履歴に揺れる欄が無いことの検査と固定の入力列の比較は TASK-114 2/2 が持つ。
//
// 入力（環境変数。未設定・不在は skip にせず失敗にする）:
//   FANDHE_L2_ZIP   tools/build-l2.mjs が作った zip
//   FANDHE_L2_IR    zip に同梱した IR の JSON
//   FANDHE_L2_SITE  zip の元にした配信する設定の木（scripts/test-l2-run.sh が渡す）
// 出力に履歴の本文・入力した値・DOM の文字を載せない（件数・ハッシュの先頭 12 桁・合否・時間だけ）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize, cspConsoleMessages,
} from './harness.mjs';
import { ledger, journey, mouseDriver } from './ch0-3-journey.mjs';
import { runL2JourneyWith, stepOrder, domObserver, L2_STOP } from './l2-journey.mjs';
import { startL2, stopL2, unzipTo } from './l2-server.mjs';
import { takeSnapshot, observeStep, sha256, firstDiffPath, readScreen, serializeScreen } from './screen-snapshot.mjs';

const RUNS = 3;
const TARGETS = ['L1-a', 'L1-b', 'L2'];
const BASES = { 'L1-a': '/fandhe-workshop/', 'L1-b': '/another-repo-name/' };
const EXPORT_BTN = '[data-testid="shell-export"]';
const LINK = '[data-testid="export-download"]';
const MID_STOP = 'main/st-1-4'; // journey が止まれる Step は main/st-1-4 と main/st-2-1 だけ
let browser;
let zip;
let siteDir;
let order;
let tmpDir;
const results = [];

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

before(async () => {
  zip = process.env.FANDHE_L2_ZIP;
  const irFile = process.env.FANDHE_L2_IR;
  siteDir = process.env.FANDHE_L2_SITE;
  assert.ok(zip && fs.statSync(zip, { throwIfNoEntry: false })?.isFile(), 'FANDHE_L2_ZIP must point to the L2 zip');
  assert.ok(irFile && fs.statSync(irFile, { throwIfNoEntry: false })?.isFile(), 'FANDHE_L2_IR must point to the IR json');
  assert.ok(siteDir && fs.statSync(siteDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_L2_SITE must point to the delivery directory');
  order = stepOrder(fs.readFileSync(irFile, 'utf8'));
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l1-l2-match-'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

/** 画面の操作だけで履歴を書き出し、保存したファイルのバイト列を返す。 */
async function exportByUi(page) {
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
function reqLabel(u) {
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
 */
async function runOnce(label, origin, url, { stopAt = L2_STOP, keep = false } = {}) {
  const t0 = Date.now();
  const { page, context, rec } = await openPage(browser, { origin });
  try {
    await page.goto(url);
    await waitMounted(page);
    const L = ledger();
    const steps = [];
    const onStep = async (uid, status) => { steps.push(await observeStep(page, order, uid, status)); };
    if (stopAt === L2_STOP) {
      await runL2JourneyWith(page, L, { order, scene: 'match', onStep });
    } else {
      await journey(page, mouseDriver(page), 'match', L, stopAt, { observe: domObserver(order), onStep });
    }
    const counts = summarize(rec);
    const irReqs = rec.requests.filter((r) => r.url === `${origin}${new URL(url).pathname}ir/main.json`);
    const problems = [];
    if (irReqs.length !== 1 || irReqs[0].status !== 200) problems.push('ir-fetch');
    if (!rec.requests.every((r) => r.status >= 200 && r.status < 300)) problems.push('non-2xx');
    if (counts.pageErrors !== 0) problems.push('page-error');
    if (counts.csp !== 0 || cspConsoleMessages(rec).length !== 0) problems.push('csp');
    if (counts.consoleErrors !== 0 || rec.consoleWarnings.length !== 0) problems.push('console');
    if (counts.external !== 0 || counts.failed !== 0) problems.push(`external(${counts.external}:${rec.external.map(reqLabel).join(',')})/failed(${counts.failed}:${rec.failed.map((f) => `${reqLabel(f.url)}=${f.error}`).join(',')})`);
    if (counts.dialogs !== 0 || counts.popups !== 0 || counts.crashed !== 0) problems.push('dialog-popup-crash');
    const snapshot = await takeSnapshot(page);
    const history = await exportByUi(page);
    const parsed = JSON.parse(history.toString('utf8'));
    assert.ok(Array.isArray(parsed.ops) && parsed.ops.length >= 1, `${label}: history has ops`);
    const stepsText = JSON.stringify({ steps, ledger: L.items.map(({ name, ok }) => ({ name, ok })) });
    const r = {
      label,
      stepsText,
      snapshot,
      history: history.toString('utf8'),
      hashes: { steps: sha256(stepsText), snapshot: sha256(snapshot), history: sha256(history) },
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

/** 配信先 `target` を起動して fn(origin, url) を呼び、必ず片づける。 */
async function withTarget(target, fn) {
  if (target === 'L2') {
    const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l1-l2-run-'));
    let run = null;
    try {
      run = await startL2(zip, work);
      return await fn(`http://127.0.0.1:${run.port}`, run.url);
    } finally {
      await stopL2(run);
      fs.rmSync(work, { recursive: true, force: true });
    }
  }
  return withServer({ dir: siteDir, base: BASES[target] }, (srv) => fn(srv.origin, srv.url));
}

const short = (h) => h.slice(0, 12);

test('前提: zip を展開した site/ と配信する木が、同じファイルの集合で同じ SHA-256 である', () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l1-l2-tree-'));
  try {
    const top = unzipTo(zip, work);
    const d = treeDiff(siteDir, path.join(top, 'site'));
    console.log(`同梱物の照合: ファイル ${d.files} 件・片方だけ ${d.onlyOne} 件・内容の違い ${d.differ} 件`);
    assert.ok(d.files >= 1);
    assert.deepEqual({ onlyOne: d.onlyOne, differ: d.differ }, { onlyOne: 0, differ: 0 });
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
});

test('対照: 照合は、展開した写しの 1 バイトの違いと余分なファイルを検出する（zip・配信する木は変えない）', () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l1-l2-tree-ctl-'));
  try {
    const top = unzipTo(zip, work);
    const copy = path.join(top, 'site');
    const f = path.join(copy, 'index.html');
    const original = fs.readFileSync(f);
    fs.writeFileSync(f, Buffer.concat([original, Buffer.from('\n')]));
    assert.deepEqual(treeDiff(siteDir, copy), { files: treeHashes(siteDir).size, onlyOne: 0, differ: 1 });
    fs.writeFileSync(f, original);
    fs.writeFileSync(path.join(copy, 'extra.txt'), 'x');
    assert.equal(treeDiff(siteDir, copy).onlyOne, 1);
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
});

for (const target of TARGETS) {
  for (let n = 1; n <= RUNS; n += 1) {
    test(`${target} ${n}/${RUNS} 回目: 同じ入力列で CH-0〜CH-1 を流し、観察・snapshot・history を取る`, async () => {
      await withTarget(target, async (origin, url) => {
        const r = await runOnce(`${target}#${n}`, origin, url);
        results.push(r);
        console.log(`${r.label}: 観察 ${JSON.parse(r.stepsText).steps.length} 件・要求 ${r.requests} 件・`
          + `steps ${short(r.hashes.steps)}・snapshot ${short(r.hashes.snapshot)}・history ${short(r.hashes.history)}・${r.ms} ms`);
        assert.deepEqual(r.failedItems, []);
        assert.deepEqual(r.problems, []);
      });
    });
  }
}

test('9 回の観察の列・snapshot.json・history.json がそれぞれ 1 種類（バイト単位で一致）', () => {
  assert.equal(results.length, TARGETS.length * RUNS, '9 回すべてが通らなかったため比べない');
  const base = results[0];
  for (const key of ['steps', 'snapshot', 'history']) {
    const kinds = new Set(results.map((r) => r.hashes[key]));
    const mismatches = results.filter((r) => r.hashes[key] !== base.hashes[key]);
    const where = mismatches.map((r) => {
      const field = key === 'history' ? firstDiffPath(base.history, r.history)
        : key === 'steps' ? firstDiffPath(base.stepsText, r.stepsText)
          : firstDiffPath(base.snapshot, r.snapshot);
      return `${r.label}@${field}(${short(r.hashes[key])})`;
    });
    console.log(`${key}: 種類 ${kinds.size}（先頭 ${short(base.hashes[key])}）`);
    assert.equal(kinds.size, 1, `${key} differs from ${base.label} (${short(base.hashes[key])}): ${where.join(' ')}`);
  }
});

test('対照: 終点の手前で止めた回は、観察・snapshot・history のハッシュが 9 回と違う', async () => {
  assert.ok(results.length >= 1);
  await withTarget('L1-a', async (origin, url) => {
    const r = await runOnce('L1-a#stop', origin, url, { stopAt: MID_STOP });
    for (const key of ['steps', 'snapshot', 'history']) {
      assert.notEqual(r.hashes[key], results[0].hashes[key], `${key} must differ when the journey stops early`);
    }
  });
});

test('対照: 直列化は画面の文字の書き換えを検出し、blob の href の違いは除く', async () => {
  await withTarget('L1-b', async (origin, url) => {
    const r = await runOnce('L1-b#ctl', origin, url, { keep: true });
    try {
      assert.equal(r.hashes.snapshot, results[0].hashes.snapshot, 'the control run matches the others first');
      const { page } = r;
      // 書き出した後はリンクが画面にある。href と data-export-url だけを別の blob に替えても、snapshot は変わらない。
      assert.equal(await page.locator(LINK).count(), 1, 'export link is on the screen');
      const before = await takeSnapshot(page);
      assert.ok(before.includes('data-testid=\\"export-download\\"'), 'the link element is in the snapshot');
      await page.evaluate(() => {
        const a = document.querySelector('[data-testid="export-download"]');
        a.setAttribute('href', 'blob:http://example.invalid/other');
        a.setAttribute('data-export-url', 'blob:http://example.invalid/other');
        a.setAttribute('data-key', 'blob:http://example.invalid/other');
      });
      assert.equal(sha256(await takeSnapshot(page)), sha256(before), 'a different blob href must not change the snapshot');
      // 画面の 1 行の文字を書き換えると、snapshot が変わる。
      await page.evaluate(() => {
        const e = document.querySelector('[data-testid="line-body"]');
        e.textContent = `${e.textContent}x`;
      });
      assert.notEqual(sha256(await takeSnapshot(page)), sha256(before), 'a text change must change the snapshot');
    } finally {
      await r.context.close().catch(() => {});
    }
  });
});

test('対照: 自己検査は、オリジン依存の文字列が残った直列化を拒む', () => {
  const screen = { mounted: 'true', active: null, draft: '', os: '', html: '<a href="http://127.0.0.1:1/x">x</a>' };
  assert.throws(() => serializeScreen(screen), /origin-dependent/);
  assert.equal(typeof readScreen, 'function');
});
