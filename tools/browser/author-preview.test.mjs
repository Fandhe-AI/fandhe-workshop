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
//   E  章の開始点への直接リンク（CSP あり・なし。TASK-427 2/2）。配信物（meta のある配信する設定）を `?start=<開始点 ID>` で新しい文脈に開くと、
//      開始点 4 件それぞれで、その章の最初の Step の題・入ったばかりの状態（確認待ちでも達成でもない）・上部の位置・いまやることと本文が出る。
//      `s-0-1` は既定の開始点と同じ節点なので `?start=` が効いている証拠にならない。効きを区別できるのは `s-1-1`・`s-2-1`・`s-3-1`。
//      負の対照: 固定の文字列と、節点ではあるが開始点でない ID（`s-1-2`）は開かず、コードだけの通知が出て値は画面に出ない。
//
// 確かめないこと:
//   - 直接リンクを開いた後の再読み込み（direct-link-reload が持つ。5.204）。章 A の履歴があるときの章 B のリンク（direct-link-roundtrip。5.206）
//   - 断りの網羅（大文字・`../`・重複など。direct-link-start が持つ。5.199）
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
  STEPS, ORDER, FIRST_STEPS, CHAPTER_ENDS, LAST, domObserver03, inspect, inspectFirstStep, summarizeIr, variantOf, startLinksOf, positionOf,
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

const NOTICE = '[data-testid="shell-notice-item"][data-notice-kind="pack-unloadable"]';
const REFUSED_TEXT = '指定された開始点を開けませんでした。（start_unknown）';
const SENTINEL = 'AUTHOR-START-SENTINEL';

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`E: 章の開始点への直接リンクが、その章の最初の Step の題と状態で開く（${mode}）`, { timeout: 180000 }, async () => {
    const ir = JSON.parse(irText);
    const links = startLinksOf(ir);
    assert.equal(links.length, FIRST_STEPS.length, 'start count');
    assert.deepEqual(links.map((l) => l.node), FIRST_STEPS, 'starts follow FIRST_STEPS');
    assert.equal(new Set(links.map((l) => l.id)).size, links.length, 'start ids are distinct');
    const L = ledger();
    const failedCount = () => L.items.filter((x) => !x.ok).length;
    let linksOk = 0;
    let refusedOk = 0;
    let external = 0;
    let pageErrors = 0;
    await withCspMode({ dir: webDir, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const open = async (query) => {
        const o = await openPage(browser, { origin: srv.origin });
        await o.page.goto(srv.url + query);
        await waitMounted(o.page);
        return o;
      };
      for (let i = 0; i < links.length; i += 1) {
        const { id, node: uid } = links[i];
        const scene = `リンク ${i + 1} ${id}`;
        const { page, context, rec } = await open(`?start=${encodeURIComponent(id)}`);
        try {
          const before = failedCount();
          L.check(scene, '配信物に試験用の口が無い', (await page.evaluate(() => typeof window.__fandheTest)) === 'undefined');
          L.check(scene, '開始点の節点が入ったばかりで示される', (await inspect(page, uid, 'entered')).ok === true);
          const next = ORDER[ORDER.indexOf(uid) + 1];
          L.check(scene, '次の Step を示さない', (await inspect(page, next, 'entered')).ok === false);
          for (const o of FIRST_STEPS.filter((u) => u !== uid)) {
            L.check(scene, `ほかの章の開始点 ${o} を示さない`, (await inspect(page, o, 'entered')).ok === false);
          }
          for (const s of ['accepted', 'achieved']) L.check(scene, `${s} を示さない`, (await inspect(page, uid, s)).ok === false);
          L.check(scene, '位置の文字', (await page.textContent(POSITION)) === positionOf(ir, uid));
          const checks = await inspectFirstStep(page, stepDo(uid), STEPS.find(([u]) => u === uid)[1]);
          for (const [name, ok] of checks) L.check(scene, name, ok);
          L.check(scene, '通知が 0 件', (await page.locator(NOTICE).count()) === 0);
          const got = rec.requests.filter((r) => new URL(r.url).pathname.endsWith('/ir/main.json'));
          L.check(scene, 'ir/main.json が 200 で 1 回取得', got.length === 1 && got[0].status === 200);
          const counts = await collectSafety(page, rec);
          L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
          L.check(scene, '配信元以外への通信が 0 件', rec.external.length === 0);
          L.check(scene, 'ページ内エラーが 0 件', rec.pageErrors.length === 0 && !rec.crashed);
          L.check(scene, 'コンソールの error が 0 件', rec.consoleErrors.length === 0);
          L.check(scene, 'コンソールの CSP 警告が 0 件', cspConsoleMessages(rec).length === 0);
          external += rec.external.length;
          pageErrors += rec.pageErrors.length;
          if (failedCount() === before) linksOk += 1;
        } finally {
          await context.close();
        }
      }
      // 負の対照: 固定の文字列と、節点だが開始点でない ID は開かず、コードだけの通知が出て値は出ない。
      for (const [value, name] of [[SENTINEL, '固定の文字列'], ['s-1-2', '節点だが開始点でない ID']]) {
        const scene = `断り ${name}`;
        const { page, context, rec } = await open(`?start=${encodeURIComponent(value)}`);
        try {
          const before = failedCount();
          L.check(scene, '通知が固定の文言とコードだけ', (await page.locator(NOTICE).innerText()) === REFUSED_TEXT);
          L.check(scene, 'Step の題が 0 件', (await page.locator('[data-testid="step-title"]').count()) === 0);
          const shown = await page.evaluate((v) => (document.body.textContent ?? '').includes(v) || document.title.includes(v), value);
          L.check(scene, '値が画面とタイトルに出ない', shown === false);
          if (value === 's-1-2') L.check(scene, 'main/st-1-2 の画面にならない', (await inspect(page, 'main/st-1-2', 'entered')).ok === false);
          L.check(scene, 'ページ内エラーが 0 件', rec.pageErrors.length === 0 && !rec.crashed);
          external += rec.external.length;
          pageErrors += rec.pageErrors.length;
          if (failedCount() === before) refusedOk += 1;
        } finally {
          await context.close();
        }
      }
    }));
    console.log(`AUTHOR-DIRECT-LINKS csp=${csp ? 'on' : 'off'} links=${linksOk}/${links.length} refused=${refusedOk}/2 external=${external} `
      + `page-errors=${pageErrors} ledger=${L.items.filter((i) => i.ok).length}/${L.items.length}`);
    assert.deepEqual(L.failed(), []);
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
