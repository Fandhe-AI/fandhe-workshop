// tools/update-l2.mjs（L2 の配布物の更新）の回帰テスト（`make test-update-l2`。TASK-116。D-58・DLV-18）。
// 合成の配信物・実行ファイルと mkdtemp の一時ディレクトリだけを使い、ビルド・通信は行わない。
// 確かめること: site/ 以外のバイト列と mode の引き継ぎ・site/ の完全な入れ替え・build との等価性と決定性・
// 1 コマンドの起動・fail-closed（不正な旧 zip・不正な新 site・digest の不一致・誤用）。
// Windows 向けの zip（start.cmd・miniserve.exe。#1932・DLV-22）の更新も同じ道具で扱う。
// 実際の miniserve を使う起動の確認は scripts/test-l2-run.sh の遅い層（tools/browser/l2-update.test.mjs）が行う。
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { after, before, describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { buildL2, PLATFORMS, TOP, writeZip } from './build-l2.mjs';
import { updateL2 } from './update-l2.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SCRIPT = path.join(HERE, 'update-l2.mjs');
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
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-test-update-l2.'));
});
after(() => {
  fs.rmSync(tmp, { recursive: true, force: true });
});

function mk(label) {
  const d = path.join(tmp, `${label}-${seq++}`);
  fs.mkdirSync(d, { recursive: true });
  return d;
}

function makeSite(ir = '{"nodes":[]}\n', extra = {}) {
  const s = mk('site');
  fs.mkdirSync(path.join(s, 'wasm'));
  fs.mkdirSync(path.join(s, 'ir'));
  fs.writeFileSync(path.join(s, 'index.html'), '<!doctype html><link rel="stylesheet" href="./app.css"><script type="module" src="./app.js"></script>\n');
  fs.writeFileSync(path.join(s, 'app.css'), 'body{margin:0}\n');
  fs.writeFileSync(path.join(s, 'app.js'), "import init from './wasm/fandhe_workshop_web.js';\ninit();\n");
  fs.writeFileSync(path.join(s, 'wasm', 'fandhe_workshop_web.js'), 'export default function init() {}\n');
  fs.writeFileSync(path.join(s, 'wasm', 'fandhe_workshop_web_bg.wasm'), WASM);
  fs.writeFileSync(path.join(s, 'ir', 'main.json'), ir);
  for (const [rel, body] of Object.entries(extra)) fs.writeFileSync(path.join(s, rel), body);
  return s;
}

function makeServer() {
  const f = path.join(mk('srv'), 'miniserve');
  fs.writeFileSync(f, FAKE_SERVER, { mode: 0o755 });
  return f;
}

const build = (site) => buildL2({ server: makeServer(), site, out: mk('out'), platform: PLATFORM, pins: PINS });
const update = (zip, site, over = {}) => updateL2({ zip, site, out: mk('out'), pins: PINS, ...over });
const read = (f) => fs.readFileSync(f);

function unzipTo(zip) {
  const d = mk('x');
  const r = spawnSync('unzip', ['-q', zip, '-d', d], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  return path.join(d, TOP);
}

function listZip(zip) {
  const r = spawnSync('unzip', ['-Z1', zip], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  return r.stdout.split('\n').filter(Boolean);
}

/** 旧 zip を writeZip で合成して書く（不正な旧 zip の再現にも使う）。 */
function synth(entries) {
  const f = path.join(mk('syn'), 'old.zip');
  fs.writeFileSync(f, writeZip(entries));
  return f;
}

const GOOD = () => [
  { name: `${TOP}/`, dir: true },
  { name: `${TOP}/start.sh`, data: Buffer.from('#!/bin/sh\n'), mode: 0o755 },
  { name: `${TOP}/miniserve`, data: Buffer.from(FAKE_SERVER), mode: 0o755 },
  { name: `${TOP}/site/`, dir: true },
];

const treeOf = (root) => {
  const out = {};
  const walk = (abs, rel) => {
    for (const ent of fs.readdirSync(abs, { withFileTypes: true })) {
      const r = `${rel}${ent.name}`;
      const a = path.join(abs, ent.name);
      if (ent.isDirectory()) walk(a, `${r}/`);
      else out[r] = { mode: fs.statSync(a).mode & 0o777, data: fs.readFileSync(a).toString('base64') };
    }
  };
  walk(root, '');
  return out;
};

describe('更新の正常系', () => {
  test('site/ 以外は旧 zip のバイト列と mode のまま、site/ は新しい配信物と一致し旧にだけあるファイルは残らない', () => {
    const oldZip = build(makeSite('{"v":"A"}\n', { 'only-in-a.txt': 'old\n' })).zip;
    const siteB = makeSite('{"v":"B"}\n');
    const r = update(oldZip, siteB);
    assert.equal(r.platform, PLATFORM);
    const a = treeOf(unzipTo(oldZip));
    const b = treeOf(unzipTo(r.zip));
    for (const k of ['start.sh', 'miniserve']) assert.deepEqual(b[k], a[k], k);
    const siteTree = treeOf(siteB);
    const got = Object.fromEntries(Object.entries(b).filter(([k]) => k.startsWith('site/')).map(([k, v]) => [k.slice(5), v]));
    assert.deepEqual(Object.keys(got).sort(), Object.keys(siteTree).sort());
    assert.equal(got['ir/main.json'].data, Buffer.from('{"v":"B"}\n').toString('base64'));
    assert.equal(got['only-in-a.txt'], undefined);
    assert.equal(a['site/only-in-a.txt'] !== undefined, true);
  });

  test('update(build(A), B) は build(B) とバイト単位で一致し、update(build(A), A) は build(A) と一致する', () => {
    const siteA = makeSite('{"v":"A"}\n');
    const siteB = makeSite('{"v":"B"}\n');
    const server = makeServer();
    const mkBuild = (site) => buildL2({ server, site, out: mk('out'), platform: PLATFORM, pins: PINS }).zip;
    const a = mkBuild(siteA);
    assert.ok(read(update(a, siteB).zip).equals(read(mkBuild(siteB))));
    assert.ok(read(update(a, siteA).zip).equals(read(a)));
  });

  test('決定性: 同じ入力で 2 回更新すると同じバイト列', () => {
    const a = build(makeSite('{"v":"A"}\n')).zip;
    const siteB = makeSite('{"v":"B"}\n');
    const x = update(a, siteB);
    const y = update(a, siteB);
    assert.ok(read(x.zip).equals(read(y.zip)));
    assert.equal(x.sha256, y.sha256);
  });

  test('更新した zip も続けて更新できる', () => {
    const a = build(makeSite('{"v":"A"}\n')).zip;
    const b = update(a, makeSite('{"v":"B"}\n')).zip;
    const c = update(b, makeSite('{"v":"C"}\n'));
    assert.equal(treeOf(unzipTo(c.zip))['site/ir/main.json'].data, Buffer.from('{"v":"C"}\n').toString('base64'));
  });

  test('パターン文字（[ ] * ?）を含む名前の旧 site でも更新でき、更新した zip をもう一度更新できる', () => {
    const odd = { 'image[1].png': 'x', 'a*b?.txt': 'y' };
    const a = build(makeSite('{"v":"A"}\n', odd)).zip;
    const b = update(a, makeSite('{"v":"B"}\n', odd)).zip;
    const c = update(b, makeSite('{"v":"C"}\n'));
    const t = treeOf(unzipTo(c.zip));
    assert.equal(t['site/ir/main.json'].data, Buffer.from('{"v":"C"}\n').toString('base64'));
    assert.equal(t['site/image[1].png'], undefined);
  });

  test('将来の同梱物（README・ライセンス文など）が site/ 以外にあっても、そのまま残る', () => {
    const entries = [
      ...GOOD(),
      { name: `${TOP}/README.txt`, data: Buffer.from('読み方\n'), mode: 0o644 },
      { name: `${TOP}/licenses/`, dir: true },
      { name: `${TOP}/licenses/MIT.txt`, data: Buffer.from('MIT\n'), mode: 0o644 },
    ];
    const old = synth(entries);
    const r = update(old, makeSite('{"v":"B"}\n'));
    const a = treeOf(unzipTo(old));
    const b = treeOf(unzipTo(r.zip));
    for (const k of ['start.sh', 'miniserve', 'README.txt', 'licenses/MIT.txt']) assert.deepEqual(b[k], a[k], k);
    assert.ok(listZip(r.zip).includes(`${TOP}/licenses/`));
  });

  test('出力先は入力の zip と同じディレクトリでもよい', () => {
    const a = build(makeSite('{"v":"A"}\n'));
    const r = updateL2({ zip: a.zip, site: makeSite('{"v":"B"}\n'), out: path.dirname(a.zip), pins: PINS });
    assert.equal(r.zip, a.zip);
    assert.equal(treeOf(unzipTo(r.zip))['site/ir/main.json'].data, Buffer.from('{"v":"B"}\n').toString('base64'));
  });
});

describe('更新した zip の 1 コマンドの起動', () => {
  test('PATH=/usr/bin:/bin だけで展開した ./start.sh が miniserve を 127.0.0.1 と site で起動する', () => {
    const a = build(makeSite('{"v":"A"}\n')).zip;
    const top = unzipTo(update(a, makeSite('{"v":"B"}\n')).zip);
    const rec = path.join(mk('rec'), 'args.txt');
    fs.writeFileSync(path.join(top, 'miniserve'), `#!/bin/sh\nprintf '%s\\n' "$@" > "${rec}"\n`, { mode: 0o755 });
    const r = spawnSync('env', ['-i', 'PATH=/usr/bin:/bin', `HOME=${tmp}`, 'FANDHE_NO_OPEN=1', 'PORT=8798', './start.sh'], {
      cwd: top,
      encoding: 'utf8',
    });
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /http:\/\/127\.0\.0\.1:8798\//);
    assert.deepEqual(fs.readFileSync(rec, 'utf8').trim().split('\n'), ['--index', 'index.html', '-i', '127.0.0.1', '-p', '8798', 'site']);
    assert.equal(read(path.join(top, 'site', 'ir', 'main.json')).toString(), '{"v":"B"}\n');
  });
});

describe('fail-closed', () => {
  const noZip = (out) => assert.deepEqual(fs.readdirSync(out), []);
  const expectFail = (zip, site, re) => {
    const out = mk('out');
    assert.throws(() => updateL2({ zip, site, out, pins: PINS }), re);
    noZip(out);
  };

  test('旧 zip の miniserve が固定表と一致しない', () => {
    const entries = GOOD();
    entries[2] = { name: `${TOP}/miniserve`, data: Buffer.from('#!/bin/sh\nexit 1\n'), mode: 0o755 };
    expectFail(synth(entries), makeSite(), /sha256/);
  });

  const badZips = {
    '..（親への参照）を含む名前': [{ name: `${TOP}/../evil`, data: Buffer.from('x'), mode: 0o644 }, /unsafe path/],
    '絶対パス': [{ name: '/etc/evil', data: Buffer.from('x'), mode: 0o644 }, /outside/],
    'TOP の外': [{ name: 'other/evil', data: Buffer.from('x'), mode: 0o644 }, /outside/],
    'バックスラッシュ': [{ name: `${TOP}\\evil`, data: Buffer.from('x'), mode: 0o644 }, /unsupported entry name/],
    '非 ASCII の名前': [{ name: `${TOP}/é`, data: Buffer.from('x'), mode: 0o644 }, /./],
    '許さない mode': [{ name: `${TOP}/x`, data: Buffer.from('x'), mode: 0o777 }, /unsupported mode/],
  };
  for (const [label, [extra, re]] of Object.entries(badZips)) {
    test(`旧 zip が不正: ${label}`, () => {
      let zip;
      try {
        zip = synth([...GOOD(), extra]);
      } catch {
        return; // 非 ASCII は writeZip の段階で拒否される
      }
      expectFail(zip, makeSite(), re);
    });
  }

  test('旧 zip が不正: 重複した名前', () => {
    expectFail(synth([...GOOD(), { name: `${TOP}/start.sh`, data: Buffer.from('x'), mode: 0o755 }]), makeSite(), /duplicate/);
  });

  for (const missing of ['start.sh', 'miniserve', 'site/']) {
    test(`旧 zip が不正: 必須エントリ ${missing} が無い`, () => {
      expectFail(synth(GOOD().filter((e) => e.name !== `${TOP}/${missing}`)), makeSite(), /required entry/);
    });
  }

  const swap = (name, over) => GOOD().map((e) => (e.name === `${TOP}/${name}` ? { ...e, ...over } : e));
  test('旧 zip が不正: start.sh が 0644', () => {
    expectFail(synth(swap('start.sh', { mode: 0o644 })), makeSite(), /executable regular file/);
  });
  test('旧 zip が不正: miniserve が 0644', () => {
    expectFail(synth(swap('miniserve', { mode: 0o644 })), makeSite(), /executable regular file/);
  });
  test('旧 zip が不正: start.sh がディレクトリ', () => {
    expectFail(synth(swap('start.sh', { dir: true, data: undefined })), makeSite(), /executable regular file|required entry/);
  });

  test('旧 zip が不正: 壊れた zip（途中で切ったもの）', () => {
    const a = build(makeSite()).zip;
    const cut = path.join(mk('cut'), 'cut.zip');
    fs.writeFileSync(cut, read(a).subarray(0, 60));
    expectFail(cut, makeSite(), /unzip failed/);
  });

  test('旧 zip が不正: シンボリックリンクのエントリ', () => {
    const z = synth(GOOD());
    // writeZip はリンクを書けないため、central directory の外部属性を symlink（0o120777）に書き換える。
    const buf = read(z);
    let i = buf.indexOf(Buffer.from([0x50, 0x4b, 0x01, 0x02]));
    const nameOf = (p) => buf.subarray(p + 46, p + 46 + buf.readUInt16LE(p + 28)).toString();
    while (i >= 0 && nameOf(i) !== `${TOP}/start.sh`) i = buf.indexOf(Buffer.from([0x50, 0x4b, 0x01, 0x02]), i + 4);
    assert.ok(i >= 0);
    buf.writeUInt32LE((0o120777 << 16) >>> 0, i + 38);
    fs.writeFileSync(z, buf);
    expectFail(z, makeSite(), /symlink/);
  });

  test('展開後の大きさの上限を超える旧 zip', () => {
    // 宣言された展開後の大きさだけを書き換える（実データは作らない）。
    const z = synth(GOOD());
    const buf = read(z);
    const sig = Buffer.from([0x50, 0x4b, 0x01, 0x02]);
    let i = buf.indexOf(sig);
    while (i >= 0 && buf.subarray(i + 46, i + 46 + buf.readUInt16LE(i + 28)).toString() !== `${TOP}/miniserve`) i = buf.indexOf(sig, i + 4);
    buf.writeUInt32LE(0xf0000000, i + 24);
    fs.writeFileSync(z, buf);
    expectFail(z, makeSite(), /too large|unzip failed/);
  });

  test('申告の大きさを小さく偽った旧 zip でも、実際に出たバイト数が上限を超えれば拒否する', () => {
    const entries = [...GOOD(), { name: `${TOP}/README`, data: Buffer.alloc(3 * 1024 * 1024), mode: 0o644 }];
    const z = synth(entries);
    const buf = read(z);
    const sig = Buffer.from([0x50, 0x4b, 0x01, 0x02]);
    let i = buf.indexOf(sig);
    while (i >= 0 && buf.subarray(i + 46, i + 46 + buf.readUInt16LE(i + 28)).toString() !== `${TOP}/README`) i = buf.indexOf(sig, i + 4);
    assert.ok(i >= 0);
    buf.writeUInt32LE(1, i + 24); // 申告だけを 1 byte に偽る
    fs.writeFileSync(z, buf);
    assert.throws(() => update(z, makeSite(), { maxUnpacked: 1024 * 1024 }), /too large when unpacked \(actual/);
  });

  const badSites = {
    'ir/main.json が JSON でない': (s) => fs.writeFileSync(path.join(s, 'ir', 'main.json'), 'not json'),
    'test-hooks.js を含む': (s) => fs.writeFileSync(path.join(s, 'test-hooks.js'), ''),
    'シンボリックリンク': (s) => fs.symlinkSync('app.css', path.join(s, 'link.css')),
  };
  for (const [label, mutate] of Object.entries(badSites)) {
    test(`新しい配信物が不正: ${label}`, () => {
      const old = build(makeSite()).zip;
      const site = makeSite();
      mutate(site);
      expectFail(old, site, /error|check-delivery|main\.json|symlink/i);
    });
  }

  test('失敗しても既存の出力は変わらない', () => {
    const a = build(makeSite('{"v":"A"}\n'));
    const before = read(a.zip);
    const bad = makeSite();
    fs.writeFileSync(path.join(bad, 'ir', 'main.json'), 'not json');
    assert.throws(() => updateL2({ zip: a.zip, site: bad, out: path.dirname(a.zip), pins: PINS }), /main\.json/);
    assert.ok(read(a.zip).equals(before));
    assert.deepEqual(fs.readdirSync(path.dirname(a.zip)), [path.basename(a.zip)]);
  });

  test('出力先の対象名がディレクトリまたはリンクなら拒否', () => {
    const old = build(makeSite()).zip;
    const name = `${TOP}-${PLATFORM}.zip`;
    const o1 = mk('out');
    fs.mkdirSync(path.join(o1, name));
    assert.throws(() => updateL2({ zip: old, site: makeSite(), out: o1, pins: PINS }), /not a regular file/);
    const o2 = mk('out');
    fs.symlinkSync(path.join(o2, 'nowhere'), path.join(o2, name));
    assert.throws(() => updateL2({ zip: old, site: makeSite(), out: o2, pins: PINS }), /not a regular file/);
  });

  test('L2_ZIP が未設定・不在・ディレクトリは終了コード 2（関数）', () => {
    const out = mk('out');
    assert.throws(() => updateL2({ zip: undefined, site: makeSite(), out, pins: PINS }), (e) => e.code === 2);
    assert.throws(() => updateL2({ zip: path.join(tmp, 'nope.zip'), site: makeSite(), out, pins: PINS }), (e) => e.code === 2);
    assert.throws(() => updateL2({ zip: mk('d'), site: makeSite(), out, pins: PINS }), (e) => e.code === 2);
    noZip(out);
  });
});

describe('Windows 向けの zip（#1932。DLV-22）', () => {
  const FAKE_WIN = 'MZ fake windows server\n';
  const WPINS = { [PLATFORMS['windows-x64']]: sha(FAKE_WIN) };
  const BOTH = { ...PINS, ...WPINS };
  const buildWin = (site) => {
    const f = path.join(mk('wsrv'), 'miniserve.exe');
    fs.writeFileSync(f, FAKE_WIN, { mode: 0o755 });
    return buildL2({ server: f, site, out: mk('out'), platform: 'windows-x64', pins: WPINS });
  };
  const updWin = (zip, site, pins = BOTH) => updateL2({ zip, site, out: mk('out'), pins });
  const GOOD_WIN = () => [
    { name: `${TOP}/`, dir: true },
    { name: `${TOP}/start.cmd`, data: Buffer.from('@echo off\r\n'), mode: 0o644 },
    { name: `${TOP}/miniserve.exe`, data: Buffer.from(FAKE_WIN), mode: 0o644 },
    { name: `${TOP}/site/`, dir: true },
  ];
  const noOut = (zip, site, re, pins = BOTH) => {
    const out = mk('out');
    assert.throws(() => updateL2({ zip, site, out, pins }), re);
    assert.deepEqual(fs.readdirSync(out), []);
  };

  test('site/ 以外は旧 zip のバイト列と mode のまま、start.cmd は CRLF のまま、site/ は差し替わる', () => {
    const oldZip = buildWin(makeSite('{"v":"A"}\n', { 'only-in-a.txt': 'old\n' })).zip;
    const siteB = makeSite('{"v":"B"}\n');
    const r = updWin(oldZip, siteB);
    assert.equal(r.platform, 'windows-x64');
    assert.match(path.basename(r.zip), /-windows-x64\.zip$/);
    const a = treeOf(unzipTo(oldZip));
    const b = treeOf(unzipTo(r.zip));
    for (const k of Object.keys(a).filter((k) => !k.startsWith('site/'))) assert.deepEqual(b[k], a[k], k);
    for (const k of ['start.cmd', 'miniserve.exe', 'LICENSE-miniserve.txt', 'THIRD-PARTY-NOTICES.txt']) assert.ok(b[k], k);
    const cmd = Buffer.from(b['start.cmd'].data, 'base64').toString('latin1');
    assert.ok(cmd.includes('\r\n'));
    assert.equal(cmd.replace(/\r\n/g, '').includes('\n'), false);
    assert.equal(b['site/only-in-a.txt'], undefined);
    assert.equal(b['site/ir/main.json'].data, Buffer.from('{"v":"B"}\n').toString('base64'));
  });

  test('build との等価性・冪等性・決定性・続けての更新', () => {
    const a = makeSite('{"v":"A"}\n');
    const b = makeSite('{"v":"B"}\n');
    const zipA = buildWin(a).zip;
    assert.deepEqual(read(updWin(zipA, b).zip), read(buildWin(b).zip));
    assert.deepEqual(read(updWin(zipA, a).zip), read(zipA));
    assert.deepEqual(read(updWin(zipA, b).zip), read(updWin(zipA, b).zip));
    const c = makeSite('{"v":"C"}\n');
    assert.deepEqual(read(updWin(updWin(zipA, b).zip, c).zip), read(buildWin(c).zip));
  });

  test('mode 644 の start.cmd・miniserve.exe を受け付け、そのまま引き継ぐ', () => {
    const r = updWin(synth(GOOD_WIN()), makeSite());
    const b = treeOf(unzipTo(r.zip));
    assert.equal(b['start.cmd'].mode, 0o644);
    assert.equal(b['miniserve.exe'].mode, 0o644);
  });

  test('固定表に両 platform があっても、zip の配置で platform が決まる', () => {
    assert.equal(update(build(makeSite()).zip, makeSite(), { pins: BOTH }).platform, 'linux-x64');
    assert.equal(updWin(buildWin(makeSite()).zip, makeSite()).platform, 'windows-x64');
  });

  const dropped = { 'start.cmd': 1, 'miniserve.exe': 2, 'site/': 3 };
  for (const [name, idx] of Object.entries(dropped)) {
    test(`必須エントリ ${name} が無い zip を拒否する`, () => {
      const e = GOOD_WIN();
      e.splice(idx, 1);
      noOut(synth(e), makeSite(), /required entry/);
    });
  }

  test('両方の実行ファイルがある・起動スクリプトが混在する配置を拒否する', () => {
    noOut(synth([...GOOD_WIN(), { name: `${TOP}/miniserve`, data: Buffer.from(FAKE_SERVER), mode: 0o755 }]), makeSite(), /more than one layout/);
    noOut(synth([...GOOD_WIN(), { name: `${TOP}/start.sh`, data: Buffer.from('#!/bin/sh\n'), mode: 0o755 }]), makeSite(), /more than one layout/);
    const e = GOOD();
    e.push({ name: `${TOP}/start.cmd`, data: Buffer.from('x'), mode: 0o644 });
    noOut(synth(e), makeSite(), /more than one layout/);
    const f = GOOD_WIN();
    f[1] = { name: `${TOP}/start.sh`, data: Buffer.from('#!/bin/sh\n'), mode: 0o755 };
    noOut(synth(f), makeSite(), /required entry/);
  });

  test('名前と固定値の食い違いを拒否する', () => {
    const e = GOOD_WIN();
    e[2] = { name: `${TOP}/miniserve.exe`, data: Buffer.from(FAKE_SERVER), mode: 0o755 };
    noOut(synth(e), makeSite(), /sha256/);
    const f = GOOD();
    f[2] = { name: `${TOP}/miniserve`, data: Buffer.from(FAKE_WIN), mode: 0o755 };
    noOut(synth(f), makeSite(), /sha256/);
  });

  test('start.cmd がディレクトリなら拒否する', () => {
    const e = GOOD_WIN();
    e[1] = { name: `${TOP}/start.cmd`, dir: true };
    noOut(synth(e), makeSite(), /./);
  });

  test('Windows で展開できない名前の新しい配信物は Windows の zip では拒否し、Linux の zip では通る', () => {
    noOut(synth(GOOD_WIN()), makeSite('{"nodes":[]}\n', { 'con.txt': 'x' }), /cannot be extracted on Windows/);
    assert.equal(update(build(makeSite()).zip, makeSite('{"nodes":[]}\n', { 'con.txt': 'x' })).platform, PLATFORM);
  });

  test('対照: mode 644 の start.sh・miniserve は従来どおり拒否する（Windows の緩和が Unix の組に漏れない）', () => {
    const e = GOOD();
    e[1] = { name: `${TOP}/start.sh`, data: Buffer.from('#!/bin/sh\n'), mode: 0o644 };
    noOut(synth(e), makeSite(), /mode 755/, PINS);
    const f = GOOD();
    f[2] = { name: `${TOP}/miniserve`, data: Buffer.from(FAKE_SERVER), mode: 0o644 };
    noOut(synth(f), makeSite(), /mode 755/, PINS);
  });
});

describe('CLI', () => {
  const run = (env, args = []) =>
    spawnSync(process.execPath, [SCRIPT, ...args], { env: { PATH: process.env.PATH, ...env }, encoding: 'utf8' });

  test('誤用は終了コード 2（位置引数あり・L2_ZIP 未設定・不在）', () => {
    assert.equal(run({}, ['extra']).status, 2);
    assert.equal(run({ L2_SITE: makeSite(), L2_OUT: mk('out') }).status, 2);
    assert.equal(run({ L2_ZIP: path.join(tmp, 'nope.zip'), L2_SITE: makeSite(), L2_OUT: mk('out') }).status, 2);
  });

  test('合成の miniserve は固定表と一致しないため終了コード 1 で何も書かない（固定値を上書きする口が無い）', () => {
    const old = build(makeSite()).zip;
    const out = mk('out');
    // L2_PINS 等で固定表を差し替えようとしても効かない。
    const r = run({ L2_ZIP: old, L2_SITE: makeSite(), L2_OUT: out, L2_SERVER: makeServer() });
    assert.equal(r.status, 1);
    assert.match(r.stderr, /sha256/);
    assert.deepEqual(fs.readdirSync(out), []);
  });
});
