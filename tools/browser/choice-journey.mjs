// 入力欄のない教材（選択肢と画面操作だけで進む教材）を開始から完走まで自動操作で進め、画面の値を測る道具（TASK-748 1/2。SECOND-4・SECOND-7）。
//
// 役割: 教材の IR を引数で受け、配信する設定の画面（試験用の口なし）を実クリックだけで進め、各時点の画面の値
// （`data-theme`・文字の大きさ・行間・行の数・ビューの切り替え・入力の領域・右の領域のボタン）と、外部通信・ページ内エラーの件数を返す。
// 主張（合否）は持たない。測った値を返すだけにし、期待値は呼び出し側が操作の列・期待値のデータとして渡す。
// 呼び出し元: choice-journey.test.mjs。2/2（#1074）が設定ファイルの読み込みと CI のジョブからの呼び出しを足す予定で、
// そのとき本モジュールの関数は変えずに済むよう、教材固有の値は一切書かず、すべて引数で受ける。
// 使う部品: harness.mjs（配信・計数）、ch0-3-journey.mjs（台帳でない部分のセレクタと install）、ch4-9-journey.mjs の inspectDom49（DOM だけの判定）。
// 限界: 製品の画面はまだ `data-theme` を付けない（docs/operation-profile.md）ので、theme は null の値が返りうる。
// `world_list: hidden` の教材は指せる行が 0 件になり、「指す」操作の Step で止まる（docs/known-gaps.md の PACK-22）。止まった位置は結果に入れる。
// 出力（summaryLine）は名前・件数・合否・数値だけで、教材の文面と入力した値は含めない。
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import {
  openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages,
} from './harness.mjs';
import { NEXT, choiceSel, rowSel, install, T } from './ch0-3-journey.mjs';
import { inspectDom49 } from './ch4-9-journey.mjs';

/** 入力の口として「無い」ことを数える 6 種（input-field-hidden.test.mjs の ABSENT と同じ対象）。`textarea`・`select` は tag 名、残りは data-testid で数える。 */
export const ABSENT = ['textarea', 'draft-input', 'submit-input', 'candidate-list', 'candidate-btn', 'select'];

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
export function measureScreen(page) {
  return page.evaluate((absent) => {
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
    return {
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
      inlineStyle: { attrs: q('[style]').filter((e) => (e.getAttribute('style') ?? '').trim() !== '').length, elements: q('style').length },
    };
  }, ABSENT);
}

/** 復習の行（Step・達成）を読む。 */
const readReview = (page) => page.evaluate(() => [...document.querySelectorAll('[data-testid="part-review-list"] > li')]
  .map((li) => ({ step: li.dataset.step, achieved: li.dataset.achieved })));

/**
 * 新しいコンテキストで配信物を開き、操作の列を実クリックで送り、画面の値と安全の計数を返す。
 *   srv: withServer が渡す { url, origin }
 *   sequence: [{ kind: 'choice', id } | { kind: 'row', path } | { kind: 'next' }, ...]
 *   expect:   各操作の後に DOM が示すべき [{ uid, status }]（sequence と同じ長さ）。initial は開いた直後の { uid, status }
 * 待てない・押す対象が無いときは例外にせず、止まった位置を stoppedAt に入れて終える。completed は、
 * 最後まで到達し、finals の節点が復習で達成になったときだけ真。
 */
export async function runJourney({ browser, srv, ir, sequence, expect, initial, titles, finals, clickTimeout = 4000 }) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
  const obs = domObserver(titles);
  const result = { reached: [], stoppedAt: null, completed: false };
  try {
    await page.goto(srv.url);
    await waitMounted(page);
    await install(page);
    await obs.at(page, initial.uid, initial.status);
    result.reached.push(`${initial.uid}:${initial.status}`);
    result.start = await measureScreen(page);
    for (let i = 0; i < sequence.length; i += 1) {
      const op = sequence[i];
      const sel = op.kind === 'choice' ? choiceSel(op.id) : op.kind === 'row' ? rowSel(op.path) : NEXT;
      try {
        await page.locator(sel).first().click({ timeout: clickTimeout });
      } catch {
        result.stoppedAt = { index: i, kind: op.kind, cause: 'no-target', after: result.reached[result.reached.length - 1] };
        break;
      }
      try {
        await obs.at(page, expect[i].uid, expect[i].status);
      } catch {
        result.stoppedAt = { index: i, kind: op.kind, cause: 'no-state', after: result.reached[result.reached.length - 1] };
        break;
      }
      result.reached.push(`${expect[i].uid}:${expect[i].status}`);
    }
    result.end = await measureScreen(page);
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
    `rows=${r.end?.rows?.count ?? 'n/a'}`,
    `view-buttons=${r.end?.viewSwitch?.visibleButtons ?? 'n/a'}`,
    `external=${r.external}`,
    `page-errors=${r.pageErrors}`,
  ].join(' ');
}
