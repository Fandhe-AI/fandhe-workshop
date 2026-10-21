// ブラウザ試験（Playwright）の共通ヘルパ。
//
// 役割: ブラウザの起動・通信の計数・CSP 違反の計数・ページ内エラーの収集を一か所に集め、
// 後続の試験が同じ数え方で「0 件」を主張できるようにする。0 件が空振りでないことは harness.test.mjs の対照で示す。
// 安全性の試験（TASK-143）向けに、記録用の関数の呼び出し・ダイアログ・ポップアップ・タブの異常終了の計数、
// 画面の走査（危険な要素・on* 属性・style 属性・http(s) 以外の URL・外部 http(s)）、CSP なしの写しでの配信を足してある。
// 呼び出し元: tools/browser/*.test.mjs（TASK-28 2/2）。Chrome の原本での実行（TASK-94。扱いと記録は docs/testing.md 5.128）・TASK-143 の対照（safety.test.mjs・safety-control.test.mjs）・
// TASK-144 以降の安全性の試験も使う。safetyCounts() のキー名は後続の試験との契約のため変えない。
// 配信は tools/serve.mjs の startServer を再利用する。エンジンは配信先の名前を知らない。
//
// channel: 'chromium'（既定。Playwright 同梱の写し）または 'chrome'（Google Chrome の原本）。
// それ以外は例外にする。Chrome を求められて無いときも Chromium へ黙って切り替えず、起動の失敗にする（fail-closed）。
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { chromium } from 'playwright';
import { startServer } from '../serve.mjs';

const CHANNELS = new Set(['chromium', 'chrome']);

// 起動オプションを決める純関数。許可リスト外の channel は例外にする。
export function resolveLaunchOptions({ channel, headless = true } = {}) {
  const c = channel === undefined || channel === null ? 'chromium' : channel;
  if (typeof c !== 'string' || !CHANNELS.has(c)) {
    throw new Error(`unsupported browser channel: ${JSON.stringify(c)} (allowed: chromium, chrome)`);
  }
  return c === 'chrome' ? { channel: 'chrome', headless } : { headless };
}

// 前提バイナリ fandhe-pack の位置を FANDHE_PACK_BIN から受け取る（BUILD-16。TASK-91。PoC-11 の F-10）。
// 設定元は scripts/test-browser.sh（scripts/build-pack-bin.sh が cargo の出力から解決する）。target/debug の直書きは
// CARGO_TARGET_DIR を変えると生の ENOENT で止まるため、未設定・存在しない・通常ファイルでない場合は
// 原因を名指す例外にする。skip にはしない（fail-closed）。
export function packBinFromEnv(env = process.env) {
  const bin = env.FANDHE_PACK_BIN;
  const hint = 'FANDHE_PACK_BIN must point to the fandhe-pack binary built by scripts/build-pack-bin.sh (run make test-browser)';
  if (!bin) throw new Error(`${hint}: not set`);
  let st;
  try {
    st = fs.statSync(bin);
  } catch {
    throw new Error(`${hint}: no such file`);
  }
  if (!st.isFile()) throw new Error(`${hint}: not a regular file`);
  return bin;
}

// 環境変数 FANDHE_BROWSER_CHANNEL から channel を読む。空・未設定は 'chromium'。
export function channelFromEnv(env = process.env) {
  const v = env.FANDHE_BROWSER_CHANNEL;
  return v === undefined || v === '' ? 'chromium' : v;
}

// ブラウザを起動する。呼び出し側が close する。
export function launchBrowser(opts = {}) {
  return chromium.launch(resolveLaunchOptions(opts));
}

// 配信サーバを起動して fn に渡し、成否にかかわらず必ず閉じる。
export async function withServer({ dir, base }, fn) {
  const srv = await startServer({ dir, base, port: 0 });
  try {
    return await fn(srv);
  } finally {
    await srv.close();
  }
}

// 記録用の関数の名前。試験データ（testdata/untrusted-input-safety/test-data/xss-cases.md）のペイロードが
// この名前で呼ぶため、別名にすると実行されても数えられず空振りの 0 件になる。testdata は写しで書き換えられない。
export const HIT_FN = '__poc7_hit';

// 画面の走査で「危険」と数える要素。参考実装（POC7）と同じ集合で、img・script を含む。
export const DANGER_TAGS = [
  'script', 'iframe', 'object', 'embed', 'link', 'meta', 'style', 'form', 'svg', 'math', 'base',
  'video', 'audio', 'source', 'img', 'frame', 'frameset', 'applet', 'template', 'noscript', 'canvas', 'picture',
];

// safetyCounts が返すキー（後続の試験との契約。名前を変えない）。
export const SAFETY_KEYS = [
  'hits', 'external', 'csp', 'dialogs', 'popups', 'crashed', 'pageErrors',
  'danger', 'onAttrs', 'styleAttrs', 'nonHttp', 'extHttp', 'scanError',
];

// タグ名の終わりと属性の区切りは HTML の空白（タブ・LF・FF・CR・空白）だけで判定する。JS の \s は NBSP・垂直タブ等も
// 含むが、ブラウザではそれらは区切りにならず（属性名やタグ名の一部になる）、CSP として働かない meta を合格させてしまうため。
// 2026-10-21 のレビュー指摘で \s を外した。
const META_TAG_RE = /<meta(?=[\t\n\f\r />])(?:"[^"]*"|'[^']*'|[^'">])*>/gi;
const ATTR_RE = /([^\t\n\f\r "'<>/=]+)(?:[\t\n\f\r ]*=[\t\n\f\r ]*(?:"([^"]*)"|'([^']*)'|([^\t\n\f\r "'=<>`]+)))?/g;

// ブラウザが要素として解釈しない領域（HTML コメント・script・style・title・textarea・noscript の中身。noscript は scripting 有効のブラウザでテキスト扱いになる）を同じ長さの空白に置き換える。
// 長さを保つので、返した文字列での位置は元の文字列でもそのまま使える。閉じていないコメントは末尾まで無効として扱う。
function maskInactive(html) {
  const blank = (s) => s.replace(/[^\r\n]/g, ' ');
  const masked = String(html)
    .replace(/<!--[\s\S]*?(?:-->|$)/g, blank)
    .replace(/(<(script|style|title|textarea|noscript)(?=[\t\n\f\r />])[^>]*>)([\s\S]*?)(?=<\/\2[\t\n\f\r ]*>|$)/gi, (_m, open, _t, body) => open + blank(body));
  return maskTemplates(masked, blank);
}

// template の中身は文書に属さず meta が効かないため、入れ子を数えて最も外側の template ごと空白にする。閉じていなければ末尾まで。
function maskTemplates(src, blank) {
  const re = /<(\/?)template(?=[\t\n\f\r />])[^>]*>/gi;
  let out = '';
  let pos = 0;
  let depth = 0;
  let start = 0;
  for (const m of src.matchAll(re)) {
    if (m[1] === '') {
      if (depth === 0) start = m.index;
      depth += 1;
    } else if (depth > 0) {
      depth -= 1;
      if (depth === 0) {
        out += src.slice(pos, start) + blank(src.slice(start, m.index + m[0].length));
        pos = m.index + m[0].length;
      }
    }
  }
  if (depth > 0) {
    out += src.slice(pos, start) + blank(src.slice(start));
    pos = src.length;
  }
  return out + src.slice(pos);
}

// meta 要素を属性単位で解析する。属性名は完全一致で比べる（data-http-equiv 等の部分一致を許さない）。
// コメント等の無効な領域の meta は数えない（ブラウザでは効かないため）。
function parseMetaTags(html) {
  return [...maskInactive(html).matchAll(META_TAG_RE)].map((m) => {
    const attrs = new Map();
    const inner = m[0].slice('<meta'.length, -1);
    for (const a of inner.matchAll(ATTR_RE)) {
      const name = a[1].toLowerCase();
      if (!attrs.has(name)) attrs.set(name, a[2] ?? a[3] ?? a[4] ?? '');
    }
    return { index: m.index, raw: m[0], attrs };
  });
}

// http-equiv は列挙属性で、ブラウザは ASCII の大文字小文字だけを無視して照合し、前後の空白は除かない
// （" Content-Security-Policy " は CSP として働かない）。働かない meta を CSP と数えて合格させないよう、
// trim せず ASCII の大文字だけを小文字にして完全一致で比べる。2026-10-21 のレビュー指摘で trim を外した。
function isCspMeta(t) {
  const v = t.attrs.get('http-equiv');
  return v !== undefined && v.replace(/[A-Z]/g, (c) => c.toLowerCase()) === 'content-security-policy';
}

// HTML から CSP の meta を除去する純関数。除去した件数も返す。
export function stripCspMeta(html) {
  const src = String(html);
  const targets = parseMetaTags(src).filter(isCspMeta);
  let out = '';
  let pos = 0;
  for (const t of targets) {
    out += src.slice(pos, t.index);
    pos = t.index + t.raw.length;
  }
  out += src.slice(pos);
  return { html: out, removed: targets.length };
}

// 配信する index.html の CSP の正（SAFE-13。TASK-151 1/2）。完全一致で比べ、正規化はしない（緩む・並び替えも退行として止める）。
export const DELIVERY_CSP =
  "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; img-src 'self'";

// CSP の meta の位置と内容を検査する純関数（fail-closed）。
// <meta http-equiv> の CSP は要素より後の内容にしか効かないため、head の中で charset の meta の直後（空白だけを挟む）に
// ちょうど 1 つあり、content が DELIVERY_CSP と一致することを求める。head が暗黙に閉じていないことは、head より前と
// head の開始から charset までの構造を限ることで保証する（下の prefix・leading の検査）。呼び出し元は csp-placement.test.mjs（make test-browser）。
export function inspectCspMeta(html) {
  const problems = [];
  const src = String(html);
  const active = maskInactive(src);
  const heads = [...active.matchAll(/<head(?=[\t\n\f\r />])[^>]*>/gi)];
  const headEnds = [...active.matchAll(/<\/head[\t\n\f\r ]*>/gi)];
  if (heads.length !== 1 || headEnds.length !== 1) {
    problems.push(`head element count is ${heads.length}/${headEnds.length} (open/close)`);
    return { ok: false, problems };
  }
  const headStart = heads[0].index + heads[0][0].length;
  const headEnd = headEnds[0].index;
  // head の所属は開始・終了タグの文字列上の位置だけでは決まらない（ブラウザは head に置けない要素や文字が来た時点で
  // head を暗黙に閉じ、以降の meta は body に入って CSP が効かない）。HTML パーサを持ち込まずに所属を保証するため、
  // 構造を次の形に限る: head より前は doctype と html の開始タグと空白だけ（body が先に始まると後の <head> は無視される）、
  // head の開始の直後は空白だけを挟んで charset の meta（後段の「charset の直後に空白だけを挟んで CSP」と合わせ、
  // CSP までに head を閉じうるものが入らない）。空白は HTML の空白（タブ・LF・FF・CR・空白）に限る（NBSP 等は文字として head を閉じる）。
  // 2026-10-21 のレビュー指摘（<head><div></div> で後続の CSP が body に入るのに合格していた）で追加。
  // active はコメント・script・style の中身を空白にした写しなので、head より前のコメントはここで許容される
  // （コメントは head を閉じない）。BOM は文字として書かず \uFEFF で書く（整形で消えないように）。
  const prefix = active.slice(0, heads[0].index).replace(/^\uFEFF/, '');
  if (!/^[\t\n\f\r ]*(?:<!doctype\b[^>]*>[\t\n\f\r ]*)?(?:<html(?=[\t\n\f\r />])[^>]*>[\t\n\f\r ]*)?$/i.test(prefix)) {
    problems.push(`unexpected content before head (head may not be the real head): ${JSON.stringify(prefix.trim().slice(0, 80))}`);
  }
  const inHead = (m) => m.index >= headStart && m.index < headEnd;
  const metas = parseMetaTags(src);
  const charsets = metas.filter((t) => t.attrs.has('charset'));
  const csps = metas.filter(isCspMeta);
  if (charsets.length !== 1 || !inHead(charsets[0])) {
    problems.push(`charset meta count is ${charsets.length} (expected exactly 1, inside head)`);
  }
  if (csps.length !== 1 || !inHead(csps[0])) {
    problems.push(`CSP meta count is ${csps.length} (expected exactly 1, inside head)`);
  }
  if (problems.length > 0) return { ok: false, problems };
  const cs = charsets[0];
  const csp = csps[0];
  const leading = src.slice(headStart, cs.index);
  if (!/^[\t\n\f\r ]*$/.test(leading)) {
    problems.push(`charset meta is not the first content of head (head may be implicitly closed; before: ${JSON.stringify(leading.slice(0, 80))})`);
  }
  const between = src.slice(cs.index + cs.raw.length, csp.index);
  if (csp.index < cs.index || !/^[\t\n\f\r ]*$/.test(between)) {
    problems.push(`CSP meta is not immediately after charset meta (between: ${JSON.stringify(between.slice(0, 80))})`);
  }
  const content = csp.attrs.get('content');
  if (content === undefined) {
    problems.push('CSP meta has no content attribute');
  } else if (content !== DELIVERY_CSP) {
    problems.push(`CSP content mismatch: ${JSON.stringify(content)}`);
  }
  return { ok: problems.length === 0, problems };
}

// CSP あり・なしの両方で同じ画面を開くための配信ディレクトリを fn に渡す。
// csp: true は dir をそのまま渡す。false は index.html の CSP の meta だけを除いた一時の写しを渡し、成否にかかわらず消す。
// route での本文の書き換えは使わない（配信は gzip で、見出しが引き継がれると画面が壊れて空振りの 0 件になりうるため）。
// 除去の対象がちょうど 1 件でなければ例外（0 件だと CSP の有無を区別できず、2 件以上だと除去漏れの恐れがある）。
export async function withCspMode({ dir, csp }, fn) {
  if (typeof csp !== 'boolean') throw new Error(`csp must be a boolean: ${JSON.stringify(csp)}`);
  if (csp) return fn(dir);
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-nocsp-'));
  try {
    fs.cpSync(dir, copy, { recursive: true });
    const index = path.join(copy, 'index.html');
    const { html, removed } = stripCspMeta(fs.readFileSync(index, 'utf8'));
    if (removed !== 1) throw new Error(`expected exactly 1 CSP meta in index.html, found ${removed}`);
    fs.writeFileSync(index, html);
    return await fn(copy);
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}

// page・context を作り、記録器 rec を付ける。配信元（origin）以外への要求はすべて abort し、external に数える。
// CSP 違反は context.exposeBinding で Node 側へ送る。遷移しても失われず、ページのスクリプトより先に登録される。
export async function openPage(browser, { origin, viewport = { width: 1024, height: 768 } }) {
  const context = await browser.newContext({ viewport });
  const rec = { requests: [], external: [], failed: [], csp: [], pageErrors: [], consoleErrors: [], hits: [], dialogs: [], popups: [], crashed: false };
  await context.exposeBinding('__fandheHitRec', (_src, id) => {
    rec.hits.push(String(id));
  });
  await context.addInitScript((name) => {
    window[name] = (id) => window.__fandheHitRec(String(id));
  }, HIT_FN);
  await context.exposeBinding('__fandheCsp', (_src, v) => {
    rec.csp.push({ violatedDirective: String(v.violatedDirective), blockedURI: String(v.blockedURI) });
  });
  await context.addInitScript(() => {
    document.addEventListener('securitypolicyviolation', (e) => {
      window.__fandheCsp({ violatedDirective: e.violatedDirective, blockedURI: e.blockedURI });
    });
  });
  await context.route('**/*', (route) => {
    const url = route.request().url();
    if (url.startsWith(`${origin}/`) || url.startsWith('data:') || url.startsWith('blob:')) return route.continue();
    rec.external.push(url);
    return route.abort();
  });
  const page = await context.newPage();
  // ポップアップ（window.open・target=_blank）は URL を記録して閉じる。page 作成後に登録するため最初のページは対象外。
  context.on('page', (p) => {
    rec.popups.push(p.url());
    p.close().catch(() => {});
  });
  page.on('dialog', (d) => {
    rec.dialogs.push({ type: d.type(), message: d.message().slice(0, 60) });
    d.dismiss().catch(() => {});
  });
  page.on('crash', () => {
    rec.crashed = true;
  });
  page.on('response', (res) => {
    const h = res.headers();
    rec.requests.push({
      url: res.url(),
      status: res.status(),
      resourceType: res.request().resourceType(),
      contentType: h['content-type'] ?? '',
      contentEncoding: h['content-encoding'] ?? '',
    });
  });
  page.on('requestfailed', (req) => rec.failed.push({ url: req.url(), error: req.failure()?.errorText ?? '' }));
  page.on('pageerror', (err) => rec.pageErrors.push(String(err && err.message ? err.message : err)));
  page.on('console', (msg) => {
    if (msg.type() === 'error') rec.consoleErrors.push(msg.text());
  });
  return { page, context, rec };
}

// 区分ごとの件数を返す。httpErrors は 400 以上の応答。
export function summarize(rec) {
  return {
    requests: rec.requests.length,
    external: rec.external.length,
    failed: rec.failed.length,
    httpErrors: rec.requests.filter((r) => r.status >= 400).length,
    csp: rec.csp.length,
    pageErrors: rec.pageErrors.length,
    consoleErrors: rec.consoleErrors.length,
    hits: rec.hits.length,
    dialogs: rec.dialogs.length,
    popups: rec.popups.length,
    crashed: rec.crashed ? 1 : 0,
  };
}

// html[data-mounted] を待つ。"true" 以外（"error" を含む）は値を添えて例外にする。
export async function waitMounted(page, { timeout = 30000 } = {}) {
  await page.waitForSelector('html[data-mounted]', { state: 'attached', timeout });
  const v = await page.evaluate(() => document.documentElement.getAttribute('data-mounted'));
  if (v !== 'true') throw new Error(`mount failed: data-mounted=${JSON.stringify(v)}`);
}

// 画面（body 配下）を走査して危険な要素・属性・URL を数える。head は対象外（meta・link は画面の一部ではない）。
// 固定の関数だけを page.evaluate へ渡し、ペイロードから文字列でコードを組み立てない。
// opts.entryScripts: 画面自身の script（baseURI 基準で解決した URL が一致するもの）を各 1 件まで danger の計数から除く。
//   on* 属性・style 属性・URL 属性の検査と能動的な発火は、除いた script にも行う。
//   2 件目以降は挿入された同名の script かもしれないため危険に数える。
// opts.provoke: 数えた後で on* を持つ要素へ合成イベントを送る（onmouseover などを発火させる）。
// 評価が失敗したときは 0 件にせず error を持つ結果を返す（safetyCounts が scanError に数える）。
export async function scanDom(page, { entryScripts = ['./app.js'], provoke = true } = {}) {
  try {
    return await page.evaluate(({ tags, entries, provoke: doProvoke }) => {
      const dangerSet = new Set(tags);
      const URL_ATTRS = ['href', 'src', 'xlink:href', 'action', 'formaction', 'data', 'poster', 'srcdoc'];
      const pending = new Set(entries.map((e) => new URL(e, document.baseURI).href));
      const out = { elements: 0, dangerTags: [], onAttrs: 0, styleAttrs: 0, nonHttp: 0, extHttp: 0, excluded: 0 };
      const withOn = [];
      const nonHttpLinks = [];
      for (const el of document.body.querySelectorAll('*')) {
        out.elements += 1;
        const tag = el.localName.toLowerCase();
        let isEntry = false;
        if (tag === 'script' && el.getAttribute('src') !== null) {
          let abs = null;
          try { abs = new URL(el.getAttribute('src'), document.baseURI).href; } catch (_e) { abs = null; }
          if (abs !== null && pending.has(abs)) {
            pending.delete(abs);
            out.excluded += 1;
            isEntry = true;
          }
        }
        if (!isEntry && dangerSet.has(tag)) out.dangerTags.push(tag);
        if (Array.from(el.attributes).some((a) => /^on/i.test(a.name))) {
          out.onAttrs += 1;
          withOn.push(el);
        }
        if (el.hasAttribute('style')) out.styleAttrs += 1;
        for (const name of URL_ATTRS) {
          const v = el.getAttribute(name);
          if (v === null) continue;
          if (name === 'srcdoc') { out.nonHttp += 1; continue; }
          let u = null;
          try { u = new URL(v, document.baseURI); } catch (_e) { u = null; }
          if (u === null || (u.protocol !== 'http:' && u.protocol !== 'https:')) {
            out.nonHttp += 1;
            if (tag === 'a' && name === 'href') nonHttpLinks.push(el);
          } else if (u.origin !== location.origin) {
            out.extHttp += 1;
          }
        }
      }
      if (doProvoke) {
        for (const el of withOn) {
          for (const t of ['mouseover', 'mouseenter', 'focus', 'click', 'pointerdown']) {
            try { el.dispatchEvent(new Event(t, { bubbles: true })); } catch (_e) { /* 発火の失敗は数えない */ }
          }
          try { el.focus(); } catch (_e) { /* 同上 */ }
        }
        for (const a of nonHttpLinks) {
          try { a.click(); } catch (_e) { /* 同上 */ }
        }
      }
      return out;
    }, { tags: DANGER_TAGS, entries: entryScripts, provoke });
  } catch (e) {
    return { error: String(e && e.message ? e.message : e) };
  }
}

// 記録器 rec と走査結果 scan から、後続の試験が使う平坦な件数を作る。キー名は SAFETY_KEYS（契約）。
export function safetyCounts(rec, scan) {
  const failed = !scan || scan.error !== undefined;
  return {
    hits: rec.hits.length,
    external: rec.external.length,
    csp: rec.csp.length,
    dialogs: rec.dialogs.length,
    popups: rec.popups.length,
    crashed: rec.crashed ? 1 : 0,
    pageErrors: rec.pageErrors.length,
    danger: failed ? 0 : scan.dangerTags.length,
    onAttrs: failed ? 0 : scan.onAttrs,
    styleAttrs: failed ? 0 : scan.styleAttrs,
    nonHttp: failed ? 0 : scan.nonHttp,
    extHttp: failed ? 0 : scan.extHttp,
    scanError: failed ? 1 : 0,
  };
}

// 0 でないキーの一覧。「全区分 0」を 1 行で主張し、失敗時に区分名を出せるようにする。
export function nonZero(counts, keys = SAFETY_KEYS) {
  return keys.filter((k) => counts[k] !== 0);
}

// 走査 → 能動の段で発火した記録が Node 側へ届くのを待つ → 集計（safetyCounts の戻り値）。
export async function collectSafety(page, rec, { settleMs = 250, ...scanOpts } = {}) {
  const scan = await scanDom(page, scanOpts);
  await new Promise((resolve) => setTimeout(resolve, settleMs)); // 閉じたページでも待てるよう Node 側で待つ
  return safetyCounts(rec, scan);
}
