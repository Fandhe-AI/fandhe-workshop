// CH-0〜CH-3 の形の 20 Step の誘導（通しとキーボードだけの区間）の補助モジュール（TASK-450 3/3 で作り、TASK-151 2/2 で移した）。
//
// 役割: ch0-3-run.test.mjs（通しの合否）と csp-runs.test.mjs（SAFE-13 の CSP あり・なしの計数）が、同じ誘導の操作を共有する。
// `.test.mjs` を import すると試験が二重に登録されるため、試験でない本モジュールに置く。
// 操作は 2 種のドライバ（mouseDriver: page.click・fill・press、keyboardDriver: page.keyboard だけ）と journey が行い、
// 合否は台帳 ledger に積む。journey の opts.onStep は、各 Step に入った（または確認待ちになった）直後に呼ぶ口で、
// csp-runs が Step ごとに画面を走査するのに使う（ch0-3-run は渡さず、従来どおり）。入力した値は台帳に載せない。
// paneAware は、960px 未満で tablist が見えているときだけ、操作の前に要るタブを選ぶドライバの包みで、shell-widths と
// shell-journey（TASK-548 1/2）が共有する。keyPaneAware はそのキーボード版で shell-boundary（TASK-369）が使う。「次の章へ進む」は操作の領域の末尾にあるので、操作のタブを選んでから押す。
import assert from 'node:assert/strict';

export const DRAFT = '[data-testid="draft-input"]';
export const SUBMIT = '[data-testid="submit-input"]';
export const CHOICE = '[data-testid="choice-btn"]';
export const APPROVAL = '[data-testid="approval-btn"]';
export const RAW = '[data-testid="raw-trigger"]';
export const LOG = '[data-testid="part-log-list"]';
export const REJECTED = `${LOG} li[data-kind="rejected"]`;
export const CLARIFIED = `${LOG} li[data-kind="clarified"]`;
export const NEXT = '[data-testid="shell-next-chapter"]';
export const T = 30000;
const TAB_LIMIT = 300;

export const engine = (page) => page.evaluate(() => JSON.parse(window.__fandheTest.stateJson()).engine);

// 現在の Step が `uid` で、状態が `status` で、処理中でないのを待つ。`entered` は状態の欠落も含める。
export const at = (page, uid, status) => page.waitForFunction(([u, st]) => {
  const e = JSON.parse(window.__fandheTest.stateJson()).engine;
  return e.current === u && (e.stateWithoutLog.run.slices.main?.status?.[u] ?? 'entered') === st
    && window.__fandheTest.nextTick() === 'null';
}, [uid, status], { timeout: T });

// 状態の観察の既定（試験の設定の `__fandheTest` を読む）。配信する設定には `__fandheTest` が無いため、
// L2 の完走試験（l2-journey.mjs）は DOM だけを読む観察を `journey` の opts.observe で差し替える（TASK-112 2/2）。
export const hookObserver = {
  at,
  stillAt: async (page, uid) => (await engine(page)).current === uid,
};

export const rowSel = (path) => `[data-testid="part-artifact-view"] [data-row="${path}"]`;
export const choiceSel = (id) => `${CHOICE}[data-choice="${id}"]`;
export const decideSel = (d) => `${APPROVAL}[data-decision="${d}"]:not([data-revise-target])`;
export const reviseSel = (t) => `${APPROVAL}[data-revise-target="${t}"]`;

// 4 つの口の数を (選択肢, 承認, 入力欄, 送信) の順で返す。
export const ports = (page) => page.evaluate(([c, a, i, s]) => [c, a, i, s].map((sel) => document.querySelectorAll(sel).length),
  [CHOICE, APPROVAL, DRAFT, SUBMIT]);

export const waitCount = (page, sel, n) => page.waitForFunction(([s, k]) => document.querySelectorAll(s).length === k, [sel, n], { timeout: T });
export const draftValue = (page) => page.inputValue(DRAFT);

// 項目の台帳。途中の合否は台帳に積み、最後に全項目の合格を一度に確かめる。入力した値は載せない。
export function ledger() {
  const items = [];
  return {
    items,
    check(scene, name, ok) {
      items.push({ scene, name, ok: ok === true });
    },
    summary() {
      return `${items.filter((i) => i.ok).length}/${items.length}`;
    },
    failed() {
      return items.filter((i) => !i.ok).map((i) => `${i.scene}: ${i.name}`);
    },
  };
}

// 実クリックの操作。`page.click` と入力欄への `page.fill`・`page.press`・`page.keyboard` だけを使う。
export function mouseDriver(page) {
  return {
    choose: (id) => page.click(choiceSel(id), { timeout: T }),
    point: (path) => page.click(rowSel(path), { timeout: T }),
    decide: (d) => page.click(decideSel(d), { timeout: T }),
    revise: (t) => page.click(reviseSel(t), { timeout: T }),
    async send(text) {
      await page.fill(DRAFT, text);
      await page.press(DRAFT, 'Enter');
    },
    async sendMultiline(first, second) {
      await page.fill(DRAFT, first);
      await page.press(DRAFT, 'Shift+Enter');
      await page.keyboard.type(second);
      await page.press(DRAFT, 'Enter');
    },
    async completeAndSend(prefix, completed) {
      await page.fill(DRAFT, prefix);
      await page.press(DRAFT, 'Tab');
      await page.waitForFunction(([sel, v]) => document.querySelector(sel)?.value === v, [DRAFT, completed], { timeout: T });
      await page.press(DRAFT, 'Enter');
    },
    esc: () => page.press(DRAFT, 'Escape'),
    nextChapter: () => page.click(NEXT, { timeout: T }),
  };
}

export async function activeMatches(page, sel) {
  return page.evaluate((s) => document.activeElement?.matches(s) ?? false, sel);
}

// key を押して sel に着くまで進める。押した回数には上限があり、回数そのものは固定しない。
export async function keyTo(page, sel, key = 'Tab') {
  for (let n = 0; n <= TAB_LIMIT; n += 1) {
    if (await activeMatches(page, sel)) return n;
    if (n === TAB_LIMIT) break;
    await page.keyboard.press(key);
  }
  assert.fail(`${key} で ${TAB_LIMIT} 回押しても ${sel} に着かない`);
  return -1;
}

// 入力欄にフォーカスを移し、残っている下書き（拒否の後は残る）を全選択して、次の入力で置き換わるようにする。
// 拒否の後は下書きが本番でも選択状態になる（TASK-284 2/2）が、通しの他の場面（受け付けの後の空欄など）でも無害で、
// ドライバの操作を変えると通しの他の段に影響するので、見直した結果として全選択は残す。選択の確認は reject-draft-select が受け持つ。
export async function replaceDraft(page) {
  await keyTo(page, DRAFT);
  await page.keyboard.press('ControlOrMeta+A');
}

// `page.keyboard` だけの操作。`page.click`・`page.fill`・`page.focus` は使わない。
export function keyboardDriver(page) {
  const activate = async (sel, key = 'Enter') => {
    await keyTo(page, sel);
    await page.keyboard.press(key);
  };
  return {
    choose: (id, key) => activate(choiceSel(id), key),
    point: (path) => activate(rowSel(path)),
    decide: (d) => activate(decideSel(d)),
    revise: (t) => activate(reviseSel(t)),
    async send(text) {
      await replaceDraft(page);
      await page.keyboard.type(text);
      await page.keyboard.press('Enter');
    },
    async sendMultiline(first, second) {
      await replaceDraft(page);
      await page.keyboard.type(first);
      await page.keyboard.press('Shift+Enter');
      await page.keyboard.type(second);
      await page.keyboard.press('Enter');
    },
    async completeAndSend(prefix, completed) {
      await replaceDraft(page);
      await page.keyboard.type(prefix);
      await page.keyboard.press('Tab');
      await page.waitForFunction(([sel, v]) => document.querySelector(sel)?.value === v, [DRAFT, completed], { timeout: T });
      await page.keyboard.press('Enter');
    },
    async esc() {
      await keyTo(page, DRAFT);
      await page.keyboard.press('Escape');
    },
    async nextChapter() {
      await keyTo(page, NEXT);
      await page.keyboard.press('Enter');
    },
  };
}

// 操作の前に、要るタブを選ぶドライバ。tablist が見えていないとき（2 列）は何もしない。見えているかは幅でなく計算された display で判定する。
// 選んだ回数を counter.clicks に積む。
export function paneAware(page, d, counter) {
  const ensure = async (id) => {
    const visible = await page.evaluate(() => {
      const e = document.querySelector('[data-testid="pane-tabs"]');
      return !!e && getComputedStyle(e).display !== 'none';
    });
    if (!visible) return;
    const selected = await page.getAttribute(`#${id}`, 'aria-selected');
    if (selected === 'true') return;
    await page.click(`#${id}`);
    counter.clicks += 1;
  };
  const ops = (name) => async (...args) => {
    await ensure('pane-tab-ops');
    return d[name](...args);
  };
  return {
    point: async (...args) => {
      await ensure('pane-tab-artifacts');
      return d.point(...args);
    },
    choose: ops('choose'),
    decide: ops('decide'),
    revise: ops('revise'),
    send: ops('send'),
    sendMultiline: ops('sendMultiline'),
    completeAndSend: ops('completeAndSend'),
    esc: ops('esc'),
    nextChapter: ops('nextChapter'),
  };
}

// paneAware のキーボード版（shell-boundary。TASK-369）。tablist が見えているときだけ、Tab で選択中の tab へ辿り着き、矢印で反対側を選ぶ。
// 選んだ回数を counter.keys に積む。`page.click`・`page.focus` は使わない。
export function keyPaneAware(page, d, counter) {
  const ensure = async (id) => {
    const visible = await page.evaluate(() => {
      const e = document.querySelector('[data-testid="pane-tabs"]');
      return !!e && getComputedStyle(e).display !== 'none';
    });
    if (!visible) return;
    if ((await page.getAttribute(`#${id}`, 'aria-selected')) === 'true') return;
    await keyTo(page, '[data-testid="pane-tab"][aria-selected="true"]');
    await page.keyboard.press('ArrowRight');
    await page.waitForFunction((want) => {
      const t = document.getElementById(want);
      return t?.getAttribute('aria-selected') === 'true' && document.activeElement === t;
    }, id, { timeout: T });
    counter.keys += 1;
  };
  const ops = (name) => async (...args) => {
    await ensure('pane-tab-ops');
    return d[name](...args);
  };
  return {
    point: async (...args) => {
      await ensure('pane-tab-artifacts');
      return d.point(...args);
    },
    choose: ops('choose'),
    decide: ops('decide'),
    revise: ops('revise'),
    send: ops('send'),
    sendMultiline: ops('sendMultiline'),
    completeAndSend: ops('completeAndSend'),
    esc: ops('esc'),
    nextChapter: ops('nextChapter'),
  };
}

// 通し。`d` の操作で教材の 20 Step を上から進める。`stopAt` の Step に入った時点で返す（指定なしは最後の達成まで）。
// `chaptered` が真のとき（教材は `?fixture=ch0-3-chapters`）、章の境目は「次の章へ進む」のボタン 1 個を押して越える（TASK-526 2/2）。
// 偽（既定）のときは、章の境目を無条件の `next` が越えるのを待つだけで、従来の経路から変わらない。
export async function journey(page, d, scene, L, stopAt = null, { onStep = null, chaptered = false, observe = hookObserver } = {}) {
  const at = (pg, uid, status) => observe.at(pg, uid, status);
  let rejected = 0;
  let clarified = 0;
  const enter = async (uid, expected) => {
    await at(page, uid, 'entered');
    L.check(scene, `${uid} 入力待ちの口`, JSON.stringify(await ports(page)) === JSON.stringify(expected));
    if (onStep) await onStep(uid, 'entered');
  };
  const wait = async (uid, expected) => {
    await at(page, uid, 'accepted');
    L.check(scene, `${uid} 確認待ちの口`, JSON.stringify(await ports(page)) === JSON.stringify(expected));
    if (onStep) await onStep(uid, 'accepted');
  };
  const reject = async (uid) => {
    rejected += 1;
    await waitCount(page, REJECTED, rejected);
    L.check(scene, `${uid} 拒否で進まない`, await observe.stillAt(page, uid));
  };
  const clarify = async (uid) => {
    clarified += 1;
    await waitCount(page, CLARIFIED, clarified);
    L.check(scene, `${uid} 問い返しで進まない`, await observe.stillAt(page, uid));
  };
  const plain = [0, 0, 1, 1];
  // 章の境目。`chaptered` なら、章の最後の達成の後にボタンがちょうど 1 個出て「移動」ボタンが無いことを確かめ、押して次の章の先頭へ入る。
  const cross = async (last, next) => {
    if (!chaptered) {
      await at(page, next, 'entered');
      return;
    }
    await at(page, last, 'achieved');
    await waitCount(page, NEXT, 1);
    const info = await page.evaluate((sel) => ({
      texts: [...document.querySelectorAll(sel)].map((b) => b.textContent ?? ''),
      moves: [...document.querySelectorAll('button')].filter((b) => (b.textContent ?? '').startsWith('移動')).length,
    }), NEXT);
    L.check(scene, `${last} の完了後に次の章へ進むが 1 個`, info.texts.length === 1 && info.texts[0].startsWith('次の章へ進む: '));
    L.check(scene, `${last} の完了後に移動ボタンが 0 件`, info.moves === 0);
    // 世代は試験の設定の `__fandheTest` からだけ読める。配信する設定（DOM だけの観察。`observe.hasState` が偽）では世代の確認を置かず、
    // 次の章の先頭へ入ったこと（`at`）と、呼び出し側が onStep で見る位置・題の変化で境目を確かめる（author-preview。TASK-427 1/2）。
    const hasState = observe.hasState !== false;
    const before = hasState ? (await engine(page)).stateWithoutLog.run.generation : 0;
    await d.nextChapter();
    await at(page, next, 'entered');
    if (hasState) {
      const after = (await engine(page)).stateWithoutLog.run.generation;
      L.check(scene, `${next} へ移って世代が進む`, after > before);
    }
    await waitCount(page, NEXT, 0);
  };

  // CH-0: 選択肢（3 件）と、答える確認 3 つ。
  await enter('main/st-0-1', [3, 0, 1, 1]);
  await d.choose('opt-left');
  await reject('main/st-0-1');
  await d.choose('opt-top');
  await reject('main/st-0-1');
  await d.choose('opt-right');
  await wait('main/st-0-1', plain);
  await d.point('cards/notice');
  for (const [uid, next, read, wrong, right] of [
    ['main/st-0-2a', 'main/st-0-2b', 'opt-read-a', 'ans-a-ng', 'ans-a-ok'],
    ['main/st-0-2b', 'main/st-0-2c', 'opt-read-b', null, 'ans-b-ok'],
    ['main/st-0-2c', 'main/st-1-1', 'opt-read-c', null, 'ans-c-ok'],
  ]) {
    await enter(uid, [1, 0, 1, 1]);
    await d.choose(read);
    await wait(uid, [2, 0, 1, 1]);
    if (wrong !== null) {
      await d.choose(wrong);
      await reject(uid);
    }
    await d.choose(right, uid === 'main/st-0-2b' ? 'Space' : 'Enter');
    if (uid === 'main/st-0-2c') await cross(uid, next);
    else await at(page, next, 'entered');
  }

  // CH-1: コマンド・補完・複数行・問い返しと取り消し・処理中・選択肢。
  await enter('main/st-1-1', plain);
  await d.send('/hepl');
  await reject('main/st-1-1');
  await d.send('/help');
  await wait('main/st-1-1', plain);
  await d.point('cards/help-1');
  await enter('main/st-1-2', plain);
  await d.completeAndSend('/he', '/help');
  await enter('main/st-1-3', plain);
  await d.sendMultiline('memo line 1', 'memo line 2');
  await enter('main/st-1-4', plain);
  if (stopAt === 'main/st-1-4') return;
  await d.send('/new-idea');
  await clarify('main/st-1-4');
  // 問い返しの後は欄が空になる（JUDGE-27・TERM-22。D-67）。書き戻しは 0 ミリ秒のタイマーで非同期のため、空になるのを待つ。
  const emptied = await page.waitForFunction((sel) => document.querySelector(sel)?.value === '', DRAFT, { timeout: T })
    .then(() => true, () => false);
  L.check(scene, 'main/st-1-4 問い返しの後の入力欄が空', emptied);
  await d.send('/advance-phase');
  await reject('main/st-1-4');
  // 拒否の後は下書きの全体が選択状態になる（TASK-284 2/2。JUDGE-27）。選択は 0 ミリ秒のタイマーで付くので待つ。
  const selected = await page.waitForFunction(
    (sel) => {
      const el = document.querySelector(sel);
      return el?.value === '/advance-phase' && el.selectionStart === 0 && el.selectionEnd === el.value.length;
    }, DRAFT, { timeout: T },
  ).then(() => true, () => false);
  L.check(scene, 'main/st-1-4 拒否の後の下書きが全体の選択状態', selected);
  await d.esc();
  await wait('main/st-1-4', [2, 0, 1, 1]);
  await d.choose('asked-back');
  await enter('main/st-1-5', plain);
  await d.send('/make-card sample');
  await wait('main/st-1-5', plain);
  await d.point('cards/sample');
  await enter('main/st-1-6', [2, 0, 1, 1]);
  await d.choose('opt-right');
  await wait('main/st-1-6', [2, 0, 1, 1]);
  await d.choose('ans-restored');
  await cross('main/st-1-6', 'main/st-2-1');
  if (stopAt === 'main/st-2-1') return;

  // CH-2: コマンド・表示名の選択・承認・自然文・処理中・対象つきの直す・画面操作。
  await enter('main/st-2-1', plain);
  await d.send('/new-idea ideas');
  await wait('main/st-2-1', plain);
  await d.point('docs/readme');
  await enter('main/st-2-2', [1, 0, 1, 1]);
  await d.choose('opt-view');
  await wait('main/st-2-2', [2, 0, 1, 1]);
  await d.choose('ans-readme');
  await enter('main/st-2-3', [0, 3, 1, 1]);
  await d.decide('approve');
  await wait('main/st-2-3', plain);
  await d.point('docs/plan');
  await enter('main/st-2-4', plain);
  await d.sendMultiline('status line 1', 'status line 2');
  await enter('main/st-2-5', plain);
  await d.send('/run-check');
  await wait('main/st-2-5', plain);
  await d.point('checks/result');
  await enter('main/st-2-6', [0, 4, 1, 1]);
  await d.decide('revise');
  await reject('main/st-2-6');
  await d.decide('approve');
  await reject('main/st-2-6');
  await d.revise('t-expect');
  await wait('main/st-2-6', [0, 3, 1, 1]);
  await d.decide('approve');
  await enter('main/st-2-7', plain);
  await d.point('tasks/t-2');
  await reject('main/st-2-7');
  await d.point('tasks/t-1');
  await wait('main/st-2-7', [2, 0, 1, 1]);
  await d.choose('ans-t2');
  await reject('main/st-2-7');
  await d.choose('ans-t1');
  await enter('main/st-2-8', [1, 0, 1, 1]);
  await d.choose('opt-go');
  await cross('main/st-2-8', 'main/st-3-1');

  // CH-3: 選択肢の誤答と指す確認、最後の承認。
  await enter('main/st-3-1', [2, 0, 1, 1]);
  await d.choose('opt-place-ng');
  await reject('main/st-3-1');
  await d.choose('opt-place-ok');
  await wait('main/st-3-1', plain);
  await d.point('work/place');
  await enter('main/st-3-2', [0, 3, 1, 1]);
  await d.decide('approve');
  await wait('main/st-3-2', [0, 3, 1, 1]);
  await d.decide('approve');
  await at(page, 'main/st-3-2', 'achieved');
}

// 信頼されたマウスのイベントと、`raw-trigger` の click を数える（capture）。
export async function install(page) {
  await page.evaluate((raw) => {
    window.__run = { mouse: 0, raw: 0 };
    for (const t of ['pointerdown', 'mousedown']) {
      window.addEventListener(t, (e) => {
        if (e.isTrusted) window.__run.mouse += 1;
      }, true);
    }
    // Enter・Space によるボタンの click も信頼されたイベントになる。マウスの click は detail が 1 以上、キーでは 0 である。
    window.addEventListener('click', (e) => {
      if (e.isTrusted && e.detail > 0) window.__run.mouse += 1;
    }, true);
    window.addEventListener('click', (e) => {
      if (e.target instanceof Element && e.target.closest(raw)) window.__run.raw += 1;
    }, true);
  }, RAW);
}

