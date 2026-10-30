// 章の開始点を指す直接リンク 10 件と、再読み込みの後の状態の試験（TASK-110 1/2。DLV-7・D-68）。
//
// 新しいブラウザの文脈（保存済みの履歴なし）で `?start=<開始点 ID>` を開くと、その章の開始点の Step と状態で開き、
// そのまま再読み込みしても、1 操作して「受付済み（確認待ち）」にした後に再読み込みしても、同じ章・同じ状態になる。
// 10 件の開始点を持つ教材は試験の設定の `?fixture=ch0-3-links`（`src/test_fixture.rs` の `ch03_links_ir_bytes`）。
// リンクの表は `delivery.mjs` の `DIRECT_LINKS` を使い、再宣言しない。期待の Step の題・章は、実行時に試験データの IR
// （`crates/web/tests/data/ch0_3/ir.json`）から引く。
//
// 限界（`docs/testing.md` 5.204）。
//   - 配信する設定は実行時に IR を取得しないため、試験の設定（TEST_HOOKS=1）の画面で確かめる。
//   - Step の題は DOM に出ていない（TASK-525）ため、現在の節点（engine.current）と IR の題の対応で判定する。
//   - 開始点の effects は適用しない。1 操作の後の再読み込みはリンク 1 だけ（PoC-10 の固定の手順と同じ）。
//   - 章 A の履歴が保存済みのときの章 B のリンク（DLV-8）は direct-link-roundtrip.test.mjs（TASK-110 2/2。5.206）が確かめる。
//
// 試験の出力・失敗の文に、鍵の文字列・履歴の中身を出さない（リンクの番号・ID・真偽・件数だけ）。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import { DIRECT_LINKS, directLinkUrl } from './delivery.mjs';
import { engine, at, choiceSel, T } from './ch0-3-journey.mjs';

const BASE = '/fandhe-direct-link-reload/';
const PACK = 'ch0-3-links';
const FIXTURE = '&fixture=ch0-3-links';
const HISTORY_PREFIX = 'fandhe-workshop/history/';
const NOTICE = '[data-testid="shell-notice-item"][data-notice-kind="pack-unloadable"]';
const STATUS = '[data-testid="step-status"]';
const ACCEPTED_TEXT = '受付済み（確認待ち）';
const ANCHOR_TITLE = '画面の右側を選ぶ';

const IR_PATH = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../crates/web/tests/data/ch0_3/ir.json');

let browser;
let expected;

// 10 件それぞれの期待（節点・章の単位・題）。IR の読み取りの空振りは件数と錨の題で検出する。
function loadExpected() {
  const ir = JSON.parse(fs.readFileSync(IR_PATH, 'utf8'));
  const byUid = new Map(ir.nodes.map((n) => [n.uid, n]));
  return DIRECT_LINKS.map((link, i) => {
    const uid = `main/st-${link.start.slice(2)}`;
    const node = byUid.get(uid);
    assert.ok(node, `link ${i + 1}: the start node exists in the fixture IR`);
    return { index: i + 1, ...link, uid, unit: node.unit, title: node.step.title };
  });
}

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1');
  expected = loadExpected();
  assert.equal(expected.length, 10, 'ten direct links');
  assert.equal(expected[0].title, ANCHOR_TITLE, 'anchor title of link 1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const historyKeys = (page) => page.evaluate((prefix) => {
  const keys = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    if (k.startsWith(prefix)) keys.push(k);
  }
  return keys.sort();
}, HISTORY_PREFIX);
const slicesOf = (e) => JSON.stringify(e.stateWithoutLog.run.slices);
const statusOf = (e) => e.stateWithoutLog.run.slices.main?.status?.[e.current] ?? 'entered';

const linkUrl = (srv, link) => `${directLinkUrl(srv.url, link, 'query')}${FIXTURE}`;

async function open(page, url) {
  await page.goto(url);
  await waitMounted(page);
}

// 入った直後（操作なし）の状態の判定。章・題・状態を 1 件分まとめて確かめる。
async function assertEntered(page, want, where, bootKind) {
  const e = await engine(page);
  const tag = `link ${want.index} (${where})`;
  assert.equal(e.irLoaded, true, `${tag}: loaded`);
  assert.equal(e.irRefusal, null, `${tag}: not refused`);
  assert.equal(e.current, want.uid, `${tag}: opens its start node`);
  assert.equal(want.unit, `ch-${want.chapter.slice(2)}`, `${tag}: node belongs to the chapter of the link`);
  assert.equal(e.bootKind, bootKind, `${tag}: boot kind`);
  assert.equal(statusOf(e), 'entered', `${tag}: entered state`);
  assert.equal(e.stepStatus, null, `${tag}: no derived step status`);
  assert.equal(await page.locator(NOTICE).count(), 0, `${tag}: no notice`);
  assert.equal((await page.locator(STATUS).innerText()).trim(), '', `${tag}: status word is empty`);
  assert.equal(await page.evaluate(() => window.__fandheTest.nextTick()), 'null', `${tag}: not processing`);
  return e;
}

function clean(rec, tag) {
  const s = summarize(rec);
  const detail = JSON.stringify({ csp: s.csp, pageErrors: s.pageErrors, consoleErrors: s.consoleErrors });
  assert.equal(s.csp, 0, `${tag}: ${detail}`);
  assert.equal(s.pageErrors, 0, `${tag}: ${detail}`);
  assert.equal(s.consoleErrors, 0, `${tag}: ${detail}`);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`直接リンク 10 件は新しい文脈でその章の開始点で開き、再読み込みしても同じ（CSP ${label}）`, { timeout: 600000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const origin = new URL(srv.url).origin;
      for (const want of expected) {
        const { page, context, rec } = await openPage(browser, { origin: srv.origin });
        const requested = [];
        page.on('request', (r) => requested.push(r.url()));
        const tag = `link ${want.index}`;

        await open(page, linkUrl(srv, want));
        assert.deepEqual(await historyKeys(page), [], `${tag}: new context has no saved history`);
        const first = await assertEntered(page, want, 'open', 'fresh');

        await page.reload();
        await waitMounted(page);
        const again = await assertEntered(page, want, 'reload', 'fresh');
        assert.equal(slicesOf(again), slicesOf(first), `${tag}: same state after reload`);
        assert.deepEqual(await historyKeys(page), [], `${tag}: no operation writes nothing`);

        const foreign = requested.filter((u) => !u.startsWith(origin) && !u.startsWith('data:') && !u.startsWith('blob:'));
        assert.equal(foreign.length, 0, `${tag}: requests to other origins: ${foreign.length}`);
        clean(rec, tag);
        await context.close();
      }
    }));
  });

  test(`1 操作して受付済みにした後に再読み込みしても、同じ章・同じ状態で開く（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const want = expected[0];
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await open(page, linkUrl(srv, want));
      await assertEntered(page, want, 'open', 'fresh');

      await page.click(choiceSel('opt-right'));
      await at(page, want.uid, 'accepted');
      await page.waitForFunction((t) => document.querySelector('[data-testid="step-status"]')?.textContent.trim() === t, ACCEPTED_TEXT, { timeout: T });
      const key = await page.evaluate(([p, s]) => window.__fandheTest.storageKey(p, s), [PACK, want.start]);
      await page.waitForFunction((k) => localStorage.getItem(k) !== null, key, { timeout: T });
      const before = await engine(page);
      assert.equal(statusOf(before), 'accepted', 'accepted before reload');

      await page.reload();
      await waitMounted(page);
      const after = await engine(page);
      assert.equal(after.bootKind, 'restored', 'history is restored');
      assert.equal(after.current, want.uid, 'same start node after reload');
      assert.equal(statusOf(after), 'accepted', 'same accepted state after reload');
      assert.equal(slicesOf(after), slicesOf(before), 'same step statuses after reload');
      assert.equal((await page.locator(STATUS).innerText()).trim(), ACCEPTED_TEXT, 'status word after reload');
      assert.equal(await page.locator(NOTICE).count(), 0, 'no notice');
      assert.equal(await page.evaluate(() => window.__fandheTest.nextTick()), 'null', 'not processing');
      clean(rec, 'link 1 operated');
      await context.close();
    }));
  });

  test(`判定に感度がある: 開始点は 10 件で別々の節点・start を無視する誤りは 9 件を落とし、1 件の教材は断る（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      assert.equal(new Set(expected.map((w) => w.uid)).size, 10, 'ten distinct start nodes');
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      await open(page, `${srv.url}?fixture=ch0-3-links`);
      const noStart = (await engine(page)).current;
      assert.equal(noStart, expected[0].uid, 'no start opens the first start');
      assert.equal(expected.filter((w) => w.uid !== noStart).length, 9, 'ignoring start would fail 9 links');

      // 開始点が 1 件の既存の教材に start=s-2-1 を付けると断られる（10 件の教材が要ることの対照）。
      await open(page, `${srv.url}?start=s-2-1&fixture=ch0-3`);
      const refused = await engine(page);
      assert.equal(refused.irLoaded, false, 'single-start fixture does not open s-2-1');
      assert.equal(refused.irRefusal, 'start_unknown', 'refused with the fixed code');
      await context.close();
      assert.equal(summarize(rec).pageErrors, 0, 'no page errors');
    }));
  });
}
