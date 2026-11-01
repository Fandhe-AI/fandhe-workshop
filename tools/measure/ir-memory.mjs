// 巨大な文字列を持つ IR を配信する設定の画面で開いたときの、renderer の RSS の最大値を測る実体（TASK-163 1/2。SAFE-25・D-45。
// tools/measure/browser.mjs の --kind ir-memory から呼ばれる）。親の #860 が基準端末 A（Apple シリコンの Mac）で人が実測するための道具で、
// ここでは Linux・macOS の両方で動く形にする。基準端末 A での実測と記録・低メモリ端末での測定・上限を外した場合の再現は範囲外。
//
// 測るもの: 1,990 万文字の `pack.title`／ヒントの本文を持つ IR（と比較用の普通の IR）を開いた間の、renderer プロセスの RSS の合計の最大値。
// 文字列の長さの上限（MAX_STRING_CHARS = 100,000）により、巨大な値は表示の前に `string_too_long` で断られる。したがって測る値は
// 「断った後」（対策後）の値で、上限を外した値ではない。ヒントの操作は断られた画面に無いため、ヒントは開かない。
// 標本: 100ms ごとに CDP の SystemInfo.getProcessInfo で renderer の pid を取り、`ps -o pid=,rss= -p <pids>` で RSS（KiB）を読む
// （出典は仕様の試作 browser-test-poc7.mjs の rssSampler）。ps は execFile に配列の引数で渡し、shell を通さない。
// 試行ごとに新しいブラウザを起動する（getProcessInfo はブラウザ全体の値で、前の試行の renderer を混ぜないため）。
// crash: page の crash を数える。crash が 1 件以上なら呼び出し元（browser.mjs）が終了コード 3 にする（JSON は出す）。
// fail-closed: 標本が 0 件・ps の行が読めない・理由が期待と違う・IR の大きさが前提を外れる、のいずれかは例外にする。
// 公開に向かない値（pid・パス・ホスト名・IR の本文）は結果に入れない。配信は 127.0.0.1 だけで、配信元以外への通信は 1 件でもあれば失敗。
// 呼び出し元: tools/measure/browser.mjs（playwright の chromium などは deps で受け取り、ここでは import しない）。
// 回帰テスト: tools/measure/ir-memory.test.mjs（純関数）、scripts/test-measure-ir-memory.sh。
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFile } from 'node:child_process';
import { startServer } from '../serve.mjs';

export const MAX_IR_BYTES = 20_971_520; // contract の MAX_IR_BYTES（D-44）
export const HUGE_CHARS = 19_900_000;
export const EXPECTED_REASON = 'string_too_long';
export const CASE_NAMES = ['base', 'pack_title', 'hint_text'];
export const TRIALS = 3;
export const SAMPLE_INTERVAL_MS = 100;
export const SETTLE_MS = 2000;
const MOUNT_TIMEOUT_MS = 120000;
const BASE = '/subpath/';
const GZIP_LEVEL = 9;
const HINT_ID = 'h-1-1';
const NOTICE = '[data-testid="shell-notice-item"][data-notice-kind="pack-unloadable"]';
const STEP_TITLE = '[data-testid="step-title"]';

// ps の出力（`pid=,rss=` で見出しなし。Linux の procps も macOS の BSD ps も rss は KiB）から、要求した pid の RSS（KiB）の合計を返す。
// 要求外の pid の行は無視し、読めない行は例外にする。消えた pid は行が出ないので合計に入らない。
export function parsePsRss(stdout, wantedPids) {
  const wanted = new Set(wantedPids);
  let total = 0;
  let seen = 0;
  for (const line of String(stdout).split('\n')) {
    if (line.trim() === '') continue;
    const m = /^\s*(\d+)\s+(\d+)\s*$/.exec(line);
    if (!m) throw new Error('unreadable ps output line');
    if (!wanted.has(Number(m[1]))) continue;
    total += Number(m[2]);
    seen += 1;
  }
  return { total, seen };
}

// getProcessInfo の応答から renderer の pid だけを数値で取り出す。
export function rendererPids(processInfo) {
  const list = processInfo && Array.isArray(processInfo.processInfo) ? processInfo.processInfo : [];
  return list.filter((p) => p && p.type === 'renderer' && Number.isInteger(p.id) && p.id > 0).map((p) => p.id);
}

// 場合ごとの IR を作る。base は無変更。巨大な値は ASCII の 'x' の繰り返しで、長さとバイト数が前提内であることを確かめる。
export function buildCaseIr(baseIrText, caseName) {
  if (!CASE_NAMES.includes(caseName)) throw new Error(`unknown case: ${caseName}`);
  if (caseName === 'base') {
    return { text: baseIrText, field: 'none', chars: 0, bytes: Buffer.byteLength(baseIrText) };
  }
  const ir = JSON.parse(baseIrText);
  const huge = 'x'.repeat(HUGE_CHARS);
  let field;
  if (caseName === 'pack_title') {
    if (!ir.pack || typeof ir.pack.title !== 'string') throw new Error('base IR has no pack.title');
    ir.pack.title = huge;
    field = 'pack.title';
  } else {
    if (!ir.hints || !ir.hints[HINT_ID] || typeof ir.hints[HINT_ID].text !== 'string') throw new Error('base IR has no hint text to replace');
    ir.hints[HINT_ID].text = huge;
    field = `hints.${HINT_ID}.text`;
  }
  const text = JSON.stringify(ir);
  const bytes = Buffer.byteLength(text);
  if ([...huge].length !== HUGE_CHARS) throw new Error('unexpected string length');
  if (bytes > MAX_IR_BYTES) throw new Error('generated IR exceeds MAX_IR_BYTES');
  return { text, field, chars: HUGE_CHARS, bytes };
}

function medianOf(values) {
  const s = [...values].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 === 1 ? s[m] : (s[m - 1] + s[m]) / 2;
}

// 場合ごとの試行の最大値（KiB）から measurements[] を作る。値は正の整数に限る（様式 v1）。
export function summarizeCases(results) {
  const out = [];
  for (const r of results) {
    if (!Array.isArray(r.peaks) || r.peaks.length === 0) throw new Error(`no peaks for ${r.name}`);
    const max = Math.max(...r.peaks);
    const med = Math.round(medianOf(r.peaks));
    if (!(max > 0) || !(med > 0) || !Number.isInteger(max)) throw new Error(`non-positive RSS for ${r.name}`);
    out.push({ name: `ir_memory_${r.name}_renderer_rss_max`, unit: 'KiB', value: max });
    out.push({ name: `ir_memory_${r.name}_renderer_rss_median`, unit: 'KiB', value: med });
  }
  return out;
}

// crash の件数から browser.mjs の終了コードを決める（0 = crash なし / 3 = crash あり。JSON は出す）。
export function exitCodeFor(crashesTotal) {
  return crashesTotal > 0 ? 3 : 0;
}

// 落ちなかった試行の観察が、場合ごとの期待と合うか。合わなければ理由の文字列、合えば null（前提の違う値を記録に貼らせない）。
export function checkExpectation(caseName, obs) {
  if (obs.crashed) return null;
  if (caseName === 'base') {
    if (obs.notices !== 0) return 'base: unexpected notice';
    if (!(obs.titles >= 1)) return 'base: no step title (blank screen)';
    return null;
  }
  if (obs.notices !== 1) return `${caseName}: expected exactly one notice, got ${obs.notices}`;
  if (obs.reason !== EXPECTED_REASON) return `${caseName}: unexpected refusal reason`;
  return null;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function psRss(pids) {
  return new Promise((resolve, reject) => {
    execFile('ps', ['-o', 'pid=,rss=', '-p', pids.join(',')], { encoding: 'utf8' }, (err, stdout) => {
      // 消えた pid があると ps は 1 で終わるが、残りの行は stdout にある。それ以外の失敗は失敗にする。
      if (err && err.code !== 1) reject(new Error('ps failed'));
      else resolve(stdout || '');
    });
  });
}

async function sampleRss(cdp) {
  const pids = rendererPids(await cdp.send('SystemInfo.getProcessInfo'));
  if (pids.length === 0) return null;
  const { total, seen } = parsePsRss(await psRss(pids), pids);
  return seen === 0 ? null : total;
}

async function runTrial(deps, url, origin) {
  const { chromium, launchOptions } = deps;
  const browser = await chromium.launch(launchOptions);
  let stop = false;
  let samplerDone = Promise.resolve();
  try {
    const cdp = await browser.newBrowserCDPSession();
    const context = await browser.newContext();
    const page = await context.newPage();
    let crashes = 0;
    let external = 0;
    page.on('crash', () => { crashes += 1; });
    await page.route('**/*', (route) => {
      if (route.request().url().startsWith(origin)) return route.continue();
      external += 1;
      return route.abort();
    });
    let peak = 0;
    let samples = 0;
    let samplerError = null;
    samplerDone = (async () => {
      while (!stop) {
        try {
          const v = await sampleRss(cdp);
          if (v !== null) { samples += 1; if (v > peak) peak = v; }
        } catch (e) {
          samplerError = e;
          return;
        }
        await sleep(SAMPLE_INTERVAL_MS);
      }
    })();
    let crashed = false;
    try {
      await page.goto(url, { waitUntil: 'commit', timeout: MOUNT_TIMEOUT_MS });
      await page.waitForSelector('html[data-mounted="true"]', { state: 'attached', timeout: MOUNT_TIMEOUT_MS });
      await sleep(SETTLE_MS);
      // 待機中の crash はイベントで数えるだけで例外にならないため、DOM を読む前に観察状態を確かめる。
      if (crashes > 0) crashed = true;
    } catch (e) {
      await sleep(SAMPLE_INTERVAL_MS * 2);
      if (crashes === 0) throw new Error(`page did not mount: ${String(e.message).split('\n')[0]}`);
      crashed = true;
    }
    let notices = 0;
    let reason = null;
    let titles = 0;
    if (!crashed) {
      try {
        ({ notices, reason, titles } = await page.evaluate(([n, t]) => {
          const items = [...document.querySelectorAll(n)];
          return { notices: items.length, reason: items.length === 1 ? items[0].getAttribute('data-notice-reason') : null, titles: document.querySelectorAll(t).length };
        }, [NOTICE, STEP_TITLE]));
      } catch (e) {
        // 取得との競合で crash した場合は、イベントが届くのを少し待って crash として記録する。crash でなければ失敗にする。
        await sleep(SAMPLE_INTERVAL_MS * 2);
        if (crashes === 0) throw e;
        crashed = true;
      }
    }
    stop = true;
    await samplerDone;
    if (samplerError) throw samplerError;
    if (external > 0) throw new Error('request to a non-origin host was attempted');
    if (samples === 0) throw new Error('no renderer RSS sample was taken');
    return { peak, crashes, crashed: crashed || crashes > 0, notices, reason, titles };
  } finally {
    stop = true;
    await samplerDone.catch(() => {});
    await browser.close().catch(() => {});
  }
}

// deps: { chromium, launchOptions, progress }。dir は IR_FILE つきの配信する設定の配信物（ir/main.json を持つ）。
export async function measureIrMemory(dir, deps) {
  const { progress } = deps;
  const baseIrPath = path.join(dir, 'ir', 'main.json');
  if (!fs.existsSync(baseIrPath)) throw new Error('ir/main.json is missing (build with IR_FILE)');
  const baseIrText = fs.readFileSync(baseIrPath, 'utf8');
  const results = [];
  const cases = [];
  for (const name of CASE_NAMES) {
    const built = buildCaseIr(baseIrText, name);
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-ir-memory.'));
    let srv;
    try {
      const copy = path.join(tmp, 'web');
      fs.cpSync(dir, copy, { recursive: true });
      fs.writeFileSync(path.join(copy, 'ir', 'main.json'), built.text);
      srv = await startServer({ dir: copy, base: BASE, port: 0, gzip: true, gzipLevel: GZIP_LEVEL });
      const origin = new URL(srv.url).origin;
      const peaks = [];
      let crashes = 0;
      let last = null;
      for (let i = 0; i < TRIALS; i += 1) {
        const obs = await runTrial(deps, srv.url, origin);
        const bad = checkExpectation(name, obs);
        if (bad) throw new Error(bad);
        peaks.push(obs.peak);
        crashes += obs.crashes;
        last = obs;
        progress(`ir-memory ${name} trial ${i + 1}/${TRIALS}: peak ${obs.peak} KiB, crashes ${obs.crashes}`);
      }
      results.push({ name, peaks });
      cases.push({
        name,
        field: built.field,
        chars: built.chars,
        ir_bytes: built.bytes,
        ir_sha256: crypto.createHash('sha256').update(built.text).digest('hex'),
        crashes,
        notices: last.notices,
        reason: last.reason,
        trial_peaks_kib: peaks,
      });
    } finally {
      if (srv) await srv.close();
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  }
  const crashesTotal = cases.reduce((a, c) => a + c.crashes, 0);
  return {
    measurements: summarizeCases(results),
    compression: {
      command: `tools/serve.mjs gzip level ${GZIP_LEVEL} (Content-Encoding: gzip)`,
      implementation: `node ${process.version} zlib ${process.versions.zlib}`,
    },
    conditions: {
      trials: TRIALS,
      sample_interval_ms: SAMPLE_INTERVAL_MS,
      settle_ms: SETTLE_MS,
      source: 'CDP SystemInfo.getProcessInfo (type=renderer) + ps -o pid=,rss=',
      aggregate: 'sum of renderer processes per sample, peak per trial',
      expected_reason: EXPECTED_REASON,
      max_ir_bytes: MAX_IR_BYTES,
      crashes_total: crashesTotal,
      cases,
      low_memory_device: 'unverified',
      base: BASE,
      scope_note: 'value after the refusal (string_too_long), not with the limit removed; hints are not opened; low-memory devices not verified',
    },
  };
}
