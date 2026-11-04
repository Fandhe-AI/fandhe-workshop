// tools/browser-durations.mjs の回帰テスト（`make test-browser-durations` から呼ばれる。#2037）。
// 合成の junit XML を一時ディレクトリに書き、CLI を子プロセスで呼ぶ。ブラウザ試験の一覧（scripts/test-browser.sh）には入れない。
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const TOOL = path.join(path.dirname(fileURLToPath(import.meta.url)), 'browser-durations.mjs');
const dir = mkdtempSync(path.join(tmpdir(), 'browser-durations-'));
test.after(() => rmSync(dir, { recursive: true, force: true }));

let seq = 0;
function run(xml, extra = []) {
  seq += 1;
  const f = path.join(dir, `in-${seq}.xml`);
  if (xml !== null) writeFileSync(f, xml);
  return spawnSync('node', [TOOL, f, ...extra], { encoding: 'utf8' });
}

const tc = (t, file, name = 'x') => `<testcase name="${name}" time="${t}" classname="test" file="${file}"/>`;
const wrap = (body, tail = '') => `<?xml version="1.0"?>\n<testsuites>\n${body}\n${tail}</testsuites>\n`;
const A = '/r/tools/browser/a.test.mjs';
const B = '/r/tools/browser/b.test.mjs';

test('複数ファイルを合計し、多い順に並べ、相対パスで出す', () => {
  const r = run(wrap([tc(1, A), tc(2.5, B), tc(3, A)].join('\n'), '<!-- duration_ms 4000 -->\n'));
  assert.equal(r.status, 0);
  const lines = r.stdout.trim().split('\n');
  assert.match(lines[2], /^1\t4\.0\t2\ttools\/browser\/a\.test\.mjs$/);
  assert.match(lines[3], /^2\t2\.5\t1\ttools\/browser\/b\.test\.mjs$/);
  assert.match(lines[4], /^total\t6\.5\t3\t/);
  assert.match(lines[5], /^node wall\t4\.0$/);
  assert.ok(!r.stdout.includes('/r/'));
});

test('--top で切る', () => {
  const r = run(wrap([tc(1, A), tc(2, B)].join('\n')), ['--top', '1']);
  assert.equal(r.status, 0);
  assert.ok(r.stdout.includes('b.test.mjs'));
  assert.ok(!r.stdout.includes('a.test.mjs'));
});

test('サブテストは親の時間だけを数え、file= の無い testsuite は子のファイルに帰す', () => {
  const suite = `<testsuite name="p" time="5" hostname="h">\n${tc(2, A)}\n${tc(3, A)}\n</testsuite>`;
  const r = run(wrap(suite));
  assert.equal(r.status, 0);
  assert.match(r.stdout, /1\t5\.0\t1\ttools\/browser\/a\.test\.mjs/);
});

test('題名と hostname を出さない', () => {
  const r = run(wrap(tc(1, A, 'secret-title &gt; x')));
  assert.equal(r.status, 0);
  assert.ok(!r.stdout.includes('secret-title'));
});

test('入力の欠落・不正は終了コード 1', () => {
  assert.equal(run(null).status, 1);
  assert.equal(run('').status, 1);
  assert.equal(run(wrap('')).status, 1);
  assert.equal(run(wrap(tc('abc', A))).status, 1);
  assert.equal(run(wrap('<testsuite name="p" time="1">\n</testsuite>')).status, 1);
});

test('usage の誤りは終了コード 2', () => {
  assert.equal(spawnSync('node', [TOOL], { encoding: 'utf8' }).status, 2);
  assert.equal(run(wrap(tc(1, A)), ['--top', '0']).status, 2);
  assert.equal(run(wrap(tc(1, A)), ['--bogus']).status, 2);
});
