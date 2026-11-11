// キーボードだけで画面を進める自動操作の部品と、マウスの click が 0 回であることの計数（TASK-92 1/2。BUILD-17）。
//
// 役割: BUILD-17 は「Tab・Enter・Space・Shift+Enter・Esc だけで全周を自動操作で進められ、そのときマウスの click が 0 回」を求める。
// 本モジュールは、その部品（許すキーの集合に閉じたキーボード・ドライバ側の監視・ページ内の計数）を試験の外へ置く。
// 呼び出し元は keyboard-only.test.mjs（本件の試験）と、keyboard-playthrough.mjs（教材の通しの流し。2/2）。`.test.mjs` を import すると
// 試験が二重に登録されるため、試験でない本モジュールに置く。journey（ch0-3-journey.mjs・ch4-9-journey.mjs）の操作の口と同じ名前の
// ドライバを返すので、journey の本体は変えずに差し替えられる。既存の keyboardDriver（csp-runs ほかが共有）は変えない。
//
// 3 つの層で「マウスを使っていない」ことを担保する。
//   1. restrictedKeyboard: 許すキーの集合（ALLOWED_KEYS）の外は押す前に例外にする（fail-closed）。
//   2. guardPage: ドライバが page.click・fill・focus などの要素を指す呼び出しをしたら数える。keyboard は 1 に差し替える。
//   3. installInputCounter: ページの中の信頼されたマウスのイベントと、許さないキーの keydown を数える。context 単位で入れるので、
//      再読み込みやページの移動の後も計数が消えない（累計は Node 側）。
//
// 限界（docs/testing.md 5.507 にも書く）:
//   - 信頼されない click（detail 0）は数えない。glue が Enter・Space で行へ click() を送る正規の経路と見分けられないため。
//     その代わりに 2 の監視と、ドライバが page.evaluate を観察だけに使うこと（要素の click を呼ばない）で補う。
//   - 文字の入力（コマンド・自然文）は許す。BUILD-17 の「操作」は画面を進めるキーを指すと解釈する。改行・タブ・制御文字は type で拒む。
//   - 幅 960px 未満の tablist は矢印キーが要り、許すキーの集合では足りないため扱わない（assertNoTablist で止める）。
// 出力は件数とキーの名前だけにする。入力した値・下書き・DOM の文字は載せない。`Function`・`eval` は使わない。
import assert from 'node:assert/strict';
import {
  DRAFT, NEXT, T, keyTo, rowSel, choiceSel, decideSel, reviseSel,
} from './ch0-3-journey.mjs';

// BUILD-17 の文言どおりに閉じた集合。Shift+Tab・矢印・Home・End・Backspace・Ctrl/Meta/Alt つきは含めない。
export const ALLOWED_KEYS = Object.freeze(['Tab', 'Enter', 'Space', 'Shift+Enter', 'Escape']);

const CTRL_CHARS = /[\u0000-\u001f\u007f]/;

// 許すキーだけを押せる keyboard。押したキーを回数で used に積む。
export function restrictedKeyboard(page) {
  const used = Object.fromEntries(ALLOWED_KEYS.map((k) => [k, 0]));
  return {
    restricted: true,
    used,
    async press(key) {
      if (!ALLOWED_KEYS.includes(key)) throw new Error(`許さないキー: ${String(key).slice(0, 40)}`);
      used[key] += 1;
      await page.keyboard.press(key);
    },
    async type(text) {
      if (typeof text !== 'string' || CTRL_CHARS.test(text)) throw new Error('type に改行・タブ・制御文字は渡せない（Shift+Enter で打つ）');
      await page.keyboard.type(text);
    },
  };
}

// ページの中の計数。信頼されたマウス系のイベントと、keydown の分類を、読み込みのたびに付ける capture の listener から Node 側へ報告する。
// 戻りの read() は件数とキーの名前だけを返す（入力した値は報告に含めない）。
export async function installInputCounter(context) {
  const state = { mouse: 0, keys: {}, chars: 0, forbidden: [] };
  await context.exposeBinding('__fandheInputRec', (_src, kind, name) => {
    if (kind === 'mouse') state.mouse += 1;
    else if (kind === 'char') state.chars += 1;
    else if (kind === 'key') state.keys[String(name)] = (state.keys[String(name)] ?? 0) + 1;
    else if (kind === 'forbidden') state.forbidden.push(String(name).slice(0, 40));
  });
  await context.addInitScript(() => {
    const rec = (kind, name) => window.__fandheInputRec(kind, name);
    for (const t of ['pointerdown', 'mousedown', 'mouseup', 'dblclick', 'auxclick', 'contextmenu', 'wheel']) {
      window.addEventListener(t, (e) => {
        if (e.isTrusted) rec('mouse', t);
      }, true);
    }
    // Enter・Space によるボタンの click も信頼されたイベントになる。マウスの click は detail が 1 以上、キーでは 0 である。
    window.addEventListener('click', (e) => {
      if (e.isTrusted && e.detail > 0) rec('mouse', 'click');
    }, true);
    window.addEventListener('keydown', (e) => {
      if (!e.isTrusted) return;
      const mods = e.ctrlKey || e.metaKey || e.altKey;
      if (e.key === 'Shift' && !mods) return;
      if (mods) rec('forbidden', `${e.ctrlKey ? 'Ctrl+' : ''}${e.metaKey ? 'Meta+' : ''}${e.altKey ? 'Alt+' : ''}${e.key}`);
      else if (e.key === 'Tab' && !e.shiftKey) rec('key', 'Tab');
      else if (e.key === 'Enter') rec('key', e.shiftKey ? 'Shift+Enter' : 'Enter');
      else if (e.key === ' ' && !e.shiftKey) rec('key', 'Space');
      else if (e.key === 'Escape' && !e.shiftKey) rec('key', 'Escape');
      else if (e.key.length === 1) rec('char');
      else rec('forbidden', `${e.shiftKey ? 'Shift+' : ''}${e.key}`);
    }, true);
  });
  return {
    state,
    // 直前の報告が Node 側に届いてから読む（報告は非同期）。
    async read(page) {
      await page.evaluate(() => new Promise((r) => setTimeout(r, 0)));
      return { mouse: state.mouse, keys: { ...state.keys }, chars: state.chars, forbidden: [...state.forbidden] };
    },
  };
}

// 要素を指して操作する呼び出し（click・fill・focus ほか）。キーボードだけの通しでは 1 回も呼ばない。
const POINTING = new Set([
  'click', 'dblclick', 'tap', 'fill', 'focus', 'hover', 'check', 'uncheck', 'setChecked', 'selectOption',
  'setInputFiles', 'dispatchEvent', 'dragAndDrop', 'press', 'type',
]);
const LOCATORS = new Set(['locator', 'first', 'last', 'nth', 'filter', 'getByRole', 'getByText', 'getByLabel', 'getByTestId',
  'getByPlaceholder', 'getByAltText', 'getByTitle', 'frameLocator']);

// page を包み、要素を指す呼び出しを数える。keyboard は restricted に差し替える。観察（evaluate・waitForFunction・count・
// getAttribute・inputValue）はそのまま通す。page.evaluate の中で要素を click することは監視できない（ドライバは観察だけに使う）。
export function guardPage(page, restricted = restrictedKeyboard(page)) {
  const log = { calls: 0, names: {} };
  const bump = (name) => {
    log.calls += 1;
    log.names[name] = (log.names[name] ?? 0) + 1;
  };
  const wrapDevice = (dev, prefix) => new Proxy(dev, {
    get(t, p) {
      const v = Reflect.get(t, p, t);
      if (typeof v !== 'function') return v;
      return (...args) => {
        bump(`${prefix}.${String(p)}`);
        return v.apply(t, args);
      };
    },
  });
  const wrap = (obj, isPage) => new Proxy(obj, {
    get(t, p) {
      const v = Reflect.get(t, p, t);
      if (isPage && p === 'keyboard') return restricted;
      if (isPage && (p === 'mouse' || p === 'touchscreen')) return wrapDevice(v, String(p));
      if (typeof v !== 'function') return v;
      return (...args) => {
        if (POINTING.has(String(p))) bump(String(p));
        const r = v.apply(t, args);
        return LOCATORS.has(String(p)) && r && typeof r === 'object' && typeof r.then !== 'function' ? wrap(r, false) : r;
      };
    },
  });
  return { page: wrap(page, true), log, keyboard: restricted };
}

// tablist が見えているときは例外にする。幅 960px 未満の切り替えは矢印キーが要り、許すキーの集合では足りない。
export async function assertNoTablist(page) {
  const visible = await page.evaluate(() => {
    const e = document.querySelector('[data-testid="pane-tabs"]');
    return !!e && getComputedStyle(e).display !== 'none';
  });
  if (visible) throw new Error('tablist が見えている（幅 960px 未満）。許すキーの集合では切り替えられない');
}

// 入力欄にフォーカスを移し、欄が空か全体が選択されていることを確かめる（次の入力で置き換わる状態）。
// 全選択の Ctrl+A は許さないキーなので使わない。どちらでもなければ理由つきで止める（禁止のキーで逃げない）。
async function focusDraft(page) {
  await keyTo(page, DRAFT);
  // 拒否の後の全選択は 0 ミリ秒のタイマーで付く（TASK-284 2/2）ので、短く待つ。待っても満たさなければ止める。
  const ok = await page.waitForFunction((sel) => {
    const el = document.querySelector(sel);
    return !!el && (el.value === '' || (el.selectionStart === 0 && el.selectionEnd === el.value.length));
  }, DRAFT, { timeout: 3000 }).then(() => true, () => false);
  if (!ok) throw new Error('入力欄が空でも全選択でもない。Ctrl+A を使わずには置き換えられない');
}

// キーボードだけのドライバ。journey が呼ぶ口（choose・point・decide・revise・send・sendMultiline・completeAndSend・esc・nextChapter）と
// 同じ名前を持つ。`page` は guardPage の戻りの page（keyboard が restricted）であること。
export function keyOnlyDriver(page) {
  assert.ok(page.keyboard?.restricted === true, 'keyOnlyDriver needs a guarded page (restricted keyboard)');
  const kb = page.keyboard;
  const activate = async (sel, key = 'Enter') => {
    await assertNoTablist(page);
    await keyTo(page, sel);
    await kb.press(key);
  };
  return {
    choose: (id, key) => activate(choiceSel(id), key),
    // 行が無く kind が渡されたときは、そのビューの切り替えボタンへ Tab で移り、Enter で切り替えてから行へ移る。
    async point(path, kind) {
      await assertNoTablist(page);
      if (kind !== undefined && (await page.locator(rowSel(path)).count()) === 0) {
        assert.match(kind, /^[a-z_]{1,32}$/, 'view kind');
        await activate(`[data-testid="view-select-button"][data-payload="${kind}"]`);
      }
      await activate(rowSel(path));
    },
    decide: (d) => activate(decideSel(d)),
    revise: (t) => activate(reviseSel(t)),
    async send(text) {
      await assertNoTablist(page);
      await focusDraft(page);
      await kb.type(text);
      await kb.press('Enter');
    },
    async sendMultiline(first, second) {
      await assertNoTablist(page);
      await focusDraft(page);
      await kb.type(first);
      await kb.press('Shift+Enter');
      await kb.type(second);
      await kb.press('Enter');
    },
    async completeAndSend(prefix, completed) {
      await assertNoTablist(page);
      await focusDraft(page);
      await kb.type(prefix);
      await kb.press('Tab');
      await page.waitForFunction(([sel, v]) => document.querySelector(sel)?.value === v, [DRAFT, completed], { timeout: T });
      await kb.press('Enter');
    },
    async esc() {
      await assertNoTablist(page);
      await keyTo(page, DRAFT);
      await kb.press('Escape');
    },
    async nextChapter() {
      await activate(NEXT);
    },
  };
}

// 教材テスト（fandhe-pack test）の 1 操作（play_script の記法を JSON にしたもの）を、ドライバの操作に写す。2/2 への引き継ぎ口。
// cmd・text → send（改行は Shift+Enter で区切る。YAML の `|` が付ける末尾の改行 1 つは除いてから分ける。
// 教材テスト側は値をそのまま core へ渡すが、判定は text を trim し cmd を空白で分けるので、末尾の改行は結果を変えない）、choice → choose、ui（"<ビュー ID> <path>"）→ point、approval（`approval: approve|reject|revise` と兄弟の `target`）→ decide か revise（target つき）、
// cancel → esc、tick → キーを押さない（画面が進行を自動で進める。観察は呼び出し側）。期待（expect）の判定と YAML の読み込みは含めない。
// viewKind は、ビュー ID から種類への写し（IR の views から呼び出し側が作る）。知らない鍵・2 つの鍵は例外にする。
export async function playStep(driver, step, { viewKind = () => undefined } = {}) {
  const keys = Object.keys(step).filter((k) => k !== 'expect' && k !== 'target');
  assert.equal(keys.length, 1, `one operation key expected, got ${keys.length}`);
  const [k] = keys;
  const v = step[k];
  switch (k) {
    case 'cmd':
    case 'text': {
      const lines = String(v).replace(/\n$/, '').split('\n');
      if (lines.length === 1) return driver.send(lines[0]);
      assert.equal(lines.length, 2, 'only two-line input is supported');
      return driver.sendMultiline(lines[0], lines[1]);
    }
    case 'choice':
      return driver.choose(String(v));
    case 'ui': {
      const [view, ...rest] = String(v).split(' ');
      return driver.point(rest.join(' '), viewKind(view));
    }
    case 'approval': {
      assert.ok(['approve', 'reject', 'revise'].includes(v), 'approval must be approve, reject or revise');
      if (v === 'revise' && step.target !== undefined) return driver.revise(String(step.target));
      assert.equal(step.target, undefined, 'target is only for revise');
      return driver.decide(v);
    }
    case 'cancel':
      return driver.esc();
    case 'tick':
      return undefined;
    default:
      throw new Error(`unknown play step key: ${k.slice(0, 40)}`);
  }
}
