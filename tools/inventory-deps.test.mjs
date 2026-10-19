// tools/inventory-deps.mjs（依存の棚卸し。TASK-99）の回帰テスト。node:test。
//
// loopback の stub サーバに試料を置いて流し、実 crates.io には通信しない。Cargo.lock は mkdtemp の一時ディレクトリに
// 組み立て、作業ツリーを変えない（実 repo の Cargo.lock は読むだけ）。
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  ACKNOWLEDGED_SINGLE_OWNER, directDependencyNames, indexPath, latestVersion, main, parseLock,
} from './inventory-deps.mjs';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const REG = 'registry+https://github.com/rust-lang/crates.io-index';

function pkg(name, version, { source = REG, deps = [], checksum = 'a'.repeat(64) } = {}) {
  let s = `[[package]]\nname = "${name}"\nversion = "${version}"\n`;
  if (source) s += `source = "${source}"\nchecksum = "${checksum}"\n`;
  if (deps.length) s += `dependencies = [\n${deps.map((d) => ` "${d}",\n`).join('')}]\n`;
  return s;
}
const lockOf = (...pkgs) => `# generated\nversion = 4\n\n${pkgs.join('\n')}`;

// stub の既定: 全 crate が 2 人所有で、取り下げ無し。crate ごとに上書きする。
async function withStub(spec, fn) {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    const send = (code, body, type = 'application/json') => { res.writeHead(code, { 'content-type': type }); res.end(body); };
    let m = /^\/index\/(?:\d|\d\/\w|\w\w\/\w\w)\/([\w-]+)$/.exec(url.pathname);
    if (m) {
      const c = spec[m[1]] ?? {};
      if (c.index === 404) return send(404, '');
      if (c.index === 500) return send(500, 'x');
      if (c.index === 'redirect') { res.writeHead(302, { location: '/index/zz' }); return res.end(); }
      if (c.index === 'hang') return; // 応答しない
      return send(200, c.indexBody ?? c.entries.map((e) => JSON.stringify(e)).join('\n') + '\n');
    }
    m = /^\/api\/v1\/crates\/([\w-]+)\/owners$/.exec(url.pathname);
    if (m) {
      const c = spec[m[1]] ?? {};
      if (c.owners === 404) return send(404, '');
      if (c.owners === 500) return send(500, 'x');
      if (c.owners === 'broken') return send(200, '{not json');
      return send(200, JSON.stringify({ users: c.owners ?? [{ kind: 'user' }, { kind: 'user' }] }));
    }
    m = /^\/api\/v1\/crates\/([\w-]+)$/.exec(url.pathname);
    if (m) return send(200, JSON.stringify({ crate: { updated_at: '2026-01-02T03:04:05Z' } }));
    return send(404, '');
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const base = `http://127.0.0.1:${server.address().port}`;
  try { return await fn(base); } finally { await new Promise((r) => { server.close(r); server.closeAllConnections(); }); }
}

function entry(vers, extra = {}) {
  return { vers, cksum: 'a'.repeat(64), yanked: false, ...extra };
}
const entries = (...v) => ({ entries: v.map((x) => entry(x)) });

// 例外の表は実物（yaml_serde）を持つ。古い例外の検査が他の試験に混ざらないよう、既定では空にして流す。
async function run(lockText, base, extraArgs = [], { keepExceptions = false } = {}) {
  const saved = new Map(ACKNOWLEDGED_SINGLE_OWNER);
  if (!keepExceptions) ACKNOWLEDGED_SINGLE_OWNER.clear();
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'inv-deps-'));
  try {
    const lock = path.join(dir, 'Cargo.lock');
    fs.writeFileSync(lock, lockText);
    let out = '';
    let err = '';
    const code = await main(
      ['--lock', lock, '--delay-ms', '0', '--index-base', `${base}/index`, '--api-base', `${base}/api/v1`, ...extraArgs],
      { stdout: (s) => { out += s; }, stderr: (s) => { err += s; } },
    );
    return { code, out, err };
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    for (const [k, v] of saved) ACKNOWLEDGED_SINGLE_OWNER.set(k, v);
  }
}

const member = (deps) => pkg('ws-member', '0.0.0', { source: null, deps });

test('正常: 例外に載った所有者 1 人だけなら 0 で、全件と基点が出る', async () => {
  const lock = lockOf(member(['yaml_serde', 'serde']), pkg('yaml_serde', '0.10.7'), pkg('serde', '1.0.0'), pkg('itoa', '1.0.0'));
  await withStub({
    yaml_serde: { ...entries('0.10.7'), owners: [{ kind: 'user' }] },
    serde: entries('1.0.0', '1.0.1'),
    itoa: entries('1.0.0'),
  }, async (base) => {
    const r = await run(lock, base, [], { keepExceptions: true });
    assert.equal(r.code, 0, r.err);
    for (const n of ['yaml_serde', 'serde', 'itoa']) assert.match(r.out, new RegExp(`\\| ${n} \\|`));
    assert.match(r.out, /所有者 1 人（例外: D-58/);
    assert.match(r.out, /実行日（UTC） \| \d{4}-\d{2}-\d{2}/);
    assert.match(r.out, /Cargo\.lock の SHA-256 \| [0-9a-f]{64}/);
    assert.match(r.out, /\| serde \| 1\.0\.0 \| 直接 \| 2\/0 \| いいえ \| 1\.0\.1 \| 2026-01-02 \|/);
    assert.match(r.out, /\| itoa \| 1\.0\.0 \| 推移 \|/);
    assert.match(r.out, /要確認: なし/);
  });
});

test('取り下げ済みの版が lock にあれば 1 で、注記に出る', async () => {
  await withStub({ bad: { entries: [entry('1.0.0', { yanked: true })] } }, async (base) => {
    const r = await run(lockOf(pkg('bad', '1.0.0')), base);
    assert.equal(r.code, 1);
    assert.match(r.out, /取り下げ済み/);
    assert.match(r.out, /bad 1\.0\.0: 取り下げ済み/);
  });
});

test('例外に無い所有者 1 人は記録して要約に出すが、終了コードは変えない', async () => {
  await withStub({ solo: { ...entries('1.0.0'), owners: [{ kind: 'user' }] } }, async (base) => {
    const r = await run(lockOf(member(['solo']), pkg('solo', '1.0.0')), base);
    assert.equal(r.code, 0);
    assert.match(r.out, /\| solo \| 1\.0\.0 \| 直接 \| 1\/0 \|.*\| 所有者 1 人 \|/);
    assert.match(r.out, /直接の依存（例外に無い）: solo/);
  });
});

test('team を持つ crate は所有者 1 人に数えない', async () => {
  await withStub({ t: { ...entries('1.0.0'), owners: [{ kind: 'user' }, { kind: 'team' }] }, u: { ...entries('1.0.0'), owners: [{ kind: 'team' }] } }, async (base) => {
    const r = await run(lockOf(pkg('t', '1.0.0'), pkg('u', '1.0.0')), base);
    assert.equal(r.code, 0, r.out);
    assert.match(r.out, /\| t \| 1\.0\.0 \| 推移 \| 1\/1 \|/);
  });
});

test('直接/推移の判定は member の dependencies（`name version` の形を含む）で決める', () => {
  const pk = parseLock(lockOf(
    pkg('m', '0.0.0', { source: null, deps: ['a', 'b 1.0.0', 'c 2.0.0 (registry+https://x)', 'm2'] }),
    pkg('m2', '0.0.0', { source: null }),
    pkg('a', '1.0.0'), pkg('b', '1.0.0'), pkg('c', '2.0.0'), pkg('d', '1.0.0', { deps: ['a'] }),
  ));
  assert.deepEqual([...directDependencyNames(pk)].sort(), ['a', 'b', 'c']);
});

test('index の 404 は 1、API の 5xx・壊れた JSON・checksum の不一致・リダイレクトは 2 で標準出力は空', async () => {
  await withStub({ gone: { index: 404 } }, async (base) => {
    const r = await run(lockOf(pkg('gone', '1.0.0')), base);
    assert.equal(r.code, 1);
    assert.match(r.out, /registry に版が無い/);
  });
  const cases = {
    api5xx: { ...entries('1.0.0'), owners: 500 },
    index5xx: { index: 500 },
    broken: { ...entries('1.0.0'), owners: 'broken' },
    badIndex: { indexBody: '{"vers":1}\n' },
    cksum: { entries: [entry('1.0.0', { cksum: 'b'.repeat(64) })] },
    redirect: { index: 'redirect' },
  };
  for (const [name, spec] of Object.entries(cases)) {
    await withStub({ x: spec }, async (base) => {
      const r = await run(lockOf(pkg('x', '1.0.0')), base);
      assert.equal(r.code, 2, `${name}: ${r.err}`);
      assert.equal(r.out, '', name);
    });
  }
});

test('応答が来なければタイムアウトで 2 になり、標準出力は空', async () => {
  await withStub({ x: { index: 'hang' } }, async (base) => {
    const r = await run(lockOf(pkg('x', '1.0.0')), base, ['--timeout-ms', '200']);
    assert.equal(r.code, 2);
    assert.equal(r.out, '');
  });
});

test('不正な crate 名・許可外の base・未知の引数・不正な lock は 2', async () => {
  await withStub({}, async (base) => {
    assert.equal((await run(lockOf(pkg('bad name', '1.0.0')), base)).code, 2);
    assert.equal((await run(lockOf(pkg('x', '1.0.0')), base, ['--bogus', '1'])).code, 2);
    assert.equal((await run('', base)).code, 2);
    assert.equal((await run('[[package]]\nname = "x"\nversion = "1"\ndependencies = [\n "a",\n', base)).code, 2);
  });
  for (const bad of ['https://example.com', 'http://127.0.0.1.evil.com:80', 'http://localhost:80', ['http://127.0.0.1', '80@evil.com'].join(':')]) {
    let out = '';
    const code = await main(['--api-base', bad], { stdout: (s) => { out += s; }, stderr: () => {} });
    assert.equal(code, 2, bad);
    assert.equal(out, '');
  }
});

test('registry 以外の source を含む lock は 1', async () => {
  await withStub({}, async (base) => {
    const r = await run(lockOf(pkg('g', '1.0.0', { source: 'git+https://example.com/g#abc' })), base);
    assert.equal(r.code, 1);
    assert.match(r.out, /crates\.io 以外/);
  });
});

test('Cargo.lock に無い名前の例外は古い例外として 1', async () => {
  await withStub({ a: entries('1.0.0') }, async (base) => {
    const r = await run(lockOf(pkg('a', '1.0.0')), base, [], { keepExceptions: true });
    assert.equal(r.code, 1);
    for (const name of ACKNOWLEDGED_SINGLE_OWNER.keys()) assert.match(r.out, new RegExp(`${name}: 例外に載っているが`));
  });
});

test('Markdown の表は `|` を含む値で壊れない', async () => {
  // 名前は検査で弾かれるため、注記側（例外の理由）を差し替えて確かめる
  ACKNOWLEDGED_SINGLE_OWNER.set('pipe', 'a|b\nc');
  try {
    await withStub({ pipe: { ...entries('1.0.0'), owners: [{ kind: 'user' }] }, yaml_serde: entries('0.10.7') }, async (base) => {
      const r = await run(lockOf(pkg('pipe', '1.0.0'), pkg('yaml_serde', '0.10.7')), base, [], { keepExceptions: true });
      const row = r.out.split('\n').find((l) => l.startsWith('| pipe |'));
      assert.match(row, /a\\\|b c/);
      assert.equal(row.replace(/\\\|/g, '').split('|').length, 10);
    });
  } finally {
    ACKNOWLEDGED_SINGLE_OWNER.delete('pipe');
  }
});

test('--format json は JSON.parse でき、必須の鍵がそろう', async () => {
  await withStub({ a: entries('1.0.0') }, async (base) => {
    const r = await run(lockOf(pkg('a', '1.0.0')), base, ['--format', 'json']);
    const j = JSON.parse(r.out);
    for (const k of ['date', 'commit', 'lock_sha256', 'node', 'hosts']) assert.ok(k in j.baseline, k);
    for (const k of ['registry_crates', 'yanked', 'single_owner', 'flags']) assert.ok(k in j.summary, k);
    assert.equal(j.rows.length, 1);
    assert.equal(r.out.includes('login'), false);
  });
});

test('indexPath は cargo の経路の規則に従う', () => {
  assert.equal(indexPath('a'), '1/a');
  assert.equal(indexPath('ab'), '2/ab');
  assert.equal(indexPath('abc'), '3/a/abc');
  assert.equal(indexPath('serde'), 'se/rd/serde');
  assert.equal(indexPath('Yaml_Serde'), 'ya/ml/yaml_serde');
});

test('最新版は build metadata の `-` を pre-release と誤らず、pre-release と取り下げは除く', () => {
  const e = (vers, yanked = false) => ({ vers, yanked });
  assert.equal(latestVersion([e('0.9.8'), e('1.1.8+spec-1.1.0'), e('1.0.4+spec-1.0.0')]), '1.1.8+spec-1.1.0');
  assert.equal(latestVersion([e('1.0.0'), e('2.0.0-rc.1'), e('1.5.0', true)]), '1.0.0');
  assert.equal(latestVersion([e('1.0.0-alpha')]), null);
});

test('実 repo の Cargo.lock を読み、registry の件数が行の数え直しと一致する（通信なし）', () => {
  const text = fs.readFileSync(path.join(REPO_ROOT, 'Cargo.lock'), 'utf8');
  const pk = parseLock(text);
  const counted = text.split('\n').filter((l) => l === `source = "${REG}"`).length;
  assert.equal(pk.filter((p) => p.source === REG).length, counted);
  assert.ok(counted > 0);
  assert.ok(directDependencyNames(pk).has('yaml_serde'));
  assert.ok(pk.filter((p) => p.name === 'yaml_serde').length === 1);
});
