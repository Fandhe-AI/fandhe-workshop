// Tab による入力欄の出入りの観察の共通の部品（TASK-373。A11Y-16・JUDGE-19）。
//
// 呼び出し元: tab-exit.test.mjs（1/2）、tab-trap.test.mjs（Shift+Tab・先頭からの一巡・罠 0 件。2/2）、tab-cycle.test.mjs、自己確認の tab-focus.test.mjs。
// 観察は DOM と defaultPrevented だけで行い、__fandheTest は読まない（試験用の口が無い画面でも使えるように）。
// page へ渡すのは固定の関数と、文字列・配列の引数だけにする。DOM には属性を足さず、保持は window の変数で行う。
export const OUT = 'OUT';

// フォーカスできる要素を DOM の順に列挙して window に保持し、testid（無ければ tagName）の一覧を返す。
// tab-cycle.test.mjs もこの列挙を使う（選び方を 1 か所に保つため）。
export async function listFocusables(page) {
  return page.evaluate(() => {
    const els = [...document.querySelectorAll('a[href], button, textarea, input, select, [tabindex]')]
      .filter((e) => e.tabIndex >= 0 && !e.disabled && e.getClientRects().length > 0);
    window.__tabFocusables = els;
    return {
      ids: els.map((e) => e.getAttribute('data-testid') ?? e.tagName),
      positive: els.filter((e) => e.tabIndex > 0).length,
    };
  });
}

// いまのフォーカスの位置。body か無ければ OUT、一覧に無い要素なら -1。
export async function focusIndex(page) {
  return page.evaluate(() => {
    const a = document.activeElement;
    if (!a || a === document.body) return 'OUT';
    return window.__tabFocusables.indexOf(a);
  });
}

// フォーカス中の要素の testid。body なら 'BODY'、testid が無ければ tagName。
export async function activeTestId(page) {
  return page.evaluate(() => {
    const a = document.activeElement;
    if (!a || a === document.body) return 'BODY';
    return a.getAttribute('data-testid') ?? a.tagName;
  });
}

// window の bubble 段で Tab の keydown の defaultPrevented を記録する listener を付ける。
// アプリの listener より後に走る段なので、アプリが preventDefault した結果を見られる。
export async function installTabProbe(page) {
  await page.evaluate(() => {
    window.__tabPrevented = [];
    window.addEventListener('keydown', (e) => {
      if (e.key === 'Tab') window.__tabPrevented.push(e.defaultPrevented);
    });
  });
}

// key を 1 回押し、押す前後の位置と、その押下で記録された defaultPrevented の列を返す。
export async function pressTracked(page, key) {
  const before = await focusIndex(page);
  await page.evaluate(() => { window.__tabPrevented = []; });
  await page.keyboard.press(key);
  const after = await focusIndex(page);
  const prevented = await page.evaluate(() => window.__tabPrevented.slice());
  return { before, after, prevented };
}

// listFocusables の一覧の index 番目の要素にフォーカスする。testid が重複する要素（ビューの選択ボタン）でも位置で指せる。
export async function focusAt(page, index) {
  await page.evaluate((i) => window.__tabFocusables[i].focus(), index);
}

// key を presses 回押し、押すたびの位置の列 seq と、全押下の defaultPrevented の列 prevented を返す。
// installTabProbe を済ませ、listFocusables で一覧を作っておくこと（listener は二重に付けない）。
export async function walkTracked(page, key, presses) {
  await page.evaluate(() => { window.__tabPrevented = []; });
  const seq = [];
  for (let i = 0; i < presses; i += 1) {
    await page.keyboard.press(key);
    seq.push(await focusIndex(page));
  }
  return { seq, prevented: await page.evaluate(() => window.__tabPrevented.slice()) };
}

// 位置の列 seq（始点 start から dir 向きに歩いた結果）の問題の一覧を返す。空なら一巡して罠が無い。
// 文書の外（OUT）へ一度出ることは罠に数えない（K-15）。n は列挙した要素の数。
// body から歩き始めるときは始点を 1 つ手前にする（Tab なら start = n - 1・dir = 1、Shift+Tab なら start = 0・dir = -1）。
export function findCycleProblems(seq, n, start, dir) {
  const problems = [];
  let cur = start;
  let prevOut = false;
  seq.forEach((pos, i) => {
    if (pos === OUT) {
      if (prevOut) problems.push(`press ${i + 1}: stuck outside the document`);
      prevOut = true;
      return;
    }
    prevOut = false;
    const expected = (((cur + dir) % n) + n) % n;
    if (pos === -1) problems.push(`press ${i + 1}: focus moved to an unlisted element`);
    else if (pos === cur) problems.push(`press ${i + 1}: focus did not move (trap at ${pos})`);
    else if (pos !== expected) problems.push(`press ${i + 1}: expected ${expected} but got ${pos}`);
    cur = pos;
  });
  const first = seq.slice(0, n + 1);
  if (!first.includes(start)) problems.push(`did not return to start within ${n + 1} presses`);
  for (let k = 0; k < n; k += 1) {
    if (!first.includes(k)) problems.push(`element ${k} was not visited in the first cycle`);
  }
  return problems;
}
