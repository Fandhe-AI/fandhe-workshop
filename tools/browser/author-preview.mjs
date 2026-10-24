// compile した IR の手元の配信と章の完走の試験（author-preview.test.mjs。TASK-427 1/2）の補助モジュール。
//
// 役割: 試験本体が使う Step の表・DOM だけの観察・章の先頭の確認・IR の構造の要約を置く。`.test.mjs` を import すると
// 試験が二重に登録されるため、試験でない本モジュールに分ける。
// 試験 E（章の開始点への直接リンク。TASK-427 2/2）が使う開始点の表と位置の期待値も置く。
// 呼び出し元: author-preview.test.mjs だけ。操作の列（journey）・ドライバ・台帳は ch0-3-journey.mjs のものを使う。
// DOM の判定は ch4-9-journey.mjs の inspectDom49 を題の表だけ差し替えて使う（判定を二重に書かない）。
// inspectDom49 の判定を変えるときは、CH-0〜CH-3 側（本試験と csp-runs・ch4-9-run）も流して確かめること。
import { T } from './ch0-3-journey.mjs';
import { inspectDom49 } from './ch4-9-journey.mjs';

// 試験用の形式 B 教材（crates/web/tests/data/ch0_3_pack）の 20 Step。題は教材と一致させる（P5 の検査が崩れを検出する）。
export const STEPS = [
  ['main/st-0-1', '画面の右側を選ぶ'],
  ['main/st-0-2a', '1 つ目の確認'],
  ['main/st-0-2b', '2 つ目の確認'],
  ['main/st-0-2c', '3 つ目の確認'],
  ['main/st-1-1', '/help を送る'],
  ['main/st-1-2', 'Tab で補完する'],
  ['main/st-1-3', '2 行のメモを送る'],
  ['main/st-1-4', '引数なしの /new-idea と、順序違いの /advance-phase'],
  ['main/st-1-5', '処理中に待つ'],
  ['main/st-1-6', '選択肢を選ぶ'],
  ['main/st-2-1', '名前つきのコマンド'],
  ['main/st-2-2', '表示名で選ぶ'],
  ['main/st-2-3', '計画を承認する'],
  ['main/st-2-4', '複数行の自然文'],
  ['main/st-2-5', '検査を流す'],
  ['main/st-2-6', '直す対象を決める'],
  ['main/st-2-7', '左の行を指す'],
  ['main/st-2-8', '章の終わり'],
  ['main/st-3-1', '置き場所を選ぶ'],
  ['main/st-3-2', '引き継ぎを承認する'],
];
export const ORDER = STEPS.map(([uid]) => uid);
export const TITLES = Object.fromEntries(STEPS);
// 各章の最初の Step。受け入れ条件の「各章の最初の Step にいまやることと本文が出る」の対象。
export const FIRST_STEPS = ['main/st-0-1', 'main/st-1-1', 'main/st-2-1', 'main/st-3-1'];
export const CHAPTER_ENDS = ['main/st-0-2c', 'main/st-1-6', 'main/st-2-8'];
export const LAST = 'main/st-3-2';

/** 配信する設定の観察（DOM だけ）。`__fandheTest` が無いので世代の確認は置かない（hasState: false）。 */
export function domObserver03() {
  return {
    async at(page, uid, status, timeout = T) {
      await page.waitForFunction(inspectDom49, [TITLES, uid, status, true], { timeout })
        .catch((e) => { throw new Error(`DOM が ${uid} の ${status} を示さない: ${String(e.message).slice(0, 200)}`); });
    },
    async stillAt(page, uid) {
      const { d } = await page.evaluate(inspectDom49, [TITLES, uid, 'entered']);
      return d.title === TITLES[uid] && d.lastStep === uid && (d.lastKind === 'rejected' || d.lastKind === 'clarified');
    },
    hasState: false,
  };
}

/** DOM の判定そのもの（較正の試験が「次の Step を示さない」を確かめるのに使う）。 */
export const inspect = (page, uid, status) => page.evaluate(inspectDom49, [TITLES, uid, status]);

/**
 * 章の最初の Step に入った直後の画面を確かめる。戻りは [名前, 合否] の列で、台帳に積む。
 * 「いまやること」の固定の見出しと IR の do、本文の区画が見えて空でないこと、その Step の本文の先頭の見出しの文字が出ることを見る。
 * 教材の本文は各 Step の題を先頭の見出し（`# 題`）にしてあるので、題が見出しの要素に出ることを期待する。入力した値は扱わない。
 */
export async function inspectFirstStep(page, doText, title) {
  const r = await page.evaluate(() => {
    const vis = (e) => {
      if (!e) return false;
      const b = e.getBoundingClientRect();
      const c = getComputedStyle(e);
      return b.width > 0 && b.height > 0 && c.display !== 'none' && c.visibility !== 'hidden';
    };
    const doEl = document.querySelector('[data-testid="step-do"]');
    const body = document.querySelector('[data-testid="step-body"]');
    return {
      doText: doEl?.textContent ?? '',
      doVisible: vis(doEl),
      bodyVisible: vis(body),
      bodyText: (body?.textContent ?? '').trim(),
      headings: body ? [...body.querySelectorAll('h1, h2')].map((h) => h.textContent ?? '') : [],
    };
  });
  return [
    ['いまやることが見える', r.doVisible],
    ['いまやることの見出しと do', r.doText === `いまやること${doText}`],
    ['本文の区画が見える', r.bodyVisible],
    ['本文が空でない', r.bodyText.length > 0],
    ['本文の見出しに題が出る', r.headings.includes(title)],
  ];
}

/** IR の開始点を { id, node } の列で返す。試験 E が `?start=<id>` を作る元で、FIRST_STEPS と順に突き合わせる。 */
export const startLinksOf = (ir) => ir.starts.map((s) => ({ id: s.id, node: s.node }));

/**
 * 節点の位置の表示の期待値「<章の ID>　Step M / K」を IR から求める（区切りは全角スペース。shell-position と同じ形）。
 * K は同じ章（unit）の節点の数、M は章の中での順番（1 始まり）。章ごとの数は決め打ちしない。
 */
export function positionOf(ir, uid) {
  const node = ir.nodes.find((n) => n.uid === uid);
  const inUnit = ir.nodes.filter((n) => n.unit === node.unit);
  return `${node.unit}　Step ${inUnit.findIndex((n) => n.uid === uid) + 1} / ${inUnit.length}`;
}

// 構造の要約。compile した IR と、手書きの ch0_3/ir.json の変種を同じ形に要約して突き合わせる。
// 比べるのは、journey の操作の列が前提にする欄（uid・章・題・do・入力の種類と選択肢・確認の種類・規則の ID・final・next・processing・開始点）。
// compile と手書きで直列化の仕方が違う欄（final: null と false など）は真偽に揃える。body・body_doc は比べない。
export function summarizeIr(ir) {
  return {
    starts: ir.starts.map((s) => s.node),
    nodes: ir.nodes.map((n) => ({
      uid: n.uid,
      unit: n.unit,
      title: n.step.title,
      do: n.step.do ?? null,
      input: n.step.input ? { kind: n.step.input.kind, options: (n.step.input.options ?? []).map((o) => o.id) } : null,
      confirm: n.step.confirm?.kind ?? null,
      rules: (n.step.rules ?? []).map((r) => r.id),
      final: n.step.final === true,
      next: (n.step.next ?? []).map((t) => t.to),
      processing: (n.step.processing ?? []).length,
    })),
  };
}

// 手書きの 20 Step の IR（ch0_3/ir.json）を、形式 B の教材と同じ変種へ書き換える。各章の最後を final にして next を除き、
// 開始点を各章の先頭の 4 件にする（test_fixture.rs の ch03_chapters_ir_bytes と同じ形。他の章を指す next は E-REF-STEP になるため）。
export function variantOf(handIr) {
  const ends = new Set([...CHAPTER_ENDS, LAST]);
  const copy = JSON.parse(JSON.stringify(handIr));
  for (const n of copy.nodes) {
    if (ends.has(n.uid)) {
      n.step.final = true;
      n.step.next = [];
    }
  }
  copy.starts = FIRST_STEPS.map((node) => ({ id: node.replace('main/st-', 's-'), node, effects: [] }));
  return copy;
}
