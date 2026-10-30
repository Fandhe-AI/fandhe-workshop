// 構造定義（教材の IR）の表示される欄と ID への注入 55 件が、画面で何も実行されず文字として出ることの試験（SAFE-2。TASK-145 1/2。CSP あり・なし）。
//
// データの正は testdata の xss-cases.md の「入り口に E2 を含む」55 件（xss-cases.mjs の e2Payloads が読む。ペイロードをここへ書き写さない）。
// IR の作成と画面の判定は e2-ir.mjs（単体試験は e2-ir.test.mjs）。元の IR は crates/web/tests/data/ch0_3/ir.json（実行時に読むだけ）。
//
// 注入の口: 配信する設定の web の一時の写しに `fandhe-ir` の meta を足し、`ir/main.json` として 1 件ごとに作った IR を配る
// （pack-body-xss と同じ。件ごとにサーバを起動し直す）。2 つの開始地点（`?start=s-0-1`・`?start=s-1-4`）から開き、各欄が画面に出る Step まで
// 実クリックと入力欄への送信で進む。試験側は innerHTML で何も入れず、画面を観察する（対照 D1 だけが意図的に生の入力を入れる）。
//
// 試験 B（欄の変種）: 題・do・選択肢の名前・拒否と問い返しの文面を P にした IR で、画面の欄が P の文字と等しく子の要素が 0 個であること
//   （拒否・問い返しの文面は差し込みの枠を通るので、枠で区切った断片が順に含まれること。XSS-53）。開始地点 2 × 55 件 = 110 回が CSP あり・なしの
//   それぞれで 110 回すべて合格すること（C で集計）。全区分の計数（実行・通信・CSP 違反・危険な要素・ページ内エラー）が 0 で、通信は配信物の
//   構成要素と ir/main.json 1 件だけ。外部通信は除外しない。
// 試験 E（ID の変種）: コマンド名・選択肢の ID・ヒントの ID を P にした IR で、計数が 0。画面に P が出るかは記録だけ（PoC-7 は 55/110）。
// 限界: ヒントの本文は開いた後だけ画面（TASK-530 1/2。tools/browser/hint-open.test.mjs）に出るため、この試験の判定には入れず、refs は受講者の画面に出る経路が無いため、「文字として出た」には数えず、IR が読まれて
//   計数が 0 であることと、画面の文字に出ないこと（今の製品の事実の固定）だけを確かめる。
// 試験 D1: 実際の題へ描画を通さない生の入力を入れると計数が 0 でなく数える。D2: 変種を作らない IR では判定が「出ていない」になる
//   （110/110 が何も見ない判定の結果でないことの対照）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
// 書き込み先は mkdtemp の一時ディレクトリの固定のパス（ir/main.json）だけで、ペイロードからパス・セレクタ・コードを作らない。
// 失敗の文には件の ID・開始地点・CSP・欄の名前だけを出し、ペイロードの全文は出さない。URL は example.invalid だけを使う（ペイロード側）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import { pageRequestProblems } from './delivery.mjs';
import { irFetchCheck } from './l1-l2-runner.mjs';
import { DRAFT, CHOICE, REJECTED, CLARIFIED, choiceSel, waitCount, mouseDriver } from './ch0-3-journey.mjs';
import { positionOf } from './author-preview.mjs';
import { e2Payloads } from './xss-cases.mjs';
import {
  loadBaseIr, withStarts, fieldVariant, idVariant, observeFields, judgeFields, textHas, STARTS, ID_TARGETS, PACK_ID,
} from './e2-ir.mjs';

const BASE = '/fandhe-structure-xss/';
const CSP_META_RE = /<meta\b[^>]*\bhttp-equiv\s*=\s*["']?content-security-policy["']?[^>]*>/gi;
const TITLE = '[data-testid="step-title"]';
const NOTICE = '[data-testid="ir-notice"]';
const T = 30000;
const STEP_0_2A = 'main/st-0-2a';

let browser;
let cases;
let baseIr; // 開始点 2 件の元の IR（変種を作る元）
const passed = new Map(); // `${csp}` -> 合格した回数（C が集計する）
const shownIds = new Map(); // ID の変種で P が画面に出た回数（記録だけ）

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  cases = e2Payloads();
  assert.equal(cases.length, 55);
  baseIr = withStarts(loadBaseIr());
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

function cspMetas(dir) {
  const found = fs.readFileSync(path.join(dir, 'index.html'), 'utf8').match(CSP_META_RE) ?? [];
  assert.equal(found.length, 1, `${dir}: exactly one CSP meta`);
  return found[0];
}

// 配信する設定の一時の写しに fandhe-ir の meta と ir/ を足して fn に渡し、成否にかかわらず消す（pack-body-xss.test.mjs と同じ足し方）。
async function withDeliveryCopy(fn) {
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-e2-web-'));
  try {
    fs.cpSync(process.env.FANDHE_WEB_DIR, copy, { recursive: true });
    const index = path.join(copy, 'index.html');
    const html = fs.readFileSync(index, 'utf8');
    assert.equal(html.split('</head>').length - 1, 1, 'exactly one </head>');
    assert.ok(!/name\s*=\s*["']?fandhe-ir/i.test(html), 'delivery index has no fandhe-ir meta');
    assert.ok(!/name\s*=\s*["']?fandhe-test-hooks/i.test(html), 'delivery index has no test-hooks meta');
    assert.equal((html.match(CSP_META_RE) ?? []).length, 1, 'exactly one CSP meta');
    fs.writeFileSync(index, html.replace('</head>', '<meta name="fandhe-ir" content="1"></head>'));
    fs.mkdirSync(path.join(copy, 'ir'), { recursive: true });
    return await fn(copy);
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}

// 全区分の計数が 0、記録用の関数の呼び出しが 0、通信が配信物の構成要素と ir/main.json 1 件だけであることを確かめる。
async function assertQuiet(page, rec, label, { settleMs } = {}) {
  const counts = await collectSafety(page, rec, settleMs === undefined ? {} : { settleMs });
  const detail = `${label}: ${JSON.stringify({ counts, hits: rec.hits.length })}`;
  assert.deepEqual(nonZero(counts), [], detail);
  assert.deepEqual(rec.hits, [], detail);
}

function assertNetwork(rec, srv, label) {
  const chk = irFetchCheck(rec, `${srv.url}ir/main.json`);
  assert.equal(chk.irCount, 1, `${label}: ir/main.json request count`);
  assert.ok(chk.irOk, `${label}: ir/main.json status`);
  const problems = pageRequestProblems({ ...rec, failed: chk.failed }, { origin: srv.origin, base: BASE });
  assert.deepEqual(problems, [], `${label}: ${JSON.stringify(problems)}`);
}

async function openStart(page, srv, start) {
  await page.goto(`${srv.url}?start=${encodeURIComponent(start)}`);
  await waitMounted(page);
  assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'no test hooks');
}

const waitChoices = (page, n) => page.waitForFunction(([sel, k]) => document.querySelectorAll(sel).length === k, [CHOICE, n], { timeout: T });

// 開始地点ごとの操作の列。観察点（snap）で画面を読んで返す。ペイロードは使わず、固定の ID と固定の入力だけで進む。
// s-0-1: 選択肢の誤り（拒否）→ 受け付け → 指す → st-0-2a → 確認の選択肢 → 誤答（拒否）。
async function drive01(page, snap) {
  const d = mouseDriver(page);
  await page.waitForSelector(TITLE, { timeout: T });
  await snap('open', { title: true, do: true, choices: 3 }, positionOf(baseIr, 'main/st-0-1'));
  await d.choose('opt-left');
  await waitCount(page, REJECTED, 1);
  await snap('rej1', { line: 'rejected' });
  await d.choose('opt-right');
  await d.point('cards/notice');
  await page.waitForSelector(choiceSel('opt-read-a'), { timeout: T });
  await waitChoices(page, 1);
  await snap('st-0-2a', { title: true, do: true, choices: 1 }, positionOf(baseIr, STEP_0_2A));
  await d.choose('opt-read-a');
  await waitChoices(page, 2);
  await snap('answers', { choices: 2 });
  await d.choose('ans-a-ng');
  await waitCount(page, REJECTED, 2);
  await snap('rej2', { line: 'rejected' });
}

// s-1-4: 引数なしの /new-idea（問い返し）→ 順序違いの /advance-phase（拒否）→ Esc で取り消し（受け付け）→ 確認の選択肢 → 誤答（拒否）。
async function drive14(page, snap) {
  const d = mouseDriver(page);
  await page.waitForSelector(TITLE, { timeout: T });
  await snap('open', { title: true, do: true, choices: 0 }, positionOf(baseIr, 'main/st-1-4'));
  await d.send('/new-idea');
  await waitCount(page, CLARIFIED, 1);
  await snap('clar', { line: 'clarified' });
  await page.waitForFunction((sel) => document.querySelector(sel)?.value === '', DRAFT, { timeout: T });
  await d.send('/advance-phase');
  await waitCount(page, REJECTED, 1);
  await snap('rej1', { line: 'rejected' });
  await d.esc();
  await waitChoices(page, 2);
  await snap('answers', { title: true, do: true, choices: 2 });
  await d.choose('rejected');
  await waitCount(page, REJECTED, 2);
  await snap('rej2', { line: 'rejected' });
}

const DRIVERS = { 's-0-1': drive01, 's-1-4': drive14 };

// 1 回（ペイロード × 開始地点）を流し、観察点ごとの読み取りを返す。各観察点で計数も 0 で確かめる。
async function runDrive(srv, start, label) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    await openStart(page, srv, start);
    const snaps = [];
    const snap = async (name, expect, position) => {
      const obs = await page.evaluate(observeFields);
      snaps.push({ name, obs, expect, position });
      await assertQuiet(page, rec, `${label} ${name}`, { settleMs: 50 });
    };
    await DRIVERS[start](page, snap);
    await assertQuiet(page, rec, `${label} final`);
    assertNetwork(rec, srv, label);
    return snaps;
  } finally {
    await context.close();
  }
}

// 観察点ごとの判定の結果（満たさなかった欄の名前、`name.欄` の形）。位置は開いた直後と st-0-2a で IR と一致すること。
function judgeAll(snaps, P) {
  const bad = [];
  for (const s of snaps) {
    for (const f of judgeFields(s.obs, s.expect, P)) bad.push(`${s.name}.${f}`);
    if (s.obs.notice !== 0) bad.push(`${s.name}.notice`);
    if (s.position !== undefined && s.obs.position !== s.position) bad.push(`${s.name}.position`);
  }
  return bad;
}

const short = (e) => String(e?.message ?? e).replaceAll(/\s+/g, ' ').slice(0, 300);

test('A: 前提（E2 が 55 件・元の IR の欄・CSP の meta が両方の設定で同一）', () => {
  assert.equal(cases.length, 55);
  assert.equal(cspMetas(process.env.FANDHE_WEB_TEST_DIR), cspMetas(process.env.FANDHE_WEB_DIR));
  assert.deepEqual(baseIr.starts.map((s) => s.id), STARTS.map((s) => s.id));
  const options = (uid) => baseIr.nodes.find((n) => n.uid === uid).step.input.options.map((o) => o.id);
  assert.deepEqual(options('main/st-0-1'), ['opt-left', 'opt-right', 'opt-top']);
  assert.deepEqual(options(STEP_0_2A), ['opt-read-a']);
  assert.ok(Object.keys(baseIr.clarifications).length >= 1);
  assert.ok(Object.keys(baseIr.hints).length >= 1);
  assert.ok(ID_TARGETS.command in baseIr.commands && ID_TARGETS.hint in baseIr.hints);
});

for (const csp of [true, false]) {
  for (const start of STARTS.map((s) => s.id)) {
    test(`B: 欄の変種 55 件が ${start} から開いた画面で文字として出て、実行・通信・危険な要素が 0 件（CSP ${csp ? 'あり' : 'なし'}）`, { timeout: 600000 }, async (t) => {
      const failures = [];
      let ok = 0;
      await withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, async (dir) => {
        for (const c of cases) {
          const label = `${c.id} ${start} csp=${csp ? 'on' : 'off'}`;
          try {
            const { ir } = fieldVariant(baseIr, c.payload);
            fs.writeFileSync(path.join(dir, 'ir', 'main.json'), JSON.stringify(ir), 'utf8');
            const snaps = await withServer({ dir, base: BASE }, (srv) => runDrive(srv, start, label));
            const bad = judgeAll(snaps, c.payload);
            assert.deepEqual(bad, [], `${label}: fields not shown as text: ${bad.join(',')}`);
            ok += 1;
          } catch (e) {
            failures.push(`${label}: ${short(e)}`);
          }
        }
      }));
      t.diagnostic(`${start} csp=${csp}: ${ok}/${cases.length}`);
      passed.set(`${csp}/${start}`, ok);
      assert.deepEqual(failures, []);
      assert.equal(ok, cases.length);
    });
  }
}

test('C: 欄の変種の合格が CSP あり・なしのそれぞれで 110 回中 110 回', () => {
  for (const csp of [true, false]) {
    const total = STARTS.reduce((a, s) => a + (passed.get(`${csp}/${s.id}`) ?? -1000), 0);
    assert.equal(total, 110, `csp=${csp ? 'on' : 'off'}: ${total}/110 (run test B first)`);
  }
});

// ID の変種 1 回。IR が読まれる（題が出る）か、理由つきの通知が出る。どちらでも計数が 0。通知の文に P の断片は含まれない。
async function runIdVariant(srv, start, P, label) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    await openStart(page, srv, start);
    await page.waitForSelector(`${TITLE}, ${NOTICE}`, { timeout: T });
    const notice = await page.evaluate((sel) => document.querySelector(sel)?.textContent ?? null, NOTICE);
    if (notice !== null) assert.ok(!textHas(notice, P), `${label}: notice repeats the input`);
    await assertQuiet(page, rec, `${label} open`, { settleMs: 50 });
    if (notice === null) {
      // コマンド名が候補に出る経路。
      if (await page.locator(DRAFT).count() === 1) {
        await page.fill(DRAFT, '/');
        await page.waitForTimeout(150);
        await assertQuiet(page, rec, `${label} candidates`, { settleMs: 50 });
      }
      // 選択肢の ID は属性値にだけ載る。CSS のセレクタに P を入れず、属性値の一致で押す要素を決める。
      const index = await page.evaluate(([sel, p]) => [...document.querySelectorAll(sel)].findIndex((b) => b.getAttribute('data-choice') === p), [CHOICE, P]);
      if (index >= 0) {
        await page.locator(CHOICE).nth(index).click({ timeout: T });
        await page.waitForTimeout(150);
        await assertQuiet(page, rec, `${label} choice`, { settleMs: 50 });
      }
    }
    const shown = await page.evaluate(() => document.body.textContent ?? '');
    await assertQuiet(page, rec, `${label} final`);
    assertNetwork(rec, srv, label);
    return { shown: textHas(shown, P), noticed: notice !== null };
  } finally {
    await context.close();
  }
}

for (const csp of [true, false]) {
  for (const start of STARTS.map((s) => s.id)) {
    test(`E: ID の変種 55 件が ${start} から開いても、実行・通信・危険な要素が 0 件（CSP ${csp ? 'あり' : 'なし'}）`, { timeout: 600000 }, async (t) => {
      const failures = [];
      let ok = 0;
      let shown = 0;
      let noticed = 0;
      await withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, async (dir) => {
        for (const c of cases) {
          const label = `${c.id} ${start} csp=${csp ? 'on' : 'off'} id`;
          try {
            const { ir, replaced, leftover } = idVariant(baseIr, c.payload);
            for (const kind of Object.keys(ID_TARGETS)) {
              assert.ok(replaced[kind] >= 1 && leftover[kind] === 0, `${label}: ${kind} replacement`);
            }
            fs.writeFileSync(path.join(dir, 'ir', 'main.json'), JSON.stringify(ir), 'utf8');
            const r = await withServer({ dir, base: BASE }, (srv) => runIdVariant(srv, start, c.payload, label));
            ok += 1;
            if (r.shown) shown += 1;
            if (r.noticed) noticed += 1;
          } catch (e) {
            failures.push(`${label}: ${short(e)}`);
          }
        }
      }));
      // 画面に P が出た回数は記録だけ（PoC-7 は ID を置き換えた変種で 55/110）。
      t.diagnostic(`${start} csp=${csp}: quiet ${ok}/${cases.length}, shown on screen ${shown}, notice ${noticed}`);
      shownIds.set(`${csp}/${start}`, shown);
      assert.deepEqual(failures, []);
      assert.equal(ok, cases.length);
    });
  }
}

test('F: ヒントと refs の変種で IR が読まれ、計数が 0 で、その入力が画面の文字に出ない（表示の経路が無いことの固定）', { timeout: 300000 }, async () => {
  const c = cases.find((x) => x.id === 'XSS-01') ?? cases[0];
  // 欄の変種のうち、ヒントの本文と refs だけを P にした IR（他の欄は元のまま）。
  const { ir: all } = fieldVariant(baseIr, c.payload);
  const ir = JSON.parse(JSON.stringify(baseIr));
  ir.pack = { ...ir.pack, id: PACK_ID };
  ir.hints = all.hints;
  ir.refs = all.refs;
  await withDeliveryCopy(async (copy) => {
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), JSON.stringify(ir), 'utf8');
    for (const start of STARTS.map((s) => s.id)) {
      await withServer({ dir: copy, base: BASE }, async (srv) => {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        try {
          const label = `hint-refs ${start}`;
          await openStart(page, srv, start);
          await page.waitForSelector(TITLE, { timeout: T });
          assert.equal(await page.locator(NOTICE).count(), 0, `${label}: ir-notice shown`);
          assert.equal(await page.locator(TITLE).count(), 1, `${label}: step-title count`);
          const text = await page.evaluate(() => document.body.textContent ?? '');
          assert.ok(!textHas(text, c.payload), `${label}: the hint/refs input appeared on screen (a display path now exists: add it to the field judgement)`);
          await assertQuiet(page, rec, label);
          assertNetwork(rec, srv, label);
        } finally {
          await context.close();
        }
      });
    }
  });
});

test('D1: 対照。実際の題へ描画を通さない生の入力を入れると、数え方が 0 でなく数える', { timeout: 300000 }, async () => {
  const byId = new Map(cases.map((c) => [c.id, c.payload]));
  const { ir } = fieldVariant(baseIr, 'ok');
  await withDeliveryCopy(async (copy) => {
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), JSON.stringify(ir), 'utf8');
    await withServer({ dir: copy, base: BASE }, async (srv) => {
      for (const id of ['XSS-01', 'XSS-07']) {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        try {
          await openStart(page, srv, 's-0-1');
          await page.waitForSelector(TITLE, { timeout: T });
          await page.evaluate(([sel, html]) => {
            document.querySelector(sel).innerHTML = html;
          }, [TITLE, byId.get(id)]);
          const counts = await collectSafety(page, rec);
          assert.notDeepEqual(nonZero(counts), [], `${id}: control should be counted ${JSON.stringify(counts)}`);
        } finally {
          await context.close();
        }
      }
    });
  });
});

test('D2: 対照。変種を作らない IR では、同じ操作の列と判定が「出ていない」になる', { timeout: 300000 }, async () => {
  const P = cases.find((x) => x.id === 'XSS-01').payload;
  await withDeliveryCopy(async (copy) => {
    const plain = JSON.parse(JSON.stringify(baseIr));
    plain.pack = { ...plain.pack, id: PACK_ID };
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), JSON.stringify(plain), 'utf8');
    for (const start of STARTS.map((s) => s.id)) {
      const snaps = await withServer({ dir: copy, base: BASE }, (srv) => runDrive(srv, start, `control ${start}`));
      for (const s of snaps) {
        // 個数 0 の選択肢の期待は空の集合への判定で、何も見ない（対照にならない）ので数えない。
        const fields = Object.entries(s.expect).filter(([k, v]) => !(k === 'choices' && v === 0));
        const bad = judgeFields(s.obs, s.expect, P);
        assert.equal(bad.length, fields.length, `${start} ${s.name}: judgement should fail on every field of an unvaried IR (got ${bad.join(',')})`);
      }
      // 位置の判定は IR の変種に依らず一致する（対照ではなく、位置の比較が働いていることの確認）。
      assert.deepEqual(judgeAll(snaps, P).filter((b) => b.endsWith('.position')), [], `${start}: position`);
    }
  });
});
