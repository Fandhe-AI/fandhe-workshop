// 履歴ファイルへの注入の試験（history-xss.test.mjs。SAFE-4。TASK-146 1/2）が使う、純粋な Node のモジュール。
//
// 役割: 基準の履歴（実操作の書き出し）から、試験用入力 1 件（E4 の 14 件）で文字列の欄を置き換えた履歴を作り、
//   画面に出ていないことを判定するための断片と、画面を読む関数を返す。
//   変種は 3 つ。`ops`（submit・draft の文字列）・`snapshot`（snapshot.log の中の全文字列。PoC-7 の E4 と同じく log に限る）・
//   `meta`（start・pack_id・versions の文字列の欄）。
// 呼び出し元はブラウザ試験 history-xss.test.mjs と単体試験 e4-history.test.mjs だけ。後続の TASK-146 2/2 も基準の履歴の作り方を再利用する。
// 入力は書き換えず、深い写しを返す。ペイロードは文字列として扱うだけで、パス・セレクタ・コードに組み立てない（eval・Function は使わない）。

// 他の試験の保存先の鍵と分けるための固定の教材 ID と、開く開始点。
export const PACK_ID = 'safe4-history';
export const START = 's-1-4';

export const KINDS = ['ops', 'snapshot', 'meta'];
// ファイル選択（file）と、保存先からの起動時の復元（restore）。
export const WAYS = ['file', 'restore'];
// SAFE-4 が許す断りの理由。広げない（外の理由が出たら失敗にして発見として報告する）。
export const ALLOWED_REASONS = ['replay_mismatch', 'other_pack', 'malformed'];

const clone = (v) => JSON.parse(JSON.stringify(v));

// 値の中の全文字列（キーは変えない）を P にする。置き換えた数を返す。
function replaceStrings(v, P, count) {
  if (typeof v === 'string') {
    count.n += 1;
    return P;
  }
  if (Array.isArray(v)) return v.map((x) => replaceStrings(x, P, count));
  if (v && typeof v === 'object') {
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, replaceStrings(x, P, count)]));
  }
  return v;
}

// 解釈した値のどこかに P が文字列として入っているか（キーは見ない）。
function containsString(v, P) {
  if (typeof v === 'string') return v === P;
  if (Array.isArray(v)) return v.some((x) => containsString(x, P));
  if (v && typeof v === 'object') return Object.values(v).some((x) => containsString(x, P));
  return false;
}

/**
 * 基準の履歴の文字列 baseText から、P で置き換えた履歴の文字列と、置き換えた数を返す。
 * 置き換えが 0 件・結果が元と同じ・P が解釈した値に入っていない場合は例外にする（空振りの防止）。
 */
export function historyVariant(baseText, P, kind) {
  const base = JSON.parse(baseText);
  const h = clone(base);
  const replaced = {};
  if (kind === 'ops') {
    replaced.submit = 0;
    replaced.draft = 0;
    h.ops = h.ops.map((op) => {
      if (op && typeof op.submit === 'string') {
        replaced.submit += 1;
        return { ...op, submit: P };
      }
      if (op && typeof op.draft === 'string') {
        replaced.draft += 1;
        return { ...op, draft: P };
      }
      return op;
    });
    if (replaced.submit < 1 || replaced.draft < 1) throw new Error(`ops variant: nothing to replace ${JSON.stringify(replaced)}`);
  } else if (kind === 'snapshot') {
    const count = { n: 0 };
    h.snapshot.log = replaceStrings(h.snapshot.log, P, count);
    replaced.log = count.n;
    if (count.n < 1) throw new Error('snapshot variant: no string in snapshot.log');
  } else if (kind === 'meta') {
    h.start = P;
    h.pack_id = P;
    const v = h.versions;
    const count = { n: 2 };
    for (const k of ['engine', 'vocab', 'content', 'content_hash']) {
      if (typeof v[k] === 'string') {
        v[k] = P;
        count.n += 1;
      }
    }
    v.refs = { [P]: P };
    count.n += 1;
    replaced.meta = count.n;
  } else {
    throw new Error(`unknown kind: ${kind}`);
  }
  const text = JSON.stringify(h);
  if (text === baseText) throw new Error(`${kind} variant: identical to base`);
  if (!containsString(h, P) && kind !== 'meta') throw new Error(`${kind} variant: payload not in parsed value`);
  if (kind === 'meta' && !(h.pack_id === P && h.start === P)) throw new Error('meta variant: payload not in parsed value');
  return { text, replaced };
}

/**
 * 画面に出ていないことを判定する断片の一覧。P 全体（U+0000 を除く）・`XSS-NN`・制御文字や空白で切った 6 文字以上の断片。
 * 空なら例外にする（何も見ない判定の防止）。
 */
export function payloadProbes(P) {
  const stripped = P.replaceAll('\u0000', '');
  const out = new Set();
  if (stripped.length > 0 && !/^[\p{Cc}\p{Cf}\s]*$/u.test(stripped)) out.add(stripped);
  const id = /XSS-\d+/.exec(P)?.[0];
  if (id) out.add(id);
  for (const part of stripped.split(/[\p{Cc}\p{Cf}\s]+/u)) {
    if ([...part].length >= 6) out.add(part);
  }
  if (out.size === 0) throw new Error('payloadProbes: no probe');
  return [...out];
}

// 画面を読む（ページ内で動く自己完結の関数。外の名前を参照しない）。HTML 全体・文字・全要素の全属性の値・入力欄の値を返す。
export function observeLeak() {
  const attrs = [];
  for (const e of document.querySelectorAll('*')) {
    for (const a of e.attributes) attrs.push(a.value);
  }
  const values = [...document.querySelectorAll('input, textarea')].map((e) => e.value);
  return {
    html: document.documentElement.outerHTML,
    text: document.body.innerText,
    textContent: document.body.textContent ?? '',
    attrs,
    values,
  };
}

// 読んだ値のどれかに断片が出ていれば、出た場所の名前の一覧を返す（断片の中身は返さない）。
export function leakedPlaces(obs, probes) {
  const has = (s) => probes.some((p) => String(s).replaceAll('\u0000', '').includes(p));
  const places = [];
  if (has(obs.html)) places.push('html');
  if (has(obs.text)) places.push('text');
  if (has(obs.textContent)) places.push('textContent');
  if (obs.attrs.some(has)) places.push('attrs');
  if (obs.values.some(has)) places.push('values');
  return places;
}
