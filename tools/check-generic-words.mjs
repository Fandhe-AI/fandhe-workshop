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

// 文字列・識別子の中身を、英数字と `_` の連なりに分けて出力に積む。
// text の中の改行ごとに行番号を進める（複数行の文字列の 2 行目以降の違反を実際の行で報告する）。
function pushTokens(out, text, line) {
  let cur = line;
  let last = 0;
  for (const m of text.matchAll(/[\p{L}\p{N}_]+/gu)) {
    for (let k = last; k < m.index; k += 1) if (text[k] === '\n') cur += 1;
    last = m.index;
    out.push({ token: m[0], line: cur });
  }
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
  // 通常の文字列（開き引用符の直後から）。エスケープを読み、閉じ引用符の次の位置を返す。
  const readString = () => {
    const startLine = line;
    let buf = '';
    while (i < n) {
      const c = s[i];
      if (c === '\\') {
        if (s[i + 1] === '\n') {
          line += 1;
          buf += '\n';
        }
        buf += ' ';
        i += 2;
        continue;
      }
      if (c === '"') {
        i += 1;
        pushTokens(out, buf, startLine);
        return;
      }
      if (c === '\n') {
        line += 1;
        buf += '\n';
      } else buf += c;
      i += 1;
    }
    fail('unterminated string');
  };
  const readRaw = (hashes) => {
    const startLine = line;
    const close = `"${'#'.repeat(hashes)}`;
    let buf = '';
    while (i < n) {
      if (s[i] === '"' && s.slice(i, i + close.length).join('') === close) {
        i += close.length;
        pushTokens(out, buf, startLine);
        return;
      }
      if (s[i] === '\n') line += 1;
      buf += s[i];
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

// JS の字句。`//`・`/* */` を除き、'...'・"..."・`...` の中身を字句にする。正規表現リテラルは読まない
// （クォートを含む正規表現は誤読する可能性があるため、閉じない文字列は CheckError にして合格にしない）。
export function lexJs(src) {
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
    } else if (c === '"' || c === "'" || c === '`') {
      const startLine = line;
      let buf = '';
      i += 1;
      let closed = false;
      while (i < n) {
        const d = s[i];
        if (d === '\\') {
          if (s[i + 1] === '\n') {
            line += 1;
            buf += '\n';
          }
          buf += ' ';
          i += 2;
          continue;
        }
        if (d === c) {
          closed = true;
          i += 1;
          break;
        }
        if (d === '\n') {
          if (c !== '`') fail('unterminated string');
          line += 1;
        }
        buf += d;
        i += 1;
      }
      if (!closed) fail('unterminated string');
      pushTokens(out, buf, startLine);
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

// `<!-- -->` または `/* */` だけをコメントとして除き、残りの全体を字句にする（HTML・SVG・CSS）。
export function lexMarkup(src, open, close) {
  const out = [];
  let line = 1;
  let pos = 0;
  while (pos < src.length) {
    const at = src.indexOf(open, pos);
    const end = at < 0 ? src.length : at;
    const chunk = src.slice(pos, end);
    pushLines(out, chunk, line);
    line += chunk.split('\n').length - 1;
    if (at < 0) break;
    const stop = src.indexOf(close, at + open.length);
    if (stop < 0) throw new CheckError(`unterminated comment at line ${line}`);
    const comment = src.slice(at, stop + close.length);
    line += comment.split('\n').length - 1;
    pos = stop + close.length;
  }
  return out;
}

function pushLines(out, text, firstLine) {
  text.split('\n').forEach((l, k) => pushTokens(out, l, firstLine + k));
}

// TOML。文字列の外の `#` から行末までがコメント。文字列（基本・リテラル・複数行）の中身は字句にする。
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
      const startLine = line;
      i += triple ? 3 : 1;
      let buf = '';
      let closed = false;
      while (i < n) {
        const d = s[i];
        if (c === '"' && d === '\\') {
          if (s[i + 1] === '\n') {
            line += 1;
            buf += '\n';
          }
          buf += ' ';
          i += 2;
          continue;
        }
        if (d === c && (!triple || (s[i + 1] === c && s[i + 2] === c))) {
          i += triple ? 3 : 1;
          closed = true;
          break;
        }
        if (d === '\n') {
          if (!triple) fail('unterminated string');
          line += 1;
        }
        buf += d;
        i += 1;
      }
      if (!closed) fail('unterminated string');
      pushTokens(out, buf, startLine);
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
  if (file.endsWith('.css')) return lexMarkup(src, '/*', '*/');
  return lexMarkup(src, '<!--', '-->');
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
