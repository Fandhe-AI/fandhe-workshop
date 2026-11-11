// 教材の通しをキーボードだけで流す部品の試験（TASK-92 2/2。BUILD-17。記録は docs/testing.md 5.508）。
//
// 入力: FANDHE_WEB_AUTHOR_DIR（試験用教材 ch0_3_pack を compile した IR 同梱の配信物）・FANDHE_AUTHOR_IR・FANDHE_PLAYTHROUGH_JSON
// （scripts/test-browser.sh が playthrough_emit.rs で書き出す）・FANDHE_PACK_BIN。未設定・不在は skip にせず失敗にする。
// 確かめること:
//   U1 結果の分類の純関数  U2 期待の語・ID の形の拒否・asserts の未判定の数え方
//   P0 同じ教材で fandhe-pack test が全件合格（期待そのものがエンジンの判定と一致している前提）
//   P1 配信する設定の画面（CSP あり・なし）で、キーボードだけで 4 区間を最後まで流し、期待がすべて一致し、マウス 0・禁止のキー 0・監視 0
//   N1 期待を 1 件書き換えると、区間番号・操作番号つきで不一致が出る
//   N2 区間 2 の開始点を書き換えると、「次の章へ進む」の data-payload の不一致で止まる
//   N3 制限なしのドライバ（実クリック）で流すと、マウスの計数が 1 以上になる（計数が素通りでない）
// 出力は項目の名前・件数だけ。入力値・本文は出さない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer, withCspMode, openPage, waitMounted,
} from './harness.mjs';
import { mouseDriver } from './ch0-3-journey.mjs';
import { installInputCounter } from './keyboard-only.mjs';
import {
  classify, matches, parseExpect, runPlaythrough, addedFrom, buildIndex,
} from './keyboard-playthrough.mjs';
import { runKeyboardPlaythrough } from './keyboard-playthrough-run.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PACK = path.resolve(HERE, '../../crates/web/tests/data/ch0_3_pack');
let browser;
let bin;
let webDir;
let ir;
let pt;

before(async () => {
  bin = packBinFromEnv();
  webDir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(webDir && fs.statSync(webDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE');
  const irFile = process.env.FANDHE_AUTHOR_IR;
  assert.ok(irFile && fs.statSync(irFile, { throwIfNoEntry: false })?.isFile(), 'FANDHE_AUTHOR_IR must point to the compiled IR');
  ir = JSON.parse(fs.readFileSync(irFile, 'utf8'));
  const ptFile = process.env.FANDHE_PLAYTHROUGH_JSON;
  assert.ok(ptFile && fs.statSync(ptFile, { throwIfNoEntry: false })?.isFile() && fs.statSync(ptFile).size > 0, 'FANDHE_PLAYTHROUGH_JSON must point to a non-empty file');
  pt = JSON.parse(fs.readFileSync(ptFile, 'utf8'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const snap = (o = {}) => ({
  kinds: [], cats: [], title: 'T', word: 'entered', run: 'idle', next: [], ...o,
});

test('U1: 結果の分類', () => {
  const b = snap({ kinds: ['received'], cats: [null] });
  assert.equal(classify(b, snap({ kinds: ['received', 'rejected'], cats: [null, 'wrong_order'] })), 'rejected:wrong_order');
  assert.equal(classify(b, snap({ kinds: ['received', 'clarified'], cats: [null, null] })), 'clarified');
  assert.equal(classify(b, snap({ kinds: ['received', 'received'], cats: [null, null], word: '受付済み（確認待ち）' })), 'accepted');
  const w = snap({ word: '受付済み（確認待ち）' });
  assert.equal(classify(w, snap({ word: '達成' })), 'achieved');
  assert.equal(classify(w, snap({ title: 'U' })), 'achieved');
  // 同じ題の Step への遷移も達成（位置の変化、または確認待ちを抜けた事実で判定する）。
  assert.equal(classify(snap({ word: '受付済み（確認待ち）', pos: '1' }), snap({ pos: '2', kinds: ['x'] })), 'achieved');
  assert.equal(classify(snap({ word: '受付済み（確認待ち）' }), snap({ word: 'entered', kinds: ['x'] })), 'achieved');
  // 確認待ちのまま取り消しが受理され行が追記された場合は受付（none にしない）。
  assert.equal(classify(w, snap({ kinds: ['received'], cats: [null], word: '受付済み（確認待ち）' })), 'accepted');
  assert.equal(classify(snap(), snap()), 'none');
  assert.equal(classify(b, snap()), 'folded');
  assert.ok(matches({ kind: 'rejected', category: 'a' }, 'rejected:a'));
  assert.ok(!matches({ kind: 'accepted' }, 'rejected:a'));
  console.log('U1: 6 分類と一致の判定');
});

test('U3: 追記位置の追跡と章つきの位置', () => {
  const f = (k, g) => `${k}|${g}||`;
  const mk = (fps) => ({ kinds: fps.map((x) => x.split('|')[0]), cats: fps.map(() => null), fps });
  // 畳みなし: 件数で決める。縮んだら追跡できない。
  assert.equal(addedFrom(mk([f('a', 1)]), mk([f('a', 1), f('b', 2)])), 1);
  assert.equal(addedFrom(mk([f('a', 1), f('b', 2)]), mk([f('a', 1)])), null);
  // 畳みあり: 件数が減っても末尾の指紋で追記位置が決まる。件数が同じまま入れ替わっても新しい行を取る。
  const before = mk(['fold|3', f('a', 1), f('b', 2), f('c', 3)]);
  assert.equal(addedFrom(before, mk(['fold|4', f('a', 1), f('b', 2), f('c', 3), f('d', 4)])), 4);
  assert.equal(addedFrom(before, mk(['fold|5', f('b', 2), f('c', 3), f('d', 4), f('e', 5)])), 3);
  assert.equal(addedFrom(before, mk(['fold|9', f('x', 9)])), null);
  // 章をまたぐ位置: 同じ題・同じ M・K でも章が違えば別の位置。モジュールの節点は呼び出し元の章へ読み替える。
  const n = (uid, scope, unit, title) => ({ uid, scope, unit, step: { title } });
  const ir = {
    nodes: [n('main/a', 'main', 'c1', 'T'), n('main/b', 'main', 'c1', 'U'), n('main/c', 'main', 'c2', 'T'), n('main/d', 'main', 'c2', 'U'), n('m@x/s', 'm@x', 'm', 'V')],
    instances: [{ id: 'm@x', caller: 'main/b' }], starts: [], views: {},
  };
  const idx = buildIndex(ir);
  assert.deepEqual(idx.posOf.get('main/a'), { chapter: 'c1', m: '1', k: '2' });
  assert.deepEqual(idx.posOf.get('main/c'), { chapter: 'c2', m: '1', k: '2' });
  assert.deepEqual(idx.posOf.get('m@x/s'), { chapter: 'c1', m: '2', k: '2' });
  // 同じモジュール内の同題の Step は位置が同じになり、区別できない。
  assert.equal(idx.sameTitle('m@x/s'), 1);
  const ir2 = { ...ir, nodes: [...ir.nodes, n('m@x/t', 'm@x', 'm', 'V')] };
  assert.equal(buildIndex(ir2).sameTitle('m@x/s'), 2);
  assert.equal(buildIndex(ir2).sameTitle('main/a'), 1);
  // 位置なしの節点（導けない）と同題の節点は衝突として数える。
  const ir3 = { ...ir, nodes: [...ir.nodes, n('orphan/x', 'zz', 'u', 'T')] };
  assert.equal(buildIndex(ir3).sameTitle('main/a'), 2);
  assert.equal(buildIndex(ir3).sameTitle('orphan/x'), 3);
});

test('U2: 期待の語・ID の形・asserts の未判定', async () => {
  assert.deepEqual(parseExpect('accepted:free_text_unjudged'), { kind: 'accepted' });
  assert.deepEqual(parseExpect('rejected:wrong_order'), { kind: 'rejected', category: 'wrong_order' });
  for (const bad of ['ignored', 'rejected', 'achieved:x', 'a"b', '']) assert.throws(() => parseExpect(bad));
  // 形の合わない ID は、ブラウザを開く前に止まる（page には触れない）。
  const evil = { id: 'x', end: 'complete', segments: [{ start: 's-0-1', steps: [{ choice: 'a"]x', expect: 'accepted' }] }] };
  await assert.rejects(() => runPlaythrough({ page: {}, driver: {}, ir, playthrough: evil }), /形が合わない/);
  // asserts は件数だけ返し、判定しない（合格に数えない）。
  const r = await runPlaythrough({ page: {}, driver: {}, ir, playthrough: { ...pt, segments: [{ start: 'no-such', steps: [] }], asserts: [{}, {}] } });
  assert.equal(r.unjudgedAsserts, 2);
  assert.equal(r.complete, false);
  console.log('U2: 語 5 件の拒否・ID の形・asserts 2 件が未判定');
});

test('P0: 同じ教材で fandhe-pack test が全件合格する', () => {
  const r = spawnSync(bin, ['test', PACK], { encoding: 'utf8' });
  assert.equal(r.status, 0);
  assert.match(r.stdout, /模範解答: \d+\/\d+ 完走/);
  assert.match(r.stdout, /入らなかった実体: なし/);
});

for (const csp of [true, false]) {
  test(`P1: キーボードだけで 4 区間を最後まで流す（CSP ${csp ? 'あり' : 'なし'}）`, { timeout: 300000 }, async () => {
    const { result, items } = await runKeyboardPlaythrough({
      browser, webDir, ir, playthrough: pt, csp, requiredKeys: ['Tab', 'Enter', 'Shift+Enter', 'Escape'],
    });
    assert.deepEqual(result.mismatches, []);
    assert.equal(result.segments, 4);
    assert.equal(result.nextPresses, 3);
    assert.equal(result.matched, result.ops);
    assert.deepEqual(items.filter((i) => !i.ok).map((i) => i.name), []);
    console.log(`P1(csp=${csp}): 操作 ${result.ops} 件一致・項目 ${items.length}/${items.length}`);
  });
}

test('N1: 期待を 1 件書き換えると、区間・操作つきで不一致になる', { timeout: 300000 }, async () => {
  const bad = JSON.parse(JSON.stringify(pt));
  bad.segments[1].steps[0].expect = 'accepted';
  const { result, items } = await runKeyboardPlaythrough({ browser, webDir, ir, playthrough: bad, requiredKeys: [] });
  assert.equal(result.mismatches.length, 1);
  assert.deepEqual(result.mismatches[0], { seg: 2, op: 1, expected: 'accepted', observed: 'rejected:unknown_command' });
  assert.equal(items[0].ok, false);
});

test('N2: 区間 2 の開始点を書き換えると、次の章へ進むの payload の不一致で止まる', { timeout: 300000 }, async () => {
  const bad = JSON.parse(JSON.stringify(pt));
  bad.segments[1].start = 's-2-1';
  const { result } = await runKeyboardPlaythrough({ browser, webDir, ir, playthrough: bad, requiredKeys: [] });
  assert.equal(result.mismatches.length, 1);
  assert.equal(result.mismatches[0].observed, 'payload_mismatch');
  assert.equal(result.mismatches[0].seg, 2);
});

test('N3: 制限なしのドライバ（実クリック）で流すと、マウスの計数が 1 以上になる', { timeout: 300000 }, async () => {
  await withCspMode({ dir: webDir, csp: true }, (dir) => withServer({ dir, base: '/fandhe-kp-control/' }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 900 } });
    const counter = await installInputCounter(context);
    await page.goto(srv.url);
    await waitMounted(page);
    const seg = { ...pt, segments: [{ start: pt.segments[0].start, steps: pt.segments[0].steps.slice(0, 3) }], end: 'x' };
    await runPlaythrough({ page, driver: mouseDriver(page), ir, playthrough: seg });
    assert.ok((await counter.read(page)).mouse >= 1);
    await context.close();
  }));
});
