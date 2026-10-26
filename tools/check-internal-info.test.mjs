// tools/check-internal-info.mjs（公開前の確認の照合。LIC-12。TASK-139）の回帰テスト。
//
// 作業ツリーは変えない。mkdtemp の一時 repo（git init 済み）を組み立てて検査を流す。語は明らかに架空のもの、
// トークン形の値は実行時に部品を連結して作る（ソースに形のまま書くと pre-commit の秘密情報の検知と本検査に当たるため）。
import { test, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const TOOL = fileURLToPath(new URL('./check-internal-info.mjs', import.meta.url));
const TOOL_URL = new URL('./check-internal-info.mjs', import.meta.url).href;
const REPO = fileURLToPath(new URL('..', import.meta.url));

const WORD = 'kakuu-runner-7';
const fakeToken = () => ['gh', 'p_', 'a'.repeat(36)].join('');

const dirs = [];
afterEach(() => {
  while (dirs.length) fs.rmSync(dirs.pop(), { recursive: true, force: true });
});

function tmp() {
  const d = fs.mkdtempSync(path.join(os.tmpdir(), 'chk-info-'));
  dirs.push(d);
  return d;
}

// files: {相対パス: 内容}。git init して add する。
function makeRepo(files) {
  const root = tmp();
  spawnSync('git', ['init', '-q', root]);
  for (const [rel, body] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(root, rel)), { recursive: true });
    fs.writeFileSync(path.join(root, rel), body);
  }
  spawnSync('git', ['-C', root, 'add', '-A']);
  return root;
}

function writeList(body) {
  const f = path.join(tmp(), 'terms.txt');
  fs.writeFileSync(f, body);
  return f;
}

function run(args) {
  const env = { ...process.env };
  const built = path.join(REPO, 'target/debug/examples/secret_scan_files');
  if (!env.FANDHE_SECRET_SCAN_BIN && fs.existsSync(built)) env.FANDHE_SECRET_SCAN_BIN = built;
  return spawnSync('node', [TOOL, ...args], { encoding: 'utf8', env });
}

test('語が対象のファイルにあれば 1、無ければ 0。出力に語は出ない', () => {
  const list = writeList(`# 試験用\n\n${WORD}\n`);
  const hit = makeRepo({ 'testdata/a.md': `x\nuse ${WORD.toUpperCase()} here\n` });
  const r = run(['--list', list, '--root', hit]);
  assert.equal(r.status, 1);
  assert.match(r.stdout, /^testdata\/a\.md:2: term#1$/m);
  assert.ok(!r.stdout.includes('kakuu') && !r.stderr.includes('kakuu'));
  assert.match(r.stderr, /list: terms=1 sha256=[0-9a-f]{64}/);
  const clean = makeRepo({ 'testdata/a.md': 'nothing\n' });
  assert.equal(run(['--list', list, '--root', clean]).status, 0);
});

test('対象外のパスの語は 0、--all を付けると 1', () => {
  const list = writeList(`${WORD}\n`);
  const root = makeRepo({ 'docs/other.md': `${WORD}\n` });
  assert.equal(run(['--list', list, '--root', root]).status, 0);
  assert.equal(run(['--list', list, '--root', root, '--all']).status, 1);
});

test('認証情報の形は secret:<ID> で出て 1。値は出ない', () => {
  const root = makeRepo({ 'docs/x.md': `a\ntoken ${fakeToken()}\n` });
  const r = run(['--secrets-only', '--root', root]);
  assert.equal(r.status, 1);
  assert.match(r.stdout, /^docs\/x\.md:2: secret:github_token$/m);
  assert.ok(!r.stdout.includes('ghp_'));
  assert.match(r.stdout, /words: not checked/);
});

test('認証情報の形が無ければ 0', () => {
  const root = makeRepo({ 'docs/x.md': 'plain\n' });
  assert.equal(run(['--secrets-only', '--root', root]).status, 0);
});

test('一覧が無い・0 語・UTF-8 でない・引数なしは 2', () => {
  const root = makeRepo({ 'testdata/a.md': 'x\n' });
  assert.equal(run(['--list', path.join(tmp(), 'none.txt'), '--root', root]).status, 2);
  assert.equal(run(['--list', writeList('# only comment\n\n'), '--root', root]).status, 2);
  const bad = path.join(tmp(), 'bad.txt');
  fs.writeFileSync(bad, Buffer.from([0xff, 0xfe, 0x41]));
  assert.equal(run(['--list', bad, '--root', root]).status, 2);
  assert.equal(run(['--root', root]).status, 2);
  assert.equal(run(['--bogus']).status, 2);
});

test('repo で追跡されている一覧は 2', () => {
  const root = makeRepo({ 'testdata/a.md': 'x\n', 'terms.txt': `${WORD}\n` });
  const r = run(['--list', path.join(root, 'terms.txt'), '--root', root]);
  assert.equal(r.status, 2);
  assert.match(r.stderr, /tracked/);
});

test('実 repo の認証情報の照合は固定した例外の件数と一致して 0', () => {
  assert.equal(run(['--secrets-only']).status, 0);
});

test('試験データの expect の ID は core の形の ID に含まれる（JS が形を写していない）', () => {
  const rs = fs.readFileSync(path.join(REPO, 'crates/core/src/secret_scan.rs'), 'utf8');
  const known = new Set([...rs.matchAll(/SecretShape::\w+ => "([a-z_]+)",/g)].map((m) => m[1]));
  assert.ok(known.has('github_token') && known.has('jwt'));
  const walk = (v, ids) => {
    if (Array.isArray(v)) v.forEach((x) => walk(x, ids));
    else if (v && typeof v === 'object') {
      for (const [k, x] of Object.entries(v)) {
        if (k === 'expect') [].concat(x).forEach((e) => typeof e === 'string' && ids.add(e));
        else walk(x, ids);
      }
    }
  };
  for (const rel of ['testdata/secret-shapes-added/added-shapes.json', 'testdata/untrusted-input-safety/test-data-addendum-4/secret-shapes.json']) {
    const ids = new Set();
    walk(JSON.parse(fs.readFileSync(path.join(REPO, rel), 'utf8')), ids);
    assert.ok(ids.size > 0, rel);
    for (const id of ids) assert.ok(known.has(id) || id === 'none', `${rel}: ${id}`);
  }
});

test('固定した例外の件数が減っても増えても 1', () => {
  // 例外の表は実 repo のものなので、ここでは applyExceptions を直接呼ぶ。
  const mod = import(TOOL_URL);
  return mod.then(({ applyExceptions }) => {
    const ex = new Map([['a.md\tjwt', 2]]);
    const h = (n) => Array.from({ length: n }, (_, i) => ({ file: 'a.md', line: i + 1, id: 'secret:jwt' }));
    assert.equal(applyExceptions(h(2), ex).drift.length, 0);
    assert.equal(applyExceptions(h(1), ex).drift.length, 1);
    assert.equal(applyExceptions(h(3), ex).drift.length, 1);
    assert.equal(applyExceptions(h(2), new Map()).rest.length, 2);
  });
});
