#!/usr/bin/env node
// エンジンの crate のコードに、教材固有の語が無いことの静的検査（W-22。TASK-471 3/3。`make check-generic-words`）。
//
// 目的: エンジンは特定の教材の知識を持たない。GitHub・Issue・PR・Claude Code・掲示板などの語が識別子や文字列として
// コードに入ると、教材ごとの分岐を呼び込む。コメントは設計の経緯や試料の説明を書く場所なので対象にしない。
//
// 語の一覧の正: docs/vocabulary/vocabulary.md の「付録 A」。ここへ写さない（正を 2 つにしない）。照合は
// `crates/pack/tests/vocabulary_extension_points.rs` と `crates/web/tests/artifact_view_generic_names.rs` と同じ規則で、
//   - 部分一致: 小文字にした字句の全体に語が部分文字列として現れたら違反
//   - 語の一致: snake_case と CamelCase で分けた語のどれかが語と完全に一致したら違反
//
// 対象: <root>/crates/*/src/** の .rs、<root>/crates/*/static/** の .js・.html・.svg・.css、<root>/crates/*/Cargo.toml。
// 対象外: crates/*/tests/**（教材を模した試験データを意図的に持ち、配信物にも公開 API にも入らないため）。
// 字句: コメントを除き、識別子と文字列リテラルの中身を、英数字と `_` の連なり（字句）に分けて照合する。
//
// 例外（下の ALLOW_TOKENS・FIXED_EXCEPTIONS。理由つき。増やすときは理由を書く）:
//   - 字句の完全一致の許可: 付録 A の語に当たるが教材と無関係な名前。部分的な一致（`thread_pool` など）は許可しない
//   - 件数を固定した例外: ファイルと語を限定し、件数が増えても減っても失敗する（check-deps.sh の EXC と同じ方針）
//
// 出力: 違反ごとに「パス:行番号: 語」。ソースの文字列の値そのものは出さない。固定の英語の文言。
// 終了コード: 0 = 違反なし / 1 = 違反あり / 2 = 誤用、または検査を完了できない（一覧が読めない・字句が閉じない等。合格にしない）。
// 使い方: node tools/check-generic-words.mjs [--root <dir>]（--root は回帰テスト用。既定はこのファイルの親の親）。
// 回帰テスト: tools/check-generic-words.test.mjs（scripts/test-check-generic-words.sh）。
// CI・make check・lefthook には組み込まない（失敗したら止めるガードにする変更は人間の承認点。docs/testing.md）。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// 字句の完全一致で許可する名前（小文字）。
export const ALLOW_TOKENS = new Map([
  ['thread_local', 'Rust 標準のマクロ名。掲示板のスレッドではない'],
  ['slashcount', '区切り文字 `/` の個数の意味（core の宛先の形）。GitHub のスラッシュコマンドではない'],
]);

// 件数を固定した例外。キーは「相対パス\t語」。
export const FIXED_EXCEPTIONS = new Map([
  // コマンドの種類 builtin・skill・flow・unsupported の 1 語（CommandKind::Skill とその綴り 2 件）。付録 A の `skill` と
  // 実際に衝突している。改名か語彙の版上げかは人間の判断が要るため、件数を固定して増減を検出する。
  ['crates/core/src/command_class.rs\tskill', 3],
]);

class CheckError extends Error {}

// 付録 A を読む。見つからない・0 語・未知の照合の値は CheckError。
export function loadTerms(root) {
  const file = path.join(root, 'docs/vocabulary/vocabulary.md');
  let text;
  try {
    text = fs.readFileSync(file, 'utf8');
  } catch {
    throw new CheckError('vocabulary file is not readable');
  }
  const lines = text.split('\n');
  const start = lines.findIndex((l) => /^## 付録 A/.test(l));
  if (start < 0) throw new CheckError('appendix A is missing');
  const terms = [];
  for (let i = start + 1; i < lines.length; i += 1) {
    const line = lines[i];
    if (/^## /.test(line)) break;
    if (!line.startsWith('|')) continue;
    const cells = line.split('|').slice(1, -1).map((c) => c.trim());
    if (cells.length !== 3) continue;
    const m = /^`([^`]+)`$/.exec(cells[1]);
    if (!m) continue; // 見出し・区切りの行
    if (cells[2] !== '部分一致' && cells[2] !== '語の一致') throw new CheckError('unknown match mode in appendix A');
    terms.push({ word: m[1].toLowerCase(), mode: cells[2] });
  }
  if (terms.length === 0) throw new CheckError('appendix A has no terms');
  return terms;
}

// 識別子を snake_case と CamelCase の語に分けて小文字にする（artifact_view_generic_names.rs の words_of と同じ）。
export function wordsOf(ident) {
  const words = [];
  for (const part of ident.split('_')) {
    const chars = Array.from(part);
    let cur = '';
    chars.forEach((ch, i) => {
      const upper = ch !== ch.toLowerCase() && ch === ch.toUpperCase();
      const lower = (c) => c !== undefined && c !== c.toUpperCase();
      const boundary = upper && i > 0 && (lower(chars[i - 1]) || lower(chars[i + 1]));
      if (boundary && cur !== '') {
        words.push(cur.toLowerCase());
        cur = '';
      }
      cur += ch;
    });
    if (cur !== '') words.push(cur.toLowerCase());
  }
  return words;
}

// 字句 1 つに当たる語の一覧（重複なし）。許可の字句は空。
export function matchToken(token, terms) {
  const lower = token.toLowerCase();
  if (ALLOW_TOKENS.has(lower)) return [];
  const words = wordsOf(token);
  const hit = [];
  for (const t of terms) {
    const ok = t.mode === '部分一致' ? lower.includes(t.word) : words.includes(t.word);
    if (ok && !hit.includes(t.word)) hit.push(t.word);
  }
  return hit;
}

const IDENT = /[\p{L}\p{N}_]/u;

// 字句にする文字の列。chars は 1 コード点ずつ、lines は各文字の行番号（エスケープの復号や行継続で、
// 元の行と文字の並びがずれても違反の行を実際の行で報告するため）。
class Buf {
  constructor() {
    this.chars = [];
    this.lines = [];
  }
  add(ch, line) {
    this.chars.push(ch);
    this.lines.push(line);
  }
}

// 文字列・識別子の中身を、英数字と `_` の連なりに分けて出力に積む。
function pushBuf(out, buf) {
  let tok = '';
  let tokLine = 0;
  const flush = () => {
    if (tok !== '') out.push({ token: tok, line: tokLine });
    tok = '';
  };
  for (let k = 0; k < buf.chars.length; k += 1) {
    if (IDENT.test(buf.chars[k])) {
      if (tok === '') tokLine = buf.lines[k];
      tok += buf.chars[k];
    } else flush();
  }
  flush();
}

const SIMPLE_ESCAPES = {
  rust: { n: ' ', r: ' ', t: ' ', '\\': '\\', 0: ' ', "'": "'", '"': '"' },
  js: { n: ' ', r: ' ', t: ' ', b: ' ', f: ' ', v: ' ' },
  toml: { b: ' ', t: ' ', n: ' ', f: ' ', r: ' ', e: ' ', '"': '"', '\\': '\\' },
};

// 開き引用符の内側の `\` から始まるエスケープを復号して buf に積み、[次の位置, 行] を返す。
// lang は rust・js・toml（1 行の基本文字列）・toml-ml（複数行の基本文字列）。未対応の形は CheckError
// （復号できないまま照合すると語を見逃すため、合格にしない）。
function readEscape(s, i, line, lang, buf, fail) {
  const c = s[i + 1];
  if (c === undefined) fail('unterminated string');
  if (c === '\n' || (c === '\r' && s[i + 2] === '\n')) {
    if (lang === 'toml') fail('unsupported escape');
    i += c === '\n' ? 2 : 3;
    line += 1;
    if (lang === 'rust' || lang === 'toml-ml') {
      while (i < s.length && /[ \t\r\n]/.test(s[i])) {
        if (s[i] === '\n') line += 1;
        i += 1;
      }
    }
    return [i, line];
  }
  const kind = lang === 'toml-ml' ? 'toml' : lang;
  const hex = (count) => {
    const digits = s.slice(i + 2, i + 2 + count).join('');
    if (digits.length !== count || !/^[0-9a-fA-F]+$/.test(digits)) fail('unsupported escape');
    return [parseInt(digits, 16), i + 2 + count];
  };
  const emit = (cp, next) => {
    if (cp > 0x10ffff) fail('unsupported escape');
    buf.add(String.fromCodePoint(cp), line);
    return [next, line];
  };
  const simple = SIMPLE_ESCAPES[kind];
  if (c === 'x') return emit(...hex(2));
  if (c === 'u' && kind === 'toml') return emit(...hex(4));
  if (c === 'U' && kind === 'toml') return emit(...hex(8));
  if (c === 'u' && (kind === 'rust' || kind === 'js') && s[i + 2] === '{') {
    let k = i + 3;
    while (k < s.length && /[0-9a-fA-F]/.test(s[k])) k += 1;
    if (s[k] !== '}' || k === i + 3 || k - (i + 3) > 8) fail('unsupported escape');
    return emit(parseInt(s.slice(i + 3, k).join(''), 16), k + 1);
  }
  if (c === 'u' && kind === 'js') return emit(...hex(4));
  if (kind === 'js') {
    if (c === '0' && !/[0-9]/.test(s[i + 2] ?? '')) return emit(0x20, i + 2);
    if (/[0-9]/.test(c)) fail('unsupported escape'); // 8 進・\8・\9
    if (Object.hasOwn(simple, c)) return emit(simple[c].codePointAt(0), i + 2);
    return emit(c.codePointAt(0), i + 2); // その他は文字そのもの（`\'`・`\"`・`` \` ``・`\\`・`\$` など）
  }
  if (Object.hasOwn(simple, c)) return emit(simple[c].codePointAt(0), i + 2);
  fail('unsupported escape');
  return [i, line];
}

// Rust の字句を読み、コメントを除いた識別子と文字列の字句を返す。閉じない形は CheckError。
export function lexRust(src) {
  const out = [];
  const s = Array.from(src);
  const n = s.length;
  let i = 0;
  let line = 1;
  const fail = (msg) => {
    throw new CheckError(`${msg} at line ${line}`);
  };
  // 通常の文字列（開き引用符の直後から）。エスケープを復号し、閉じ引用符の次の位置まで読む。
  const readString = () => {
    const buf = new Buf();
    while (i < n) {
      const c = s[i];
      if (c === '\\') {
        [i, line] = readEscape(s, i, line, 'rust', buf, fail);
        continue;
      }
      if (c === '"') {
        i += 1;
        pushBuf(out, buf);
        return;
      }
      buf.add(c, line);
      if (c === '\n') line += 1;
      i += 1;
    }
    fail('unterminated string');
  };
  const readRaw = (hashes) => {
    const close = `"${'#'.repeat(hashes)}`;
    const buf = new Buf();
    while (i < n) {
      if (s[i] === '"' && s.slice(i, i + close.length).join('') === close) {
        i += close.length;
        pushBuf(out, buf);
        return;
      }
      buf.add(s[i], line);
      if (s[i] === '\n') line += 1;
      i += 1;
    }
    fail('unterminated raw string');
  };
  while (i < n) {
    const c = s[i];
    if (c === '\n') {
      line += 1;
      i += 1;
    } else if (c === '/' && s[i + 1] === '/') {
      while (i < n && s[i] !== '\n') i += 1;
    } else if (c === '/' && s[i + 1] === '*') {
      let depth = 1;
      i += 2;
      while (i < n && depth > 0) {
        if (s[i] === '/' && s[i + 1] === '*') {
          depth += 1;
          i += 2;
        } else if (s[i] === '*' && s[i + 1] === '/') {
          depth -= 1;
          i += 2;
        } else {
          if (s[i] === '\n') line += 1;
          i += 1;
        }
      }
      if (depth > 0) fail('unterminated block comment');
    } else if (c === '"') {
      i += 1;
      readString();
    } else if (c === "'") {
      // 文字リテラルかライフタイムか。`'\...'` か `'x'` なら文字リテラル（中身は照合しない: 1 文字）。
      if (s[i + 1] === '\\') {
        i += 2;
        while (i < n && s[i] !== "'") {
          if (s[i] === '\n') fail('unterminated char literal');
          i += 1;
        }
        if (i >= n) fail('unterminated char literal');
        i += 1;
      } else if (s[i + 2] === "'" && s[i + 1] !== "'") {
        i += 3;
      } else {
        i += 1; // ライフタイム・ラベル。続く識別子は通常どおり読む
      }
    } else if (IDENT.test(c)) {
      let j = i;
      while (j < n && IDENT.test(s[j])) j += 1;
      const word = s.slice(i, j).join('');
      if (['r', 'br', 'cr'].includes(word) && (s[j] === '"' || s[j] === '#')) {
        let k = j;
        while (s[k] === '#') k += 1;
        if (s[k] === '"') {
          i = k + 1;
          readRaw(k - j);
          continue;
        }
        // r#ident（raw identifier）。`r#` を飛ばして識別子として読む。
        if (word === 'r' && k - j === 1) {
          i = k;
          continue;
        }
        fail('malformed raw string');
      }
      if (['b', 'c'].includes(word) && s[j] === '"') {
        i = j + 1;
        readString();
        continue;
      }
      if (word === 'b' && s[j] === "'") {
        // バイト文字 b'x'・b'\''。
        // `b'\x41'`・`b'\n'` のように、エスケープの後は閉じ引用符まで読む（`\x` は 2 桁の 16 進が続く）。
        let k = j + 1;
        if (s[k] === '\\') k += 2;
        else k += 1;
        while (k < n && s[k] !== "'") {
          if (s[k] === '\n') fail('malformed byte literal');
          k += 1;
        }
        if (k >= n) fail('malformed byte literal');
        i = k + 1;
        continue;
      }
      out.push({ token: word, line });
      i = j;
    } else {
      i += 1;
    }
  }
  return out;
}

// JS の字句。`//`・`/* */` を除き、'...'・"..."・`...` の中身を字句にする。テンプレートリテラルは補間 `${...}` を
// 入れ子の式として読み（式の中の文字列・入れ子のテンプレートも照合する）、エスケープは復号してから照合する。
// 正規表現リテラルは、直前の字句から開始と判定できたとき本体を読む（同じ行で閉じなければ CheckError）。
// 識別子の `\uXXXX` エスケープは復号する。
export function lexJs(src) {
  const out = [];
  const s = Array.from(src);
  const n = s.length;
  let i = 0;
  let line = 1;
  const fail = (msg) => {
    throw new CheckError(`${msg} at line ${line}`);
  };
  const readQuoted = (q) => {
    const buf = new Buf();
    i += 1;
    while (i < n) {
      const d = s[i];
      if (d === '\\') {
        [i, line] = readEscape(s, i, line, 'js', buf, fail);
        continue;
      }
      if (d === q) {
        i += 1;
        pushBuf(out, buf);
        return;
      }
      if (d === '\n') fail('unterminated string');
      buf.add(d, line);
      i += 1;
    }
    fail('unterminated string');
  };
  // 開きのバッククォートの位置から。`${` の手前までの断片を字句にし、式を code() で読んでから続ける。
  const readTemplate = () => {
    let buf = new Buf();
    i += 1;
    while (i < n) {
      const d = s[i];
      if (d === '\\') {
        [i, line] = readEscape(s, i, line, 'js', buf, fail);
        continue;
      }
      if (d === '`') {
        i += 1;
        pushBuf(out, buf);
        return;
      }
      if (d === '$' && s[i + 1] === '{') {
        pushBuf(out, buf);
        buf = new Buf();
        i += 2;
        code(true);
        continue;
      }
      buf.add(d, line);
      if (d === '\n') line += 1;
      i += 1;
    }
    fail('unterminated template literal');
  };
  // 識別子を読む。`\uXXXX`・`\u{...}` のエスケープは復号して 1 つの字句にする（`github` は github）。
  const readIdent = () => {
    const buf = new Buf();
    const startLine = line;
    for (;;) {
      if (i < n && IDENT.test(s[i])) {
        buf.add(s[i], line);
        i += 1;
      } else if (s[i] === '\\' && s[i + 1] === 'u') {
        let cp;
        if (s[i + 2] === '{') {
          let k = i + 3;
          while (k < n && /[0-9a-fA-F]/.test(s[k])) k += 1;
          if (s[k] !== '}' || k === i + 3 || k - (i + 3) > 8) fail('unsupported escape');
          cp = parseInt(s.slice(i + 3, k).join(''), 16);
          i = k + 1;
        } else {
          const digits = s.slice(i + 2, i + 6).join('');
          if (digits.length !== 4 || !/^[0-9a-fA-F]+$/.test(digits)) fail('unsupported escape');
          cp = parseInt(digits, 16);
          i += 6;
        }
        if (cp > 0x10ffff) fail('unsupported escape');
        buf.add(String.fromCodePoint(cp), startLine);
      } else break;
    }
    const word = buf.chars.join('');
    pushBuf(out, buf);
    return word;
  };
  // 正規表現リテラル（開きの `/` から）。本体の字句は照合し、`\x` の組は飛ばす。クラス `[...]` の中の `/` は終端にしない。
  // 同じ行で閉じない形は CheckError（除算との取り違えで以降を読み飛ばさない）。
  const readRegex = () => {
    const buf = new Buf();
    i += 1;
    let inClass = false;
    while (i < n) {
      const d = s[i];
      if (d === '\n') fail('unterminated regular expression');
      if (d === '\\') {
        if (s[i + 1] === undefined || s[i + 1] === '\n') fail('unterminated regular expression');
        buf.add(' ', line);
        i += 2;
        continue;
      }
      if (d === '[') inClass = true;
      else if (d === ']') inClass = false;
      else if (d === '/' && !inClass) {
        i += 1;
        while (i < n && IDENT.test(s[i])) i += 1; // フラグ
        pushBuf(out, buf);
        return;
      }
      buf.add(d, line);
      i += 1;
    }
    fail('unterminated regular expression');
  };
  const KEYWORDS_BEFORE_REGEX = new Set([
    'return', 'typeof', 'instanceof', 'in', 'of', 'new', 'delete', 'void', 'throw', 'case', 'do', 'else', 'yield', 'await',
  ]);
  // 直前の字句から、`/` が正規表現の開始かを決める。値の終わり（識別子・文字列・数・`)`・`]`）の後は除算。
  const regexAllowed = (prev) => {
    if (prev === null) return true;
    if (prev === 'operand' || prev === ')' || prev === ']') return false;
    if (prev.startsWith('kw:')) return KEYWORDS_BEFORE_REGEX.has(prev.slice(3));
    return true;
  };
  // コードを読む。inInterp のときは、対応する `}` の手前で止まる（`}` の次の位置へ進めて返る）。
  const code = (inInterp) => {
    let depth = 0;
    // 直前の有意な字句（null・'operand'・'kw:<語>'・記号 1 文字）。
    let prev = null;
    while (i < n) {
      const c = s[i];
      if (c === '\n') {
        line += 1;
        i += 1;
      } else if (c === '/' && s[i + 1] === '/') {
        while (i < n && s[i] !== '\n') i += 1;
      } else if (c === '/' && s[i + 1] === '*') {
        i += 2;
        while (i < n && !(s[i] === '*' && s[i + 1] === '/')) {
          if (s[i] === '\n') line += 1;
          i += 1;
        }
        if (i >= n) fail('unterminated block comment');
        i += 2;
      } else if (c === '/' && regexAllowed(prev)) {
        readRegex();
        prev = 'operand';
      } else if (c === '"' || c === "'") {
        readQuoted(c);
        prev = 'operand';
      } else if (c === '`') {
        readTemplate();
        prev = 'operand';
      } else if (inInterp && c === '{') {
        depth += 1;
        i += 1;
        prev = c;
      } else if (inInterp && c === '}') {
        i += 1;
        if (depth === 0) return;
        depth -= 1;
        prev = c;
      } else if (IDENT.test(c) || (c === '\\' && s[i + 1] === 'u')) {
        const word = readIdent();
        prev = KEYWORDS_BEFORE_REGEX.has(word) ? `kw:${word}` : 'operand';
      } else {
        if (!/\s/.test(c)) prev = c;
        i += 1;
      }
    }
    if (inInterp) fail('unterminated template interpolation');
  };
  code(false);
  return out;
}

// HTML・SVG。`<!-- -->` だけをコメントとして除く。タグの中の引用符つきの属性値にある `<!--` はコメントにしない。
// 数値の文字参照（`&#103;`・`&#x67;`。ブラウザと同じく末尾の `;` が無くても復号する）は復号する。タグの引用符が閉じない・コメントが閉じない形は CheckError。
export function lexHtml(src) {
  const out = [];
  const s = Array.from(src);
  const n = s.length;
  const buf = new Buf();
  let i = 0;
  let line = 1;
  let inTag = false;
  let quote = null;
  const fail = (msg) => {
    throw new CheckError(`${msg} at line ${line}`);
  };
  while (i < n) {
    const c = s[i];
    if (!inTag && c === '<' && s[i + 1] === '!' && s[i + 2] === '-' && s[i + 3] === '-') {
      const start = line;
      i += 4;
      while (i < n && !(s[i] === '-' && s[i + 1] === '-' && s[i + 2] === '>')) {
        if (s[i] === '\n') line += 1;
        i += 1;
      }
      if (i >= n) {
        line = start;
        fail('unterminated comment');
      }
      i += 3;
      buf.add(' ', line);
      continue;
    }
    if (c === '&' && s[i + 1] === '#') {
      const m = /^&#(?:[xX]([0-9a-fA-F]+)|([0-9]+));?/.exec(s.slice(i, i + 12).join(''));
      if (m) {
        const cp = m[1] ? parseInt(m[1], 16) : parseInt(m[2], 10);
        if (cp > 0x10ffff) fail('unsupported character reference');
        buf.add(String.fromCodePoint(cp), line);
        i += m[0].length;
        continue;
      }
    }
    if (!inTag) {
      if (c === '<' && /[A-Za-z/!?]/.test(s[i + 1] ?? '')) inTag = true;
    } else if (quote !== null) {
      if (c === quote) quote = null;
    } else if (c === '"' || c === "'") {
      quote = c;
    } else if (c === '>') {
      inTag = false;
    }
    buf.add(c, line);
    if (c === '\n') line += 1;
    i += 1;
  }
  if (quote !== null) fail('unterminated attribute value');
  pushBuf(out, buf);
  return out;
}

// CSS。`/* */` だけをコメントとして除く。文字列（'...'・"..."）の中の `/*` はコメントにしない。
// CSS のエスケープ（`\67 ` のような 16 進、`\g` のような文字そのもの）は復号してから照合する。
export function lexCss(src) {
  const out = [];
  const s = Array.from(src);
  const n = s.length;
  const buf = new Buf();
  let i = 0;
  let line = 1;
  let quote = null;
  const fail = (msg) => {
    throw new CheckError(`${msg} at line ${line}`);
  };
  while (i < n) {
    const c = s[i];
    if (c === '\\') {
      const d = s[i + 1];
      if (d === undefined) fail('unsupported escape');
      if (d === '\n') {
        if (quote === null) fail('unsupported escape');
        line += 1;
        i += 2;
        continue;
      }
      if (/[0-9a-fA-F]/.test(d)) {
        let k = i + 1;
        while (k < n && k < i + 7 && /[0-9a-fA-F]/.test(s[k])) k += 1;
        const cp = parseInt(s.slice(i + 1, k).join(''), 16);
        if (cp > 0x10ffff) fail('unsupported escape');
        buf.add(cp === 0 ? ' ' : String.fromCodePoint(cp), line);
        if (s[k] === ' ' || s[k] === '\t') k += 1;
        else if (s[k] === '\n') {
          line += 1;
          k += 1;
        }
        i = k;
        continue;
      }
      buf.add(d, line);
      i += 2;
      continue;
    }
    if (quote !== null) {
      if (c === quote) quote = null;
      else if (c === '\n') fail('unterminated string');
      buf.add(c, line);
      i += 1;
      continue;
    }
    if (c === '/' && s[i + 1] === '*') {
      const start = line;
      i += 2;
      while (i < n && !(s[i] === '*' && s[i + 1] === '/')) {
        if (s[i] === '\n') line += 1;
        i += 1;
      }
      if (i >= n) {
        line = start;
        fail('unterminated comment');
      }
      i += 2;
      buf.add(' ', line);
      continue;
    }
    if (c === '"' || c === "'") quote = c;
    buf.add(c, line);
    if (c === '\n') line += 1;
    i += 1;
  }
  if (quote !== null) fail('unterminated string');
  pushBuf(out, buf);
  return out;
}

// TOML。文字列の外の `#` から行末までがコメント。文字列（基本・リテラル・複数行）の中身は字句にする。
// 基本文字列のエスケープは復号してから照合する。
export function lexToml(src) {
  const out = [];
  const s = Array.from(src);
  const n = s.length;
  let i = 0;
  let line = 1;
  const fail = (msg) => {
    throw new CheckError(`${msg} at line ${line}`);
  };
  while (i < n) {
    const c = s[i];
    if (c === '\n') {
      line += 1;
      i += 1;
    } else if (c === '#') {
      while (i < n && s[i] !== '\n') i += 1;
    } else if (c === '"' || c === "'") {
      const triple = s[i + 1] === c && s[i + 2] === c;
      const buf = new Buf();
      i += triple ? 3 : 1;
      let closed = false;
      while (i < n) {
        const d = s[i];
        if (c === '"' && d === '\\') {
          [i, line] = readEscape(s, i, line, triple ? 'toml-ml' : 'toml', buf, fail);
          continue;
        }
        if (d === c && (!triple || (s[i + 1] === c && s[i + 2] === c))) {
          i += triple ? 3 : 1;
          closed = true;
          break;
        }
        if (d === '\n' && !triple) fail('unterminated string');
        buf.add(d, line);
        if (d === '\n') line += 1;
        i += 1;
      }
      if (!closed) fail('unterminated string');
      pushBuf(out, buf);
    } else if (IDENT.test(c)) {
      let j = i;
      while (j < n && IDENT.test(s[j])) j += 1;
      out.push({ token: s.slice(i, j).join(''), line });
      i = j;
    } else {
      i += 1;
    }
  }
  return out;
}

// 走査。シンボリックリンクは辿らない（読まずに 2 で止める）。
function rejectLink(p) {
  let st;
  try {
    st = fs.lstatSync(p);
  } catch (e) {
    if (e.code === 'ENOENT') return false;
    throw new CheckError('a target path is not readable');
  }
  if (st.isSymbolicLink()) throw new CheckError('symbolic link in the target tree');
  return true;
}

function listFiles(dir, accept, acc = []) {
  if (!rejectLink(dir)) return acc;
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch (e) {
    if (e.code === 'ENOENT') return acc;
    throw new CheckError('directory is not readable');
  }
  for (const e of entries.sort((a, b) => a.name.localeCompare(b.name))) {
    const p = path.join(dir, e.name);
    if (e.isSymbolicLink()) throw new CheckError('symbolic link in the target tree');
    if (e.isDirectory()) listFiles(p, accept, acc);
    else if (e.isFile() && accept(e.name)) acc.push(p);
  }
  return acc;
}

function targets(root) {
  const crates = path.join(root, 'crates');
  rejectLink(crates);
  let names;
  try {
    names = fs.readdirSync(crates, { withFileTypes: true });
  } catch {
    throw new CheckError('crates directory is not readable');
  }
  for (const e of names) if (e.isSymbolicLink()) throw new CheckError('symbolic link in the target tree');
  names = names.filter((e) => e.isDirectory()).map((e) => e.name).sort();
  if (names.length === 0) throw new CheckError('no crates found');
  const files = [];
  for (const name of names) {
    const base = path.join(crates, name);
    listFiles(path.join(base, 'src'), (f) => f.endsWith('.rs'), files);
    listFiles(path.join(base, 'static'), (f) => /\.(js|mjs|html|svg|css)$/.test(f), files);
    const toml = path.join(base, 'Cargo.toml');
    if (rejectLink(toml)) files.push(toml);
  }
  return files;
}

function lexFile(file, src) {
  if (file.endsWith('.rs')) return lexRust(src);
  if (file.endsWith('.js') || file.endsWith('.mjs')) return lexJs(src);
  if (file.endsWith('.toml')) return lexToml(src);
  if (file.endsWith('.css')) return lexCss(src);
  return lexHtml(src);
}

// 検査の本体。{ violations: ['path:line: word', ...], files } を返す。検査を完了できなければ CheckError。
export function check(root) {
  const terms = loadTerms(root);
  const files = targets(root);
  if (files.length === 0) throw new CheckError('no target files');
  const violations = [];
  const exceptionCounts = new Map();
  for (const file of files) {
    const rel = path.relative(root, file).split(path.sep).join('/');
    let src;
    try {
      src = fs.readFileSync(file, 'utf8');
    } catch {
      throw new CheckError('a target file is not readable');
    }
    let tokens;
    try {
      tokens = lexFile(file, src);
    } catch (e) {
      if (e instanceof CheckError) throw new CheckError(`${rel}: ${e.message}`);
      throw e;
    }
    for (const { token, line } of tokens) {
      for (const word of matchToken(token, terms)) {
        const key = `${rel}\t${word}`;
        if (FIXED_EXCEPTIONS.has(key)) exceptionCounts.set(key, (exceptionCounts.get(key) ?? 0) + 1);
        else violations.push(`${rel}:${line}: ${word}`);
      }
    }
  }
  for (const [key, want] of FIXED_EXCEPTIONS) {
    const got = exceptionCounts.get(key) ?? 0;
    if (got !== want) {
      const [rel, word] = key.split('\t');
      violations.push(`${rel}: ${word}: fixed exception count differs (expected ${want}, found ${got})`);
    }
  }
  return { violations, files: files.length };
}

function main(argv) {
  let root = fileURLToPath(new URL('..', import.meta.url));
  const args = argv.slice(2);
  if (args.length === 2 && args[0] === '--root') root = path.resolve(args[1]);
  else if (args.length !== 0) {
    console.error('usage: check-generic-words.mjs [--root <dir>]');
    return 2;
  }
  try {
    const { violations, files } = check(root);
    if (violations.length > 0) {
      for (const v of violations) console.error(`violation: ${v}`);
      console.error(`error: ${violations.length} generic-word violation(s)`);
      return 1;
    }
    console.log(`ok: no engine-specific words in ${files} files`);
    return 0;
  } catch (e) {
    if (!(e instanceof CheckError)) throw e;
    console.error(`error: ${e.message}`);
    return 2;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv);
}
