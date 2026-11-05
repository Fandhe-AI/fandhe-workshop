// 入力欄のない教材（選択肢と画面操作だけで進む教材）を開始から完走まで自動操作で進め、画面の値を測る道具（TASK-748 1/2・2/2。SECOND-4・SECOND-7）。
//
// 役割: 教材の IR を引数で受け、配信する設定の画面（試験用の口なし）を実クリックだけで進め、各時点の画面の値
// （`data-theme`・文字の大きさ・行間・行の数・ビューの切り替え・入力の領域・入口の部品・案内・ファイル選択・右の領域の操作要素）と、外部通信・ページ内エラーの件数を返す。
// 主張（合否）は持たない。測った値を返すだけにし、期待値は呼び出し側が操作の列・期待値のデータとして渡す。
// 呼び出し元: choice-journey-config.mjs（設定ファイルの読み込み・期待との照合。2/2・#1074）と、それを使う
// choice-journey-run.mjs（CLI。CI の browser-test ジョブから scripts/check-choice-journey.sh 経由で呼ばれる）・choice-journey.test.mjs。
// 教材固有の値は一切書かず、すべて引数で受ける（設定ファイルを差し替えるだけで別の教材にも使える）。
// 使う部品: harness.mjs（配信・計数）、ch0-3-journey.mjs（台帳でない部分のセレクタと install）、ch4-9-journey.mjs の inspectDom49（DOM だけの判定）。
// 限界: 製品の画面はまだ `data-theme` を付けない（docs/operation-profile.md）ので、theme は null の値が返りうる。外観（data-theme・本文の px・行間・中身のある style の 0 件）と幅 800px の「成果物」タブは TASK-749 3/3（docs/testing.md 5.449）。
// 幅 960px 未満は tablist が出るので、操作の種類に合わせて実クリックでタブを切り替え、開始・終了は artifactsTab にも測る。
// 左の領域（見えるビュー・行・変化の印）は measureLeft が測り、結果の left に入れる（TASK-749 2/3）。
// 世界状態の一覧を出さない設定（`world_list` の hidden）の教材は指せる行が 0 件になり、「指す」操作の Step で止まる（docs/known-gaps.md の PACK-22）。止まった位置は結果に入れる。
// 拒否を期待する操作（status が rejected）は、実クリックのあと右のログの拒否の行（分類・理由・次の行動）を読み、IR の文面（refIr）と比べ、前後で左の行と Step が動かないことの観察を rejections に積む（TASK-750。SECOND-12。合否は config 側の judgeRejections）。
// 出力（summaryLine）は名前・件数・合否・数値だけで、教材の文面と入力した値は含めない。
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import {
  openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages,
} from './harness.mjs';
import { NEXT, choiceSel, rowSel, decideSel, install, T } from './ch0-3-journey.mjs';
import { inspectDom49 } from './ch4-9-journey.mjs';

/** 入力の口として「無い」ことを数える 6 種（input-field-hidden.test.mjs の ABSENT と同じ対象）。`textarea`・`select` は tag 名、残りは data-testid で数える。 */
export const ABSENT = ['textarea', 'draft-input', 'submit-input', 'candidate-list', 'candidate-btn', 'select'];

/** 操作の入口になりうる部品 7 種（crates/web/src/terminal_area.rs の InputPort と同じ対象）。data-testid で数える。 */
export const PORTS = ['draft-input', 'submit-input', 'stop-btn', 'candidate-list', 'candidate-btn', 'choice-btn', 'approval-btn'];

/** 入力欄がないと描かれない案内 4 種（crates/web/src/hello.rs）。件数と見えるかを測る。 */
export const GUIDES = ['key-hint', 'op-guide', 'os-field', 'os-select'];

/** 右の領域・上部の操作要素。設定は名前で選び、ページ内で測る testid はここで固定する（設定の文字列から selector を組み立てない）。 */
export const CONTROLS = {
  stop: 'stop-btn',
  checkpointSave: 'checkpoint-save',
  checkpointRestore: 'checkpoint-restore',
  export: 'shell-export',
  import: 'shell-import',
  delete: 'shell-delete',
  hintContainer: 'shell-hint',
  hintOpen: 'hint-open',
};

/** 仕様（PoC）の左の領域の名前。製品の DOM には無いので、「0 件」であることを data-testid で数える（TASK-749 2/3。SECOND-8）。 */
export const LEFT_ABSENT = ['world-list', 'view-tabs', 'view-tab'];

/** 変化の印の 3 手段の対応（crates/web/src/a11y.rs の ChangeMark の写し。エンジン共通の値で、教材固有ではない）。キーは data-mark の値。 */
export const MARK_TABLE = {
  '': ['', ''],
  add: ['+', '【追加】'],
  remove: ['-', '【削除】'],
  change: ['~', '【変更】'],
};

/**
 * 領域の見え方とタブの testid・id（crates/web/src/a11y.rs・shell.rs の定数の写し。エンジン共通で教材固有ではない）。
 * 幅 960px 未満では tablist が出て、選んでいない側の領域は `display: none` になる（`hidden` 属性ではない）。設定の文字列から selector は組み立てない。
 */
export const PANES = {
  tablist: 'pane-tabs', ops: 'pane-tab-ops', artifacts: 'pane-tab-artifacts', left: 'shell-left', right: 'shell-right',
};

/** ページ内で領域の見え方を測る（page.evaluate に渡す固定の関数）。 */
function panesInPage(P) {
  const vis = (e) => {
    if (!e) return false;
    const b = e.getBoundingClientRect();
    const c = getComputedStyle(e);
    return b.width > 0 && b.height > 0 && c.display !== 'none' && c.visibility !== 'hidden';
  };
  const byId = (id) => document.querySelector(`[data-testid="${id}"]`);
  const tl = byId(P.tablist);
  return { tablist: vis(tl), selected: tl?.getAttribute('data-pane') ?? null, left: vis(byId(P.left)), right: vis(byId(P.right)) };
}

/** 領域の見え方。判定はしない（TASK-749 3/3。SECOND-9）。 */
export const measurePanes = (page) => page.evaluate(panesInPage, PANES);

/**
 * tablist が見えるとき（幅 960px 未満）だけ、該当のタブを実クリックして data-pane が切り替わるのを待つ。見えないとき（960px 以上）は何もせず真。
 * 失敗しても例外にせず偽を返す（呼び出し側が stoppedAt の no-tab にする）。
 */
export async function ensurePane(page, which, timeout = 4000) {
  const p = await measurePanes(page);
  if (!p.tablist || p.selected === which) return true;
  try {
    await page.locator(`#${PANES[which]}`).click({ timeout });
    await page.waitForFunction(([t, w]) => document.querySelector(`[data-testid="${t}"]`)?.getAttribute('data-pane') === w, [PANES.tablist, which], { timeout });
    return true;
  } catch {
    return false;
  }
}

/** 入力の注意書き（常に 1 個あるが、普段は空の文字）。 */
const NOTICE = 'input-notice';

/** IR の節点から、Step の表と章の区切りを導く。教材ごとの値は書かない。 */
export function stepsOf(ir) {
  const steps = ir.nodes.map((n) => [n.uid, n.step.title]);
  const order = steps.map(([uid]) => uid);
  const titles = Object.fromEntries(steps);
  const units = [...new Set(ir.nodes.map((n) => n.unit))];
  const firstOf = Object.fromEntries(units.map((u) => [u, ir.nodes.find((n) => n.unit === u).uid]));
  const lastOf = Object.fromEntries(units.map((u) => [u, [...ir.nodes].reverse().find((n) => n.unit === u).uid]));
  const finals = ir.nodes.filter((n) => n.step.final === true).map((n) => n.uid);
  return { steps, order, titles, units, firstOf, lastOf, finals };
}

/** DOM だけの観察（`__fandheTest` が無いので世代の確認は置かない）。題の表を渡して作る。 */
export function domObserver(titles) {
  return {
    async at(page, uid, status, timeout = T) {
      await page.waitForFunction(inspectDom49, [titles, uid, status, true], { timeout })
        .catch((e) => { throw new Error(`DOM が ${uid} の ${status} を示さない: ${String(e.message).slice(0, 200)}`); });
    },
    async stillAt(page, uid) {
      const { d } = await page.evaluate(inspectDom49, [titles, uid, 'entered']);
      return d.title === titles[uid] && d.lastStep === uid && (d.lastKind === 'rejected' || d.lastKind === 'clarified');
    },
    hasState: false,
  };
}

/**
 * IR を同梱した配信する設定（webDir）を一時ディレクトリへ写し、ir/main.json を irText に置き換える。
 * index.html の fandhe-ir の meta は IR の中身に依らない（tools/build-web.sh）ので、wasm を作り直さずに別の教材の配信物になる。
 * 戻りは { dir, cleanup }。試験用の口（meta・test-hooks.js）が残っていれば例外にする（fail-closed）。
 */
export function prepareDelivery({ webDir, irText }) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-choice-web-'));
  const cleanup = () => fs.rmSync(dir, { recursive: true, force: true });
  try {
    fs.cpSync(webDir, dir, { recursive: true });
    const html = fs.readFileSync(path.join(dir, 'index.html'), 'utf8');
    const metas = html.match(/<meta\s+name="fandhe-ir"\s+content="1">/g) ?? [];
    if (metas.length !== 1) throw new Error(`fandhe-ir meta must be exactly 1, got ${metas.length}`);
    if (/fandhe-test-hooks/.test(html) || fs.existsSync(path.join(dir, 'test-hooks.js'))) throw new Error('delivery copy must have no test hooks');
    fs.mkdirSync(path.join(dir, 'ir'), { recursive: true });
    fs.writeFileSync(path.join(dir, 'ir', 'main.json'), irText, 'utf8');
    return { dir, cleanup };
  } catch (e) {
    cleanup();
    throw e;
  }
}

/**
 * 教材パックを一時ディレクトリへ写し、edit(work) で書き換えてから compile し、IR の文字列を返す。
 * edit を省くと書き換えなし。一時ディレクトリは必ず消す。シェルを通さず配列で起動する。
 */
export function compilePack({ bin, packDir, edit = null }) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-choice-pack-'));
  try {
    fs.cpSync(packDir, work, { recursive: true });
    if (edit) edit(work);
    const r = spawnSync(bin, ['compile', work], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    if (r.status !== 0) throw new Error(`compile exit ${r.status}`);
    return r.stdout;
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

/** ページ内で画面の値を測る。判定はしない。ページ内の関数は固定で、教材の文字列から組み立てない。 */
export async function measureScreen(page) {
  const ids = [...new Set([...PORTS, ...GUIDES, ...Object.values(CONTROLS)])];
  const m = await page.evaluate(([absent, ids, noticeId]) => {
    const vis = (e) => {
      const b = e.getBoundingClientRect();
      const c = getComputedStyle(e);
      return b.width > 0 && b.height > 0 && c.display !== 'none' && c.visibility !== 'hidden' && !e.closest('[hidden]');
    };
    const q = (sel) => [...document.querySelectorAll(sel)];
    const tid = (id) => `[data-testid="${id}"]`;
    const fontOf = (sel) => {
      const e = document.querySelector(sel);
      if (!e) return null;
      const c = getComputedStyle(e);
      const px = parseFloat(c.fontSize);
      const lh = c.lineHeight === 'normal' ? 1.2 * px : parseFloat(c.lineHeight);
      return { px, lineHeightRatio: Math.round((lh / px) * 1000) / 1000 };
    };
    const root = document.querySelector('#app');
    const withTheme = root ? [...root.querySelectorAll('[data-theme]'), ...(root.hasAttribute('data-theme') ? [root] : [])] : [];
    const main = document.querySelector('main');
    const left = document.querySelector(tid('shell-left'));
    const rowEls = left ? [...left.querySelectorAll(`${tid('part-artifact-view')} [data-row]`)].filter(vis) : [];
    const sels = q('[data-testid="view-select"]');
    const btns = q('[data-testid="view-select-button"]');
    const desc = (b) => [b.dataset.testid ?? '', b.dataset.decision ?? '', b.dataset.choice ?? ''].join('|');
    const rightBtns = [...document.querySelectorAll(`${tid('shell-right')} button, ${tid('shell-top')} button`)].filter(vis);
    const parts = Object.fromEntries(ids.map((id) => {
      const els = q(tid(id));
      return [id, { count: els.length, visible: els.filter(vis).length }];
    }));
    const notices = q(tid(noticeId));
    const files = q('input').filter((e) => vis(e) && e.getAttribute('type') === 'file');
    return {
      parts,
      notice: { count: notices.length, visible: notices.filter(vis).length, textLength: notices.reduce((n, e) => n + (e.textContent ?? '').trim().length, 0) },
      fileInputs: { visible: files.length, inImport: files.filter((e) => e.closest(tid('shell-import'))).length },
      theme: { values: withTheme.map((e) => e.getAttribute('data-theme')), main: main?.getAttribute('data-theme') ?? null },
      font: { body: fontOf(tid('step-body')), do: fontOf(tid('step-do')), choice: fontOf(tid('choice-btn')) },
      rows: { count: rowEls.length, paths: rowEls.map((e) => e.getAttribute('data-row')) },
      viewSwitch: { select: sels.length, buttons: btns.length, visibleButtons: btns.filter(vis).length },
      inputArea: {
        absent: Object.fromEntries(absent.map((id) => [id, q(id === 'textarea' || id === 'select' ? id : tid(id)).length])),
        visibleInputs: q('input, textarea').filter(vis).map((e) => `${e.dataset.testid ?? ''}|${e.tagName.toLowerCase()}|${e.getAttribute('type') ?? ''}`),
        stop: { count: q(tid('stop-btn')).length, visible: q(tid('stop-btn')).some(vis) },
      },
      rightButtons: rightBtns.map(desc),
      // 中身のある style 属性と <style> 要素の数（空の style 属性・空の <style> は数えない）。elementsTotal は観察用。
      inlineStyle: {
        attrs: q('[style]').filter((e) => (e.getAttribute('style') ?? '').trim() !== '').length,
        elements: q('style').filter((e) => (e.textContent ?? '').trim() !== '').length,
        elementsTotal: q('style').length,
      },
    };
  }, [ABSENT, ids, NOTICE]);
  return { ...m, panes: await measurePanes(page) };
}

/**
 * 左の領域（成果物のビュー）を測る。判定はしない（TASK-749 2/3。SECOND-6・SECOND-8）。ページ内の関数は固定で、設定の文字列から組み立てない。
 * 見えるビューは `hidden` の無い `view-list` の data-view（空の ul は箱が 0 なので箱の大きさでは測らない）。行は `li[data-path]`。
 * 行には変化の印の文字（markText）と、印・本文を除いた行の文字（text）・行の種類（kind）も持つ（Issue #2052）。
 * 描画がログと揃うまで待ち、揃わなければ settled を偽にして返す（例外にしない）。
 */
export async function measureLeft(page, timeout = 2000) {
  const settled = await page.waitForFunction(() => {
    const t = (id) => document.querySelector(`[data-testid="${id}"]`);
    const a = t('part-artifact-consumed');
    const l = t('part-log-consumed');
    return a !== null && l !== null && (a.textContent ?? '').trim() === (l.textContent ?? '').trim();
  }, null, { timeout }).then(() => true, () => false);
  const m = await page.evaluate((absent) => {
    const q = (sel, root = document) => [...root.querySelectorAll(sel)];
    const tid = (id) => `[data-testid="${id}"]`;
    const vis = (e) => {
      const b = e.getBoundingClientRect();
      const c = getComputedStyle(e);
      return b.width > 0 && b.height > 0 && c.display !== 'none' && c.visibility !== 'hidden' && !e.closest('[hidden]');
    };
    const left = document.querySelector(tid('shell-left'));
    const lists = left ? q(tid('view-list'), left) : [];
    const shown = lists.filter((e) => !e.closest('[hidden]')).map((e) => e.getAttribute('data-view'));
    const rowsOf = lists.filter((e) => !e.closest('[hidden]')).flatMap((e) => q(':scope > li[data-path]', e));
    const sels = q(tid('view-select'));
    const btns = q(tid('view-select-button'));
    return {
      views: { shown, total: lists.length },
      select: { count: sels.length, visible: sels.filter(vis).length, buttons: btns.length, visibleButtons: btns.filter(vis).length },
      absent: Object.fromEntries(absent.map((id) => [id, q(tid(id)).length])),
      rows: rowsOf.map((li) => ({
        path: li.getAttribute('data-path'),
        mark: li.getAttribute('data-mark') ?? '',
        glyph: (li.querySelector(':scope > ' + tid('mark-glyph'))?.textContent ?? '').trim(),
        markText: (li.querySelector(':scope > ' + tid('mark-text'))?.textContent ?? '').trim(),
        // 行の文字（Issue #2052・TASK-799・D-100）。変化の印と本文の区画を除き、空白を詰める。
        // 文書のビューの行は data-kind="item" を持ち、文字は題・状態・補足の連結になる（Issue #2053）。他の種類の行は kind が空文字のことがある。
        text: (() => {
          const c = li.cloneNode(true);
          for (const e of q([tid('mark-glyph'), tid('mark-text'), tid('artifact-doc-body')].join(','), c)) e.remove();
          return (c.textContent ?? '').replace(/\s+/g, ' ').trim();
        })(),
        kind: li.getAttribute('data-kind') ?? '',
        // 行の中の本文の区画の件数（G-4・ART-18）。中身は読まない（件数だけ。TASK-755）。
        docBody: q(tid('artifact-doc-body'), li).length,
      })),
      // 左の領域全体の本文の区画の件数。行の外にある区画も見落とさない。
      docBodyTotal: left ? q(tid('artifact-doc-body'), left).length : 0,
    };
  }, LEFT_ABSENT);
  return { ...m, settled, panes: await measurePanes(page) };
}

/**
 * 左の領域の成果物ビューの行の写し（拒否の前後の比較用）。行ごとの HTML を文字列で持つ。
 * 変化の印（data-mark・mark-glyph・mark-text）は「直前の操作からの差」を示し、何も変えない操作のあとには消えるので、比べる対象から外す。
 * 行の中身（パス・文字・そのほかの属性）は外さない。
 */
const snapshotRows = (page) => page.evaluate(() => {
  const view = document.querySelector('[data-testid="shell-left"] [data-testid="part-artifact-view"]');
  if (!view) return null;
  return [...view.querySelectorAll('[data-row]')].map((e) => {
    const c = e.cloneNode(true);
    c.removeAttribute('data-mark');
    c.querySelectorAll('[data-testid="mark-glyph"], [data-testid="mark-text"]').forEach((m) => m.remove());
    return c.outerHTML;
  }).join('\n');
});

/** Step の題と状態の語（拒否で動かないことの比較用）。 */
const readStepHead = (page) => page.evaluate(() => ({
  title: document.querySelector('[data-testid="step-title"]')?.textContent ?? '',
  word: document.querySelector('[data-testid="step-status"]')?.textContent ?? '',
}));

const WAITING = '受付済み（確認待ち）';
const REJECTED_ROWS = '[data-testid="part-log-list"] li[data-kind="rejected"]';

/** 右のログの拒否の行の件数。 */
const countRejected = (page) => page.locator(REJECTED_ROWS).count();

/**
 * 拒否の行が before + 1 件になり、処理が idle に戻るのを待ち、最後の拒否の行を読む。待てなければ null。
 * 上部の「応答」（live-response）は製品の画面にまだ描かれない（docs/known-gaps.md）ので、件数と文面の有無を観察として返すだけにする。
 */
async function readRejection(page, before, timeout) {
  try {
    await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length >= n
      && document.querySelector('[data-testid="run-status"]')?.getAttribute('data-state') === 'idle', [REJECTED_ROWS, before + 1], { timeout });
  } catch {
    return null;
  }
  return page.evaluate((sel) => {
    const rows = [...document.querySelectorAll(sel)];
    const last = rows[rows.length - 1];
    const text = (id) => last.querySelector(`[data-testid="${id}"]`)?.textContent ?? null;
    const live = [...document.querySelectorAll('[data-testid="live-response"]')];
    return {
      step: last.getAttribute('data-step'),
      category: last.getAttribute('data-category'),
      reason: text('line-reason'),
      next: text('line-next-action'),
      live: { count: live.length, text: live.map((e) => e.textContent ?? '').join('\n') },
    };
  }, REJECTED_ROWS);
}

/** 拒否の 1 操作の観察。IR の拒否の分類と文面は refIr から引く。出なければ null。 */
async function observeRejection(page, { op, uid, refIr, before, titles, timeout }) {
  const got = await readRejection(page, before.count, Math.max(timeout, T));
  if (!got || got.step !== uid) return null;
  const ref = refIr.rejections?.[op.rejection] ?? null;
  const after = await snapshotRows(page);
  const head = await readStepHead(page);
  return {
    category: got.category,
    irCategory: ref?.category ?? null,
    reasonOk: ref !== null && got.reason !== null && got.reason !== '' && got.reason === ref.message,
    nextOk: ref !== null && got.next !== null && got.next !== '' && got.next === ref.next_action,
    rowsUnchanged: before.rows !== null && after === before.rows,
    // 状態の語は、確認待ち（受付済み）かどうかだけを比べる（Step に入った直後は前の Step の語が残り、最初の応答の行で消える表示のため。docs/testing.md 5.446）。
    stepUnchanged: head.title === before.head.title && head.title === titles[uid] && (head.word === WAITING) === (before.head.word === WAITING),
    // 観察のみ（合否に使わない）。
    liveResponse: { count: got.live.count, hasText: ref !== null && ref.message !== '' && got.live.text.includes(ref.message) },
  };
}

/** 復習の行（Step・達成）を読む。 */
const readReview = (page) => page.evaluate(() => [...document.querySelectorAll('[data-testid="part-review-list"] > li')]
  .map((li) => ({ step: li.dataset.step, achieved: li.dataset.achieved })));

/**
 * 新しいコンテキストで配信物を開き、操作の列を実クリックで送り、画面の値と安全の計数を返す。
 *   srv: withServer が渡す { url, origin }
 *   sequence: [{ kind: 'choice', id } | { kind: 'row', path } | { kind: 'decide', decision } | { kind: 'next' }, ...]
 *   expect:   各操作の後に DOM が示すべき [{ uid, status }]（sequence と同じ長さ）。initial は開いた直後の { uid, status }
 * 待てない・押す対象が無いときは例外にせず、止まった位置を stoppedAt に入れて終える。completed は、
 * 最後まで到達し、finals の節点が復習で達成になったときだけ真。
 */
export async function runJourney({
  browser, srv, ir, refIr = ir, sequence, expect, initial, titles, finals, clickTimeout = 4000, viewport = { width: 1280, height: 800 },
}) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport });
  const obs = domObserver(titles);
  const result = { reached: [], stoppedAt: null, completed: false, rejections: [] };
  try {
    await page.goto(srv.url);
    await waitMounted(page);
    await install(page);
    await obs.at(page, initial.uid, initial.status);
    result.reached.push(`${initial.uid}:${initial.status}`);
    result.start = await measureScreen(page);
    // 幅 960px 未満（tablist が見える）では、左の領域は「成果物」タブでしか見えない。開始・終了は両方のタブで測り、
    // 操作は種類に合わせて実クリックでタブを切り替える（row は成果物、それ以外は操作。TASK-749 3/3。SECOND-9）。
    const tabbed = result.start.panes.tablist;
    result.artifactsTab = tabbed ? { start: null, end: null } : null;
    const toTab = async (which, i, kind) => {
      if (await ensurePane(page, which, clickTimeout)) return true;
      result.stoppedAt = { index: i, kind, cause: 'no-tab', after: result.reached[result.reached.length - 1] };
      return false;
    };
    result.left = { start: null, ops: [], end: null };
    const leftNow = async (i, kind) => ((await toTab('artifacts', i, kind)) ? measureLeft(page) : null);
    const run = async () => {
      if (tabbed) {
        if (!(await toTab('artifacts', -1, 'start'))) return;
        result.artifactsTab.start = await measureScreen(page);
      }
      result.left.start = await leftNow(-1, 'start');
      if (result.stoppedAt) return;
      for (let i = 0; i < sequence.length; i += 1) {
        const op = sequence[i];
        const sel = {
          choice: () => choiceSel(op.id), row: () => rowSel(op.path), decide: () => decideSel(op.decision), next: () => NEXT,
        }[op.kind]?.();
        if (sel === undefined) throw new Error('unknown operation kind');
        if (!(await toTab(op.kind === 'row' ? 'artifacts' : 'ops', i, op.kind))) return;
        const pre = expect[i].status === 'rejected'
          ? { rows: await snapshotRows(page), head: await readStepHead(page), count: await countRejected(page) }
          : null;
        try {
          await page.locator(sel).first().click({ timeout: clickTimeout });
        } catch {
          result.stoppedAt = { index: i, kind: op.kind, cause: 'no-target', after: result.reached[result.reached.length - 1] };
          return;
        }
        if (expect[i].status === 'rejected') {
          // 拒否の期待の操作は、クリックの前に取った写しと比べる。refIr は期待の文面の出どころ（表示する IR と別に渡せる）。
          const rec = await observeRejection(page, { op, uid: expect[i].uid, refIr, before: pre, titles, timeout: clickTimeout });
          if (!rec) {
            result.stoppedAt = { index: i, kind: op.kind, cause: 'no-rejection', after: result.reached[result.reached.length - 1] };
            return;
          }
          result.rejections.push({ index: i, scene: op.scene, rejection: op.rejection, ...rec });
          // 拒否の操作にも marks を宣言できるので、拒否の後の左の領域も同じ index で測る（judgeLeft が観察の有無を見る）。
          const m = await leftNow(i, op.kind);
          if (!m) return;
          result.left.ops.push({ index: i, ...m });
          continue;
        }
        try {
          await obs.at(page, expect[i].uid, expect[i].status);
        } catch {
          result.stoppedAt = { index: i, kind: op.kind, cause: 'no-state', after: result.reached[result.reached.length - 1] };
          return;
        }
        result.reached.push(`${expect[i].uid}:${expect[i].status}`);
        const m = await leftNow(i, op.kind);
        if (!m) return;
        result.left.ops.push({ index: i, ...m });
      }
    };
    await run();
    // 終了の測定。tablist があれば操作タブで end、成果物タブで artifactsTab.end と left.end を測る（切り替えの失敗は stoppedAt を上書きしない）。
    if (tabbed) await ensurePane(page, 'ops', clickTimeout);
    result.end = await measureScreen(page);
    if (tabbed) {
      const ok = await ensurePane(page, 'artifacts', clickTimeout);
      if (ok) result.artifactsTab.end = await measureScreen(page);
      else result.stoppedAt ??= { index: sequence.length, kind: 'end', cause: 'no-tab', after: result.reached[result.reached.length - 1] };
    }
    result.left.end = await measureLeft(page);
    result.review = await readReview(page);
    result.completed = result.stoppedAt === null
      && finals.every((u) => result.review.some((r) => r.step === u && r.achieved === 'true'));
    const counts = await collectSafety(page, rec);
    result.safety = nonZero(counts);
    result.external = rec.external.length;
    result.pageErrors = rec.pageErrors.length + (rec.crashed ? 1 : 0);
    result.consoleErrors = rec.consoleErrors.length;
    result.cspWarnings = cspConsoleMessages(rec).length;
    result.run = await page.evaluate(() => window.__run);
    result.nextChapter = await page.locator(NEXT).count();
    result.irFetch = rec.requests.filter((r) => new URL(r.url).pathname.endsWith('/ir/main.json')).map((r) => r.status);
    result.stepCount = ir.nodes.length;
    return result;
  } finally {
    await context.close();
  }
}

/** 1 行の要約。値と件数だけで、教材の文面は含めない。 */
export function summaryLine(label, csp, r) {
  const f = r.end?.font ?? {};
  const fmt = (x) => (x ? `${x.px}px/${x.lineHeightRatio}` : 'none');
  return [
    `CHOICE-JOURNEY ${label} csp=${csp ? 'on' : 'off'}`,
    `reached=${r.reached.length}`,
    `completed=${r.completed}`,
    `stopped=${r.stoppedAt ? `${r.stoppedAt.cause}@${r.stoppedAt.after}` : 'none'}`,
    `theme=${JSON.stringify(r.end?.theme?.values ?? [])}`,
    `font=body:${fmt(f.body)},do:${fmt(f.do)},choice:${fmt(f.choice)}`,
    `rows=${(r.artifactsTab?.end ?? r.end)?.rows?.count ?? 'n/a'}`,
    `views=${r.left?.end?.views?.shown?.length ?? 'n/a'}`,
    `left-ops=${r.left?.ops?.length ?? 'n/a'}`,
    `view-buttons=${r.end?.viewSwitch?.visibleButtons ?? 'n/a'}`,
    `ports=${PORTS.filter((id) => (r.end?.parts?.[id]?.visible ?? 0) > 0).join(',') || 'none'}`,
    `file-inputs=${r.end?.fileInputs?.visible ?? 'n/a'}`,
    `inline-style=${r.end?.inlineStyle?.attrs ?? 'n/a'}/${r.end?.inlineStyle?.elements ?? 'n/a'}`,
    `tabs=${r.start?.panes?.tablist ?? 'n/a'}`,
    `main-theme=${r.end?.theme?.main ?? 'none'}`,
    `external=${r.external}`,
    `page-errors=${r.pageErrors}`,
    ...rejectionParts(r),
  ].join(' ');
}

/** 拒否の観察があるときだけ足す要約の欄。合格した拒否の数 / 拒否の数と、観察した分類（固定の語）。文面は含めない。 */
function rejectionParts(r) {
  const rs = r.rejections ?? [];
  if (rs.length === 0) return [];
  const ok = rs.filter((x) => x.category === x.irCategory && x.reasonOk && x.nextOk && x.rowsUnchanged && x.stepUnchanged).length;
  const cats = [...new Set(rs.map((x) => x.category))].sort().join('+');
  return [`rejections=${ok}/${rs.length}`, `categories=${cats}`, `live-response=${rs.filter((x) => x.liveResponse.count > 0).length}/${rs.length}`];
}
