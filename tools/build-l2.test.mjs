// tools/build-l2.mjs（L2 の配布物の作成）の回帰テスト（`make test-build-l2`。TASK-112 1/2）。
// 合成の配信物と合成の実行ファイルだけを使い、実ビルド・通信は行わない。すべて mkdtemp の一時ディレクトリで完結する。
// 確かめること: 配布物の木・mode・site の一致・決定性・start.sh の 1 コマンドの起動・fail-closed・固定表の形。
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { after, before, describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { buildL2, PLATFORMS, readPins } from './build-l2.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SCRIPT = path.join(HERE, 'build-l2.mjs');
const CHECK = path.join(HERE, 'check-delivery.sh');
// mount_app だけを書き出す最小の wasm（check-delivery の錨を満たす）。
const WASM = Buffer.from([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x03, 0x02, 0x01, 0x00, 0x07, 0x0d,
  0x01, 0x09, 0x6d, 0x6f, 0x75, 0x6e, 0x74, 0x5f, 0x61, 0x70, 0x70, 0x00, 0x00, 0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b,
]);
const FAKE_SERVER = '#!/bin/sh\nexit 0\n';
const sha = (s) => createHash('sha256').update(s).digest('hex');
const PLATFORM = 'linux-x64';
const PINS = { [PLATFORMS[PLATFORM]]: sha(FAKE_SERVER) };

let tmp;
let seq = 0;
before(() => {
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-test-build-l2.'));
});
after(() => {
  fs.rmSync(tmp, { recursive: true, force: true });
});

function mk(label) {
  const d = path.join(tmp, `${label}-${seq++}`);
  fs.mkdirSync(d, { recursive: true });
  return d;
}

function makeSite() {
  const s = mk('site');
  fs.mkdirSync(path.join(s, 'wasm'));
  fs.mkdirSync(path.join(s, 'ir'));
  fs.writeFileSync(path.join(s, 'index.html'), '<!doctype html><link rel="stylesheet" href="./app.css"><script type="module" src="./app.js"></script>\n');
  fs.writeFileSync(path.join(s, 'app.css'), 'body{margin:0}\n');
  fs.writeFileSync(path.join(s, 'icon.svg'), '<svg xmlns="http://www.w3.org/2000/svg"/>\n');
  fs.writeFileSync(path.join(s, 'app.js'), "import init from './wasm/fandhe_workshop_web.js';\ninit();\n");
  fs.writeFileSync(path.join(s, 'wasm', 'fandhe_workshop_web.js'), 'export default function init() {}\n');
  fs.writeFileSync(path.join(s, 'wasm', 'fandhe_workshop_web_bg.wasm'), WASM);
  fs.writeFileSync(path.join(s, 'ir', 'main.json'), '{"nodes":[]}\n');
  return s;
}

function makeServer(content = FAKE_SERVER) {
  const f = path.join(mk('srv'), 'miniserve');
  fs.writeFileSync(f, content, { mode: 0o755 });
  return f;
}

function build(site, over = {}) {
  return buildL2({ server: makeServer(), site, out: mk('out'), platform: PLATFORM, pins: PINS, ...over });
}

function unzipTo(zip) {
  const d = mk('x');
  const r = spawnSync('unzip', ['-q', zip, '-d', d], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  return path.join(d, 'fandhe-workshop-l2');
}

function listZip(zip) {
  const r = spawnSync('unzip', ['-Z1', zip], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  return r.stdout.split('\n').filter(Boolean);
}

const run = (site, env) =>
  spawnSync(process.execPath, [SCRIPT], { env: { PATH: process.env.PATH, ...env }, encoding: 'utf8' });

describe('配信物の錨', () => {
  test('合成の配信物は check-delivery を通る', () => {
    const r = spawnSync('bash', [CHECK, makeSite()], { encoding: 'utf8' });
    assert.equal(r.status, 0, r.stdout + r.stderr);
  });
});

describe('正常系', () => {
  test('木・mode・site の一致', () => {
    const site = makeSite();
    const { zip } = build(site);
    assert.deepEqual(listZip(zip), [
      'fandhe-workshop-l2/',
      'fandhe-workshop-l2/miniserve',
      'fandhe-workshop-l2/site/',
      'fandhe-workshop-l2/site/app.css',
      'fandhe-workshop-l2/site/app.js',
      'fandhe-workshop-l2/site/icon.svg',
      'fandhe-workshop-l2/site/index.html',
      'fandhe-workshop-l2/site/ir/',
      'fandhe-workshop-l2/site/ir/main.json',
      'fandhe-workshop-l2/site/wasm/',
      'fandhe-workshop-l2/site/wasm/fandhe_workshop_web.js',
      'fandhe-workshop-l2/site/wasm/fandhe_workshop_web_bg.wasm',
      'fandhe-workshop-l2/start.sh',
    ]);
    const top = unzipTo(zip);
    const mode = (p) => fs.statSync(path.join(top, p)).mode & 0o777;
    assert.equal(mode('start.sh'), 0o755);
    assert.equal(mode('miniserve'), 0o755);
    assert.equal(mode('site/index.html'), 0o644);
    assert.equal(mode('site/wasm'), 0o755);
    for (const rel of ['index.html', 'app.css', 'icon.svg', 'app.js', 'ir/main.json', 'wasm/fandhe_workshop_web_bg.wasm']) {
      assert.ok(fs.readFileSync(path.join(top, 'site', rel)).equals(fs.readFileSync(path.join(site, rel))), rel);
    }
    assert.equal(fs.readFileSync(path.join(top, 'start.sh'), 'utf8'), fs.readFileSync(path.join(HERE, 'l2', 'start.sh'), 'utf8'));
  });

  test('決定性: 同じ入力で同じバイト列', () => {
    const site = makeSite();
    const a = build(site);
    const b = build(site);
    assert.ok(fs.readFileSync(a.zip).equals(fs.readFileSync(b.zip)));
    assert.equal(a.sha256, b.sha256);
  });
});

describe('start.sh の 1 コマンドの起動', () => {
  function prepared() {
    const top = unzipTo(build(makeSite()).zip);
    const rec = path.join(mk('rec'), 'args.txt');
    // 実行ファイルを、引数と作業ディレクトリを記録して終わる stub に置き換える。
    fs.writeFileSync(path.join(top, 'miniserve'), `#!/bin/sh\npwd > "${rec}"\nprintf '%s\\n' "$@" >> "${rec}"\n`, { mode: 0o755 });
    return { top, rec };
  }
  const start = (top, port) =>
    spawnSync('env', ['-i', 'PATH=/usr/bin:/bin', `HOME=${tmp}`, 'FANDHE_NO_OPEN=1', ...(port === undefined ? [] : [`PORT=${port}`]), './start.sh'], {
      cwd: top,
      encoding: 'utf8',
    });

  test('URL を表示し、miniserve を 127.0.0.1 だけで起動する', () => {
    const { top, rec } = prepared();
    const r = start(top, '8799');
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /http:\/\/127\.0\.0\.1:8799\//);
    const lines = fs.readFileSync(rec, 'utf8').trim().split('\n');
    assert.equal(fs.realpathSync(lines[0]), fs.realpathSync(top));
    assert.deepEqual(lines.slice(1), ['--index', 'index.html', '-i', '127.0.0.1', '-p', '8799', 'site']);
  });

  test('PORT 未指定は 8787', () => {
    const { top } = prepared();
    assert.match(start(top).stdout, /127\.0\.0\.1:8787\//);
  });

  for (const bad of ['abc', '0', '70000', '080', '1;ls']) {
    test(`不正な PORT ${bad} は 2 で miniserve を呼ばない`, () => {
      const { top, rec } = prepared();
      const r = start(top, bad);
      assert.equal(r.status, 2);
      assert.equal(fs.existsSync(rec), false);
    });
  }
});

describe('fail-closed', () => {
  const noZip = (out) => assert.deepEqual(fs.readdirSync(out), []);

  test('digest の不一致（固定表に対して合成の実行ファイル）', () => {
    const out = mk('out');
    const r = run(null, { L2_SERVER: makeServer(), L2_SITE: makeSite(), L2_OUT: out, L2_PLATFORM: PLATFORM });
    assert.equal(r.status, 1);
    assert.match(r.stderr, /sha256/);
    noZip(out);
  });

  test('digest の不一致（注入した表）', () => {
    const out = mk('out');
    assert.throws(() => buildL2({ server: makeServer('other'), site: makeSite(), out, platform: PLATFORM, pins: PINS }), /sha256/);
    noZip(out);
  });

  const bad = {
    'test-hooks.js を含む': (s) => fs.writeFileSync(path.join(s, 'test-hooks.js'), ''),
    'fandhe-test-hooks の meta': (s) => fs.appendFileSync(path.join(s, 'index.html'), '<meta name="fandhe-test-hooks">\n'),
    '.d.ts を含む': (s) => fs.writeFileSync(path.join(s, 'wasm', 'x.d.ts'), ''),
    'シンボリックリンク': (s) => fs.symlinkSync('app.css', path.join(s, 'link.css')),
    'ir/main.json が無い': (s) => fs.rmSync(path.join(s, 'ir', 'main.json')),
    'ir/main.json が JSON でない': (s) => fs.writeFileSync(path.join(s, 'ir', 'main.json'), 'not json'),
  };
  for (const [label, mutate] of Object.entries(bad)) {
    test(`配信物が不正: ${label}`, () => {
      const site = makeSite();
      mutate(site);
      const out = mk('out');
      assert.throws(() => buildL2({ server: makeServer(), site, out, platform: PLATFORM, pins: PINS }), /error|check-delivery|main\.json/i);
      noZip(out);
    });
  }

  test('誤用は終了コード 2', () => {
    const base = { L2_SERVER: makeServer(), L2_SITE: makeSite(), L2_OUT: mk('out') };
    assert.equal(run(null, { ...base, L2_PLATFORM: 'windows' }).status, 2);
    assert.equal(run(null, { L2_SITE: base.L2_SITE, L2_OUT: base.L2_OUT }).status, 2);
    const r = spawnSync(process.execPath, [SCRIPT, 'extra'], { env: { PATH: process.env.PATH }, encoding: 'utf8' });
    assert.equal(r.status, 2);
  });

  test('出力先の対象名がディレクトリまたはリンクなら拒否', () => {
    const name = `fandhe-workshop-l2-${PLATFORM}.zip`;
    const o1 = mk('out');
    fs.mkdirSync(path.join(o1, name));
    assert.throws(() => build(makeSite(), { out: o1 }), /not a regular file/);
    const o2 = mk('out');
    fs.symlinkSync(path.join(o2, 'nowhere'), path.join(o2, name));
    assert.throws(() => build(makeSite(), { out: o2 }), /not a regular file/);
  });

  test('存在せず dist/ の外の出力先は拒否', () => {
    assert.throws(() => build(makeSite(), { out: path.join(tmp, 'nope', 'deeper') }), /not under dist/);
  });
});

describe('固定表の形', () => {
  test('tools/l2/miniserve.sha256 は 2 行で、期待する資産名と 64 桁の 16 進', () => {
    const pins = readPins(path.join(HERE, 'l2', 'miniserve.sha256'));
    assert.deepEqual(Object.keys(pins).sort(), Object.values(PLATFORMS).sort());
    for (const v of Object.values(pins)) assert.match(v, /^[0-9a-f]{64}$/);
    assert.equal(fs.readFileSync(path.join(HERE, 'l2', 'miniserve.sha256'), 'utf8').trim().split('\n').length, 2);
  });
});
