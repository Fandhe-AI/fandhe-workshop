// keyed list の親要素の属性が、更新の後も `view()` の属性列と一致することの試験（TASK-359 1/2。FEND-15。PoC-1 の F-2 相当）。
//
// 上流（fandhe-frontend-wasm-client 0.6.1）は keyed list の適用のたびに親要素の属性を `view()` の列へ同期し
// （keyed_dom.rs の sync_parent_attrs・sync_attrs）、列に無い属性を予約属性 `data-bind-list` を除いて消す。
// 本試験は、親要素の参照を保持して、(i) 更新の後も同じ要素であること（全再描画でない）、(ii) JS で後付けした属性が
// 消えること、(iii) 属性の集合が `view()` の列（`data-testid` と `data-bind-list`）とちょうど一致することを確かめる。
// 後付けの属性は、その field が dirty になった更新（ログの長さが変わる操作・送信）で消える。
// 宣言済みの属性の値を JS が書き換えた場合は、キャッシュと同値なら 1 回の更新では戻らない（上流の同値スキップ。
// keyed_dom.rs の sync_attrs）ため、ここでは assert しない。
//
// 範囲: 部品がある 4 つのリスト（ログ・世界状態・復習・入力欄の送信済み一覧）。選択肢・チェックポイント・候補の一覧は
// 部品が未実装で、各部品の実装タスクが確かめる（親の `view()` への記述は `tests/list_parent_attrs.rs` が網羅で守る）。
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）と FANDHE_WEB_DIR（配信設定）。どちらも未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero, summarize,
} from './harness.mjs';

const BASE = '/fandhe-list-attrs/';
const RAW = '[data-testid="raw-trigger"]';
const DRAFT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const SAMPLE = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../testdata/contract/event-sequence.json');
// 親の選択子と、束縛される field 名・view() が書く属性（data-bind-list を除く）。
// 成果物の親はビューごとに 1 本で data-testid が共通のため、data-view でも絞る（TASK-470）。
// 既定の教材はビューを宣言しないので、ファイルツリーが選ばれ、他は hidden を持つ。
const tid = (id) => `[data-testid="${id}"]`;
const viewList = (kind) => `[data-testid="view-list"][data-view="${kind}"]`;
const ENGINE_LISTS = [
  { sel: tid('part-log-list'), field: 'engine_log_rows', attrs: { 'data-testid': 'part-log-list' } },
  {
    sel: viewList('file_tree'),
    field: 'view_file_tree',
    attrs: { 'data-testid': 'view-list', 'data-view': 'file_tree' },
  },
  {
    sel: viewList('document'),
    field: 'view_document',
    attrs: { 'data-testid': 'view-list', 'data-view': 'document', hidden: '' },
    untouched: true,
  },
  { sel: tid('part-review-list'), field: 'review_items', attrs: { 'data-testid': 'part-review-list' } },
];
const LEARNER_LIST = { sel: tid('log-list'), field: 'learner_log', attrs: { 'data-testid': 'log-list' } };

let browser;
let ops;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  const sample = JSON.parse(fs.readFileSync(SAMPLE, 'utf8'));
  ops = sample.entries.map((e) => e.op);
  assert.ok(ops.length > 0, 'sample has no ops');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function sendRaw(page, op) {
  await page.evaluate(([sel, json]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', json);
    el.click();
  }, [RAW, JSON.stringify(op)]);
}

const logLen = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine.logLen;

// 親要素の参照を window に保持し、後付けの属性を 3 つ付ける。
async function pin(page, sels) {
  await page.evaluate((ids) => {
    window.__pinned = {};
    for (const id of ids) {
      const el = document.querySelector(id);
      el.setAttribute('aria-live', 'polite');
      el.setAttribute('class', 'injected');
      el.setAttribute('data-injected', '1');
      window.__pinned[id] = el;
    }
  }, sels);
}

// 保持した参照が今の DOM の要素と同一か、後付けの属性が無いか、属性の集合を返す。
async function inspect(page, sel) {
  return page.evaluate((id) => {
    const el = document.querySelector(id);
    const attrs = {};
    for (const a of el.attributes) attrs[a.name] = a.value;
    return {
      same: window.__pinned[id] === el,
      connected: window.__pinned[id].isConnected,
      attrs,
    };
  }, sel);
}

function assertParent(info, { sel: testid, field, attrs, untouched }, detail) {
  assert.equal(info.same, true, `same element ${testid} ${detail}`);
  assert.equal(info.connected, true, `connected ${testid} ${detail}`);
  if (untouched) {
    // 選ばれていないビューの list は、中身だけが変わる更新では dirty にならず、親の属性を同期し直さない
    // （更新の範囲をビューごとに絞る。TASK-470 2/2）。後付けの属性が残ることが、触れていない証拠である。
    assert.equal(info.attrs['data-injected'], '1', `${testid} should not be re-synced ${detail}`);
    return;
  }
  for (const injected of ['aria-live', 'class', 'data-injected']) {
    assert.equal(injected in info.attrs, false, `${injected} should be gone on ${testid} ${detail}`);
  }
  assert.deepEqual(info.attrs, { ...attrs, 'data-bind-list': field }, `${testid} ${detail}`);
}

async function assertSafe(page, rec) {
  const counts = await collectSafety(page, rec);
  assert.deepEqual(nonZero(counts), [], JSON.stringify({ counts, rec }));
  const s = summarize(rec);
  assert.equal(s.csp + s.pageErrors + s.consoleErrors, 0, JSON.stringify(rec));
}

test('エンジンの 3 リストの親は、更新の後も同じ要素で、属性が view() の列と一致する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    await pin(page, ENGINE_LISTS.map((l) => l.sel));
    // 増えないと field が dirty にならず、何も確かめないことになる。
    const start = await logLen(page);
    let grew = false;
    for (const op of ops) {
      await sendRaw(page, op);
      if ((await logLen(page)) > start) {
        grew = true;
        break;
      }
    }
    assert.equal(grew, true, '操作列でログが 1 件も増えていない');
    for (const l of ENGINE_LISTS) assertParent(await inspect(page, l.sel), l, 'after update');

    // 全操作を流しても同じである。
    for (const op of ops) await sendRaw(page, op);
    for (const l of ENGINE_LISTS) assertParent(await inspect(page, l.sel), l, 'after all ops');

    // view() から作り直した結果とも一致する。
    assert.equal(await page.evaluate(() => window.__fandheTest.rerender()), true);
    for (const l of ENGINE_LISTS) {
      const attrs = await page.evaluate((i) => {
        const el = document.querySelector(i);
        return Object.fromEntries([...el.attributes].map((a) => [a.name, a.value]));
      }, l.sel);
      assert.deepEqual(attrs, { ...l.attrs, 'data-bind-list': l.field }, `rerender ${l.sel}`);
    }
    await assertSafe(page, rec);
    await context.close();
  });
});

test('入力欄の送信済み一覧の親は、送信の後も同じ要素で、属性が view() の列と一致する', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    const l = LEARNER_LIST;
    await pin(page, [l.sel]);
    await page.fill(DRAFT, 'hello');
    await page.click(SUBMIT);
    assert.equal(await page.locator(`${l.sel} > li`).count(), 1, '送信が反映されていない');
    assertParent(await inspect(page, l.sel), l, 'after submit');
    await assertSafe(page, rec);
    await context.close();
  });
});

test('配信設定でも、成果物のビューごとの親を含む親 ul が 1 つずつあり、行は 0 件である', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    await page.goto(srv.url);
    await waitMounted(page);
    for (const l of ENGINE_LISTS) {
      assert.equal(await page.locator(`ul${l.sel}`).count(), 1, l.sel);
      assert.equal(await page.locator(`${l.sel} li`).count(), 0, l.sel);
    }
    await context.close();
  });
});
