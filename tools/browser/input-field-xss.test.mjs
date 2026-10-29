// 入力欄に入れた HTML 風の文字列が、配信する設定の画面で何も実行されず、右のログに文字として出ることの試験
// （SAFE-5・TERM-21。TASK-147 1/2。CSP あり・なし）。
//
// データの正は testdata の xss-cases.md の「入り口に E5（入力欄）を含む」49 件（xss-cases.mjs の e5Payloads が読む。
// ペイロードをここへ書き写さない）。1 件を「そのまま」「`/` の後ろ」「コマンドの引数」の 3 通りで送る（49 × 3 = 147 回）。
// 受け入れ条件は、147 回のすべてで実行 0・外部通信 0・CSP 違反 0 で、入力が右のログに文字として出る（要素にならない）こと。
//
// log-lines-xss.test.mjs（TASK-434 2/2）との違い: あちらは試験の設定の見えない `raw-trigger` へ `Op` を書く口で、状態との突き合わせを担う。
// 本試験は (a) 本物の入力欄（draft-input に入れて Enter。host の接頭辞による分類を通る）から、(b) 試験用の口が無い配信する設定で、
// (c) 通信が配信物の構成要素と ir/main.json 1 件だけであることまで確かめる。観察は DOM だけで行う。
//
// 注入の口: 配信する設定の web の一時の写しに `fandhe-ir` の meta と `ir/main.json`（開始点 s-1-4 を足した ch0_3 の IR）を置き、
// `?start=s-1-4`（Step st-1-4。`/new-idea [name]`。入力欄がある）で開く。E5 の 49 件は `/` で始まらず kebab_case にも合わないので、
// 3 通りのどれも受け付けられず（拒否）、Step は進まない。どの種類の拒否になるかは assert せず記録だけにする。
//
// 判定（1 回ごと）: 最後の受付の行が data-kind=received・data-input が送り方どおり・子が line-body だけ・本文が文字ノード 1 つだけで
// 入力欄から読み戻した値と完全に一致（連結が起きない）・禁止の要素 0 件。続く拒否・問い返しの行は子が文字ノードだけで禁止の要素 0 件。
// Step の題が変わらない。秘密情報の警告が出ない。記録用の関数の呼び出し（実行）が 0。全体で計数 0・通信が構成要素と ir/main.json 1 件だけ。
// 対照 D1（数え方が解釈された要素を数える）・D2（3 通りが別の経路に分かれ拒否が出る）・D3（正しい形のコマンドは受け付けられ Step が進む）。
//
// 入力: FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1。配信側の確認用）。未設定・不在は skip にせず失敗にする。
// 書き込み先は mkdtemp の一時ディレクトリの固定のパス（ir/main.json）だけで、ペイロードからパス・セレクタ・コードを作らない。
// 失敗の文には件の ID・送り方・CSP だけを出し、ペイロードの全文は出さない。URL は example.invalid だけを使う（ペイロード側）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, DANGER_TAGS,
} from './harness.mjs';
import { pageRequestProblems } from './delivery.mjs';
import { irFetchCheck } from './l1-l2-runner.mjs';
import { DRAFT, LOG } from './ch0-3-journey.mjs';
import { e5Payloads } from './xss-cases.mjs';
import { loadBaseIr, withStarts } from './e2-ir.mjs';

const BASE = '/fandhe-input-field-xss/';
const PACK_ID = 'safe5-input';
const START = 's-1-4';
const TITLE = '[data-testid="step-title"]';
const ITEMS = `${LOG} > li`;
const RECEIVED = `${ITEMS}[data-kind="received"]`;
const BODY = '[data-testid="line-body"]';
const SECRET = '[data-testid="secret-notice"]';
const CHOICE_BTN = '[data-testid="choice-btn"]';
const CSP_META_RE = /<meta\b[^>]*\bhttp-equiv\s*=\s*["']?content-security-policy["']?[^>]*>/gi;
const T = 30000;
// ログの行の中にあってはならない要素（log-lines-xss と同じ集合）。
const FORBIDDEN = [
  'a', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'pre', 'code', 'strong', 'em', 'b', 'i', 'blockquote', 'ul', 'ol', 'table', 'hr',
  'details', ...DANGER_TAGS,
].join(', ');
// 送り方（TASK-147 の 3 通り）。コマンドの引数は Step 自身のコマンドを使い、引数の解析と拒否の理由への差し込みを通す。
const MODES = [
  { name: 'そのまま', input: 'text', make: (p) => p },
  { name: '/ の後ろ', input: 'command', make: (p) => `/${p}` },
  { name: 'コマンドの引数', input: 'command', make: (p) => `/new-idea ${p}` },
];

let browser;
let cases;
let ir;
const passed = new Map(); // csp -> 合格した回数（C が集計する）
const rejectKinds = new Map(); // 記録だけ

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  cases = e5Payloads();
  assert.equal(cases.length, 49);
  // 3 通りのどれも受け付けられない前提（`/` で始まらず、kebab_case に合わない）。崩れたら Step が進み判定が別物になる。
  for (const c of cases) {
    assert.ok(!c.payload.startsWith('/'), `${c.id}: starts with /`);
    assert.ok(!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(c.payload), `${c.id}: matches kebab-case`);
  }
  ir = withStarts(loadBaseIr());
  ir.pack = { ...ir.pack, id: PACK_ID };
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 配信する設定の一時の写しに fandhe-ir の meta と ir/main.json を足して fn に渡し、成否にかかわらず消す（structure-xss と同じ足し方）。
async function withDeliveryCopy(fn) {
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-e5-web-'));
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
    return await fn(copy);
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}

async function openStart(page, srv) {
  await page.goto(`${srv.url}?start=${START}`);
  await waitMounted(page);
  assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'no test hooks');
  await page.waitForSelector(TITLE, { timeout: T });
  return page.evaluate((sel) => document.querySelector(sel).textContent, TITLE);
}

function assertNetwork(rec, srv, label) {
  const chk = irFetchCheck(rec, `${srv.url}ir/main.json`);
  assert.equal(chk.irCount, 1, `${label}: ir/main.json request count`);
  assert.ok(chk.irOk, `${label}: ir/main.json status`);
  const problems = pageRequestProblems({ ...rec, failed: chk.failed }, { origin: srv.origin, base: BASE });
  assert.deepEqual(problems, [], `${label}: ${JSON.stringify(problems)}`);
}

async function assertQuiet(page, rec, label) {
  const counts = await collectSafety(page, rec);
  const detail = `${label}: ${JSON.stringify({ counts, hits: rec.hits.length })}`;
  assert.deepEqual(nonZero(counts), [], detail);
  assert.deepEqual(rec.hits, [], detail);
}

// 最後の受付の行と、その直後の行（あれば）、Step の題、秘密情報の警告の数を 1 回の evaluate で読む。
const observe = (page) => page.evaluate(([items, received, body, forbidden, title, secret, choice]) => {
  const describe = (li) => {
    if (!li) return null;
    const b = li.querySelector(body);
    return {
      kind: li.dataset.kind,
      input: li.dataset.input ?? null,
      children: [...li.children].map((c) => c.dataset.testid),
      childKinds: [...li.children].map((c) => [...c.childNodes].map((n) => n.nodeType)),
      bodyKinds: b ? [...b.childNodes].map((n) => n.nodeType) : [],
      text: b ? b.textContent : null,
      forbidden: li.querySelectorAll(forbidden).length,
    };
  };
  const recs = [...document.querySelectorAll(received)];
  const li = recs[recs.length - 1];
  return {
    received: recs.length,
    total: document.querySelectorAll(items).length,
    line: describe(li),
    next: describe(li ? li.nextElementSibling : null),
    title: document.querySelector(title)?.textContent ?? null,
    secret: (document.querySelector(secret)?.textContent ?? '').length,
    choices: document.querySelectorAll(choice).length,
  };
}, [ITEMS, RECEIVED, BODY, FORBIDDEN, TITLE, SECRET, CHOICE_BTN]);

// 入力欄に入れて Enter で送る。受付の行が 1 つ増え、続く行（拒否・問い返し）が出揃って行の数が落ち着くまで待つ。
// 続く行は同じ更新で描かれるが、固定の sleep は使わず、行の総数が連続する 3 回の読みで変わらないことを待つ。
async function sendOnce(page, value, before) {
  await page.fill(DRAFT, value);
  const sent = await page.inputValue(DRAFT);
  await page.press(DRAFT, 'Enter');
  await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [RECEIVED, before + 1], { timeout: T });
  await page.waitForFunction(([items]) => {
    const w = window;
    const n = document.querySelectorAll(items).length;
    if (w.__e5Last === n) w.__e5Same = (w.__e5Same ?? 0) + 1; else { w.__e5Last = n; w.__e5Same = 0; }
    return w.__e5Same >= 3;
  }, [ITEMS], { polling: 50, timeout: T });
  return { sent, obs: await observe(page) };
}

function judge(obs, sent, mode, title0) {
  const bad = [];
  const l = obs.line;
  if (!l) return ['no-received-line'];
  if (l.kind !== 'received') bad.push('kind');
  if (l.input !== mode.input) bad.push(`input=${l.input}`);
  if (JSON.stringify(l.children) !== '["line-body"]') bad.push('children');
  if (JSON.stringify(l.bodyKinds) !== '[3]') bad.push('body-not-single-text-node');
  if (l.text !== sent) bad.push('text-mismatch');
  if (l.forbidden !== 0) bad.push('forbidden-elements');
  const n = obs.next;
  // 続く行（拒否・問い返し）の欠落は不合格にする。欠落を許すと、拒否理由への差し込み経路を通ったことを保証できない。
  if (!n || (n.kind !== 'rejected' && n.kind !== 'clarified')) {
    bad.push(`next-missing:${n?.kind ?? 'none'}`);
  } else {
    if (n.forbidden !== 0) bad.push('next-forbidden');
    if (n.childKinds.some((k) => k.some((t) => t !== 3))) bad.push('next-not-text');
  }
  // 受け付けられると確認の選択肢が出る（この Step は取り消しと引数なしだけを受け付ける）。拒否なら出ない。
  if (obs.title !== title0) bad.push('step-moved');
  if (obs.choices !== 0) bad.push('accepted');
  if (obs.secret !== 0) bad.push('secret-warning');
  return bad;
}

const short = (e) => String(e?.message ?? e).replaceAll(/\s+/g, ' ').slice(0, 300);

for (const csp of [true, false]) {
  test(`E5 の 49 件 × 3 通りが配信する設定の入力欄から送られ、ログに文字として出て、実行・通信・危険な要素が 0 件（CSP ${csp ? 'あり' : 'なし'}）`, { timeout: 900000 }, async (t) => {
    const failures = [];
    let ok = 0;
    await withDeliveryCopy((copy) => withCspMode({ dir: copy, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        const title0 = await openStart(page, srv);
        let n = (await observe(page)).received;
        assert.equal(n, 0);
        const kinds = {};
        for (const c of cases) {
          for (const m of MODES) {
            const label = `${c.id} ${m.name} csp=${csp ? 'on' : 'off'}`;
            try {
              const { sent, obs } = await sendOnce(page, m.make(c.payload), n);
              n += 1;
              const bad = judge(obs, sent, m, title0);
              assert.deepEqual(bad, [], `${label}: ${bad.join(',')}`);
              assert.deepEqual(rec.hits, [], `${label}: executed`);
              const k = obs.next?.kind ?? 'none';
              kinds[`${m.input}/${k}`] = (kinds[`${m.input}/${k}`] ?? 0) + 1;
              ok += 1;
            } catch (e) {
              failures.push(`${label}: ${short(e)}`);
              n = (await observe(page)).received;
            }
          }
        }
        rejectKinds.set(csp, kinds);
        assert.equal((await observe(page)).received, cases.length * MODES.length);
        await assertQuiet(page, rec, `final csp=${csp ? 'on' : 'off'}`);
        assertNetwork(rec, srv, `final csp=${csp ? 'on' : 'off'}`);
      } catch (e) {
        failures.push(`run csp=${csp ? 'on' : 'off'}: ${short(e)}`);
      } finally {
        await context.close();
      }
    })));
    t.diagnostic(`csp=${csp}: ${ok}/${cases.length * MODES.length} ${JSON.stringify(rejectKinds.get(csp))}`);
    passed.set(csp, ok);
    assert.deepEqual(failures, []);
    assert.equal(ok, cases.length * MODES.length);
  });
}

test('C: 合格が CSP あり・なしのそれぞれで 147 回中 147 回', () => {
  for (const csp of [true, false]) {
    assert.equal(passed.get(csp) ?? -1, 147, `csp=${csp ? 'on' : 'off'}: ${passed.get(csp)}/147 (run the tests above first)`);
  }
});

test('D1: 対照。禁止の要素の数え方は、解釈された要素があれば 0 より大きく数える', async () => {
  await withDeliveryCopy((copy) => withServer({ dir: copy, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await openStart(page, srv);
      // 切り離した要素に当てる（img に src は付けず、配信元以外へ取りに行かない）。
      const n = await page.evaluate((forbidden) => {
        const li = document.createElement('li');
        li.innerHTML = '<img><svg></svg><a href="#x">l</a>';
        return li.querySelectorAll(forbidden).length;
      }, FORBIDDEN);
      assert.equal(n, 3);
    } finally {
      await context.close();
    }
  }));
});

test('D2: 対照。固定の無害な入力は 3 通りで別の経路に分かれ、拒否の行が出て Step が進まない', async () => {
  await withDeliveryCopy((copy) => withServer({ dir: copy, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      const title0 = await openStart(page, srv);
      let n = 0;
      for (const m of MODES) {
        const { sent, obs } = await sendOnce(page, m.make('abc def'), n);
        n += 1;
        assert.deepEqual(judge(obs, sent, m, title0), [], m.name);
        assert.equal(obs.next?.kind, 'rejected', `${m.name}: a rejection line follows`);
      }
    } finally {
      await context.close();
    }
  }));
});

test('D3: 対照。受け付けられる操作（引数なしの送信の問い返しと Esc の取り消し）では確認の選択肢が出て、「受け付けられない」の判定が働く', async () => {
  await withDeliveryCopy((copy) => withServer({ dir: copy, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      const title0 = await openStart(page, srv);
      await page.fill(DRAFT, '/new-idea');
      await page.press(DRAFT, 'Enter');
      await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 1, `${ITEMS}[data-kind="clarified"]`, { timeout: T });
      await page.waitForFunction((sel) => document.querySelector(sel)?.value === '', DRAFT, { timeout: T });
      await page.press(DRAFT, 'Escape');
      await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 2, CHOICE_BTN, { timeout: T });
      const obs = await observe(page);
      assert.ok(judge(obs, '/new-idea', MODES[2], title0).includes('accepted'));
    } finally {
      await context.close();
    }
  }));
});
