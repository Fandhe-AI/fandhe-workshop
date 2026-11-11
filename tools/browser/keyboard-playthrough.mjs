// 教材の通し（tests/playthrough.yaml の 1 本）を、キーボードだけで最後まで画面に流して判定する部品（TASK-92 2/2。BUILD-17）。
//
// 役割: 1/2 の部品（keyboard-only.mjs の keyOnlyDriver・playStep・guardPage・installInputCounter）の上に、通しの流し・各操作の期待の判定・
// 区間の境目（「次の章へ進む」）・終わりの条件を置く。教材には依存しない。通しの ID も教材の知識も持たず、入力は通しの JSON
// （crates/pack/tests/playthrough_emit.rs が fandhe-pack の読み込み器を通して書き出す）と IR だけである。
// 呼び出し元: keyboard-playthrough.test.mjs（自 repo の試験用教材での自己確認）と keyboard-playthrough-run.mjs
// （scripts/check-keyboard-playthrough.sh が呼ぶ。教材 repo の教材を流す入口）。
//
// 観察は DOM の属性と件数だけで、文字の本文は返さない（題の照合だけは題の文字列を使うが、戻り値には載せない）。配信する設定（試験用の口なし）で動く。
// 判定の限界（docs/testing.md 5.508 にも書く）:
//   - 操作の結果は DOM の観察から分類する。受付（accepted）と達成（achieved）は、操作の前が確認待ちかどうかで分ける。
//     自然文の注記（accepted:free_text_unjudged）は受付として扱い、注記の有無は見ない。
//   - tick（progress）は 1 つずつ照合せず、連続する tick を 1 まとまりにして、直前の操作以降に進行の行が 1 件以上出て処理が終わったことで見る。
//   - asserts は DOM から範囲ごとの状態を読めないため判定しない。件数を unjudgedAsserts に返し、合格に数えない。
//   - 画面の現れ方が決められない期待（ignored など）は、未知の期待として止める（fail-closed）。
// `Function`・`eval` は使わない。selector に入れる ID は形を確かめてから使う。
import { playStep } from './keyboard-only.mjs';

// 確認待ちの語（terminal_area の step-status の表示。ch4-9-journey.mjs の inspectDom49 と同じ語）。
const WAITING = '受付済み（確認待ち）';
const DONE = '達成';
// selector や期待の語に入れてよい ID の形。合わなければ止める（CSS の注入の対策）。
const SAFE_ID = /^[A-Za-z0-9._@#/_-]{1,128}$/;

// ページ内で評価する観察。件数と属性だけを返し、行の本文は返さない。
export function snapshotInPage() {
  const rows = [...document.querySelectorAll('[data-testid="part-log-list"] li')];
  const next = [...document.querySelectorAll('[data-testid="shell-next-chapter"]')];
  const posEl = document.querySelector('[data-testid="shell-position"]');
  // 位置の文字は「<章の ID>（全角空白）Step M / K」。章の ID は文字から取る（属性には出ていない）。
  const posText = /^(.*)\u3000Step \d+ \/ \d+$/s.exec(posEl?.textContent ?? '');
  return {
    kinds: rows.map((r) => r.getAttribute('data-kind') ?? ''),
    cats: rows.map((r) => r.getAttribute('data-category')),
    // 行の指紋。畳みの要約行（data-fold）は 'fold|' で始める。件数ではなく中身で追記位置を決めるのに使う。
    fps: rows.map((r) => (r.hasAttribute('data-fold')
      ? `fold|${r.getAttribute('data-fold-rows') ?? ''}`
      : [r.getAttribute('data-kind'), r.getAttribute('data-gen'), r.getAttribute('data-category'), r.getAttribute('data-index'), r.getAttribute('data-total')].join('|'))),
    chapter: posText === null ? '' : posText[1],
    title: document.querySelector('[data-testid="step-title"]')?.textContent ?? '',
    word: document.querySelector('[data-testid="step-status"]')?.textContent ?? '',
    run: document.querySelector('[data-testid="run-status"]')?.getAttribute('data-state') ?? '',
    next: next.map((b) => b.getAttribute('data-payload') ?? ''),
    pos: posEl?.getAttribute('data-position-m') ?? '',
    posK: posEl?.getAttribute('data-position-k') ?? '',
  };
}

// IR から写しを作る。節点の uid → 題、ビューの ID → 種類、開始点の ID → 節点。
export function buildIndex(ir) {
  const titleOf = new Map(ir.nodes.map((n) => [n.uid, n.step.title]));
  const kindOf = new Map(Object.entries(ir.views ?? {}).map(([id, v]) => [id, v.kind]));
  const startNode = new Map(ir.starts.map((s) => [s.id, s.node]));
  // 章の中の順番 M と総数 K。shell（top_position.rs）と同じく、章 = scope が main で unit が同じ節点だけを数える。
  // モジュールの展開した節点は、呼び出した use の節点（instances[].caller）をたどって章の節点に読み替える。
  const mainOf = new Map();
  for (const n of ir.nodes) if (n.scope === 'main') mainOf.set(n.unit, [...(mainOf.get(n.unit) ?? []), n.uid]);
  const nodeOf = new Map(ir.nodes.map((n) => [n.uid, n]));
  const callerOf = new Map((ir.instances ?? []).map((i) => [i.id, i.caller ?? null]));
  const posOf = new Map();
  for (const n of ir.nodes) {
    let cur = n;
    let budget = (ir.instances ?? []).length + 1;
    while (cur !== undefined && cur.scope !== 'main' && budget > 0) {
      budget -= 1;
      const c = callerOf.get(cur.scope);
      cur = c === null || c === undefined ? undefined : nodeOf.get(c);
    }
    if (cur === undefined || cur.scope !== 'main') continue; // 導けない節点は位置なし（fail-closed）
    const uids = mainOf.get(cur.unit);
    posOf.set(n.uid, { chapter: cur.unit, m: String(uids.indexOf(cur.uid) + 1), k: String(uids.length) });
  }
  const sameTitle = (uid) => ir.nodes.filter((n) => n.step.title === titleOf.get(uid)).length;
  return { titleOf, kindOf, startNode, posOf, sameTitle };
}

// 期待の語を { kind, category } にする。知らない語は例外（fail-closed）。
export function parseExpect(s) {
  assertSafe(typeof s === 'string' ? s.replace(':', '/') : '', 'expect');
  const [head, tail] = s.split(':');
  if (head === 'accepted' && (tail === undefined || tail === 'free_text_unjudged')) return { kind: 'accepted' };
  if (head === 'rejected' && tail !== undefined) return { kind: 'rejected', category: tail };
  if (['clarified', 'achieved', 'progress'].includes(head) && tail === undefined) return { kind: head };
  throw new Error(`知らない期待: ${String(s).slice(0, 40)}`);
}

function assertSafe(v, what) {
  if (typeof v !== 'string' || !SAFE_ID.test(v)) throw new Error(`${what} の形が合わない（selector に入れない）`);
}

// 行の指紋の列。fps が無い観察（単体の試験）は kind と category から作る。
const fpsOf = (s) => s.fps ?? s.kinds.map((k, i) => `${k}|${s.cats?.[i] ?? ''}`);

// 操作の後に追記された行が after の何番目から始まるか。畳みの要約行が前後どちらにも無ければ件数で決める（縮んでいたら null）。
// 畳みがある（ログが 1,000 項目を超えて古い行が要約になった）ときは、件数が当てにならないので、
// 前の末尾の行の指紋（最大 5 件）が after の末尾から何件手前に並ぶかで決める。見つからなければ null（追跡できない。fail-closed）。
export function addedFrom(before, after) {
  const fb = fpsOf(before);
  const fa = fpsOf(after);
  const folding = fb.some((f) => f.startsWith('fold|')) || fa.some((f) => f.startsWith('fold|'));
  if (!folding) return fa.length < fb.length ? null : fb.length;
  const plain = fb.filter((f) => !f.startsWith('fold|')).slice(-5);
  if (plain.length === 0) return 0;
  // 末尾 5 件がそのまま残っていればそれで、古い側が畳まれて欠けていれば短い末尾（最短 1 件）で探す。
  for (let len = plain.length; len >= 1; len -= 1) {
    const tail = plain.slice(-len);
    for (let end = fa.length; end >= len; end -= 1) {
      if (tail.every((f, i) => fa[end - len + i] === f)) return end;
    }
  }
  return null;
}

// 操作の前後の観察から、結果の分類を出す純関数。戻りは 'rejected:<cat>'・'clarified'・'achieved'・'accepted'・'none'・'folded'。
export function classify(before, after) {
  const from = addedFrom(before, after);
  if (from === null) return 'folded';
  const added = after.kinds.slice(from);
  const rej = added.indexOf('rejected');
  if (rej >= 0) return `rejected:${after.cats[from + rej] ?? ''}`;
  if (added.includes('clarified')) return 'clarified';
  const waiting = before.word === WAITING;
  const moved = after.word === DONE || after.title !== before.title;
  if (waiting && moved) return 'achieved';
  if (!waiting && (after.word === WAITING || moved || added.length > 0)) return 'accepted';
  return 'none';
}

// 期待と分類が合うか。
export function matches(exp, got) {
  if (exp.kind === 'rejected') return got === `rejected:${exp.category}`;
  return exp.kind === got;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// 操作の後、処理が終わり（run-status が idle）、前から何かが変わり、観察が 2 回続けて同じになるまで待つ。
async function settle(page, before, timeout) {
  const deadline = Date.now() + timeout;
  let prev = null;
  while (Date.now() < deadline) {
    const s = await page.evaluate(snapshotInPage);
    const changed = fpsOf(s).join('\n') !== fpsOf(before).join('\n') || s.title !== before.title || s.word !== before.word;
    const key = JSON.stringify(s);
    if (s.run === 'idle' && changed && key === prev) return s;
    prev = key;
    await sleep(60);
  }
  return null;
}

async function waitFor(page, pred, timeout) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const s = await page.evaluate(snapshotInPage);
    if (pred(s)) return s;
    await sleep(60);
  }
  return null;
}

// 通し 1 本を流す。呼び出し側が区間 1 の開始点で画面を開いておく（既定と違うときは ?start=）。
// page は guardPage の戻りの page、driver は keyOnlyDriver。最初の不一致か止まった所で打ち切り、結果を返す。
// 戻り値: { segments, ops, matched, mismatches: [{ seg, op, expected, observed }], nextPresses, unjudgedAsserts, complete }。入力値・本文は含めない。
export async function runPlaythrough({ page, driver, ir, playthrough, observeTimeout = 15000 }) {
  const idx = buildIndex(ir);
  const segs = playthrough.segments;
  const res = {
    segments: segs.length, ops: 0, matched: 0, mismatches: [], nextPresses: 0, complete: false,
    unjudgedAsserts: Array.isArray(playthrough.asserts) ? playthrough.asserts.length : 0,
  };
  const stop = (seg, op, expected, observed) => {
    res.mismatches.push({ seg, op, expected, observed });
    return res;
  };
  // 先に全操作の形を確かめる（途中で selector に不正な値が入らないように）。
  for (const [si, seg] of segs.entries()) {
    assertSafe(seg.start, `segment ${si + 1} start`);
    for (const st of seg.steps) {
      for (const k of ['choice', 'target']) if (k in st) assertSafe(String(st[k]), k);
      if ('ui' in st) for (const part of String(st.ui).split(' ')) assertSafe(part, 'ui');
      if ('approval' in st && !['approve', 'reject', 'revise'].includes(st.approval)) throw new Error('approval の語が合わない');
      parseExpect(st.expect);
    }
  }
  for (const [si, seg] of segs.entries()) {
    const seq = si + 1;
    const node = idx.startNode.get(seg.start);
    if (node === undefined) return stop(seq, 0, 'start', 'unknown_start');
    if (si > 0) {
      const s = await waitFor(page, (x) => x.next.length === 1, observeTimeout);
      if (s === null) return stop(seq, 0, 'next_chapter', 'no_button');
      if (s.next[0] !== seg.start) return stop(seq, 0, `next:${seg.start}`, 'payload_mismatch');
      await driver.nextChapter();
      res.nextPresses += 1;
    }
    const entered = await waitFor(page, (x) => x.title === idx.titleOf.get(node) && x.run === 'idle' && x.next.length === 0, observeTimeout);
    if (entered === null) return stop(seq, 0, 'start_title', 'not_entered');
    let opBase = entered;
    for (let i = 0; i < seg.steps.length; i += 1) {
      const step = seg.steps[i];
      const exp = parseExpect(step.expect);
      const opNo = i + 1;
      if (exp.kind === 'progress') {
        // 連続する tick は 1 まとまり。最初の tick で、直前の操作以降の進行の行と処理の終わりを確かめる。
        if (i > 0 && parseExpect(seg.steps[i - 1].expect).kind === 'progress') {
          res.ops += 1;
          res.matched += 1;
          continue;
        }
        const s = await waitFor(page, (x) => x.run === 'idle', observeTimeout);
        const from = s === null ? null : addedFrom(opBase, s);
        const n = from === null ? 0 : s.kinds.slice(from).filter((k) => k === 'progress').length;
        res.ops += 1;
        if (n < 1) return stop(seq, opNo, 'progress', s === null ? 'busy' : from === null ? 'log_unaligned' : 'no_progress_row');
        res.matched += 1;
        continue;
      }
      const before = await page.evaluate(snapshotInPage);
      opBase = before;
      await playStep(driver, step, { viewKind: (v) => idx.kindOf.get(v) });
      res.ops += 1;
      const after = await settle(page, before, observeTimeout);
      if (after === null) return stop(seq, opNo, step.expect, 'no_change');
      const got = classify(before, after);
      if (!matches(exp, got)) return stop(seq, opNo, step.expect, got);
      res.matched += 1;
    }
  }
  // 終わりの条件。
  const last = await page.evaluate(snapshotInPage);
  const end = playthrough.end;
  if (end === 'complete') {
    if (last.word !== DONE) return stop(segs.length, 0, 'complete', 'not_complete');
  } else {
    const endUid = String(end);
    const want = idx.titleOf.get(endUid);
    if (want === undefined || last.title !== want) return stop(segs.length, 0, 'end_step', 'not_at_end');
    // 題は一意と限らない。章の ID と章内の位置（M・K）が実体のものと一致することまで、題が重ならなくても確かめる
    // （play_run.rs は end を現在の Step の実体 ID との完全一致で判定する）。位置が導けない節点で題が重なるときは止める（fail-closed）。
    const p = idx.posOf.get(endUid);
    if (p === undefined) {
      if (idx.sameTitle(endUid) > 1) return stop(segs.length, 0, 'end_step', 'ambiguous_title');
    } else {
      if (last.chapter === '' || last.pos === '' || last.posK === '') return stop(segs.length, 0, 'end_step', 'ambiguous_title');
      if (last.chapter !== p.chapter || last.pos !== p.m || last.posK !== p.k) return stop(segs.length, 0, 'end_step', 'not_at_end');
    }
  }
  res.complete = true;
  return res;
}

