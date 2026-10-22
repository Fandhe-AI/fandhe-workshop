#!/usr/bin/env node
// 影響する Step の洗い出し（VER-15・UMAP-5・C-18。TASK-182 1/2）。
//
// 目的: 教材の参照元（上流の repo）の版を上げるとき、演出と実際の挙動の対応表から、見直すべき Step を機械的に一覧にする。
// 対応表の置き場は教材 repo（読み込まれない別の文書。D-70）で、エンジン（crates/）は対応表を持たない。
// 本道具は教材に依存しない作成者向けの道具として、対応表の機械可読な最小の形（版 1）と照合だけを持つ。
// 実際の対応表・Step ID との対応の教材テストは教材 repo の担当。手順書は docs/upstream-bump-procedure.md（TASK-182 2/2）。
//
// 入力の契約（版 1）:
//   --map <path>（必須）: UTF-8 の CSV。引用符は使わない（`vocab` の表と同じ流儀）。
//     1 行目はヘッダで `scope,step,ref` が必須。任意の列 `path`（上流 repo の中の相対パス。空はその参照名の全体に依拠）と
//     `lines`（人が読む欄。照合に使わない）を持てる。それ以外の列は読み飛ばし、出力に写さない。列の順は自由。
//     空行と `#` で始まる行は注記として読み飛ばす。`"`・制御文字・U+2028・U+2029 を含む行は誤り。
//   --changed <ref>（繰り返し可）: その参照名の全体が変わった。表のその参照名の全行が影響する。
//   --changes <path>: CSV（ヘッダ `ref,path`）。上流の差分の 1 ファイルを 1 行で書く（`git diff --name-only` から作る想定）。
//     表の行の `ref` が一致し、かつ（表の行の `path` が空、または完全一致）なら影響する。
//   --changed と --changes は少なくとも 1 つが必要で、併用すると和集合。
//   --vocab <path>（任意）: `fandhe-pack vocab <pack-dir>` の標準出力を保存したもの。与えると、表の行の (scope, step) が
//     一覧に無いとき要確認（終了コード 1）にする。本文は各行がちょうど 4 列で、scope・step が空でなく、kind が step か use であること。
//   入力はすべて UTF-8 として厳密に復号し、不正なバイト列は誤り（終了コード 2）にする。
//   行番号では照合しない（版を上げると行がずれ、行の重なりでは取りこぼすため）。
//
// 出力: 標準出力は結果だけ。ヘッダ `scope,step` のあと、影響する Step を重複なく (scope, step) の UTF-16 のコード単位順で
// 1 行 1 件並べ、続けて `# ` で始まる集計行を出す。写すのは ID と参照名の数だけで、自由記述の列は出さない。
//
// 終了コード:
//   0 = 列挙できた（0 件を含む）
//   1 = 列挙できたが要確認がある（--vocab 指定時に教材に無い Step を指す行がある）
//   2 = 誤用、または入力の形が不正。fail-closed とし、標準出力は空にする。標準エラーには行番号と理由だけを出し、セルの値は写さない
//
// 使い方: node tools/upstream-impact.mjs --map <path> (--changed <ref>... | --changes <path>) [--vocab <path>]
// 回帰テスト: tools/upstream-impact.test.mjs（scripts/test-upstream-impact.sh・make test-upstream-impact）。
// 失敗したら止めるガードにする変更は人間の承認点のため、CI・make check・lefthook には組み込まない。
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

export const MAX_INPUT_BYTES = 4 * 1024 * 1024;
export const MAX_INPUT_LINES = 10000;

// 入力の形の誤り。where は `map`・`changes`・`vocab`・`args` のいずれか。値は持たない（理由だけ）。
export class ImpactInputError extends Error {
  constructor(where, line, reason) {
    super(line > 0 ? `${where}:${line}: ${reason}` : `${where}: ${reason}`);
    this.where = where;
    this.line = line;
  }
}

const BAD_CHARS = /["\u0000-\u0008\u000b-\u001f\u007f\u2028\u2029]/;

// ヘッダと本文の行へ分ける。行番号は元のテキストの 1 始まりで返す。
function csvLines(text, where) {
  const t = text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
  const raw = t.split('\n');
  if (raw.length > MAX_INPUT_LINES) throw new ImpactInputError(where, 0, 'too many lines');
  const out = [];
  raw.forEach((l, i) => {
    const line = l.endsWith('\r') ? l.slice(0, -1) : l;
    if (line === '' || line.startsWith('#')) return;
    if (BAD_CHARS.test(line)) throw new ImpactInputError(where, i + 1, 'forbidden character in line');
    out.push({ no: i + 1, cells: line.split(',') });
  });
  return out;
}

function headerIndex(head, where, required) {
  const idx = new Map();
  for (const [i, name] of head.cells.entries()) {
    if (idx.has(name)) throw new ImpactInputError(where, head.no, 'duplicate column name');
    idx.set(name, i);
  }
  for (const r of required) {
    if (!idx.has(r)) throw new ImpactInputError(where, head.no, `missing required column ${r}`);
  }
  return idx;
}

function checkPath(p, where, no) {
  if (p === '') return;
  if (p.startsWith('/') || p.includes('\\')) throw new ImpactInputError(where, no, 'path must be relative without backslash');
  for (const seg of p.split('/')) {
    if (seg === '' || seg === '..') throw new ImpactInputError(where, no, 'path has an empty or ".." segment');
  }
}

// 対応表を [{scope, step, ref, path}] にする。
export function parseMap(text) {
  const lines = csvLines(text, 'map');
  if (lines.length === 0) throw new ImpactInputError('map', 0, 'missing header');
  const [head, ...body] = lines;
  const idx = headerIndex(head, 'map', ['scope', 'step', 'ref']);
  const rows = [];
  for (const l of body) {
    if (l.cells.length !== head.cells.length) throw new ImpactInputError('map', l.no, 'column count differs from header');
    const scope = l.cells[idx.get('scope')];
    const step = l.cells[idx.get('step')];
    const ref = l.cells[idx.get('ref')];
    const p = idx.has('path') ? l.cells[idx.get('path')] : '';
    if (scope === '' || step === '' || ref === '') throw new ImpactInputError('map', l.no, 'scope, step and ref must not be empty');
    checkPath(p, 'map', l.no);
    rows.push({ scope, step, ref, path: p });
  }
  return rows;
}

// 変わったファイルの一覧を [{ref, path}] にする。
export function parseChanges(text) {
  const lines = csvLines(text, 'changes');
  if (lines.length === 0) throw new ImpactInputError('changes', 0, 'missing header');
  const [head, ...body] = lines;
  if (head.cells.join(',') !== 'ref,path') throw new ImpactInputError('changes', head.no, 'header must be ref,path');
  return body.map((l) => {
    if (l.cells.length !== 2) throw new ImpactInputError('changes', l.no, 'column count differs from header');
    const [ref, p] = l.cells;
    if (ref === '' || p === '') throw new ImpactInputError('changes', l.no, 'ref and path must not be empty');
    checkPath(p, 'changes', l.no);
    return { ref, path: p };
  });
}

// `fandhe-pack vocab` の出力から Step の組の集合を取る（鍵は `scope\u0000step`）。`# ` の集計行は読み飛ばす。
export function parseVocab(text) {
  const lines = csvLines(text, 'vocab');
  if (lines.length === 0 || lines[0].cells.join(',') !== 'scope,step,kind,extension_points') {
    throw new ImpactInputError('vocab', lines.length ? lines[0].no : 0, 'header must be scope,step,kind,extension_points');
  }
  const set = new Set();
  for (const l of lines.slice(1)) {
    if (l.cells.length !== 4) throw new ImpactInputError('vocab', l.no, 'column count differs from header');
    if (l.cells[0] === '' || l.cells[1] === '') throw new ImpactInputError('vocab', l.no, 'scope and step must not be empty');
    if (l.cells[2] !== 'step' && l.cells[2] !== 'use') throw new ImpactInputError('vocab', l.no, 'kind must be step or use');
    set.add(`${l.cells[0]}\u0000${l.cells[1]}`);
  }
  return set;
}

const cmp = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

// 変わった参照名・ファイルに当たる行から、重複のない (scope, step) を整列して返す。
export function impactedSteps(rows, { refs = new Set(), files = [] } = {}) {
  const fileKeys = new Set(files.map((f) => `${f.ref}\u0000${f.path}`));
  const hit = new Map();
  for (const r of rows) {
    // path が空の行は参照名の全体に依拠するため、その参照名のどのファイルが変わっても影響する。
    const byFile = r.path === '' ? files.some((f) => f.ref === r.ref) : fileKeys.has(`${r.ref}\u0000${r.path}`);
    const by = refs.has(r.ref) || byFile;
    if (by) hit.set(`${r.scope}\u0000${r.step}`, { scope: r.scope, step: r.step });
  }
  return [...hit.values()].sort((a, b) => cmp(a.scope, b.scope) || cmp(a.step, b.step));
}

// 表の行が指す Step のうち教材に無いものを返す（重複なし・整列済み）。
export function unknownSteps(rows, vocabSet) {
  const miss = new Map();
  for (const r of rows) {
    if (!vocabSet.has(`${r.scope}\u0000${r.step}`)) miss.set(`${r.scope}\u0000${r.step}`, { scope: r.scope, step: r.step });
  }
  return [...miss.values()].sort((a, b) => cmp(a.scope, b.scope) || cmp(a.step, b.step));
}

export function render({ steps, unknown, unknownRefs }) {
  const out = ['scope,step', ...steps.map((s) => `${s.scope},${s.step}`), `# 影響する Step: ${steps.length}`];
  if (unknown) out.push(`# 教材に無い Step: ${unknown.map((s) => `${s.scope},${s.step}`).join(' ') || 'なし'}`);
  out.push(`# 表に無い参照名: ${unknownRefs}`);
  return `${out.join('\n')}\n`;
}

const USAGE = 'usage: node tools/upstream-impact.mjs --map <path> (--changed <ref>... | --changes <path>) [--vocab <path>]\n';

function parseArgs(argv) {
  const o = { map: null, changes: null, vocab: null, changed: [], help: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--help') o.help = true;
    else if (a === '--map' || a === '--changes' || a === '--vocab' || a === '--changed') {
      const v = argv[++i];
      if (v === undefined) throw new ImpactInputError('args', 0, `${a} needs a value`);
      if (a === '--changed') o.changed.push(v);
      else if (o[a.slice(2)] !== null) throw new ImpactInputError('args', 0, `${a} given twice`);
      else o[a.slice(2)] = v;
    } else throw new ImpactInputError('args', 0, 'unknown option');
  }
  return o;
}

function readInput(p, where) {
  let st;
  try {
    st = fs.statSync(p);
  } catch {
    throw new ImpactInputError(where, 0, 'cannot read file');
  }
  if (!st.isFile()) throw new ImpactInputError(where, 0, 'not a regular file');
  if (st.size > MAX_INPUT_BYTES) throw new ImpactInputError(where, 0, 'file too large');
  let buf;
  try {
    buf = fs.readFileSync(p);
  } catch {
    throw new ImpactInputError(where, 0, 'cannot read file');
  }
  // 不正なバイト列は U+FFFD に置換されると照合から静かに外れるため、置換せず入力の誤りにする。
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(buf);
  } catch {
    throw new ImpactInputError(where, 0, 'invalid UTF-8');
  }
}

export function run(argv) {
  const o = parseArgs(argv);
  if (o.help) return { code: 0, stdout: USAGE };
  if (o.map === null) throw new ImpactInputError('args', 0, '--map is required');
  if (o.changed.length === 0 && o.changes === null) throw new ImpactInputError('args', 0, '--changed or --changes is required');
  for (const r of o.changed) {
    if (r === '' || BAD_CHARS.test(r) || r.includes(',')) throw new ImpactInputError('args', 0, 'invalid --changed value');
  }
  const rows = parseMap(readInput(o.map, 'map'));
  const files = o.changes === null ? [] : parseChanges(readInput(o.changes, 'changes'));
  const vocab = o.vocab === null ? null : parseVocab(readInput(o.vocab, 'vocab'));
  const refs = new Set(o.changed);
  const steps = impactedSteps(rows, { refs, files });
  const known = new Set(rows.map((r) => r.ref));
  const asked = new Set([...refs, ...files.map((f) => f.ref)]);
  const unknownRefs = [...asked].filter((r) => !known.has(r)).length;
  const unknown = vocab === null ? null : unknownSteps(rows, vocab);
  return { code: unknown && unknown.length > 0 ? 1 : 0, stdout: render({ steps, unknown, unknownRefs }) };
}

function main() {
  try {
    const r = run(process.argv.slice(2));
    process.stdout.write(r.stdout);
    process.exitCode = r.code;
  } catch (e) {
    if (!(e instanceof ImpactInputError)) throw e;
    process.stderr.write(`error: ${e.message}\n${e.where === 'args' ? USAGE : ''}`);
    process.exitCode = 2;
  }
}

if (process.argv[1] && fs.realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) main();
