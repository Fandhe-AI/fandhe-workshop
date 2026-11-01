#!/usr/bin/env node
// 初回表示と再描画の時間を、Playwright の Chromium で測る（tools/measure/first-paint.sh・redraw.sh・log-redraw.sh・log-slope.sh・input-paint.sh・view-redraw.sh から呼ばれる）。
//
// 使い方: node tools/measure/browser.mjs --kind first-paint|first-paint-direct|redraw|log-redraw|log-slope|log-rounds|input-paint|view-redraw|product-redraw|view-redraw-cpu4x|product-redraw-cpu4x --dir <配る一式>
//
// 役割の境界: ブラウザの操作と統計だけを担う。結果の様式（基点・schema）はシェル側（lib.sh の measure_emit）が組み立てて検証する。
// ここは標準出力へ JSON を 1 つだけ出す（measurements[]・compression・conditions・browser・playwright）。進行状況は標準エラーへ出す。
// 配信は tools/serve.mjs の startServer（127.0.0.1 固定）を使い、外部へ通信しない。
// log-redraw（TASK-223 1/2。EVT-24）は redraw とは別の量で、ログが 100〜119 項目の状態での「隠し要素の click 1 回の同期的な処理」
// （イベントの委譲・状態の更新・DOM への反映）を測る。出典は docs/spec/03-poc/engine-event-consistency/env.md。値は既定の最小教材
// （min-3step）のもので、教材全体を代表しない。閾値（16ms）を超えても失敗にせず値を出す（合否は記録の文書に書く）。
// log-slope（TASK-232 1/3。EVT-26）は log-redraw と同じ量を、ログの長さ 100〜4,000 項目の 12 段階で測り、項目数ごとの傾きを見る。
// 段階は同じ context で昇順に進める。傾きは様式 v1 の value が正の整数のため measurements[] に入れず、記録の文書で値から計算する。
// log-rounds（TASK-232 3/3。EVT-26）は同じ量を、go_to で世代と Step をまたいだ 6・48 グループのログ（2 周分の通し）で測る。判定は 500 項目の段。
// input-paint（TASK-223 2/2。EVT-24）は、ログが 120〜158 項目の状態での「送信ボタンの信頼された click（CDP）から、次の
// requestAnimationFrame の中の setTimeout(0) まで」を測る。起点は click のキャプチャ段階の受け取りと event.timeStamp の 2 通りで、
// 判定には値が大きくなる timeStamp 起点を使う（出典は同じ env.md の「計測の条件」）。PoC の停止ボタンは現行の実装では処理中でなければ
// 状態を変えないため、拒否される入力の送信（1 回でログが 2 件増える）に置き換えた。閾値（100ms）を超えても失敗にせず値を出す。
// first-paint-direct（TASK-108。DLV-5）は、IR を同梱した配信する設定を同じ条件（10Mbps・40ms・キャッシュ無効・CPU 減速なし）で配り、
// `?start=s-1-1` の直接リンクを開いてから「操作できる」状態になるまでを 10 回測る。操作できるの定義は installOperableObserver の述語
// （docs/measurement.md）。3 秒を超えても失敗にせず judgement に入れて JSON を出す（非 0 で終わる判定はシェル側の lib.sh が行う）。
// view-redraw（TASK-485 1/3。ART-4）は、左の成果物ビューの部品単位の再描画を、代役の部品の画面（固定データ）で測る。実体は
// view-redraw.mjs で、ここは統計と起動の共通部を渡すだけ。本体の view() での測定（ART-26）や CPU 4 倍（ART-15）は別のイシュー。
// product-redraw（TASK-485 2/3。ART-26）は、本体の画面（製品の ArtifactPart の view()）の同じ再描画を、選択のボタンの click の同期区間で測る。実体は product-redraw.mjs。
// ブラウザは FANDHE_BROWSER_CHANNEL=chrome で Chrome の原本にできる（基準端末 A での人の測定用）。
// 条件の出典は docs/spec/02-poc-plan.md §7（CPU 減速なし、下り・上り 10Mbps、遅延 40ms、キャッシュ無効、1 条件 10 回）。
//
// 終了コード: 0 = 成功 / 1 = 測定の失敗、または前提の欠如（playwright・Chromium が無い） / 2 = 誤用。
// 値の単位はマイクロ秒の整数（様式 v1 の value が正の整数のため）。0 に丸まる値は失敗にする（fail-closed）。
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { startServer } from '../serve.mjs';
import { measureViewRedraw, measureViewRedrawCpu4x } from './view-redraw.mjs';
import { measureProductRedraw, measureProductRedrawCpu4x } from './product-redraw.mjs';

const USAGE = 'usage: node tools/measure/browser.mjs --kind first-paint|first-paint-direct|redraw|log-redraw|log-slope|log-rounds|input-paint|view-redraw|product-redraw|view-redraw-cpu4x|product-redraw-cpu4x --dir <path>';

const BASE = '/subpath/';
const TRIALS = 10;
const LATENCY_MS = 40;
const THROUGHPUT_BPS = 10e6 / 8; // 10 Mbps（バイト毎秒）
const GZIP_LEVEL = 9; // wasm-size.sh の gzip -9 とレベルを揃える
const REDRAW_SAMPLES = 20;
const REDRAW_BATCH = 200;
const NAV_TIMEOUT_MS = 60000;

// first-paint-direct の条件。開始点 ID は tools/measure/first-paint-direct.sh が IR に足す値と同じ（読み込んだ IR で一致を確かめる）。
export const DIRECT_START = 's-1-1';
export const FIRST_PAINT_DIRECT_THRESHOLD_US = 3000000;
const DIRECT_NEXT_NODE = 'main/st-1-2';
const START_ID_RE = /^s-[0-9]+-[0-9]+[a-z]?$/;
const OPERABLE_DEFINITION =
  'data-mounted=true, run-status idle, step title equals the start node title, no log row for that step, and at least one enabled visible input port (choice, approval, or draft with submit)';

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

// input-paint の条件。1 試行で窓の下端 120 まで時間を測らずに埋め、20 回の送信を測る（操作前のログは 120, 122, …, 158）。
// 1 試行 20 操作 × 10 試行 = 計 200 値（PoC の「1 条件 10 試行 × 20 操作」と同じ）。
const INPUT_DRAFT = '/make-note Bad_Name';
const INPUT_DELTA = 2;
const INPUT_WINDOW = [120, 158];
const INPUT_START = 120;
const INPUT_TRIALS = 10;
const INPUT_SAMPLES = 20;
const INPUT_THRESHOLD_US = 100000;
const INPUT_GAP_MS = 30; // 操作の間の待ち（PoC と同じ）
const INPUT_SETTLE_MS = 200; // 下書きを入れた後の待ち。保存などの後続の処理を最初の計測に入れないため
const INPUT_WAIT_MS = 10000; // 1 操作の完了待ちの上限。rAF が来ない環境で止まらず失敗させる
const SUBMIT_SEL = '[data-testid="submit-input"]';
const DRAFT_SEL = '[data-testid="draft-input"]';

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

// 全試行共通: 新しい context を作り、必要なら回線を絞る。CPU の減速は行わない（cpu4x の種別だけが、mount の後に calibrateAndThrottle で掛ける）。
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

// ---- CPU 4 倍の記録（ART-15。TASK-485 3/3）の減速と較正 ----
// 減速は記録だけの種別（view-redraw-cpu4x・product-redraw-cpu4x）が、mount の後・測る操作の前に掛ける。読み込み・mount の時間と検査を巻き込まない。
// CDP へ渡す率は CPU4X_RATE の完全一致だけ。減速が効いていないまま走ると速い値が「4 倍の値」として残るため、固定の仕事量の時間の比を測って
// CPU_CALIBRATION_MIN_RATIO 以上でなければ失敗にする（first-paint の転送の下限の検査と同じ fail-closed の考え方）。
// 呼び出し元: view-redraw.mjs・product-redraw.mjs（deps で受け取る。循環 import を避ける）。
export const CPU4X_RATE = 4;
export const CPU_CALIBRATION_MIN_RATIO = 2;
export const CPU_CALIBRATION_ITERATIONS = 2000000;

export async function applyCpuThrottle(context, page, rate) {
  if (rate !== CPU4X_RATE) throw new Error(`cpu throttle rate must be ${CPU4X_RATE}`);
  const cdp = await context.newCDPSession(page);
  await cdp.send('Emulation.setCPUThrottlingRate', { rate });
  return cdp; // detach すると減速が戻る。呼び出し側は context を閉じるまで参照を持つ
}

// ページ内の固定の関数。決まった回数の整数演算を 3 回流し、各回の時間（ms）の中央値を返す（時間で止めず、仕事の量を固定する）。
function cpuCalibrationInPage({ iterations }) {
  const times = [];
  let acc = 0;
  for (let r = 0; r < 3; r++) {
    const t0 = performance.now();
    for (let i = 0; i < iterations; i++) acc = (acc + (i ^ (acc << 1))) | 0;
    times.push(performance.now() - t0);
  }
  times.sort((a, b) => a - b);
  if (acc === 0.5) throw new Error('unreachable'); // 結果を使い、ループを消されない
  return times[1];
}

// 純関数。減速なし・ありの時間（ms）の比が下限以上なら記録の値を返し、外れたら例外にする。
export function checkCalibration(unthrottledMs, throttledMs, minRatio = CPU_CALIBRATION_MIN_RATIO) {
  if (!Number.isFinite(unthrottledMs) || !Number.isFinite(throttledMs) || unthrottledMs <= 0 || throttledMs <= 0) {
    throw new Error('CPU throttling is not in effect (calibration times are not positive numbers)');
  }
  const ratio = throttledMs / unthrottledMs;
  if (!(ratio >= minRatio)) {
    throw new Error(`CPU throttling is not in effect (calibration ratio ${ratio.toFixed(2)} < ${minRatio})`);
  }
  return { unthrottled_ms: unthrottledMs, throttled_ms: throttledMs, ratio };
}

export async function calibrateAndThrottle(context, page) {
  const arg = { iterations: CPU_CALIBRATION_ITERATIONS };
  const unthrottled = await page.evaluate(cpuCalibrationInPage, arg);
  const session = await applyCpuThrottle(context, page, CPU4X_RATE);
  const throttled = await page.evaluate(cpuCalibrationInPage, arg);
  return { session, calibration: checkCalibration(unthrottled, throttled) };
}

// 試行ごとの較正の比を、結果の conditions に残す形にまとめる。
export function calibrationInfo(list) {
  const ratios = list.map((c) => c.ratio);
  if (ratios.length === 0 || !ratios.every(Number.isFinite)) throw new Error('no calibration was recorded');
  return {
    iterations: CPU_CALIBRATION_ITERATIONS,
    min_ratio_required: CPU_CALIBRATION_MIN_RATIO,
    checks: ratios.length,
    ratio_min: Math.min(...ratios),
    ratio_max: Math.max(...ratios),
  };
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

// 3 秒の判定。values は整数マイクロ秒の 10 件。中央値は丸めた整数、最大とともに閾値以下（ちょうどは以内）を以内とする。
export function operableJudgement(valuesUs, thresholdUs = FIRST_PAINT_DIRECT_THRESHOLD_US) {
  if (!Array.isArray(valuesUs) || valuesUs.length !== TRIALS || !valuesUs.every((v) => Number.isInteger(v) && v > 0)) {
    throw new Error(`operableJudgement needs ${TRIALS} positive integer values`);
  }
  const medianUs = Math.round(median(valuesUs));
  const maxUs = Math.max(...valuesUs);
  const medianWithin = medianUs <= thresholdUs;
  const maxWithin = maxUs <= thresholdUs;
  return { medianUs, maxUs, judgement: { median_within: medianWithin, max_within: maxWithin, within: medianWithin && maxWithin } };
}

// IR から開始点の節点と題を引く。ID の形・starts の存在・節点の題を確かめ、欠ければ失敗にする（シェル側の jq との二重定義のずれの検出）。
export function directStartOf(ir, startId) {
  if (typeof startId !== 'string' || !START_ID_RE.test(startId)) throw new Error(`invalid start id: ${JSON.stringify(startId)}`);
  const start = (ir?.starts ?? []).find((s) => s?.id === startId);
  if (!start || typeof start.node !== 'string') throw new Error(`start ${startId} is missing from the IR`);
  const node = (ir?.nodes ?? []).find((n) => n?.uid === start.node);
  const title = node?.step?.title;
  if (typeof title !== 'string' || title === '') throw new Error(`start node ${start.node} has no step title`);
  return { node: start.node, title };
}

// ページ内の固定の関数（addInitScript に引数つきで渡す。文字列は評価しない）。MutationObserver の各回と導入の直後に「操作できる」の
// 述語を評価し、初めて真になった時点の performance.now()（ナビゲーションの開始が起点）を window.__operableAt に残す。
// data-mounted の最初の値と時刻も window.__mountedValue / __mountedAt に残す（参考値）。ポーリングでは測らない。
function installOperableObserver({ title, uid }) {
  const usable = (el) => {
    if (!el.isConnected || el.disabled) return false;
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  };
  const q = (id) => [...document.querySelectorAll(`[data-testid="${id}"]`)];
  const check = () => {
    const html = document.documentElement;
    if (!html) return;
    const mounted = html.getAttribute('data-mounted');
    if (mounted && window.__mountedValue === undefined) {
      window.__mountedValue = mounted;
      window.__mountedAt = performance.now();
    }
    if (window.__operableAt !== undefined || mounted !== 'true') return;
    const run = document.querySelector('[data-testid="run-status"]')?.getAttribute('data-state');
    const heading = document.querySelector('[data-testid="step-title"]')?.textContent ?? '';
    const word = document.querySelector('[data-testid="step-status"]')?.textContent ?? '';
    const logged = [...document.querySelectorAll('[data-testid="part-log-list"] li')].some((li) => li.getAttribute('data-step') === uid);
    const ports =
      q('choice-btn').some(usable) || q('approval-btn').some(usable) || (q('draft-input').some(usable) && q('submit-input').some(usable));
    if (run === 'idle' && heading === title && word !== '受付済み（確認待ち）' && !logged && ports) {
      window.__operableAt = performance.now();
    }
  };
  new MutationObserver(check).observe(document, { attributes: true, childList: true, subtree: true, characterData: true });
  check();
}

async function measureFirstPaintDirect(browser, dir, { channel, headless }) {
  const ir = JSON.parse(fs.readFileSync(`${dir}/ir/main.json`, 'utf8'));
  const start = directStartOf(ir, DIRECT_START);
  const nextTitle = (ir.nodes ?? []).find((n) => n?.uid === DIRECT_NEXT_NODE)?.step?.title;
  if (typeof nextTitle !== 'string' || nextTitle === '' || nextTitle === start.title) throw new Error('the next step title is unusable');
  const srv = await startServer({ dir, base: BASE, port: 0, gzip: true, gzipLevel: GZIP_LEVEL });
  try {
    const origin = new URL(srv.url).origin;
    const url = `${srv.url}?start=${encodeURIComponent(DIRECT_START)}`;
    const operable = [];
    const mounted = [];
    let wasmTransfer = 0;
    let irTransfer = 0;
    for (let i = 0; i < TRIALS; i++) {
      const { context, page } = await newPage(browser, true);
      try {
        await page.addInitScript(installOperableObserver, { title: start.title, uid: start.node });
        const wasmEnc = [];
        const irRes = [];
        const foreign = [];
        const errors = [];
        page.on('response', (res) => {
          const u = new URL(res.url());
          if (u.pathname.endsWith('.wasm')) wasmEnc.push(res.headers()['content-encoding'] ?? '');
          if (u.pathname.endsWith('/ir/main.json')) irRes.push(res.status());
        });
        page.on('request', (req) => {
          const u = req.url();
          if (!u.startsWith('data:') && !u.startsWith('blob:') && new URL(u).origin !== origin) foreign.push(u);
        });
        page.on('pageerror', (e) => errors.push(String(e.message).slice(0, 100)));
        await page.goto(url, { waitUntil: 'commit', timeout: NAV_TIMEOUT_MS });
        await page.waitForFunction(
          () => window.__operableAt !== undefined || (window.__mountedValue !== undefined && window.__mountedValue !== 'true'),
          null,
          { timeout: NAV_TIMEOUT_MS },
        );
        const r = await page.evaluate(() => {
          const res = performance.getEntriesByType('resource');
          const wasm = res.filter((e) => new URL(e.name).pathname.endsWith('.wasm'));
          const irs = res.filter((e) => new URL(e.name).pathname.endsWith('/ir/main.json'));
          return {
            mountedValue: window.__mountedValue,
            mountedAt: window.__mountedAt,
            operableAt: window.__operableAt,
            wasmEncoded: wasm.length === 1 ? wasm[0].encodedBodySize : null,
            irEncoded: irs.length === 1 ? irs[0].encodedBodySize : null,
            hooks: typeof window.__fandheTest,
            notices: document.querySelectorAll('[data-testid="shell-notice-item"]').length,
            title: document.querySelector('[data-testid="step-title"]')?.textContent ?? '',
          };
        });
        if (r.mountedValue !== 'true') throw new Error(`data-mounted is "${r.mountedValue}" (mount failed)`);
        if (!Number.isFinite(r.operableAt)) throw new Error('the page never became operable');
        if (!Number.isInteger(r.wasmEncoded) || r.wasmEncoded <= 0) throw new Error('could not read the wasm transfer size');
        if (!Number.isInteger(r.irEncoded) || r.irEncoded <= 0) throw new Error('could not read the IR transfer size');
        if (wasmEnc.length !== 1 || wasmEnc[0] !== 'gzip') throw new Error(`wasm was not served with Content-Encoding: gzip (${JSON.stringify(wasmEnc)})`);
        if (irRes.length !== 1 || irRes[0] !== 200) throw new Error(`ir/main.json was not fetched exactly once with 200 (${JSON.stringify(irRes)})`);
        if (foreign.length !== 0) throw new Error(`requests to other origins: ${foreign.length}`);
        if (errors.length !== 0) throw new Error(`page errors: ${errors.length}`);
        if (r.notices !== 0) throw new Error(`shell notices appeared: ${r.notices}`);
        if (r.hooks !== 'undefined') throw new Error('test hooks are present (not a delivery build)');
        if (r.title === nextTitle) throw new Error('the page is at the next step, not the start');
        // 10Mbps で wasm と IR を受け取る最短時間（ミリ秒）より速ければ、制限が効いていない。
        const floorMs = (((r.wasmEncoded + r.irEncoded) * 8) / 10e6) * 1000;
        if (r.operableAt < floorMs) {
          throw new Error(`operable (${r.operableAt} ms) is faster than the 10 Mbps transfer floor (${floorMs} ms): throttling is not in effect`);
        }
        operable.push(toMicros(r.operableAt, 'operable'));
        mounted.push(toMicros(r.mountedAt, 'mounted'));
        wasmTransfer = r.wasmEncoded;
        irTransfer = r.irEncoded;
        progress(`first-paint-direct trial ${i + 1}/${TRIALS}: operable ${r.operableAt.toFixed(1)} ms (mounted ${r.mountedAt.toFixed(1)} ms)`);
      } finally {
        await context.close();
      }
    }
    const j = operableJudgement(operable);
    return {
      measurements: [
        { name: 'first_paint_direct_operable_median', unit: 'us', value: j.medianUs },
        { name: 'first_paint_direct_operable_max', unit: 'us', value: j.maxUs },
        { name: 'first_paint_direct_mounted_median', unit: 'us', value: Math.round(median(mounted)) },
        { name: 'first_paint_direct_wasm_transfer', unit: 'bytes', value: wasmTransfer },
        { name: 'first_paint_direct_ir_transfer', unit: 'bytes', value: irTransfer },
      ],
      compression: {
        command: `tools/serve.mjs gzip level ${GZIP_LEVEL} (Content-Encoding: gzip)`,
        implementation: `node ${process.version} zlib ${process.versions.zlib}`,
      },
      conditions: {
        network: { down_mbps: 10, up_mbps: 10, latency_ms: LATENCY_MS },
        cpu_throttle: 1,
        cache: 'disabled',
        trials: TRIALS,
        base: BASE,
        viewport: { width: 1280, height: 720 },
        browser: { channel, headless },
        start: DIRECT_START,
        start_node: start.node,
        operable_definition: OPERABLE_DEFINITION,
        values_us: operable,
        mounted_values_us: mounted,
        threshold_us: FIRST_PAINT_DIRECT_THRESHOLD_US,
        judgement: j.judgement,
        scope_note:
          'Local subpath delivery, not the public site (DLV-6). The pack is a CH-0 to CH-3 shaped copy, not a real course. Compare only values with the same start, pack, browser channel and headless setting.',
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

// log-slope（TASK-232 1/3。EVT-26）の条件。段階は約 1,000 項目の畳み（TASK-360）の前後にまたがる。偶数なのは初期の logLen と偶奇を揃えるため。
// 窓 [n, n + 2*(20-1)] が 1,000 をまたがない（900 段は 900〜938、1100 段は 1100〜1138）。
const SLOPE_STEPS = [100, 200, 400, 600, 800, 900, 1100, 1200, 1600, 2000, 3000, 4000];
const SLOPE_TRIALS = 10;
const SLOPE_SAMPLES = 20;
const SLOPE_FOLD_TARGET = 1000;

// ページ内の 1 試行。段階を昇順に同じ context で進める。各段階でログを n まで時間を測らずに埋め、samples 回の click を 1 回ずつ測る。
// 各操作の直後に、ログの長さ・li の数・畳みの有無・最後のデータ行の変化を確かめ、外れたら throw する（何もしない再描画を速いと誤読しない）。
async function logSlopeInPage({ op, delta, steps, samples, startStep, target }) {
  const st = () => JSON.parse(window.__fandheTest.stateJson()).engine;
  const el = document.querySelector('[data-testid="raw-trigger"]');
  const list = document.querySelector('[data-testid="part-log-list"]');
  if (!el || !list) throw new Error('raw-trigger or part-log-list is missing');
  const rows = () => list.querySelectorAll(':scope > li').length;
  const folds = () => list.querySelectorAll(':scope > li[data-fold]').length;
  const lastEvent = () => {
    const data = list.querySelectorAll(':scope > li:not([data-fold])');
    return data.length ? data[data.length - 1].getAttribute('data-event') : null;
  };
  if (st().current !== startStep) throw new Error(`unexpected start step ${st().current} (not the minimal pack?)`);
  el.setAttribute('data-payload', JSON.stringify(op));
  const out = [];
  for (const n of steps) {
    const fillCount = (n - st().logLen) / delta;
    if (!Number.isInteger(fillCount) || fillCount < 0) throw new Error(`cannot fill the log to ${n}`);
    for (let i = 0; i < fillCount; i++) el.click();
    if (st().logLen !== n) throw new Error(`log length is ${st().logLen} after filling (expected ${n})`);
    const hi = n + delta * (samples - 1);
    const values = [];
    let rowsMin = Infinity;
    let rowsMax = 0;
    let foldedOps = 0;
    for (let s = 0; s < samples; s++) {
      const before = st().logLen;
      const rowsBefore = rows();
      const evBefore = lastEvent();
      if (before < n || before > hi) throw new Error(`log length ${before} is outside the window [${n}, ${hi}]`);
      const t0 = performance.now();
      el.click();
      const t1 = performance.now();
      const after = st().logLen;
      if (after !== before + delta) throw new Error(`log grew by ${after - before} (expected ${delta})`);
      if (document.querySelector('[data-list-error]')) throw new Error('a list limit notice appeared');
      const rowsAfter = rows();
      if (rowsAfter > target) throw new Error(`log rows ${rowsAfter} exceed ${target}`);
      if (folds() === 0) {
        if (rowsAfter !== rowsBefore + delta) throw new Error(`log rows grew by ${rowsAfter - rowsBefore} before folding (expected ${delta})`);
      } else {
        foldedOps++;
      }
      if (lastEvent() === evBefore) throw new Error('the last data row did not change (no real redraw?)');
      if (st().current !== startStep) throw new Error(`step moved to ${st().current}`);
      rowsMin = Math.min(rowsMin, rowsAfter);
      rowsMax = Math.max(rowsMax, rowsAfter);
      values.push(t1 - t0);
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
    out.push({ n, values, window: [n, hi], rowsMin, rowsMax, foldedOps });
  }
  return out;
}

async function measureLogSlope(browser, dir) {
  // 回線制限は掛けない。ブラウザ内の同期的な処理を測る。
  const srv = await startServer({ dir, base: BASE, port: 0, gzip: true, gzipLevel: GZIP_LEVEL });
  try {
    const perStep = SLOPE_STEPS.map(() => ({ values: [], rowsMin: Infinity, rowsMax: 0, foldedOps: 0 }));
    for (let i = 0; i < SLOPE_TRIALS; i++) {
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
        const res = await page.evaluate(logSlopeInPage, {
          op: LOG_OP, delta: LOG_DELTA, steps: SLOPE_STEPS, samples: SLOPE_SAMPLES, startStep: LOG_START_STEP, target: SLOPE_FOLD_TARGET,
        });
        if (!Array.isArray(res) || res.length !== SLOPE_STEPS.length) throw new Error('unexpected step count');
        res.forEach((r, k) => {
          if (r.n !== SLOPE_STEPS[k] || r.values.length !== SLOPE_SAMPLES) throw new Error('unexpected sample count');
          perStep[k].values.push(...r.values);
          perStep[k].rowsMin = Math.min(perStep[k].rowsMin, r.rowsMin);
          perStep[k].rowsMax = Math.max(perStep[k].rowsMax, r.rowsMax);
          perStep[k].foldedOps += r.foldedOps;
        });
        progress(`log-slope trial ${i + 1}/${SLOPE_TRIALS} done`);
      } finally {
        await context.close();
      }
    }
    const measurements = [];
    SLOPE_STEPS.forEach((n, k) => {
      const v = perStep[k].values;
      const tag = String(n).padStart(4, '0');
      const p95us = toMicros(p95(v), `log slope ${n} p95`);
      measurements.push(
        { name: `log_slope_${tag}_median`, unit: 'us', value: toMicros(median(v), `log slope ${n} median`) },
        { name: `log_slope_${tag}_p95`, unit: 'us', value: p95us },
        { name: `log_slope_${tag}_max`, unit: 'us', value: toMicros(Math.max(...v), `log slope ${n} max`) },
      );
      progress(`log-slope ${n}: p95 ${p95us} us (threshold ${LOG_THRESHOLD_US} us): ${p95us <= LOG_THRESHOLD_US ? 'within' : 'over'}`);
    });
    return {
      measurements,
      compression: {
        command: `tools/serve.mjs gzip level ${GZIP_LEVEL} (Content-Encoding: gzip)`,
        implementation: `node ${process.version} zlib ${process.versions.zlib}`,
      },
      conditions: {
        network: 'unthrottled',
        cache: 'default',
        cpu_throttle: 1,
        trials: SLOPE_TRIALS,
        samples_per_trial: SLOPE_SAMPLES,
        op: 'input /make-note Bad_Name (rejected; hidden raw-trigger click)',
        log_delta: LOG_DELTA,
        log_steps: SLOPE_STEPS,
        log_windows: SLOPE_STEPS.map((n) => [n, n + LOG_DELTA * (SLOPE_SAMPLES - 1)]),
        rows_observed: SLOPE_STEPS.map((n, k) => ({ n, rows_min: perStep[k].rowsMin, rows_max: perStep[k].rowsMax, folded_ops: perStep[k].foldedOps })),
        fold_target: SLOPE_FOLD_TARGET,
        threshold_p95_us: LOG_THRESHOLD_US,
        base: BASE,
        scope_note: 'minimal pack (min-3step); single Step, so only the head fold applies above 1,000; not representative of a full pack',
      },
    };
  } finally {
    await srv.close();
  }
}

// log-rounds（TASK-232 3/3。EVT-26）の条件。min-3step の st-1→st-2→st-3 を go_to で巡回し、(世代, Step) の別のグループを作って 2 周分の通しの形にする。
// 試作（2026-10-27）で、go_to は logLen を 2 増やすが li は増やさず、拒否される入力（text 'x'・/make-note Bad_Name のどちらも）は 3 Step のどこでも
// logLen と li を 2 ずつ増やすことを確かめた。ロード直後の logLen・li は 0。
const ROUNDS_OP = { input: { text: 'x' } };
const ROUNDS_STEPS = ['main/st-1', 'main/st-2', 'main/st-3'];
const ROUNDS_GROUPS = [6, 48]; // 2 周 × 3 Step、PoC-4 の 1 周約 24 Step × 2 周
const ROUNDS_STAGES = [500, 1200, 2000]; // 500 は PoC-6 の 2 周分の最大 506 件の代表値。判定はこの段
const ROUNDS_REPRESENTATIVE = 500;
const ROUNDS_TRIALS = 5;
const ROUNDS_SAMPLES = 40;
const ROUNDS_GO_TO_DELTA = 2;

// 段階 R・グループ数 G の割り当ての計画（純関数）。各グループは go_to の 1 回（logLen +2）と、拒否される入力 n_g 回（各 +2 件）からなる。
// 埋める入力の総数は (R - 2G)/2 で、端数は古い側から 1 回ずつ足す。窓 [R, R + 2*(samples-1)] の間、li の数が 1,000 をまたがないことを検査する。
export function planRoundsStage({ groups, rows, samples = ROUNDS_SAMPLES, delta = LOG_DELTA, target = SLOPE_FOLD_TARGET }) {
  if (!Number.isInteger(groups) || groups < 1) throw new Error('groups must be a positive integer');
  if (!Number.isInteger(rows) || rows % delta !== 0) throw new Error('rows must be a multiple of the delta');
  const fillTotal = (rows - ROUNDS_GO_TO_DELTA * groups) / delta;
  if (!Number.isInteger(fillTotal) || fillTotal < groups) throw new Error('rows are too few for one input per group');
  const base = Math.floor(fillTotal / groups);
  const rest = fillTotal % groups;
  const perGroup = Array.from({ length: groups }, (_, g) => base + (g < rest ? 1 : 0));
  const liStart = fillTotal * delta;
  const liEnd = liStart + delta * samples;
  if (liStart < target && liEnd > target) throw new Error(`stage ${rows} x ${groups} crosses the fold target`);
  return { perGroup, fillTotal, liStart, window: [rows, rows + delta * (samples - 1)], folded: liStart >= target };
}

// ページ内の 1 試行。plan.perGroup に従って groups 個のグループを時間を測らずに作り、最新のグループで samples 回の click を 1 回ずつ測る。
async function logRoundsInPage({ op, delta, steps, plan, samples, target }) {
  const st = () => JSON.parse(window.__fandheTest.stateJson()).engine;
  const el = document.querySelector('[data-testid="raw-trigger"]');
  const list = document.querySelector('[data-testid="part-log-list"]');
  if (!el || !list) throw new Error('raw-trigger or part-log-list is missing');
  const send = (p) => { el.setAttribute('data-payload', JSON.stringify(p)); el.click(); };
  const rows = () => list.querySelectorAll(':scope > li').length;
  const folds = () => list.querySelectorAll(':scope > li[data-fold]').length;
  const lastEvent = () => {
    const data = list.querySelectorAll(':scope > li:not([data-fold])');
    return data.length ? data[data.length - 1].getAttribute('data-event') : null;
  };
  const check = (c, msg) => { if (!c) throw new Error(msg); };
  check(st().current === steps[0] && st().logLen === 0 && rows() === 0, 'unexpected initial state (not the minimal pack?)');
  const gen0 = st().generation;
  let uid = steps[0];
  for (let g = 0; g < plan.perGroup.length; g++) {
    uid = steps[g % steps.length];
    const before = st().logLen;
    send({ control: { go_to: uid } });
    check(st().current === uid, `go_to ${uid} did not move`);
    check(st().logLen === before + 2, 'go_to did not add 2 log entries');
    for (let i = 0; i < plan.perGroup[g]; i++) send(op);
  }
  const start = plan.window[0];
  check(st().logLen === start, `log length is ${st().logLen} after building (expected ${start})`);
  check(st().generation > gen0, 'generation did not advance');
  const out = { values: [], rowsMin: Infinity, rowsMax: 0, foldedOps: 0, foldKinds: {}, generationEnd: st().generation };
  for (let s = 0; s < samples; s++) {
    const before = st().logLen;
    const rowsBefore = rows();
    const evBefore = lastEvent();
    check(before === start + delta * s, `log length ${before} is outside the window`);
    const t0 = performance.now();
    el.click();
    const t1 = performance.now();
    check(st().logLen === before + delta, `log grew by ${st().logLen - before} (expected ${delta})`);
    check(!document.querySelector('[data-list-error]') && !document.querySelector('[data-list-limit]'), 'a list limit notice appeared');
    const rowsAfter = rows();
    check(rowsAfter <= target, `log rows ${rowsAfter} exceed ${target}`);
    if (folds() === 0) check(rowsAfter === rowsBefore + delta, `log rows grew by ${rowsAfter - rowsBefore} before folding (expected ${delta})`);
    else out.foldedOps++;
    check(lastEvent() !== evBefore, 'the last data row did not change (no real redraw?)');
    check(st().current === uid, `step moved to ${st().current}`);
    out.rowsMin = Math.min(out.rowsMin, rowsAfter);
    out.rowsMax = Math.max(out.rowsMax, rowsAfter);
    out.values.push(t1 - t0);
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  list.querySelectorAll(':scope > li[data-fold]').forEach((li) => {
    const k = li.getAttribute('data-fold');
    out.foldKinds[k] = (out.foldKinds[k] ?? 0) + 1;
  });
  return out;
}

async function measureLogRounds(browser, dir) {
  // 回線制限は掛けない。ブラウザ内の同期的な処理を測る。
  const srv = await startServer({ dir, base: BASE, port: 0, gzip: true, gzipLevel: GZIP_LEVEL });
  try {
    const cells = [];
    for (const groups of ROUNDS_GROUPS) {
      for (const rows of ROUNDS_STAGES) {
        const plan = planRoundsStage({ groups, rows });
        const cell = { groups, rows, plan, values: [], rowsMin: Infinity, rowsMax: 0, foldedOps: 0, foldKinds: {}, generationEnd: 0 };
        for (let i = 0; i < ROUNDS_TRIALS; i++) {
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
            const res = await page.evaluate(logRoundsInPage, {
              op: ROUNDS_OP, delta: LOG_DELTA, steps: ROUNDS_STEPS, plan, samples: ROUNDS_SAMPLES, target: SLOPE_FOLD_TARGET,
            });
            if (!Array.isArray(res.values) || res.values.length !== ROUNDS_SAMPLES) throw new Error('unexpected sample count');
            cell.values.push(...res.values);
            cell.rowsMin = Math.min(cell.rowsMin, res.rowsMin);
            cell.rowsMax = Math.max(cell.rowsMax, res.rowsMax);
            cell.foldedOps += res.foldedOps;
            cell.foldKinds = res.foldKinds;
            cell.generationEnd = res.generationEnd;
          } finally {
            await context.close();
          }
        }
        progress(`log-rounds G=${groups} R=${rows}: trials done`);
        cells.push(cell);
      }
    }
    const measurements = [];
    for (const c of cells) {
      const tag = `g${String(c.groups).padStart(2, '0')}_${String(c.rows).padStart(4, '0')}`;
      const label = `log rounds ${tag}`;
      const p95us = toMicros(p95(c.values), `${label} p95`);
      measurements.push(
        { name: `log_rounds_${tag}_median`, unit: 'us', value: toMicros(median(c.values), `${label} median`) },
        { name: `log_rounds_${tag}_p95`, unit: 'us', value: p95us },
        { name: `log_rounds_${tag}_max`, unit: 'us', value: toMicros(Math.max(...c.values), `${label} max`) },
      );
      progress(`log-rounds G=${c.groups} R=${c.rows}: p95 ${p95us} us (threshold ${LOG_THRESHOLD_US} us): ${p95us <= LOG_THRESHOLD_US ? 'within' : 'over'}`);
    }
    return {
      measurements,
      compression: {
        command: `tools/serve.mjs gzip level ${GZIP_LEVEL} (Content-Encoding: gzip)`,
        implementation: `node ${process.version} zlib ${process.versions.zlib}`,
      },
      conditions: {
        network: 'unthrottled',
        cache: 'default',
        cpu_throttle: 1,
        trials: ROUNDS_TRIALS,
        samples_per_trial: ROUNDS_SAMPLES,
        op: 'input text x (rejected; hidden raw-trigger click); groups made by control go_to cycling st-1, st-2, st-3',
        log_delta: LOG_DELTA,
        laps: 2,
        group_counts: ROUNDS_GROUPS,
        row_stages: ROUNDS_STAGES,
        log_windows: cells.map((c) => ({ groups: c.groups, rows: c.rows, window: c.plan.window })),
        rows_observed: cells.map((c) => ({
          groups: c.groups, rows: c.rows, rows_min: c.rowsMin, rows_max: c.rowsMax, folded_ops: c.foldedOps,
          fold_kinds: c.foldKinds, generation_end: c.generationEnd,
        })),
        fold_target: SLOPE_FOLD_TARGET,
        threshold_p95_us: LOG_THRESHOLD_US,
        representative_stage: ROUNDS_REPRESENTATIVE,
        representative_source: 'docs/spec/03-poc/save-restore-versions/measurements/history-size.csv (2 laps: log_entries up to 506)',
        base: BASE,
        scope_note: 'minimal pack (min-3step) walked by go_to to form 6 or 48 (generation, Step) groups; log line mix (progress, choices) of a real pack is not modeled; not representative of a full pack',
      },
    };
  } finally {
    await srv.close();
  }
}

// ページ内: 時間を測らずにログを start まで埋める（隠し raw-trigger の click）。割り切れ・戻る向きの検査はここが唯一の実体。
async function fillLogInPage({ op, delta, start, startStep }) {
  const st = () => JSON.parse(window.__fandheTest.stateJson()).engine;
  const el = document.querySelector('[data-testid="raw-trigger"]');
  if (!el) throw new Error('raw-trigger is missing');
  const first = st();
  if (first.current !== startStep) throw new Error(`unexpected start step ${first.current} (not the minimal pack?)`);
  el.setAttribute('data-payload', JSON.stringify(op));
  const fillCount = (start - first.logLen) / delta;
  if (!Number.isInteger(fillCount) || fillCount < 0) throw new Error('cannot fill the log to the start length');
  for (let i = 0; i < fillCount; i++) el.click();
  if (st().logLen !== start) throw new Error(`log length is ${st().logLen} after filling (expected ${start})`);
  return st().logLen;
}

// ページ内: 送信ボタンへの click のキャプチャ段階のリスナ。click ごとに、キャプチャの受け取りと event.timeStamp を起点にして、
// 次の requestAnimationFrame の中の setTimeout(0) までの時間（ミリ秒）を window.__inputPaint へ積む。固定の関数だけを渡す。
function installInputPaintListener(submitSel) {
  window.__inputPaint = [];
  document.addEventListener(
    'click',
    (e) => {
      if (!(e.target instanceof Element) || !e.target.closest(submitSel)) return;
      const cap = performance.now();
      const stamp = e.timeStamp;
      const trusted = e.isTrusted;
      requestAnimationFrame(() => {
        setTimeout(() => {
          const end = performance.now();
          window.__inputPaint.push({ fromEvent: end - stamp, fromCapture: end - cap, trusted });
        }, 0);
      });
    },
    true,
  );
}

async function measureInputPaint(browser, dir) {
  // 回線制限は掛けない。ブラウザ内の処理を測る。
  const srv = await startServer({ dir, base: BASE, port: 0, gzip: true, gzipLevel: GZIP_LEVEL });
  try {
    const fromEvent = [];
    const fromCapture = [];
    for (let i = 0; i < INPUT_TRIALS; i++) {
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
        await page.evaluate(fillLogInPage, { op: LOG_OP, delta: INPUT_DELTA, start: INPUT_START, startStep: LOG_START_STEP });
        const snap = () =>
          page.evaluate(() => {
            const e = JSON.parse(window.__fandheTest.stateJson()).engine;
            return {
              logLen: e.logLen,
              current: e.current,
              rows: document.querySelectorAll('[data-testid="part-log-list"] > li').length,
              listError: document.querySelector('[data-list-error]') !== null,
            };
          });
        await page.fill(DRAFT_SEL, INPUT_DRAFT);
        await page.waitForTimeout(INPUT_SETTLE_MS);
        const base = await snap();
        if (base.logLen !== INPUT_START) throw new Error(`typing the draft changed the log (${base.logLen})`);
        await page.evaluate(installInputPaintListener, SUBMIT_SEL);
        let prev = base;
        for (let s = 0; s < INPUT_SAMPLES; s++) {
          if (prev.logLen < INPUT_WINDOW[0] || prev.logLen > INPUT_WINDOW[1]) throw new Error(`log length ${prev.logLen} is outside the window`);
          await page.click(SUBMIT_SEL);
          await page.waitForFunction((n) => window.__inputPaint.length >= n, s + 1, { timeout: INPUT_WAIT_MS });
          await page.waitForFunction(
            (want) => JSON.parse(window.__fandheTest.stateJson()).engine.logLen === want,
            prev.logLen + INPUT_DELTA,
            { timeout: INPUT_WAIT_MS },
          );
          const now = await snap();
          if (now.rows !== prev.rows + INPUT_DELTA) throw new Error(`log rows grew by ${now.rows - prev.rows} (expected ${INPUT_DELTA})`);
          if (now.current !== LOG_START_STEP) throw new Error(`step moved to ${now.current}`);
          if (now.listError) throw new Error('a list limit notice appeared');
          const draft = await page.inputValue(DRAFT_SEL);
          if (draft !== INPUT_DRAFT) throw new Error(`the draft changed to ${JSON.stringify(draft)} (the next click would mean something else)`);
          prev = now;
          await page.waitForTimeout(INPUT_GAP_MS);
        }
        if (prev.logLen !== INPUT_START + INPUT_DELTA * INPUT_SAMPLES) throw new Error('unexpected end state');
        const got = await page.evaluate(() => window.__inputPaint);
        if (!Array.isArray(got) || got.length !== INPUT_SAMPLES) throw new Error('unexpected sample count');
        for (const g of got) {
          if (g.trusted !== true) throw new Error('a click was not trusted (isTrusted false)');
          if (!Number.isFinite(g.fromEvent) || !Number.isFinite(g.fromCapture) || g.fromCapture < 0 || g.fromEvent < g.fromCapture) {
            throw new Error(`inconsistent timing ${JSON.stringify(g)}`);
          }
        }
        fromEvent.push(...got.map((g) => g.fromEvent));
        fromCapture.push(...got.map((g) => g.fromCapture));
        progress(`input-paint trial ${i + 1}/${INPUT_TRIALS}: median ${(median(got.map((g) => g.fromEvent)) * 1000).toFixed(1)} us`);
      } finally {
        await context.close();
      }
    }
    const p95us = toMicros(p95(fromEvent), 'input paint p95');
    progress(`input-paint p95 ${p95us} us (threshold ${INPUT_THRESHOLD_US} us): ${p95us <= INPUT_THRESHOLD_US ? 'within' : 'over'}`);
    return {
      measurements: [
        { name: 'input_paint_median', unit: 'us', value: toMicros(median(fromEvent), 'input paint median') },
        { name: 'input_paint_p95', unit: 'us', value: p95us },
        { name: 'input_paint_max', unit: 'us', value: toMicros(Math.max(...fromEvent), 'input paint max') },
        { name: 'input_paint_capture_p95', unit: 'us', value: toMicros(p95(fromCapture), 'input paint capture p95') },
      ],
      compression: {
        command: `tools/serve.mjs gzip level ${GZIP_LEVEL} (Content-Encoding: gzip)`,
        implementation: `node ${process.version} zlib ${process.versions.zlib}`,
      },
      conditions: {
        network: 'unthrottled',
        cache: 'default',
        cpu_throttle: 1,
        trials: INPUT_TRIALS,
        samples_per_trial: INPUT_SAMPLES,
        op: 'trusted click (CDP) on the submit button with the rejected draft /make-note Bad_Name',
        log_delta: INPUT_DELTA,
        log_window: INPUT_WINDOW,
        threshold_p95_us: INPUT_THRESHOLD_US,
        origin: 'event.timeStamp',
        end: 'rAF + setTimeout(0)',
        gap_ms: INPUT_GAP_MS,
        settle_ms: INPUT_SETTLE_MS,
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
  if (!['first-paint', 'first-paint-direct', 'redraw', 'log-redraw', 'log-slope', 'log-rounds', 'input-paint', 'view-redraw', 'product-redraw', 'view-redraw-cpu4x', 'product-redraw-cpu4x'].includes(values.kind) || !values.dir) {
    console.error('error: --kind (first-paint|first-paint-direct|redraw|log-redraw|log-slope|log-rounds|input-paint|view-redraw|product-redraw|view-redraw-cpu4x|product-redraw-cpu4x) and --dir are required');
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
    const runs = {
      'first-paint': measureFirstPaint,
      'first-paint-direct': (b, d) => measureFirstPaintDirect(b, d, { channel: launchOptions.channel ?? 'chromium', headless: launchOptions.headless ?? true }),
      redraw: measureRedraw, 'log-redraw': measureLogRedraw, 'log-slope': measureLogSlope,
      'log-rounds': measureLogRounds, 'input-paint': measureInputPaint,
      'view-redraw': (b, d) => measureViewRedraw(b, d, { newPage, installMountObserver, median, p95, toMicros, progress }),
      'product-redraw': (b, d) => measureProductRedraw(b, d, { newPage, installMountObserver, median, p95, toMicros, progress }),
      'view-redraw-cpu4x': (b, d) => measureViewRedrawCpu4x(b, d, { newPage, installMountObserver, median, p95, toMicros, progress, calibrateAndThrottle, calibrationInfo }),
      'product-redraw-cpu4x': (b, d) => measureProductRedrawCpu4x(b, d, { newPage, installMountObserver, median, p95, toMicros, progress, calibrateAndThrottle, calibrationInfo }),
    };
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
