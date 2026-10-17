// 自動ブラウザの 15 地点で再読み込みの一致を試験する（TASK-319 3/3。SAVE-3・SAVE-28）。
//
// 出所は PoC-6 の S-1（CH-2 に相当する 15 地点。入った直後 8・確認待ち 7・処理中 0。各手は処理中が終わるのを待ってから再読み込みした）。
// 試験の設定（TEST_HOOKS=1）の画面を `?fixture=reload-points` で開くと、起動の前に 8 Step の教材
// （pack `reload-points-ch2`。`crates/web/tests/data/reload_points_ch2/ir.json`。core の 16 Step の先頭 8 節点の写し）が選ばれる。
// 教材の差し替え（`load-fixture:`）は起動の後に働き、保存先の鍵が mount の教材のままになって復元を試せないため、使わない。
//
// 15 地点のそれぞれで、新しい browser context（保存領域は context ごとに独立）に、誘導つきの手を k 手打って
// 下書きを入れ、実際に `page.reload()` する。次を確かめる。
//   - 起動の種類が `restored`、世代がちょうど +1、engine のログは前のログの末尾に `reloaded` が 1 件足されたもの
//   - ログを除いた状態（Step・左の状態・下書きなど。世代を除く）・復習・左の行・現在の Step・状態表示が等しい
//   - 画面の左の行・復習の行が等しく、ログの行は前の行の末尾に再読み込みの告知（SAVE-6）が 1 件足されたもの
//   - 入力欄の値とホストの下書きが前と等しい（SAVE-3 の入力欄の値）
//   - そこから完走した最終の世界状態が、再読み込みなしの完走と等しい
// 手の経路は、実際の画面が 8 手（入力欄の送信・選択肢・承認のボタン）、テスト専用の口（raw-trigger）が 7 手。
// spec の検証方法の「10 手が画面・5 手がテスト専用」とは違う。入力方式 choice の画面の口（TASK-450）と、確認の
// point_at・ui_action の画面からの結線が未実装のためで、教材の形は変えない（7 手は PoC と同じ形のまま raw で送る）。
//
// 対照: クエリなし・未知の名前では `min-3step` で始まり、鍵は `min-3step` の側に書かれる（選択が完全一致だけであること）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
// 試験の出力・失敗の文に、保存した値・下書きの中身を出さない（履歴は受講者のデータ。有無・長さ・比較の結果だけを出す）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-reload-points/';
const QUERY = '?fixture=reload-points';
const PACK = 'reload-points-ch2';
const DEFAULT_PACK = 'min-3step';
const START = 's-1';
const LAST = 'main/st-8';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const RAW = '[data-testid="raw-trigger"]';
const LOG_ITEMS = '[data-testid="part-log-list"] > li';
const ARTIFACT_ITEMS = '[data-testid="part-artifact-view"] [data-testid="view-list"]:not([hidden]) > li';
const REVIEW_ITEMS = '[data-testid="part-review-list"] > li';
const FINISH_LIMIT = 60;
const T = 30000;

let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function open(srv, query = QUERY) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(`${srv.url}${query}`);
  await waitMounted(opened.page);
  return opened;
}

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const keyOf = (page, pack) => page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [pack, START]);
const savedLen = (page, key) => page.evaluate((k) => {
  const v = localStorage.getItem(k);
  return v === null ? null : v.length;
}, key);

// テスト専用の口へ `Op` の JSON を渡す。文字列としてだけ渡し、コードの文字列を組み立てない。
async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

const uiOp = (n) => ({ input: { ui: { view: 'v-files', path: `notes/n${n}` } } });

// 現在の Step の番号と、受付の後か（確認待ち）か。完走していれば null。
function position(eng) {
  const status = eng.stateWithoutLog.run.slices.main?.status ?? {};
  if (status[LAST] === 'achieved') return null;
  const n = Number(eng.current.replace('main/st-', ''));
  return { n, accepted: status[eng.current] === 'accepted' };
}

// 誘導つきの次の 1 手。route は 'screen'（実際の画面）か 'raw'（テスト専用の口）。
function guidedMove({ n, accepted }) {
  const screen = (run) => ({ route: 'screen', run });
  const raw = (op) => ({ route: 'raw', run: (page) => sendRaw(page, op) });
  const submit = (text) => screen(async (page) => {
    await page.fill(INPUT, text);
    await page.click(SUBMIT);
  });
  const yes = screen((page) => page.getByRole('button', { name: 'はい', exact: true }).click());
  if (accepted) {
    if (n === 2 || n === 7) {
      return screen((page) => page.click('[data-testid="choice-btn"][data-choice="opt-a"]'));
    }
    if (n === 6) return yes;
    return raw(uiOp(n));
  }
  switch (n) {
    case 2: return submit('メモ 2');
    case 3: case 8: return yes;
    case 4: case 5: return raw({ input: { choice: 'opt-a' } });
    case 6: return screen((page) => page.getByRole('button', { name: '直す（対象: expect）', exact: true }).click());
    case 7: return raw(uiOp(n));
    default: return submit(`/make-note note-${n}`);
  }
}

// 手の前後で変わる印（現在の Step と受付の状態）。手が進んだことを待つのに使う。
const signature = (eng) => `${eng.current}:${eng.stateWithoutLog.run.slices.main?.status?.[eng.current] ?? ''}`;

// 誘導つきの 1 手を打ち、状態が進んで処理中が終わるのを待つ。完走していれば null。
async function playOne(page) {
  const eng = await engine(page);
  const pos = position(eng);
  if (pos === null) return null;
  const move = guidedMove(pos);
  const sig = signature(eng);
  await move.run(page);
  await page.waitForFunction(([s]) => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    const st = e.stateWithoutLog.run.slices.main?.status ?? {};
    return `${e.current}:${st[e.current] ?? ''}` !== s && window.__fandheTest.nextTick() === 'null';
  }, [sig], { timeout: T });
  return { route: move.route, accepted: pos.accepted };
}

async function finish(page) {
  for (let i = 0; i < FINISH_LIMIT; i += 1) {
    if ((await playOne(page)) === null) return;
  }
  assert.fail(`did not finish within ${FINISH_LIMIT} moves`);
}

const texts = (page, sel) => page.$$eval(sel, (lis) => lis.map((li) => li.textContent));
const fieldValue = (page) => page.$eval(INPUT, (el) => el.value);

// 再読み込みの前後で比べる観測の束。保存した値・下書きの中身は、比べるだけで出力しない。
async function observe(page) {
  const eng = await engine(page);
  return {
    eng,
    logRows: await texts(page, LOG_ITEMS),
    logKinds: await page.$$eval(LOG_ITEMS, (lis) => lis.map((li) => li.dataset.kind ?? '')),
    artifactRows: await texts(page, ARTIFACT_ITEMS),
    reviewRows: await texts(page, REVIEW_ITEMS),
    field: await fieldValue(page),
    draft: await page.evaluate(() => window.__fandheTest.draft()),
  };
}

const withoutGeneration = (eng) => {
  const s = JSON.parse(JSON.stringify(eng.stateWithoutLog));
  s.run.generation = 0;
  return s;
};

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify({ csp: s.csp, pageErrors: s.pageErrors, consoleErrors: s.consoleErrors });
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

// 再読み込みなしで完走したときの最終の世界状態。
async function baselineWorld(srv) {
  const { page, context, rec } = await open(srv);
  await finish(page);
  const world = (await engine(page)).stateWithoutLog.run.world;
  assert.equal(position(await engine(page)), null, 'baseline must complete');
  clean(rec);
  await context.close();
  return JSON.stringify(world);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`15 地点で再読み込みの前後が世代を除いて一致し完走した世界状態が一致する（CSP ${label}）`, { timeout: 900000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const baseWorld = await baselineWorld(srv);
      const counts = { points: 0, entered: 0, busy: 0, awaitingConfirm: 0, screen: 0, raw: 0 };

      for (let k = 0; k < 15; k += 1) {
        const { page, context, rec } = await open(srv);
        const key = await keyOf(page, PACK);
        const defaultKey = await keyOf(page, DEFAULT_PACK);
        assert.equal(await savedLen(page, key), null, `point ${k}: nothing is saved yet`);

        // 誘導つきの手を k 手打つ。
        for (let i = 0; i < k; i += 1) await playOne(page);
        const pos = position(await engine(page));
        assert.notEqual(pos, null, `point ${k}: not completed yet`);
        const eng0 = await engine(page);
        counts.points += 1;
        if (eng0.runStatus === 'busy') counts.busy += 1;
        else if (pos.accepted) counts.awaitingConfirm += 1;
        else counts.entered += 1;
        // 地点から先へ打つ手の経路を数える（15 地点で 15 手）。
        counts[guidedMove(pos).route] += 1;

        // 地点ごとに違う空でない下書きを入れる（未操作の履歴にしないため。入力欄の値の比較を空同士にしないため）。
        const draft = `下書き ${k}`;
        await page.fill(INPUT, draft);
        await page.waitForFunction((d) => window.__fandheTest.draft() === d, draft, { timeout: T });
        const before = await observe(page);
        assert.equal(before.field, before.draft, `point ${k}: field equals host draft before reload`);

        await page.reload();
        await waitMounted(page);

        assert.notEqual(await savedLen(page, key), null, `point ${k}: history is saved on leave`);
        assert.equal(await savedLen(page, defaultKey), null, `point ${k}: the default fixture key is untouched`);
        const after = await observe(page);
        assert.equal(after.eng.bootKind, 'restored', `point ${k}: boot kind`);
        assert.equal(after.eng.generation, before.eng.generation + 1, `point ${k}: generation`);
        assert.equal(after.eng.logLen, before.eng.logLen + 1, `point ${k}: log length`);
        const kinds = after.eng.log.map((r) => r.kind);
        assert.deepEqual(kinds.slice(0, before.eng.log.length), before.eng.log.map((r) => r.kind), `point ${k}: log prefix`);
        assert.equal(kinds[kinds.length - 1], 'reloaded', `point ${k}: log ends with reloaded`);
        assert.equal(kinds.filter((x) => x === 'reloaded').length, before.eng.log.filter((r) => r.kind === 'reloaded').length + 1);
        assert.deepEqual(withoutGeneration(after.eng), withoutGeneration(before.eng), `point ${k}: state`);
        assert.deepEqual(after.eng.review, before.eng.review, `point ${k}: review`);
        assert.deepEqual(after.eng.artifacts, before.eng.artifacts, `point ${k}: artifacts`);
        assert.equal(after.eng.current, before.eng.current, `point ${k}: current`);
        assert.equal(after.eng.runStatus, before.eng.runStatus, `point ${k}: run status`);
        assert.equal(after.eng.incrementalMatches, true, `point ${k}: incremental projection`);
        assert.equal(after.eng.worldMatchesState, true, `point ${k}: world matches state`);
        assert.deepEqual(after.artifactRows, before.artifactRows, `point ${k}: artifact rows`);
        assert.deepEqual(after.reviewRows, before.reviewRows, `point ${k}: review rows`);
        assert.deepEqual(after.logRows.slice(0, before.logRows.length), before.logRows, `point ${k}: log rows prefix`);
        assert.equal(after.logRows.length, before.logRows.length + 1, `point ${k}: one notice row`);
        assert.equal(after.logKinds[after.logKinds.length - 1], 'reloaded', `point ${k}: notice row kind`);
        assert.equal(after.field === before.field, true, `point ${k}: input field value`);
        assert.equal(after.draft === before.draft, true, `point ${k}: host draft`);

        // 復元した側から誘導つきの手で完走し、最終の世界状態が再読み込みなしの完走と一致する。
        await finish(page);
        assert.equal(position(await engine(page)), null, `point ${k}: completes`);
        assert.equal(JSON.stringify((await engine(page)).stateWithoutLog.run.world), baseWorld, `point ${k}: final world`);
        clean(rec);
        await context.close();
      }

      // PoC の S-1 と同じく観測した値を出す（保存した値・下書きは出さない）。
      console.log(`OBS csp=${csp} ${JSON.stringify(counts)}`);
      assert.deepEqual(counts, { points: 15, entered: 8, busy: 0, awaitingConfirm: 7, screen: 8, raw: 7 });
    }));
  });

  test(`対照: クエリなし・未知の名前では既定の教材で始まり、鍵は既定の教材の側に書かれる（CSP ${label}）`, { timeout: 120000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      for (const query of ['', '?fixture=unknown', '?fixture=reload-points&x=1']) {
        const { page, context, rec } = await open(srv, query);
        const defaultKey = await keyOf(page, DEFAULT_PACK);
        const pointsKey = await keyOf(page, PACK);
        assert.equal((await engine(page)).current, 'main/st-1', `query ${JSON.stringify(query)}`);
        await page.fill(INPUT, '/make-note first-note');
        await page.click(SUBMIT);
        await page.waitForFunction((sel) => document.querySelector(sel).value === '', INPUT);
        await page.reload();
        await waitMounted(page);
        assert.equal((await engine(page)).bootKind, 'restored', `query ${JSON.stringify(query)}`);
        assert.notEqual(await savedLen(page, defaultKey), null, `query ${JSON.stringify(query)}: default key is written`);
        assert.equal(await savedLen(page, pointsKey), null, `query ${JSON.stringify(query)}: reload-points key is untouched`);
        clean(rec);
        await context.close();
      }
    }));
  });
}
