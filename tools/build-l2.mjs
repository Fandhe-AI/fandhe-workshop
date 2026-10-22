#!/usr/bin/env node
// L2 の配布物（zip）の作成スクリプト（TASK-112 1/2。D-58・D-71・DLV-13・DLV-18。`make build-l2` から呼ばれる）。
//
// 目的: 受講者が Rust なしに教材を開くための zip を作る。形は miniserve（上流のビルド済み実行ファイル）と静的ファイル:
//   fandhe-workshop-l2/{start.sh, miniserve, site/}。site/ は配信する設定の配信物（tools/build-web.sh の出力）そのもの。
//   受講者の操作は「展開・./start.sh・URL を開く」の 3 つ。
// 入力（環境変数。位置引数は誤用）:
//   L2_SERVER   必須。miniserve の実行ファイル。sha256 を tools/l2/miniserve.sha256 の固定値と照合する（A08）
//   L2_SITE     既定 <repo>/dist/web。tools/check-delivery.sh を通ること（試験用の口・.d.ts・シンボリックリンクが無い）
//   L2_OUT      既定 <repo>/dist/l2。書くのは fandhe-workshop-l2-<platform>.zip の 1 ファイルだけ
//   L2_PLATFORM macos-arm64（既定）または linux-x64（模擬環境の試験 #532 用）
// 固定値を上書きする口は作らない。どの段階で失敗しても zip を作らない（fail-closed）。出力は決定的（同じ入力から同じバイト列）。
// 終了コード: 0 = 作成 / 1 = 検査の失敗 / 2 = 誤用・入力の不備。
// 回帰テストは tools/build-l2.test.mjs（`make test-build-l2`）。
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
export const TOP = 'fandhe-workshop-l2';
export const PLATFORMS = {
  'macos-arm64': 'miniserve-0.35.0-aarch64-apple-darwin',
  'linux-x64': 'miniserve-0.35.0-x86_64-unknown-linux-musl',
};
const MAX = 0xffffffff;
// DOS 時刻 1980-01-01 00:00:00。決定性のため固定する。
const DOS_TIME = 0;
const DOS_DATE = (0 << 9) | (1 << 5) | 1;

/** 誤用・入力の不備（終了コード 2）と検査の失敗（1）を区別する例外。 */
export class L2Error extends Error {
  constructor(message, code = 1) {
    super(message);
    this.code = code;
  }
}

/** sha256sum 互換の固定表を読む。{ 資産名: 64 桁の 16 進 } を返す。 */
export function readPins(file) {
  const pins = {};
  for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
    if (line === '') continue;
    const m = /^([0-9a-f]{64}) {2}(miniserve-[0-9.]+-[a-z0-9_-]+)$/.exec(line);
    if (!m) throw new L2Error(`malformed pin line in ${file}`, 2);
    pins[m[2]] = m[1];
  }
  return pins;
}

/** 配信物を再帰で辿り、通常のファイルとディレクトリだけを集める。リンク・特殊ファイル・非 ASCII 名は拒否（A01）。 */
export function collectSite(siteDir) {
  const out = [];
  const walk = (abs, rel) => {
    for (const ent of fs.readdirSync(abs, { withFileTypes: true })) {
      const name = ent.name;
      if (name === '..' || name === '.' || !/^[\x21-\x7e]+$/.test(name)) {
        throw new L2Error(`unsupported file name under site: ${rel}${name}`);
      }
      const a = path.join(abs, name);
      const r = `${rel}${name}`;
      if (ent.isSymbolicLink()) throw new L2Error(`symlink is not allowed: ${r}`);
      if (ent.isDirectory()) {
        out.push({ name: `${r}/`, dir: true });
        walk(a, `${r}/`);
      } else if (ent.isFile()) {
        out.push({ name: r, file: a, mode: 0o644 });
      } else {
        throw new L2Error(`special file is not allowed: ${r}`);
      }
    }
  };
  walk(siteDir, '');
  return out;
}

/** エントリ [{ name, dir?, data?, mode }] から決定的な zip（zip64 なし）を作る。名前の順に並べる。 */
export function writeZip(entries) {
  const sorted = [...entries].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  const parts = [];
  const central = [];
  let offset = 0;
  for (const e of sorted) {
    const name = Buffer.from(e.name, 'ascii');
    const data = e.dir ? Buffer.alloc(0) : e.data;
    const comp = e.dir ? Buffer.alloc(0) : zlib.deflateRawSync(data, { level: 9 });
    const method = e.dir ? 0 : 8;
    const crc = e.dir ? 0 : zlib.crc32(data);
    if (data.length > MAX || comp.length > MAX || offset > MAX) throw new L2Error('zip64 is not supported');
    const attr = ((e.dir ? 0o040755 : 0o100000 | e.mode) << 16) >>> 0;
    const lh = Buffer.alloc(30);
    lh.writeUInt32LE(0x04034b50, 0);
    lh.writeUInt16LE(20, 4);
    lh.writeUInt16LE(0, 6);
    lh.writeUInt16LE(method, 8);
    lh.writeUInt16LE(DOS_TIME, 10);
    lh.writeUInt16LE(DOS_DATE, 12);
    lh.writeUInt32LE(crc, 14);
    lh.writeUInt32LE(comp.length, 18);
    lh.writeUInt32LE(data.length, 22);
    lh.writeUInt16LE(name.length, 26);
    lh.writeUInt16LE(0, 28);
    const ch = Buffer.alloc(46);
    ch.writeUInt32LE(0x02014b50, 0);
    ch.writeUInt16LE((3 << 8) | 20, 4);
    ch.writeUInt16LE(20, 6);
    ch.writeUInt16LE(0, 8);
    ch.writeUInt16LE(method, 10);
    ch.writeUInt16LE(DOS_TIME, 12);
    ch.writeUInt16LE(DOS_DATE, 14);
    ch.writeUInt32LE(crc, 16);
    ch.writeUInt32LE(comp.length, 20);
    ch.writeUInt32LE(data.length, 24);
    ch.writeUInt16LE(name.length, 28);
    ch.writeUInt32LE(attr, 38);
    ch.writeUInt32LE(offset, 42);
    central.push(Buffer.concat([ch, name]));
    parts.push(lh, name, comp);
    offset += 30 + name.length + comp.length;
  }
  const cd = Buffer.concat(central);
  if (offset + cd.length > MAX || sorted.length > 0xffff) throw new L2Error('zip64 is not supported');
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(sorted.length, 8);
  eocd.writeUInt16LE(sorted.length, 10);
  eocd.writeUInt32LE(cd.length, 12);
  eocd.writeUInt32LE(offset, 16);
  return Buffer.concat([...parts, cd, eocd]);
}

function sha256(buf) {
  return createHash('sha256').update(buf).digest('hex');
}

/**
 * 配布物を作って出力先へ書く。pins は { 資産名: sha256 }（試験は注入し、CLI は固定表を読む）。
 * 戻り値は { zip, bytes, sha256 }。
 */
export function buildL2({ server, site, out, platform, pins, checkDelivery = true }) {
  const asset = PLATFORMS[platform];
  if (!asset) throw new L2Error(`unknown L2_PLATFORM: ${platform}`, 2);
  if (!server) throw new L2Error('L2_SERVER is required', 2);
  let st;
  try {
    st = fs.statSync(server);
  } catch {
    throw new L2Error(`L2_SERVER not found: ${server}`, 2);
  }
  if (!st.isFile()) throw new L2Error(`L2_SERVER is not a regular file: ${server}`, 2);
  if (!fs.existsSync(site) || !fs.statSync(site).isDirectory()) {
    throw new L2Error(`L2_SITE is not a directory: ${site}`, 2);
  }
  const bin = fs.readFileSync(server);
  if (pins[asset] === undefined || sha256(bin) !== pins[asset]) {
    throw new L2Error(`sha256 of L2_SERVER does not match the pinned value for ${asset}`);
  }
  if (checkDelivery) {
    const r = spawnSync('bash', [path.join(HERE, 'check-delivery.sh'), site], { encoding: 'utf8' });
    if (r.status !== 0) {
      throw new L2Error(`check-delivery failed (exit ${r.status}) for ${site}`);
    }
  }
  const ir = path.join(site, 'ir', 'main.json');
  let irStat;
  try {
    irStat = fs.lstatSync(ir);
    if (!irStat.isFile()) throw new Error('not a file');
    JSON.parse(fs.readFileSync(ir, 'utf8'));
  } catch {
    throw new L2Error('site/ir/main.json must be a regular file holding JSON');
  }
  const entries = [
    { name: `${TOP}/`, dir: true },
    { name: `${TOP}/start.sh`, data: fs.readFileSync(path.join(HERE, 'l2', 'start.sh')), mode: 0o755 },
    { name: `${TOP}/miniserve`, data: bin, mode: 0o755 },
    { name: `${TOP}/site/`, dir: true },
  ];
  for (const e of collectSite(site)) {
    entries.push(e.dir ? { name: `${TOP}/site/${e.name}`, dir: true } : { name: `${TOP}/site/${e.name}`, data: fs.readFileSync(e.file), mode: e.mode });
  }
  const zip = writeZip(entries);

  const outDir = path.resolve(out);
  if (!fs.existsSync(outDir)) {
    const distDir = path.join(ROOT, 'dist');
    if (outDir !== distDir && !outDir.startsWith(distDir + path.sep)) {
      throw new L2Error(`L2_OUT does not exist and is not under dist/: ${outDir}`, 2);
    }
    fs.mkdirSync(outDir, { recursive: true });
  } else if (!fs.statSync(outDir).isDirectory()) {
    throw new L2Error(`L2_OUT is not a directory: ${outDir}`, 2);
  }
  const target = path.join(outDir, `${TOP}-${platform}.zip`);
  try {
    if (!fs.lstatSync(target).isFile()) throw new L2Error(`output target is not a regular file: ${target}`);
  } catch (e) {
    if (e instanceof L2Error) throw e;
    // 対象が無いのは正常（新規作成）
  }
  const tmp = `${target}.tmp-${process.pid}`;
  try {
    fs.writeFileSync(tmp, zip, { flag: 'wx' });
    fs.renameSync(tmp, target);
  } catch (e) {
    fs.rmSync(tmp, { force: true });
    throw e;
  }
  return { zip: target, bytes: zip.length, sha256: sha256(zip) };
}

function main() {
  if (process.argv.length > 2) {
    console.error('usage: build-l2.mjs (no arguments; configure with L2_SERVER, L2_SITE, L2_OUT, L2_PLATFORM)');
    return 2;
  }
  const env = process.env;
  try {
    const platform = env.L2_PLATFORM || 'macos-arm64';
    const r = buildL2({
      server: env.L2_SERVER,
      site: env.L2_SITE || path.join(ROOT, 'dist', 'web'),
      out: env.L2_OUT || path.join(ROOT, 'dist', 'l2'),
      platform,
      pins: readPins(path.join(HERE, 'l2', 'miniserve.sha256')),
    });
    console.error(`built: ${r.zip}\nbytes: ${r.bytes}\nsha256: ${r.sha256}\nplatform: ${platform}`);
    return 0;
  } catch (e) {
    if (e instanceof L2Error) {
      console.error(`error: ${e.message}`);
      return e.code;
    }
    console.error(`error: ${e && e.message ? e.message : e}`);
    return 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main();
}
