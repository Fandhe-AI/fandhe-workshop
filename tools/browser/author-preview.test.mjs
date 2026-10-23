// compile した IR の手元の配信と、CH-0〜CH-3 の完走の試験（TASK-427 1/2。AUTHOR-21）。
//
// 作成者の手順そのものを自動ブラウザで通す。形式 B の教材パック → `fandhe-pack compile` → `IR_FILE=<IR> tools/build-web.sh`
// （配信する設定。試験用の口なし）→ `tools/serve.mjs` によるサブパス・gzip の配信 → 実クリックでの 20 Step の完走。
// ビルドは scripts/test-browser.sh が行い、本試験は結果の配信物（FANDHE_WEB_AUTHOR_DIR）と compile した IR（FANDHE_AUTHOR_IR）を受ける。
//
// 解釈（最大の限界）: 教材 repo の本物の CH-0〜CH-3（TASK-643・TASK-642）がまだ無い。そのため教材は、手書きの
// `crates/web/tests/data/ch0_3/ir.json`（20 Step）と同じ形を形式 B で書いた試験用の教材
// （`crates/web/tests/data/ch0_3_pack`）である。手順は端から端まで実物で流すが、本物の教材での流し直しは未実施
// （`docs/testing.md` の 5.292）。試作の 73 項目は再現せず、台帳の件数は「N/N 項目」と書く。AI の代理の確認であり、人の評価には数えない（D-34）。
//
// 確かめること:
//   P1 `validate` が終了コード 0。P2 もう一度 compile した出力が、配信物の ir/main.json と受け取った IR のどちらともバイト単位で一致する。
//   P3 配信物の index.html に fandhe-ir の meta がちょうど 1 個で、試験用の口（meta・test-hooks.js）が無い。
//   P4 compile した IR の構造が、手書きの IR の変種（各章の最後を final にし、開始点を章の先頭 4 件にしたもの）と一致する。
//   P5 STEPS の題が IR の題と一致し、20 件とも違う。
//   B  配信物の画面（CSP あり・なし）を DOM だけの観察で、サブパスの下で最後の Step の達成まで通す。各章の最初の Step に
//      「いまやること」と本文が出る。章の境目で「次の章へ進む」がちょうど 1 個。配信元以外への通信・ページ内エラー 0 件。
//   C  DOM の観察の較正。試験の設定へ同じ IR を入れ、各時点で DOM が stateJson と一致し、1 Step 先・別の状態を示さない。
//   D  負の対照。本文を隠す・do を書き換えると、先頭の Step の確認が不合格になる。
//
// 確かめないこと:
//   - 章の開始点への直接リンクと作成者向けの文書（TASK-427 2/2）
//   - 本物の CH-0〜CH-3（TASK-643・TASK-642）
//   - キーボードだけの完走（ch0-3-run が持つ）。処理中の Esc による停止（5.117 の限界）
//   - 人が作成者として画面を確かめた記録（仕様が「含まない」と定める）
//
// 出力は名前・件数・合否だけで、入力した値と本文は出さない。書き込むパスは一時ディレクトリの固定のパスだけ。
// 入力: FANDHE_PACK_BIN・FANDHE_WEB_AUTHOR_DIR・FANDHE_AUTHOR_IR・FANDHE_WEB_TEST_DIR。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages,
} from './harness.mjs';
import { ledger, mouseDriver, journey, install, engine } from './ch0-3-journey.mjs';
import {
  STEPS, ORDER, FIRST_STEPS, CHAPTER_ENDS, LAST, domObserver03, inspect, inspectFirstStep, summarizeIr, variantOf,
} from './author-preview.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PACK = path.resolve(HERE, '../../crates/web/tests/data/ch0_3_pack');
const HAND_IR = path.resolve(HERE, '../../crates/web/tests/data/ch0_3/ir.json');
const BASE = '/fandhe-author-preview/';
const POSITION = '[data-testid="shell-position"]';
let browser;
let bin;
let webDir;
let irFile;
let irText;

function assertDir(name, what) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${what}`);
  return dir;
}

before(async () => {
  bin = packBinFromEnv();
  webDir = assertDir('FANDHE_WEB_AUTHOR_DIR', 'a web directory built with IR_FILE and without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  irFile = process.env.FANDHE_AUTHOR_IR;
  assert.ok(irFile && fs.statSync(irFile, { throwIfNoEntry: false })?.isFile(), 'FANDHE_AUTHOR_IR must point to the compiled IR');
  irText = fs.readFileSync(irFile, 'utf8');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const run = (args) => spawnSync(bin, args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });

test('P1: 教材パックの validate が終了コード 0', () => {
  const r = run(['validate', PACK]);
  assert.equal(r.status, 0, `validate exit ${r.status}`);
});

test('P2: もう一度 compile した出力が配信物の ir/main.json と受け取った IR のどちらともバイト単位で一致する', () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-author-preview-'));
  try {
    const out = path.join(work, 'again.json');
    const r = run(['compile', PACK, '-o', out]);
    assert.equal(r.status, 0, `compile exit ${r.status}`);
    const again = fs.readFileSync(out);
    assert.ok(again.equals(fs.readFileSync(path.join(webDir, 'ir', 'main.json'))), 'delivered ir/main.json differs from compile output');
    assert.ok(again.equals(fs.readFileSync(irFile)), 'received IR differs from compile output');
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
});

test('P3: 配信物の index.html に fandhe-ir の meta が 1 個で、試験用の口が無い', () => {
  const html = fs.readFileSync(path.join(webDir, 'index.html'), 'utf8');
  assert.equal((html.match(/<meta\s+name="fandhe-ir"\s+content="1">/g) ?? []).length, 1, 'one fandhe-ir meta');
  assert.ok(!/fandhe-test-hooks/.test(html), 'no test-hooks meta');
  assert.ok(!fs.existsSync(path.join(webDir, 'test-hooks.js')), 'no test-hooks.js');
});

test('P4: compile した IR の構造が手書きの IR の変種と一致する', () => {
  const got = summarizeIr(JSON.parse(irText));
  const want = summarizeIr(variantOf(JSON.parse(fs.readFileSync(HAND_IR, 'utf8'))));
  assert.deepEqual(got.starts, want.starts, 'starts');
  assert.equal(got.nodes.length, want.nodes.length, 'node count');
  for (let i = 0; i < want.nodes.length; i += 1) {
    assert.deepEqual(got.nodes[i], want.nodes[i], `node ${want.nodes[i].uid}`);
  }
});

test('P5: STEPS の題が IR の題と一致し、20 件とも違う', () => {
  const ir = JSON.parse(irText);
  assert.deepEqual(ir.nodes.map((n) => [n.uid, n.step.title]), STEPS);
  assert.equal(new Set(STEPS.map(([, t]) => t)).size, STEPS.length);
  assert.equal(STEPS.length, 20);
});

const stepDo = (uid) => JSON.parse(irText).nodes.find((n) => n.uid === uid).step.do;

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`B: 配信物をサブパスの下で配り、DOM だけの観察で 20 Step を最後まで通す（${mode}）`, { timeout: 300000 }, async () => {
    let failed = [];
    let line = '';
    await withCspMode({ dir: webDir, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      const L = ledger();
      const scene = '配信';
      await page.goto(srv.url);
      await waitMounted(page);
      assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'delivery build must have no test hooks');
      await install(page);
      let firstOk = 0;
      let reached = 0;
      let crossings = 0;
      const onStep = async (uid, status) => {
        if (status !== 'entered') return;
        reached += 1;
        if (!FIRST_STEPS.includes(uid)) return;
        const checks = await inspectFirstStep(page, stepDo(uid), STEPS.find(([u]) => u === uid)[1]);
        for (const [name, ok] of checks) L.check(scene, `${uid} ${name}`, ok);
        if (checks.every(([, ok]) => ok)) firstOk += 1;
      };
      // 章の境目の回数は、章の先頭に入った回数から数える（先頭の章を除く）。
      const counted = async (uid, status) => {
        if (status === 'entered' && FIRST_STEPS.includes(uid) && uid !== FIRST_STEPS[0]) crossings += 1;
        await onStep(uid, status);
      };
      await journey(page, mouseDriver(page), scene, L, null, { chaptered: true, observe: domObserver03(), onStep: counted });
      L.check(scene, `${STEPS.length} Step に入った`, reached === STEPS.length);
      L.check(scene, '章の境目が 3 回', crossings === CHAPTER_ENDS.length);
      const review = await page.evaluate(() => [...document.querySelectorAll('[data-testid="part-review-list"] > li')]
        .map((li) => ({ step: li.dataset.step, achieved: li.dataset.achieved, skipped: li.dataset.skipped })));
      L.check(scene, '復習の行で 20 Step がすべて達成', review.length === STEPS.length && ORDER.every((u) => review.some((r) => r.step === u && r.achieved === 'true')));
      L.check(scene, '復習の行で省略が 0 件', review.every((r) => r.skipped === 'false'));
      L.check(scene, '最後の位置の表示が ch-3', ((await page.textContent(POSITION)) ?? '').includes('ch-3'));
      const ir = rec.requests.filter((r) => new URL(r.url).pathname.endsWith('/ir/main.json'));
      L.check(scene, 'ir/main.json が 200 で 1 回取得', ir.length === 1 && ir[0].status === 200);
      const marks = await page.evaluate(() => window.__run);
      L.check(scene, 'raw-trigger の click が 0 回', marks.raw === 0);
      L.check(scene, '実マウスの操作が通っている', marks.mouse > 0);
      const counts = await collectSafety(page, rec);
      for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
      L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
      L.check(scene, '配信元以外への通信が 0 件', rec.external.length === 0);
      L.check(scene, 'ページ内エラーが 0 件', rec.pageErrors.length === 0 && !rec.crashed);
      L.check(scene, 'コンソールの error が 0 件', rec.consoleErrors.length === 0);
      L.check(scene, 'コンソールの CSP 警告が 0 件', cspConsoleMessages(rec).length === 0);
      failed = L.failed();
      line = `AUTHOR-PREVIEW csp=${csp ? 'on' : 'off'} steps=${reached}/${STEPS.length} crossings=${crossings} first-steps=${firstOk}/${FIRST_STEPS.length} `
        + `external=${rec.external.length} page-errors=${rec.pageErrors.length} ledger=${L.items.filter((i) => i.ok).length}/${L.items.length}`;
      await context.close();
    }));
    console.log(line);
    assert.deepEqual(failed, []);
  });
}

async function openTestHooks(fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const n0 = await engine(page).then((e) => e.installCount);
    assert.equal(await page.evaluate((t) => window.__fandheTest.loadIr(t), irText), true, 'load-ir sent');
    await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.installCount > n, n0, { timeout: 15000 });
    await install(page);
    try {
      await fn(page);
    } finally {
      await context.close();
    }
  }));
}

test('C: DOM の観察の目印が試験の設定の stateJson と各時点で一致する（較正）', { timeout: 300000 }, async () => {
  const dom = domObserver03();
  const L = ledger();
  await openTestHooks(async (page) => {
    const scene = '較正';
    const wrapped = {
      hasState: true,
      stillAt: dom.stillAt,
      async at(pg, uid, status) {
        await dom.at(pg, uid, status);
        const e = await engine(pg);
        const st = e.stateWithoutLog.run.slices.main?.status?.[uid] ?? 'entered';
        L.check(scene, `${uid} ${status} が stateJson と一致`, e.current === uid && st === status);
        const nextUid = ORDER[ORDER.indexOf(uid) + 1];
        if (nextUid) L.check(scene, `${uid} ${status} の時点で次の Step を示さない`, (await inspect(pg, nextUid, 'entered')).ok === false);
        for (const other of ['entered', 'accepted', 'achieved'].filter((s) => s !== status)) {
          L.check(scene, `${uid} ${status} の時点で ${other} を示さない`, (await inspect(pg, uid, other)).ok === false);
        }
      },
    };
    await journey(page, mouseDriver(page), scene, L, null, { chaptered: true, observe: wrapped });
  });
  console.log(`C: ${L.summary()} 項目`);
  assert.deepEqual(L.failed(), []);
});

test('D: 本文を隠す・do を書き換えると先頭の Step の確認が不合格になる（負の対照）', { timeout: 120000 }, async () => {
  await withCspMode({ dir: webDir, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      const [uid] = FIRST_STEPS;
      const title = STEPS[0][1];
      await page.waitForSelector('[data-testid="step-body"]');
      const pass = await inspectFirstStep(page, stepDo(uid), title);
      assert.deepEqual(pass.filter(([, ok]) => !ok), [], 'baseline must pass');
      await page.evaluate(() => { document.querySelector('[data-testid="step-body"]').hidden = true; });
      const hidden = await inspectFirstStep(page, stepDo(uid), title);
      assert.ok(hidden.some(([n, ok]) => n === '本文の区画が見える' && !ok), 'hidden body must fail');
      await page.evaluate(() => {
        document.querySelector('[data-testid="step-body"]').hidden = false;
        document.querySelector('[data-testid="step-do"]').textContent = '書き換えた文';
      });
      const changed = await inspectFirstStep(page, stepDo(uid), title);
      assert.ok(changed.some(([n, ok]) => n === 'いまやることの見出しと do' && !ok), 'rewritten do must fail');
    } finally {
      await context.close();
    }
  }));
});
