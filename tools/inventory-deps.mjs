#!/usr/bin/env node
// 依存の棚卸し（BUILD-25・D-58。TASK-99。`make inventory-deps`）。
//
// 目的: 初回公開の前とリリースごとに、`Cargo.lock` の依存が「取り下げ済み」でないか、「所有者が 1 人」でないかを
// 日付つきで記録する。TASK-18 の決定 D-58 は、対象を直接の依存と、取り下げ済みの crate・所有者が 1 人の crate と定めた。
// 手順と結果の記録の置き場は docs/dependency-inventory.md。
//
// データ源（既定は次の 2 つのホストだけ。認証情報は送らない）:
//   - sparse index `https://index.crates.io/<prefix>/<name>`: 版ごとの `yanked` と `cksum`（cargo と同じ取得先）
//   - crates.io API `/crates/<name>/owners`・`/crates/<name>`: 所有者の数と最終更新日（参考の列。合否に使わない）
//
// 判定:
//   - 「所有者 1 人」は user がちょうど 1 で team が 0 のこと。team を持つ crate は数えない
//   - 所有者 1 人は D-58 の棚卸しの対象で、注記と要約に記録する。終了コードには影響させない（実測で 66 件中 32 件が該当し、
//     失敗にすると常に 1 になって信号にならないため）。依存を変えるかの判断は人間（TASK-18）が記録を見て行う
//   - 決定で認めた例外は ACKNOWLEDGED_SINGLE_OWNER に理由つきで持つ（維持を決めた crate の印）。増やすときは決定（D-n）を根拠に書く
//   - 例外が Cargo.lock に無い名前を指していたら古い例外として要確認にする
//
// 終了コード:
//   0 = 完了。取り下げ済み・registry に無い版・crates.io 以外の source・古い例外のいずれも無い
//   1 = 完了したが要確認がある（TASK-18 に戻す材料。上記のいずれか）
//   2 = 誤用、または棚卸しを完了できない（通信の失敗・5xx・タイムアウト・壊れた JSON・Cargo.lock の形が想定外・
//       checksum の不一致・crate 名が不正）。fail-closed とし、標準出力は空にする
//
// 出力: 標準出力は結果だけ（既定は Markdown。`--format json` で JSON）。進行状況は標準エラー。
// 所有者の login 名などの個人に関わる値は出さず、数だけを出す。
// 使い方: node tools/inventory-deps.mjs [--lock <path>] [--format md|json] [--delay-ms <n>] [--timeout-ms <n>]
//         [--index-base <url>] [--api-base <url>]
//   --index-base・--api-base は回帰テスト用。既定の 2 ホストか loopback（http://127.0.0.1:<port>）だけを受け付ける（SSRF 対策）。
// 回帰テスト: tools/inventory-deps.test.mjs（scripts/test-inventory-deps.sh）。実 crates.io には通信しない。
// 通信が要り、数分かかるため、CI・make check・lefthook には組み込まない（失敗したら止めるガードにする変更は人間の承認点）。
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const DEFAULT_INDEX_BASE = 'https://index.crates.io';
export const DEFAULT_API_BASE = 'https://crates.io/api/v1';
const REGISTRY_SOURCE = 'registry+https://github.com/rust-lang/crates.io-index';
const USER_AGENT = 'fandhe-workshop-inventory-deps (https://github.com/Fandhe-AI/fandhe-workshop)';
const MAX_BODY_BYTES = 16 * 1024 * 1024; // web-sys の index が 4 MB を超えるため
const NAME_RE = /^[A-Za-z0-9_-]{1,64}$/;
const LOOPBACK_RE = /^http:\/\/127\.0\.0\.1:\d{1,5}(\/[A-Za-z0-9_/-]*[A-Za-z0-9_-])?$/;

// 所有者 1 人を決定に基づいて認めた crate（名前 -> 理由）。理由のない例外は置かない。
export const ACKNOWLEDGED_SINGLE_OWNER = new Map([
  ['yaml_serde', 'D-58 / PACK-47（維持して版を固定する）'],
]);

// 中断の種類。2 は棚卸しを完了できない失敗（fail-closed）。
export class InventoryError extends Error {}

// ---------- Cargo.lock のパーサ ----------

// 想定外の形は InventoryError にする。読むのは name・version・source・checksum・dependencies だけ。
export function parseLock(text) {
  const packages = [];
  let cur = null;
  let inDeps = false;
  const lines = text.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].replace(/\r$/, '');
    const at = `Cargo.lock:${i + 1}`;
    if (inDeps) {
      if (line === ']') { inDeps = false; continue; }
      const m = /^ "([^"\\]+)",?$/.exec(line);
      if (!m) throw new InventoryError(`unexpected dependency line at ${at}`);
      cur.dependencies.push(m[1]);
      continue;
    }
    if (line === '[[package]]') {
      cur = { name: null, version: null, source: null, checksum: null, dependencies: [] };
      packages.push(cur);
      continue;
    }
    const kv = /^([a-z]+) = (.*)$/.exec(line);
    if (!kv || cur === null) continue; // 先頭の version や [metadata] などは読まない
    const [, key, rawVal] = kv;
    if (key === 'dependencies') {
      if (rawVal === '[]') continue;
      if (rawVal !== '[') throw new InventoryError(`unexpected dependencies form at ${at}`);
      inDeps = true;
      continue;
    }
    if (key === 'name' || key === 'version' || key === 'source' || key === 'checksum') {
      const sm = /^"([^"\\]*)"$/.exec(rawVal);
      if (!sm) throw new InventoryError(`unexpected string value at ${at}`);
      cur[key] = sm[1];
    }
  }
  if (inDeps) throw new InventoryError('unterminated dependencies array in Cargo.lock');
  if (packages.length === 0) throw new InventoryError('no [[package]] found in Cargo.lock');
  for (const p of packages) {
    if (p.name === null || p.version === null) throw new InventoryError('package without name or version in Cargo.lock');
  }
  return packages;
}

// 直接の依存の名前の集合。workspace の member（source の無い package）の dependencies に現れる名前で、
// dev・target 別の依存も含む（D-58 の「直接の依存」に合わせるため）。
export function directDependencyNames(packages) {
  const members = packages.filter((p) => p.source === null);
  const memberNames = new Set(members.map((p) => p.name));
  const direct = new Set();
  for (const m of members) {
    for (const dep of m.dependencies) {
      const depName = dep.split(' ')[0]; // `name` か `name version` か `name version (source)`
      if (!memberNames.has(depName)) direct.add(depName);
    }
  }
  return direct;
}

// ---------- 取得 ----------

// sparse index の経路（cargo と同じ規則）。
export function indexPath(name) {
  const n = name.toLowerCase();
  if (n.length === 1) return `1/${n}`;
  if (n.length === 2) return `2/${n}`;
  if (n.length === 3) return `3/${n[0]}/${n}`;
  return `${n.slice(0, 2)}/${n.slice(2, 4)}/${n}`;
}

function assertName(name) {
  if (!NAME_RE.test(name)) throw new InventoryError(`invalid crate name: ${JSON.stringify(name).slice(0, 80)}`);
}

// 応答の本文を上限つきで読む。リダイレクトは追わない。404 は null、他の失敗は InventoryError。
async function fetchBody(fetchFn, url, timeoutMs) {
  let res;
  try {
    res = await fetchFn(url, {
      redirect: 'error',
      headers: { 'user-agent': USER_AGENT, accept: 'application/json, text/plain' },
      signal: AbortSignal.timeout(timeoutMs),
    });
  } catch (e) {
    throw new InventoryError(`request failed: ${new URL(url).host} (${e && e.name ? e.name : 'error'})`);
  }
  if (res.status === 404) { await res.body?.cancel?.(); return null; }
  if (!res.ok) { await res.body?.cancel?.(); throw new InventoryError(`unexpected status ${res.status} from ${new URL(url).host}`); }
  const chunks = [];
  let total = 0;
  try {
    for await (const chunk of res.body) {
      total += chunk.length;
      if (total > MAX_BODY_BYTES) throw new InventoryError(`response too large from ${new URL(url).host}`);
      chunks.push(chunk);
    }
  } catch (e) {
    if (e instanceof InventoryError) throw e;
    throw new InventoryError(`reading response failed: ${new URL(url).host} (${e && e.name ? e.name : 'error'})`);
  }
  return Buffer.concat(chunks).toString('utf8');
}

function parseJson(text, what) {
  try {
    return JSON.parse(text);
  } catch {
    throw new InventoryError(`malformed JSON from ${what}`);
  }
}

// index の 1 crate 分。見つからなければ null。行は 1 版 1 行の JSON。
async function fetchIndex(fetchFn, base, name, timeoutMs) {
  const text = await fetchBody(fetchFn, `${base}/${indexPath(name)}`, timeoutMs);
  if (text === null) return null;
  const entries = [];
  for (const line of text.split('\n')) {
    if (line.trim() === '') continue;
    const e = parseJson(line, 'index');
    if (typeof e !== 'object' || e === null || typeof e.vers !== 'string' || typeof e.cksum !== 'string' || typeof e.yanked !== 'boolean') {
      throw new InventoryError('unexpected index entry shape');
    }
    entries.push({ vers: e.vers, cksum: e.cksum, yanked: e.yanked });
  }
  return entries;
}

// API の所有者の数。個人の値（login 名など）は読み捨てて数だけを返す。
async function fetchOwners(fetchFn, base, name, timeoutMs) {
  const text = await fetchBody(fetchFn, `${base}/crates/${encodeURIComponent(name)}/owners`, timeoutMs);
  if (text === null) return null;
  const j = parseJson(text, 'api owners');
  if (typeof j !== 'object' || j === null || !Array.isArray(j.users)) throw new InventoryError('unexpected owners shape');
  let users = 0;
  let teams = 0;
  for (const u of j.users) {
    if (typeof u !== 'object' || u === null || (u.kind !== 'user' && u.kind !== 'team')) throw new InventoryError('unexpected owner entry shape');
    if (u.kind === 'user') users++; else teams++;
  }
  return { users, teams };
}

async function fetchUpdatedAt(fetchFn, base, name, timeoutMs) {
  const text = await fetchBody(fetchFn, `${base}/crates/${encodeURIComponent(name)}`, timeoutMs);
  if (text === null) return null;
  const j = parseJson(text, 'api crate');
  const at = j && j.crate && j.crate.updated_at;
  if (typeof at !== 'string') throw new InventoryError('unexpected crate shape');
  return at.slice(0, 10);
}

// build metadata（`+` 以降。例: 1.1.8+spec-1.1.0）は版の大小にも pre-release の判定にも使わない。
const stripBuild = (v) => v.split('+')[0];

function compareVersions(a, b) {
  a = stripBuild(a);
  b = stripBuild(b);
  const pa = a.split(/[.+-]/).map((x) => (/^\d+$/.test(x) ? Number(x) : -1));
  const pb = b.split(/[.+-]/).map((x) => (/^\d+$/.test(x) ? Number(x) : -1));
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

export function latestVersion(entries) {
  const stable = entries.filter((e) => !e.yanked && !stripBuild(e.vers).includes('-'));
  if (stable.length === 0) return null;
  return stable.map((e) => e.vers).sort(compareVersions).pop();
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------- 棚卸し ----------

// packages を調べて行と要確認を返す。通信は fetchFn を通す（試験から差し替えられる）。
export async function inventory({ packages, fetchFn, indexBase, apiBase, delayMs, timeoutMs, log = () => {} }) {
  const direct = directDependencyNames(packages);
  const lockNames = new Set(packages.map((p) => p.name));
  const rows = [];
  const flags = [];
  const registry = packages.filter((p) => p.source !== null);
  const perName = new Map(); // 同じ crate の API は 1 回だけ呼ぶ
  let apiCalls = 0;
  const throttle = async () => { if (apiCalls++ > 0 && delayMs > 0) await sleep(delayMs); };

  const sorted = [...registry].sort((a, b) => a.name.localeCompare(b.name) || compareVersions(a.version, b.version));
  let n = 0;
  for (const p of sorted) {
    n++;
    const row = {
      crate: p.name, version: p.version, relation: direct.has(p.name) ? 'direct' : 'transitive',
      owners: null, yanked: null, latest: null, updated: null, notes: [],
    };
    rows.push(row);
    if (p.source !== REGISTRY_SOURCE) {
      row.notes.push('crates.io 以外');
      flags.push(`${p.name} ${p.version}: crates.io 以外の source`);
      continue;
    }
    assertName(p.name);
    log(`[${n}/${sorted.length}] ${p.name} ${p.version}`);

    const entries = await fetchIndex(fetchFn, indexBase, p.name, timeoutMs);
    const hit = entries && entries.find((e) => e.vers === p.version);
    if (!hit) {
      row.notes.push('registry に版が無い');
      flags.push(`${p.name} ${p.version}: registry（index）に見つからない`);
    } else {
      if (p.checksum !== null && hit.cksum !== p.checksum) throw new InventoryError(`checksum mismatch for ${p.name} ${p.version}`);
      row.yanked = hit.yanked;
      row.latest = latestVersion(entries);
      if (hit.yanked) {
        row.notes.push('取り下げ済み');
        flags.push(`${p.name} ${p.version}: 取り下げ済み`);
      }
    }

    if (!perName.has(p.name)) {
      await throttle();
      const owners = await fetchOwners(fetchFn, apiBase, p.name, timeoutMs);
      await throttle();
      const updated = owners === null ? null : await fetchUpdatedAt(fetchFn, apiBase, p.name, timeoutMs);
      perName.set(p.name, { owners, updated });
    }
    const info = perName.get(p.name);
    row.updated = info.updated;
    if (info.owners === null) {
      row.notes.push('API に crate が無い');
      flags.push(`${p.name}: crates.io の API に見つからない`);
    } else {
      row.owners = info.owners;
      if (info.owners.users === 1 && info.owners.teams === 0) {
        const why = ACKNOWLEDGED_SINGLE_OWNER.get(p.name);
        if (why) {
          row.notes.push(`所有者 1 人（例外: ${why}）`);
        } else {
          // 実測では推移の依存の約半数が該当し、要確認にすると常に 1 になって信号にならない。記録して要約に出す
          row.notes.push('所有者 1 人');
        }
      }
    }
  }
  for (const [name] of ACKNOWLEDGED_SINGLE_OWNER) {
    if (!lockNames.has(name)) flags.push(`${name}: 例外に載っているが Cargo.lock に無い（古い例外）`);
  }
  return { rows, flags, directCount: direct.size };
}

// ---------- 出力 ----------

const escapeCell = (s) => String(s).replace(/\\/g, '\\\\').replace(/\|/g, '\\|').replace(/[\r\n]+/g, ' ');

export function summarize(result) {
  const { rows, flags, directCount } = result;
  const single = rows.filter((r) => r.notes.some((x) => x.startsWith('所有者 1 人')));
  return {
    registry_crates: rows.length,
    direct: rows.filter((r) => r.relation === 'direct').length,
    direct_names: directCount,
    yanked: rows.filter((r) => r.yanked === true).length,
    single_owner: single.length,
    single_owner_acknowledged: single.filter((r) => r.notes.some((x) => x.includes('例外:'))).length,
    single_owner_direct_unlisted: single.filter((r) => r.relation === 'direct' && !r.notes.some((x) => x.includes('例外:'))).map((r) => r.crate),
    flags,
  };
}

export function renderMarkdown(baseline, result) {
  const s = summarize(result);
  const out = [];
  out.push('### 基点', '');
  out.push('| 項目 | 内容 |', '|---|---|');
  out.push(`| 実行日（UTC） | ${baseline.date} |`);
  out.push(`| 元にしたコミット | ${escapeCell(baseline.commit)}${baseline.dirty ? '（作業ツリーに変更あり）' : ''} |`);
  out.push(`| Cargo.lock の SHA-256 | ${baseline.lock_sha256} |`);
  out.push(`| Node | ${escapeCell(baseline.node)} |`);
  out.push(`| 取得先 | ${escapeCell(baseline.hosts.join('、'))} |`, '');
  out.push('### 一覧', '');
  out.push('| crate | 版 | 直接/推移 | 所有者（user/team） | 取り下げ | 最新版 | 最終更新日 | 注記 |', '|---|---|---|---|---|---|---|---|');
  for (const r of result.rows) {
    const owners = r.owners ? `${r.owners.users}/${r.owners.teams}` : '-';
    const yanked = r.yanked === null ? '-' : r.yanked ? 'はい' : 'いいえ';
    const rel = r.relation === 'direct' ? '直接' : '推移';
    out.push(`| ${escapeCell(r.crate)} | ${escapeCell(r.version)} | ${rel} | ${owners} | ${yanked} | ${escapeCell(r.latest ?? '-')} | ${escapeCell(r.updated ?? '-')} | ${escapeCell(r.notes.join('、'))} |`);
  }
  out.push('', '### 要約', '');
  out.push(`- registry の crate: ${s.registry_crates} 件（直接 ${s.direct} 件）`);
  out.push(`- 取り下げ済み: ${s.yanked} 件`);
  out.push(`- 所有者 1 人: ${s.single_owner} 件（うち例外 ${s.single_owner_acknowledged} 件）`);
  out.push(`- 所有者 1 人で直接の依存（例外に無い）: ${s.single_owner_direct_unlisted.length === 0 ? 'なし' : s.single_owner_direct_unlisted.map(escapeCell).join('、')}`);
  if (s.flags.length === 0) {
    out.push('- 要確認: なし');
  } else {
    out.push('- 要確認:');
    for (const f of s.flags) out.push(`  - ${escapeCell(f)}`);
  }
  return out.join('\n') + '\n';
}

export function renderJson(baseline, result) {
  return JSON.stringify({ baseline, summary: summarize(result), rows: result.rows }, null, 2) + '\n';
}

// ---------- 入口 ----------

const USAGE = 'usage: node tools/inventory-deps.mjs [--lock <path>] [--format md|json] [--delay-ms <n>] [--timeout-ms <n>] [--index-base <url>] [--api-base <url>]';

function parseArgs(argv) {
  const o = { lock: path.join(REPO_ROOT, 'Cargo.lock'), format: 'md', delayMs: 1000, timeoutMs: 30000, indexBase: DEFAULT_INDEX_BASE, apiBase: DEFAULT_API_BASE };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const v = argv[i + 1];
    if (!['--lock', '--format', '--delay-ms', '--timeout-ms', '--index-base', '--api-base'].includes(a) || v === undefined) {
      throw new InventoryError(`unknown or incomplete argument: ${a}`);
    }
    i++;
    if (a === '--lock') o.lock = path.resolve(v);
    else if (a === '--format') {
      if (v !== 'md' && v !== 'json') throw new InventoryError(`invalid --format: ${v}`);
      o.format = v;
    } else if (a === '--delay-ms' || a === '--timeout-ms') {
      if (!/^\d{1,7}$/.test(v)) throw new InventoryError(`invalid ${a}: ${v}`);
      if (a === '--delay-ms') o.delayMs = Number(v); else o.timeoutMs = Number(v);
    } else if (a === '--index-base') {
      if (v !== DEFAULT_INDEX_BASE && !LOOPBACK_RE.test(v)) throw new InventoryError('--index-base must be the default host or loopback');
      o.indexBase = v;
    } else {
      if (v !== DEFAULT_API_BASE && !LOOPBACK_RE.test(v)) throw new InventoryError('--api-base must be the default host or loopback');
      o.apiBase = v;
    }
  }
  return o;
}

function gitInfo() {
  try {
    const opts = { cwd: REPO_ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] };
    const commit = execFileSync('git', ['rev-parse', '--short', 'HEAD'], opts).trim();
    const dirty = execFileSync('git', ['status', '--porcelain'], opts).trim() !== '';
    return { commit, dirty };
  } catch {
    return { commit: 'unknown', dirty: false };
  }
}

// io は { stdout(s), stderr(s) }。終了コードを返す。fail-closed のため、標準出力は最後に 1 回だけ書く。
export async function main(argv, io, fetchFn = globalThis.fetch) {
  try {
    const o = parseArgs(argv);
    const lockBytes = fs.readFileSync(o.lock);
    const packages = parseLock(lockBytes.toString('utf8'));
    const result = await inventory({
      packages, fetchFn, indexBase: o.indexBase, apiBase: o.apiBase, delayMs: o.delayMs, timeoutMs: o.timeoutMs,
      log: (m) => io.stderr(`${m}\n`),
    });
    const git = gitInfo();
    const baseline = {
      date: new Date().toISOString().slice(0, 10),
      commit: git.commit,
      dirty: git.dirty,
      lock_sha256: crypto.createHash('sha256').update(lockBytes).digest('hex'),
      node: process.version,
      hosts: [...new Set([o.indexBase, o.apiBase].map((u) => new URL(u).host))],
    };
    io.stdout(o.format === 'json' ? renderJson(baseline, result) : renderMarkdown(baseline, result));
    return result.flags.length === 0 ? 0 : 1;
  } catch (e) {
    const msg = e instanceof InventoryError ? e.message : `cannot complete inventory: ${e && e.code ? e.code : 'error'}`;
    io.stderr(`error: ${msg}\n`);
    if (e instanceof InventoryError && /^unknown or incomplete|^invalid --|must be the default/.test(e.message)) io.stderr(`${USAGE}\n`);
    return 2;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const code = await main(process.argv.slice(2), {
    stdout: (s) => process.stdout.write(s),
    stderr: (s) => process.stderr.write(s),
  });
  process.exitCode = code;
}
