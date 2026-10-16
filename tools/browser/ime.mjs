// CDP による IME の模擬のヘルパ（TASK-86 1/2。BUILD-10）。
//
// 役割: Playwright の信頼入力（page.keyboard）では作れない「変換中」の状態を、Chromium の CDP
// （Input.imeSetComposition・Input.insertText・Input.dispatchKeyEvent）で作る。変換中の Enter・Tab・Esc が
// isComposing=true・keyCode=229 で届くことは、PoC-2 の観察（logs/cdp-ime-probe.txt）に合わせている。
// これはシミュレーターであり、実 IME でも人の評価でもない。実 IME の確認は TASK-93 / BUILD-18 の担当で、ここでは扱わない。
// 呼び出し元: ime.test.mjs（ヘルパの自己確認）・ime-input.test.mjs（入力欄と IME の層 3）。後続の TASK-276 の試験文 30 文の行列も使う。
// CDP は Chromium 系でしか使えない（FANDHE_BROWSER_CHANNEL=chrome でも可。Firefox・WebKit は対象外）。
// 引数は page・CDP セッションと文字列だけで、page.evaluate へ文字列からコードを組み立てない。
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

// 各試験の assert メッセージに添える。証拠の種類を取り違えないための印。
export const SIM_LABEL = 'シミュレーター（CDP による IME の模擬。人の評価ではない）';

const SENTENCES_URL = new URL('../../testdata/input-field-ime/test-data/ime-sentences.json', import.meta.url);

// 試験文 30 文を読む。信頼しない入力として型を検査し、不正なら例外（黙って空にしない）。
export function loadSentences() {
  const rows = JSON.parse(fs.readFileSync(fileURLToPath(SENTENCES_URL), 'utf8'));
  if (!Array.isArray(rows) || rows.length === 0) throw new Error('ime-sentences.json must be a non-empty array');
  for (const r of rows) {
    const ok = r !== null && typeof r === 'object'
      && typeof r.id === 'string' && typeof r.kind === 'string' && typeof r.text === 'string'
      && Array.isArray(r.comps) && r.comps.every((c) => typeof c === 'string')
      && (r.lines === undefined || (Array.isArray(r.lines) && r.lines.every((l) => Array.isArray(l) && l.every((s) => typeof s === 'string'))));
    if (!ok) throw new Error(`invalid sentence row: ${JSON.stringify(r)}`);
  }
  return rows;
}

// CDP セッションを開く。
export function openIme(page) {
  return page.context().newCDPSession(page);
}

// 対象がフォーカスされていなければ例外にする。フォーカスが無いと CDP の合成は黙って何も起こさず、
// 「誤送信 0 件」が空振りになるため、fail-closed にする。
export async function assertFocused(page, selector) {
  const ok = await page.evaluate((sel) => document.activeElement === document.querySelector(sel), selector);
  if (!ok) throw new Error(`IME target is not focused: ${selector}`);
}

// 変換中の文字列を置く（compositionstart / compositionupdate が出る）。選択位置はコードポイント数。
export function imeCompose(cdp, text) {
  const n = [...text].length;
  return cdp.send('Input.imeSetComposition', { text, selectionStart: n, selectionEnd: n });
}

// 確定する（compositionend が出て、文字列が欄に入る）。
export function imeCommit(cdp, text) {
  return cdp.send('Input.insertText', { text });
}

// 変換を取り消す（空の文字列の imeSetComposition）。
export function imeCancel(cdp) {
  return cdp.send('Input.imeSetComposition', { text: '', selectionStart: 0, selectionEnd: 0 });
}

const KEYS = {
  Enter: { code: 'Enter', vk: 13 },
  Tab: { code: 'Tab', vk: 9 },
  Escape: { code: 'Escape', vk: 27 },
};

// キーを送る。composing: true は keyCode 229（IME が処理中のキー）の rawKeyDown、false は実キーコード。
// keyUp は常に実キーコード。
export async function imeKey(cdp, key, { composing }) {
  const k = KEYS[key];
  if (!k) throw new Error(`unsupported IME key: ${key}`);
  const vk = composing ? 229 : k.vk;
  await cdp.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key, code: k.code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk });
  await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code: k.code, windowsVirtualKeyCode: k.vk, nativeVirtualKeyCode: k.vk });
}

export const imeEnterKey = (cdp) => imeKey(cdp, 'Enter', { composing: true });
export const imeTabKey = (cdp) => imeKey(cdp, 'Tab', { composing: true });
export const imeEscKey = (cdp) => imeKey(cdp, 'Escape', { composing: true });

// 試験文の 1 行を、変換の途中の文字列（comps）を順に置いて合成する。最後の comps が確定前の変換中の文字列になる。
// comps が空の行（multi）は呼び出し側が lines を行ごとに使う。
export async function composeSentence(cdp, sentence) {
  if (sentence.comps.length === 0) throw new Error(`sentence has no comps: ${sentence.id}`);
  for (const c of sentence.comps) await imeCompose(cdp, c);
}

// keydown・composition 系の記録器を付ける。window の bubble 段に付けるため、アプリの listener より後に走り、
// defaultPrevented がアプリの判断を反映する。固定の関数だけを渡す。
export async function installKeyProbe(context) {
  await context.addInitScript(() => {
    const log = [];
    window.__fandheKeyProbe = log;
    for (const type of ['keydown', 'compositionstart', 'compositionupdate', 'compositionend']) {
      window.addEventListener(type, (e) => {
        log.push({
          type,
          key: e.key === undefined ? null : e.key,
          keyCode: e.keyCode === undefined ? null : e.keyCode,
          isComposing: Boolean(e.isComposing),
          defaultPrevented: e.defaultPrevented,
          data: typeof e.data === 'string' ? e.data : null,
        });
      });
    }
  });
}

export function readKeyProbe(page) {
  return page.evaluate(() => (window.__fandheKeyProbe ?? []).map((r) => ({ ...r })));
}

export function clearKeyProbe(page) {
  return page.evaluate(() => { if (window.__fandheKeyProbe) window.__fandheKeyProbe.length = 0; });
}
