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
import { buildL2, LAYOUTS, PLATFORMS, readPins } from './build-l2.mjs';

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
    assert.equal(run(null, { ...base, L2_PLATFORM: 'windows-arm64' }).status, 2);
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
  test('tools/l2/miniserve.sha256 は 3 行で、期待する資産名と 64 桁の 16 進', () => {
    const pins = readPins(path.join(HERE, 'l2', 'miniserve.sha256'));
    assert.deepEqual(Object.keys(pins).sort(), Object.values(PLATFORMS).sort());
    for (const v of Object.values(pins)) assert.match(v, /^[0-9a-f]{64}$/);
    assert.equal(fs.readFileSync(path.join(HERE, 'l2', 'miniserve.sha256'), 'utf8').trim().split('\n').length, 3);
  });

  test('.exe の資産名の行を受け付け、不正な拡張子・資産名は終了コード 2', () => {
    const h = 'a'.repeat(64);
    const f = path.join(mk('pins'), 'pins.sha256');
    fs.writeFileSync(f, `${h}  miniserve-0.35.0-x86_64-pc-windows-msvc.exe\n`);
    assert.deepEqual(readPins(f), { 'miniserve-0.35.0-x86_64-pc-windows-msvc.exe': h });
    for (const bad of ['miniserve-0.35.0-x.dll', 'miniserve-0.35.0-x.exe.bak','other-0.35.0-x.exe']) {
      fs.writeFileSync(f, `${h}  ${bad}\n`);
      assert.throws(() => readPins(f), (e) => e.code === 2, bad);
    }
  });
});

describe('Windows 向け（TASK-198。DLV-22）', () => {
  const WIN = 'windows-x64';
  const WPINS = { [PLATFORMS[WIN]]: sha(FAKE_SERVER) };
  const buildWin = (site, over = {}) =>
    buildL2({ server: makeServer(), site, out: mk('out'), platform: WIN, pins: WPINS, ...over });
  const CMD = fs.readFileSync(path.join(HERE, 'l2', 'start.cmd'), 'utf8');
  const noZip = (out) => assert.deepEqual(fs.readdirSync(out), []);

  test('木・mode・実行ファイルの一致', () => {
    const { zip } = buildWin(makeSite());
    const names = listZip(zip);
    assert.ok(names.includes('fandhe-workshop-l2/start.cmd'));
    assert.ok(names.includes('fandhe-workshop-l2/miniserve.exe'));
    assert.ok(!names.includes('fandhe-workshop-l2/start.sh'));
    assert.ok(!names.includes('fandhe-workshop-l2/miniserve'));
    const top = unzipTo(zip);
    const mode = (p) => fs.statSync(path.join(top, p)).mode & 0o777;
    assert.equal(mode('start.cmd'), 0o755);
    assert.equal(mode('miniserve.exe'), 0o755);
    assert.equal(mode('site/index.html'), 0o644);
    assert.equal(fs.readFileSync(path.join(top, 'miniserve.exe'), 'utf8'), FAKE_SERVER);
  });

  test('start.cmd は元の LF を CRLF にしたもの（裸の LF 0 件・ASCII のみ）', () => {
    const top = unzipTo(buildWin(makeSite()).zip);
    const got = fs.readFileSync(path.join(top, 'start.cmd'));
    assert.equal(got.toString('latin1'), CMD.replace(/\n/g, '\r\n'));
    assert.ok(!/(?<!\r)\n/.test(got.toString('latin1')));
    assert.ok(got.toString('latin1').endsWith('\r\n'));
    assert.ok(got.every((b) => b < 0x80));
    assert.ok(!CMD.includes('\r'), 'repo の start.cmd は LF');
  });

  test('start.cmd の起動の引数は start.sh と同じで、安全な書き方を守る', () => {
    const sh = fs.readFileSync(path.join(HERE, 'l2', 'start.sh'), 'utf8');
    const shArgs = /exec \.\/miniserve (.+)$/m.exec(sh)[1].replaceAll('"${PORT}"', '<PORT>');
    const cmdArgs = /^\.\\miniserve\.exe (.+)$/m.exec(CMD)[1].replaceAll('!PORT!', '<PORT>');
    assert.equal(cmdArgs, shArgs);
    assert.match(sh, /PORT="\$\{PORT:-8787\}"/);
    assert.match(CMD, /set "PORT=8787"/);
    assert.ok(CMD.includes('cd /d "%~dp0"'));
    assert.ok(CMD.indexOf('cd /d "%~dp0"') < CMD.indexOf('setlocal EnableDelayedExpansion'));
    assert.ok(!CMD.includes('%PORT%'));
    assert.ok(!CMD.includes('set /a'));
    assert.ok(!CMD.includes('0.0.0.0'));
    assert.match(CMD, /127\.0\.0\.1:!PORT!/);
  });

  test('site は linux-x64 の配布物と同じ中身', () => {
    const site = makeSite();
    const a = unzipTo(build(site).zip);
    const b = unzipTo(buildWin(site).zip);
    const files = (d) => fs.readdirSync(path.join(d, 'site'), { recursive: true }).sort();
    assert.deepEqual(files(b), files(a));
    for (const rel of files(a)) {
      const pa = path.join(a, 'site', rel);
      if (fs.statSync(pa).isFile()) assert.ok(fs.readFileSync(pa).equals(fs.readFileSync(path.join(b, 'site', rel))), rel);
    }
  });

  test('決定性: 同じ入力で同じバイト列', () => {
    const site = makeSite();
    assert.equal(buildWin(site).sha256, buildWin(site).sha256);
  });

  test('他の platform の実行ファイル（固定値の不一致）では zip を作らない', () => {
    const out = mk('out');
    assert.throws(() => buildWin(makeSite(), { server: makeServer('MZ-other'), out }), /sha256/);
    noZip(out);
    assert.throws(() => buildWin(makeSite(), { pins: { [PLATFORMS['linux-x64']]: sha(FAKE_SERVER) }, out }), /sha256/);
    noZip(out);
  });

  test('元の起動スクリプトに CR・非 ASCII があれば拒否', () => {
    for (const body of ['@echo off\r\n', '@echo off\nrem 日本\n']) {
      const src = path.join(mk('cmd'), 'start.cmd');
      fs.writeFileSync(src, body);
      const layouts = { ...LAYOUTS, [WIN]: { ...LAYOUTS[WIN], launcherSrc: path.relative(path.join(HERE, 'l2'), src) } };
      const out = mk('out');
      assert.throws(() => buildWin(makeSite(), { layouts, out }), /LF-only ASCII/);
      noZip(out);
    }
  });

  for (const [label, name] of [
    ['予約名', 'CON.txt'],
    ['禁止文字', 'a:b.txt'],
    ['バックスラッシュ', 'a\\b.js'],
    ['末尾の点', 'x.'],
  ]) {
    test(`Windows で作れない名前（${label}）は windows-x64 で拒否し、linux-x64 は通る`, () => {
      const site = makeSite();
      fs.writeFileSync(path.join(site, name), 'x\n');
      const out = mk('out');
      assert.throws(() => buildWin(site, { out }), /cannot be extracted on Windows/);
      noZip(out);
      assert.ok(fs.existsSync(build(site).zip));
    });
  }

  test('大文字小文字だけ違う 2 件は windows-x64 で拒否し、linux-x64 は通る', () => {
    const site = makeSite();
    fs.writeFileSync(path.join(site, 'Readme.txt'), 'a\n');
    fs.writeFileSync(path.join(site, 'README.txt'), 'b\n');
    assert.throws(() => buildWin(site), /differ only in case/);
    assert.ok(fs.existsSync(build(site).zip));
  });

  const REAL = process.env.L2_SERVER_WINDOWS;
  test('実物の exe の照合（L2_SERVER_WINDOWS があるときだけ）', { skip: REAL ? false : 'L2_SERVER_WINDOWS is not set' }, () => {
    const pins = readPins(path.join(HERE, 'l2', 'miniserve.sha256'));
    const site = makeSite();
    const a = buildL2({ server: REAL, site, out: mk('out'), platform: WIN, pins });
    const b = buildL2({ server: REAL, site, out: mk('out'), platform: WIN, pins });
    assert.equal(a.sha256, b.sha256);
    const exe = fs.readFileSync(path.join(unzipTo(a.zip), 'miniserve.exe'));
    assert.ok(exe.equals(fs.readFileSync(REAL)));
    assert.equal(exe.subarray(0, 2).toString('latin1'), 'MZ');
  });
});
