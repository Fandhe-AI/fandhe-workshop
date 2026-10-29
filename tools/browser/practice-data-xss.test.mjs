// 練習データ（世界状態の欄）への注入 50 件が、画面で何も実行されず文字として出ることの試験（SAFE-3。TASK-145 2/2。CSP あり・なし）。
//
// データの正は testdata の xss-cases.md の「入り口に E3 を含む」50 件（xss-cases.mjs の e3Payloads が読む。ペイロードをここへ書き写さない）。
// IR の作成と画面の判定は e3-ir.mjs（単体試験は e3-ir.test.mjs）。元の IR は crates/web/tests/data/ch0_3/ir.json（実行時に読むだけ）。
//
// 入れる欄: `put` のパス、`put` の属性の値、`set_attr` の値（PoC-7 の injectE3 と同じ方法。同じパスの全出現を置き換え、異なるパスの
// 1 つ目は P、2 つ目以降は `P.i`。属性の値と `set_attr` の値は P）。2 つの開始地点（`?start=s-0-1`・`?start=s-1-4`）から開き、
// 実クリックと入力欄への送信で、練習データが左の成果物に現れる Step まで進む。試験側は innerHTML で何も入れず、画面を観察する
// （対照 D1 だけが意図的に生の入力を入れる）。
//
// 試験 B: 左の成果物の行（`put` のパス）が P の文字と等しく子の要素が 0 個であること、その行を指した受付がログに文字として出ること、
//   全区分の計数（実行・通信・CSP 違反・危険な要素・ページ内エラー）が 0 で、通信は配信物の構成要素と ir/main.json 1 件だけであること。
//   開始地点 2 × 50 件 = 100 回が CSP あり・なしのそれぞれで 100 回すべて合格すること（C で集計）。外部通信は除外しない。
// 限界: 属性の値（`put.attrs`・`set_attr.value`）を受講者の画面に出す経路は今の製品に無い（カードの部品は未結線）ため、「文字として出た」には
//   数えない。IR が読まれて計数が 0 であることと、その入力が画面の文字にも要素の属性にも出ないこと（今の製品の事実の固定）を F で確かめる。
//   属性の値が世界状態へ届いたことの直接の確認は、試験の設定が任意の IR を受けられない（固定の教材だけ）ため行わない。
// 試験 D1: 実際の行へ描画を通さない生の入力を入れると計数が 0 でなく数える。D2: 変種を作らない IR では判定が「出ていない」になる
//   （100/100 が何も見ない判定の結果でないことの対照）。
//
// 入力: FANDHE_WEB_DIR（配信設定）。未設定・不在は skip にせず失敗にする。
// 書き込み先は mkdtemp の一時ディレクトリの固定のパス（ir/main.json）だけで、ペイロードからパス・セレクタ・コードを作らない。
// 行を押すときは、属性値の一致で添字を求めて nth で押す（P を CSS のセレクタに入れない）。
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
import { e3Payloads } from './xss-cases.mjs';
import {
  loadBaseIr, withStarts, e3Variant, attrOnlyVariant, putPaths, observeRows, judgeRows, judgeReceived, textHas, STARTS, PACK_ID,
} from './e3-ir.mjs';

const BASE = '/fandhe-practice-data-xss/';
const CSP_META_RE = /<meta\b[^>]*\bhttp-equiv\s*=\s*["']?content-security-policy["']?[^>]*>/gi;
const TITLE = '[data-testid="step-title"]';
const ROWS = '[data-testid="part-artifact-view"] li[data-path]';
const POINTABLE = '[data-testid="part-artifact-view"] li[data-row]';
const RUN = '[data-testid="run-status"]';
const T = 30000;

let browser;
let cases;
let baseIr; // 開始点 2 件の元の IR（変種を作る元）
let identity; // 元のパスから元のパスへの写像（変種でない IR の画面を操作・判定する）
const passed = new Map(); // `${csp}/${start}` -> 合格した回数（C が集計する）

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  cases = e3Payloads();
  assert.equal(cases.length, 50);
  baseIr = withStarts(loadBaseIr());
  identity = Object.fromEntries(putPaths(baseIr).map((p) => [p, p]));
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

// 配信する設定の一時の写しに fandhe-ir の meta と ir/ を足して fn に渡し、成否にかかわらず消す（structure-xss.test.mjs と同じ足し方）。
async function withDeliveryCopy(fn) {
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-e3-web-'));
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

// 全区分の計数が 0、記録用の関数の呼び出しが 0 であることを確かめる。
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
const waitRows = (page, n) => page.waitForFunction(([sel, k]) => document.querySelectorAll(sel).length === k, [ROWS, n], { timeout: T });
const waitIdle = (page) => page.waitForFunction((sel) => document.querySelector(sel)?.getAttribute('data-state') === 'idle', RUN, { timeout: T });

// 指す行を、data-row の値の一致（U+0000 を除いて比べる）で決めて押す。P を CSS のセレクタに入れない。
async function pointAt(page, rowPath) {
  const index = await page.evaluate(([sel, want]) => {
    const strip = (s) => String(s).replaceAll('\u0000', '');
    return [...document.querySelectorAll(sel)].findIndex((e) => strip(e.getAttribute('data-row') ?? '') === strip(want));
  }, [POINTABLE, rowPath]);
  assert.ok(index >= 0, 'pointable row not found');
  await page.locator(POINTABLE).nth(index).click({ timeout: T });
}

// 開始地点ごとの操作の列。観察点（snap）で画面を読み、期待の元のパスの一覧（names）と、指した行（received）を記録する。
// s-0-1: 右を選ぶ（受け付け。カードが 1 件）→ 指す → 読む（カードが 3 件）→ 答える。
async function drive01(page, snap, pm) {
  const d = mouseDriver(page);
  await page.waitForSelector(TITLE, { timeout: T });
  await snap('open', []);
  await d.choose('opt-right');
  await waitRows(page, 1);
  await snap('accepted', ['cards/notice']);
  await pointAt(page, pm['cards/notice']);
  await page.waitForSelector(choiceSel('opt-read-a'), { timeout: T });
  await waitChoices(page, 1);
  await snap('pointed', ['cards/notice'], 'cards/notice');
  await d.choose('opt-read-a');
  await waitRows(page, 3);
  await snap('added', ['cards/notice', 'cards/repo-a', 'cards/repo-b']);
  await waitChoices(page, 2);
  await d.choose('ans-a-ok');
  await page.waitForSelector(choiceSel('opt-read-b'), { timeout: T });
  await snap('next', ['cards/notice', 'cards/repo-a', 'cards/repo-b']);
}

// s-1-4: 問い返し → 順序違いの拒否 → Esc で取り消し（受け付け。カードが 1 件）→ 答える → 処理中の作成と更新（カードが 2 件）→ 指す。
async function drive14(page, snap, pm) {
  const d = mouseDriver(page);
  await page.waitForSelector(TITLE, { timeout: T });
  await snap('open', []);
  await d.send('/new-idea');
  await waitCount(page, CLARIFIED, 1);
  await page.waitForFunction((sel) => document.querySelector(sel)?.value === '', DRAFT, { timeout: T });
  await d.send('/advance-phase');
  await waitCount(page, REJECTED, 1);
  await d.esc();
  await waitRows(page, 1);
  await snap('cancelled', ['cards/idea']);
  await waitChoices(page, 2);
  await d.choose('asked-back');
  await page.waitForSelector(DRAFT, { timeout: T });
  await d.send('/make-card sample');
  await waitRows(page, 2);
  await waitIdle(page);
  await snap('processed', ['cards/idea', 'cards/sample']);
  await pointAt(page, pm['cards/sample']);
  await waitChoices(page, 2);
  await snap('pointed', ['cards/idea', 'cards/sample'], 'cards/sample');
}

const DRIVERS = { 's-0-1': drive01, 's-1-4': drive14 };

// 1 回（ペイロード × 開始地点）を流し、観察点ごとの読み取りを返す。各観察点で計数も 0 で確かめる。
// pm は操作に使う写像（元のパス → 画面に出ているパス）。
async function runDrive(srv, start, pm, label, { extra } = {}) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    await openStart(page, srv, start);
    const snaps = [];
    const snap = async (name, names, received) => {
      const obs = await page.evaluate(observeRows);
      snaps.push({ name, obs, names, received });
      await assertQuiet(page, rec, `${label} ${name}`, { settleMs: 50 });
    };
    await DRIVERS[start](page, snap, pm);
    await assertQuiet(page, rec, `${label} final`);
    assertNetwork(rec, srv, label);
    if (extra) await extra(page);
    return snaps;
  } finally {
    await context.close();
  }
}

// 観察点ごとの判定の結果（満たさなかった項目、`name.項目` の形）。pm は期待のパスを引く写像。
function judgeAll(snaps, pm) {
  const bad = [];
  for (const s of snaps) {
    for (const f of judgeRows(s.obs, s.names.map((n) => pm[n]))) bad.push(`${s.name}.${f}`);
    if (s.received) for (const f of judgeReceived(s.obs, pm[s.received])) bad.push(`${s.name}.received.${f}`);
  }
  return bad;
}

const short = (e) => String(e?.message ?? e).replaceAll(/\s+/g, ' ').slice(0, 300);

test('A: 前提（E3 が 50 件・CSP の meta が両方の設定で同一・元の IR の練習データ）', () => {
  assert.equal(cases.length, 50);
  assert.equal(cspMetas(process.env.FANDHE_WEB_TEST_DIR), cspMetas(process.env.FANDHE_WEB_DIR));
  assert.deepEqual(baseIr.starts.map((s) => s.id), STARTS.map((s) => s.id));
  const options = (uid) => baseIr.nodes.find((n) => n.uid === uid).step.input.options.map((o) => o.id);
  assert.deepEqual(options('main/st-0-1'), ['opt-left', 'opt-right', 'opt-top']);
  assert.deepEqual(options('main/st-0-2a'), ['opt-read-a']);
  for (const p of ['cards/notice', 'cards/idea', 'cards/sample', 'cards/repo-a', 'cards/repo-b']) {
    assert.ok(p in identity, `${p} is a put path`);
  }
  const { counts, leftover } = e3Variant(baseIr, 'x');
  assert.equal(counts.setAttr, 2);
  assert.ok(counts.paths >= 17 && counts.attrs >= 18);
  assert.equal(leftover, 0);
});

for (const csp of [true, false]) {
  for (const start of STARTS.map((s) => s.id)) {
    test(`B: 練習データの変種 50 件が ${start} から開いた画面で文字として出て、実行・通信・危険な要素が 0 件（CSP ${csp ? 'あり' : 'なし'}）`, { timeout: 600000 }, async (t) => {
      const failures = [];
      let ok = 0;
      await withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, async (dir) => {
        for (const c of cases) {
          const label = `${c.id} ${start} csp=${csp ? 'on' : 'off'}`;
          try {
            const { ir, pathMap, leftover } = e3Variant(baseIr, c.payload);
            assert.equal(leftover, 0, `${label}: original path left`);
            fs.writeFileSync(path.join(dir, 'ir', 'main.json'), JSON.stringify(ir), 'utf8');
            const snaps = await withServer({ dir, base: BASE }, (srv) => runDrive(srv, start, pathMap, label));
            const bad = judgeAll(snaps, pathMap);
            assert.deepEqual(bad, [], `${label}: not shown as text: ${bad.join(',')}`);
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

test('C: 練習データの変種の合格が CSP あり・なしのそれぞれで 100 回中 100 回', () => {
  for (const csp of [true, false]) {
    const total = STARTS.reduce((a, s) => a + (passed.get(`${csp}/${s.id}`) ?? -1000), 0);
    assert.equal(total, 100, `csp=${csp ? 'on' : 'off'}: ${total}/100 (run test B first)`);
  }
});

test('F: 属性の値だけの変種で IR が読まれ、計数が 0 で、その入力が画面の文字にも要素の属性にも出ない（表示の経路が無いことの固定）', { timeout: 300000 }, async () => {
  const c = cases.find((x) => x.id === 'XSS-01') ?? cases[0];
  const { ir, counts } = attrOnlyVariant(baseIr, c.payload);
  assert.equal(counts.setAttr, 2);
  assert.ok(counts.attrs >= 18);
  await withDeliveryCopy(async (copy) => {
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), JSON.stringify(ir), 'utf8');
    for (const start of STARTS.map((s) => s.id)) {
      await withServer({ dir: copy, base: BASE }, async (srv) => {
        const label = `attr-only ${start}`;
        // パスは元のまま。操作が最後まで通る（put と set_attr が受け付けられた）うえで、行は元のパスの文字として出る。
        const snaps = await runDrive(srv, start, identity, label, {
          extra: async (page) => {
            const text = await page.evaluate(() => document.body.textContent ?? '');
            assert.ok(!textHas(text, c.payload), `${label}: the attribute input appeared on screen (a display path now exists: add it to the judgement)`);
            const inAttr = await page.evaluate((p) => {
              const strip = (s) => String(s).replaceAll('\u0000', '');
              return [...document.querySelectorAll('*')].some((e) => [...e.attributes].some((a) => strip(a.value).includes(strip(p))));
            }, c.payload);
            assert.equal(inAttr, false, `${label}: the attribute input appeared in an element attribute`);
          },
        });
        assert.deepEqual(judgeAll(snaps, identity), [], `${label}: rows`);
      });
    }
  });
});

test('D1: 対照。実際の行へ描画を通さない生の入力を入れると、数え方が 0 でなく数える', { timeout: 300000 }, async () => {
  const byId = new Map(cases.map((c) => [c.id, c.payload]));
  const { ir } = e3Variant(baseIr, 'ok');
  await withDeliveryCopy(async (copy) => {
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), JSON.stringify(ir), 'utf8');
    await withServer({ dir: copy, base: BASE }, async (srv) => {
      for (const id of ['XSS-01', 'XSS-07']) {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        try {
          await openStart(page, srv, 's-0-1');
          await page.waitForSelector(TITLE, { timeout: T });
          await mouseDriver(page).choose('opt-right');
          await waitRows(page, 1);
          await page.evaluate(([sel, html]) => {
            document.querySelector(sel).innerHTML = html;
          }, [ROWS, byId.get(id)]);
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
  const { pathMap } = e3Variant(baseIr, P);
  await withDeliveryCopy(async (copy) => {
    const plain = JSON.parse(JSON.stringify(baseIr));
    plain.pack = { ...plain.pack, id: PACK_ID };
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), JSON.stringify(plain), 'utf8');
    for (const start of STARTS.map((s) => s.id)) {
      const snaps = await withServer({ dir: copy, base: BASE }, (srv) => runDrive(srv, start, identity, `control ${start}`));
      for (const s of snaps) {
        // 行が 0 件の観察点は空の集合への判定で、何も見ない（対照にならない）ので数えない。
        if (s.names.length > 0) {
          assert.notDeepEqual(judgeRows(s.obs, s.names.map((n) => pathMap[n])), [], `${start} ${s.name}: row judgement should fail on an unvaried IR`);
        }
        if (s.received) {
          assert.notDeepEqual(judgeReceived(s.obs, pathMap[s.received]), [], `${start} ${s.name}: received judgement should fail on an unvaried IR`);
        }
      }
      // 操作と判定が元のパスでは通る（対照の操作の列が正しく動いている確認）。
      assert.deepEqual(judgeAll(snaps, identity), [], `${start}: control rows`);
    }
  });
});
