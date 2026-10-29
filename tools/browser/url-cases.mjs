// URL のクエリとハッシュからの注入の試験（SAFE-7。TASK-147 2/2）が使う入力を読む純粋な Node のモジュール。
//
// データの正は testdata の extra-cases.json（e6_positions・e6_payloads・ir_param_fixed）と xss-cases.md の 1 か所に置き、
// ペイロードを tools 側へ書き写さない。url-xss.test.mjs から呼ばれ、61 回分の URL の部品と「画面に出てはならない印」を返す。
// 読む前に sha256 の一覧と照合し、件数・位置の名前・ID が合わなければ例外にする（fail-closed。0 件のまま通る空振りを防ぐ）。
// 文字列を返すだけで、評価（eval・Function）はしない。ペイロードからセレクタ・パス・コードを作らない。
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { loadXssCases } from './xss-cases.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../testdata/untrusted-input-safety');
export const DEFAULT_FILE = path.join(ROOT, 'test-data-addendum-4/extra-cases.json');
export const DEFAULT_SHA_FILE = path.join(ROOT, 'test-data-addendum-4.sha256');
export const QUERY_NAMES = ['ir', 'os', 'osmode', 'store', 'tick', 'x'];
export const EXPECTED_RUNS = 61;

// sha256 の一覧（`<hex>  <相対パス>`）で file の中身を照合する。一覧に無い・不一致は例外。
export function verifySha(file, shaFile) {
  const rel = path.relative(path.dirname(shaFile), file).split(path.sep).join('/');
  const line = fs.readFileSync(shaFile, 'utf8').split('\n').find((l) => l.endsWith(`  ${rel}`));
  if (!line) throw new Error(`sha256 entry not found for ${rel}`);
  const want = line.split(/\s+/)[0];
  const got = crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  if (want !== got) throw new Error(`sha256 mismatch for ${rel}`);
}

export function parseUrlCases(json, xssById) {
  const positions = json.e6_positions;
  if (!Array.isArray(positions) || positions.length !== 7) throw new Error('expected 7 e6_positions');
  const ps = positions.map((p) => {
    if (p.where !== 'query' && p.where !== 'hash') throw new Error(`${p.id}: unknown where`);
    return p.where === 'query' ? { id: p.id, where: 'query', name: p.name } : { id: p.id, where: 'hash' };
  });
  const names = ps.filter((p) => p.where === 'query').map((p) => p.name).sort();
  if (JSON.stringify(names) !== JSON.stringify([...QUERY_NAMES].sort())) throw new Error('unexpected query names');
  if (ps.filter((p) => p.where === 'hash').length !== 1) throw new Error('expected exactly one hash position');
  const ids = json.e6_payloads;
  if (!Array.isArray(ids) || ids.length !== 8) throw new Error('expected 8 e6_payloads');
  const payloads = ids.map((id) => {
    const payload = xssById.get(id);
    if (typeof payload !== 'string' || payload === '') throw new Error(`${id}: payload not found`);
    return { id, payload };
  });
  const fixed = json.ir_param_fixed;
  if (!Array.isArray(fixed) || fixed.length !== 5) throw new Error('expected 5 ir_param_fixed');
  const irFixed = fixed.map((f) => {
    if (typeof f.id !== 'string' || typeof f.value !== 'string' || f.value === '') throw new Error('bad ir_param_fixed');
    return { id: f.id, value: f.value };
  });
  return { positions: ps, payloads, irFixed };
}

export function loadUrlCases({ file = DEFAULT_FILE, shaFile = DEFAULT_SHA_FILE } = {}) {
  verifySha(file, shaFile);
  const xssById = new Map(loadXssCases().map((c) => [c.id, c.payload]));
  return parseUrlCases(JSON.parse(fs.readFileSync(file, 'utf8')), xssById);
}

// 画面に出てはならない印。ID・値の全文（デコード前後）・共通の印。
function needlesOf(caseId, value, other) {
  return [...new Set([caseId, value, other, '__poc7_hit', 'example.invalid'])].filter((s) => s !== '');
}

// 61 回分。query は `?<name>=<符号化した値>`、hash は `#<符号化した値>`、?ir= の固定の値は再符号化せずそのまま付ける。
export function buildRuns({ positions, payloads, irFixed }) {
  const runs = [];
  for (const pos of positions) {
    for (const c of payloads) {
      const enc = encodeURIComponent(c.payload);
      runs.push({
        runId: `${pos.id}/${c.id}`, positionId: pos.id, caseId: c.id, where: pos.where, name: pos.name ?? null,
        query: pos.where === 'query' ? `?${pos.name}=${enc}` : '',
        hash: pos.where === 'hash' ? `#${enc}` : '',
        expected: c.payload, rawIr: false,
        needles: needlesOf(c.id, c.payload, enc),
      });
    }
  }
  for (const f of irFixed) {
    const decoded = decodeURIComponent(f.value);
    runs.push({
      runId: `E6-q-ir/${f.id}`, positionId: 'E6-q-ir', caseId: f.id, where: 'query', name: 'ir',
      query: `?ir=${f.value}`, hash: '',
      expected: decoded, rawIr: true,
      // IR-5 の値は ID を含まないので、ファイル名の部分を印に足す。
      needles: needlesOf(f.id === 'IR-5' ? 'min-3step' : f.id, f.value, decoded),
    });
  }
  if (runs.length !== EXPECTED_RUNS) throw new Error(`expected ${EXPECTED_RUNS} runs, got ${runs.length}`);
  return runs;
}
