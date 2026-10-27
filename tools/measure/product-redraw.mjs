// 本体の画面（製品の EngineHost と ArtifactPart の view()）の左の成果物の再描画の時間を測る実体（TASK-485 2/3。ART-26。
// tools/measure/browser.mjs の --kind product-redraw から呼ばれる）。1/3（view-redraw.mjs。代役の部品の画面）の続きで、条件は同じ
// （基準端末 A の Chrome〔ヘッドレス〕、CPU の減速なし、10 試行 × 20 操作 = 200 値、p95 は nearest-rank、閾値 16ms）。
//
// 測るもの: 選択のボタン（data-testid="view-select-button"）の el.click() の前後の performance.now() の差。click の委譲・属性の読み取り・
// update・view() の構築・keyed list の差分・DOM への反映を含む同期区間で、log-redraw と同じ形。view-redraw（委譲を含まない）の値とは
// 測る量が違うため直接は比べない。操作は switch（別の種類から対象へ。行がすべて入れ替わる）だけ。製品には前後の切り替え（toggle）が無く、
// 代わりの操作は作らない（該当なしとして記録する）。
// 限界: 種類ごとの描画部品（diff_view など）は製品の view() に結線されていない。測る行は世界状態の経路の行で、文書のブロックでも
// 差分の hunk でもない。結線した後に測り直しが要る（docs/views.md の 1 節）。CPU 4 倍（ART-15）は別のイシュー。
//
// 安全: ページへ渡すのは固定の関数で、データは引数で渡す。ボタンは getAttribute の完全一致で選び、文字列から選択子を組み立てない。
// 配信は tools/serve.mjs の 127.0.0.1 だけ。外部へは通信しない。
// 呼び出し元: tools/measure/browser.mjs（統計・起動の共通部は deps で受け取る）。
// 回帰テスト: tools/measure/browser.test.mjs（純関数）、tools/browser/product-redraw.test.mjs（ページ内の関数）、scripts/test-measure.sh。
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { startServer } from '../serve.mjs';

export const PR_TRIALS = 10;
export const PR_SAMPLES = 20;
export const PR_THRESHOLD_US = 16000;
export const PR_GAP_MS = 20;
const BASE = '/subpath/';
const GZIP_LEVEL = 9;
const NAV_TIMEOUT_MS = 60000;
const SAMPLE_DIFF = new URL('../../testdata/artifact-view-parts/test-data/sample/diff.json', import.meta.url);

// 組。judged は計画に大きさが書かれた 3 種（判定の対象）。残りは参考。from は 1 つ前の種類（巡回）。
const ORDER = ['file_tree', 'document', 'diff', 'task_tree', 'card_list', 'check_list', 'app_demo'];
const SIZES = [
  ['task_tree', 50, true],
  ['document', 200, true],
  ['diff', 500, true],
  ['file_tree', 200, false],
  ['card_list', 20, false],
  ['check_list', 20, false],
  ['app_demo', 9, false],
];

export function cases() {
  return SIZES.map(([kind, rows, judged]) => ({
    kind,
    rows,
    judged,
    from: ORDER[(ORDER.indexOf(kind) + ORDER.length - 1) % ORDER.length],
  }));
}

export function measurementName(c, stat) {
  return `product_redraw_${c.kind}_${c.rows}_switch_${stat}`;
}

// 判定（純関数）。p95Us: `${kind}:${rows}` -> p95（us）。toggle が無いため部分的な判定（partial）。
export function judge(p95Us, thresholdUs = PR_THRESHOLD_US) {
  const verdict = {};
  for (const c of cases()) {
    const key = `${c.kind}:${c.rows}`;
    const p = p95Us[key];
    if (!Number.isFinite(p)) throw new Error(`missing p95 for ${key}`);
    verdict[c.kind] = { judged: c.judged, rows: c.rows, switch_p95_us: p, within: p <= thresholdUs, partial: true };
  }
  return verdict;
}

// sample の差分の行数を Node で数える（値を書き込まず、ファイルから数える）。
export function sampleDiffRows() {
  const rows = JSON.parse(fs.readFileSync(fileURLToPath(SAMPLE_DIFF), 'utf8')).rows;
  if (!Array.isArray(rows) || rows.length === 0) throw new Error('sample diff rows are missing');
  const hunk = rows.filter((r) => r && r.kind === 'hunk').length;
  return { total: rows.length, hunk, line: rows.length - hunk };
}

// ページ内の 1 試行。固定の関数で、データは引数で受け取る。時間を測るのは対象のボタンの click 1 回の同期処理だけ。
// 各操作の後に、選んだ list の行数・ほかの list が空で hidden・全行が指せる行の属性を持つこと・状態が変わらないこと・root が
// 置き換わっていないことを検査する。外れたら例外にする（何もしない更新を速いと誤読しない）。
export async function productRedrawInPage({ kind, from, rows, samples, gapMs }) {
  const api = window.__fandheTest;
  const part = () => document.querySelector('#app [data-testid="part-artifact-view"]');
  const part0 = part();
  if (!part0) throw new Error('artifact view part is missing');
  const button = (k) => {
    const found = [...document.querySelectorAll('#app [data-testid="view-select-button"]')].filter(
      (b) => b.getAttribute('data-payload') === k,
    );
    if (found.length !== 1) throw new Error(`select button for ${k}: ${found.length}`);
    return found[0];
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const snap = () => {
    const s = JSON.parse(api.stateJson());
    return `${s.engine.logLen}|${s.engine.current}`;
  };
  const state0 = snap();
  const check = () => {
    let found = false;
    for (const l of document.querySelectorAll('#app [data-testid="view-list"]')) {
      const items = [...l.querySelectorAll(':scope > li')];
      if (l.getAttribute('data-view') === kind) {
        found = true;
        if (items.length !== rows) throw new Error(`${kind}: expected ${rows} items, got ${items.length}`);
        if (l.hasAttribute('hidden')) throw new Error('selected list is hidden');
        const pointed = items.filter(
          (li) => li.hasAttribute('data-action') && li.hasAttribute('data-payload') && li.hasAttribute('data-row') && li.hasAttribute('tabindex'),
        ).length;
        if (pointed !== rows) throw new Error(`${kind}: ${pointed} of ${rows} rows have the pointing attributes`);
      } else if (l.children.length !== 0 || !l.hasAttribute('hidden')) {
        throw new Error('a list that is not selected is not empty and hidden');
      }
    }
    if (!found) throw new Error('selected list is missing');
    if (document.querySelector('#app [data-list-error], #app [data-list-limit], #app [data-fold]')) {
      throw new Error('a list error, limit or fold marker is shown');
    }
    if (snap() !== state0) throw new Error('engine state changed');
    if (part() !== part0) throw new Error('artifact view part was replaced');
  };
  const values = [];
  for (let i = 0; i < samples; i++) {
    button(from).click();
    await sleep(gapMs);
    const target = button(kind);
    const t0 = performance.now();
    target.click();
    const t1 = performance.now();
    check();
    values.push(t1 - t0);
    await sleep(gapMs);
  }
  return values;
}

export function trialRotation(trial, total) {
  return trial % total;
}

export async function measureProductRedraw(browser, dir, deps) {
  const { newPage, installMountObserver, median, p95, toMicros, progress } = deps;
  const list = cases();
  const diffRows = sampleDiffRows();
  const srv = await startServer({ dir, base: BASE, port: 0, gzip: true, gzipLevel: GZIP_LEVEL });
  try {
    const all = {};
    for (let t = 0; t < PR_TRIALS; t++) {
      for (let k = 0; k < list.length; k++) {
        const c = list[(k + trialRotation(t, list.length)) % list.length];
        const { context, page } = await newPage(browser, false);
        try {
          await page.addInitScript(installMountObserver);
          await page.goto(`${srv.url}?fixture=artifact-redraw-${c.rows}`, { waitUntil: 'commit', timeout: NAV_TIMEOUT_MS });
          await page.waitForFunction(() => window.__mountedValue !== undefined, null, { timeout: NAV_TIMEOUT_MS });
          const state = await page.evaluate(() => ({
            mounted: window.__mountedValue,
            hooks: typeof window.__fandheTest?.stateJson === 'function',
          }));
          if (state.mounted !== 'true') throw new Error(`data-mounted is "${state.mounted}" (mount failed)`);
          if (!state.hooks) throw new Error('window.__fandheTest.stateJson is missing (not a test-hooks build?)');
          const v = await page.evaluate(productRedrawInPage, {
            kind: c.kind, from: c.from, rows: c.rows, samples: PR_SAMPLES, gapMs: PR_GAP_MS,
          });
          if (!Array.isArray(v) || v.length !== PR_SAMPLES) throw new Error(`unexpected sample count: ${c.kind}`);
          (all[`${c.kind}:${c.rows}`] ??= []).push(...v);
        } finally {
          await context.close();
        }
      }
      progress(`product-redraw trial ${t + 1}/${PR_TRIALS} done`);
    }
    const measurements = [];
    const p95Us = {};
    for (const c of list) {
      const key = `${c.kind}:${c.rows}`;
      const v = all[key];
      if (!v || v.length !== PR_TRIALS * PR_SAMPLES) throw new Error(`unexpected value count: ${key}`);
      const p = toMicros(p95(v), `${key} p95`);
      p95Us[key] = p;
      measurements.push({ name: measurementName(c, 'median'), unit: 'us', value: toMicros(median(v), `${key} median`) });
      measurements.push({ name: measurementName(c, 'p95'), unit: 'us', value: p });
      measurements.push({ name: measurementName(c, 'max'), unit: 'us', value: toMicros(Math.max(...v), `${key} max`) });
      progress(`product-redraw ${key}: p95 ${p} us`);
    }
    const judgement = judge(p95Us);
    for (const [kind, j] of Object.entries(judgement)) {
      progress(`product-redraw ${kind}${j.judged ? '' : ' (reference)'}: ${j.within ? 'within' : 'over'} ${PR_THRESHOLD_US} us`);
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
        trials: PR_TRIALS,
        samples_per_trial: PR_SAMPLES,
        threshold_p95_us: PR_THRESHOLD_US,
        op_trigger: 'click on the view-select-button (includes click delegation, attribute reads, update, view() build, keyed list diff and DOM apply; synchronous)',
        ops: { switch: 'measured', toggle: 'not_applicable (no before/after toggle in the product view())' },
        switch_from: Object.fromEntries(list.map((c) => [c.kind, c.from])),
        sizes: Object.fromEntries(list.map((c) => [c.kind, { fixture: `artifact-redraw-${c.rows}`, rows: c.rows }])),
        judged: list.filter((c) => c.judged).map((c) => c.kind),
        judgement,
        point_attrs: 'all rows (7 kinds declared on main/st-1)',
        seed: { pack_id: 'artifact-redraw', path_shape: 'p/{i:04}', step: 'main/st-1' },
        sample_diff_rows: diffRows,
        trial_rotation: 'case order rotated by the trial index; a new context per case',
        base: BASE,
        scope_note: 'product view(); per-kind renderers are not wired, rows are world-state paths; not comparable with view-redraw',
      },
    };
  } finally {
    await srv.close();
  }
}
