// 誤入力の拒否が、右のログに理由と次の行動の行として出ることの試験（TERM-32。TASK-437 1/2）。
//
// 試験の設定（TEST_HOOKS=1）の画面は、試験データの教材（min-3step）と開始直後の状態で始まる。誤入力は
// testdata の min-3step-pack/tests/playthrough.toml の `[[error_inputs]]`（7 分類・7 件。error-inputs.mjs が読む）で、
// 書き写さない。1 件ごとに新しい画面を開き（処理中の状態を次の件へ持ち越さない）、`pre` → 本体の順に、
// 見えない `raw-trigger` へ `Op` の JSON を書いて click する（three-views・log-lines-xss と同じ口。入力欄の結線は TASK-438）。
//
// 確かめること（CSP あり・なしの両方）:
//   - 拒否 1 件につき `data-kind="rejected"` の行がちょうど 1 件あり、分類が期待どおりである
//   - 行の子は `line-reason`・`line-next-action` の 2 つだけで、どちらも空白以外を含む文字ノード 1 つである
//   - 文字は、エンジンの書き出し（`judgedCategories` の `reason`・`nextAction`）と一致する（画面は文面を作らない）
//   - 危険な要素が 0 件で、実行・CSP 違反・ダイアログが 0 件である
// 47 件の誤入力（試作パック）は公開の可否が未決のため使わない（docs/testing.md）。`e-busy` は `pre` の受付の後の
// 処理中の窓で送るため、窓を外れると分類が `busy` にならず失敗する（偶発的に通ることはない）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, DANGER_TAGS,
} from './harness.mjs';
import { loadErrorInputs } from './error-inputs.mjs';

const BASE = '/fandhe-reject-lines/';
const RAW = '[data-testid="raw-trigger"]';
const LOG = '[data-testid="part-log-list"]';
const IR = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../testdata/engine-event-consistency/test-data/min-3step.json');
const FORBIDDEN = [
  'a', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'pre', 'code', 'strong', 'em', 'b', 'i', 'blockquote', 'ul', 'ol', 'table', 'hr',
  'details', ...DANGER_TAGS,
].join(', ');

let browser;
let cases;
let startNodes;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  cases = loadErrorInputs();
  assert.equal(cases.length, 7);
  const ir = JSON.parse(fs.readFileSync(IR, 'utf8'));
  startNodes = Object.fromEntries(ir.starts.map((s) => [s.id, s.node]));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// `Op` の JSON は文字列としてだけ渡し、コードの文字列を組み立てない。
async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

const engineOf = (page) => page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine);

// 送った操作がログに届くまで待つ。
async function sendAndWait(page, op) {
  const before = (await engineOf(page)).logLen;
  await sendRaw(page, op);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen > n, before);
}

// 位置 `event` の行と、エンジンの書き出しを同じ evaluate で読む（進行のタイマーが間に入らない）。
async function readRow(page, event) {
  return page.evaluate(([log, ev, forbidden]) => {
    const engine = JSON.parse(window.__fandheTest.stateJson()).engine;
    const li = document.querySelector(`${log} > li[data-event="${ev}"]`);
    return {
      judged: engine.judgedCategories.find((j) => j.event === ev) ?? null,
      judgedCount: engine.judgedCategories.length,
      rejectedRows: document.querySelectorAll(`${log} > li[data-kind="rejected"]`).length,
      found: li !== null,
      kind: li?.dataset.kind ?? null,
      category: li?.dataset.category ?? null,
      children: li ? [...li.children].map((c) => c.dataset.testid) : [],
      childKinds: li ? [...li.children].map((c) => [...c.childNodes].map((n) => n.nodeType)) : [],
      texts: li ? [...li.children].map((c) => c.textContent) : [],
      forbidden: li ? li.querySelectorAll(forbidden).length : -1,
    };
  }, [LOG, event, FORBIDDEN]);
}

async function runCase(page, rec, c) {
  const start = await engineOf(page);
  assert.equal(start.current, startNodes[c.start], `${c.id}: 開始点`);
  for (const pre of c.pre) await sendAndWait(page, pre);
  const judgedBefore = (await engineOf(page)).judgedCategories.length;
  await sendRaw(page, c.op);
  await page.waitForFunction((n) => JSON.parse(window.__fandheTest.stateJson()).engine.judgedCategories.length > n, judgedBefore);
  const e = (await engineOf(page)).judgedCategories;
  const last = e[e.length - 1];
  await page.waitForSelector(`${LOG} > li[data-event="${last.event}"]`);
  const r = await readRow(page, last.event);
  const d = `${c.id} ${JSON.stringify(r)}`;
  assert.equal(r.judged.category, c.category, d);
  assert.equal(r.found, true, d);
  assert.equal(r.kind, 'rejected', d);
  assert.equal(r.category, c.category, d);
  assert.deepEqual(r.children, ['line-reason', 'line-next-action'], d);
  assert.deepEqual(r.childKinds, [[3], [3]], d); // 文字ノード 1 つずつ
  assert.equal(r.forbidden, 0, d);
  const [reason, next] = r.texts;
  assert.ok(reason.trim() !== '' && next.trim() !== '', `空の理由・次の行動: ${d}`);
  // 画面は文面を作らない: エンジンの値と一致する。
  assert.equal(reason, r.judged.reason, d);
  assert.equal(next, r.judged.nextAction, d);
  assert.equal(r.rejectedRows, r.judgedCount, `拒否の件数と行の件数: ${d}`);
  assert.deepEqual(rec.hits, [], d);
}

for (const csp of [true, false]) {
  test(`誤入力 7 件の拒否が理由と空でない次の行動の行として出る（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      for (const c of cases) {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        await page.goto(srv.url);
        await waitMounted(page);
        await runCase(page, rec, c);
        const counts = await collectSafety(page, rec);
        assert.deepEqual(nonZero(counts), [], `${c.id} ${JSON.stringify({ counts, rec })}`);
        await context.close();
      }
    }));
  });
}
