// 配信物（配信する設定の dist/web）の構成要素の一覧と、相対パスの参照の静的な検査（DLV-1・DLV-2。TASK-106 1/3）。
//
// 役割: 構成要素の一覧を 1 か所で宣言し、後続の試験（TASK-106 2/3・3/3。章への直接リンク・複数サブパス・ルート配信）が
// このモジュールを import して再宣言しないようにする。Playwright に依存しない純関数と定数だけを置く。
// 呼び出し元: delivery-subpath.test.mjs。
//
// 限界: web は現時点で実行時に ir/main.json を取得しない（読み込みは TASK-109・TASK-121）。そのため ir/main.json は
// required: false（IR_FILE 指定時だけ存在）とし、「ページが IR を取得する」ことは確かめない。取得するようになった時点で
// 必須の要素へ見直す。inspectDeliveryRefs は配信する設定の専用で、試験の設定（test-hooks.js・試験用の meta がある配信物）は対象外。
import fs from 'node:fs';
import path from 'node:path';
import { maskInactive } from './harness.mjs';

// Content-Type の期待値は tools/serve.mjs の MIME 表と一致させる。二重管理になるため、試験が実際の応答と照合する。
export const DELIVERY_COMPONENTS = Object.freeze([
  { path: 'index.html', contentType: 'text/html; charset=utf-8', gzip: true, required: true },
  { path: 'app.css', contentType: 'text/css; charset=utf-8', gzip: true, required: true },
  { path: 'icon.svg', contentType: 'image/svg+xml', gzip: true, required: true },
  { path: 'app.js', contentType: 'text/javascript; charset=utf-8', gzip: true, required: true },
  { path: 'wasm/fandhe_workshop_web.js', contentType: 'text/javascript; charset=utf-8', gzip: true, required: true },
  { path: 'wasm/fandhe_workshop_web_bg.wasm', contentType: 'application/wasm', gzip: true, required: true },
  { path: 'ir/main.json', contentType: 'application/json', gzip: true, required: false },
].map((c) => Object.freeze(c)));

const BY_PATH = new Map(DELIVERY_COMPONENTS.map((c) => [c.path, c]));

// 要求の URL を base からの相対パスへ写し、構成要素なら返す。base 直下（空）は index.html。構成要素でなければ null。
export function componentFromUrl(url, { origin, base }) {
  const prefix = `${origin}${base}`;
  if (!String(url).startsWith(prefix)) return null;
  const rel = String(url).slice(prefix.length).replace(/[?#].*$/, '');
  return BY_PATH.get(rel === '' ? 'index.html' : rel) ?? null;
}

// 期待する参照（空振り防止の錨）。抽出器が何も拾えずに 0 件で通ることを防ぐ。
const ANCHORS = [
  ['index.html', './app.css'],
  ['index.html', './icon.svg'],
  ['index.html', './app.js'],
  ['app.js', './wasm/fandhe_workshop_web.js'],
  ['wasm/fandhe_workshop_web.js', 'fandhe_workshop_web_bg.wasm'],
];
// 配信する設定で取得されない経路として 1 件だけ許す動的 import（試験用の meta があるときだけ読まれる）。
const HOOKS_REF = ['app.js', './test-hooks.js'];

function listFiles(dir, problems) {
  const out = [];
  const walk = (rel) => {
    let entries;
    try {
      entries = fs.readdirSync(path.join(dir, rel), { withFileTypes: true });
    } catch (e) {
      problems.push(`cannot read directory ${rel || '.'}: ${e.code ?? e.message}`);
      return;
    }
    for (const ent of entries) {
      const r = rel === '' ? ent.name : `${rel}/${ent.name}`;
      if (ent.isDirectory()) walk(r);
      else if (ent.isFile()) out.push(r);
      else problems.push(`not a regular file: ${r}`);
    }
  };
  walk('');
  return out;
}

const HTML_ATTR_RE = /\b(?:href|src)[\t\n\f\r ]*=[\t\n\f\r ]*(?:"([^"]*)"|'([^']*)'|([^\t\n\f\r "'=<>`]+))/gi;
const JS_PATTERNS = [
  /\bimport[\t\n\f\r ]*\([\t\n\f\r ]*(["'`])([^"'`]*)\1/g,
  /\bimport[\t\n\f\r ]*(["'])([^"']*)\1/g,
  /\bfrom[\t\n\f\r ]*(["'])([^"']*)\1/g,
  /\bnew[\t\n\f\r ]+URL\([\t\n\f\r ]*(["'`])([^"'`]*)\1/g,
  /\bfetch\([\t\n\f\r ]*(["'`])([^"'`]*)\1/g,
];
const CSS_PATTERNS = [/\burl\([\t\n\f\r ]*(["']?)([^"')]*)\1[\t\n\f\r ]*\)/gi, /@import[\t\n\f\r ]*(["'])([^"']*)\1/gi];

function extractRefs(file, text) {
  const refs = [];
  if (file.endsWith('.html')) {
    for (const m of maskInactive(text).matchAll(HTML_ATTR_RE)) refs.push(m[1] ?? m[2] ?? m[3]);
  } else if (file.endsWith('.js')) {
    for (const re of JS_PATTERNS) for (const m of text.matchAll(re)) refs.push(m[2]);
  } else if (file.endsWith('.css')) {
    for (const re of CSS_PATTERNS) for (const m of text.matchAll(re)) refs.push(m[2]);
  }
  return refs;
}

// 参照の規則: ./ で始まるか、スキームも / も持たない名前だけを許す。理由の文字列を返し、許すなら null。
function refRuleViolation(ref) {
  if (ref === '') return 'empty reference';
  if (/^[a-z][a-z0-9+.-]*:/i.test(ref)) return 'has a scheme';
  if (ref.startsWith('//')) return 'scheme-relative';
  if (ref.startsWith('/')) return 'root-absolute';
  if (ref.includes('\\')) return 'contains a backslash';
  if (ref.split(/[?#]/)[0].split('/').includes('..')) return 'contains a ".." segment';
  return null;
}

// 配信物ディレクトリを読み、問題の一覧（文字列配列）を返す。0 件が合格。fail-closed（読めない・拾えない場合は問題にする）。
export function inspectDeliveryRefs(dir) {
  const problems = [];
  const files = listFiles(dir, problems);
  const fileSet = new Set(files);
  for (const f of files) {
    if (!BY_PATH.has(f)) problems.push(`file is not a delivery component: ${f}`);
  }
  for (const c of DELIVERY_COMPONENTS) {
    if (c.required && !fileSet.has(c.path)) problems.push(`missing required component: ${c.path}`);
  }

  const texts = new Map();
  for (const f of files) {
    if (!/\.(html|js|css)$/.test(f) || !BY_PATH.has(f)) continue;
    try {
      texts.set(f, fs.readFileSync(path.join(dir, f), 'utf8'));
    } catch (e) {
      problems.push(`cannot read ${f}: ${e.code ?? e.message}`);
    }
  }

  const indexText = texts.get('index.html') ?? '';
  const hooksMeta = /fandhe-test-hooks/.test(maskInactive(indexText));
  if (hooksMeta) problems.push('index.html has the test-hooks meta (this check is for the delivery configuration)');

  let hooksSeen = 0;
  const found = new Set();
  for (const [file, text] of texts) {
    for (const ref of extractRefs(file, text)) {
      found.add(`${file}\n${ref}`);
      if (file === HOOKS_REF[0] && ref === HOOKS_REF[1] && !hooksMeta) {
        hooksSeen += 1;
        continue;
      }
      const why = refRuleViolation(ref);
      if (why) {
        problems.push(`${file}: reference ${JSON.stringify(ref)} is not a relative path (${why})`);
        continue;
      }
      const resolved = path.posix.normalize(path.posix.join(path.posix.dirname(file), ref.split(/[?#]/)[0]));
      if (resolved.startsWith('../') || resolved === '..') {
        problems.push(`${file}: reference ${JSON.stringify(ref)} resolves outside the delivery root`);
      } else if (!BY_PATH.has(resolved)) {
        problems.push(`${file}: reference ${JSON.stringify(ref)} resolves to ${resolved}, which is not a delivery component`);
      } else if (!fileSet.has(resolved)) {
        problems.push(`${file}: reference ${JSON.stringify(ref)} resolves to ${resolved}, which does not exist`);
      }
    }
  }
  if (!hooksMeta && hooksSeen !== 1 && texts.has('app.js')) {
    problems.push(`app.js: expected exactly 1 dynamic import of ./test-hooks.js, found ${hooksSeen}`);
  }
  for (const [file, ref] of ANCHORS) {
    if (!found.has(`${file}\n${ref}`)) problems.push(`anchor not found (extractor may be broken): ${file} -> ${ref}`);
  }
  return problems;
}
