// 配信物の部品を CH-0〜CH-3 で調べるブラウザ試験（TASK-547 3/3。SHELL-35）。配信する設定（試験用の口なし）の画面と配信物に、
// 試験用の口が 0 件であることを、実ブラウザで 20 Step を通しながら調べる。native の delivery_ports.rs（1/3・2/3）と対になる。
//
// 入力: FANDHE_WEB_DIR（配信する設定。IR なし）・FANDHE_WEB_AUTHOR_DIR（配信する設定。CH-0〜CH-3 の IR 同梱）・
// FANDHE_WEB_TEST_DIR（試験の設定。対照）。未設定・不在は skip にせず失敗にする。ビルドは scripts/test-browser.sh が行う。
//
// 確かめること:
//   A 配信する設定の 20 Step の通し（CSP あり・なし）。全 Step の「入った直後」と「確認待ち」の両時点で、移動ボタン・隠し要素 4 種・
//     章をまたぐ操作（境目以外）・window.__fandheTest・test-hooks の meta が 0 件。時点の数が 0 なら失敗（空振りの防止）。
//   B 試験用クエリ 7 種を全部付けた 20 Step の通し。クエリなしの通しと、時点ごとの探針・入った Step の列・最後の画面・保存先のキー・
//     取得した資源が一致する（query-inert が 1 回の操作までを持ち、全量の通しは本試験が受け持つ）。
//   C 試験の設定の対照。同じ通しで raw-trigger が全時点で 1 件あり、window.__fandheTest と meta がある（口が存在する構成では検出できる）。
//   D 配信物の観察と tools/check-delivery.sh の突き合わせ。3 つの配信物それぞれで、配信元から取得した WASM の書き出し・バイト列・
//     test-hooks.js と .d.ts の HTTP の状態・meta・raw-trigger の DOM を観察し、check-delivery.sh の項目ごとの合否と一致させる。
//   E 探針の自己確認。口の代役を DOM に足すと数が増え、取り除くと戻る。check-delivery.sh の出力の読み取りは必須項目が欠けると例外になる。
//
// 確かめないこと:
//   - 9 通りのクエリごとのマウント・1 操作（query-inert）。章の境目の件数（author-preview の B）。章を選ぶ部品（chapter-select-absent）
//   - 「本文の参照:」の行（TERM-41 は検討中。件数を記録するだけで合否に使わない）
//   - check-delivery.sh 自体の拡張（TASK-82）。本物の CH-0〜CH-3（TASK-643・TASK-642）
//
// 限界: make test-browser は wasm-opt を適用しない（SKIP_WASM_OPT=1）ため、WASM に core の名前（debug_ を含む文字列）が残り、
// check-delivery.sh の「no debug_ bytes in wasm」は FAIL になり終了コードは 1 になる。そのため終了コード 0 は断定せず、
// 項目ごとの一致を確かめる。許す失敗の行はこの 1 件だけで、他の失敗の行が 1 件でもあれば失敗にする（wasm-opt 適用済みなら終了コード 0 を確かめる）。
// 記録は docs/testing.md 5.407。出力は名前・件数・合否だけで、DOM の文字と入力値は出さない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages, requestKinds,
} from './harness.mjs';
import { ledger, mouseDriver, journey, install } from './ch0-3-journey.mjs';
import { ORDER, STEPS, domObserver03 } from './author-preview.mjs';
import { readScreen, sha256 } from './screen-snapshot.mjs';
import {
  probeParts, isPortFree, inspectWasm, statusOf, parseCheckDelivery, expectedTable, HIDDEN_NAMES, REQUIRED_ITEMS,
} from './delivery-ports.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const CHECK_DELIVERY = path.resolve(HERE, '../check-delivery.sh');
const BASE = '/fandhe-delivery-ports/';
const ALL_QUERIES = '?persist=0&store=memory&manual=1&tick=5&os=windows&osmode=windows&flood=1000';
const KNOWN_FAIL = 'no debug_ bytes in wasm';
let browser;
const dirs = {};

function requireDir(name) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a built web directory`);
  return dir;
}

before(async () => {
  dirs.web = requireDir('FANDHE_WEB_DIR');
  dirs.author = requireDir('FANDHE_WEB_AUTHOR_DIR');
  dirs.test = requireDir('FANDHE_WEB_TEST_DIR');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const storageKeys = (page) => page.evaluate(async () => {
  const dbs = typeof indexedDB.databases === 'function' ? (await indexedDB.databases()).map((d) => `idb:${d.name}`) : [];
  return [...Object.keys(localStorage), ...dbs].sort();
});

/**
 * 20 Step を通し、入った直後と確認待ちの両時点で探針を呼んで返す。testHooks が真なら試験の設定（既定の観察）、偽なら DOM だけの観察。
 * 返す値に DOM の文字は含めない。
 */
async function run(srv, query, { testHooks }) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin });
  try {
    const L = ledger();
    const scene = testHooks ? '試験の設定' : '配信';
    await page.goto(`${srv.url}${query}`);
    await waitMounted(page);
    await install(page);
    const points = [];
    const entered = [];
    const onStep = async (uid, status) => {
      if (status === 'entered') entered.push(uid);
      const probe = await probeParts(page);
      const point = { uid, status, probe };
      points.push(point);
    };
    const opts = testHooks ? { chaptered: true, onStep } : { chaptered: true, observe: domObserver03(), onStep };
    await journey(page, mouseDriver(page), scene, L, null, opts);
    const review = await page.evaluate(() => [...document.querySelectorAll('[data-testid="part-review-list"] > li')]
      .map((li) => ({ step: li.dataset.step, achieved: li.dataset.achieved })));
    const marks = await page.evaluate(() => window.__run);
    const final = await probeParts(page);
    const counts = await collectSafety(page, rec);
    return {
      L, points, entered, review, marks, final, counts, rec,
      keys: await storageKeys(page),
      kinds: requestKinds(rec, { origin: srv.origin, base: BASE }),
      finalShot: sha256(JSON.stringify(await page.evaluate(readScreen))),
    };
  } finally {
    await context.close();
  }
}

function checkCommon(r, csp) {
  assert.deepEqual(r.L.failed(), [], '台帳');
  assert.deepEqual(r.entered, ORDER, '入った Step の列');
  assert.equal(r.entered.length, STEPS.length);
  assert.ok(ORDER.every((u) => r.review.some((x) => x.step === u && x.achieved === 'true')), '復習の行で 20 Step が達成');
  assert.equal(r.marks.raw, 0, 'raw-trigger の click');
  assert.ok(r.marks.mouse > 0, '実マウスの操作');
  assert.deepEqual(nonZero(r.counts), [], '安全の計数');
  assert.equal(r.rec.external.length, 0, '配信元以外への通信');
  assert.equal(r.rec.pageErrors.length, 0, 'ページ内エラー');
  assert.equal(r.rec.crashed, false);
  if (csp) assert.equal(cspConsoleMessages(r.rec).length, 0, 'CSP 警告');
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  test(`A: 配信する設定の 20 Step の全時点で部品が 0 件（${mode}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: dirs.author, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const r = await run(srv, '', { testHooks: false });
      checkCommon(r, csp);
      assert.ok(r.points.length > 0, '時点の数が 0（空振り）');
      assert.equal(r.points.filter((p) => p.status === 'entered').length, STEPS.length, '入った直後の時点の数');
      assert.ok(r.points.filter((p) => p.status === 'accepted').length >= 1, '確認待ちの時点の数');
      const bad = r.points.filter((p) => !isPortFree(p.probe)).map((p) => `${p.uid} ${p.status}`);
      assert.deepEqual(bad, [], '部品がある時点');
      assert.ok(isPortFree(r.final), '最後の時点');
      const bodyRef = Math.max(...r.points.map((p) => p.probe.bodyRef));
      console.log(`DELIVERY-PORTS A csp=${csp ? 'on' : 'off'} points=${r.points.length} steps=${r.entered.length}/${STEPS.length} `
        + `ledger=${r.L.summary()} body-ref-max=${bodyRef} external=${r.rec.external.length}`);
    }));
  });
}

test('B: 試験用クエリ 7 種を全部付けた 20 Step の通しが、クエリなしと一致する', { timeout: 600000 }, async () => {
  await withCspMode({ dir: dirs.author, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const base = await run(srv, '', { testHooks: false });
    const got = await run(srv, ALL_QUERIES, { testHooks: false });
    checkCommon(base, true);
    checkCommon(got, true);
    assert.equal(got.points.length, base.points.length, '時点の数');
    assert.ok(got.points.length > 0);
    assert.deepEqual(got.points.map((p) => [p.uid, p.status, p.probe]), base.points.map((p) => [p.uid, p.status, p.probe]), '各時点の探針');
    assert.equal(got.finalShot, base.finalShot, '最後の画面');
    assert.deepEqual(got.keys, base.keys, '保存先のキー');
    assert.deepEqual(got.kinds, base.kinds, '取得した資源');
    console.log(`DELIVERY-PORTS B points=${got.points.length} probes-equal=${got.points.length}/${base.points.length} final-screen-equal=true keys=${got.keys.length}`);
  }));
});

test('C: 試験の設定の対照では口があり、同じ通しで検出できる', { timeout: 300000 }, async () => {
  await withCspMode({ dir: dirs.test, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const r = await run(srv, '?fixture=ch0-3-chapters', { testHooks: true });
    assert.deepEqual(r.L.failed(), [], '台帳');
    assert.ok(r.points.length > 0, '時点の数が 0（空振り）');
    const bad = r.points.filter((p) => p.probe.hidden['raw-trigger'] !== 1 || p.probe.hooks !== 'object' || p.probe.meta !== 1
      || p.probe.moves !== 0 || HIDDEN_NAMES.filter((n) => n !== 'raw-trigger').some((n) => p.probe.hidden[n] !== 0)
      || p.probe.nextChapter !== 0).map((p) => `${p.uid} ${p.status}`);
    assert.deepEqual(bad, [], '期待と違う時点');
    console.log(`DELIVERY-PORTS C points=${r.points.length} raw-trigger=1 in all`);
  }));
});

/** 1 つの配信物を配信元から観察する。マウントできない配信物（IR なし）は、マウントの判定を飛ばして index.html を開くだけにする。 */
async function observeDelivery(dir, { expectMount }) {
  return withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      if (expectMount) await waitMounted(page);
      else await page.waitForSelector('html[data-mounted]', { state: 'attached', timeout: 30000 }).catch(() => {});
      const wasm = await inspectWasm(page);
      const probe = await probeParts(page);
      const obs = {
        exports: wasm.exports,
        bytes: wasm.bytes,
        meta: probe.meta,
        rawDom: probe.hidden['raw-trigger'],
        hooks: probe.hooks,
        hooksJsStatus: await statusOf(srv, 'test-hooks.js'),
        dtsStatuses: [await statusOf(srv, 'wasm/fandhe_workshop_web.d.ts'), await statusOf(srv, 'wasm/fandhe_workshop_web_bg.wasm.d.ts')],
      };
      assert.equal(rec.external.length, 0, '配信元以外への通信');
      assert.equal(rec.pageErrors.length, 0, 'ページ内エラー');
      return obs;
    } finally {
      await context.close();
    }
  });
}

function runCheckDelivery(dir) {
  const r = spawnSync(CHECK_DELIVERY, [dir], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
  assert.notEqual(r.status, 2, 'check-delivery.sh が検査を完了できない');
  assert.ok(r.status === 0 || r.status === 1, `check-delivery.sh exit ${r.status}`);
  return { status: r.status, table: parseCheckDelivery(r.stdout) };
}

for (const [label, key, expectMount, delivery] of [
  ['配信する設定(IR なし)', 'web', false, true],
  ['配信する設定(IR 同梱)', 'author', true, true],
  ['試験の設定(対照)', 'test', true, false],
]) {
  test(`D: ${label}の観察が check-delivery.sh の項目ごとの合否と一致する`, { timeout: 180000 }, async () => {
    const obs = await observeDelivery(dirs[key], { expectMount });
    const want = expectedTable(obs);
    const cd = runCheckDelivery(dirs[key]);
    const diffs = REQUIRED_ITEMS.filter((n) => want.get(n) !== cd.table.get(n));
    assert.deepEqual(diffs, [], '観察と check-delivery.sh で合否が違う項目');
    const failed = [...cd.table].filter(([, ok]) => !ok).map(([n]) => n);
    if (delivery) {
      assert.equal(obs.hooks, 'undefined');
      assert.equal(obs.rawDom, 0, 'DOM の raw-trigger');
      assert.equal(obs.meta, 0);
      assert.equal(obs.hooksJsStatus, 404);
      assert.ok(obs.dtsStatuses.every((s) => s === 404));
      assert.ok(!obs.exports.some((n) => n.startsWith('debug_')));
      assert.ok(obs.exports.includes('mount_app'));
      const unexpected = failed.filter((n) => n !== KNOWN_FAIL);
      assert.deepEqual(unexpected, [], '許す 1 件以外の失敗の行');
      if (failed.length === 0) assert.equal(cd.status, 0);
      else assert.equal(cd.status, 1);
    } else {
      assert.equal(cd.status, 1);
      assert.equal(obs.hooks, 'object');
      assert.equal(obs.meta, 1);
      assert.equal(obs.hooksJsStatus, 200);
      assert.ok(obs.dtsStatuses.every((s) => s === 200));
      assert.equal(obs.rawDom, 1);
      assert.ok(obs.bytes['raw-trigger']);
      for (const n of ['debug_state_json', 'debug_next_tick', 'debug_draft']) assert.ok(obs.exports.includes(n), n);
    }
    console.log(`DELIVERY-PORTS D ${key} items=${REQUIRED_ITEMS.length}/${REQUIRED_ITEMS.length} exit=${cd.status} `
      + `failed=${failed.length} debug-bytes=${obs.bytes['debug_'] ? 'present' : 'absent'}`);
  });
}

test('E: 探針が口の代役の追加と除去を検出し、出力の読み取りは必須項目の欠落で例外になる', async () => {
  await withCspMode({ dir: dirs.author, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      const before0 = await probeParts(page);
      assert.ok(isPortFree(before0));
      await page.evaluate(() => {
        const add = (tag, attrs, text) => {
          const el = document.createElement(tag);
          for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
          if (text) el.textContent = text;
          el.setAttribute('data-probe-self-check', '1');
          document.body.append(el);
        };
        add('button', {}, '移動: x');
        add('button', { 'data-testid': 'raw-trigger' });
        add('button', { 'data-testid': 'tick-trigger' });
      });
      const added = await probeParts(page);
      assert.equal(added.moves, 1);
      assert.equal(added.hidden['raw-trigger'], 1);
      assert.equal(added.hidden['tick-trigger'], 1);
      assert.ok(!isPortFree(added));
      await page.evaluate(() => document.querySelectorAll('[data-probe-self-check]').forEach((e) => e.remove()));
      assert.deepEqual(await probeParts(page), before0);
    } finally {
      await context.close();
    }
  }));
  const good = REQUIRED_ITEMS.map((n) => `ok: ${n}`).join('\n');
  assert.equal(parseCheckDelivery(good).size, REQUIRED_ITEMS.length);
  assert.throws(() => parseCheckDelivery(good.split('\n').slice(1).join('\n')), /lacks required items/);
  assert.throws(() => parseCheckDelivery(''), /lacks required items/);
  const failForm = good.replace('ok: no test-hooks.js', 'FAIL: test-hooks.js found: ./test-hooks.js');
  assert.equal(parseCheckDelivery(failForm).get('no test-hooks.js'), false);
});
