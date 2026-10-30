// e4-history.mjs（履歴ファイルへの注入の変種・断片・判定）の単体試験。ブラウザは使わない（TASK-146 1/2）。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { e4Payloads } from './xss-cases.mjs';
import {
  historyVariant, payloadProbes, leakedPlaces, savedPayloadOps, judgeRegularLine, KINDS, ALLOWED_REASONS,
} from './e4-history.mjs';

const BASE = fs.readFileSync(new URL('../../testdata/contract/history/draft-rejected.json', import.meta.url), 'utf8');
const cases = e4Payloads();

test('E4 は 14 件で、許す理由は SAFE-4 の 3 種', () => {
  assert.equal(cases.length, 14);
  assert.deepEqual([...ALLOWED_REASONS].sort(), ['malformed', 'other_pack', 'replay_mismatch']);
});

test('3 変種 × 14 件で、元を変えずに置き換え、解釈した値に入る', () => {
  for (const kind of KINDS) {
    for (const c of cases) {
      const before = BASE;
      const { text, replaced } = historyVariant(BASE, c.payload, kind);
      assert.equal(BASE, before);
      assert.notEqual(text, BASE, `${c.id} ${kind}`);
      assert.ok(Object.values(replaced).every((n) => n >= 1), `${c.id} ${kind}: ${JSON.stringify(replaced)}`);
      const h = JSON.parse(text);
      if (kind === 'ops') {
        assert.ok(h.ops.some((o) => o.submit === c.payload) && h.ops.some((o) => o.draft === c.payload));
      } else if (kind === 'snapshot') {
        assert.ok(JSON.stringify(h.snapshot.log).includes(JSON.stringify(c.payload).slice(1, -1)));
        assert.deepEqual(h.ops, JSON.parse(BASE).ops);
      } else {
        assert.equal(h.pack_id, c.payload);
        assert.equal(h.start, c.payload);
        assert.equal(h.versions.ir_format, 1);
        assert.equal(h.versions.refs[c.payload], c.payload);
      }
    }
  }
});

test('置き換える欄が無い履歴は例外にする（空振りの防止）', () => {
  const noSubmit = JSON.parse(BASE);
  noSubmit.ops = noSubmit.ops.filter((o) => !('submit' in o));
  assert.throws(() => historyVariant(JSON.stringify(noSubmit), 'x', 'ops'), /nothing to replace/);
  const noLog = JSON.parse(BASE);
  noLog.snapshot.log = [];
  assert.throws(() => historyVariant(JSON.stringify(noLog), 'x', 'snapshot'), /no string/);
  assert.throws(() => historyVariant(BASE, 'x', 'nope'), /unknown kind/);
});

test('断片は 14 件のすべてで空でなく、XSS-49 の制御文字だけの部分を拾う', () => {
  for (const c of cases) assert.ok(payloadProbes(c.payload).length > 0, c.id);
  const p49 = cases.find((c) => c.id === 'XSS-49').payload;
  assert.ok(payloadProbes(p49).some((x) => x.includes('紛らわしい制御文字だけ')));
  assert.throws(() => payloadProbes('\u0000  '), /no probe/);
});

test('判定は出ている場所の名前だけを返す', () => {
  const probes = payloadProbes('<img src=x onerror=__poc7_hit(1)>');
  const empty = { html: '<p>a</p>', text: 'a', textContent: 'a', attrs: ['x'], values: [''] };
  assert.deepEqual(leakedPlaces(empty, probes), []);
  assert.deepEqual(leakedPlaces({ ...empty, attrs: ['onerror=__poc7_hit(1)>'] }, probes), ['attrs']);
  assert.deepEqual(leakedPlaces({ ...empty, values: ['\u0000onerror=__poc7_hit(1)>'] }, probes), ['values']);
});

test('savedPayloadOps: 基準の履歴では 0 件、ops を置き換えた履歴では 1 件以上', () => {
  const P = cases[0].payload;
  assert.deepEqual(savedPayloadOps(BASE, P), { submit: 0, draft: 0 });
  const { text } = historyVariant(BASE, P, 'ops');
  const r = savedPayloadOps(text, P);
  assert.ok(r.submit >= 1 && r.draft >= 1);
  assert.throws(() => savedPayloadOps('{"ops":1}', P));
});

const goodLine = {
  kind: 'received', input: 'text', children: ['line-body'], childKinds: [[3]], bodyKinds: [3], text: 'abc', forbidden: 0,
};
const goodNext = { kind: 'rejected', input: null, children: ['line-body'], childKinds: [[3]], bodyKinds: [3], text: 'x', forbidden: 0 };
const goodObs = () => ({ line: { ...goodLine }, next: { ...goodNext }, choices: 0, secret: 0 });

test('judgeRegularLine: 合格の観察は空、崩れた観察は対応する理由を返す', () => {
  assert.deepEqual(judgeRegularLine(goodObs(), 'abc'), []);
  assert.deepEqual(judgeRegularLine({ ...goodObs(), line: null }, 'abc'), ['no-received-line']);
  assert.ok(judgeRegularLine(goodObs(), 'abd').includes('text-mismatch'));
  const el = goodObs();
  el.line.bodyKinds = [1];
  assert.ok(judgeRegularLine(el, 'abc').includes('body-not-single-text-node'));
  const fb = goodObs();
  fb.line.forbidden = 1;
  assert.ok(judgeRegularLine(fb, 'abc').includes('forbidden-elements'));
  assert.ok(judgeRegularLine({ ...goodObs(), next: null }, 'abc').some((s) => s.startsWith('next-missing')));
  assert.ok(judgeRegularLine({ ...goodObs(), choices: 1 }, 'abc').includes('accepted'));
});
