// tools/upstream-impact.mjs（影響する Step の洗い出し。TASK-182 1/2）の回帰テスト。
// 一時ディレクトリへ試験用の対応表を書いて実行し、作業ツリーを変えない。名前は中立（教材固有の語を使わない）。
// 変異注入（scripts/check-mutations.sh）の対象外の Node の道具のため、負の対照は試験 7・9 が担う。
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { parseMap, impactedSteps, ImpactInputError, MAX_INPUT_BYTES } from './upstream-impact.mjs';

const TOOL = path.join(path.dirname(fileURLToPath(import.meta.url)), 'upstream-impact.mjs');

const MAP = [
  '# 注記行',
  'scope,step,ref,path,lines,link',
  'c1,a,upstream-a,src/x.rs,1-9,LINK_CELL_VALUE',
  'c1,a,upstream-b,docs/y.md,3,',
  'c1,b,upstream-a,src/z.rs,,',
  'c2,c,upstream-b,,,',
  'm,a,upstream-a,src/x.rs,,',
  'm,m2,upstream-b,docs/y.md,,',
  '',
].join('\n');

const VOCAB = [
  'scope,step,kind,extension_points',
  'c1,a,skill,',
  'c1,b,skill,',
  'c2,c,skill,',
  'm,a,skill,',
  'm,m2,skill,',
  'use,m,x,',
  '# steps: 5',
  '',
].join('\n');

function setup(files) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'upstream-impact-'));
  const p = {};
  for (const [name, body] of Object.entries(files)) {
    p[name] = path.join(dir, name);
    fs.writeFileSync(p[name], body);
  }
  return { dir, p, done: () => fs.rmSync(dir, { recursive: true, force: true }) };
}

function run(args) {
  const r = spawnSync(process.execPath, [TOOL, ...args], { encoding: 'utf8' });
  return { code: r.status, out: r.stdout, err: r.stderr };
}

const steps = (out) => out.split('\n').filter((l) => l && !l.startsWith('#') && l !== 'scope,step');

test('1: 参照名の全体の変更で、同名の Step も scope で区別して完全一致で列挙する', () => {
  const t = setup({ map: MAP });
  try {
    const r = run(['--map', t.p.map, '--changed', 'upstream-a']);
    assert.equal(r.code, 0);
    assert.deepEqual(steps(r.out), ['c1,a', 'c1,b', 'm,a']);
    assert.match(r.out, /^scope,step\n/);
    assert.match(r.out, /# 影響する Step: 3\n/);
  } finally {
    t.done();
  }
});

test('2: 参照名を 2 つ変えると和集合で、重複なし・整列済み', () => {
  const t = setup({ map: MAP });
  try {
    const r = run(['--map', t.p.map, '--changed', 'upstream-b', '--changed', 'upstream-a']);
    assert.deepEqual(steps(r.out), ['c1,a', 'c1,b', 'c2,c', 'm,a', 'm,m2']);
  } finally {
    t.done();
  }
});

test('3: --changes はファイル単位で照合し、path が空の行は参照名のどのファイルでも影響する', () => {
  const t = setup({ map: MAP, changes: 'ref,path\nupstream-a,src/x.rs\nupstream-b,other/file.md\n' });
  try {
    const r = run(['--map', t.p.map, '--changes', t.p.changes]);
    assert.deepEqual(steps(r.out), ['c1,a', 'c2,c', 'm,a']);
  } finally {
    t.done();
  }
});

test('4: --changed と --changes の併用は和集合', () => {
  const t = setup({ map: MAP, changes: 'ref,path\nupstream-b,docs/y.md\n' });
  try {
    const r = run(['--map', t.p.map, '--changed', 'upstream-a', '--changes', t.p.changes]);
    assert.deepEqual(steps(r.out), ['c1,a', 'c1,b', 'c2,c', 'm,a', 'm,m2']);
  } finally {
    t.done();
  }
});

test('5: 該当なしはヘッダと 0 件の集計行だけで 0', () => {
  const t = setup({ map: MAP, changes: 'ref,path\nupstream-a,nothing.txt\n' });
  try {
    const r = run(['--map', t.p.map, '--changes', t.p.changes]);
    assert.equal(r.code, 0);
    assert.equal(r.out, 'scope,step\n# 影響する Step: 0\n# 表に無い参照名: 0\n');
  } finally {
    t.done();
  }
});

test('6: 入力の行を並べ替えても出力がバイト一致する', () => {
  const lines = MAP.split('\n');
  const shuffled = [lines[0], lines[1], ...lines.slice(2).reverse()].join('\n');
  const t = setup({ a: MAP, b: shuffled });
  try {
    const x = run(['--map', t.p.a, '--changed', 'upstream-a', '--changed', 'upstream-b']);
    const y = run(['--map', t.p.b, '--changed', 'upstream-b', '--changed', 'upstream-a']);
    assert.equal(x.out, y.out);
  } finally {
    t.done();
  }
});

test('7: --vocab は教材に無い Step・誤った scope で 1、全行が一覧にあれば 0。use 行と集計行を読める', () => {
  const t = setup({
    map: MAP,
    vocab: VOCAB,
    bad: `${MAP}c1,gone,upstream-a,,,\nzz,a,upstream-a,,,\n`,
  });
  try {
    const ok = run(['--map', t.p.map, '--changed', 'upstream-a', '--vocab', t.p.vocab]);
    assert.equal(ok.code, 0);
    assert.match(ok.out, /# 教材に無い Step: なし\n/);
    const ng = run(['--map', t.p.bad, '--changed', 'upstream-a', '--vocab', t.p.vocab]);
    assert.equal(ng.code, 1);
    assert.match(ng.out, /# 教材に無い Step: c1,gone zz,a\n/);
    assert.deepEqual(steps(ng.out), ['c1,a', 'c1,b', 'c1,gone', 'm,a', 'zz,a']);
  } finally {
    t.done();
  }
});

test('8: 表に無い参照名は 0 件・0 で、集計行に数だけが出る', () => {
  const t = setup({ map: MAP });
  try {
    const r = run(['--map', t.p.map, '--changed', 'upstream-zzz']);
    assert.equal(r.code, 0);
    assert.equal(r.out, 'scope,step\n# 影響する Step: 0\n# 表に無い参照名: 1\n');
  } finally {
    t.done();
  }
});

test('9: 不正な入力はすべて 2 で標準出力が空', () => {
  const h = 'scope,step,ref,path\n';
  const cases = {
    nocol: 'scope,step\nc1,a\n',
    count: `${h}c1,a,upstream-a\n`,
    quote: `${h}c1,a,"upstream-a",\n`,
    ctrl: `${h}c1,a,upstream-a,x\u0001y\n`,
    ls: `${h}c1,a,upstream-a,x\u2028y\n`,
    dotdot: `${h}c1,a,upstream-a,../x\n`,
    abs: `${h}c1,a,upstream-a,/x\n`,
    bslash: `${h}c1,a,upstream-a,x\\y\n`,
    emptyseg: `${h}c1,a,upstream-a,x//y\n`,
    emptyscope: `${h},a,upstream-a,\n`,
    dup: 'scope,step,ref,ref\nc1,a,x,y\n',
    empty: '',
  };
  const t = setup({
    ...cases,
    ok: MAP,
    badvocab: 'scope,step\nc1,a\n',
    badchanges: 'path,ref\nx,y\n',
    huge: 'x'.repeat(MAX_INPUT_BYTES + 1),
  });
  try {
    for (const k of Object.keys(cases)) {
      const r = run(['--map', t.p[k], '--changed', 'upstream-a']);
      assert.equal(r.code, 2, k);
      assert.equal(r.out, '', k);
    }
    const bad = [
      ['--map', t.p.ok, '--changed', 'upstream-a', '--vocab', t.p.badvocab],
      ['--map', t.p.ok, '--changes', t.p.badchanges],
      ['--map', t.p.ok, '--changed', 'upstream-a', '--nope'],
      ['--changed', 'upstream-a'],
      ['--map', t.p.ok],
      ['--map', path.join(t.dir, 'missing'), '--changed', 'upstream-a'],
      ['--map', t.dir, '--changed', 'upstream-a'],
      ['--map', t.p.huge, '--changed', 'upstream-a'],
      ['--map', t.p.ok, '--changed', 'a,b'],
    ];
    for (const a of bad) {
      const r = run(a);
      assert.equal(r.code, 2, a.join(' '));
      assert.equal(r.out, '', a.join(' '));
    }
  } finally {
    t.done();
  }
});

test('10: 自由記述の列と不正なセルの値を標準出力にも標準エラーにも写さない', () => {
  const secret = ['sk', 'live', 'ABCDEF0123456789'].join('_');
  const t = setup({
    map: MAP.replace('LINK_CELL_VALUE', secret),
    bad: `scope,step,ref,path\nc1,a,upstream-a,${secret}"\n`,
  });
  try {
    const ok = run(['--map', t.p.map, '--changed', 'upstream-a']);
    assert.ok(!ok.out.includes(secret) && !ok.err.includes(secret));
    assert.ok(!ok.out.includes('LINK_CELL_VALUE'));
    const ng = run(['--map', t.p.bad, '--changed', 'upstream-a']);
    assert.equal(ng.code, 2);
    assert.ok(!ng.out.includes(secret) && !ng.err.includes(secret));
    assert.match(ng.err, /^error: map:2: /);
  } finally {
    t.done();
  }
});

test('11: --help は 0 で usage を出す', () => {
  const r = run(['--help']);
  assert.equal(r.code, 0);
  assert.match(r.out, /^usage: /);
});

test('関数: CRLF・BOM を受け、列の順は自由で、誤りは ImpactInputError', () => {
  const rows = parseMap('﻿ref,step,scope\r\nu,s,c\r\n');
  assert.deepEqual(rows, [{ scope: 'c', step: 's', ref: 'u', path: '' }]);
  assert.deepEqual(impactedSteps(rows, { refs: new Set(['u']) }), [{ scope: 'c', step: 's' }]);
  assert.throws(() => parseMap('scope,step\n'), ImpactInputError);
});
