#!/usr/bin/env node
// L2 の配布物（zip）の更新スクリプト（TASK-116。D-58・D-71・DLV-18。`make update-l2` から呼ばれる）。
//
// 目的: 教材を更新したとき、既存の配布物の site/ だけを新しい配信物に差し替えた新しい zip を作る。
//   案 (ii)（miniserve ＋静的ファイル）を採った利点である「更新は site/ の差し替え 1 操作」の実体。
//   site/ 以外（start.sh・miniserve・将来の README・ライセンス文）は旧 zip のバイト列と mode をそのまま引き継ぐ。
//   サーバの実行ファイルの取得（L2_SERVER）・Rust・通信は要らない。手順は docs/l2-update-procedure.md。
// 入力（環境変数。位置引数は誤用）:
//   L2_ZIP   必須。既存の配布物（build-l2.mjs か update-l2.mjs の出力）
//   L2_SITE  既定 <repo>/dist/web。build-l2.mjs と同じ検査（check-delivery.sh・ir/main.json）を通ること
//   L2_OUT   既定 <repo>/dist/l2。規則は build-l2.mjs と同じ。書くのは fandhe-workshop-l2-<platform>.zip の 1 ファイルだけ
//   platform は旧 zip の miniserve の sha256 を tools/l2/miniserve.sha256 の固定値と照合して決める（一致しなければ失敗。A08）。
// 旧 zip は信頼しない入力として扱う（A01・A04）: 展開の前に名前を検査し、リンクを拒否し、読み出した実バイト数に上限を置く（申告値は信用しない）。
// どの段階で失敗しても zip を作らない（fail-closed）。出力は決定的（同じ入力から同じバイト列。build-l2.mjs と同じ writer）。
// 終了コード: 0 = 作成 / 1 = 検査の失敗 / 2 = 誤用・入力の不備。
// 回帰テストは tools/update-l2.test.mjs（`make test-update-l2`）。
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkSite, L2Error, platformOfServer, readPins, siteEntries, TOP, writeOutput, writeZip } from './build-l2.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
const UNZIP = '/usr/bin/unzip';
// 展開後の合計の大きさの上限（zip bomb の対策）。配布物は miniserve（約 5MB）と WASM を含む数十 MB 以内に収まる。
export const MAX_UNPACKED = 512 * 1024 * 1024;
const MODES = { 'rwxr-xr-x': 0o755, 'rw-r--r--': 0o644 };

function unzip(args) {
  const r = spawnSync(UNZIP, args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  if (r.error) throw new L2Error(`unzip could not run: ${r.error.message}`);
  if (r.status !== 0) throw new L2Error(`unzip failed (exit ${r.status ?? r.signal}): ${(r.stderr ?? '').trim().slice(0, 1000)}`);
  return r.stdout;
}

/** 旧 zip の名前の一覧を展開の前に検査し（許す名前は build-l2 の collectSite と同じ集合。パターン文字は unzip へ渡す前にエスケープする）、[{ name, dir, mode }] を返す。 */
function listEntries(zip) {
  const names = unzip(['-Z1', zip]).split('\n').filter(Boolean);
  const seen = new Set();
  for (const n of names) {
    if (!/^[\x21-\x7e]+$/.test(n) || n.includes('\\')) throw new L2Error(`unsupported entry name in zip: ${JSON.stringify(n)}`);
    const comps = n.split('/');
    if (!n.startsWith(`${TOP}/`) || comps.some((c, i) => c === '..' || c === '.' || (c === '' && i !== comps.length - 1))) {
      throw new L2Error(`entry outside ${TOP}/ or with an unsafe path: ${n}`);
    }
    if (seen.has(n)) throw new L2Error(`duplicate entry in zip: ${n}`);
    seen.add(n);
  }
  const entries = [];
  let total = 0;
  for (const line of unzip(['-Z', zip]).split('\n')) {
    const m = /^([-dl])(\S{9})\s+\S+\s+\S+\s+(\d+)\s+\S+\s+\S+\s+\S+\s+\S+\s+(\S+)$/.exec(line);
    if (!m) continue;
    const [, kind, perm, size, name] = m;
    if (kind === 'l') throw new L2Error(`symlink is not allowed in zip: ${name}`);
    const dir = kind === 'd';
    if (!dir && MODES[perm] === undefined) throw new L2Error(`unsupported mode ${perm} in zip: ${name}`);
    total += Number(size);
    entries.push({ name, dir, mode: dir ? 0o755 : MODES[perm] });
  }
  if (entries.length !== names.length || entries.some((e) => !seen.has(e.name))) {
    throw new L2Error('zip listing could not be parsed consistently');
  }
  if (total > MAX_UNPACKED) throw new L2Error(`zip is too large when unpacked: ${total} bytes (limit ${MAX_UNPACKED})`);
  for (const need of [`${TOP}/start.sh`, `${TOP}/miniserve`, `${TOP}/site/`]) {
    if (!seen.has(need)) throw new L2Error(`zip lacks the required entry: ${need}`);
  }
  // 名前の存在だけでなく種類と権限も確かめる（0644 の旧 zip は受講者の ./start.sh や miniserve 起動が Permission denied になる）。
  for (const exe of [`${TOP}/start.sh`, `${TOP}/miniserve`]) {
    const e = entries.find((x) => x.name === exe);
    if (!e || e.dir || e.mode !== 0o755) throw new L2Error(`required entry must be an executable regular file (mode 755): ${exe}`);
  }
  const site = entries.find((x) => x.name === `${TOP}/site/`);
  if (!site || !site.dir) throw new L2Error(`required entry must be a directory: ${TOP}/site/`);
  return entries;
}

/**
 * site/ 以外のファイルを、1 件ずつ unzip -p でメモリへ読む。申告値は改変できるため、上限は実際に出たバイト数で判定する。
 * spawnSync の maxBuffer は残りの予算とし、超えた時点で unzip を打ち切る（ディスクへは何も書かない）。
 */
function readKept(zip, entries, limit) {
  let left = limit;
  const out = new Map();
  for (const e of entries) {
    if (e.dir) continue;
    const r = spawnSync(UNZIP, ['-p', zip, e.name.replace(/[\\*?[\]]/g, '\\$&')], { maxBuffer: left + 1 });
    if (r.error) {
      if (r.error.code === 'ENOBUFS') throw new L2Error(`zip is too large when unpacked (actual bytes exceed the limit ${limit})`);
      throw new L2Error(`unzip could not run: ${r.error.message}`);
    }
    if (r.status !== 0) throw new L2Error(`unzip failed (exit ${r.status ?? r.signal}): ${String(r.stderr ?? '').trim().slice(0, 1000)}`);
    left -= r.stdout.length;
    if (left < 0) throw new L2Error(`zip is too large when unpacked (actual bytes exceed the limit ${limit})`);
    out.set(e.name, r.stdout);
  }
  return out;
}

/**
 * 旧 zip の site/ を site の内容に差し替えた新しい zip を out へ書く。pins は { 資産名: sha256 }。
 * 戻り値は { zip, bytes, sha256, platform, kept, files }。
 */
export function updateL2({ zip, site, out, pins, checkDelivery = true, maxUnpacked = MAX_UNPACKED }) {
  if (!zip) throw new L2Error('L2_ZIP is required', 2);
  let st;
  try {
    st = fs.lstatSync(zip);
  } catch {
    throw new L2Error(`L2_ZIP not found: ${zip}`, 2);
  }
  if (!st.isFile()) throw new L2Error(`L2_ZIP is not a regular file: ${zip}`, 2);
  const entries = listEntries(zip);
  {
    const kept = entries.filter((e) => !e.name.startsWith(`${TOP}/site/`));
    const data = readKept(zip, kept, maxUnpacked);
    const bin = data.get(`${TOP}/miniserve`);
    const platform = platformOfServer(bin, pins);
    if (platform === null) throw new L2Error('sha256 of miniserve in L2_ZIP does not match any pinned value');
    checkSite(site, { checkDelivery });
    const next = kept.map((e) => (e.dir ? { name: e.name, dir: true } : { name: e.name, data: data.get(e.name), mode: e.mode }));
    const files = siteEntries(site);
    next.push({ name: `${TOP}/site/`, dir: true }, ...files);
    const r = writeOutput(out, platform, writeZip(next));
    return { ...r, platform, kept: kept.length, files: files.filter((f) => !f.dir).length };
  }
}

function main() {
  if (process.argv.length > 2) {
    console.error('usage: update-l2.mjs (no arguments; configure with L2_ZIP, L2_SITE, L2_OUT)');
    return 2;
  }
  const env = process.env;
  try {
    const r = updateL2({
      zip: env.L2_ZIP,
      site: env.L2_SITE || path.join(ROOT, 'dist', 'web'),
      out: env.L2_OUT || path.join(ROOT, 'dist', 'l2'),
      pins: readPins(path.join(HERE, 'l2', 'miniserve.sha256')),
    });
    console.error(`updated: ${r.zip}\nbytes: ${r.bytes}\nsha256: ${r.sha256}\nplatform: ${r.platform}\nkept entries: ${r.kept}\nsite files: ${r.files}`);
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
