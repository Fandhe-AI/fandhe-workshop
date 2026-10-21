// 開始点の数を増やしても、配信物の IR が写しで増えず、同じ IR のどの開始点から開いても内容のハッシュが同じであることの試験
// （TASK-109 2/2。DLV-9・D-68。1/2 は direct-link-start.test.mjs）。
//
// 背景: 旧方式（PoC-10 の make-start-irs.mjs）は開始点ごとに `starts` の順を変えた IR の写しを `ir/start-<id>.json` として配り、
// 開始点の数だけ IR が複製され、写しごとに内容のハッシュが別になった（F-1。別の章のリンクで「変換して開きました」が出た）。
// 新方式は IR を `ir/main.json` の 1 つに保ち、URL の `start` で開始点を選ぶ。その利点が崩れないことを回帰から守る。
//
// 定義（誤読しないこと）: `content_hash`（crates/contract/src/content_hash.rs。版 1）は `starts` を含む。
// 開始点を足した教材（別の IR）のハッシュが変わるのは正しく、ここでは「同じ IR を別の開始点から開いたときに同じ」だけを確かめる。
//
// 確かめること。
//   (P1) 配信する設定（FANDHE_WEB_DIR）。開始点 1 件と 3 件の教材の配信物は、どちらも IR が `ir/main.json` の 1 つだけで、
//        3 件の IR の大きさは 1 件の 2 倍未満、`starts` を除いた IR は一致する。指定なしと各 `?start=` の取得資源は構成要素だけで、
//        要求の種類は全ページ・両配信物で同じ（開始点ごとの資源が無い）。
//   (P2) 試験の設定（FANDHE_WEB_TEST_DIR。CSP あり・なし）。開始点 3 件の 1 つの IR を `start=s-1`・`s-4`・`s-7` で開き、
//        それぞれ 1 手進めて保存させた値の `versions.content_hash` が 3 件とも同じ。`start` の欄・鍵は開始点ごとに違う。
//   (対照) 旧方式を模した配信物（`ir/start-<id>.json` を足したもの）を静的な検査が検出する。開始点ごとに別の IR を配る
//        `?fixture=chapter-a`・`?fixture=chapter-b` は `content_hash` が割れる（測り方に感度があることを示す）。
// 限界: 配信する設定は実行時に IR を取得しないため、ハッシュは試験の設定の保存先でだけ確かめる（`docs/testing.md` 5.200）。
// 配信する設定の `ir/main.json` は build-web.sh がそのまま入れる（scripts/test-build-web.sh が保証）ので、写しに置いて検査する。
// 出力: 失敗の文・診断に、IR の中身・鍵・ハッシュの値・履歴の中身を出さない（件数・真偽・記号名だけ）。
//
// 入力: FANDHE_WEB_DIR（配信する設定）・FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_PACK_BIN。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer, withCspMode, openPage, waitMounted, summarize, requestKinds,
} from './harness.mjs';
import { inspectDeliveryRefs, pageRequestProblems } from './delivery.mjs';

const BASE = '/fandhe-start-count/';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const PACK_DIR = path.join(ROOT, 'testdata', 'engine-event-consistency', 'test-data', 'min-3step-pack');
const CHAPTER_TOML = path.join('chapters', 'ch-min', 'chapter.toml');
// 最小教材の開始点は s-1（st-1）だけ。足す 2 件は固定の文字列で、入力由来の値は埋め込まない。
const START_ANCHOR = 'node = "st-1"\n\n# Step 1';
const START_EXTRA = 'node = "st-1"\n\n[[starts]]\nid = "s-2"\nnode = "st-2"\n\n[[starts]]\nid = "s-3"\nnode = "st-3"\n\n# Step 1';
const START_IDS = ['s-1', 's-2', 's-3'];

const PACK = 'start-links';
const POINTS = [
  { id: 's-1', node: 'main/st-1' },
  { id: 's-4', node: 'main/st-4' },
  { id: 's-7', node: 'main/st-7' },
];
const HISTORY_PREFIX = 'fandhe-workshop/history/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const RAW = '[data-testid="raw-trigger"]';
const CHOICE_A = '[data-testid="choice-btn"][data-choice="opt-a"]';
const HEX64 = /^[0-9a-f]{64}$/;
const T = 30000;

let browser;

before(async () => {
  for (const name of ['FANDHE_WEB_DIR', 'FANDHE_WEB_TEST_DIR']) {
    const dir = process.env[name];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a built web directory`);
  }
  packBinFromEnv();
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// ---- 純関数 ----

// 配信物の中の IR の一覧と合計バイト。`ir/` の配下と、どこにあっても `*.json` を数える（旧方式の写しの置き場が変わっても拾う）。
function irFiles(dir) {
  const found = [];
  const walk = (rel) => {
    for (const e of fs.readdirSync(path.join(dir, rel), { withFileTypes: true })) {
      const r = rel === '' ? e.name : `${rel}/${e.name}`;
      if (e.isDirectory()) walk(r);
      else if (r.startsWith('ir/') || r.endsWith('.json')) found.push({ path: r, bytes: fs.statSync(path.join(dir, r)).size });
    }
  };
  walk('');
  found.sort((a, b) => (a.path < b.path ? -1 : 1));
  return { files: found.map((f) => f.path), total: found.reduce((n, f) => n + f.bytes, 0) };
}

const withoutStarts = (ir) => {
  const { starts, ...rest } = ir;
  return { starts, rest: JSON.stringify(rest) };
};

// ---- 配信物の用意 ----

function compile(dir) {
  const r = spawnSync(packBinFromEnv(), ['compile', dir], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  assert.equal(r.status, 0, `compile exit ${r.status}: ${String(r.stderr).slice(0, 300)}`);
  return r.stdout;
}

// 開始点 1 件・3 件の IR を compile する（一時ディレクトリは必ず消す）。
function compileBoth() {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-starts-'));
  try {
    const three = path.join(tmp, 'pack-three');
    fs.cpSync(PACK_DIR, three, { recursive: true });
    const toml = path.join(three, CHAPTER_TOML);
    const src = fs.readFileSync(toml, 'utf8');
    assert.ok(src.includes(START_ANCHOR), 'the pack has the single start anchor');
    fs.writeFileSync(toml, src.replace(START_ANCHOR, START_EXTRA));
    return { one: compile(PACK_DIR), three: compile(three) };
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

// 配信物を一時ディレクトリへ写し、IR を ir/main.json に置いて fn に渡し、必ず消す。
async function withDelivery(irText, fn, mutate = () => {}) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-delivery-'));
  try {
    const copy = path.join(tmp, 'web');
    fs.cpSync(process.env.FANDHE_WEB_DIR, copy, { recursive: true });
    fs.mkdirSync(path.join(copy, 'ir'));
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), irText);
    mutate(copy);
    return await fn(copy);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

async function openUrl(srv, url) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    const res = await page.goto(url);
    await waitMounted(page);
    await page.waitForLoadState('networkidle');
    return { rec, finalUrl: page.url(), entryStatus: res ? res.status() : 0 };
  } finally {
    await context.close();
  }
}

// ---- P1: 配信する設定 ----

test('開始点を 1 件から 3 件に増やしても、配信物の IR は ir/main.json の 1 つのままで写しが増えない（P1）', { timeout: 300000 }, async () => {
  const { one, three } = compileBoth();
  const a = JSON.parse(one);
  const b = JSON.parse(three);
  // 空振り防止: 開始点の数が実際に 1 と 3 である。
  assert.equal(a.starts.length, 1, 'single-start IR has 1 start');
  assert.equal(b.starts.length, START_IDS.length, 'multi-start IR has 3 starts');
  assert.deepEqual(b.starts.map((s) => s.id), START_IDS);
  // 増えたのは開始点の項目だけ。
  const wa = withoutStarts(a);
  const wb = withoutStarts(b);
  assert.equal(wa.rest, wb.rest, 'IRs match except for starts');

  const results = [];
  for (const [label, irText] of [['one', one], ['three', three]]) {
    await withDelivery(irText, async (dir) => {
      assert.deepEqual(inspectDeliveryRefs(dir), [], `${label}: delivery refs`);
      const ir = irFiles(dir);
      assert.deepEqual(ir.files, ['ir/main.json'], `${label}: the only IR file is ir/main.json`);
      await withServer({ dir, base: BASE }, async (srv) => {
        const ids = label === 'one' ? ['s-1'] : START_IDS;
        const urls = [srv.url, ...ids.map((id) => `${srv.url}?start=${id}`)];
        const kinds = [];
        for (const url of urls) {
          const r = await openUrl(srv, url);
          const problems = pageRequestProblems(r.rec, { origin: srv.origin, base: BASE });
          if (r.entryStatus !== 200) problems.push(`entry status ${r.entryStatus}`);
          if (r.finalUrl !== url) problems.push('redirected or rewritten');
          assert.deepEqual(problems, [], `${label}: ${urls.indexOf(url)}`);
          kinds.push(JSON.stringify(requestKinds(r.rec, { origin: srv.origin, base: BASE })));
        }
        assert.equal(new Set(kinds).size, 1, `${label}: request kinds are the same on every page`);
        results.push({ label, total: ir.total, kinds: kinds[0] });
      });
    });
  }
  const [r1, r3] = results;
  assert.equal(r1.kinds, r3.kinds, 'request kinds are the same for 1 and 3 starts');
  assert.ok(r3.total < r1.total * 2, `IR total grows by less than 2x: ${r1.total} -> ${r3.total}`);
  assert.ok(r3.total > r1.total, 'IR total grows by the start items only (non-vacuous)');
});

// 負の対照: 旧方式の写し（開始点ごとに starts の順を変えた ir/start-<id>.json）を足した配信物を、静的な検査が検出する。
test('負の対照: 旧方式の写し ir/start-<id>.json を足した配信物は、構成要素の外のファイルと IR の増加として検出される（P1）', async () => {
  const { three } = compileBoth();
  const ir = JSON.parse(three);
  const copies = START_IDS.map((id) => {
    const first = ir.starts.find((s) => s.id === id);
    return [id, JSON.stringify({ ...ir, starts: [first, ...ir.starts.filter((s) => s.id !== id)] })];
  });
  await withDelivery(three, (dir) => {
    const clean = irFiles(dir);
    const mainOnly = inspectDeliveryRefs(dir);
    assert.deepEqual(mainOnly, [], 'baseline is clean (non-vacuous)');
    for (const [id, text] of copies) fs.writeFileSync(path.join(dir, 'ir', `start-${id}.json`), text);
    const after = irFiles(dir);
    assert.equal(after.files.length, clean.files.length + START_IDS.length, 'IR files grow by one per start');
    assert.ok(after.total >= clean.total * 2, 'IR total at least doubles');
    const problems = inspectDeliveryRefs(dir).filter((p) => p.startsWith('file is not a delivery component'));
    assert.equal(problems.length, START_IDS.length, 'each copy is reported as outside the delivery components');
  });
});

// ---- P2: 試験の設定 ----

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const signature = (eng) => `${eng.current}:${eng.stateWithoutLog.run.slices.main?.status?.[eng.current] ?? ''}`;

async function go(page, srv, query) {
  await page.goto(`${srv.url}${query}`);
  await waitMounted(page);
}

// 開始点ごとに受け付けられる 1 手。s-1 はコマンド、s-4 は選択肢、s-7 はテスト専用の口へ画面操作の入力を渡す（reload-points と同じ手）。
async function oneMove(page, id) {
  const before = signature(await engine(page));
  if (id === 's-1') {
    await page.fill(INPUT, '/make-note note-1');
    await page.click(SUBMIT);
  } else if (id === 's-4') {
    await page.click(CHOICE_A);
  } else {
    await page.evaluate(([sel, json]) => {
      const el = document.querySelector(sel);
      el.setAttribute('data-payload', json);
      el.click();
    }, [RAW, JSON.stringify({ input: { ui: { view: 'v-files', path: 'notes/n7' } } })]);
  }
  await page.waitForFunction((sig) => {
    const e = JSON.parse(window.__fandheTest.stateJson()).engine;
    const cur = `${e.current}:${e.stateWithoutLog.run.slices.main?.status?.[e.current] ?? ''}`;
    return cur !== sig && window.__fandheTest.nextTick() === 'null';
  }, before, { timeout: T });
}

const historyKeys = (page) => page.evaluate((prefix) => {
  const keys = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    if (k.startsWith(prefix)) keys.push(k);
  }
  return keys.sort();
}, HISTORY_PREFIX);

// 保存先の履歴が 1 本だけ書かれるのを待ち、その包みを返す。
async function savedEnvelope(page) {
  await page.waitForFunction((prefix) => Object.keys(localStorage).some((k) => k.startsWith(prefix)), HISTORY_PREFIX, { timeout: T });
  const keys = await historyKeys(page);
  assert.equal(keys.length, 1, 'exactly one history key is written');
  const raw = await page.evaluate((k) => localStorage.getItem(k), keys[0]);
  return { key: keys[0], env: JSON.parse(raw) };
}

function clean(rec) {
  const s = summarize(rec);
  const detail = JSON.stringify({ csp: s.csp, pageErrors: s.pageErrors, consoleErrors: s.consoleErrors });
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`同じ IR のどの開始点から開いても、保存した履歴の内容のハッシュが同じで、開始点・鍵は違う（CSP ${label}）（P2）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const saved = [];
      for (const { id, node } of POINTS) {
        // 開始点ごとに新しい文脈（保存領域が空）で開き、保存先に前の開始点の履歴が混ざらないようにする。
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        await go(page, srv, `?fixture=multi-start&start=${id}`);
        const eng = await engine(page);
        assert.equal(eng.irLoaded, true, `${id}: loaded`);
        assert.equal(eng.current, node, `${id}: opens its node (non-vacuous)`);
        const key = await page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, id]);
        await oneMove(page, id);
        const got = await savedEnvelope(page);
        assert.equal(got.key, key, `${id}: written under its own key`);
        saved.push({ id, key: got.key, start: got.env.start, hash: got.env.versions?.content_hash, current: (await engine(page)).current });
        clean(rec);
        await context.close();
      }
      assert.equal(new Set(saved.map((s) => s.key)).size, POINTS.length, 'keys differ per start');
      assert.deepEqual(saved.map((s) => s.start), POINTS.map((p) => p.id), 'the start field is each start id');
      for (const s of saved) assert.match(String(s.hash), HEX64, `${s.id}: content_hash is 64 lower-case hex`);
      assert.equal(new Set(saved.map((s) => s.hash)).size, 1, 'content_hash is the same for every start');
    }));
  });
}

// 対照: 開始点ごとに別の IR を配る（旧方式の再現）と、内容のハッシュが割れる。測り方に感度があることを示す。
test('対照: 開始点ごとに別の IR を配ると内容のハッシュが割れる（P2）', { timeout: 300000 }, async () => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const hashes = [];
    for (const [fixture, id, move] of [['chapter-a', 's-1', 's-1'], ['chapter-b', 's-4', 's-4']]) {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await go(page, srv, `?fixture=${fixture}`);
      const eng = await engine(page);
      assert.equal(eng.irLoaded, true, `${fixture}: loaded`);
      await oneMove(page, move);
      const got = await savedEnvelope(page);
      assert.equal(got.env.start, id, `${fixture}: start field`);
      assert.match(String(got.env.versions?.content_hash), HEX64, `${fixture}: content_hash form`);
      hashes.push(got.env.versions.content_hash);
      clean(rec);
      await context.close();
    }
    assert.notEqual(hashes[0], hashes[1], 'per-start IRs have different content_hash');
  });
});
