// e2-ir.mjs（構造定義の欄・ID への注入の IR の作成と判定）の単体試験。ブラウザは使わない（TASK-145 1/2。SAFE-2）。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  loadBaseIr, withStarts, fieldVariant, idVariant, slotFragments, judgeFields, textHas, STARTS, ID_TARGETS, PACK_ID,
} from './e2-ir.mjs';
import { e2Payloads } from './xss-cases.mjs';

const base = loadBaseIr();
const KNOWN_SLOTS = ['input_head', 'nearest_command', 'step_title', 'step_do', 'remaining', 'total'];

test('開始点が 2 件あり、宛先の節点が解決する。元の IR は変わらない', () => {
  const before = JSON.stringify(base);
  const ir = withStarts(base);
  assert.deepEqual(ir.starts.map((s) => s.id), STARTS.map((s) => s.id));
  for (const s of ir.starts) assert.ok(ir.nodes.some((n) => n.uid === s.node), s.id);
  assert.equal(JSON.stringify(base), before);
  assert.equal(base.starts.length, 1);
});

test('欄の変種: 置き換えた欄の数が元の IR から求めた数と一致し、元の IR は変わらない', () => {
  const before = JSON.stringify(base);
  const { ir, counts } = fieldVariant(withStarts(base), 'P<b>');
  const nodes = base.nodes;
  assert.equal(counts.title, nodes.length);
  assert.equal(counts.do, nodes.filter((n) => typeof n.step.do === 'string').length);
  assert.equal(counts.optionLabels, nodes.reduce((a, n) => a + (n.step.input?.options?.length ?? 0), 0));
  assert.equal(counts.confirmLabels, nodes.reduce((a, n) => a + (n.step.confirm?.options?.length ?? 0), 0));
  assert.equal(counts.rejection, Object.keys(base.rejections).length * 2);
  assert.equal(counts.clarification, Object.keys(base.clarifications).length * 2);
  assert.equal(counts.hints, Object.keys(base.hints).length);
  assert.equal(counts.refs, 2);
  assert.ok(counts.title > 0 && counts.do > 0 && counts.optionLabels > 0 && counts.confirmLabels > 0 && counts.rejection > 0 && counts.clarification > 0 && counts.hints > 0);
  assert.equal(JSON.stringify(base), before);
  assert.equal(ir.pack.id, PACK_ID);
  assert.ok(ir.nodes.every((n) => n.step.title === 'P<b>'));
  assert.ok(Object.values(ir.rejections).every((r) => r.message === 'P<b>' && r.next_action === 'P<b>'));
  assert.ok(Object.values(ir.clarifications).every((c) => c.message === 'P<b>' && c.how_to_answer === 'P<b>'));
});

test('欄の変種の refs は文字列の name と commit を 1 件持つ（record_versions が失敗しない形）', () => {
  const { ir } = fieldVariant(base, 'x');
  assert.equal(ir.refs.length, 1);
  assert.equal(typeof ir.refs[0].name, 'string');
  assert.equal(typeof ir.refs[0].commit, 'string');
});

test('ID の変種: 種類ごとに 1 件以上を置き換え、元の ID が残らない', () => {
  const { ir, replaced, leftover } = idVariant(base, 'Q"><i>');
  for (const kind of Object.keys(ID_TARGETS)) {
    assert.ok(replaced[kind] >= 1, `${kind}: replaced ${replaced[kind]}`);
    assert.equal(leftover[kind], 0, `${kind}: leftover`);
  }
  assert.ok('Q"><i>' in ir.commands);
  assert.ok('Q"><i>' in ir.hints);
  assert.ok(ir.nodes.some((n) => n.step.input?.options?.some((o) => o.id === 'Q"><i>')));
  assert.ok(!(ID_TARGETS.command in ir.commands));
  assert.ok(!(ID_TARGETS.hint in ir.hints));
});

test('ID の変種: __proto__ の鍵でもプロトタイプを書き換えず自分の欄になる', () => {
  const { ir } = idVariant(base, '__proto__');
  assert.ok(Object.hasOwn(ir.commands, '__proto__'));
  assert.equal(Object.getPrototypeOf(ir.commands), Object.prototype);
});

test('E2 の 55 件: 差し込みの枠を含むのは XSS-53 だけで、その枠名は既知の語', () => {
  const withSlot = e2Payloads().filter((c) => c.payload.includes('{{') || c.payload.includes('}}'));
  assert.deepEqual(withSlot.map((c) => c.id), ['XSS-53']);
  const names = [...withSlot[0].payload.matchAll(/\{\{\s*([^}|]*?)\s*(?:\|[^}]*)?\}\}/g)].map((m) => m[1]);
  assert.ok(names.length > 0);
  for (const n of names) assert.ok(KNOWN_SLOTS.includes(n), n);
});

test('断片: 枠で区切った空でない断片を返す', () => {
  assert.deepEqual(slotFragments('{{input_head}}<img>{{input_head}}'), ['<img>']);
  assert.deepEqual(slotFragments('a'), ['a']);
});

const shape = (text, kids = 0) => ({ text, kids });
const obsOf = (o = {}) => ({ title: null, do: null, choices: [], line: null, position: '', notice: 0, ...o });

test('判定: 一致で合格、文字の食い違い・子の要素・NUL の差を区別する', () => {
  const P = '<b>x</b>';
  assert.deepEqual(judgeFields(obsOf({ title: shape(P) }), { title: true }, P), []);
  assert.deepEqual(judgeFields(obsOf({ title: shape('&lt;b&gt;x') }), { title: true }, P), ['title']);
  assert.deepEqual(judgeFields(obsOf({ title: shape(P, 1) }), { title: true }, P), ['title']);
  assert.deepEqual(judgeFields(obsOf({ title: null }), { title: true }, P), ['title']);
  assert.deepEqual(judgeFields(obsOf({ title: shape('a\u0000b') }), { title: true }, 'ab'), []);
});

test('判定: do は本文の span が 2 つ目で、選択肢は個数と全件を見る', () => {
  const P = 'p';
  assert.deepEqual(judgeFields(obsOf({ do: { parts: 2, body: shape(P) } }), { do: true }, P), []);
  assert.deepEqual(judgeFields(obsOf({ do: { parts: 1, body: shape(P) } }), { do: true }, P), ['do']);
  assert.deepEqual(judgeFields(obsOf({ choices: [shape(P), shape(P)] }), { choices: 2 }, P), []);
  assert.deepEqual(judgeFields(obsOf({ choices: [shape(P)] }), { choices: 2 }, P), ['choices']);
  assert.deepEqual(judgeFields(obsOf({ choices: [shape(P), shape('q')] }), { choices: 2 }, P), ['choices']);
});

test('判定: 行は種類・文面・次の行動・span だけであること。枠のある入力は断片で見る', () => {
  const P = '{{input_head}}<img>{{input_head}}';
  const line = (o) => ({ kind: 'rejected', reason: shape('/x<img>/x'), question: null, next: shape('<img>'), tags: ['span'], ...o });
  assert.deepEqual(judgeFields(obsOf({ line: line({}) }), { line: 'rejected' }, P), []);
  assert.deepEqual(judgeFields(obsOf({ line: line({ reason: shape('zzz') }) }), { line: 'rejected' }, P), ['line:rejected']);
  assert.deepEqual(judgeFields(obsOf({ line: line({ tags: ['span', 'img'] }) }), { line: 'rejected' }, P), ['line:rejected']);
  assert.deepEqual(judgeFields(obsOf({ line: line({ kind: 'clarified' }) }), { line: 'rejected' }, P), ['line:rejected']);
  const clar = { kind: 'clarified', reason: null, question: shape('q'), next: shape('q'), tags: ['span'] };
  assert.deepEqual(judgeFields(obsOf({ line: clar }), { line: 'clarified' }, 'q'), []);
  assert.deepEqual(judgeFields(obsOf({ line: null }), { line: 'clarified' }, 'q'), ['line:clarified']);
});

test('画面の文字に断片が出るかの判定（記録用）', () => {
  assert.equal(textHas('abc <img> def', '<img>'), true);
  assert.equal(textHas('abc', '<img>'), false);
  assert.equal(textHas('abc', '{{input_head}}'), false);
});
