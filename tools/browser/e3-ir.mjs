// 練習データ（世界状態の欄）への注入の試験（practice-data-xss.test.mjs。SAFE-3。TASK-145 2/2）が使う、純粋な Node のモジュール。
//
// 役割: 元にする IR（crates/web/tests/data/ch0_3/ir.json。実行時に読むだけで書き換えない）から、試験用入力 1 件を入れた IR を作る。
//   - e3Variant: `put` のパスと、`put` の属性の値と、`set_attr` の値を入力にした IR
//   - attrOnlyVariant: パスは元のまま、属性の値と `set_attr` の値だけを入力にした IR（属性の値を画面に出す経路が無いことの固定に使う）
//   - observeRows: 左の成果物の行（カードの変種 (b)。ラベルは data-testid=card-label）と右のログの最後の行を読む（ページ内で動く自己完結の関数。評価はしない）
//   - judgeRows・judgeReceived: 読んだ値が「文字として出ている」かを判定する（ノード側の純関数）
// 方法は PoC-7（docs/spec/prototype/tools/browser-test-poc7.mjs の injectE3）に合わせる。同じパスの全出現を置き換え、
// 異なるパスの 1 つ目は P、2 つ目以降は `P.i` にして BTreeMap の鍵を一意に保つ。属性の値と `set_attr` の値は P にする。
// 呼び出し元はブラウザ試験 practice-data-xss.test.mjs と単体試験 e3-ir.test.mjs だけ。入力は書き換えず、深い写しを返す。
// 入力（ペイロード）は文字列として扱うだけで、パス・セレクタ・コードに組み立てない。
import { loadBaseIr, withStarts, replaceExact, STARTS } from './e2-ir.mjs';

export { loadBaseIr, withStarts, STARTS };

// 他の試験の保存先の鍵と分けるための固定の教材 ID。
export const PACK_ID = 'safe3-practice';

const clone = (v) => JSON.parse(JSON.stringify(v));
const nul = (s) => String(s).replaceAll('\u0000', '');

// IR の中の全 `put` のパスを、初出の順に重複なしで集める。効果の置き場（規則・処理中・開始点など）に依らず再帰で拾う。
export function putPaths(ir) {
  const out = [];
  const walk = (v) => {
    if (Array.isArray(v)) {
      v.forEach(walk);
    } else if (v && typeof v === 'object') {
      if (v.put && typeof v.put.path === 'string' && !out.includes(v.put.path)) out.push(v.put.path);
      Object.values(v).forEach(walk);
    }
  };
  walk(ir);
  return out;
}

// 全 `put.attrs` の値と全 `set_attr.value` を P にする（その場で書き換える。clone の上で呼ぶ）。数を返す。
function setAttrValues(ir, P) {
  const counts = { attrs: 0, setAttr: 0 };
  const walk = (v) => {
    if (Array.isArray(v)) {
      v.forEach(walk);
    } else if (v && typeof v === 'object') {
      if (v.put?.attrs && typeof v.put.attrs === 'object') {
        for (const k of Object.keys(v.put.attrs)) {
          v.put.attrs[k] = P;
          counts.attrs += 1;
        }
      }
      if (v.set_attr && typeof v.set_attr === 'object') {
        v.set_attr.value = P;
        counts.setAttr += 1;
      }
      Object.values(v).forEach(walk);
    }
  };
  walk(ir);
  return counts;
}

/**
 * 練習データの変種。戻りは { ir, pathMap, counts: { paths, attrs, setAttr }, leftover }。
 * pathMap は { 元のパス: 置き換え後のパス }。leftover は置き換えの後に元のパスが残る数の合計（0 が正）。
 */
export function e3Variant(base, P) {
  let ir = clone(base);
  ir.pack = { ...ir.pack, id: PACK_ID };
  const paths = putPaths(ir);
  const pathMap = {};
  paths.forEach((p, i) => {
    pathMap[p] = i === 0 ? P : `${P}.${i}`;
  });
  for (const p of paths) ir = replaceExact(ir, p, pathMap[p], { n: 0 });
  const counts = { paths: paths.length, ...setAttrValues(ir, P) };
  let leftover = 0;
  for (const p of paths) {
    const tally = { n: 0 };
    replaceExact(ir, p, p, tally);
    leftover += tally.n;
  }
  return { ir, pathMap, counts, leftover };
}

// パスは元のまま、属性の値と `set_attr` の値だけを P にした IR。
export function attrOnlyVariant(base, P) {
  const ir = clone(base);
  ir.pack = { ...ir.pack, id: PACK_ID };
  const counts = setAttrValues(ir, P);
  return { ir, counts };
}

/**
 * 画面を読む。ページ内で評価する自己完結の関数（外の名前を参照しない。Playwright が関数を文字列にして渡すため）。
 * rows: 左の成果物の各行（カードの変種 (b) の行）の
 *   { path（data-path）, row（data-row。指せない行は null）, labels（行の直下の article の中のカードのラベルの数）,
 *     text（そのラベルの文字）, kids（ラベルの子の要素の数）, articles（行の直下の article の数）,
 *     shape（その article の子のタグの列）, badge（状態のバッジの文字）, tags（行の中の要素のタグ） }。
 * logs: 右のログの各行の { kind, text, tags }。
 */
export function observeRows() {
  const tagsOf = (el) => [...new Set([...el.querySelectorAll('*')].map((e) => e.tagName.toLowerCase()))].sort();
  const rows = [...document.querySelectorAll('[data-testid="part-artifact-view"] li[data-path]')].map((li) => {
    const articles = li.querySelectorAll(':scope > article');
    const article = articles[0] ?? null;
    const labels = li.querySelectorAll(':scope > article > [data-testid="card-label"]');
    const label = labels[0] ?? null;
    return {
      path: li.getAttribute('data-path'),
      row: li.getAttribute('data-row'),
      labels: labels.length,
      text: label?.textContent ?? null,
      kids: label ? label.children.length : -1,
      articles: articles.length,
      shape: article ? [...article.children].map((e) => e.tagName.toLowerCase()) : [],
      badge: article?.children[1]?.textContent ?? null,
      tags: tagsOf(li),
    };
  });
  const logs = [...document.querySelectorAll('[data-testid="part-log-list"] li')].map((li) => ({
    kind: li.getAttribute('data-kind'),
    text: li.textContent ?? '',
    tags: tagsOf(li),
  }));
  return { rows, logs, notice: document.querySelectorAll('[data-testid="shell-notice-item"][data-notice-kind="pack-unloadable"]').length };
}

// カードの状態の閉じた語彙（CardState::text）。バッジの文字はこのどれかでなければならない。
const CARD_STATES = ['下書き', 'オープン', 'マージ済み', 'クローズ', '不明'];
const CARD_SHAPE = ['span', 'div', 'p', 'ul'];
const CARD_TAGS = ['article', 'div', 'li', 'p', 'span', 'ul'];

/**
 * 観察 obs の行が期待のパスの集合と等しく、各行のカードのラベルが置き換え後の文字列そのもの（子の要素 0 個）で、
 * 行の形が (b) の閉じた形（article 1 個・子の列が span・div・p・ul・行の中のタグが閉じた集合・バッジが語彙の 5 語）かを判定し、
 * 満たさなかった項目の名前の一覧を返す（空が合格）。比較は両辺の U+0000 を除いて行う（DOM の文字列は NUL を保てない場合がある）。
 */
export function judgeRows(obs, expectedPaths) {
  const bad = [];
  const want = expectedPaths.map(nul).sort();
  const got = obs.rows.map((r) => nul(r.path ?? '')).sort();
  if (JSON.stringify(want) !== JSON.stringify(got)) bad.push('set');
  for (const r of obs.rows) {
    if (r.labels !== 1 || r.kids !== 0 || r.text === null || nul(r.text) !== nul(r.path ?? '')) bad.push('text');
    if (r.row !== null && nul(r.row) !== nul(r.path ?? '')) bad.push('row');
    if (r.tags.some((t) => !CARD_TAGS.includes(t))) bad.push('tags');
    if (r.articles !== 1 || JSON.stringify(r.shape) !== JSON.stringify(CARD_SHAPE)) bad.push('shape');
    if (r.badge === null || !CARD_STATES.includes(r.badge)) bad.push('state');
  }
  if (obs.notice !== 0) bad.push('notice');
  return [...new Set(bad)];
}

/**
 * 指した後のログに、path を文字として含む受付の行が 1 件以上あり、その行の中が span だけかを判定する（空が合格）。
 * 最後の行ではなく全行を見る（指した直後に達成の行などが続いても揺れない）。
 */
export function judgeReceived(obs, path) {
  const hits = obs.logs.filter((l) => l.kind === 'received' && nul(l.text).includes(nul(path)));
  if (hits.length === 0) return ['log'];
  return hits.some((l) => l.tags.some((t) => t !== 'span')) ? ['tags'] : [];
}

/** 画面の文字のどこかに P が（そのまま）出るか。 */
export function textHas(text, P) {
  return nul(text).includes(nul(P));
}
