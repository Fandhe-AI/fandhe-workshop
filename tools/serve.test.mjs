// tools/serve.mjs の試験（`make test-serve` から呼ばれる）。
//
// 一時ディレクトリに固定データを作り、サブパス配信・gzip・404・パストラバーサルの拒否を確かめる。
// fetch は gzip を自動で復号してヘッダと本文を確かめられないため、node:http で直接要求する。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import zlib from 'node:zlib';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { startServer, normalizeBase } from './serve.mjs';

const SERVE = fileURLToPath(new URL('./serve.mjs', import.meta.url));
let tmp;
let root;
let srv;
const wasm = Buffer.alloc(4096, 7);

function get(origin, rawPath, { method = 'GET', headers = {} } = {}) {
  return new Promise((resolve, reject) => {
    const u = new URL(origin);
    const req = http.request({ host: u.hostname, port: u.port, path: rawPath, method, headers }, (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => resolve({ status: res.statusCode, headers: res.headers, body: Buffer.concat(chunks) }));
    });
    req.on('error', reject);
    req.end();
  });
}
const GZ = { 'accept-encoding': 'gzip' };

before(async () => {
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-serve-test-'));
  root = path.join(tmp, 'root');
  fs.mkdirSync(path.join(root, 'wasm'), { recursive: true });
  fs.mkdirSync(path.join(root, 'sub'));
  fs.writeFileSync(path.join(root, 'index.html'), '<!doctype html><title>x</title>'.repeat(20));
  fs.writeFileSync(path.join(root, 'app.js'), 'export const a = 1;\n'.repeat(50));
  fs.writeFileSync(path.join(root, 'wasm', 'x.wasm'), wasm);
  fs.writeFileSync(path.join(root, 'data.json'), '{"a":1}');
  fs.writeFileSync(path.join(root, 'icon.svg'), '<svg xmlns="http://www.w3.org/2000/svg"></svg>');
  fs.writeFileSync(path.join(root, 'sub', 'index.html'), 'sub index');
  fs.writeFileSync(path.join(tmp, 'secret.txt'), 'TOP-SECRET');
  fs.symlinkSync(path.join(tmp, 'secret.txt'), path.join(root, 'link.txt'));
  srv = await startServer({ dir: root, base: '/sub/' });
});

after(async () => {
  await srv.close();
  fs.rmSync(tmp, { recursive: true, force: true });
});

test('index.html を gzip で返し、復号すると元と一致する', async () => {
  const r = await get(srv.origin, '/sub/', { headers: GZ });
  assert.equal(r.status, 200);
  assert.equal(r.headers['content-type'], 'text/html; charset=utf-8');
  assert.equal(r.headers['content-encoding'], 'gzip');
  assert.deepEqual(zlib.gunzipSync(r.body), fs.readFileSync(path.join(root, 'index.html')));
});

test('wasm は application/wasm と gzip で返り、Content-Length は圧縮後の長さ', async () => {
  const r = await get(srv.origin, '/sub/wasm/x.wasm', { headers: GZ });
  assert.equal(r.status, 200);
  assert.equal(r.headers['content-type'], 'application/wasm');
  assert.equal(r.headers['content-encoding'], 'gzip');
  assert.equal(Number(r.headers['content-length']), r.body.length);
  assert.deepEqual(zlib.gunzipSync(r.body), wasm);
});

test('js・json・svg の Content-Type と gzip', async () => {
  for (const [p, type] of [['app.js', 'text/javascript; charset=utf-8'], ['data.json', 'application/json'], ['icon.svg', 'image/svg+xml']]) {
    const r = await get(srv.origin, `/sub/${p}`, { headers: GZ });
    assert.equal(r.status, 200, p);
    assert.equal(r.headers['content-type'], type, p);
    assert.equal(r.headers['content-encoding'], 'gzip', p);
  }
});

test('Accept-Encoding が無い・gzip;q=0 のときは圧縮せず Vary を付ける', async () => {
  for (const headers of [{}, { 'accept-encoding': 'gzip;q=0' }]) {
    const r = await get(srv.origin, '/sub/wasm/x.wasm', { headers });
    assert.equal(r.status, 200);
    assert.equal(r.headers['content-encoding'], undefined);
    assert.equal(r.headers.vary, 'Accept-Encoding');
    assert.deepEqual(r.body, wasm);
  }
});

test('gzip: false では圧縮しない', async () => {
  const s = await startServer({ dir: root, base: '/sub/', gzip: false });
  try {
    const r = await get(s.origin, '/sub/wasm/x.wasm', { headers: GZ });
    assert.equal(r.headers['content-encoding'], undefined);
    assert.deepEqual(r.body, wasm);
  } finally {
    await s.close();
  }
});

test('クエリ付きの入口は index.html を返す', async () => {
  const r = await get(srv.origin, '/sub/?ir=x.json');
  assert.equal(r.status, 200);
  assert.equal(r.headers['content-type'], 'text/html; charset=utf-8');
});

test('サブパスの外・末尾スラッシュなし・存在しないものは 404', async () => {
  for (const p of ['/', '/other/', '/sub', '/sub/nope.js', '/sub/sub', '/sub/wasm/', '/sub/app.js/']) {
    const r = await get(srv.origin, p, { headers: GZ });
    assert.equal(r.status, 404, p);
    assert.doesNotMatch(r.body.toString(), /nope|other/);
  }
});

test('サブディレクトリの index.html は末尾スラッシュ付きで返る', async () => {
  const r = await get(srv.origin, '/sub/sub/');
  assert.equal(r.status, 200);
  assert.equal(r.body.toString(), 'sub index');
});

test('パストラバーサルは root の外を返さない', async () => {
  for (const p of ['/sub/../secret.txt', '/sub/%2e%2e/secret.txt', '/sub/a%2f..%2f..%2fsecret.txt', '/sub/%2e%2e%5csecret.txt', '/sub/%00', '/sub/..%2fsecret.txt']) {
    const r = await get(srv.origin, p, { headers: GZ });
    assert.notEqual(r.status, 200, p);
    assert.ok(!r.body.includes('TOP-SECRET'), p);
  }
  assert.equal((await get(srv.origin, '/sub/%E0%A4%A')).status, 400);
});

test('root の外を指すシンボリックリンクは 404', async () => {
  const r = await get(srv.origin, '/sub/link.txt');
  assert.equal(r.status, 404);
});

test('HEAD は本文が空で GET と同じヘッダ、POST は 405', async () => {
  const g = await get(srv.origin, '/sub/app.js', { headers: GZ });
  const h = await get(srv.origin, '/sub/app.js', { method: 'HEAD', headers: GZ });
  assert.equal(h.status, 200);
  assert.equal(h.body.length, 0);
  assert.equal(h.headers['content-length'], g.headers['content-length']);
  assert.equal(h.headers['content-type'], g.headers['content-type']);
  const p = await get(srv.origin, '/sub/', { method: 'POST' });
  assert.equal(p.status, 405);
  assert.equal(p.headers.allow, 'GET, HEAD');
});

test('共通ヘッダ', async () => {
  const r = await get(srv.origin, '/sub/app.js');
  assert.equal(r.headers['cache-control'], 'no-store');
  assert.equal(r.headers['x-content-type-options'], 'nosniff');
});

test('base の正規化と任意の名前', async () => {
  for (const b of ['x', '/x', 'x/', '/x/']) assert.equal(normalizeBase(b), '/x/');
  assert.equal(normalizeBase('/'), '/');
  assert.equal(normalizeBase('/x/y/'), '/x/y/');
  for (const b of ['/../', '/a?b/', '/a/./b/', '/a//b/', '/a%2e/', '/a\\b/', '/a#b/']) {
    assert.throws(() => normalizeBase(b), undefined, b);
  }
  for (const b of ['/a-b_c/', '/x/y/', '/']) {
    const s = await startServer({ dir: root, base: b });
    try {
      const r = await get(s.origin, `${b}app.js`);
      assert.equal(r.status, 200, b);
      assert.ok(s.url.startsWith('http://127.0.0.1:'));
    } finally {
      await s.close();
    }
  }
});

test('不正な引数は例外', async () => {
  await assert.rejects(startServer({ dir: path.join(tmp, 'missing') }));
  await assert.rejects(startServer({ dir: root, gzipLevel: 0 }));
  await assert.rejects(startServer({ dir: root, gzipLevel: 10 }));
  await assert.rejects(startServer({ dir: root, port: 70000 }));
});

test('ファイルの書き換えが次の応答に反映される', async () => {
  const f = path.join(root, 'data.json');
  const before = zlib.gunzipSync((await get(srv.origin, '/sub/data.json', { headers: GZ })).body).toString();
  fs.writeFileSync(f, '{"a":1,"changed":"yes-longer"}');
  const after = zlib.gunzipSync((await get(srv.origin, '/sub/data.json', { headers: GZ })).body).toString();
  assert.notEqual(before, after);
  assert.match(after, /changed/);
});

function runCli(args) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, [SERVE, ...args], { stdio: ['ignore', 'pipe', 'pipe'] });
    resolve(child);
  });
}

test('CLI: 起動行の URL で配信し、SIGTERM で終了コード 0', async () => {
  const child = await runCli(['--dir', root, '--base', '/sub/', '--port', '0']);
  const line = await new Promise((resolve, reject) => {
    let buf = '';
    child.stdout.on('data', (c) => {
      buf += c;
      if (buf.includes('\n')) resolve(buf.split('\n')[0]);
    });
    child.once('error', reject);
    child.once('exit', () => reject(new Error('exited early')));
  });
  const m = /^serving .+ at (http:\/\/127\.0\.0\.1:\d+)(\/sub\/)$/.exec(line);
  assert.ok(m, line);
  const r = await get(m[1], m[2]);
  assert.equal(r.status, 200);
  const code = new Promise((resolve) => child.once('exit', (c) => resolve(c)));
  child.kill('SIGTERM');
  assert.equal(await code, 0);
});

test('CLI: 誤用は終了コード 2 と usage', async () => {
  for (const args of [['--bogus'], ['--dir', path.join(tmp, 'missing')], ['--gzip-level', '0'], ['--port', 'abc']]) {
    const child = await runCli(args);
    let err = '';
    child.stderr.on('data', (c) => (err += c));
    const code = await new Promise((resolve) => child.once('exit', (c) => resolve(c)));
    assert.equal(code, 2, args.join(' '));
    assert.match(err, /usage:/);
  }
});
