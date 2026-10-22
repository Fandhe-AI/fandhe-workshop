// `?ir=` の値で読み込み先が変わらず、開始点 `start` だけが `starts` の ID と完全一致で許可されることの試験
// （TASK-168。SAFE-33・SAFE-7・DLV-9・D-68）。
//
// 許可の方式（D-68）。読み込む IR の場所は固定の `./ir/main.json` で、URL の値から場所を作らない。`?ir=` は読まない。
// 直接リンクは `?start=` で、値は IR の `starts` の ID と完全一致で照らし、一致しなければ開かずに固定のコードだけを示す。
//
// 確かめること。
//   (D) 配信する設定（CSP あり）。`?ir=` に `../something`・符号化した `..%2Fsomething`・別名・絶対 URL・`//host` を入れても、
//       取得する資源の集合がクエリなしと同じで、配信元の別の JSON（囮 3 件）を一度も取得せず、配信元以外への通信が 0 件。
//   (T) 試験の設定（CSP あり・なし）。`?ir=` を付けても教材と現在の Step がクエリなしと同じ（`start` が一致すれば開く）。
//       `start` が一致しなければ `?ir=` の有無にかかわらず断り、値は DOM・タイトル・コンソールに出ず、保存先は変わらない。
// 限界。配信する設定は実行時に IR を取得しないので、(D) では `ir/main.json` の取得は期待しない。「一覧にある値は開く」は
// 試験の設定でだけ確かめる（docs/known-gaps.md）。`?fixture=` は `ir=` と併用しない（試験の設定の教材の選び方が
// `start=` の対だけを除いた残りを照らすため、併用すると既定の教材に倒れ、`ir` が教材を変えたように見えるから）。
// 試験の出力・失敗の文に、入力した値・鍵・履歴の中身を出さない。
//
// 入力: FANDHE_WEB_DIR（配信する設定）・FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_PACK_BIN。未設定・不在は失敗にする。
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
import { pageRequestProblems } from './delivery.mjs';

const BASE = '/fandhe-ir-location/';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const PACK_DIR = path.join(ROOT, 'testdata', 'engine-event-consistency', 'test-data', 'min-3step-pack');
const NOTICE = '[data-testid="ir-notice"]';
const SENTINEL = 'IR-SENTINEL-X';
const REFUSED_TEXT = '指定された開始点を開けませんでした。（start_unknown）';
const DECOYS = ['/something.json', '/ir/something.json', '/main.json'];

const IR_VALUES = [
  ['dot-dot', '?ir=../something'],
  ['dot-dot-encoded', '?ir=..%2Fsomething'],
  ['bare-name', '?ir=something'],
  ['main', '?ir=main'],
  ['absolute-url', `?ir=${encodeURIComponent('https://example.invalid/x')}`],
  ['scheme-relative', '?ir=//example.invalid/x'],
  ['with-start', '?ir=../something&start=s-1'],
];

let browser;

before(async () => {
  for (const v of ['FANDHE_WEB_DIR', 'FANDHE_WEB_TEST_DIR']) {
    const dir = process.env[v];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${v} must point to a built web directory`);
  }
  packBinFromEnv();
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 配信物を写し、compile した IR を ir/main.json に置き、囮の JSON を 3 か所に置く。
async function withDecoys(fn) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-irloc-'));
  try {
    const copy = path.join(tmp, 'web');
    fs.cpSync(process.env.FANDHE_WEB_DIR, copy, { recursive: true });
    const r = spawnSync(packBinFromEnv(), ['compile', PACK_DIR], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    assert.equal(r.status, 0, `compile exit ${r.status}`);
    fs.mkdirSync(path.join(copy, 'ir'), { recursive: true });
    fs.writeFileSync(path.join(copy, 'ir', 'main.json'), r.stdout);
    for (const d of DECOYS) fs.writeFileSync(path.join(copy, d), '{"decoy":"DECOY-MARK"}');
    return await fn(copy);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

function clean(rec, what) {
  const s = summarize(rec);
  const detail = JSON.stringify({ what, csp: s.csp, pageErrors: s.pageErrors, consoleErrors: s.consoleErrors });
  assert.equal(s.csp, 0, detail);
  assert.equal(s.pageErrors, 0, detail);
  assert.equal(s.consoleErrors, 0, detail);
}

test('(D) ?ir= の値で取得する資源が変わらず、配信元の別の JSON を取得しない', { timeout: 300000 }, async () => {
  await withDecoys(async (dir) => {
    await withServer({ dir, base: BASE }, async (srv) => {
      const opts = { origin: srv.origin, base: BASE };
      let baseline = null;
      let sawComponent = false;
      for (const [name, query] of [['none', ''], ...IR_VALUES]) {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        try {
          await page.goto(`${srv.url}${query}`);
          await waitMounted(page);
          await page.waitForLoadState('networkidle');
        } finally {
          await context.close();
        }
        assert.deepEqual(pageRequestProblems(rec, opts), [], name);
        const paths = rec.requests.map((r) => new URL(r.url).pathname.slice(BASE.length - 1));
        sawComponent = sawComponent || paths.length > 0;
        for (const d of DECOYS) assert.equal(paths.includes(d), false, `${name}: decoy ${d} was requested`);
        assert.equal(rec.external.length, 0, `${name}: external requests`);
        const kinds = JSON.stringify(requestKinds(rec, opts));
        if (baseline === null) baseline = kinds;
        else assert.equal(kinds, baseline, `${name}: request kinds differ from no query`);
        clean(rec, name);
      }
      assert.equal(sawComponent, true, 'requests are recorded (non-vacuous)');
    });
  });
});

const engine = async (page) => JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson())).engine;
const storageDump = (page) => page.evaluate(() => JSON.stringify(
  Object.keys(localStorage).sort().map((k) => [k, localStorage.getItem(k)]),
));

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`(T) ?ir= を付けても教材が変わらず、一致しない start は ?ir= の有無にかかわらず断る（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      const consoleTexts = [];
      page.on('console', (m) => consoleTexts.push(m.text()));
      const open = async (query) => { await page.goto(`${srv.url}${query}`); await waitMounted(page); };

      await open('');
      const base = await engine(page);
      assert.equal(base.irLoaded, true);
      assert.equal(base.current, 'main/st-1');

      // 許可される回。
      for (const query of ['?ir=../something&start=s-1', '?ir=main', '?ir=..%2Fsomething', '?start=s-1&ir=main']) {
        await open(query);
        const e = await engine(page);
        assert.equal(e.irLoaded, true, 'loaded');
        assert.equal(e.irRefusal, null, 'not refused');
        assert.equal(e.current, base.current, 'same current step');
        assert.equal(JSON.stringify(e.stateWithoutLog.run.slices), JSON.stringify(base.stateWithoutLog.run.slices), 'same slices');
        assert.equal(await page.locator(NOTICE).count(), 0, 'no notice');
      }

      // 断られる回。
      const refused = [
        ['dot-dot-start', '?ir=../something&start=..%2Fsomething'],
        ['unknown-start', '?ir=main&start=nope'],
        ['sentinel', `?ir=../something&start=${SENTINEL}`],
      ];
      for (const [name, query] of refused) {
        await open(query);
        const baseline = await storageDump(page);
        assert.equal(await page.locator(NOTICE).count(), 1, `${name}: one notice`);
        assert.equal(await page.locator(NOTICE).innerText(), REFUSED_TEXT, `${name}: fixed wording`);
        const eng = await engine(page);
        assert.equal(eng.irLoaded, false, `${name}: not loaded`);
        assert.equal(eng.irRefusal, 'start_unknown', `${name}: reason code`);
        const html = await page.evaluate(() => document.documentElement.outerHTML);
        const title = await page.title();
        assert.equal(html.includes(SENTINEL) || title.includes(SENTINEL), false, `${name}: sentinel is not shown`);
        assert.equal(html.includes('something'), false, `${name}: ir value is not shown`);
        assert.equal(consoleTexts.some((t) => t.includes(SENTINEL) || t.includes('something')), false, `${name}: value is not logged`);
        await page.reload();
        await waitMounted(page);
        assert.equal(await storageDump(page), baseline, `${name}: storage is unchanged`);
      }

      assert.equal(rec.external.length, 0, `external requests: ${rec.external.length}`);
      clean(rec, 'T');
      await context.close();
    }));
  });
}
