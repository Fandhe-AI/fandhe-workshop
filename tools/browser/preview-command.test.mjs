// `fandhe-pack preview` の URL を開いた通しと、文章を直した再実行の反映の試験（TASK-429 3/3。AUTHOR-22）。
//
// 作成者の手順（教材パックの文章を直す → `preview` をもう一度実行する → 新しい URL を開く）を、同梱つきの実行ファイルと
// 自動ブラウザで通す。実行ファイルは scripts/test-browser.sh が `scripts/build-pack-preview.sh` で作り、
// FANDHE_PACK_PREVIEW_BIN で渡す。`validate`・`compile` の照合に使う通常の実行ファイルは FANDHE_PACK_BIN で受ける。
// 未設定・不在は skip にせず失敗にする（fail-closed）。
//
// 解釈: 「文章の修正」は Step の本文（Markdown）の 1 文を直すことと読み、`st-0-1`（既定の開始点）と `st-1-1`（`?start=s-1-1`）で確かめた。
// 同梱物は CSP の meta つきの `web-author` を `include_bytes!` で固めたもので、CSP なしの写しを作れない。そのため CSP ありだけを流す。
// 教材は手書きの IR と同じ形を形式 B で書いた試験用の教材（crates/web/tests/data/ch0_3_pack）で、本物の CH-0〜CH-3 での流し直しは未実施
// （docs/testing.md 5.428）。AI の代理の確認であり、人の評価には数えない（D-34）。
//
// 確かめること:
//   A 診断あり: 同じ写しで `validate` が 1、`preview` が時間内に 1 で終わり、URL を出さず、標準出力が `validate` と一致し、標準エラー出力が空。
//   B 診断なし: URL（127.0.0.1・/preview/）を開き、DOM だけの観察で 20 Step を実マウスで最後まで通す（AUTHOR-21 の B と同じ台帳）。
//     画面の取得した ir/main.json が `compile` の出力とバイト単位で一致する。配信元以外への通信・ページ内エラー 0 件。
//   C 再実行: 文を直して実行し直すと、新しい URL の画面に直した文が出て直す前の文は出ない（既定の開始点と `?start=s-1-1`）。
//     取得した IR が直した写しの `compile` の出力と一致し、直す前と違う。見出し（題）は変わらない。
//     負の対照: 直さずに実行し直すと同じ文・同じ IR になる。診断を挟む流れ（壊す → 1 で終わる → 直す → 反映）も通す。
//
// 確かめないこと:
//   - 前の URL が読めなくなること（OS が同じポートを割り当て直すと揺れる）。ファイルの監視・自動の再読み込み（決定の外）
//   - CSP なしの画面、本物の CH-0〜CH-3、人が作成者として画面を確かめた記録
//
// 出力は名前・件数・合否だけで、直した文の中身は出さない。書き込むのは一時ディレクトリの教材の写しだけで、元の教材は変えない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, previewBinFromEnv, openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages,
} from './harness.mjs';
import { ledger, mouseDriver, journey, install } from './ch0-3-journey.mjs';
import {
  STEPS, FIRST_STEPS, CHAPTER_ENDS, ORDER, domObserver03, inspectFirstStep,
} from './author-preview.mjs';
import {
  copyPack, rewriteSentence, sentenceOf, startPreview, runPreviewToExit,
} from './preview-command.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PACK = path.resolve(HERE, '../../crates/web/tests/data/ch0_3_pack');
const POSITION = '[data-testid="shell-position"]';
const BODY = '[data-testid="step-body"]';
const ST_0_1 = path.join('chapters', 'ch-0', 'st-0-1.md');
const ST_1_1 = path.join('chapters', 'ch-1', 'st-1-1.md');
let browser;
let packBin;
let previewBin;

before(async () => {
  packBin = packBinFromEnv();
  previewBin = previewBinFromEnv();
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const run = (args) => spawnSync(packBin, args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
const compileOf = (dir) => {
  const r = spawnSync(packBin, ['compile', dir], { maxBuffer: 64 * 1024 * 1024 });
  assert.equal(r.status, 0, `compile exit ${r.status}`);
  return r.stdout;
};
const fetchIr = async (origin) => {
  const res = await fetch(`${origin}/preview/ir/main.json`);
  assert.equal(res.status, 200, 'ir/main.json status');
  return Buffer.from(await res.arrayBuffer());
};
const stepTitle = (uid) => STEPS.find(([u]) => u === uid)[1];
const stepDo = (ir, uid) => JSON.parse(ir.toString('utf8')).nodes.find((n) => n.uid === uid).step.do;
const uniqueSentence = (tag) => `修正後の本文 ${tag}-${crypto.randomBytes(6).toString('hex')}。`;

/** preview を起動して新しい文脈で開き、fn に渡す。終わりに文脈を閉じ、preview を止める。 */
async function withPreviewPage(packDir, query, fn) {
  const srv = await startPreview(previewBin, packDir);
  try {
    assert.ok(srv.url.startsWith('http://127.0.0.1:') && srv.url.endsWith('/preview/'), 'URL shape');
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url + query);
      await waitMounted(page);
      await page.waitForSelector(BODY);
      return await fn({ page, rec, srv });
    } finally {
      await context.close();
    }
  } finally {
    await srv.stop();
  }
}

const bodyOf = (page) => page.evaluate((sel) => document.querySelector(sel)?.textContent ?? '', BODY);

test('A: 診断のある教材は配られず、validate と同じ出力で 1 で終わる', { timeout: 60000 }, async () => {
  const work = copyPack(PACK);
  try {
    fs.rmSync(path.join(work.dir, ST_0_1));
    const v = run(['validate', work.dir]);
    assert.equal(v.status, 1, 'validate exit');
    const p = await runPreviewToExit(previewBin, work.dir);
    assert.equal(p.status, 1, 'preview exit');
    assert.ok(!/^URL:/m.test(p.stdout), 'no URL line');
    assert.equal(p.stdout, v.stdout, 'stdout equals validate');
    assert.equal(p.stderr, '', 'stderr empty');
    console.log(`PREVIEW-COMMAND scene=diagnostics exit=${p.status} url-lines=0 same-as-validate=true`);
  } finally {
    work.dispose();
  }
});

test('B: preview の URL を開き、DOM だけの観察で 20 Step を最後まで通す', { timeout: 300000 }, async () => {
  const work = copyPack(PACK);
  let failed = [];
  let line = '';
  try {
    const want = compileOf(work.dir);
    await withPreviewPage(work.dir, '', async ({ page, rec, srv }) => {
      const L = ledger();
      const scene = 'preview';
      L.check(scene, '配信した ir/main.json が compile の出力と一致', (await fetchIr(srv.origin)).equals(want));
      L.check(scene, '試験用の口が無い', (await page.evaluate(() => typeof window.__fandheTest)) === 'undefined');
      await install(page);
      let firstOk = 0;
      let reached = 0;
      let crossings = 0;
      const onStep = async (uid, status) => {
        if (status !== 'entered') return;
        reached += 1;
        if (FIRST_STEPS.includes(uid) && uid !== FIRST_STEPS[0]) crossings += 1;
        if (!FIRST_STEPS.includes(uid)) return;
        const checks = await inspectFirstStep(page, stepDo(want, uid), stepTitle(uid));
        for (const [name, ok] of checks) L.check(scene, `${uid} ${name}`, ok);
        if (checks.every(([, ok]) => ok)) firstOk += 1;
      };
      await journey(page, mouseDriver(page), scene, L, null, { chaptered: true, observe: domObserver03(), onStep });
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
      line = `PREVIEW-COMMAND scene=journey steps=${reached}/${STEPS.length} crossings=${crossings} first-steps=${firstOk}/${FIRST_STEPS.length} `
        + `external=${rec.external.length} page-errors=${rec.pageErrors.length} ledger=${L.items.filter((i) => i.ok).length}/${L.items.length}`;
    });
  } finally {
    work.dispose();
  }
  console.log(line);
  assert.deepEqual(failed, []);
});

test('C: 文章を直して preview を実行し直すと、新しい URL の画面に直した文が出る', { timeout: 240000 }, async () => {
  const work = copyPack(PACK);
  const L = ledger();
  let reflected = 0;
  let total = 0;
  try {
    const f01 = path.join(work.dir, ST_0_1);
    const f11 = path.join(work.dir, ST_1_1);
    const old01 = sentenceOf(f01);
    const old11 = sentenceOf(f11);

    // C-1: 1 回目。元の文が出る。
    const first = await withPreviewPage(work.dir, '', async ({ page, srv }) => {
      const scene = '1 回目';
      L.check(scene, '元の文が出る', (await bodyOf(page)).includes(old01));
      return fetchIr(srv.origin);
    });

    // C-5（負の対照）: 直さずに実行し直す。同じ文・同じ IR になり、観察が「実行し直しただけ」と「直した」を見分けられる。
    await withPreviewPage(work.dir, '', async ({ page, srv }) => {
      const scene = '対照（直さない）';
      L.check(scene, '元の文が出る', (await bodyOf(page)).includes(old01));
      L.check(scene, 'ir/main.json が 1 回目とバイト単位で同じ', (await fetchIr(srv.origin)).equals(first));
    });

    // C-2/C-3: st-0-1 の 1 文を直して実行し直す。
    const new01 = uniqueSentence('R01');
    rewriteSentence(f01, new01);
    const want = compileOf(work.dir);
    await withPreviewPage(work.dir, '', async ({ page, rec, srv }) => {
      const scene = '再実行 st-0-1';
      const body = await bodyOf(page);
      const got = await fetchIr(srv.origin);
      const checks = [
        ['直した文が出る', body.includes(new01)],
        ['直す前の文が出ない', !body.includes(old01)],
        ['ir/main.json が直した写しの compile の出力と一致', got.equals(want)],
        ['ir/main.json が 1 回目と違う', !got.equals(first)],
        ['配信元以外への通信が 0 件', rec.external.length === 0],
        ['ページ内エラーが 0 件', rec.pageErrors.length === 0 && !rec.crashed],
      ];
      for (const [name, ok] of checks) L.check(scene, name, ok);
      for (const [name, ok] of await inspectFirstStep(page, stepDo(want, FIRST_STEPS[0]), stepTitle(FIRST_STEPS[0]))) L.check(scene, name, ok);
      total += checks.length;
      reflected += checks.filter(([, ok]) => ok).length;
    });

    // C-4: 別の章（既定の開始点でない）の文も反映される。
    const new11 = uniqueSentence('R11');
    rewriteSentence(f11, new11);
    const want2 = compileOf(work.dir);
    await withPreviewPage(work.dir, '?start=s-1-1', async ({ page, srv }) => {
      const scene = '再実行 st-1-1';
      const body = await bodyOf(page);
      const checks = [
        ['直した文が出る', body.includes(new11)],
        ['直す前の文が出ない', !body.includes(old11)],
        ['ir/main.json が直した写しの compile の出力と一致', (await fetchIr(srv.origin)).equals(want2)],
      ];
      for (const [name, ok] of checks) L.check(scene, name, ok);
      for (const [name, ok] of await inspectFirstStep(page, stepDo(want2, FIRST_STEPS[1]), stepTitle(FIRST_STEPS[1]))) L.check(scene, name, ok);
      total += checks.length;
      reflected += checks.filter(([, ok]) => ok).length;
    });

    // C-6: 診断を挟む。壊す → 配られず 1 で終わる → 直す → URL が出て、直した文が画面に出る。
    const broken = path.join(work.dir, 'chapters', 'ch-0', 'st-0-1.md.bak');
    fs.renameSync(f01, broken);
    const bad = await runPreviewToExit(previewBin, work.dir);
    L.check('診断を挟む', '壊した写しは 1 で終わり URL が出ない', bad.status === 1 && !/^URL:/m.test(bad.stdout));
    fs.renameSync(broken, f01);
    const new01b = uniqueSentence('R02');
    rewriteSentence(f01, new01b);
    await withPreviewPage(work.dir, '', async ({ page }) => {
      const scene = '診断を挟む';
      const body = await bodyOf(page);
      L.check(scene, '直した後の文が出る', body.includes(new01b));
      L.check(scene, '前回の直した文が出ない', !body.includes(new01));
    });
  } finally {
    work.dispose();
  }
  console.log(`PREVIEW-COMMAND scene=rerun reflected=${reflected}/${total} ledger=${L.items.filter((i) => i.ok).length}/${L.items.length}`);
  assert.deepEqual(L.failed(), []);
});
