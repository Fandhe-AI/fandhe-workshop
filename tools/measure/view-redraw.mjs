// 左の成果物ビューの部品単位の再描画の時間を測る実体（TASK-485 1/3。ART-4。tools/measure/browser.mjs の --kind view-redraw から呼ばれる）。
//
// 出典: docs/spec/03-poc/artifact-view-parts/README.md と testdata/artifact-view-parts/test-data/README.md の「計測の定義」。
// 条件は、試行ごとに新しい context、10 試行 × 20 操作 = 200 値、CPU の減速なし、95 パーセンタイル（nearest-rank）、閾値 16ms。
// 操作は toggle（同じビューの前後を交互に切り替える）と switch（別のビューから対象のビューへ切り替える。全項目の入れ替え）で、
// どちらも判定に使う。判定の規則は「ビューごとに、toggle と switch の両方で p95 が 16ms 以内の変種が 1 つ以上あれば閾値内」。
//
// 測るもの: 試験の設定の代役の部品（crates/web/src/view_fixture.rs）の画面に対する、
// window.__fandheTest.viewFixtureDispatch の同期処理（update・view() の構築・keyed list の差分・DOM への反映）の前後の
// performance.now() の差。click の委譲と同じ dispatch_and_apply を通る。
// 測らないもの: 本体の view() に組み込んだ画面（ART-26。別のイシュー）、CPU 4 倍（ART-15。別のイシュー）、実際のウィンドウの
// レイアウト・ペイント、click の委譲の属性の読み取り。文書 (a)・差分 (a) は契約が拒否する組のため測らない。
//
// 安全: ページへ渡すのは固定の関数で、データは引数で渡す。選択子は固定の文字列と data-view の値の完全一致の比較で作り、
// 文字列からコードを組み立てない。外部へは通信しない（tools/serve.mjs の 127.0.0.1 だけ）。
// 呼び出し元: tools/measure/browser.mjs（統計・起動の共通部は deps で受け取る。循環 import を避けるため）。
// 回帰テスト: tools/measure/browser.test.mjs（純関数）、tools/browser/view-scale.test.mjs（ページ内の関数）、scripts/test-measure.sh。
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { startServer } from '../serve.mjs';
import { KINDS, variantAllowed, expectedFromRows } from '../browser/artifact-view-check.mjs';

export const VR_TRIALS = 10;
export const VR_SAMPLES = 20;
export const VR_THRESHOLD_US = 16000;
export const VR_GAP_MS = 20; // 操作の間の待ち。後続の処理（GC など）を次の計測に入れない
const BASE = '/subpath/';
const GZIP_LEVEL = 9;
const NAV_TIMEOUT_MS = 60000;
const ROOT_ID = 'view-fixture-root';

// 判定の対象の 3 種（計画に大きさが書かれたもの）。残りは参考。
export const JUDGED = ['document', 'diff', 'task_tree'];

// 規模のデータの差し替え。種類 -> payload。カードとチェックの 20 件は既存の規模のデータ、アプリのデモは既定の sample（9 行）のまま。
export const DATA_PAYLOADS = [
  ['file_tree', 'filetree:200'],
  ['document', 'document:200'],
  ['diff', 'diff:500'],
  ['task_tree', 'tasks:50'],
  ['card_list', 'cards:20'],
  ['check_list', 'checks:20'],
];

const SCALE_DIR = new URL('../../crates/web/tests/data/artifact_view_scale/', import.meta.url);
const SAMPLE_DIR = new URL('../../testdata/artifact-view-parts/test-data/sample/', import.meta.url);
const DATA_FILES = {
  file_tree: [SCALE_DIR, 'filetree-200.json'],
  diff: [SCALE_DIR, 'diff-500.json'],
  task_tree: [SCALE_DIR, 'tasks-50.json'],
  card_list: [SCALE_DIR, 'cards-20.json'],
  check_list: [SCALE_DIR, 'checks-20.json'],
  app_demo: [SAMPLE_DIR, 'appdemo.json'],
};
// 文書は JSON の行が無い。手で数えた固定値（PoC-8 の表。Rust の単体試験が同じ値を固定している）。
export const DOC_COUNTS = { before: 80, after: 97 };

// 12 組 × 2 操作の一覧。(b) は 7 種すべて、(a) は契約が許す 5 種。switch の相手は KINDS の並びで 1 つ前の、同じ変種を許す種類（巡回）。
export function cases() {
  const out = [];
  for (let i = 0; i < KINDS.length; i++) {
    const kind = KINDS[i].kind;
    for (const variant of ['a', 'b']) {
      if (!variantAllowed(kind, variant)) continue;
      let from = null;
      for (let step = 1; step < KINDS.length; step++) {
        const cand = KINDS[(i - step + KINDS.length) % KINDS.length].kind;
        if (variantAllowed(cand, variant)) {
          from = cand;
          break;
        }
      }
      for (const op of ['toggle', 'switch']) out.push({ kind, variant, op, from });
    }
  }
  return out;
}

// 前後の項目数（種類 -> {before, after}）。Node 側で JSON の行から数える。Rust・wasm の値を使わない。
export function expectedCounts() {
  const counts = { document: { ...DOC_COUNTS } };
  for (const [kind, [dir, file]] of Object.entries(DATA_FILES)) {
    const rows = JSON.parse(fs.readFileSync(fileURLToPath(new URL(file, dir)), 'utf8')).rows;
    if (!Array.isArray(rows) || rows.length === 0) throw new Error(`rows are missing: ${file}`);
    const { before, after } = expectedFromRows(rows).counts;
    counts[kind] = { before, after };
  }
  return counts;
}

// 組と操作の測定名。
export function measurementName(c, stat) {
  return `view_redraw_${c.kind}_${c.variant}_${c.op}_${stat}`;
}

// 判定（純関数）。p95Us: 測定名の接頭辞（measurementName(c, 'p95')）ではなく `${kind}:${variant}:${op}` -> p95（us）の表。
// ビューごとに、toggle と switch の両方が閾値以内の変種が 1 つ以上あれば within。
export function judge(p95Us, thresholdUs = VR_THRESHOLD_US) {
  const verdict = {};
  for (const { kind } of KINDS) {
    const variants = ['a', 'b'].filter((v) => variantAllowed(kind, v));
    const detail = {};
    let within = false;
    for (const v of variants) {
      const t = p95Us[`${kind}:${v}:toggle`];
      const s = p95Us[`${kind}:${v}:switch`];
      if (!Number.isFinite(t) || !Number.isFinite(s)) throw new Error(`missing p95 for ${kind}:${v}`);
      const ok = t <= thresholdUs && s <= thresholdUs;
      detail[v] = { toggle_p95_us: t, switch_p95_us: s, within: ok };
      if (ok) within = true;
    }
    verdict[kind] = { judged: JUDGED.includes(kind), within, variants: detail };
  }
  return verdict;
}

// ページ内の 1 試行。固定の関数で、データは引数で受け取る。時間を測るのは viewFixtureDispatch の 1 回の同期処理だけ。
// 各操作の後に、同期処理が実際に画面を変えたことを検査する（何もしない更新を速いと誤読しない）。外れたら例外にする。
export async function viewRedrawInPage({ cases: list, counts, payloads, samples, gapMs, rootId, rotate }) {
  const root = document.getElementById(rootId);
  if (!root) throw new Error('fixture root is missing');
  const api = window.__fandheTest;
  const call = (name, payload) => {
    const r = JSON.parse(api.viewFixtureDispatch(name, payload));
    if (r.outcome !== 'applied') throw new Error(`dispatch ${name} was ${r.outcome}`);
  };
  const lists = () => [...root.querySelectorAll('[data-testid="view-list"]')];
  const check = (kind, want) => {
    if (root.querySelector('[data-testid="view-fixture-error"]')) throw new Error('fixture error is shown');
    let found = false;
    for (const l of lists()) {
      const n = l.querySelectorAll(':scope > li').length;
      if (l.getAttribute('data-view') === kind) {
        found = true;
        if (n !== want) throw new Error(`${kind}: expected ${want} items, got ${n}`);
      } else if (n !== 0 || !l.hasAttribute('hidden')) {
        throw new Error('a list that is not selected is not empty and hidden');
      }
    }
    if (!found) throw new Error('selected list is missing');
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const time = (name, payload) => {
    const t0 = performance.now();
    const raw = api.viewFixtureDispatch(name, payload);
    const t1 = performance.now();
    if (JSON.parse(raw).outcome !== 'applied') throw new Error(`dispatch ${name} was not applied`);
    return t1 - t0;
  };

  for (const [, payload] of payloads) call('fixture_data', payload);
  await sleep(gapMs);

  const out = {};
  const n = list.length;
  for (let k = 0; k < n; k++) {
    const c = list[(k + rotate) % n];
    const want = counts[c.kind];
    const values = [];
    if (c.op === 'toggle') {
      call('fixture_select', `${c.kind}:${c.variant}`);
      call('fixture_phase', 'before');
      check(c.kind, want.before);
      for (let i = 0; i < samples; i++) {
        const toAfter = i % 2 === 0;
        values.push(time('fixture_phase', toAfter ? 'after' : 'before'));
        check(c.kind, toAfter ? want.after : want.before);
        await sleep(gapMs);
      }
    } else {
      for (let i = 0; i < samples; i++) {
        call('fixture_select', `${c.from}:${c.variant}`);
        call('fixture_phase', 'after');
        await sleep(gapMs);
        values.push(time('fixture_select', `${c.kind}:${c.variant}`));
        check(c.kind, want.after);
        await sleep(gapMs);
      }
    }
    out[`${c.kind}:${c.variant}:${c.op}`] = values;
  }
  return out;
}

// 試行ごとの組の開始位置（順序による偏りを散らす）。
export function trialRotation(trial, total) {
  return trial % total;
}

// 測定の本体。deps は browser.mjs の共通部（newPage・installMountObserver・median・p95・toMicros・progress）。
export async function measureViewRedraw(browser, dir, deps) {
  const { newPage, installMountObserver, median, p95, toMicros, progress } = deps;
  const list = cases();
  const counts = expectedCounts();
  const srv = await startServer({ dir, base: BASE, port: 0, gzip: true, gzipLevel: GZIP_LEVEL });
  try {
    const all = {};
    for (let t = 0; t < VR_TRIALS; t++) {
      const { context, page } = await newPage(browser, false);
      try {
        await page.addInitScript(installMountObserver);
        await page.goto(srv.url, { waitUntil: 'commit', timeout: NAV_TIMEOUT_MS });
        await page.waitForFunction(() => window.__mountedValue !== undefined, null, { timeout: NAV_TIMEOUT_MS });
        const state = await page.evaluate(() => ({
          mounted: window.__mountedValue,
          hooks: typeof window.__fandheTest?.viewFixtureMount === 'function',
        }));
        if (state.mounted !== 'true') throw new Error(`data-mounted is "${state.mounted}" (mount failed)`);
        if (!state.hooks) throw new Error('window.__fandheTest.viewFixtureMount is missing (not a test-hooks build?)');
        await page.evaluate((id) => {
          const box = document.createElement('div');
          box.id = id;
          document.body.append(box);
        }, ROOT_ID);
        const mounted = JSON.parse(await page.evaluate((id) => window.__fandheTest.viewFixtureMount(id), ROOT_ID));
        if (mounted.ok !== true) throw new Error('view fixture mount failed');
        const res = await page.evaluate(viewRedrawInPage, {
          cases: list,
          counts,
          payloads: DATA_PAYLOADS,
          samples: VR_SAMPLES,
          gapMs: VR_GAP_MS,
          rootId: ROOT_ID,
          rotate: trialRotation(t, list.length),
        });
        for (const c of list) {
          const key = `${c.kind}:${c.variant}:${c.op}`;
          const v = res[key];
          if (!Array.isArray(v) || v.length !== VR_SAMPLES) throw new Error(`unexpected sample count: ${key}`);
          (all[key] ??= []).push(...v);
        }
        progress(`view-redraw trial ${t + 1}/${VR_TRIALS} done`);
      } finally {
        await context.close();
      }
    }
    const measurements = [];
    const p95Us = {};
    for (const c of list) {
      const key = `${c.kind}:${c.variant}:${c.op}`;
      const v = all[key];
      if (v.length !== VR_TRIALS * VR_SAMPLES) throw new Error(`unexpected value count: ${key}`);
      const p = toMicros(p95(v), `${key} p95`);
      p95Us[key] = p;
      measurements.push({ name: measurementName(c, 'median'), unit: 'us', value: toMicros(median(v), `${key} median`) });
      measurements.push({ name: measurementName(c, 'p95'), unit: 'us', value: p });
      measurements.push({ name: measurementName(c, 'max'), unit: 'us', value: toMicros(Math.max(...v), `${key} max`) });
      progress(`view-redraw ${key}: p95 ${p} us`);
    }
    const judgement = judge(p95Us);
    for (const [kind, j] of Object.entries(judgement)) {
      progress(`view-redraw ${kind}${j.judged ? '' : ' (reference)'}: ${j.within ? 'within' : 'over'} ${VR_THRESHOLD_US} us`);
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
        trials: VR_TRIALS,
        samples_per_trial: VR_SAMPLES,
        threshold_p95_us: VR_THRESHOLD_US,
        op_trigger: 'window.__fandheTest.viewFixtureDispatch (synchronous; no click delegation)',
        switch_phase: 'after',
        switch_from: Object.fromEntries(list.filter((c) => c.op === 'switch').map((c) => [`${c.kind}:${c.variant}`, c.from])),
        sizes: Object.fromEntries(
          [['document', 'document:200'], ...DATA_PAYLOADS.filter(([k]) => k !== 'document'), ['app_demo', 'appdemo:sample']].map(([k, p]) => [k, { data: p, ...counts[k] }]),
        ),
        judged: JUDGED,
        excluded: ['document:a (rejected by contract)', 'diff:a (rejected by contract)'],
        trial_rotation: 'case order rotated by the trial index',
        judgement,
        base: BASE,
        scope_note: 'fixture screen (view_fixture); not the product view() (ART-26 is measured separately)',
      },
    };
  } finally {
    await srv.close();
  }
}
