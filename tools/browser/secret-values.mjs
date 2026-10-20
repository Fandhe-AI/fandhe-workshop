// 秘密情報らしい値の組み立て（SAFE-27・TASK-164 1/3）。
//
// `testdata/untrusted-input-safety/test-data-addendum-4/secret-shapes.json` の部品を実行時に連結して値を作る。
// 完成した値は repo のどこにも置かない（push protection と pre-commit の検査のため）。呼び出し側は値を変数の中だけに置き、
// ファイル・console・assert のメッセージ・diagnostic に出さない（出してよいのは試験データの ID と形の ID だけ）。
// 展開は `{N×c}`・`{seqN}`・`\n` の 3 つ。知らない記法は例外にする（黙って別の値にしない）。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const FILE = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..', '..', 'testdata', 'untrusted-input-safety', 'test-data-addendum-4', 'secret-shapes.json',
);
const SEQ = '0123456789abcdefghijklmnopqrstuvwxyz';

export function expand(part) {
  let out = '';
  let rest = part;
  while (rest.length > 0) {
    if (rest.startsWith('\\n')) {
      out += '\n';
      rest = rest.slice(2);
    } else if (rest.startsWith('{')) {
      const close = rest.indexOf('}');
      if (close < 0) throw new Error('unclosed expansion');
      const body = rest.slice(1, close);
      const seq = /^seq(\d+)$/.exec(body);
      const rep = /^(\d+)×(.)$/u.exec(body);
      if (seq) out += SEQ.slice(0, Number(seq[1]));
      else if (rep) out += rep[2].repeat(Number(rep[1]));
      else throw new Error('unknown expansion form');
      rest = rest.slice(close + 1);
    } else {
      const m = /^[^{\\]+|^\\/.exec(rest);
      out += m[0];
      rest = rest.slice(m[0].length);
    }
  }
  return out;
}

function load() {
  return JSON.parse(fs.readFileSync(FILE, 'utf8'));
}

// 10 形。{ id: 試験データの ID, expect: 形の ID, value: 組み立てた値 }。
export function secretShapes() {
  return load().shapes.map((s) => ({ id: s.id, expect: s.expect, value: s.parts.map(expand).join('') }));
}

// 対照 6 件。{ id, text }。
export function controls() {
  const f = load();
  return [...f.controls, ...f.extra_controls].map((c) => ({ id: c.id, text: c.text }));
}
