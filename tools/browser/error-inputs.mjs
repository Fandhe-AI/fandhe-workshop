// 誤入力の試験データ（min-3step-pack/tests/playthrough.toml の `[[error_inputs]]`）を読む純粋な Node のモジュール。
//
// データの正は testdata の写し 1 か所に置き、誤入力を tools 側へ書き写さない（xss-cases.mjs と同じ方式。TASK-437）。
// ブラウザ試験（reject-lines.test.mjs）から呼ばれ、ケースの ID・開始点・事前の入力・入力・期待の分類を返す。
// TOML の汎用の解析器は足さず、このファイルの `[[error_inputs]]` が使う形（1 行の文字列・1 行のインラインテーブル）だけを読む。
// 件数・必須の欄・入力の形が合わなければ例外にする（fail-closed。0 件のまま通る空振りを防ぐ）。
// 文字列は返すだけで、評価（eval・Function）はしない。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const DEFAULT_FILE = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../testdata/engine-event-consistency/test-data/min-3step-pack/tests/playthrough.toml',
);
export const EXPECTED_TOTAL = 7;

const KEY = /^(\w+)[ \t]*=[ \t]*(.*)$/;
const APPROVALS = new Set(['approve', 'reject', 'revise']);

// `{ k = "v", k2 = "v2" }` を文字列の対へ読む。値は二重引用符の文字列だけ（`\"` と `\\` のみ解く）。
function inlineTable(src, where) {
  const m = /^\{(.*)\}$/.exec(src.trim());
  if (!m) throw new Error(`${where}: not an inline table`);
  const out = {};
  const re = /\s*(\w+)\s*=\s*"((?:[^"\\]|\\.)*)"\s*(?:,|$)/y;
  let pos = 0;
  const body = m[1];
  while (pos < body.length) {
    re.lastIndex = pos;
    const e = re.exec(body);
    if (!e) throw new Error(`${where}: unreadable table near ${JSON.stringify(body.slice(pos, pos + 20))}`);
    if (Object.hasOwn(out, e[1])) throw new Error(`${where}: duplicate key ${e[1]}`);
    out[e[1]] = e[2].replace(/\\(["\\])/g, '$1');
    pos = re.lastIndex;
  }
  return out;
}

// インラインテーブルを、ブラウザの試験の口（`Op` の JSON）へ写す。入力の形は 1 つだけ。
export function toOp(table, where) {
  const forms = Object.keys(table).filter((k) => k !== 'expect' && k !== 'target');
  if (forms.length !== 1) throw new Error(`${where}: expected exactly one input form, got ${forms.join(',')}`);
  const [form] = forms;
  const v = table[form];
  switch (form) {
    case 'cmd': return { input: { command: v } };
    case 'text': return { input: { text: v } };
    case 'choice': return { input: { choice: v } };
    case 'ui': {
      const i = v.indexOf(' ');
      if (i <= 0) throw new Error(`${where}: ui needs "<view> <path>"`);
      return { input: { ui: { view: v.slice(0, i), path: v.slice(i + 1) } } };
    }
    case 'approval':
      if (!APPROVALS.has(v)) throw new Error(`${where}: unknown decision`);
      return { input: { approval: { decision: v, target: table.target ?? null } } };
    default: throw new Error(`${where}: unknown input form ${form}`);
  }
}

export function parseErrorInputs(toml, { total = EXPECTED_TOTAL } = {}) {
  const lines = toml.replace(/\r\n?/g, '\n').split('\n');
  const blocks = [];
  let cur = null;
  for (const raw of lines) {
    const line = raw.trim();
    if (line === '' || line.startsWith('#')) continue;
    if (line.startsWith('[')) {
      cur = line === '[[error_inputs]]' ? {} : null;
      if (cur) blocks.push(cur);
      continue;
    }
    if (!cur) continue;
    const m = KEY.exec(line);
    if (!m) throw new Error(`unreadable line in error_inputs: ${JSON.stringify(line)}`);
    if (Object.hasOwn(cur, m[1])) throw new Error(`duplicate key ${m[1]} in error_inputs`);
    cur[m[1]] = m[2];
  }
  const cases = blocks.map((b, n) => {
    const where = `error_inputs[${n}]`;
    const str = (k) => {
      const m = /^"((?:[^"\\]|\\.)*)"$/.exec(b[k] ?? '');
      if (!m) throw new Error(`${where}: ${k} must be a string`);
      return m[1];
    };
    const id = str('id');
    const start = str('start');
    if (b.input === undefined) throw new Error(`${where}: input is missing`);
    const input = inlineTable(b.input, `${id}.input`);
    const expect = /^rejected:(\w+)$/.exec(input.expect ?? '');
    if (!expect) throw new Error(`${id}: expect must be rejected:<category>`);
    let pre = [];
    if (b.pre !== undefined) {
      const arr = /^\[(.*)\]$/.exec(b.pre.trim());
      if (!arr) throw new Error(`${id}: pre must be an array`);
      pre = arr[1]
        .split(/\}\s*,\s*\{/)
        .map((s) => s.trim().replace(/^\{?/, '{').replace(/\}?$/, '}'))
        .map((s, i) => toOp(inlineTable(s, `${id}.pre[${i}]`), `${id}.pre[${i}]`));
    }
    return { id, start, pre, op: toOp(input, id), category: expect[1] };
  });
  if (cases.length !== total) throw new Error(`expected ${total} error inputs, got ${cases.length}`);
  const ids = new Set(cases.map((c) => c.id));
  if (ids.size !== cases.length) throw new Error('duplicate error input id');
  return cases;
}

export function loadErrorInputs(file = DEFAULT_FILE) {
  return parseErrorInputs(fs.readFileSync(file, 'utf8'));
}
