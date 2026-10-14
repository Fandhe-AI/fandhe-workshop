// ブラウザ試験（Playwright）の共通ヘルパ。
//
// 役割: ブラウザの起動・通信の計数・CSP 違反の計数・ページ内エラーの収集を一か所に集め、
// 後続の試験が同じ数え方で「0 件」を主張できるようにする。0 件が空振りでないことは harness.test.mjs の対照で示す。
// 呼び出し元: tools/browser/*.test.mjs（TASK-28 2/2）。後続の TASK-94（Chrome 原本）・TASK-143（安全性の試験）も使う。
// 配信は tools/serve.mjs の startServer を再利用する。エンジンは配信先の名前を知らない。
//
// channel: 'chromium'（既定。Playwright 同梱の写し）または 'chrome'（Google Chrome の原本）。
// それ以外は例外にする。Chrome を求められて無いときも Chromium へ黙って切り替えず、起動の失敗にする（fail-closed）。
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

// page・context を作り、記録器 rec を付ける。配信元（origin）以外への要求はすべて abort し、external に数える。
// CSP 違反は context.exposeBinding で Node 側へ送る。遷移しても失われず、ページのスクリプトより先に登録される。
export async function openPage(browser, { origin, viewport = { width: 1024, height: 768 } }) {
  const context = await browser.newContext({ viewport });
  const rec = { requests: [], external: [], failed: [], csp: [], pageErrors: [], consoleErrors: [] };
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
  };
}

// html[data-mounted] を待つ。"true" 以外（"error" を含む）は値を添えて例外にする。
export async function waitMounted(page, { timeout = 30000 } = {}) {
  await page.waitForSelector('html[data-mounted]', { state: 'attached', timeout });
  const v = await page.evaluate(() => document.documentElement.getAttribute('data-mounted'));
  if (v !== 'true') throw new Error(`mount failed: data-mounted=${JSON.stringify(v)}`);
}
