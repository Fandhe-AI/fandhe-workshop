// e3-ir.mjs（練習データの変種の組み立てと画面の判定）の単体試験。ブラウザは使わない。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  loadBaseIr, withStarts, e3Variant, attrOnlyVariant, putPaths, judgeRows, judgeReceived, textHas, PACK_ID,
} from './e3-ir.mjs';

const base = withStarts(loadBaseIr());
const P = '<img src=x onerror=alert(1)>';

test('putPaths は put のパスを初出の順に重複なしで集める', () => {
  const paths = putPaths(base);
  assert.equal(paths.length, 17);
  assert.equal(paths[0], 'cards/notice');
  assert.equal(new Set(paths).size, paths.length);
  assert.ok(paths.includes('cards/sample') && paths.includes('docs/handoff'));
});

test('e3Variant はパスの全出現を置き換え、属性の値と set_attr の値を P にし、元の IR を書き換えない', () => {
  const before = JSON.stringify(base);
  const { ir, pathMap, counts, leftover } = e3Variant(base, P);
  assert.equal(JSON.stringify(base), before);
  assert.equal(leftover, 0);
  assert.equal(counts.paths, 17);
  assert.equal(counts.setAttr, 2);
  assert.equal(counts.attrs, 18);
  assert.equal(ir.pack.id, PACK_ID);
  assert.equal(pathMap['cards/notice'], P);
  assert.equal(pathMap['cards/repo-a'], `${P}.1`);
  assert.equal(new Set(Object.values(pathMap)).size, 17);
  assert.deepEqual(putPaths(ir), Object.values(pathMap));
  // 確認の対象の参照も同じ文字列に置き換わる。
  const confirm = ir.nodes.find((n) => n.uid === 'main/st-0-1').step.confirm;
  assert.equal(confirm.targets[0].path, P);
  const sample = ir.nodes.find((n) => n.uid === 'main/st-1-5').step;
  assert.equal(sample.confirm.targets[0].path, pathMap['cards/sample']);
  assert.equal(sample.processing[1].effects[0].put.attrs.state, P);
  assert.equal(sample.processing[3].effects[0].set_attr.value, P);
  assert.equal(sample.processing[3].effects[0].set_attr.path, pathMap['cards/sample']);
});

test('attrOnlyVariant はパスを変えず、属性の値と set_attr の値だけを P にする', () => {
  const { ir, counts } = attrOnlyVariant(base, P);
  assert.deepEqual(putPaths(ir), putPaths(base));
  assert.equal(counts.attrs, 18);
  assert.equal(counts.setAttr, 2);
  assert.equal(ir.nodes.find((n) => n.uid === 'main/st-0-1').step.rules[0].outcome.effects[0].put.attrs.state, P);
});

const row = (p, over = {}) => ({
  path: p, row: p, labels: 1, text: p, kids: 0, articles: 1, shape: ['span', 'div', 'p', 'ul'], badge: '不明', badgeKids: 1,
  tags: ['article', 'div', 'li', 'p', 'span', 'ul'], ...over,
});

test('judgeRows は集合・文字・子の要素・タグの過不足を捉え、U+0000 の違いは許す', () => {
  const ok = { rows: [row('a'), row('b')], logs: [], notice: 0 };
  assert.deepEqual(judgeRows(ok, ['b', 'a']), []);
  assert.deepEqual(judgeRows(ok, ['a']), ['set']);
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { text: 'x' }), row('b')] }, ['a', 'b']), ['text']);
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { kids: 1 }), row('b')] }, ['a', 'b']), ['text']);
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { tags: ['span', 'img'] }), row('b')] }, ['a', 'b']), ['tags']);
  for (const t of ['a', 'script', 'img']) {
    assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { tags: ['article', 'li', 'span', t] }), row('b')] }, ['a', 'b']), ['tags']);
  }
  for (const labels of [0, 2]) {
    assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { labels }), row('b')] }, ['a', 'b']), ['text']);
  }
  for (const over of [{ articles: 0 }, { articles: 2 }, { shape: ['span', 'p', 'div', 'ul'] }, { shape: ['span', 'div', 'p'] }]) {
    assert.deepEqual(judgeRows({ ...ok, rows: [row('a', over), row('b')] }, ['a', 'b']), ['shape']);
  }
  // バッジは教材の語のまま出る（D-101）。期待の文字と違えば state、期待が無ければ空でない文字なら通る。
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { badge: 'a' }), row('b')] }, ['a', 'b']), []);
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { badge: 'a' }), row('b', { badge: 'a' })] }, ['a', 'b'], 'a'), []);
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { badge: 'a' }), row('b')] }, ['a', 'b'], 'a'), ['state']);
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { badge: 'a' }), row('b', { badge: 'b' })] }, ['a', 'b'], (p) => p), []);
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { badge: 'a\u0000' }), row('b', { badge: 'a' })] }, ['a', 'b'], 'a'), []);
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { badge: '' }), row('b')] }, ['a', 'b']), ['state']);
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { badgeKids: 2 }), row('b')] }, ['a', 'b']), ['state']);
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { badge: null }), row('b')] }, ['a', 'b']), ['state']);
  assert.deepEqual(judgeRows({ ...ok, rows: [row('a', { row: 'z' }), row('b')] }, ['a', 'b']), ['row']);
  assert.deepEqual(judgeRows({ ...ok, notice: 1 }, ['a', 'b']), ['notice']);
  assert.deepEqual(judgeRows({ rows: [row('a\u0000b')], logs: [], notice: 0 }, ['ab']), []);
  assert.deepEqual(judgeRows({ rows: [], logs: [], notice: 0 }, []), []);
});

test('judgeReceived は path を含む受付の行と span だけの中身を要求する', () => {
  const log = (text, tags = ['span'], kind = 'received') => ({ kind, text, tags });
  assert.deepEqual(judgeReceived({ logs: [log('v-cards zz')] }, 'zz'), []);
  assert.deepEqual(judgeReceived({ logs: [log('x'), log('v-cards zz'), log('done', [], 'achieved')] }, 'zz'), []);
  assert.deepEqual(judgeReceived({ logs: [log('v-cards yy')] }, 'zz'), ['log']);
  assert.deepEqual(judgeReceived({ logs: [log('v-cards zz', [], 'achieved')] }, 'zz'), ['log']);
  assert.deepEqual(judgeReceived({ logs: [log('v-cards zz', ['span', 'a'])] }, 'zz'), ['tags']);
  assert.deepEqual(judgeReceived({ logs: [] }, 'zz'), ['log']);
});

test('textHas は P がそのまま含まれるかを U+0000 を除いて判定する', () => {
  assert.equal(textHas(`x ${P} y`, P), true);
  assert.equal(textHas('x y', P), false);
  assert.equal(textHas('a\u0000b', 'ab'), true);
});
