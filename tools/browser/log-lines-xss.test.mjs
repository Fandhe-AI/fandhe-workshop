// 右のログに、受講者の入力が文字として出ることの試験（TERM-21。TASK-434 2/2）。
//
// 入力欄に HTML 風の文字列を入れて送ると、ログには文字として出て、要素にならず、実行もされない。
// データの正は testdata の xss-cases.md の「入り口に E5（入力欄）を含む」49 件で、TASK-147 と共有する（xss-cases.mjs が読む。
// ペイロードをここへ書き写さない）。TASK-147 の送り方に合わせ、1 件を「そのまま」「`/` の後ろ」「コマンドの引数」の 3 通りで
// 送る（49 × 3 = 147 回）。CSP あり・なしの両方で、右のログの `received` の行が、文字ノード 1 つだけの本文を持ち、
// 危険な要素が 0 件で、実行・外部通信・CSP 違反・ダイアログ・ポップアップ・ページの例外が 0 件であることを確かめる。
//
// 注入の口: 入力欄からエンジンへの結線は TASK-438 が持つ。それまでは、試験の設定（TEST_HOOKS=1）の見えない `raw-trigger` へ
// `Op` の JSON を書いて click する（three-views.test.mjs と同じ口）。口は sendPayload の 1 か所にまとめてあり、TASK-438 の後は
// ここを draft-input + submit-input の操作へ差し替える。
// 範囲: 今の結線は受付の行（InputReceived）だけ。拒否・問い返し（`{{input_head}}` の差し込みの経路）は TASK-437 の結線の後に、
// 観察の対象へ足す。教材の文面（進行の名前・拒否の理由など）の網羅は E2 の担当（TASK-146 系）で、全 6 種の行を文字ノードで
// 描くことは native の crates/web/tests/log_lines_contract.rs が代わる。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定。口が無いことの確認と CSP の試験）。どちらも未設定・
// 不在は skip にせず失敗にする。URL は example.invalid だけを使う（ペイロード側）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, DANGER_TAGS,
} from './harness.mjs';
import { e5Payloads } from './xss-cases.mjs';

const BASE = '/fandhe-log-lines-xss/';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const ITEMS = `${LOG} > li`;
const BODY = '[data-testid="line-body"]';
// ログの行の中にあってはならない要素。
const FORBIDDEN = [
  'a', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'pre', 'code', 'strong', 'em', 'b', 'i', 'blockquote', 'ul', 'ol', 'table', 'hr',
  'details', ...DANGER_TAGS,
].join(', ');
// 送り方（TASK-147 の 3 通り）。
const MODES = [
  { name: 'そのまま', input: 'text', make: (p) => ({ text: p }) },
  { name: '/ の後ろ', input: 'command', make: (p) => ({ command: `/${p}` }) },
  { name: 'コマンドの引数', input: 'command', make: (p) => ({ command: `/make-note ${p}` }) },
];

let browser;
let cases;

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
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 入力 1 件を送る（注入の口。TASK-438 の後は入力欄の操作へ差し替える）。
// ペイロードは JSON の文字列としてだけ渡し、コードの文字列を組み立てない。
async function sendPayload(page, input) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify({ input })]);
}

// 末尾の行を調べる。DOM と状態は 1 回の evaluate で読む（進行のタイマーが間に入らない）。
async function lastItem(page) {
  return page.evaluate(([items, body, forbidden]) => {
    const lis = [...document.querySelectorAll(items)];
    const li = lis[lis.length - 1];
    const b = li ? li.querySelector(body) : null;
    return {
      count: lis.length,
      kind: li?.dataset.kind ?? null,
      input: li?.dataset.input ?? null,
      children: li ? [...li.children].map((c) => c.dataset.testid) : [],
      bodyKinds: b ? [...b.childNodes].map((n) => n.nodeType) : [],
      text: b ? b.textContent : null,
      forbidden: li ? li.querySelectorAll(forbidden).length : -1,
    };
  }, [ITEMS, BODY, FORBIDDEN]);
}

function assertLine(it, expected, input, detail) {
  assert.equal(it.kind, 'received', detail);
  assert.equal(it.input, input, detail);
  assert.deepEqual(it.children, ['line-body'], detail);
  assert.deepEqual(it.bodyKinds, [3], detail); // 文字ノード 1 つだけ
  assert.equal(it.text, expected, detail);
  assert.equal(it.forbidden, 0, detail);
}

async function sendAll(page, rec) {
  let n = (await lastItem(page)).count;
  for (const c of cases) {
    for (const m of MODES) {
      await sendPayload(page, m.make(c.payload));
      n += 1;
      await page.waitForFunction(([sel, want]) => document.querySelectorAll(sel).length === want, [ITEMS, n]);
      const expected = m.make(c.payload)[m.input];
      assertLine(await lastItem(page), expected, m.input, `${c.id} ${m.name} ${JSON.stringify(rec.hits)}`);
      assert.deepEqual(rec.hits, [], `executed at ${c.id} ${m.name}`);
    }
  }
  return n;
}

async function assertSafe(page, rec) {
  const counts = await collectSafety(page, rec);
  assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  assert.deepEqual(rec.hits, [], JSON.stringify(rec));
}

// 状態の側の行（line が null でないもの）と DOM の行の数が一致し、受付の行がちょうど 147 であること。
async function assertLogMatchesState(page) {
  const r = await page.evaluate((items) => {
    const log = JSON.parse(window.__fandheTest.stateJson()).engine.log;
    return {
      dom: document.querySelectorAll(items).length,
      lines: log.filter((e) => e.line !== null && e.line !== undefined).length,
      received: document.querySelectorAll(`${items}[data-kind="received"]`).length,
    };
  }, ITEMS);
  assert.equal(r.dom, r.lines, JSON.stringify(r));
  assert.equal(r.received, cases.length * MODES.length, JSON.stringify(r));
}

async function startedPage(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  const start = await opened.page.evaluate((items) => ({
    n: document.querySelectorAll(items).length,
    loaded: JSON.parse(window.__fandheTest.stateJson()).engine.irLoaded,
  }), ITEMS);
  assert.deepEqual(start, { n: 0, loaded: true });
  return opened;
}

for (const csp of [true, false]) {
  test(`E5 の入力は右のログに文字のまま出て、実行・通信・危険な要素が 0 件（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await startedPage(srv);
      const n = await sendAll(page, rec);
      assert.equal(n, cases.length * MODES.length);
      await assertLogMatchesState(page);
      await assertSafe(page, rec);
      await context.close();
    }));
  });
}

test('再描画（HTML の文字列からの作り直し）の後も文字のまま出る', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await startedPage(srv);
    await sendAll(page, rec);
    assert.equal(await page.evaluate(() => window.__fandheTest.rerender()), true);
    const items = await page.$$eval(ITEMS, (lis, [body, forbidden]) => lis.map((li) => {
      const b = li.querySelector(body);
      return { kind: li.dataset.kind, text: b.textContent, kinds: [...b.childNodes].map((n) => n.nodeType), forbidden: li.querySelectorAll(forbidden).length };
    }), [BODY, FORBIDDEN]);
    assert.equal(items.length, cases.length * MODES.length);
    let i = 0;
    for (const c of cases) {
      for (const m of MODES) {
        const it = items[i];
        i += 1;
        const expected = m.make(c.payload)[m.input];
        // 作り直しは HTML の文字列から木を作る経路で、U+0000 は構文解析で落ちうる（XSS-47）。表示の安全性とは関係しない。
        const same = it.text === expected || it.text === expected.replaceAll('\u0000', '');
        assert.ok(same, `${c.id} ${m.name} rerender: ${JSON.stringify(it.text)}`);
        assert.equal(it.kind, 'received', c.id);
        assert.deepEqual(it.kinds, [3], c.id);
        assert.equal(it.forbidden, 0, c.id);
      }
    }
    await assertSafe(page, rec);
    await context.close();
  });
});

test('対照: 数え方は、解釈された要素があれば 0 より大きく数える', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    // 切り離した要素に当てる（img に src は付けず、配信元以外へ取りに行かない）。
    const n = await page.evaluate((forbidden) => {
      const li = document.createElement('li');
      li.innerHTML = '<img><svg></svg><a href="#x">l</a>';
      return li.querySelectorAll(forbidden).length;
    }, FORBIDDEN);
    assert.equal(n, 3);
    await context.close();
  });
});

test('配信設定の画面に試験の口（raw-trigger）が無い', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    assert.equal(await page.locator(RAW).count(), 0);
    await context.close();
  });
});
