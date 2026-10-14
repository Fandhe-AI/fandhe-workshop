#!/usr/bin/env node
// 静的ファイルの一式をサブパスの下で gzip つきで配る、手元専用の配信サーバ。
//
// 役割: GitHub Pages のプロジェクトサイト（https://<owner>.github.io/<任意の名前>/）を模して、
// tools/build-web.sh の出力（dist/web）がサブパスで動くこと（相対参照・wasm の MIME・gzip）を確かめる。
// 呼び出し元: `make serve`（人が手で確かめる）、TASK-28 2/2 の Playwright 共通ヘルパ（startServer を直接呼ぶ）、
// TASK-32 の計測。エンジンは配信先の名前を知らない。base は呼び出し側が任意に決める。
//
// 使い方: node tools/serve.mjs [--dir <path>] [--base <path>] [--port <n>] [--no-gzip] [--gzip-level <1-9>]
// 既定: --dir は repo の dist/web、--base は /subpath/、--port は 8765、gzip は有効。
// 手元専用のため 127.0.0.1 に固定して待ち受ける（host を変える引数は置かない）。
//
// 終了コード: 0 = 正常終了 / 1 = 起動の失敗（ポート使用中など） / 2 = 誤用（未知の引数・不正な値）。
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const HOST = '127.0.0.1';
const DEFAULT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'dist', 'web');

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.wasm': 'application/wasm',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.txt': 'text/plain; charset=utf-8',
};
// 静的ホストが圧縮して配る種類に合わせる。
const GZIP_EXTS = new Set(['.html', '.js', '.mjs', '.json', '.wasm', '.css', '.svg']);

// base を `/x/` の形に正規化する。曖昧な値は誤用として例外にする。
export function normalizeBase(input) {
  if (typeof input !== 'string') throw new Error('base must be a string');
  if (/[\\?#%\u0000-\u001f\u007f]/.test(input)) throw new Error(`invalid base: ${JSON.stringify(input)}`);
  const trimmed = input.replace(/^\//, '').replace(/\/$/, '');
  if (trimmed === '') return '/';
  const parts = trimmed.split('/');
  if (parts.some((s) => s === '' || s === '.' || s === '..')) {
    throw new Error(`invalid base: ${JSON.stringify(input)}`);
  }
  return `/${parts.join('/')}/`;
}

// エラー本文は固定文字列にし、要求のパスを反射させない。
function sendText(res, status, text, extra = {}) {
  const body = Buffer.from(`${text}\n`);
  res.writeHead(status, {
    'Content-Type': 'text/plain; charset=utf-8',
    'Content-Length': body.length,
    'Cache-Control': 'no-store',
    'X-Content-Type-Options': 'nosniff',
    ...extra,
  });
  res.end(body);
}

function acceptsGzip(header) {
  if (typeof header !== 'string') return false;
  for (const item of header.split(',')) {
    const [name, ...params] = item.trim().split(';');
    if (name.trim().toLowerCase() !== 'gzip') continue;
    const q = params.map((p) => p.trim()).find((p) => /^q=/i.test(p));
    return !q || Number(q.slice(2)) > 0;
  }
  return false;
}

function inside(root, p) {
  return p === root || p.startsWith(root + path.sep);
}

// 要求を root 配下の実ファイルへ解決する。返り値は { file, st } か { status }。
function resolveRequest(rawUrl, base, root) {
  const pathname = rawUrl.split('?')[0].split('#')[0];
  if (!pathname.startsWith(base)) return { status: 404 };
  const rest = pathname.slice(base.length);
  const trailingSlash = rest === '' || rest.endsWith('/');
  const segments = rest === '' ? [] : rest.split('/');
  if (trailingSlash && segments.length > 0) segments.pop();
  const decoded = [];
  for (const seg of segments) {
    let d;
    try {
      d = decodeURIComponent(seg);
    } catch {
      return { status: 400 };
    }
    if (d === '' || d === '.' || d === '..' || /[/\\\u0000]/.test(d)) return { status: 404 };
    decoded.push(d);
  }
  let real;
  try {
    real = fs.realpathSync(path.join(root, ...decoded));
  } catch {
    return { status: 404 };
  }
  if (!inside(root, real)) return { status: 404 };
  let st = fs.statSync(real);
  if (st.isDirectory()) {
    if (!trailingSlash) return { status: 404 };
    try {
      real = fs.realpathSync(path.join(real, 'index.html'));
    } catch {
      return { status: 404 };
    }
    if (!inside(root, real)) return { status: 404 };
    st = fs.statSync(real);
  } else if (trailingSlash) {
    return { status: 404 };
  }
  if (!st.isFile()) return { status: 404 };
  return { file: real, st };
}

export async function startServer({ dir = DEFAULT_DIR, base = '/subpath/', port = 0, gzip = true, gzipLevel = 6 } = {}) {
  const normBase = normalizeBase(base);
  if (!Number.isInteger(port) || port < 0 || port > 65535) throw new Error('port must be an integer in 0-65535');
  if (!Number.isInteger(gzipLevel) || gzipLevel < 1 || gzipLevel > 9) throw new Error('gzipLevel must be an integer in 1-9');
  let root;
  try {
    root = fs.realpathSync(dir);
    if (!fs.statSync(root).isDirectory()) throw new Error('not a directory');
  } catch {
    throw new Error(`dir is not an existing directory: ${dir}`);
  }
  // 圧縮結果のキャッシュ。mtime・size をキーに含め、再ビルド後に古い内容を返さない。
  const cache = new Map();

  const server = http.createServer((req, res) => {
    try {
      if (req.method !== 'GET' && req.method !== 'HEAD') {
        sendText(res, 405, 'Method Not Allowed', { Allow: 'GET, HEAD' });
        return;
      }
      const r = resolveRequest(req.url ?? '/', normBase, root);
      if (!r.file) {
        sendText(res, r.status, r.status === 400 ? 'Bad Request' : 'Not Found');
        return;
      }
      const ext = path.extname(r.file).toLowerCase();
      const headers = {
        'Content-Type': MIME[ext] ?? 'application/octet-stream',
        'Cache-Control': 'no-store',
        'X-Content-Type-Options': 'nosniff',
      };
      let body = fs.readFileSync(r.file);
      if (gzip && GZIP_EXTS.has(ext)) {
        headers.Vary = 'Accept-Encoding';
        if (acceptsGzip(req.headers['accept-encoding'])) {
          const key = `${r.file}\0${r.st.mtimeMs}\0${r.st.size}\0${body.length}`;
          let z = cache.get(key);
          if (!z) {
            z = zlib.gzipSync(body, { level: gzipLevel });
            cache.set(key, z);
          }
          body = z;
          headers['Content-Encoding'] = 'gzip';
        }
      }
      headers['Content-Length'] = body.length;
      res.writeHead(200, headers);
      res.end(req.method === 'HEAD' ? undefined : body);
    } catch {
      if (!res.headersSent) sendText(res, 500, 'Internal Server Error');
      else res.destroy();
    }
  });

  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(port, HOST, resolve);
  });
  const actual = server.address().port;
  const origin = `http://${HOST}:${actual}`;
  return {
    url: `${origin}${normBase}`,
    origin,
    port: actual,
    base: normBase,
    root,
    close: () => new Promise((resolve) => {
      server.close(() => resolve());
      server.closeAllConnections();
    }),
  };
}

const USAGE = 'usage: node tools/serve.mjs [--dir <path>] [--base <path>] [--port <n>] [--no-gzip] [--gzip-level <1-9>]';

async function main() {
  let values;
  try {
    ({ values } = parseArgs({
      options: {
        dir: { type: 'string' },
        base: { type: 'string' },
        port: { type: 'string' },
        'no-gzip': { type: 'boolean' },
        'gzip-level': { type: 'string' },
      },
      allowPositionals: false,
      strict: true,
    }));
  } catch (e) {
    console.error(`error: ${e.message}`);
    console.error(USAGE);
    process.exit(2);
  }
  const num = (v, d) => (v === undefined ? d : /^\d+$/.test(v) ? Number(v) : NaN);
  const opts = {
    dir: values.dir ?? DEFAULT_DIR,
    base: values.base ?? '/subpath/',
    port: num(values.port, 8765),
    gzip: !values['no-gzip'],
    gzipLevel: num(values['gzip-level'], 6),
  };
  let srv;
  try {
    srv = await startServer(opts);
  } catch (e) {
    if (e.code === 'EADDRINUSE') {
      console.error(`error: port ${opts.port} is already in use`);
      process.exit(1);
    }
    console.error(`error: ${e.message}`);
    console.error(USAGE);
    process.exit(2);
  }
  console.log(`serving ${srv.root} at ${srv.url}`);
  const stop = () => srv.close().then(() => process.exit(0));
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
}

if (process.argv[1] && fs.realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main();
}
