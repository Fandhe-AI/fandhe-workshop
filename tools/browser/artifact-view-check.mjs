// 左の成果物ビュー共通の検査器（TASK-471 2/3。PoC-8 の検査の写し。ART-1・ART-2・ART-3・ART-12）。
//
// 役割: 7 種のビュー × 2 変種（14 通り）を、各通りで前後を 2 往復して切り替え、次を数える。
// - 前後の項目数と key の並びが、JSON から独立に数えた期待値と一致する
// - 印の 3 手段（記号・文字・data-mark）が対応している（印の宿主は項目ごとにちょうど 1 つ）
// - key が重複しない
// - 選ばれていない list が空で hidden である（選ばれた list は hidden でない）
// - 要素の同一性（未変更の行は 100%、変更のあった行の置換は 0）
// - 契約が許さない 2 通り（文書 (a)・差分 (a)）は、拒否され、状態も画面も変わらず、(b) へ黙って退避しない
// 外部へのリクエスト・ページのエラー・ダイアログが 0 件であることは、呼び出し側が harness の collectSafety で主張する。
//
// 呼び出し元: 本検査器の自己確認（artifact-view-check.test.mjs。マウントする代役の部品に向ける）。
// 後続は、自分のビューを差し込むアダプタを渡して使う（TASK-469・TASK-470・TASK-473〜TASK-479。
// 負の対照 3 種と教材固有の語の検査は TASK-471 3/3）。
//
// アダプタの契約（検査器は画面の作り方を知らない）:
//   select(kind, variant) -> Promise<{ ok: boolean }>   ビューの種類と変種を選ぶ。拒否されたら ok: false
//   phase('before' | 'after') -> Promise<{ ok: boolean }>  前後を切り替える
//   state() -> Promise<string>                           任意。拒否で状態が変わらないことの照合に使う
//   keyOf(rowId) -> string                               任意。行の id から data-key への写像（既定は恒等）
// 代役の部品のアダプタは自己確認の試験に置く。本体の画面へ向けるアダプタは data-action を click する形になる。
//
// 期待値の独立性: 期待値は testdata の JSON を Node 側で数えた値（expectedFromRows）で、Rust・wasm から導かない。
// 文書だけは JSON の行が無く、手で数えた固定値（DOC_EXPECTED）にする。出所は sample の doc-before.md・doc-after.md の
// 最上位のブロックで、見出し・段落・一覧・表・引用を各 1 ブロックと数える。変更されたブロックは削除と追加の 2 行になる。
// 既知の制約: TASK-476 で本文の木の粒度を結んだら、この固定値を見直す。
//
// 安全: ページへ渡すのは固定の関数で、行の値・key は引数で渡す。文字列からコードや選択子を組み立てない。
// 違反の文面は固定の英語で、行の値を埋め込まない。検査は assert せず違反の一覧を返す（負の対照が
// 「特定の違反が出ること」を主張できるようにするため）。
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

// 契約（artifact_view::ViewKind）の綴りと、sample のファイルの対応。document は JSON の行を持たない。
export const KINDS = [
  { kind: 'file_tree', file: 'filetree.json' },
  { kind: 'document', file: null },
  { kind: 'diff', file: 'diff.json' },
  { kind: 'task_tree', file: 'tasks.json' },
  { kind: 'card_list', file: 'cards.json' },
  { kind: 'check_list', file: 'checks.json' },
  { kind: 'app_demo', file: 'appdemo.json' },
];
export const VARIANTS = ['a', 'b'];

// ViewKind::allows の写し。文書・差分は変種 (b) だけ（ART-7・ART-8）。ずれは自己確認の T3 が拒否の側で検出する。
export function variantAllowed(kind, variant) {
  return !((kind === 'document' || kind === 'diff') && variant === 'a');
}

// 14 通り（7 種 × 2 変種）。allowed が false の 2 通りは拒否の検査（runRejectedCase）で流す。
export const CASES = KINDS.flatMap(({ kind }) =>
  VARIANTS.map((variant) => ({ kind, variant, allowed: variantAllowed(kind, variant) })),
);

// 印の 3 手段の対応（A11Y-22・ART-3）。data-mark の値 -> [記号, 文字]。印なしは空。
const MARK_TABLE = { add: ['+', '【追加】'], remove: ['-', '【削除】'], change: ['~', '【変更】'], '': ['', ''] };

// 文書の期待値（手で数えた固定値）。before は前のブロック数、after は後に出す行数（削除を残す）。
export const DOC_EXPECTED = { before: 10, after: 13, unchanged: 7, add: 3, remove: 3, change: 0 };

const SAMPLE_DIR = new URL('../../testdata/artifact-view-parts/test-data/sample/', import.meta.url);
const ESCAPE_ROWS = new URL('../../testdata/artifact-view-parts/test-data-addendum/escape-rows.json', import.meta.url);

// sample の JSON（6 種）の行を読む。形が違えば例外にする（空の期待で通さない）。
export function loadSampleRows(kind) {
  const entry = KINDS.find((k) => k.kind === kind);
  if (!entry || entry.file === null) throw new Error('no JSON rows for this kind');
  const parsed = JSON.parse(fs.readFileSync(fileURLToPath(new URL(entry.file, SAMPLE_DIR)), 'utf8'));
  if (!Array.isArray(parsed.rows) || parsed.rows.length === 0) throw new Error('sample rows are missing');
  return parsed.rows;
}

// escape の行（ART-6 の 24 通りの入力）を読み、view の名前だけ差し替えて返す。流すのは後続の試験である。
export function loadEscapeRows(kind) {
  if (!KINDS.some((k) => k.kind === kind)) throw new Error('unknown kind');
  const parsed = JSON.parse(fs.readFileSync(fileURLToPath(ESCAPE_ROWS), 'utf8'));
  if (!Array.isArray(parsed.rows) || parsed.rows.length === 0) throw new Error('escape rows are missing');
  return { ...parsed, view: kind };
}

function sameCell(a, b) {
  return JSON.stringify([a.label, a.detail, a.state, a.tags]) === JSON.stringify([b.label, b.detail, b.state, b.tags]);
}

// 行から期待値を数える純関数（testdata/artifact-view-parts/test-data/README.md の規則）。
// beforeIds: before が null でない行（印なし）。afterIds: 全行（削除は前の中身を残して出す）。
export function expectedFromRows(rows) {
  const beforeIds = [];
  const afterIds = [];
  const markById = {};
  const unchangedIds = [];
  const changedIds = [];
  const removedIds = [];
  const addedIds = [];
  for (const r of rows) {
    afterIds.push(r.id);
    if (r.before !== null) beforeIds.push(r.id);
    if (r.before === null) {
      markById[r.id] = 'add';
      addedIds.push(r.id);
    } else if (r.after === null) {
      markById[r.id] = 'remove';
      removedIds.push(r.id);
    } else if (sameCell(r.before, r.after)) {
      markById[r.id] = '';
      unchangedIds.push(r.id);
    } else {
      markById[r.id] = 'change';
      changedIds.push(r.id);
    }
  }
  return {
    beforeIds, afterIds, markById, unchangedIds, changedIds, removedIds, addedIds,
    counts: { before: beforeIds.length, after: afterIds.length, add: addedIds.length, remove: removedIds.length, change: changedIds.length },
  };
}

// 種類ごとの期待値。文書は行が無いので件数だけ（ids は null）。
function expectedFor(kind, keyOf) {
  if (kind === 'document') {
    return { ids: null, counts: { ...DOC_EXPECTED }, keyOf };
  }
  return { ids: expectedFromRows(loadSampleRows(kind)), counts: null, keyOf };
}

// ページ内の検査（固定の関数。データは引数で渡す）。root の下の 7 本の list と、各項目の印の宿主を数える。
function inspectInPage({ root }) {
  const rootEl = document.querySelector(root);
  if (!rootEl) return { error: true };
  const lists = [...rootEl.querySelectorAll('[data-testid="view-list"]')];
  return {
    error: false,
    lists: lists.map((l) => {
      const items = [...l.children];
      return {
        view: l.getAttribute('data-view'),
        bind: l.getAttribute('data-bind-list'),
        hidden: l.hasAttribute('hidden'),
        children: items.length,
        keys: items.map((i) => i.getAttribute('data-key')),
        hosts: items.map((item) => {
          // 変種 (b) は項目自身、(a) は項目の中の外枠が印の宿主になる。両方にあれば 2 つと数える（違反）。
          const all = (item.hasAttribute('data-mark') ? [item] : []).concat([...item.querySelectorAll('[data-mark]')]);
          const host = all[0] ?? null;
          const part = (id) => (host ? [...host.children].filter((c) => c.getAttribute('data-testid') === id) : []);
          const glyph = part('mark-glyph');
          const text = part('mark-text');
          return {
            hostCount: all.length,
            onItem: host === item,
            glyphCount: glyph.length,
            textCount: text.length,
            mark: host ? host.getAttribute('data-mark') : null,
            glyph: glyph.length > 0 ? glyph[0].textContent : null,
            text: text.length > 0 ? text[0].textContent : null,
          };
        }),
      };
    }),
  };
}

// 選ばれた list の key -> 要素の Map を、ページ内の JSHandle として取る。属性や大域変数は足さない
// （上流の同期が後付けの属性を消すため）。同一性は、この Map どうしの === で比べる。
function elementMapInPage({ root, view }) {
  const map = new Map();
  const rootEl = document.querySelector(root);
  const list = rootEl
    ? [...rootEl.querySelectorAll('[data-testid="view-list"]')].find((l) => l.getAttribute('data-view') === view)
    : undefined;
  if (list) for (const c of list.children) map.set(c.getAttribute('data-key'), c);
  return map;
}

// 2 つの Map で両方にある key の要素が同じか。接続が切れた要素は置換と数える。
function compareMapsInPage([a, b]) {
  const same = [];
  const replaced = [];
  for (const [k, el] of a) {
    if (!b.has(k)) continue;
    (b.get(k) === el && el.isConnected ? same : replaced).push(k);
  }
  return { same, replaced };
}

async function snapshot(page, rootSelector, kind) {
  const info = await page.evaluate(inspectInPage, { root: rootSelector });
  const handle = await page.evaluateHandle(elementMapInPage, { root: rootSelector, view: kind });
  return { info, handle };
}

// 1 つの状態の検査。違反の文面は固定で、tag（種類・変種・状態の名前）だけを前置する。
function checkState({ info, kind, variant, phase, expected, v }) {
  const bad = (msg) => v.push(`${kind}/${variant}/${phase}: ${msg}`);
  if (info.error) return bad('root element not found');
  const { lists } = info;
  if (lists.length !== 7) bad('expected exactly 7 view lists');
  const views = lists.map((l) => l.view).sort();
  const want = KINDS.map((k) => k.kind).sort();
  if (JSON.stringify(views) !== JSON.stringify(want)) bad('view list set differs from the 7 kinds');
  const shown = lists.filter((l) => !l.hidden);
  for (const l of lists) {
    if (l.view === kind) continue;
    if (l.children !== 0) bad('an unselected view list is not empty');
    if (!l.hidden) bad('an unselected view list is not hidden');
  }
  const sel = lists.find((l) => l.view === kind);
  if (!sel) return bad('selected view list is missing');
  if (sel.hidden) bad('the selected view list is hidden');
  if (shown.length !== 1) bad('expected exactly one visible view list');
  if (sel.bind !== `view_${kind}`) bad('selected list binding name differs');

  const keys = sel.keys;
  if (keys.some((k) => k === null) || new Set(keys).size !== keys.length) bad('item keys are missing or duplicated');

  // 件数と key の並び。
  const isAfter = phase === 'after';
  if (expected.ids) {
    const ids = isAfter ? expected.ids.afterIds : expected.ids.beforeIds;
    if (sel.children !== ids.length) bad('item count differs from the expected count');
    if (JSON.stringify(keys) !== JSON.stringify(ids.map(expected.keyOf))) bad('item keys differ from the expected keys');
  } else {
    const n = isAfter ? expected.counts.after : expected.counts.before;
    if (sel.children !== n) bad('item count differs from the expected count');
  }

  // 印の宿主と 3 手段の対応。
  let hostsOk = true;
  const marks = sel.hosts.map((h) => {
    if (h.hostCount !== 1 || h.glyphCount !== 1 || h.textCount !== 1) {
      hostsOk = false;
      return null;
    }
    // 変種 (b) は項目自身、(a) は項目の中の外枠が宿主。位置が変種と食い違えば、変種を無視した実装として違反にする。
    if (h.onItem !== (variant === 'b')) {
      hostsOk = false;
      return null;
    }
    const row = MARK_TABLE[h.mark];
    if (row === undefined || h.glyph !== row[0] || h.text !== row[1]) {
      hostsOk = false;
      return null;
    }
    return h.mark;
  });
  if (!hostsOk) bad('a mark host is missing, duplicated, misplaced for the variant, or its glyph/text/data-mark do not correspond');

  // 印の値。前は全行が印なし、後は行ごとに期待と一致する。
  if (!isAfter) {
    if (marks.some((m) => m !== '' && m !== null)) bad('a mark is shown in the before state');
  } else if (expected.ids) {
    const got = JSON.stringify(marks);
    const wantMarks = JSON.stringify(expected.ids.afterIds.map((id) => expected.ids.markById[id]));
    if (hostsOk && got !== wantMarks) bad('row marks differ from the expected marks');
  } else if (hostsOk) {
    const tally = (m) => marks.filter((x) => x === m).length;
    const c = expected.counts;
    if (tally('add') !== c.add || tally('remove') !== c.remove || tally('change') !== c.change || tally('') !== c.unchanged) {
      bad('mark counts differ from the expected counts');
    }
    // 関係: 未変更 + 追加 = 後のブロック数、未変更 + 削除 = 前のブロック数。
    if (c.unchanged + c.add !== c.after - c.remove || c.unchanged + c.remove !== c.before) {
      bad('expected document counts are inconsistent');
    }
  }
}

// 同一性。両方の状態にある key は同じ要素でなければならない。置換 0、保持の数は前の行数に等しい。
async function checkIdentity({ page, prev, next, kind, variant, expected, v, tag }) {
  const { same, replaced } = await page.evaluate(compareMapsInPage, [prev.handle, next.handle]);
  const bad = (msg) => v.push(`${kind}/${variant}/${tag}: ${msg}`);
  if (replaced.length !== 0) bad('rows present in both states were replaced');
  const beforeCount = expected.ids ? expected.ids.counts.before : expected.counts.before;
  if (same.length !== beforeCount) bad('preserved row count differs from the before row count');
  if (expected.ids) {
    const { unchangedIds, changedIds } = expected.ids;
    const keep = new Set(same);
    const key = expected.keyOf;
    if (!unchangedIds.every((id) => keep.has(key(id)))) bad('an unchanged row was replaced');
    if (!changedIds.every((id) => keep.has(key(id)))) bad('a changed row was replaced');
  }
  return same.length;
}

// 14 通りのうち許される 12 通りを流す。前 → 後 → 前 → 後 → 前（rounds = 2 で 2 往復）の各状態を検査する。
// 返り値: { violations: string[], stats: { states, itemCounts, preserved } }。
export async function runViewCase(page, adapter, { kind, variant, rootSelector, rounds = 2 }) {
  const v = [];
  const stats = { states: 0, itemCounts: [], preserved: [] };
  const expected = expectedFor(kind, adapter.keyOf ?? ((id) => id));
  const sel = await adapter.select(kind, variant);
  if (!sel.ok) {
    v.push(`${kind}/${variant}: the adapter refused to select an allowed variant`);
    return { violations: v, stats };
  }
  const phases = ['before'];
  for (let i = 0; i < rounds; i += 1) phases.push('after', 'before');
  let prev = null;
  try {
    for (let i = 0; i < phases.length; i += 1) {
      const phase = phases[i];
      const sw = await adapter.phase(phase);
      if (!sw.ok) {
        v.push(`${kind}/${variant}/${phase}: the adapter refused to switch the state`);
        break;
      }
      const cur = await snapshot(page, rootSelector, kind);
      checkState({ info: cur.info, kind, variant, phase, expected, v });
      stats.states += 1;
      const sel1 = cur.info.error ? undefined : cur.info.lists.find((l) => l.view === kind);
      stats.itemCounts.push(sel1 ? sel1.children : -1);
      if (prev) {
        const n = await checkIdentity({ page, prev, next: cur, kind, variant, expected, v, tag: `${phases[i - 1]}->${phase}` });
        stats.preserved.push(n);
        await prev.handle.dispose();
      }
      prev = cur;
    }
  } finally {
    if (prev) await prev.handle.dispose();
  }
  return { violations: v, stats };
}

// 許されない 2 通り（文書 (a)・差分 (a)）の拒否を検査する（fail-closed）。
// 基準の状態（既定は card_list の (a)・後）を作り、許されない組を選ぶ。拒否され、状態も画面も変わらず、
// 別の変種や別のビューへ黙って切り替わらないことを確かめる。基準に (b) を使うと黙った退避を見分けられない。
export async function runRejectedCase(page, adapter, { kind, variant, rootSelector, baseline = { kind: 'card_list', variant: 'a' } }) {
  const v = [];
  const bad = (msg) => v.push(`${kind}/${variant}: ${msg}`);
  if (variantAllowed(kind, variant)) {
    bad('this combination is allowed; use runViewCase');
    return { violations: v };
  }
  const base = await adapter.select(baseline.kind, baseline.variant);
  const ph = await adapter.phase('after');
  if (!base.ok || !ph.ok) {
    bad('the adapter could not set up the baseline state');
    return { violations: v };
  }
  const read = async () => {
    const info = await page.evaluate(inspectInPage, { root: rootSelector });
    const visible = info.error ? null : info.lists.filter((l) => !l.hidden).map((l) => l.view);
    const keys = info.error ? null : info.lists.map((l) => l.keys);
    const marks = info.error ? null : info.lists.map((l) => l.hosts.map((h) => h.mark));
    // 行のラベル・印の文字・項目内の構造まで比べるため、root 配下の DOM の中身（テキストと属性）も取る。
    const dom = await page.evaluate((sel) => document.querySelector(sel)?.innerHTML ?? null, rootSelector);
    return { visible, keys, marks, dom, state: adapter.state ? await adapter.state() : null };
  };
  const before = await read();
  if (before.visible === null || JSON.stringify(before.visible) !== JSON.stringify([baseline.kind])) {
    bad('the baseline view is not the only visible list');
    return { violations: v };
  }
  const res = await adapter.select(kind, variant);
  if (res.ok) bad('a disallowed variant was accepted');
  const after = await read();
  if (JSON.stringify(after) !== JSON.stringify(before)) bad('state or screen changed after a refused selection');
  if (after.visible && after.visible.includes(kind)) bad('the refused view was shown');
  return { violations: v };
}
