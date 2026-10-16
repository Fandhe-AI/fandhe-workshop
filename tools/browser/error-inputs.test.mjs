// 誤入力の読み取り器（error-inputs.mjs）の試験。ブラウザは使わない（TASK-437）。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadErrorInputs, parseErrorInputs } from './error-inputs.mjs';

const CATEGORIES = [
  'unknown_command', 'wrong_order', 'missing_arg', 'bad_arg_shape', 'busy', 'not_in_this_step', 'not_supported',
];

test('7 件を読み、7 分類をすべて含む', () => {
  const cases = loadErrorInputs();
  assert.equal(cases.length, 7);
  assert.deepEqual([...new Set(cases.map((c) => c.category))].sort(), [...CATEGORIES].sort());
  for (const c of cases) assert.equal(c.start, 's-1');
});

test('事前の入力と入力を試験の口の形へ写す', () => {
  const busy = loadErrorInputs().find((c) => c.id === 'e-busy');
  assert.deepEqual(busy.pre, [{ input: { command: '/make-note first-note' } }]);
  assert.deepEqual(busy.op, { input: { choice: 'opt-memo' } });
});

test('件数が合わなければ例外にする', () => {
  assert.throws(() => parseErrorInputs('', { total: 7 }), /expected 7/);
});

test('不正な形は例外にする', () => {
  const one = (input) => `[[error_inputs]]\nid = "a"\nstart = "s-1"\ninput = ${input}\n`;
  assert.throws(() => parseErrorInputs(one('{ cmd = "/x", expect = "accepted" }'), { total: 1 }), /rejected/);
  assert.throws(() => parseErrorInputs(one('{ expect = "rejected:busy" }'), { total: 1 }), /exactly one input form/);
  assert.throws(() => parseErrorInputs(one('{ cmd = "/x", text = "y", expect = "rejected:busy" }'), { total: 1 }), /exactly one input form/);
  assert.throws(() => parseErrorInputs(one('{ zap = "/x", expect = "rejected:busy" }'), { total: 1 }), /unknown input form/);
  assert.throws(() => parseErrorInputs('[[error_inputs]]\nid = "a"\n', { total: 1 }), /start must be a string/);
});
