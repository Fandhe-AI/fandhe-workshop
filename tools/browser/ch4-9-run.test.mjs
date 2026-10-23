// CH-4 から CH-9 の ST-9.1 までを自動ブラウザで通す試験（TASK-674 1/2。ROUND1-1・ART-24・SHELL-10）。
//
// 解釈（最大の限界）: 教材 repo の本物の CH-4〜CH-9（TASK-672）がまだ無い。そのため本試験は ch0-3-run.test.mjs と同じく、
// 章ごとの入力方式・確認の種類・拒否・処理中・ビューの種類の**形**を写した試験用の教材（`crates/web/tests/data/ch4_9/ir.json`。
// 17 Step・6 章。`pack.id` は `ch4-9-run`）を通す。受け入れ条件は「形」について満たすもので、本物での流し直しは未実施
// （`docs/testing.md` の 5.283）。AI の代理の確認であり、人の評価には数えない（D-34）。
//
// 確かめること:
//   A 試験の設定（TEST_HOOKS=1。`?fixture=ch4-9`）で、実クリックと実キーだけ（`raw-trigger`・`load-fixture:` は 0 回）で
//     CH-4 の先頭から CH-9 の ST-9.1 の達成まで通す（CSP あり・なし）。ページ内エラー・配信元以外への通信・ダイアログ・
//     危険な要素・CSP 違反・コンソールの error が 0 件。CH-4〜CH-8 の各章の最後の達成の後に「次の章へ進む」がちょうど 1 個
//     （計 5 回押す）で「移動」ボタンが 0 件、章の途中と ST-9.1 の達成の後は 0 個。全 Step の画面にパスワード欄が 0 件。
//   B 配信する設定（試験用の口なし）の一時の写しに教材の IR を置いて、DOM だけの観察で同じ通しを CH-9 まで通す。本 Issue の主張の本体。
//   C DOM の目印の較正。B と同じ DOM の観察が、各時点で試験の設定の stateJson と一致し、1 Step 先を示さない。B の空振りを防ぐ。
//   D パスワード欄の検出器の対照。type=password・autocomplete・「パスワード」のラベルつきの欄を 1 件ずつ足すと検出数が 1 になり、
//     除くと 0 に戻る。製品の左の成果物は行を描くだけで入力欄を持たないので、本検査は構造上ほぼ自明に通る。検出器が働くことは D で示す。
//
// 確かめないこと:
//   - 配信元以外への通信の正式な計数と記録。csp-runs.test.mjs の T5・D2 が TASK-151 の道具で数える（TASK-674 2/2。
//     `docs/testing.md` 5.285）。ここでは台帳に 0 件の 1 項目だけを置く
//   - 本物の CH-4〜CH-9。分岐の Step 化・止まる場面の主経路・実際の保存と書き出しの Step・CH-10 への周回（TASK-254）
//   - 処理中の送信が busy で拒否されること（ブラウザでは時間の窓に頼る。native の ch4_9_run_host.rs が受け持つ）
//   - 全周のキーボード完走（TASK-92）
//   - 試験の出力に、入力した値や下書きの中身は出さない（名前・件数・合否だけ）
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信する設定）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages,
} from './harness.mjs';
import {
  FIXTURE_QUERY, STEPS, ORDER, LAST, ledger, mouseDriver, install, journey49, hookObserver49, domObserver49, inspectDom49,
  passwordFields, positionText, withDeliveryCopy,
} from './ch4-9-journey.mjs';
import { engine } from './ch0-3-journey.mjs';

const IR_PATH = fileURLToPath(new URL('../../crates/web/tests/data/ch4_9/ir.json', import.meta.url));
const BASE = '/fandhe-ch4-9-run/';
const NEXT_PRESSES = 5;
let browser;

function assertDir(name, what) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${what}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assert.ok(fs.existsSync(IR_PATH), 'ch4_9 ir.json is missing');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 共通の終わりの確認。台帳に項目名と合否だけを積む。`hook` が真なら試験の設定の `__fandheTest` で警告・無視・省略を数える。
async function finish(page, rec, scene, L, { hook, presses }) {
  const run = hook ? await page.evaluate(() => window.__run) : null;
  if (run) L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
  if (run) L.check(scene, '実マウスの操作が通っている', run.mouse > 0);
  L.check(scene, `次の章へ進むを押した回数が ${NEXT_PRESSES}`, presses === NEXT_PRESSES);
  if (hook) {
    const e = await engine(page);
    L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
    L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
    // 投影の行（e.log）には Event::Skipped が出ない。省略は Step ごとの見直し項目（e.review）の skipped に現れる。
    L.check(scene, '省略した Step が 0 件', e.review.filter((r) => r.skipped).length === 0);
    const status = e.stateWithoutLog.run.slices.main.status;
    L.check(scene, '最後の Step が達成', e.current === LAST && status[LAST] === 'achieved');
    L.check(scene, `${STEPS.length} Step がすべて達成`, Object.keys(status).length === STEPS.length && Object.values(status).every((s) => s === 'achieved'));
    L.check(scene, `達成が ${STEPS.length} 件`, e.log.filter((r) => r.kind === 'achieved').length === STEPS.length);
  }
  const counts = await collectSafety(page, rec);
  for (const k of nonZero(counts)) L.check(scene, `安全の計数 ${k} が 0`, false);
  L.check(scene, '安全の計数がすべて 0', nonZero(counts).length === 0);
  L.check(scene, 'ページ内エラーが 0 件', rec.pageErrors.length === 0 && !rec.crashed);
  L.check(scene, 'コンソールの error が 0 件', rec.consoleErrors.length === 0);
  L.check(scene, 'コンソールの CSP 警告が 0 件', cspConsoleMessages(rec).length === 0);
  L.check(scene, '配信元の外への通信が 0 件', rec.external.length === 0);
}

// 画面の観察の間にパスワード欄を数える口。最大値と観察した回数を持つ。
function passwordProbe(page) {
  const seen = { max: 0, n: 0 };
  return {
    seen,
    onStep: async () => {
      seen.n += 1;
      seen.max = Math.max(seen.max, await page.evaluate(passwordFields));
    },
  };
}

async function openFixture(csp, fn) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    const L = ledger();
    await page.goto(`${srv.url}${FIXTURE_QUERY}`);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    await install(page);
    await fn(page, rec, L);
    await context.close();
    return L;
  }));
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`A: ${STEPS.length} Step を実クリックと実キーだけで CH-9 まで通す（試験の設定。${mode}）`, { timeout: 300000 }, async () => {
    let summary = '';
    let failed = [];
    await openFixture(csp, async (page, rec, L) => {
      const scene = '通し';
      const probe = passwordProbe(page);
      const { presses } = await journey49(page, mouseDriver(page), scene, L, { observe: hookObserver49, onStep: probe.onStep });
      await finish(page, rec, scene, L, { hook: true, presses });
      L.check(scene, 'パスワード欄が全 Step で 0 件', probe.seen.n >= STEPS.length && probe.seen.max === 0);
      L.check(scene, '最後の位置の表示が CH-9', (await positionText(page)).includes('ch-9'));
      summary = L.summary();
      failed = L.failed();
      console.log(`A (${mode}): ${summary} 項目`);
    });
    assert.deepEqual(failed, []);
  });
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`B: 配信する設定の画面を DOM だけの観察で CH-9 まで通す（${mode}）`, { timeout: 300000 }, async () => {
    let failed = [];
    await withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, (dir) => {
      fs.writeFileSync(path.join(dir, 'ir', 'main.json'), fs.readFileSync(IR_PATH));
      return withServer({ dir, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        const L = ledger();
        const scene = '配信';
        await page.goto(srv.url);
        await waitMounted(page);
        assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'delivery build must have no test hooks');
        await install(page);
        const probe = passwordProbe(page);
        const { presses } = await journey49(page, mouseDriver(page), scene, L, { observe: domObserver49(), onStep: probe.onStep });
        await finish(page, rec, scene, L, { hook: false, presses });
        const run = await page.evaluate(() => window.__run);
        L.check(scene, 'raw-trigger の click が 0 回', run.raw === 0);
        L.check(scene, '実マウスの操作が通っている', run.mouse > 0);
        L.check(scene, 'パスワード欄が全 Step で 0 件', probe.seen.n >= STEPS.length && probe.seen.max === 0);
        // 復習の部品の行で、全 Step が達成・省略 0 件（DOM だけの判定）。
        const review = await page.evaluate(() => [...document.querySelectorAll('[data-testid="part-review-list"] > li')]
          .map((li) => ({ step: li.dataset.step, achieved: li.dataset.achieved, skipped: li.dataset.skipped })));
        L.check(scene, `復習の行で ${STEPS.length} Step がすべて達成`, review.length === STEPS.length && ORDER.every((u) => review.some((r) => r.step === u && r.achieved === 'true')));
        L.check(scene, '復習の行で省略が 0 件', review.every((r) => r.skipped === 'false'));
        L.check(scene, '最後の位置の表示が CH-9', (await positionText(page)).includes('ch-9'));
        const ir = rec.requests.filter((r) => new URL(r.url).pathname.endsWith('/ir/main.json'));
        L.check(scene, 'ir/main.json が 200 で 1 回取得', ir.length === 1 && ir[0].status === 200);
        console.log(`B (${mode}): ${L.summary()} 項目`);
        failed = L.failed();
        await context.close();
      });
    }));
    assert.deepEqual(failed, []);
  });
}

test('C: DOM の観察の目印が試験の設定の stateJson と各時点で一致する（較正）', { timeout: 300000 }, async () => {
  const dom = domObserver49();
  const titles = Object.fromEntries(STEPS);
  let failed = [];
  await openFixture(true, async (page, rec, L) => {
    const scene = '較正';
    // 観察を包む。DOM が示した後に、同じ時点の stateJson の現在の Step・状態と照らし、1 Step 先を示さないことも確かめる。
    const wrapped = {
      hasState: true,
      stillAt: dom.stillAt,
      async at(pg, uid, status) {
        await dom.at(pg, uid, status);
        const e = await engine(pg);
        const st = e.stateWithoutLog.run.slices.main?.status?.[uid] ?? 'entered';
        L.check(scene, `${uid} ${status} が stateJson と一致`, e.current === uid && st === status);
        const nextUid = ORDER[ORDER.indexOf(uid) + 1];
        if (nextUid) {
          const { ok } = await pg.evaluate(inspectDom49, [titles, nextUid, 'entered']);
          L.check(scene, `${uid} ${status} の時点で次の Step を示さない`, ok === false);
        }
        // 同じ Step の別の状態を示さない（入力待ちと確認待ち・達成の取り違えを防ぐ）。
        for (const other of ['entered', 'accepted', 'achieved'].filter((s) => s !== status)) {
          const { ok } = await pg.evaluate(inspectDom49, [titles, uid, other]);
          L.check(scene, `${uid} ${status} の時点で ${other} を示さない`, ok === false);
        }
      },
    };
    await journey49(page, mouseDriver(page), scene, L, { observe: wrapped });
    console.log(`C: ${L.summary()} 項目`);
    failed = L.failed();
  });
  assert.deepEqual(failed, []);
});

test('D: パスワード欄の検出器が 4 形の欄を検出し、除くと 0 に戻る（対照）', { timeout: 300000 }, async () => {
  let failed = [];
  await openFixture(true, async (page, rec, L) => {
    const scene = '対照';
    let tried = false;
    const observe = {
      ...hookObserver49,
      async at(pg, uid, status) {
        await hookObserver49.at(pg, uid, status);
        if (uid !== 'main/st-8-1' || status !== 'entered' || tried) return;
        tried = true;
        // 模擬ログインの Step の画面で、中身の無い欄を 1 件ずつ足す。送信も通信もしない。
        const probes = {
          'type=password': { type: 'password' },
          'autocomplete=current-password': { type: 'text', autocomplete: 'current-password' },
          'label=パスワード': { type: 'text', labelText: 'パスワード' },
          'type=PASSWORD（大文字）': { type: 'PASSWORD' },
        };
        L.check(scene, '足す前は 0 件', (await pg.evaluate(passwordFields)) === 0);
        for (const [name, spec] of Object.entries(probes)) {
          await pg.evaluate((s) => {
            const input = document.createElement('input');
            input.id = 'probe-pw';
            input.setAttribute('type', s.type);
            if (s.autocomplete) input.setAttribute('autocomplete', s.autocomplete);
            document.body.append(input);
            if (s.labelText) {
              const label = document.createElement('label');
              label.id = 'probe-pw-label';
              label.setAttribute('for', 'probe-pw');
              label.textContent = s.labelText;
              document.body.append(label);
            }
          }, spec);
          L.check(scene, `${name} を 1 件検出`, (await pg.evaluate(passwordFields)) === 1);
          await pg.evaluate(() => {
            document.getElementById('probe-pw')?.remove();
            document.getElementById('probe-pw-label')?.remove();
          });
          L.check(scene, `${name} を除くと 0 件に戻る`, (await pg.evaluate(passwordFields)) === 0);
        }
      },
    };
    await journey49(page, mouseDriver(page), scene, L, { observe });
    L.check(scene, '対照を実行した', tried);
    L.check(scene, 'raw-trigger を使っていない', (await page.evaluate(() => window.__run)).raw === 0);
    console.log(`D: ${L.summary()} 項目`);
    failed = L.failed();
  });
  assert.deepEqual(failed, []);
});

