// 本体の画面（製品の EngineHost と ArtifactPart の view()）の左の成果物の再描画の時間を測る実体（TASK-485 2/3。ART-26。
// tools/measure/browser.mjs の --kind product-redraw から呼ばれる）。1/3（view-redraw.mjs。代役の部品の画面）の続きで、条件は同じ
// （基準端末 A の Chrome〔ヘッドレス〕、CPU の減速なし、10 試行 × 20 操作 = 200 値、p95 は nearest-rank、閾値 16ms）。
//
// 測るもの: 選択のボタン（data-testid="view-select-button"）の el.click() の前後の performance.now() の差。click の委譲・属性の読み取り・
// update・view() の構築・keyed list の差分・DOM への反映を含む同期区間で、log-redraw と同じ形。view-redraw（委譲を含まない）の値とは
// 測る量が違うため直接は比べない。操作は switch（別の種類から対象へ。行がすべて入れ替わる）だけ。製品には前後の切り替え（toggle）が無く、
// 代わりの操作は作らない（該当なしとして記録する）。
// 限界: 種類ごとの描画部品（diff_view など）は製品の view() に結線済み（2026-11-05。#2062〜#2068）だが、この測定（product-redraw）の行は属性の無い
// 世界状態の経路の行で、文書のブロックでも差分の hunk でもない。各部品の実際の形は通らない（docs/views.md の 1 節。測り直しは docs/testing.md 5.471）。
// 実際の形の行（hunk・dir と file・入れ子・文書の本文など）の測り直しは measureProductRedrawShaped（--kind product-redraw-shaped。#2074。docs/testing.md 5.472）。CPU 4 倍の記録（ART-15。TASK-485 3/3）は measureProductRedrawCpu4x で、判定に使わない。
// 形つきの固定データでの CPU 4 倍の記録は measureProductRedrawCpu4x の { shaped: true }（--kind product-redraw-shaped-cpu4x。#2127。記録だけ）。
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

// 形つきの参考の変種（判定に入れない。#2127）。文書の全 200 行に本文を付けた固定データ。cases() には足さない
// （3 つの種別と回帰テストが cases() を共有するため）。
export function shapedReferenceCases() {
  return [{ kind: 'document', rows: 200, judged: false, variant: 'body_all', from: 'file_tree' }];
}

// 集計の鍵。参考の変種は同じ kind・rows の組と衝突しないよう変種名を含める。
export function caseKey(c) {
  return c.variant ? `${c.kind}_${c.variant}:${c.rows}` : `${c.kind}:${c.rows}`;
}

// カード・チェックの形つきの固定データの状態のバッジの文字の分布（native の test_fixture の単体試験と同じ値）。
export const SHAPED_STATE_TEXTS = {
  card_list: { 下書き: 5, オープン: 5, マージ済み: 5, クローズ: 5 },
  check_list: { '✓ 成功': 4, '✗ 失敗': 4, '… 実行中': 4, '… 待機中': 4, '– 省略': 4 },
};

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
export async function productRedrawInPage({ kind, from, rows, samples, gapMs, shape = false, variant = '', stateTexts = {} }) {
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
  // 形つきの固定データだけ: 描画部品の実際の形（hunk・dir と file・入れ子・本文の区画・kind の混在）が出ていること。
  // 外れたら例外にする（別の経路に退いた描画を速いと誤読しない）。カードとチェックは、全行の data-kind と状態のバッジの
  // 文字の分布（stateTexts。語彙の固定の文字）を完全一致で比べる。textContent の比較だけで、文字列から選択子は作らない。
  const checkShape = (l, items) => {
    const kinds = new Set(items.map((li) => li.getAttribute('data-kind')));
    if (kind === 'diff' && !kinds.has('hunk')) throw new Error('diff: no hunk row');
    if (kind === 'file_tree' && !(kinds.has('dir') && kinds.has('file'))) throw new Error('file_tree: dir and file rows are not both present');
    if (kind === 'app_demo' && kinds.size < 2) throw new Error('app_demo: fewer than 2 row kinds');
    if (kind === 'task_tree' && ![...l.querySelectorAll('[aria-level]')].some((e) => Number(e.getAttribute('aria-level')) >= 2)) {
      throw new Error('task_tree: no nested row');
    }
    if (kind === 'document') {
      const bodies = l.querySelectorAll('[data-testid="artifact-doc-body"]').length;
      const want = variant === 'body_all' ? rows : 1;
      if (bodies !== want) throw new Error(`document: expected ${want} body blocks, got ${bodies}`);
    }
    if (kind === 'card_list' || kind === 'check_list') {
      const rowKind = kind === 'card_list' ? 'card' : 'check';
      if (items.some((li) => li.getAttribute('data-kind') !== rowKind)) throw new Error(`${kind}: a row is not data-kind=${rowKind}`);
      const counts = {};
      for (const li of items) {
        const badges = li.querySelectorAll('[data-scope="marker"][data-part="root"]');
        if (badges.length !== 1) throw new Error(`${kind}: ${badges.length} state badges in a row`);
        const t = badges[0].textContent.trim();
        counts[t] = (counts[t] ?? 0) + 1;
      }
      const want = stateTexts[kind];
      if (!want) throw new Error(`${kind}: expected state texts are missing`);
      const keys = new Set([...Object.keys(counts), ...Object.keys(want)]);
      for (const k of keys) {
        if ((counts[k] ?? 0) !== (want[k] ?? 0)) throw new Error(`${kind}: state text "${k}" count ${counts[k] ?? 0}, expected ${want[k] ?? 0}`);
      }
    }
  };
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
        if (shape) checkShape(l, items);
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
        scope_note: 'product view(); product view() with the 7 per-kind renderers wired (2026-11-05); fixture rows are attribute-less world-state paths, so renderer-specific shapes are not exercised; not comparable with view-redraw',
      },
    };
  } finally {
    await srv.close();
  }
}

// ---- 形つきの固定データでの測り直し（#2074）----

export function shapedMeasurementName(c, stat) {
  return `product_redraw_shaped_${c.kind}${c.variant ? `_${c.variant}` : ''}_${c.rows}_switch_${stat}`;
}

// 形つきの固定データのクエリ。cases() の 7 組と shapedReferenceCases() の参考の変種だけを受ける（native 側の完全一致の表と同じ）。
export function shapedFixtureQuery(c) {
  return `artifact-redraw-shaped-${c.kind}-${c.rows}${c.variant ? `-${c.variant}` : ''}`;
}

// 種類ごとの行の形の要約（結果の conditions に残す。値は native の shaped_seed と対応する）。
export const SHAPED_ROW_SHAPES = {
  file_tree: 'nested paths: dir and file rows mixed (20 dirs x sub dir, files, readme)',
  document: 'title, state and detail on every row; first row carries an 80-block body (fixture_docs, body: shown)',
  document_body_all: 'reference variant (not judged): same rows as document, but every row carries the same 80-block body (one shared fixture_docs entry)',
  diff: 'hunk row every 20 rows; others cycle ctx/add/del with a one-line detail',
  task_tree: 'depth 0-2 nested paths; state cycles open/progress/done',
  card_list: 'state cycles draft/open/merged/closed with title and detail; shape checked: every row data-kind=card with one state badge, badge texts match the vocabulary distribution',
  check_list: 'state cycles success/failure/running/pending/skipped with title and detail; shape checked: every row data-kind=check with one state badge, badge texts match the vocabulary distribution',
  app_demo: 'kind cycles heading/tab/field/button/item/text; state empty/active/disabled',
};

// 流れは measureProductRedraw と同じで、固定データのクエリと形の検査（shape: true）だけが違う。判定は同じ judge（閾値は緩めない）。
export async function measureProductRedrawShaped(browser, dir, deps) {
  const { newPage, installMountObserver, median, p95, toMicros, progress } = deps;
  const list = [...cases(), ...shapedReferenceCases()];
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
          await page.goto(`${srv.url}?fixture=${shapedFixtureQuery(c)}`, { waitUntil: 'commit', timeout: NAV_TIMEOUT_MS });
          await page.waitForFunction(() => window.__mountedValue !== undefined, null, { timeout: NAV_TIMEOUT_MS });
          const state = await page.evaluate(() => ({
            mounted: window.__mountedValue,
            hooks: typeof window.__fandheTest?.stateJson === 'function',
          }));
          if (state.mounted !== 'true') throw new Error(`data-mounted is "${state.mounted}" (mount failed)`);
          if (!state.hooks) throw new Error('window.__fandheTest.stateJson is missing (not a test-hooks build?)');
          const v = await page.evaluate(productRedrawInPage, {
            kind: c.kind, from: c.from, rows: c.rows, samples: PR_SAMPLES, gapMs: PR_GAP_MS, shape: true, variant: c.variant ?? '', stateTexts: SHAPED_STATE_TEXTS,
          });
          if (!Array.isArray(v) || v.length !== PR_SAMPLES) throw new Error(`unexpected sample count: ${c.kind}`);
          (all[caseKey(c)] ??= []).push(...v);
        } finally {
          await context.close();
        }
      }
      progress(`product-redraw-shaped trial ${t + 1}/${PR_TRIALS} done`);
    }
    const measurements = [];
    const p95Us = {};
    for (const c of list) {
      const key = caseKey(c);
      const v = all[key];
      if (!v || v.length !== PR_TRIALS * PR_SAMPLES) throw new Error(`unexpected value count: ${key}`);
      const p = toMicros(p95(v), `${key} p95`);
      p95Us[key] = p;
      measurements.push({ name: shapedMeasurementName(c, 'median'), unit: 'us', value: toMicros(median(v), `${key} median`) });
      measurements.push({ name: shapedMeasurementName(c, 'p95'), unit: 'us', value: p });
      measurements.push({ name: shapedMeasurementName(c, 'max'), unit: 'us', value: toMicros(Math.max(...v), `${key} max`) });
      progress(`product-redraw-shaped ${key}: p95 ${p} us`);
    }
    // 判定は cases() の 7 組だけ。参考の変種は入れない（閾値・判定の対象を変えない）。
    const judgement = judge(p95Us);
    const reference = {};
    for (const c of shapedReferenceCases()) {
      const p = p95Us[caseKey(c)];
      reference[`${c.kind}_${c.variant}`] = { kind: c.kind, rows: c.rows, fixture: shapedFixtureQuery(c), switch_p95_us: p, over_threshold: p > PR_THRESHOLD_US };
      progress(`product-redraw-shaped ${caseKey(c)} (reference variant): p95 ${p} us`);
    }
    for (const [kind, j] of Object.entries(judgement)) {
      progress(`product-redraw-shaped ${kind}${j.judged ? '' : ' (reference)'}: ${j.within ? 'within' : 'over'} ${PR_THRESHOLD_US} us`);
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
        switch_from: Object.fromEntries(cases().map((c) => [c.kind, c.from])),
        sizes: Object.fromEntries(cases().map((c) => [c.kind, { fixture: shapedFixtureQuery(c), rows: c.rows }])),
        reference_variants: reference,
        judged: cases().filter((c) => c.judged).map((c) => c.kind),
        judgement,
        point_attrs: 'all rows (7 kinds declared on main/st-1)',
        seed: { pack_id: 'artifact-redraw-shaped', row_shapes: SHAPED_ROW_SHAPES, step: 'main/st-1' },
        sample_diff_rows: diffRows,
        trial_rotation: 'case order rotated by the trial index; a new context per case',
        base: BASE,
        scope_note: 'product view() with per-kind shaped rows (hunk, dir/file, nested tasks, document body, app demo kinds); same op, sizes and threshold as product-redraw but not the same rows, so not directly comparable with it; reference variants (document body on every row) are recorded in reference_variants and are not judged',
      },
    };
  } finally {
    await srv.close();
  }
}

// ---- CPU 4 倍の記録（ART-15。TASK-485 3/3）。判定に使わない ----

// 記録の組。cases() から ART-15 に大きさが書かれた 3 種（タスク 50 件・文書 200 行・差分 500 行）だけを残す。
export function cpu4xCases() {
  return cases().filter((c) => c.judged);
}

export function cpu4xMeasurementName(c, stat, shaped = false) {
  return `product_redraw_${shaped ? 'shaped_' : ''}cpu4x_${c.kind}_${c.rows}_switch_${stat}`;
}

// 記録（純関数）。p95Us: `${kind}:${rows}` -> p95（us）。判定の欄（within・judged・partial）は持たず、参考の超過の印だけを返す。
export function cpu4xRecord(p95Us, thresholdUs = PR_THRESHOLD_US) {
  const out = {};
  for (const c of cpu4xCases()) {
    const p = p95Us[`${c.kind}:${c.rows}`];
    if (!Number.isFinite(p)) throw new Error(`missing p95 for ${c.kind}:${c.rows}`);
    out[c.kind] = { rows: c.rows, switch_p95_us: p, over_threshold: p > thresholdUs };
  }
  return out;
}

// 記録だけの測定。流れは measureProductRedraw と同じで、mount の確認の後に CPU を 4 倍遅くし、較正で効いていることを確かめてから測る。
export async function measureProductRedrawCpu4x(browser, dir, deps, { shaped = false } = {}) {
  const { newPage, installMountObserver, median, p95, toMicros, progress, calibrateAndThrottle, calibrationInfo } = deps;
  const list = cpu4xCases();
  const diffRows = sampleDiffRows();
  const srv = await startServer({ dir, base: BASE, port: 0, gzip: true, gzipLevel: GZIP_LEVEL });
  try {
    const all = {};
    const calibrations = [];
    for (let t = 0; t < PR_TRIALS; t++) {
      for (let k = 0; k < list.length; k++) {
        const c = list[(k + trialRotation(t, list.length)) % list.length];
        const { context, page } = await newPage(browser, false);
        try {
          await page.addInitScript(installMountObserver);
          await page.goto(`${srv.url}?fixture=${shaped ? shapedFixtureQuery(c) : `artifact-redraw-${c.rows}`}`, { waitUntil: 'commit', timeout: NAV_TIMEOUT_MS });
          await page.waitForFunction(() => window.__mountedValue !== undefined, null, { timeout: NAV_TIMEOUT_MS });
          const state = await page.evaluate(() => ({
            mounted: window.__mountedValue,
            hooks: typeof window.__fandheTest?.stateJson === 'function',
          }));
          if (state.mounted !== 'true') throw new Error(`data-mounted is "${state.mounted}" (mount failed)`);
          if (!state.hooks) throw new Error('window.__fandheTest.stateJson is missing (not a test-hooks build?)');
          // 減速は mount の確認の後に掛ける。context を閉じるまで session を保持する（detach すると減速が戻る）。
          const throttled = await calibrateAndThrottle(context, page);
          calibrations.push(throttled.calibration);
          const v = await page.evaluate(productRedrawInPage, {
            kind: c.kind, from: c.from, rows: c.rows, samples: PR_SAMPLES, gapMs: PR_GAP_MS,
            ...(shaped ? { shape: true, stateTexts: SHAPED_STATE_TEXTS } : {}),
          });
          if (!Array.isArray(v) || v.length !== PR_SAMPLES) throw new Error(`unexpected sample count: ${c.kind}`);
          (all[`${c.kind}:${c.rows}`] ??= []).push(...v);
        } finally {
          await context.close();
        }
      }
      progress(`product-redraw${shaped ? '-shaped' : ''}-cpu4x trial ${t + 1}/${PR_TRIALS} done`);
    }
    const measurements = [];
    const p95Us = {};
    for (const c of list) {
      const key = `${c.kind}:${c.rows}`;
      const v = all[key];
      if (!v || v.length !== PR_TRIALS * PR_SAMPLES) throw new Error(`unexpected value count: ${key}`);
      const p = toMicros(p95(v), `${key} p95`);
      p95Us[key] = p;
      measurements.push({ name: cpu4xMeasurementName(c, 'median', shaped), unit: 'us', value: toMicros(median(v), `${key} median`) });
      measurements.push({ name: cpu4xMeasurementName(c, 'p95', shaped), unit: 'us', value: p });
      measurements.push({ name: cpu4xMeasurementName(c, 'max', shaped), unit: 'us', value: toMicros(Math.max(...v), `${key} max`) });
      progress(`product-redraw${shaped ? '-shaped' : ''}-cpu4x ${key}: p95 ${p} us (record only)`);
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
        cpu_throttle: 4,
        record_only: true,
        cpu_calibration: calibrationInfo(calibrations),
        trials: PR_TRIALS,
        samples_per_trial: PR_SAMPLES,
        threshold_p95_us: PR_THRESHOLD_US,
        threshold_use: 'reference only (not judged; ART-15)',
        op_trigger: 'click on the view-select-button (includes click delegation, attribute reads, update, view() build, keyed list diff and DOM apply; synchronous)',
        ops: { switch: 'measured', toggle: 'not_applicable (no before/after toggle in the product view())' },
        switch_from: Object.fromEntries(list.map((c) => [c.kind, c.from])),
        sizes: Object.fromEntries(list.map((c) => [c.kind, { fixture: shaped ? shapedFixtureQuery(c) : `artifact-redraw-${c.rows}`, rows: c.rows }])),
        record_only_cpu4x: cpu4xRecord(p95Us),
        seed: shaped
          ? { pack_id: 'artifact-redraw-shaped', row_shapes: SHAPED_ROW_SHAPES, step: 'main/st-1' }
          : { pack_id: 'artifact-redraw', path_shape: 'p/{i:04}', step: 'main/st-1' },
        sample_diff_rows: diffRows,
        trial_rotation: 'case order rotated by the trial index; a new context per case',
        base: BASE,
        scope_note: shaped
          ? 'product view() with per-kind shaped rows and the CPU slowed 4x via CDP; record only, not used for the judgement (ART-15); the document variant with a body on every row is not included; not comparable with product-redraw-cpu4x (attribute-less rows) or product-redraw-shaped (no throttle)'
          : 'product view() with the CPU slowed 4x via CDP; record only, not used for the judgement (ART-15); not comparable with product-redraw',
      },
    };
  } finally {
    await srv.close();
  }
}
