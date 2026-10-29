// 構造定義（教材の IR）の表示される欄と ID への注入の試験（structure-xss.test.mjs。SAFE-2。TASK-145 1/2）が使う、純粋な Node のモジュール。
//
// 役割: 元にする IR（crates/web/tests/data/ch0_3/ir.json。実行時に読むだけで書き換えない）から、試験用入力 1 件を入れた IR を作る。
//   - fieldVariant: 表示される欄（題・do・選択肢の名前・拒否と問い返しの文面・ヒントの本文・refs）を入力にした IR
//   - idVariant: 種類ごとに 1 つの ID（コマンド名・選択肢の ID・ヒントの ID）の全出現を入力にした IR
//   - observeFields: 画面を読む（ページ内で動く自己完結の関数。評価はしない）
//   - judgeFields: 読んだ値が「文字として出ている」かを判定する（ノード側の純関数）
// 呼び出し元はブラウザ試験 structure-xss.test.mjs と単体試験 e2-ir.test.mjs だけ。入力は書き換えず、深い写しを返す。
// 入力（ペイロード）は文字列として扱うだけで、パス・セレクタ・コードに組み立てない。
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

export const BASE_IR_FILE = fileURLToPath(new URL('../../crates/web/tests/data/ch0_3/ir.json', import.meta.url));

// 他の試験の保存先の鍵と分けるための固定の教材 ID。
export const PACK_ID = 'safe2-fields';

// 2 つの開始地点。s-0-1 は元の IR にあり、s-1-4 を足す（effects を空にする扱いは ch0-3-links の先例と同じ）。
export const STARTS = [
  { id: 's-0-1', node: 'main/st-0-1' },
  { id: 's-1-4', node: 'main/st-1-4' },
];

// ID の変種で置き換える ID（種類ごとに 1 つ）。
export const ID_TARGETS = { command: 'help', choice: 'opt-right', hint: 'h-0-1' };

export function loadBaseIr(file = BASE_IR_FILE) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

const clone = (v) => JSON.parse(JSON.stringify(v));

// 開始点を 2 件にした写しを返す。元の starts に無い開始点だけを effects なしで足す。
export function withStarts(ir) {
  const out = clone(ir);
  for (const s of STARTS) {
    if (!out.starts.some((x) => x.id === s.id)) out.starts.push({ id: s.id, node: s.node, effects: [] });
  }
  for (const s of STARTS) {
    if (!out.nodes.some((n) => n.uid === s.node)) throw new Error(`start ${s.id}: node ${s.node} not found`);
  }
  return out;
}

/**
 * 表示される欄を P にした IR と、置き換えた欄の数（種類ごと）を返す。
 * refs は record_versions が文字列の name・commit の両方を求めるため、両方を P にした 1 件にする。
 */
export function fieldVariant(base, P) {
  const ir = clone(base);
  const counts = { title: 0, do: 0, doLevels: 0, optionLabels: 0, confirmLabels: 0, rejection: 0, clarification: 0, hints: 0, refs: 0 };
  ir.pack = { ...ir.pack, id: PACK_ID };
  for (const n of ir.nodes) {
    const s = n.step;
    s.title = P;
    counts.title += 1;
    if (typeof s.do === 'string') {
      s.do = P;
      counts.do += 1;
    }
    if (s.do_levels && typeof s.do_levels === 'object') {
      for (const k of Object.keys(s.do_levels)) {
        s.do_levels[k] = P;
        counts.doLevels += 1;
      }
    }
    for (const o of s.input?.options ?? []) {
      o.label = P;
      counts.optionLabels += 1;
    }
    for (const o of s.confirm?.options ?? []) {
      o.label = P;
      counts.confirmLabels += 1;
    }
  }
  for (const r of Object.values(ir.rejections)) {
    r.message = P;
    r.next_action = P;
    counts.rejection += 2;
  }
  for (const c of Object.values(ir.clarifications)) {
    c.message = P;
    c.how_to_answer = P;
    counts.clarification += 2;
  }
  for (const h of Object.values(ir.hints)) {
    h.text = P;
    counts.hints += 1;
  }
  ir.refs = [{ name: P, commit: P }];
  counts.refs = 2;
  return { ir, counts };
}

// 深い写しの中で、鍵・文字列の値のうち from と等しいものを to に置き換える。置き換えた数を返す。
// 鍵は Object.fromEntries で作り直す（`__proto__` の鍵でも自分の欄になり、プロトタイプを書き換えない）。
function replaceExact(value, from, to, tally) {
  if (typeof value === 'string') {
    if (value === from) {
      tally.n += 1;
      return to;
    }
    return value;
  }
  if (Array.isArray(value)) return value.map((v) => replaceExact(v, from, to, tally));
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([k, v]) => {
      if (k === from) tally.n += 1;
      return [k === from ? to : k, replaceExact(v, from, to, tally)];
    }));
  }
  return value;
}

/**
 * ID の変種。種類ごとに 1 つの ID の全出現（オブジェクトの鍵と等しい文字列の値）を P にする。
 * 戻りは { ir, replaced: { command, choice, hint }, leftover: { command, choice, hint } }。leftover は置き換えの後に元の ID が残る数（0 が正）。
 */
export function idVariant(base, P) {
  let ir = clone(base);
  ir.pack = { ...ir.pack, id: PACK_ID };
  const replaced = {};
  for (const [kind, id] of Object.entries(ID_TARGETS)) {
    const tally = { n: 0 };
    ir = replaceExact(ir, id, P, tally);
    replaced[kind] = tally.n;
  }
  const leftover = {};
  for (const [kind, id] of Object.entries(ID_TARGETS)) {
    const tally = { n: 0 };
    replaceExact(ir, id, id, tally);
    leftover[kind] = tally.n;
  }
  return { ir, replaced, leftover };
}

// 差し込みの枠（{{…}}）で P を区切った空でない断片。拒否・問い返しの文面は枠を通るため、断片が順に含まれることで判定する。
export function slotFragments(P) {
  return P.split(/\{\{[^}]*\}\}/).filter((x) => x !== '');
}

const nul = (s) => String(s).replaceAll('\u0000', '');

/**
 * 画面の欄を読む。ページ内で評価する自己完結の関数（外の名前を参照しない。Playwright が関数を文字列にして渡すため）。
 * 戻りは JSON にできる値だけ。子の要素の数（kids）は「文字として出た」ことの根拠（要素が増えていない）に使う。
 */
export function observeFields() {
  const one = (sel) => document.querySelector(sel);
  const shape = (el) => (el ? { text: el.textContent ?? '', kids: el.children.length } : null);
  const doEl = one('[data-testid="step-do"]');
  const rows = [...document.querySelectorAll('[data-testid="part-log-list"] li')];
  const last = rows.length === 0 ? null : rows[rows.length - 1];
  const part = (id) => (last ? shape(last.querySelector(`[data-testid="${id}"]`)) : null);
  const tags = new Set();
  if (last) for (const e of last.querySelectorAll('*')) tags.add(e.tagName.toLowerCase());
  return {
    title: shape(one('[data-testid="step-title"]')),
    do: doEl ? { parts: doEl.children.length, body: shape(doEl.children[1] ?? null) } : null,
    choices: [...document.querySelectorAll('[data-testid="choice-btn"]')].map(shape),
    line: last ? {
      kind: last.getAttribute('data-kind'),
      reason: part('line-reason'),
      question: part('line-question'),
      next: part('line-next-action'),
      tags: [...tags].sort(),
    } : null,
    position: one('[data-testid="shell-position"]')?.textContent ?? '',
    notice: document.querySelectorAll('[data-testid="ir-notice"]').length,
  };
}

const eq = (shape, P) => shape !== null && shape.kids === 0 && nul(shape.text) === nul(P);

// 差し込みの枠を通る欄。枠が無い P は完全一致、枠がある P は断片が順に含まれること。
function slotted(shape, P) {
  if (shape === null || shape.kids !== 0) return false;
  const text = nul(shape.text);
  const frags = slotFragments(P).map(nul);
  if (!P.includes('{{')) return text === nul(P);
  let from = 0;
  for (const f of frags) {
    const i = text.indexOf(f, from);
    if (i < 0) return false;
    from = i + f.length;
  }
  return true;
}

/**
 * 観察 obs が expect の欄を P の文字として示しているかを判定し、満たさなかった欄の名前の一覧を返す（空が合格）。
 * expect: { title?: true, do?: true, choices?: 個数, line?: 'rejected' | 'clarified' }。
 */
export function judgeFields(obs, expect, P) {
  const bad = [];
  if (expect.title && !eq(obs.title, P)) bad.push('title');
  if (expect.do && !(obs.do !== null && obs.do.parts === 2 && eq(obs.do.body, P))) bad.push('do');
  if (expect.choices !== undefined) {
    const ok = obs.choices.length === expect.choices && obs.choices.every((c) => eq(c, P));
    if (!ok) bad.push('choices');
  }
  if (expect.line) {
    const l = obs.line;
    const first = expect.line === 'rejected' ? l?.reason : l?.question;
    const spanOnly = l !== null && l !== undefined && l.tags.every((t) => t === 'span');
    if (!(l && l.kind === expect.line && slotted(first ?? null, P) && slotted(l.next ?? null, P) && spanOnly)) bad.push(`line:${expect.line}`);
  }
  return bad;
}

// 画面の文字のどこかに P の断片が出るか（記録用。ヒント・refs には画面の経路が無いことの固定と、ID の変種の記録に使う）。
export function textHas(text, P) {
  const frags = slotFragments(P).map(nul);
  const t = nul(text);
  return frags.length > 0 && frags.every((f) => t.includes(f));
}
