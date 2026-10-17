#!/usr/bin/env node
// エンジンの crate のコードに、教材固有の語が無いことの静的検査（W-22。TASK-471 3/3。`make check-generic-words`）。
//
// 目的: エンジンは特定の教材の知識を持たない。GitHub・Issue・PR・Claude Code・掲示板などの語が識別子や文字列として
// コードに入ると、教材ごとの分岐を呼び込む。
//
// 語の一覧の正: docs/vocabulary/vocabulary.md の「付録 A」。ここへ写さない（正を 2 つにしない）。照合は
// `crates/pack/tests/vocabulary_extension_points.rs` と `crates/web/tests/artifact_view_generic_names.rs` と同じ規則で、
//   - 部分一致: 小文字にした字句の全体に語が部分文字列として現れたら違反
//   - 語の一致: snake_case と CamelCase で分けた語のどれかが語と完全に一致したら違反
//
// 対象: <root>/crates/*/src/** の .rs、<root>/crates/*/static/** の .js・.mjs・.html・.svg・.css、<root>/crates/*/Cargo.toml。
// 対象外: crates/*/tests/**（教材を模した試験データを意図的に持ち、配信物にも公開 API にも入らないため）。
// 字句: 英数字と `_` の連なりを字句として切り出して照合する。何から切り出すかはファイルの種類で分ける。
//   - .rs・Cargo.toml: 字句解析でコメントを除き、識別子と文字列リテラルの中身（エスケープは復号する）から切り出す。
//     コメントは設計の経緯や試料の説明を書く場所なので対象にしない
//   - static の .js・.mjs・.html・.svg・.css: 字句解析をせず、コメントも含むファイル全体の素の文字列と、エスケープを
//     文脈を見ずに全体で復号した文字列（HTML の文字参照・CSS・JS のエスケープとその組み合わせ）から切り出し、どれか 1 つの
//     読みで語が見つかれば違反にする。JS・HTML・CSS を自前で字句解析すると、正規表現の開始の判定や引用符の無い属性値
//     などの読み違いで語を見逃す経路が残るため、コメントも照合して文脈の判定そのものを無くした
//
// 例外（下の ALLOW_TOKENS・FIXED_EXCEPTIONS。理由つき。増やすときは理由を書く）:
//   - 字句の完全一致の許可: 付録 A の語に当たるが教材と無関係な名前。部分的な一致（`thread_pool` など）は許可しない
//   - 件数を固定した例外: ファイルと語を限定し、件数が増えても減っても失敗する（check-deps.sh の EXC と同じ方針）
//
// 出力: 違反ごとに「パス:行番号: 語」。ソースの文字列の値そのものは出さない。固定の英語の文言。
// 終了コード: 0 = 違反なし / 1 = 違反あり / 2 = 誤用、または検査を完了できない（一覧が読めない・.rs や Cargo.toml の
// 字句が閉じない・static が UTF-8 でない等。合格にしない）。static の復号は全域の関数で、正当な JS・HTML・CSS では 2 にならない。
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

// 文字の列（文字列・識別子の中身、static ではファイル全体の読み）を、英数字と `_` の連なりに分けて出力に積む。
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
  toml: { b: ' ', t: ' ', n: ' ', f: ' ', r: ' ', e: ' ', '"': '"', '\\': '\\' },
};

// Rust の `\u{...}` の `{` の次の位置から 16 進を読み、[符号点, `}` の次の位置] を返す。先頭の 0 は何個でもよい
// （Rust は 6 桁までだが、それを超える形はコンパイルできないので、広く受けても見逃しにはならない）。
// 数字が無い・閉じない・範囲外は fail（読めないまま照合しない）。
function bracedHex(s, start, fail) {
  let k = start;
  while (k < s.length && /[0-9a-fA-F]/.test(s[k])) k += 1;
  if (s[k] !== '}' || k === start) fail('unsupported escape');
  const digits = s.slice(start, k).join('').replace(/^0+/, '');
  if (digits.length > 6) fail('unsupported escape');
  const cp = digits === '' ? 0 : parseInt(digits, 16);
  if (cp > 0x10ffff) fail('unsupported escape');
  return [cp, k + 1];
}

// 開き引用符の内側の `\` から始まるエスケープを復号して buf に積み、[次の位置, 行] を返す。
// lang は rust・toml（1 行の基本文字列）・toml-ml（複数行の基本文字列）。未対応の形は CheckError
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
  if (c === 'u' && kind === 'rust' && s[i + 2] === '{') return emit(...bracedHex(s, i + 3, fail));
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

// static のファイルは、コメント・文字列・正規表現・タグなどの文脈を判定しない。字句の読み違い（正規表現の開始の判定、
// `<script>` の中の `<!--`、引用符の無い属性値など）で語を見逃す経路を無くすため、ファイル全体を次の読みで照合する。
//   - 生の文字列（コメントも含む）
//   - エスケープを復号した文字列。復号は文脈を見ずに全体へ形式ごとに行い、形式が重なる場合のために組み合わせる（STATIC_CHAINS）
// どれか 1 つの読みで語が見つかれば違反。復号は全域の関数で、読めない形（範囲外の参照、桁の足りないエスケープなど）は
// ブラウザと同じく置換文字にするか、文字のまま残す（生の読みも必ず照合するため、それで語を隠すことはできない）。
// そのため正当な JS・HTML・CSS で終了コード 2 にはならない。

// 文字列を 1 コード点ずつの Buf にする（各文字に元の行番号を付ける）。
function toBuf(src) {
  const buf = new Buf();
  let line = 1;
  for (const ch of src) {
    buf.add(ch, line);
    if (ch === '\n') line += 1;
  }
  return buf;
}

const REPLACEMENT = '�';
const HEX = /[0-9a-fA-F]/;

// 符号点を文字にする。0・サロゲート・範囲外は置換文字（HTML の数値文字参照・CSS のエスケープの扱いと同じ）。
function codePointText(cp) {
  if (!(cp > 0) || cp > 0x10ffff || (cp >= 0xd800 && cp <= 0xdfff)) return REPLACEMENT;
  return String.fromCodePoint(cp);
}

// 数値文字参照の数字列を復号する。数字列は先頭の 0 を含めて全体を読む（`&#000000000104;` は h）。
// C1（0x80〜0x9F）はブラウザでは windows-1252 の ASCII でない文字になり、ASCII の語の一部にならないため置換文字にする。
function numericRefText(digits, hex) {
  const trimmed = digits.replace(/^0+/, '');
  if (trimmed.length > (hex ? 6 : 7)) return REPLACEMENT;
  const cp = trimmed === '' ? 0 : parseInt(trimmed, hex ? 16 : 10);
  if (cp >= 0x80 && cp <= 0x9f) return REPLACEMENT;
  return codePointText(cp);
}

// 末尾の `;` が無くても復号される旧来の名前つき文字参照（HTML の named character references の表で `;` の無い形が
// 載っている名前）。本文では、後ろに英数字が続いても最長の名前の分だけ復号される（`&ltpr` は `<pr`）。
const LEGACY_REFS = [
  'AElig', 'AMP', 'Aacute', 'Acirc', 'Agrave', 'Aring', 'Atilde', 'Auml', 'COPY', 'Ccedil', 'ETH', 'Eacute', 'Ecirc',
  'Egrave', 'Euml', 'GT', 'Iacute', 'Icirc', 'Igrave', 'Iuml', 'LT', 'Ntilde', 'Oacute', 'Ocirc', 'Ograve', 'Oslash',
  'Otilde', 'Ouml', 'QUOT', 'REG', 'THORN', 'Uacute', 'Ucirc', 'Ugrave', 'Uuml', 'Yacute', 'aacute', 'acirc',
  'acute', 'aelig', 'agrave', 'amp', 'aring', 'atilde', 'auml', 'brvbar', 'ccedil', 'cedil', 'cent', 'copy',
  'curren', 'deg', 'divide', 'eacute', 'ecirc', 'egrave', 'eth', 'euml', 'frac12', 'frac14', 'frac34', 'gt',
  'iacute', 'icirc', 'iexcl', 'igrave', 'iquest', 'iuml', 'laquo', 'lt', 'macr', 'micro', 'middot', 'nbsp', 'not',
  'ntilde', 'oacute', 'ocirc', 'ograve', 'ordf', 'ordm', 'oslash', 'otilde', 'ouml', 'para', 'plusmn', 'pound',
  'quot', 'raquo', 'reg', 'sect', 'shy', 'sup1', 'sup2', 'sup3', 'szlig', 'thorn', 'times', 'uacute', 'ucirc',
  'ugrave', 'uml', 'uuml', 'yacute', 'yen', 'yuml',
];

// 英数字か `_` に復号される名前つき文字参照（`;` つき）。表の中でこの 3 つだけで、復号して照合する。
const WORD_REFS = new Map([
  ['fjlig', 'fj'],
  ['lowbar', '_'],
  ['UnderBar', '_'],
]);

// `&` の位置から文字参照を読む。参照でなければ null。{ text: 復号した文字, next: 次の位置 }。
// 数値の参照は `;` の有無を問わない。名前つきの参照は表の全部を写さない。WORD_REFS は復号し、旧来の名前（`;` の有無を
// 問わない。後ろに英数字が続いても最長の名前の分だけ）は区切りにする（`&ltpr` は `<pr`）。それ以外は文字のまま残す
// （名前が字句として残るだけで、語を隠す向きには倒れない。属性値か本文かで異なる旧来の名前の扱いは、生の読みが補う）。
function htmlRef(s, i) {
  if (s[i + 1] === '#') {
    let k = i + 2;
    const hex = s[k] === 'x' || s[k] === 'X';
    if (hex) k += 1;
    const start = k;
    while (k < s.length && (hex ? HEX : /[0-9]/).test(s[k])) k += 1;
    if (k === start) return null; // `&#`・`&#x` だけは参照ではなく文字のまま
    const text = numericRefText(s.slice(start, k).join(''), hex);
    if (s[k] === ';') k += 1;
    return { text, next: k };
  }
  if (!/[A-Za-z]/.test(s[i + 1] ?? '')) return null;
  let k = i + 1;
  while (k < s.length && /[A-Za-z0-9]/.test(s[k])) k += 1;
  const name = s.slice(i + 1, k).join('');
  if (s[k] === ';' && WORD_REFS.has(name)) return { text: WORD_REFS.get(name), next: k + 1 };
  let legacy = '';
  for (const l of LEGACY_REFS) if (name.startsWith(l) && l.length > legacy.length) legacy = l;
  if (legacy === '') return null;
  let next = i + 1 + legacy.length;
  if (legacy === name && s[next] === ';') next += 1;
  return { text: ' ', next };
}

// HTML の文字参照を全体で復号する。
function decodeHtml(b) {
  const out = new Buf();
  const s = b.chars;
  let i = 0;
  while (i < s.length) {
    const r = s[i] === '&' ? htmlRef(s, i) : null;
    if (r) {
      for (const ch of r.text) out.add(ch, b.lines[i]);
      i = r.next;
    } else {
      out.add(s[i], b.lines[i]);
      i += 1;
    }
  }
  return out;
}

// CSS のエスケープを全体で復号する。`\` + 16 進 1〜6 桁（後ろの空白 1 つ。CRLF は 1 つと数える）は符号点、
// `\` + 改行は行の継続として消し、`\` + その他の 1 文字はその文字。
function decodeCss(b) {
  const out = new Buf();
  const s = b.chars;
  let i = 0;
  while (i < s.length) {
    const c = s[i + 1];
    if (s[i] !== '\\' || c === undefined) {
      out.add(s[i], b.lines[i]);
      i += 1;
    } else if (c === '\n' || c === '\f') {
      i += 2;
    } else if (c === '\r') {
      i += s[i + 2] === '\n' ? 3 : 2;
    } else if (HEX.test(c)) {
      let k = i + 1;
      while (k < s.length && k < i + 7 && HEX.test(s[k])) k += 1;
      out.add(codePointText(parseInt(s.slice(i + 1, k).join(''), 16)), b.lines[i]);
      if (s[k] === '\r' && s[k + 1] === '\n') k += 2;
      else if (/[ \t\n\r\f]/.test(s[k] ?? '')) k += 1;
      i = k;
    } else {
      out.add(c, b.lines[i]);
      i += 2;
    }
  }
  return out;
}

// 位置 k から 16 進がちょうど count 桁あれば符号点を返す。無ければ -1。
function fixedHex(s, k, count) {
  const digits = s.slice(k, k + count).join('');
  return digits.length === count && /^[0-9a-fA-F]+$/.test(digits) ? parseInt(digits, 16) : -1;
}

const JS_WHITESPACE_ESCAPES = new Set(['n', 'r', 't', 'b', 'f', 'v']);

// JS のエスケープを全体で復号する。`\xHH`・`\uXXXX`・`\u{...}`（先頭の 0 は何個でもよい）は符号点、旧来の 8 進
// （`\150` は h）も符号点、`\n` などは区切り、`\` + 改行は行の継続として消し、その他（読めない `\x`・`\u` を含む）は
// `\` の次の文字そのもの（文字列や正規表現の identity escape と同じ）。
function decodeJs(b) {
  const out = new Buf();
  const s = b.chars;
  let i = 0;
  const emit = (text, next) => {
    for (const ch of text) out.add(ch, b.lines[i]);
    i = next;
  };
  while (i < s.length) {
    const c = s[i + 1];
    if (s[i] !== '\\' || c === undefined) {
      emit(s[i], i + 1);
    } else if (c === '\n' || c === ' ' || c === ' ') {
      i += 2;
    } else if (c === '\r') {
      i += s[i + 2] === '\n' ? 3 : 2;
    } else if (c === 'x' && fixedHex(s, i + 2, 2) >= 0) {
      emit(codePointText(fixedHex(s, i + 2, 2)), i + 4);
    } else if (c === 'u' && s[i + 2] === '{') {
      let k = i + 3;
      while (k < s.length && HEX.test(s[k])) k += 1;
      const digits = s.slice(i + 3, k).join('').replace(/^0+/, '');
      if (s[k] === '}' && k > i + 3 && digits.length <= 6) {
        emit(codePointText(digits === '' ? 0 : parseInt(digits, 16)), k + 1);
      } else {
        emit('u', i + 2);
      }
    } else if (c === 'u' && fixedHex(s, i + 2, 4) >= 0) {
      emit(codePointText(fixedHex(s, i + 2, 4)), i + 6);
    } else if (/[0-7]/.test(c)) {
      let k = i + 2;
      const max = /[0-3]/.test(c) ? 3 : 2;
      while (k < s.length && k < i + 1 + max && /[0-7]/.test(s[k])) k += 1;
      emit(codePointText(parseInt(s.slice(i + 1, k).join(''), 8)), k);
    } else if (JS_WHITESPACE_ESCAPES.has(c)) {
      emit(' ', i + 2);
    } else {
      emit(c, i + 2);
    }
  }
  return out;
}

const STATIC_DECODERS = { html: decodeHtml, css: decodeCss, js: decodeJs };

// 照合する読み。生・各形式・2 形式の順序つきの組（例: HTML の参照を復号した後の CSS のエスケープ（style 属性の
// `git&#92;68 ub`）、JS の文字列を復号した後の HTML の参照（`innerHTML = "git\x26#104;ub"`））。
const STATIC_CHAINS = [
  [],
  ['html'],
  ['css'],
  ['js'],
  ['html', 'css'],
  ['html', 'js'],
  ['css', 'html'],
  ['css', 'js'],
  ['js', 'html'],
  ['js', 'css'],
];

// static のファイルの違反の候補。{ line, word } の列で、同じ行・語は読みごとの件数の最大だけ数える
// （同じ箇所を読みの数だけ重ねて報告しないため）。
export function staticHits(src, terms) {
  const raw = toBuf(src);
  const best = new Map();
  for (const chain of STATIC_CHAINS) {
    const buf = chain.reduce((b, name) => STATIC_DECODERS[name](b), raw);
    const tokens = [];
    pushBuf(tokens, buf);
    const counts = new Map();
    for (const { token, line } of tokens) {
      for (const word of matchToken(token, terms)) {
        const key = `${line}\t${word}`;
        counts.set(key, (counts.get(key) ?? 0) + 1);
      }
    }
    for (const [key, n] of counts) if (n > (best.get(key) ?? 0)) best.set(key, n);
  }
  const hits = [];
  for (const [key, n] of best) {
    const [line, word] = key.split('\t');
    for (let k = 0; k < n; k += 1) hits.push({ line: Number(line), word });
  }
  return hits.sort((a, b) => a.line - b.line || a.word.localeCompare(b.word));
}

// static のファイルを UTF-8 として読む（先頭の UTF-8 の BOM は除く）。UTF-8 として不正なバイト列（BOM つきの UTF-16 を
// 含む。FF FE・FE FF は UTF-8 として不正）は、語が別のバイト列になって全部の読みから隠れうるため、検査不能（2）にする。
// NUL は正当な JS の文字列にも書けるため、それだけでは 2 にしない。
function decodeStaticBytes(bytes) {
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    throw new CheckError('unsupported encoding (not UTF-8)');
  }
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
        if (triple && d === c && s[i + 1] === c && s[i + 2] === c) {
          // 終端の引用符は 3〜5 個（3 個を超える分は文字列の末尾の引用符）。
          let k = i;
          while (s[k] === c) k += 1;
          if (k - i > 5) fail('unterminated string');
          for (let m = 0; m < k - i - 3; m += 1) buf.add(c, line);
          i = k;
          closed = true;
          break;
        }
        if (!triple && d === c) {
          i += 1;
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

// ファイル 1 つの違反の候補（{ line, word } の列）。.rs と Cargo.toml は字句解析でコメントを除き、static は素の文字列で照合する。
function hitsOf(file, bytes, terms) {
  if (!file.endsWith('.rs') && !file.endsWith('.toml')) return staticHits(decodeStaticBytes(bytes), terms);
  const src = bytes.toString('utf8');
  const tokens = file.endsWith('.rs') ? lexRust(src) : lexToml(src);
  const hits = [];
  for (const { token, line } of tokens) for (const word of matchToken(token, terms)) hits.push({ line, word });
  return hits;
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
    let bytes;
    try {
      bytes = fs.readFileSync(file);
    } catch {
      throw new CheckError('a target file is not readable');
    }
    let hits;
    try {
      hits = hitsOf(file, bytes, terms);
    } catch (e) {
      if (e instanceof CheckError) throw new CheckError(`${rel}: ${e.message}`);
      throw e;
    }
    for (const { line, word } of hits) {
      const key = `${rel}\t${word}`;
      if (FIXED_EXCEPTIONS.has(key)) exceptionCounts.set(key, (exceptionCounts.get(key) ?? 0) + 1);
      else violations.push(`${rel}:${line}: ${word}`);
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
