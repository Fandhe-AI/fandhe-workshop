// Tab による入力欄の出入りの観察の共通の部品（TASK-373。A11Y-16・JUDGE-19）。
//
// 呼び出し元: tab-exit.test.mjs（1/2）と、同じ部品を使う予定の Shift+Tab・一巡の試験（2/2）、自己確認の tab-focus.test.mjs。
// 観察は DOM と defaultPrevented だけで行い、__fandheTest は読まない（試験用の口が無い画面でも使えるように）。
// page へ渡すのは固定の関数と、文字列・配列の引数だけにする。DOM には属性を足さず、保持は window の変数で行う。
export const OUT = 'OUT';

// フォーカスできる要素を DOM の順に列挙して window に保持し、testid（無ければ tagName）の一覧を返す。
// 選び方は tab-cycle.test.mjs と同じ。
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
