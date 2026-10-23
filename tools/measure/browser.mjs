#!/usr/bin/env node
// 初回表示と再描画の時間を、Playwright の Chromium で測る（tools/measure/first-paint.sh・redraw.sh・log-redraw.sh から呼ばれる）。
//
// 使い方: node tools/measure/browser.mjs --kind first-paint|redraw|log-redraw --dir <配る一式>
//
// 役割の境界: ブラウザの操作と統計だけを担う。結果の様式（基点・schema）はシェル側（lib.sh の measure_emit）が組み立てて検証する。
// ここは標準出力へ JSON を 1 つだけ出す（measurements[]・compression・conditions・browser・playwright）。進行状況は標準エラーへ出す。
// 配信は tools/serve.mjs の startServer（127.0.0.1 固定）を使い、外部へ通信しない。
// log-redraw（TASK-223 1/2。EVT-24）は redraw とは別の量で、ログが 100〜119 項目の状態での「隠し要素の click 1 回の同期的な処理」
// （イベントの委譲・状態の更新・DOM への反映）を測る。出典は docs/spec/03-poc/engine-event-consistency/env.md。値は既定の最小教材
// （min-3step）のもので、教材全体を代表しない。閾値（16ms）を超えても失敗にせず値を出す（合否は記録の文書に書く）。
// ブラウザは FANDHE_BROWSER_CHANNEL=chrome で Chrome の原本にできる（基準端末 A での人の測定用）。
// 条件の出典は docs/spec/02-poc-plan.md §7（CPU 減速なし、下り・上り 10Mbps、遅延 40ms、キャッシュ無効、1 条件 10 回）。
//
// 終了コード: 0 = 成功 / 1 = 測定の失敗、または前提の欠如（playwright・Chromium が無い） / 2 = 誤用。
// 値の単位はマイクロ秒の整数（様式 v1 の value が正の整数のため）。0 に丸まる値は失敗にする（fail-closed）。
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { startServer } from '../serve.mjs';

const USAGE = 'usage: node tools/measure/browser.mjs --kind first-paint|redraw|log-redraw --dir <path>';

const BASE = '/subpath/';
const TRIALS = 10;
const LATENCY_MS = 40;
const THROUGHPUT_BPS = 10e6 / 8; // 10 Mbps（バイト毎秒）
const GZIP_LEVEL = 9; // wasm-size.sh の gzip -9 とレベルを揃える
const REDRAW_SAMPLES = 20;
const REDRAW_BATCH = 200;
const NAV_TIMEOUT_MS = 60000;

// log-redraw の条件。拒否される入力 1 回でログが 2 件増える（受信と拒否）ため、100 から始めて 10 操作で 118 までに収まる。
// 1 試行 10 操作 × 20 試行 = 計 200 値（EVT-24 の「1 条件 10 試行 × 20 操作」と値の数を揃える）。
const LOG_OP = { input: { command: '/make-note Bad_Name' } };
const LOG_DELTA = 2;
const LOG_WINDOW = [100, 119];
const LOG_START = 100;
const LOG_TRIALS = 20;
const LOG_SAMPLES = 10;
const LOG_THRESHOLD_US = 16000;
const LOG_START_STEP = 'main/st-1';

// 中央値。偶数件は中央 2 値の平均（PoC の measure-poc1.mjs と同じ定義）。
export function median(values) {
  if (!Array.isArray(values) || values.length === 0) throw new Error('median of an empty list');
  const s = [...values].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 === 1 ? s[m] : (s[m - 1] + s[m]) / 2;
}

// 95 パーセンタイル（nearest-rank: s[ceil(0.95n)-1]）。
export function p95(values) {
  if (!Array.isArray(values) || values.length === 0) throw new Error('p95 of an empty list');
  const s = [...values].sort((a, b) => a - b);
  return s[Math.ceil(0.95 * s.length) - 1];
}

// ミリ秒をマイクロ秒の整数へ丸める。0 以下・非有限は失敗（値を 1 へ繰り上げない）。
export function toMicros(ms, label = 'value') {
  const us = Math.round(ms * 1000);
  if (!Number.isFinite(us) || us < 1) throw new Error(`${label} rounds to 0 us (got ${ms} ms)`);
  return us;
}

// ブラウザの種類。tools/browser/harness.mjs の resolveLaunchOptions と同じ許可リスト（chromium|chrome）。そちらは playwright を
// 静的に import するため、playwright が無いときの案内を出せるよう、ここでは同じ規則を持つ。chrome が無ければ起動の失敗にする（fail-closed）。
export function resolveLaunchOptions(env = process.env) {
  const c = env.FANDHE_BROWSER_CHANNEL === undefined || env.FANDHE_BROWSER_CHANNEL === '' ? 'chromium' : env.FANDHE_BROWSER_CHANNEL;
  if (c !== 'chromium' && c !== 'chrome') throw new Error(`unsupported browser channel: ${JSON.stringify(c)} (allowed: chromium, chrome)`);
  return c === 'chrome' ? { channel: 'chrome' } : {};
}

function progress(msg) {
  console.error(`[browser.mjs] ${msg}`);
}

// 全試行共通: 新しい context を作り、必要なら回線を絞る。CPU の減速は行わない。
async function newPage(browser, throttle) {
  const context = await browser.newContext();
  const page = await context.newPage();
  if (throttle) {
    const cdp = await context.newCDPSession(page);
    await cdp.send('Network.enable');
    await cdp.send('Network.setCacheDisabled', { cacheDisabled: true });
    await cdp.send('Network.emulateNetworkConditions', {
      offline: false,
      latency: LATENCY_MS,
      downloadThroughput: THROUGHPUT_BPS,
      uploadThroughput: THROUGHPUT_BPS,
    });
  }
  return { context, page };
}

// ページ内で data-mounted の変化を MutationObserver で捉え、その時点の performance.now() を残す（ポーリングで測らない）。
// 固定の関数だけを渡し、外部の文字列を評価しない。
function installMountObserver() {
  const record = () => {
    const v = document.documentElement && document.documentElement.getAttribute('data-mounted');
    if (v && window.__mountedValue === undefined) {
      window.__mountedValue = v;
      window.__mountedAt = performance.now();
    }
  };
  new MutationObserver(record).observe(document, {
    attributes: true,
    subtree: true,
    attributeFilter: ['data-mounted'],
  });
  record();
}

async function measureFirstPaint(browser, dir) {
  const srv = await startServer({ dir, base: BASE, port: 0, gzip: true, gzipLevel: GZIP_LEVEL });
  try {
    const mounted = [];
    const fcp = [];
    let wasmTransfer = 0;
    for (let i = 0; i < TRIALS; i++) {
      const { context, page } = await newPage(browser, true);
      try {
        await page.addInitScript(installMountObserver);
        const encodings = [];
        page.on('response', (res) => {
          if (new URL(res.url()).pathname.endsWith('.wasm')) {
            encodings.push(res.headers()['content-encoding'] ?? '');
          }
        });
        await page.goto(srv.url, { waitUntil: 'commit', timeout: NAV_TIMEOUT_MS });
        await page.waitForFunction(() => window.__mountedValue !== undefined, null, { timeout: NAV_TIMEOUT_MS });
        // FCP はマウントの描画の後に記録されるため、記録されるまで待つ（無ければ失敗）。
        await page.waitForFunction(
          () => performance.getEntriesByType('paint').some((e) => e.name === 'first-contentful-paint'),
          null,
          { timeout: 10000 },
        );
        const r = await page.evaluate(() => {
          const wasm = performance
            .getEntriesByType('resource')
            .filter((e) => new URL(e.name).pathname.endsWith('.wasm'));
          const paint = performance.getEntriesByType('paint').find((e) => e.name === 'first-contentful-paint');
          return {
            value: window.__mountedValue,
            at: window.__mountedAt,
            fcp: paint ? paint.startTime : null,
            wasmEncoded: wasm.length === 1 ? wasm[0].encodedBodySize : null,
          };
        });
        if (r.value !== 'true') throw new Error(`data-mounted is "${r.value}" (mount failed)`);
        if (r.fcp === null) throw new Error('first-contentful-paint entry is missing');
        if (!Number.isInteger(r.wasmEncoded) || r.wasmEncoded <= 0) {
          throw new Error('could not read the wasm transfer size');
        }
        // 回線制限が効いていることの確かめ。圧縮が効いていなければ、基点の「gzip -9」と食い違う。
        if (encodings.length !== 1 || encodings[0] !== 'gzip') {
          throw new Error(`wasm was not served with Content-Encoding: gzip (${JSON.stringify(encodings)})`);
        }
        // 10Mbps で wasm を受け取る最短時間（ミリ秒）より速ければ、制限が効いていない。
        const floorMs = ((r.wasmEncoded * 8) / 10e6) * 1000;
        if (r.at < floorMs) {
          throw new Error(`mount (${r.at} ms) is faster than the 10 Mbps transfer floor (${floorMs} ms): throttling is not in effect`);
        }
        mounted.push(r.at);
        fcp.push(r.fcp);
        wasmTransfer = r.wasmEncoded;
        progress(`first-paint trial ${i + 1}/${TRIALS}: mounted ${r.at.toFixed(1)} ms`);
      } finally {
        await context.close();
      }
    }
    return {
      measurements: [
        { name: 'first_paint_mounted_median', unit: 'us', value: toMicros(median(mounted), 'mounted median') },
        { name: 'first_paint_mounted_max', unit: 'us', value: toMicros(Math.max(...mounted), 'mounted max') },
        { name: 'first_contentful_paint_median', unit: 'us', value: toMicros(median(fcp), 'fcp median') },
        { name: 'first_paint_wasm_transfer', unit: 'bytes', value: wasmTransfer },
      ],
      compression: {
        command: `tools/serve.mjs gzip level ${GZIP_LEVEL} (Content-Encoding: gzip)`,
        implementation: `node ${process.version} zlib ${process.versions.zlib}`,
      },
      conditions: {
        network: { down_mbps: 10, latency_ms: LATENCY_MS },
        cache: 'disabled',
        cpu_throttle: 1,
        trials: TRIALS,
        base: BASE,
      },
    };
  } finally {
    await srv.close();
  }
}

// ページ内の 1 試行。固定の関数で、K 回の再描画を連続で呼ぶ時間を K で割った 1 回あたりの値（ミリ秒）を samples 件返す。
// 各サンプルで、保持した旧 root が DOM から外れ、新しい root が Hello を持つことを確かめる（no-op の再描画を速いと誤読しない）。
async function redrawInPage({ samples, batch }) {
  const out = [];
  for (let s = 0; s < samples; s++) {
    const old = document.querySelector('[data-testid=hello-root]');
    if (!old) throw new Error('hello-root is missing before the sample');
    const t0 = performance.now();
    for (let k = 0; k < batch; k++) {
      if (window.__fandheTest.rerender() !== true) throw new Error('rerender did not run');
    }
    const t1 = performance.now();
    const fresh = document.querySelector('[data-testid=hello-root]');
    if (old.isConnected !== false || !fresh || fresh === old || !fresh.textContent.includes('Hello')) {
      throw new Error('rerender did not rebuild the screen');
    }
    out.push((t1 - t0) / batch);
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  return out;
}

async function measureRedraw(browser, dir) {
  // 回線制限は掛けない。再描画はブラウザ内の処理で、回線に依存しない。
  const srv = await startServer({ dir, base: BASE, port: 0, gzip: true, gzipLevel: GZIP_LEVEL });
  try {
    const all = [];
    for (let i = 0; i < TRIALS; i++) {
      const { context, page } = await newPage(browser, false);
      try {
        await page.addInitScript(installMountObserver);
        await page.goto(srv.url, { waitUntil: 'commit', timeout: NAV_TIMEOUT_MS });
        await page.waitForFunction(() => window.__mountedValue !== undefined, null, { timeout: NAV_TIMEOUT_MS });
        const state = await page.evaluate(() => ({
          mounted: window.__mountedValue,
          hooks: typeof window.__fandheTest?.rerender === 'function',
        }));
        if (state.mounted !== 'true') throw new Error(`data-mounted is "${state.mounted}" (mount failed)`);
        if (!state.hooks) throw new Error('window.__fandheTest.rerender is missing (not a test-hooks build?)');
        const samples = await page.evaluate(redrawInPage, { samples: REDRAW_SAMPLES, batch: REDRAW_BATCH });
        if (!Array.isArray(samples) || samples.length !== REDRAW_SAMPLES) throw new Error('unexpected sample count');
        all.push(...samples);
        progress(`redraw trial ${i + 1}/${TRIALS}: median ${(median(samples) * 1000).toFixed(1)} us`);
      } finally {
        await context.close();
      }
    }
    return {
      measurements: [
        { name: 'redraw_median', unit: 'us', value: toMicros(median(all), 'redraw median') },
        { name: 'redraw_p95', unit: 'us', value: toMicros(p95(all), 'redraw p95') },
        { name: 'redraw_max', unit: 'us', value: toMicros(Math.max(...all), 'redraw max') },
      ],
      compression: {
        command: `tools/serve.mjs gzip level ${GZIP_LEVEL} (Content-Encoding: gzip)`,
        implementation: `node ${process.version} zlib ${process.versions.zlib}`,
      },
      conditions: {
        network: 'unthrottled',
        cache: 'default',
        cpu_throttle: 1,
        trials: TRIALS,
        samples_per_trial: REDRAW_SAMPLES,
        batch: REDRAW_BATCH,
        base: BASE,
      },
    };
  } finally {
    await srv.close();
  }
}

// ページ内の 1 試行。固定の関数と数値・固定の JSON だけを受け取る。ログを start まで時間を測らずに埋め、samples 回の click を 1 回ずつ測る。
// 測るのは el.click() の直前と直後の performance.now() の差だけ（Playwright の境界をまたがない）。
// 各操作の直後に、ログの長さが窓の中で想定どおり増え、ログの li が同期的に同じだけ増え、[data-list-error] が無いことを確かめる。
async function logRedrawInPage({ op, delta, win, start, samples, startStep }) {
  const st = () => JSON.parse(window.__fandheTest.stateJson()).engine;
  const el = document.querySelector('[data-testid="raw-trigger"]');
  const list = document.querySelector('[data-testid="part-log-list"]');
  if (!el || !list) throw new Error('raw-trigger or part-log-list is missing');
  const rows = () => list.querySelectorAll(':scope > li').length;
  const first = st();
  if (first.current !== startStep) throw new Error(`unexpected start step ${first.current} (not the minimal pack?)`);
  el.setAttribute('data-payload', JSON.stringify(op));
  // 埋める回数の割り切れ・戻る向きの検査はここ（ページ内）が唯一の実体。
  const fillCount = (start - first.logLen) / delta;
  if (!Number.isInteger(fillCount) || fillCount < 0) throw new Error('cannot fill the log to the start length');
  for (let i = 0; i < fillCount; i++) el.click();
  if (st().logLen !== start) throw new Error(`log length is ${st().logLen} after filling (expected ${start})`);
  const out = [];
  for (let s = 0; s < samples; s++) {
    const before = st().logLen;
    const rowsBefore = rows();
    if (before < win[0] || before > win[1]) throw new Error(`log length ${before} is outside the window`);
    const t0 = performance.now();
    el.click();
    const t1 = performance.now();
    const after = st().logLen;
    if (after !== before + delta) throw new Error(`log grew by ${after - before} (expected ${delta})`);
    if (rows() !== rowsBefore + delta) throw new Error(`log rows grew by ${rows() - rowsBefore} synchronously (expected ${delta})`);
    if (document.querySelector('[data-list-error]')) throw new Error('a list limit notice appeared');
    out.push(t1 - t0);
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  const end = st();
  if (end.logLen !== start + delta * samples || end.current !== startStep) throw new Error('unexpected end state');
  return out;
}

async function measureLogRedraw(browser, dir) {
  // 回線制限は掛けない。ブラウザ内の同期的な処理を測る。
  const srv = await startServer({ dir, base: BASE, port: 0, gzip: true, gzipLevel: GZIP_LEVEL });
  try {
    const all = [];
    for (let i = 0; i < LOG_TRIALS; i++) {
      const { context, page } = await newPage(browser, false);
      try {
        await page.addInitScript(installMountObserver);
        await page.goto(srv.url, { waitUntil: 'commit', timeout: NAV_TIMEOUT_MS });
        await page.waitForFunction(() => window.__mountedValue !== undefined, null, { timeout: NAV_TIMEOUT_MS });
        const state = await page.evaluate(() => ({
          mounted: window.__mountedValue,
          hooks: typeof window.__fandheTest?.stateJson === 'function',
        }));
        if (state.mounted !== 'true') throw new Error(`data-mounted is "${state.mounted}" (mount failed)`);
        if (!state.hooks) throw new Error('window.__fandheTest.stateJson is missing (not a test-hooks build?)');
        const samples = await page.evaluate(logRedrawInPage, {
          op: LOG_OP, delta: LOG_DELTA, win: LOG_WINDOW, start: LOG_START, samples: LOG_SAMPLES, startStep: LOG_START_STEP,
        });
        if (!Array.isArray(samples) || samples.length !== LOG_SAMPLES) throw new Error('unexpected sample count');
        all.push(...samples);
        progress(`log-redraw trial ${i + 1}/${LOG_TRIALS}: median ${(median(samples) * 1000).toFixed(1)} us`);
      } finally {
        await context.close();
      }
    }
    const p95us = toMicros(p95(all), 'log redraw p95');
    progress(`log-redraw p95 ${p95us} us (threshold ${LOG_THRESHOLD_US} us): ${p95us <= LOG_THRESHOLD_US ? 'within' : 'over'}`);
    return {
      measurements: [
        { name: 'log_redraw_median', unit: 'us', value: toMicros(median(all), 'log redraw median') },
        { name: 'log_redraw_p95', unit: 'us', value: p95us },
        { name: 'log_redraw_max', unit: 'us', value: toMicros(Math.max(...all), 'log redraw max') },
      ],
      compression: {
        command: `tools/serve.mjs gzip level ${GZIP_LEVEL} (Content-Encoding: gzip)`,
        implementation: `node ${process.version} zlib ${process.versions.zlib}`,
      },
      conditions: {
        network: 'unthrottled',
        cache: 'default',
        cpu_throttle: 1,
        trials: LOG_TRIALS,
        samples_per_trial: LOG_SAMPLES,
        op: 'input /make-note Bad_Name (rejected; hidden raw-trigger click)',
        log_delta: LOG_DELTA,
        log_window: LOG_WINDOW,
        threshold_p95_us: LOG_THRESHOLD_US,
        base: BASE,
        scope_note: 'minimal pack (min-3step); not representative of a full pack',
      },
    };
  } finally {
    await srv.close();
  }
}

async function main() {
  let values;
  try {
    ({ values } = parseArgs({
      options: { kind: { type: 'string' }, dir: { type: 'string' } },
      allowPositionals: false,
      strict: true,
    }));
  } catch (e) {
    console.error(`error: ${e.message}`);
    console.error(USAGE);
    process.exit(2);
  }
  if (!['first-paint', 'redraw', 'log-redraw'].includes(values.kind) || !values.dir) {
    console.error('error: --kind (first-paint|redraw|log-redraw) and --dir are required');
    console.error(USAGE);
    process.exit(2);
  }

  let chromium;
  let playwrightVersion;
  try {
    ({ chromium } = await import('playwright'));
    playwrightVersion = (await import('playwright/package.json', { with: { type: 'json' } })).default.version;
  } catch {
    console.error('error: playwright is not available');
    console.error('fix: npm ci && npx playwright install chromium');
    process.exit(1);
  }

  let launchOptions;
  try {
    launchOptions = resolveLaunchOptions();
  } catch (e) {
    console.error(`error: ${e.message}`);
    process.exit(2);
  }

  let browser;
  try {
    try {
      browser = await chromium.launch(launchOptions);
    } catch {
      console.error('error: could not launch Chromium');
      console.error('fix: npm ci && npx playwright install chromium');
      process.exit(1);
    }
    const runs = { 'first-paint': measureFirstPaint, redraw: measureRedraw, 'log-redraw': measureLogRedraw };
    const result = await runs[values.kind](browser, values.dir);
    result.browser = `${launchOptions.channel ?? 'chromium'} ${browser.version()}`;
    result.playwright = playwrightVersion;
    process.stdout.write(`${JSON.stringify(result)}\n`);
  } catch (e) {
    console.error(`error: ${e.message}`);
    process.exitCode = 1;
  } finally {
    if (browser) await browser.close();
  }
}

if (process.argv[1] && fs.realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main();
}
