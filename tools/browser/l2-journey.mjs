// L2 の完走試験（l2-run.test.mjs）と、その DOM の目印の較正（l2-journey-calibrate.test.mjs）が共有する、
// DOM だけで CH-0〜CH-1 を進める誘導（TASK-112 2/2。DLV-13）。
//
// 役割: 配信する設定には `__fandheTest`（試験の口）が無いため、`ch0-3-journey.mjs` の journey が使う状態の観察
// （「いま <uid> が入力待ち・確認待ちか」「拒否の後も <uid> のままか」）を、画面の DOM だけから導く観察に差し替える。
// 操作の列は journey を共有し（二重に書かない）、操作は mouseDriver（実クリックと入力欄への fill・Enter）だけを使う。
// `raw-trigger` などの隠し要素は使わない。
//
// DOM の目印（較正の試験が試験の設定の stateJson と各 Step で一致することを確かめる）:
//   - 確認待ち: `step-status` の語が「受付済み（確認待ち）」で、`run-status` が idle で、ログの最後の行の `data-step` が当の Step
//   - 入力待ち: `step-status` の語が「達成」で（直前の Step の達成の後に次の Step へ入った状態）、idle で、
//     ログの最後の行の `data-step` が直前の Step。先頭の Step はログが 0 行
//   - 拒否・問い返しの後に動かない: ログの最後の行が拒否（rejected）か問い返し（clarified）で、その `data-step` が当の Step
//   - 終点（`main/st-2-1`）の入力待ちは、上に加えて入力の口が下書き・送信の各 1 個で選択肢が 0 個（直前の選択肢の Step で止まった状態を除く）
// 現在の Step の ID は DOM に出ないため、IR の node の並び（章をまたぐ無条件の `next` で線形）から直前の Step を引く。
import { journey, mouseDriver, DRAFT } from './ch0-3-journey.mjs';

export { DRAFT };

const T = 30000;
export const L2_STOP = 'main/st-2-1';

/** IR の JSON から node の uid の並びを返す。 */
export function stepOrder(irJson) {
  return JSON.parse(irJson).nodes.map((n) => n.uid);
}

/**
 * 画面を読み、`uid` が `status`（entered・accepted）かを判定する。ページ内で評価する自己完結の関数（外の名前を参照しない。
 * Playwright が関数を文字列にして渡すため。`Function`・`eval` は CSP が止めるので使わない）。
 * 戻りは JSON にできる値だけ（`d` は読み取り、`ok` は判定。末尾の引数 `waiting` が真なら `ok` だけ）。
 */
export function inspectDom([order, uid, status, waiting = false]) {
  const rows = [...document.querySelectorAll('[data-testid="part-log-list"] li')];
  const last = rows.length === 0 ? null : rows[rows.length - 1];
  const d = {
    word: document.querySelector('[data-testid="step-status"]')?.textContent ?? '',
    run: document.querySelector('[data-testid="run-status"]')?.getAttribute('data-state') ?? '',
    rows: rows.length,
    lastStep: last?.getAttribute('data-step') ?? null,
    lastKind: last?.getAttribute('data-kind') ?? null,
    choices: document.querySelectorAll('[data-testid="choice-btn"]').length,
    draft: document.querySelectorAll('[data-testid="draft-input"]').length,
    submit: document.querySelectorAll('[data-testid="submit-input"]').length,
  };
  const i = order.indexOf(uid);
  let ok = false;
  if (d.run === 'idle' && i >= 0) {
    if (status === 'accepted') ok = d.word === '受付済み（確認待ち）' && d.lastStep === uid;
    else if (status === 'entered') {
      ok = i === 0
        ? d.rows === 0 || (d.word === '' && d.lastStep === uid)
        : d.word === '達成' && d.lastStep === order[i - 1];
      // 終点（main/st-2-1。コマンドの入力）は、直前の main/st-1-6（選択肢）の達成の画面と入力の口で区別する。
      // 直前の Step の達成だけでは、次の Step への遷移が欠落した状態（直前で止まった状態）も同じ語・同じログになる。
      if (ok && uid === 'main/st-2-1') ok = d.choices === 0 && d.draft === 1 && d.submit === 1;
    }
  }
  // 待つ呼び出し（waitForFunction）は真偽だけを返し、読む呼び出しは読み取りと判定を返す。
  return waiting ? ok : { d, ok };
}

/** journey の opts.observe に渡す、DOM だけの観察。`order` は IR の node の並び。 */
export function domObserver(order) {
  return {
    async at(page, uid, status, timeout = T) {
      // 文字列の式は CSP（unsafe-eval なし）が止めるため、関数のまま渡す（引数の末尾の true が「待つ」呼び出し）。
      await page.waitForFunction(inspectDom, [order, uid, status, true], { timeout })
        .catch((e) => { throw new Error(`DOM が ${uid} の ${status} を示さない: ${String(e.message).slice(0, 200)}`); });
    },
    async stillAt(page, uid) {
      const { d } = await page.evaluate(inspectDom, [order, uid, 'entered']);
      // 確認待ちの Step でも拒否は起きる（語は「受付済み」のまま）ため、語でなく最後の行の種類（拒否・問い返し）で見る。
      return d.lastStep === uid && (d.lastKind === 'rejected' || d.lastKind === 'clarified');
    },
  };
}

/**
 * CH-0 から CH-1 を通し、`main/st-2-1` に入ったところで返す。操作は mouseDriver。観察は既定で DOM だけ（`observe` は較正の試験が
 * 観察を包んで差し替える口）。
 */
export async function runL2JourneyWith(page, L, { order, onStep = null, scene = 'L2', observe = domObserver(order) } = {}) {
  await journey(page, mouseDriver(page), scene, L, L2_STOP, { observe, onStep });
}

/** [runL2JourneyWith] の既定の形（DOM だけの観察）。 */
export function runL2Journey(page, L, opts = {}) {
  return runL2JourneyWith(page, L, opts);
}
