// CH-9 から CH-10 の引き継ぎ（1 周目の終わりから 2 周目の始まり）を自動ブラウザで確かめる試験（TASK-254 2/2。MOD-19・MOD-5・MOD-8）。
//
// 解釈（最大の限界）: 教材 repo の本物の CH-9・CH-10 がまだ無い。そのため「CH-9 → CH-10」を、2 周目の実体の根を持つ
// `testdata/module-reuse-rounds/test-data/pack` の「`ch-round1` の最後 → `ch-round2` の最初」に読み替える（native の
// `round_slice_init.rs`・`round_begin_host.rs` と同じ教材）。本物の CH-9・CH-10 での流し直しは未実施（`docs/testing.md` の 5.343）。
// AI の代理の確認であり、人の評価には数えない（D-34）。2 周目の開始の画面の操作（ボタン 1 つ・確認なし）は SHELL-10 の
// 仮置きのままで（ROUND2-2・TASK-690 は検討中）、本試験はこの仮置きを固定する。決定が出たら本試験を直す。
//
// 入力は scripts/test-browser.sh が native の `emit_round_handoff_for_browser` で書き出す。IR・履歴・2 周目の計画は同じ
// `compile_dir` の出力から作るので、`pack_id` と `content_hash` が合う。1 周目は `GoTo` を 3 か所含み、配信する設定では
// `GoTo` を送れないため、1 周目の終わりの履歴は製品の読み込み欄（ファイル入力）から開く。
//
// 確かめること:
//   B 配信する設定・DOM だけの観察（CSP あり・なし）。1 周目の終わりの履歴を開き、「次の章へ進む」がちょうど 1 個（`s-r2`）で
//     あることを確かめ、実クリックで 2 周目を始める。書き出しの JSON で、世界状態が押す前と一致（MOD-5）し、ops が追記だけで
//     増え、世代が +1 され、現在の Step が 2 周目の根にあり、1 周目の根と子が閉じたまま（MOD-8）であることを確かめる。
//     続いて 2 周目を最初の文書の節の指定の直前まで画面の操作だけで進め、1 周目の世界状態が全要素同一のまま残ることを確かめる。
//     再読み込みの後も位置が同じでボタンが 0 個である。
//   C 試験の設定・較正と深い引き継ぎ（CSP あり）。B の DOM の目印が `stateJson` の現在の Step・ログと一致する。
//     最初の `GoTo` だけ `raw-trigger` で送り（未実装の `on_enter` を飛ばすため。1 回）、それ以外は画面の操作で `/create-issue-tree`
//     まで進めて、2 周目の根が open、1 周目の根と子が閉じたまま、1 周目の世界状態が全要素同一であることを確かめる。
//   D 負の対照。1 周目の途中の履歴ではボタンが 0 個で、提示のない押下は状態を変えない。比べる部品が差を検出する。
//
// 確かめないこと: 本物の CH-9・CH-10、3 つ目のハッシュ（`s-r2-fresh`。native の `round_slice_init` の記録）、停止・復帰を挟む流し
// （TASK-88）、2 周目の最後までの通し（`GoTo` が 2 か所要る）、2 周目専用の案内文と確認の有無（ROUND2-2）。
// SHA-256 の正式な記録は native の `round_slice_init.rs`・`round_begin_host.rs`。ブラウザ側は直列化した world の構造の一致で示す。
//
// 出力は名前・件数・合否だけで、履歴の中身・入力した文・世界状態の値・教材の題は出さない。書くファイルは一時ディレクトリの中だけ。
// 入力: FANDHE_WEB_DIR・FANDHE_WEB_TEST_DIR・FANDHE_ROUND_HANDOFF_DIR。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages, summarize,
} from './harness.mjs';
import {
  NEXT, RAW, T, rowSel, ledger, mouseDriver, install, engine, waitCount,
} from './ch0-3-journey.mjs';
import { withDeliveryCopy } from './ch4-9-journey.mjs';

const BASE = '/fandhe-round-handoff/';
const POSITION = '[data-testid="shell-position"]';
const EXPORT_BTN = '[data-testid="shell-export"]';
const LINK = '[data-testid="export-download"]';
const FILE_INPUT = '[data-testid="shell-import"] input[type="file"]';
const LOG_ITEMS = '[data-testid="part-log-list"] li';
let browser;
let tmpDir;
let dir;
let ir;
let endFile;
let halfFile;
let plan;
let expect;

before(async () => {
  for (const k of ['FANDHE_WEB_DIR', 'FANDHE_WEB_TEST_DIR', 'FANDHE_ROUND_HANDOFF_DIR']) {
    const v = process.env[k];
    assert.ok(v && fs.statSync(v, { throwIfNoEntry: false })?.isDirectory(), `${k} must point to a directory`);
  }
  dir = process.env.FANDHE_ROUND_HANDOFF_DIR;
  const read = (n) => {
    assert.ok(fs.existsSync(path.join(dir, n)), `${n} must exist`);
    return fs.readFileSync(path.join(dir, n));
  };
  ir = read('ir.json');
  endFile = path.join(dir, 'round1-end.history.json');
  halfFile = path.join(dir, 'round1-half.history.json');
  read('round1-end.history.json');
  read('round1-half.history.json');
  plan = JSON.parse(read('round2-plan.json'));
  expect = JSON.parse(read('expect.json'));
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-round-handoff-'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
// 並び順の違いを無視した構造の一致。world は BTreeMap の整列順で出るが、比べる側の都合で順序に依存しない。
const canon = (v) => {
  if (Array.isArray(v)) return v.map(canon);
  if (v && typeof v === 'object') return Object.fromEntries(Object.keys(v).sort().map((k) => [k, canon(v[k])]));
  return v;
};
const deepSame = (a, b) => same(canon(a), canon(b));

// 計画の 1 手を「開始点の直前」で切るための印（最初の GoTo の位置）。
// 文書の節（`path#section`）の指定は、文書のビューの行（文書の単位）として画面に出ず、実クリックで指せない。
// そのため B（実クリックだけ）は、最初の節の指定の直前までを通す。
const isSection = (s) => s.op?.input?.ui?.path?.includes('#') === true;
const firstSection = () => plan.findIndex(isSection);
const createTreeAt = () => plan.findIndex((s) => s.op?.input?.command?.startsWith('/create-issue-tree'));

const chapterOf = async (page) => ((await page.locator(POSITION).textContent()) ?? '').trim().split(/\s+/)[0];
const positionTextOf = async (page) => ((await page.locator(POSITION).textContent()) ?? '').trim();

async function exportByUi(page) {
  await page.locator(EXPORT_BTN).click();
  const link = page.locator(LINK);
  await link.waitFor({ state: 'visible', timeout: 10000 });
  const [download] = await Promise.all([page.waitForEvent('download'), link.click()]);
  const file = path.join(tmpDir, `${Date.now()}-${Math.random().toString(16).slice(2)}.json`);
  await download.saveAs(file);
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

// 指す行が今のビューに無ければ、その行のビューの種類（IR の views[id].kind）の切り替えのボタンを実クリックしてから指す。
// それでも行が無いとき（画面から指せない宛先）は、`rawOp` があればそれで送り、送ったことを 1 で返す（無ければ失敗）。
async function pointRow(page, d, view, rowPath, rawOp) {
  const kind = JSON.parse(ir.toString('utf8')).views[view].kind;
  const row = page.locator(rowSel(rowPath)).first();
  const shown = () => row.waitFor({ state: 'attached', timeout: 1500 }).then(() => true, () => false);
  if (!(await shown())) {
    await page.click(`[data-testid="view-select-button"][data-payload="${kind}"]`, { timeout: T });
    if (!(await shown())) {
      assert.ok(rawOp, 'row is not reachable by the screen');
      await rawOp();
      return 1;
    }
  }
  await d.point(rowPath);
  return 0;
}

// 計画の 1 手を画面の操作に直して流す。`GoTo` は呼び出し側が扱う（ここでは受けない）。
// `Tick` は処理中の終わりを待つだけ（進行はタイマーで進む）。次の入力の口が出るまでは各操作の click が待つ。
// `rawOps` を渡すと、画面から指せない手（制御の op と、文書の節 `path#section` の指定）をそれで送る（試験の設定だけ）。送った数を返す。
async function drive(page, d, steps, rawOps = null) {
  let raw = 0;
  for (const s of steps) {
    if (rawOps && s.op && (s.op.control || isSection(s))) {
      await rawOps(JSON.stringify(s.op));
      raw += 1;
      continue;
    }
    if (s.tick) {
      await page.waitForFunction(() => document.querySelector('[data-testid="run-status"]')?.getAttribute('data-state') === 'idle', null, { timeout: T });
    } else if (s.expect_suffix !== undefined) {
      // 位置の確認は C の較正が受け持つ。
    } else {
      const op = s.op;
      assert.ok(!op.control && !isSection(s), 'unreachable-by-screen op needs rawOps');
      const input = op.input;
      if (input.command !== undefined) await d.send(input.command);
      else if (input.text !== undefined) await d.send(input.text);
      else if (input.ui) {
        raw += await pointRow(page, d, input.ui.view, input.ui.path, rawOps && (() => rawOps(JSON.stringify(op))));
      }
      else if (input.choice !== undefined) await d.choose(input.choice);
      else if (input.approval) {
        if (input.approval.target) await d.revise(input.approval.target);
        else await d.decide(input.approval.decision);
      } else assert.fail('unknown input kind');
    }
  }
  return raw;
}

async function importFile(page, file) {
  await page.setInputFiles(FILE_INPUT, file);
}

// 配信する設定の画面を開き、履歴をファイル入力から読み込む。読み込みの完了は、右のログに行が出ることで待つ
// （位置の表示は読み込む前から ch-round1 で、完了の印にならない）。
async function openImported(page, srv, file) {
  await page.goto(srv.url);
  await waitMounted(page);
  assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'delivery build must have no test hooks');
  await install(page);
  await importFile(page, file);
  await page.waitForFunction((sel) => document.querySelectorAll(sel).length > 0, LOG_ITEMS, { timeout: T });
}

function closedOk(e, expectCfg) {
  return expectCfg.closed_paths.every((p) => e.snapshot.run.world[p]?.attrs?.state === 'closed');
}

// 1 周目の世界状態の全要素が、`world` に同一で残っているか。
function round1Kept(world) {
  return Object.entries(expect.world).every(([k, v]) => deepSame(world[k], v));
}

async function runChecks(page, scene, L, raw) {
  const run = await page.evaluate(() => window.__run);
  L.check(scene, raw === 0 ? 'raw-trigger の click が 0 回' : `raw-trigger の click が ${raw} 回`, run.raw === raw);
  L.check(scene, '実マウスの操作が通っている', run.mouse > 0);
}

async function finish(page, rec, scene, L) {
  const counts = await collectSafety(page, rec);
  for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
  L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
  const s = summarize(rec);
  L.check(scene, 'ページ内エラーが 0 件', s.pageErrors === 0);
  L.check(scene, 'ダイアログが 0 件', s.dialogs === 0);
  L.check(scene, 'コンソールの CSP 警告が 0 件', cspConsoleMessages(rec).length === 0);
  L.check(scene, '配信元の外への通信が 0 件', rec.external.length === 0);
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`B: 1 周目の終わりから次の章へ進むで 2 周目が新しい根で始まる（配信する設定・${mode}）`, { timeout: 300000 }, async () => {
    let failed = [];
    await withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, (webDir) => {
      fs.writeFileSync(path.join(webDir, 'ir', 'main.json'), ir);
      return withServer({ dir: webDir, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        const L = ledger();
        const scene = '配信';
        await openImported(page, srv, endFile);
        const d = mouseDriver(page);

        // 押す前。
        const e0 = await exportByUi(page);
        const loaded = JSON.parse(fs.readFileSync(endFile, 'utf8'));
        L.check(scene, '読み込みが履歴の ops を変えない', deepSame(e0.ops, loaded.ops));
        L.check(scene, '読み込みが世界状態を変えない', deepSame(e0.snapshot.run, loaded.snapshot.run));
        L.check(scene, '押す前の位置の章が ch-round1', (await chapterOf(page)) === 'ch-round1');
        L.check(scene, '次の章へ進むがちょうど 1 個', (await page.locator(NEXT).count()) === 1);
        L.check(scene, '開始点が s-r2（s-r2-fresh でない）', (await page.locator(NEXT).getAttribute('data-payload')) === expect.start);
        L.check(scene, '押す前の世界状態が native の期待と一致', deepSame(e0.snapshot.run.world, expect.world));
        L.check(scene, '押す前に閉じた根と子が closed', closedOk(e0, expect));
        const posBefore = await positionTextOf(page);

        // 押す（確認なし。仮置き）。
        await d.nextChapter();
        await waitCount(page, NEXT, 0);
        await page.waitForFunction((p) => document.querySelector(p)?.textContent?.trim().startsWith('ch-round2'), POSITION, { timeout: T });
        L.check(scene, '押した後に次の章へ進むが 0 個', (await page.locator(NEXT).count()) === 0);
        L.check(scene, '位置の章が ch-round2 に変わる', (await chapterOf(page)) === 'ch-round2');
        L.check(scene, '位置の表示が変わった', (await positionTextOf(page)) !== posBefore);

        const e1 = await exportByUi(page);
        L.check(scene, 'MOD-5: 押した後の世界状態が押す前と一致', deepSame(e1.snapshot.run.world, e0.snapshot.run.world));
        L.check(scene, 'MOD-5: 押した後の世界状態が native の期待と一致', deepSame(e1.snapshot.run.world, expect.world));
        L.check(scene, 'ops が追記だけで 1 件増える', e1.ops.length === e0.ops.length + 1 && deepSame(e1.ops.slice(0, e0.ops.length), e0.ops));
        L.check(scene, '追記された op が begin: s-r2', e1.ops[e1.ops.length - 1]?.control?.begin === expect.start);
        L.check(scene, '世代が +1', e1.snapshot.run.generation === e0.snapshot.run.generation + 1 && e0.snapshot.run.generation === expect.generation);
        const cur = e1.snapshot.run.current;
        L.check(scene, '現在の Step が 2 周目の根にある', expect.r2_roots.some((r) => cur.startsWith(`${r}/`)));
        L.check(scene, '現在の Step が 1 周目の根にない', !expect.r1_roots.some((r) => cur.startsWith(`${r}/`)));
        L.check(scene, 'MOD-8: 閉じた根と子が closed のまま', closedOk(e1, expect));
        L.check(scene, '1 周目の根の切り身が一致', expect.r1_roots.every((r) => deepSame(
          Object.fromEntries(Object.entries(e1.snapshot.run.slices).filter(([k]) => k === r)),
          Object.fromEntries(Object.entries(e0.snapshot.run.slices).filter(([k]) => k === r)),
        )));

        // 2 周目を最初の GoTo の直前まで画面の操作だけで進める。
        const g = firstSection();
        L.check(scene, '計画に節の指定がある', g > 0);
        await drive(page, d, plan.slice(0, g));
        const e2 = await exportByUi(page);
        L.check(scene, '1 周目の世界状態の全要素が同一に残る', round1Kept(e2.snapshot.run.world));
        L.check(scene, '2 周目で世界状態が増える', Object.keys(e2.snapshot.run.world).length > Object.keys(expect.world).length);
        L.check(scene, '増えた経路が 1 周目の経路と交わらない', Object.keys(e2.snapshot.run.world)
          .filter((k) => !(k in expect.world)).every((k) => !expect.r1_paths.includes(k)));
        L.check(scene, '閉じた根と子が closed のまま（2 周目の途中）', closedOk(e2, expect));
        L.check(scene, '2 周目の途中でボタンが 0 個', (await page.locator(NEXT).count()) === 0);

        // 再読み込みでページ内の計数が消えるため、先に操作の計数を確かめる。
        await runChecks(page, scene, L, 0);

        // 再読み込み。保存から復元する。
        const posMid = await positionTextOf(page);
        await page.reload();
        await waitMounted(page);
        await page.waitForFunction((p) => (document.querySelector(p)?.textContent ?? '').trim().length > 0, POSITION, { timeout: T });
        L.check(scene, '再読み込みの後も位置が同じ', (await positionTextOf(page)) === posMid);
        L.check(scene, '再読み込みの後もボタンが 0 個', (await page.locator(NEXT).count()) === 0);

        await finish(page, rec, scene, L);
        const irReq = rec.requests.filter((r) => new URL(r.url).pathname.endsWith('/ir/main.json'));
        L.check(scene, 'ir/main.json が 200 で取得', irReq.length >= 1 && irReq.every((r) => r.status === 200));
        console.log(`B (${mode}): ${L.summary()} 項目`);
        failed = L.failed();
        await context.close();
      });
    }));
    assert.deepEqual(failed, []);
  });
}

async function withTestPage(fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: true }, (webDir) => withServer({ dir: webDir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await fn(page, rec, srv);
    } finally {
      await context.close();
    }
  }));
}

const settle = (page) => page.waitForFunction(() => window.__fandheTest.nextTick() === 'null', null, { timeout: T });
const sendRaw = async (page, json) => {
  await page.evaluate(([sel, j]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', j);
    el.click();
  }, [RAW, json]);
  await settle(page);
};
// 計画の `op` を raw-trigger で送る（試験の設定だけ）。ticks はタイマーで進むので待つだけ。
async function driveRaw(page, steps) {
  let sent = 0;
  for (const s of steps) {
    if (s.op) {
      await sendRaw(page, JSON.stringify(s.op));
      sent += 1;
    } else if (s.tick) await settle(page);
  }
  return sent;
}

test('C: DOM の目印が stateJson と一致し、2 周目の根が open で 1 周目が閉じたまま残る（試験の設定）', { timeout: 300000 }, async () => {
  let failed = [];
  const plan1 = JSON.parse(fs.readFileSync(path.join(dir, 'round1-plan.json'), 'utf8'));
  await withTestPage(async (page, rec, srv) => {
    const L = ledger();
    const scene = '較正';
    // `loadIr` で入れ替えた教材の初期状態は、履歴を書き出した配信する設定の初期状態と違い、履歴を開けない。
    // そのため 1 周目は計画の op を raw-trigger で送り直す（B が履歴の読み込みで通す経路と別の、較正と深い引き継ぎの確認）。
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), ir.toString('utf8')), true, 'load-ir sent');
    await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > 0, null, { timeout: 15000 });
    await install(page);
    const d = mouseDriver(page);
    const stateOf = () => engine(page);

    // 負の対照: 1 周目の途中では、提示のないまま次の章の action を送っても状態は変わらない。
    const half = plan1.slice(0, 7);
    let rawSent = await driveRaw(page, half);
    L.check(scene, '1 周目の途中でボタンが 0 個', (await page.locator(NEXT).count()) === 0);
    const mid = await stateOf();
    await page.evaluate((sel) => {
      const el = document.querySelector(sel);
      const original = el.getAttribute('data-action');
      el.setAttribute('data-action', 'next-chapter');
      el.setAttribute('data-payload', 's-r2');
      el.click();
      el.setAttribute('data-action', original);
    }, RAW);
    await settle(page);
    const midAfter = await stateOf();
    L.check(scene, '提示のない押下の後も現在の Step・世代・世界状態が同じ', midAfter.current === mid.current
      && midAfter.stateWithoutLog.run.generation === mid.stateWithoutLog.run.generation
      && deepSame(midAfter.stateWithoutLog.run.world, mid.stateWithoutLog.run.world));
    L.check(scene, '提示のない押下の後も ch-round1 のまま', (await chapterOf(page)) === 'ch-round1');
    rawSent += 1;

    rawSent += await driveRaw(page, plan1.slice(half.length));
    let e = await stateOf();
    L.check(scene, '押す前: stateJson の現在の Step が 2 周目の根でない', !expect.r2_roots.some((r) => e.current.startsWith(`${r}/`)));
    L.check(scene, '押す前: DOM の章が ch-round1', (await chapterOf(page)) === 'ch-round1');
    L.check(scene, '押す前: ボタンが 1 個で開始点が s-r2', (await page.locator(NEXT).count()) === 1 && (await page.locator(NEXT).getAttribute('data-payload')) === expect.start);
    L.check(scene, '押す前: 閉じた根と子が closed', expect.closed_paths.every((p) => e.stateWithoutLog.run.world[p]?.attrs?.state === 'closed'));
    const world0 = e.stateWithoutLog.run.world;
    const gen0 = e.stateWithoutLog.run.generation;
    const logLen0 = e.log.length;

    await d.nextChapter();
    await waitCount(page, NEXT, 0);
    await page.waitForFunction((p) => document.querySelector(p)?.textContent?.trim().startsWith('ch-round2'), POSITION, { timeout: T });
    e = await stateOf();
    L.check(scene, '押した後: stateJson の現在の Step が 2 周目の根', expect.r2_roots.some((r) => e.current.startsWith(`${r}/`)));
    L.check(scene, '押した後: DOM の章が ch-round2', (await chapterOf(page)) === 'ch-round2');
    L.check(scene, '押した後: 世代が +1', e.stateWithoutLog.run.generation === gen0 + 1);
    const added = e.log.slice(logLen0);
    L.check(scene, '押した後: ログの追加が 2 周目の根への入場だけ', added.length > 0 && added[0].kind === 'module_entered' && added.some((r) => r.kind === 'entered'));
    L.check(scene, '押した後: ログの追加に moved・警告・無視が無い（GoTo でない）', !added.some((r) => ['moved', 'warning', 'ignored'].includes(r.kind)));
    L.check(scene, '押した後: 世界状態が不変', deepSame(e.stateWithoutLog.run.world, world0));

    // 2 周目を /create-issue-tree の直前まで進める。制御の op（未実装の on_enter を飛ばす GoTo）と文書の節の指定だけ
    // raw-trigger で送り、それ以外は画面の操作で進める。
    const t = createTreeAt();
    const first = plan.slice(0, t);
    const sectionsAndControls = first.filter((x) => x.op && (x.op.control || isSection(x))).length;
    rawSent += await drive(page, d, first, (j) => sendRaw(page, j));
    L.check(scene, 'raw で送った手が制御と節の指定だけ', rawSent > 0 && sectionsAndControls > 0);
    for (const p of expect.r2_paths.slice(0, 2)) {
      L.check(scene, '/create-issue-tree の前: 2 周目の新しい経路がまだ無い', !(p in (await stateOf()).stateWithoutLog.run.world));
    }
    await drive(page, d, plan.slice(t, t + 1));
    e = await stateOf();
    const w = e.stateWithoutLog.run.world;
    L.check(scene, '2 周目の根が open', w[expect.r2_paths[0]]?.attrs?.state === 'open');
    L.check(scene, '2 周目の子が open', w[expect.r2_paths[1]]?.attrs?.state === 'open');
    L.check(scene, '1 周目の根と子が closed のまま', expect.closed_paths.every((p) => w[p]?.attrs?.state === 'closed'));
    L.check(scene, '1 周目の世界状態の全要素が同一', Object.entries(world0).every(([k, v]) => deepSame(w[k], v)));
    L.check(scene, '2 周目の経路が 1 周目の経路と交わらない', expect.r2_paths.every((p) => !expect.r1_paths.includes(p)));
    // 警告は 1 周目の raw の GoTo（飛ばした Step の省略）に付くため、全体の 0 件は主張しない。無視は 0 件。
    L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);

    await runChecks(page, scene, L, rawSent);
    await finish(page, rec, scene, L);
    console.log(`C: ${L.summary()} 項目`);
    failed = L.failed();
  });
  assert.deepEqual(failed, []);
});

test('D: 1 周目の途中の履歴ではボタンが出ず、比べる部品が差を検出する（負の対照）', { timeout: 300000 }, async () => {
  let failed = [];
  await withDeliveryCopy((copy) => withCspMode({ dir: copy, csp: true }, (webDir) => {
    fs.writeFileSync(path.join(webDir, 'ir', 'main.json'), ir);
    return withServer({ dir: webDir, base: BASE }, async (srv) => {
      const { page, context } = await openPage(browser, { origin: srv.origin });
      const L = ledger();
      const scene = '対照';
      await openImported(page, srv, halfFile);
      L.check(scene, '1 周目の途中の履歴を開いた（右のログに行が出る）', (await page.locator(LOG_ITEMS).count()) > 0);
      L.check(scene, '1 周目の途中でボタンが 0 個', (await page.locator(NEXT).count()) === 0);
      L.check(scene, '位置の章が ch-round1', (await chapterOf(page)) === 'ch-round1');
      const e = await exportByUi(page);
      L.check(scene, '書き出しが 1 周目の途中のまま（begin: s-r2 を含まない）', !e.ops.some((o) => o.control?.begin === expect.start));

      // 比べる部品の自己確認（一致の判定が差を見逃さない）。
      const w = JSON.parse(JSON.stringify(expect.world));
      L.check(scene, '写しは一致と判定する', deepSame(w, expect.world) && round1Kept(w));
      const firstKey = Object.keys(w)[0];
      const changed = JSON.parse(JSON.stringify(w));
      changed[firstKey].attrs = { ...changed[firstKey].attrs, probe: 'x' };
      L.check(scene, '要素の attrs を変えた写しは不一致', !deepSame(changed, expect.world) && !round1Kept(changed));
      const missing = JSON.parse(JSON.stringify(w));
      delete missing[firstKey];
      L.check(scene, '要素を 1 件欠いた写しは「全要素が残る」を満たさない', !round1Kept(missing));
      const reopened = { snapshot: { run: { world: JSON.parse(JSON.stringify(w)) } } };
      reopened.snapshot.run.world[expect.closed_paths[0]].attrs.state = 'open';
      L.check(scene, '閉じた根を open にした写しは closed の判定が偽', !closedOk(reopened, expect));
      L.check(scene, '元の写しでは closed の判定が真', closedOk({ snapshot: { run: { world: w } } }, expect));
      console.log(`D: ${L.summary()} 項目`);
      failed = L.failed();
      await context.close();
    });
  }));
  assert.deepEqual(failed, []);
});
