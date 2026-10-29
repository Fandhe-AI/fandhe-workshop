// URL のクエリとハッシュに入れた注入用の文字列が、配信する設定の画面で何も実行されず、画面のどこにも出ず、
// 通信も増やさないことの試験（SAFE-7・SAFE-33・D-68。TASK-147 2/2。CSP あり・なし）。
//
// データの正は testdata の extra-cases.json（url-cases.mjs が sha256 を照合して読む）。7 位置（query の ir・os・osmode・store・tick・
// 本体が読まない x、と hash）× ペイロード 8 件 = 56 回と、`?ir=` の固定の値 5 件を足した 61 回を、CSP あり・なしの両方で行う。
// 受け入れ条件は、61 回のすべてで実行 0・配信元以外への通信 0・CSP 違反 0 で、URL の値が画面のどこにも出ない（61 回中 0 回）こと。
//
// `?ir=` の文言の読み替え。SAFE-7 には「`?ir=` の値が `./ir/<値>.json` の相対パスとして扱われる」とあるが、これは D-68（2026-10-21）より前の
// 観察（PoC-7）である。D-68 と SAFE-33 で IR の場所は固定の `./ir/main.json` に決まり、本体は `?ir=` を読まない。
// そこで本試験は文言より強い現在の性質を確かめる: 値がどうであっても、取得する資源の集合がクエリなしと同じで、`ir/main.json` をちょうど
// 1 件だけ取得し、値から作ったパス（囮の JSON）を一度も取得せず、外部への通信が 0 件。PoC では 61 回のうち 13 回が `?ir=` を読んで起動しなかったが、
// 今は 61 回すべてがマウントするはずなので、マウントを必須の条件にする。
//
// 組み立ての取り決め: query は encodeURIComponent した値を付ける（`#` を含む値がハッシュに切られず、`URLSearchParams.get` が元の値に戻る）。
// hash も同じ。`?ir=` の固定の値は再符号化せず、そのまま付ける（IR-5 は意図して符号化済み）。`start` は付けない（direct-link-start・ir-location が担う）。
// 対象は配信する設定だけ。試験の設定は `?fixture=` 等を読むため「本体が読まない名前」の前提が成り立たない。
//
// 判定（1 回ごと）: マウントする・試験用の口が無い・値が location に届いている（空振りの防止）・画面の走査（HTML 全体・全属性の値・
// 文字・title・入力欄の値・コンソール）で印が 0 件・実行 0・計数 0・通信が基準（クエリなし）と同じ・Step の題と画面の文字が基準と同じ。
// 対照 D1（走査が印を数える）・D3（囮の取得を検出する）。D2（到達の確認）は各回の中で行い、C で 7 位置すべてに届いたことを集計する。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1。配信側の確認用）。未設定・不在は skip にせず失敗にする。
// 書き込み先は mkdtemp の一時ディレクトリの固定のパスだけで、ペイロードからパス・セレクタ・コードを作らない。
// 失敗の文には回の ID・位置・CSP だけを出し、ペイロードの全文は出さない。URL は example.invalid だけを使う（ペイロード側）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, requestKinds,
} from './harness.mjs';
import { pageRequestProblems } from './delivery.mjs';
import { irFetchCheck } from './l1-l2-runner.mjs';
import { loadBaseIr, withStarts } from './e2-ir.mjs';
import { loadUrlCases, buildRuns, EXPECTED_RUNS } from './url-cases.mjs';

const BASE = '/fandhe-url-xss/';
const PACK_ID = 'safe7-url';
const TITLE = '[data-testid="step-title"]';
const CSP_META_RE = /<meta\b[^>]*\bhttp-equiv\s*=\s*["']?content-security-policy["']?[^>]*>/gi;
const T = 30000;
// 値から読み込み先を作ったなら取得されうる場所（BASE からの相対）。どれも本体は取得しないはず。
const DECOYS = ['ir/min-3step.json', 'ir/something.json', 'ir/XSS-IR3.json', 'XSS-IR3.json', 'min-3step.json', 'something.json', 'main.json'];

let browser;
let runs;
let ir;
const passed = new Map(); // csp -> 合格した回数
const reached = new Map(); // csp -> Set(positionId)

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  runs = buildRuns(loadUrlCases());
  assert.equal(runs.length, EXPECTED_RUNS);
  ir = withStarts(loadBaseIr());
  ir.pack = { ...ir.pack, id: PACK_ID };
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 配信する設定の一時の写しに fandhe-ir の meta・ir/main.json・囮の JSON を足して fn に渡し、成否にかかわらず消す。
async function withDeliveryCopy(fn) {
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-e6-web-'));
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
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), JSON.stringify(ir), 'utf8');
    for (const d of DECOYS.filter((x) => x !== 'main.json')) fs.writeFileSync(path.join(copy, d), '{"decoy":"DECOY-MARK"}');
    fs.writeFileSync(path.join(copy, 'main.json'), '{"decoy":"DECOY-MARK"}');
    return await fn(copy);
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}

// 画面の走査。印が現れた回数の合計を返す。location は読まない。useDetached が真なら、印を属性と文字に入れた切り離した要素を走査する（D1）。
const scanPage = (page, needles, useDetached = false) => page.evaluate(([ns, detached]) => {
  let root = document.documentElement;
  const extra = [];
  if (detached) {
    root = document.createElement('div');
    root.innerHTML = `<img title="${ns[0]}"><p>${ns[0]}</p>`;
  } else {
    extra.push(document.title);
    for (const el of document.querySelectorAll('input, textarea')) extra.push(el.value);
  }
  const hay = [root.outerHTML, root.textContent, ...extra];
  for (const el of [root, ...root.querySelectorAll('*')]) for (const a of el.attributes) hay.push(a.name, a.value);
  let count = 0;
  for (const n of ns) for (const h of hay) count += String(h).split(n).length - 1;
  return count;
}, [needles, useDetached]);

const readShown = (page) => page.evaluate((title) => ({
  title: document.querySelector(title)?.textContent ?? null,
  text: document.body.innerText,
}), TITLE);

async function openOne(page, srv, url, consoleAll) {
  page.on('console', (m) => consoleAll.push(m.text()));
  await page.goto(url);
  await waitMounted(page);
  await page.waitForSelector(TITLE, { timeout: T });
  await page.waitForLoadState('networkidle');
}

function decoyHits(rec, srv) {
  const paths = new Set(DECOYS.map((d) => `${srv.origin}${BASE}${d}`));
  return rec.requests.filter((r) => paths.has(r.url.replace(/[?#].*$/, ''))).length;
}

const short = (e) => String(e?.message ?? e).replaceAll(/\s+/g, ' ').slice(0, 300);
const allNeedles = () => [...new Set(runs.flatMap((r) => r.needles))];

// クエリなしで開いた基準。要求の種類・画面・印が無いこと（空振りの防止）を記録する。
async function baseline(srv, kindOpts) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    const consoleAll = [];
    await openOne(page, srv, srv.url, consoleAll);
    const shown = await readShown(page);
    const found = await scanPage(page, allNeedles());
    assert.equal(found, 0, 'baseline page must not contain any needle');
    return { kinds: requestKinds(rec, kindOpts), ...shown };
  } finally {
    await context.close();
  }
}

for (const csp of [true, false]) {
  test(`E6 の ${EXPECTED_RUNS} 回が配信する設定の URL から入り、実行・通信の増加・画面への表示が 0 件（CSP ${csp ? 'あり' : 'なし'}）`, { timeout: 900000 }, async (t) => {
    const failures = [];
    let ok = 0;
    const seen = new Set();
    const perPosition = {};
    await withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const kindOpts = { origin: srv.origin, base: BASE };
      const base = await baseline(srv, kindOpts);
      assert.ok(base.kinds.some((k) => k.startsWith('ir/main.json ')), 'baseline fetches ir/main.json');
      for (const run of runs) {
        const label = `${run.runId} csp=${csp ? 'on' : 'off'}`;
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        try {
          const consoleAll = [];
          await openOne(page, srv, `${srv.url}${run.query}${run.hash}`, consoleAll);
          assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'no test hooks');
          // 到達の確認: 値が実際に location に届いている。届いていなければ空振りなので不合格にする。
          const got = await page.evaluate(([where, name]) => (where === 'query'
            ? new URLSearchParams(location.search).get(name)
            : decodeURIComponent(location.hash.slice(1))), [run.where, run.name]);
          const want = run.where === 'query' ? new URLSearchParams(run.query).get(run.name) : run.expected;
          assert.equal(got, want, `${label}: value did not reach location`);
          seen.add(run.positionId);
          // 画面の走査: URL の値が画面のどこにも出ない。
          assert.equal(await scanPage(page, run.needles), 0, `${label}: needle shown on screen`);
          assert.equal(consoleAll.filter((m) => run.needles.some((n) => m.includes(n))).length, 0, `${label}: needle in console`);
          const counts = await collectSafety(page, rec);
          assert.deepEqual(nonZero(counts), [], `${label}: ${JSON.stringify(counts)}`);
          assert.deepEqual(rec.hits, [], `${label}: executed`);
          // 通信: ir/main.json がちょうど 1 件・外部 0・囮 0・要求の種類が基準と同じ。
          const chk = irFetchCheck(rec, `${srv.url}ir/main.json`);
          assert.equal(chk.irCount, 1, `${label}: ir/main.json request count`);
          assert.ok(chk.irOk, `${label}: ir/main.json status`);
          assert.deepEqual(pageRequestProblems({ ...rec, failed: chk.failed }, { origin: srv.origin, base: BASE }), [], `${label}: request problems`);
          assert.deepEqual(rec.external, [], `${label}: external`);
          assert.equal(decoyHits(rec, srv), 0, `${label}: decoy fetched`);
          assert.deepEqual(requestKinds(rec, kindOpts), base.kinds, `${label}: request kinds differ from baseline`);
          // 画面が URL の値で変わらない。
          const shown = await readShown(page);
          assert.equal(shown.title, base.title, `${label}: step title differs`);
          assert.equal(shown.text, base.text, `${label}: screen text differs`);
          ok += 1;
          perPosition[run.positionId] = (perPosition[run.positionId] ?? 0) + 1;
        } catch (e) {
          failures.push(`${label}: ${short(e)}`);
        } finally {
          await context.close();
        }
      }
    })));
    t.diagnostic(`csp=${csp}: ${ok}/${runs.length} ${JSON.stringify(perPosition)}`);
    passed.set(csp, ok);
    reached.set(csp, seen);
    assert.deepEqual(failures, []);
    assert.equal(ok, runs.length);
  });
}

test(`C: 合格が CSP あり・なしのそれぞれで ${EXPECTED_RUNS} 回中 ${EXPECTED_RUNS} 回で、値が 7 位置すべてに届いた`, () => {
  for (const csp of [true, false]) {
    assert.equal(passed.get(csp) ?? -1, EXPECTED_RUNS, `csp=${csp ? 'on' : 'off'}: ${passed.get(csp)}/${EXPECTED_RUNS} (run the tests above first)`);
    assert.equal(reached.get(csp)?.size, 7, `csp=${csp ? 'on' : 'off'}: positions reached`);
  }
});

test('D1: 対照。画面の走査は、印が属性と文字にあれば 0 より大きく数える', async () => {
  await withDeliveryCopy((copy) => withServer({ dir: copy, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await openOne(page, srv, srv.url, []);
      assert.ok((await scanPage(page, ['MARK-D1'], true)) >= 3, 'attribute (outerHTML, attr value) and text are counted');
      assert.equal(await scanPage(page, ['MARK-D1'], false), 0);
    } finally {
      await context.close();
    }
  }));
});

test('D3: 対照。値から作ったパスの取得（囮）が起きれば、囮の検出が働く', async () => {
  await withDeliveryCopy((copy) => withServer({ dir: copy, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await openOne(page, srv, srv.url, []);
      assert.equal(decoyHits(rec, srv), 0);
      await page.evaluate(() => fetch('./ir/min-3step.json').then((r) => r.text()));
      assert.equal(decoyHits(rec, srv), 1);
    } finally {
      await context.close();
    }
  }));
});
