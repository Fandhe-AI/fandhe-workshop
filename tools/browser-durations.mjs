#!/usr/bin/env node
// node:test の junit reporter の出力から、試験ファイルごとの所要時間を多い順に出す（#2037）。
//
// 呼び出し元: .github/workflows/ci.yml の browser-test ジョブの step（FANDHE_TEST_JUNIT_OUT で書いた XML を渡す）。
// 目的: 約 40 分に伸びた browser-test の内訳を、推定でなく run のログから読めるようにする。
//
// 使い方: node tools/browser-durations.mjs <junit.xml> [--top N]（N の既定は 10）
// 終了コード: 0 = 出力 / 1 = 入力の欠落・不正（fail-closed） / 2 = 誤用。
//
// 数え方: <testsuites> の直下（深さ 1）の testcase・testsuite だけを最上位の試験として足す。
// サブテストの時間は親の time に含まれるため、子を足すと二重になる。
// <testsuite> は file= を持たないので、最初の子孫の testcase の file= を使う。
// 出力はリポジトリ内の相対パス・秒・件数だけで、試験の題名や hostname は出さない（A09）。
// 合計は並列に走った試験の時間の和で、実時間ではない（実時間は末尾の node wall）。
import { readFileSync } from 'node:fs';
import path from 'node:path';

function usage() {
  process.stderr.write('usage: node tools/browser-durations.mjs <junit.xml> [--top N]\n');
  process.exit(2);
}

function fail(msg) {
  process.stderr.write(`error: ${msg}\n`);
  process.exit(1);
}

const args = process.argv.slice(2);
let file = null;
let top = 10;
for (let i = 0; i < args.length; i += 1) {
  if (args[i] === '--top') {
    const v = args[i + 1];
    if (v === undefined || !/^[1-9][0-9]*$/.test(v)) usage();
    top = Number(v);
    i += 1;
  } else if (args[i].startsWith('-') || file !== null) {
    usage();
  } else {
    file = args[i];
  }
}
if (file === null) usage();

let xml;
try {
  xml = readFileSync(file, 'utf8');
} catch {
  fail(`cannot read ${file}`);
}
if (xml.trim() === '') fail(`empty file: ${file}`);

function attr(attrs, name) {
  const m = new RegExp(`(?:^|\\s)${name}="([^"]*)"`).exec(attrs);
  return m === null ? null : m[1];
}

function toRel(f) {
  const marker = `${path.sep}tools${path.sep}browser${path.sep}`;
  const i = f.lastIndexOf(marker);
  return i >= 0 ? f.slice(i + 1) : path.basename(f);
}

// 開始・終了・自己終了のタグを順に読み、深さで最上位を見分ける。属性値の中の ">" は引用符の内として読む。
const tagRe = /<(\/?)([A-Za-z]+)((?:[^>"]|"[^"]*")*?)(\/?)>/g;
const stack = [];
const tops = [];
const dur = /<!--\s*duration_ms\s+([0-9.]+)\s*-->/.exec(xml);
const nodeMs = dur === null ? null : Number(dur[1]);

for (let m = tagRe.exec(xml); m !== null; m = tagRe.exec(xml)) {
  const [, closing, name, attrs, selfClose] = m;
  if (closing === '/') {
    stack.pop();
    continue;
  }
  const isTest = name === 'testcase' || name === 'testsuite';
  let topEntry = null;
  if (isTest) {
    const f = attr(attrs, 'file');
    if (stack.length === 1) {
      const t = attr(attrs, 'time');
      if (t === null || !Number.isFinite(Number(t)) || Number(t) < 0) fail('a top-level test has no valid time');
      topEntry = { time: Number(t), file: f };
      tops.push(topEntry);
    } else if (stack.length > 1 && f !== null) {
      const root = stack[1];
      if (root.top !== null && root.top.file === null) root.top.file = f;
    }
  }
  if (selfClose !== '/') stack.push({ top: topEntry });
}

if (tops.length === 0) fail('no top-level tests found');

const byFile = new Map();
for (const t of tops) {
  if (t.file === null) fail('a top-level test has no file');
  const rel = toRel(t.file);
  const cur = byFile.get(rel) ?? { sec: 0, n: 0 };
  cur.sec += t.time;
  cur.n += 1;
  byFile.set(rel, cur);
}

const rows = [...byFile.entries()].sort((a, b) => b[1].sec - a[1].sec || a[0].localeCompare(b[0]));
let total = 0;
for (const [, v] of rows) total += v.sec;

process.stdout.write(
  `browser-test durations per file (top ${Math.min(top, rows.length)} of ${rows.length}; sum of parallel test times, not wall time)\n`,
);
process.stdout.write('rank\tseconds\ttests\tfile\n');
rows.slice(0, top).forEach(([f, v], i) => {
  process.stdout.write(`${i + 1}\t${v.sec.toFixed(1)}\t${v.n}\t${f}\n`);
});
process.stdout.write(`total\t${total.toFixed(1)}\t${tops.length}\t(${rows.length} files)\n`);
if (nodeMs !== null) process.stdout.write(`node wall\t${(nodeMs / 1000).toFixed(1)}\n`);
