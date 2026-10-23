// 枠の通し（CH-0〜CH-3 の 20 Step を最後まで）の誘導と Step ごとの観察の補助モジュール（TASK-548 1/2。SHELL-1・SHELL-7・SHELL-8・SHELL-10）。
//
// 役割: shell-journey.test.mjs（1 件目の骨格）が使う。2/2（TASK-548 2/2）は、チェックポイントの保存→復帰・再読み込み・
// 書き出し→別のオリジンでの読み込み・保存の失敗の表示を、本モジュールの `hooks.afterStep(uid, status, ctx)` から差し込む。
// 差し込む場所はここ 1 か所だけにして、Step ごとの観察と通しの誘導（ch0-3-journey.mjs の journey）を書き直さずに足せるようにする。
// `.test.mjs` を import すると試験が二重に登録されるため、試験でない本モジュールに置く。
//
// 期待値は、試験の中で教材の IR（crates/web/tests/data/ch0_3/ir.json）を読んで独自に数える。画面の data-* を読み返して合格にしない。
// 試験の出力に、入力した値・下書き・本文の中身は出さない（名前・件数・合否だけ）。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  journey, mouseDriver, paneAware, engine, LOG,
} from './ch0-3-journey.mjs';

export const IR = JSON.parse(fs.readFileSync(
  path.join(path.dirname(fileURLToPath(import.meta.url)), '../../crates/web/tests/data/ch0_3/ir.json'),
  'utf8',
));
export const STEPS = IR.nodes.filter((n) => n.scope === 'main').length;
export const LAST = 'main/st-3-2';

export const LEFT = '[data-testid="shell-left"]';
export const RIGHT = '[data-testid="shell-right"]';
export const ARTIFACT = '[data-testid="part-artifact-view"]';
export const TITLE = '[data-testid="step-title"]';
export const DO = '[data-testid="step-do"]';
export const POSITION = '[data-testid="shell-position"]';
const TAB_OPS = 'pane-tab-ops';
const TAB_ART = 'pane-tab-artifacts';

// IR から独自に数えた位置の期待値。
export function expectedPosition(uid) {
  const node = IR.nodes.find((n) => n.uid === uid);
  const list = IR.nodes.filter((n) => n.scope === 'main' && n.unit === node.unit);
  return { chapter: node.unit, m: list.findIndex((n) => n.uid === uid) + 1, k: list.length };
}

// 左に出る変化（put・set_attr）を起こす規則を持つ Step か。effects が無い Step には左の変化を要求しない。
export function hasLeftEffect(uid) {
  const node = IR.nodes.find((n) => n.uid === uid);
  const text = JSON.stringify(node.step.rules);
  return text.includes('"put"') || text.includes('"set_attr"');
}

// 左の指紋。左のエリアの行の path と文字の列。差分の印（mark）の属性は表示の状態で揺れるので含めない。
const leftPrint = (page) => page.evaluate(([left]) => [...document.querySelector(left).querySelectorAll('[data-row]')]
  .map((e) => `${e.getAttribute('data-row')}\u0000${e.textContent}`).join('\u0001'), [LEFT]);

const logCount = (page) => page.evaluate((sel) => document.querySelectorAll(`${sel} li`).length, LOG);

// 取り付け点の数。左右の逆側に載っていないことも数える。
const anchorCounts = (page) => page.evaluate(([left, right, art, log, title]) => {
  const n = (root, sel) => document.querySelector(root)?.querySelectorAll(sel).length ?? -1;
  return {
    leftArt: n(left, art), rightLog: n(right, log), rightTitle: n(right, title),
    leftLog: n(left, log), rightArt: n(right, art),
  };
}, [LEFT, RIGHT, ARTIFACT, LOG, TITLE]);

export const anchorsOk = (a) => a.leftArt === 1 && a.rightLog === 1 && a.rightTitle === 1 && a.leftLog === 0 && a.rightArt === 0;

const isVisible = (page, sel) => page.evaluate((s) => {
  const e = document.querySelector(s);
  if (!e) return false;
  const r = e.getBoundingClientRect();
  const c = getComputedStyle(e);
  return r.width > 0 && r.height > 0 && c.display !== 'none' && c.visibility !== 'hidden';
}, sel);

const tabsShown = (page) => page.evaluate(() => {
  const e = document.querySelector('[data-testid="pane-tabs"]');
  return !!e && getComputedStyle(e).display !== 'none';
});

// tablist が見えているときだけ `tabId` を選び、`sel` が見えるかを返して、もとのタブに戻す。見えていないとき（2 列）は選ばない。
export async function visibleViaTab(page, counter, tabId, sel) {
  if (!(await tabsShown(page))) return isVisible(page, sel);
  const prev = await page.evaluate(() => document.querySelector('[role="tab"][aria-selected="true"]')?.id ?? null);
  if (prev !== tabId) {
    await page.click(`#${tabId}`);
    counter.clicks += 1;
  }
  const v = await isVisible(page, sel);
  if (prev && prev !== tabId) {
    await page.click(`#${prev}`);
    counter.clicks += 1;
  }
  return v;
}

// 位置の判定。見えること・tabpanel の外（shell-top の中）・文字が IR から数えた形と一致すること。失敗した項目を返す。
export async function positionFailures(page, uid) {
  const dom = await page.evaluate((sel) => {
    const all = document.querySelectorAll(sel);
    if (all.length !== 1) return { count: all.length };
    const e = all[0];
    return {
      count: 1, text: e.textContent, inPanel: !!e.closest('[role="tabpanel"]'), inTop: !!e.closest('[data-testid="shell-top"]'),
    };
  }, POSITION);
  const failed = [];
  if (dom.count !== 1) return ['位置が 1 個'];
  const e = expectedPosition(uid);
  if (dom.text !== `${e.chapter}　Step ${e.m} / ${e.k}`) failed.push('位置の文字');
  if (dom.inPanel || !dom.inTop) failed.push('位置が shell-top の中・tabpanel の外');
  if (!(await isVisible(page, POSITION))) failed.push('位置が見える');
  return failed;
}

// 通し。幅 `width` の画面で 20 Step を最後まで進め、Step ごとの観察と最後の確認を行う。返すのは記録に使う件数と、幅をまたぐ比較用の値。
export async function runShellJourney(page, { width, scene, L, hooks = {} }) {
  const counter = { clicks: 0 };
  const stat = {
    visited: 0, crossings: 0, left: [0, 0], right: [0, 0], title: [0, 0], position: [0, 0],
  };
  const mark = (pair, ok) => {
    pair[1] += 1;
    if (ok) pair[0] += 1;
  };
  let prev = null;
  let lastChapter = null;
  let lastStep = null;
  let prevArtifacts = null;
  const observeEntered = async (uid) => {
    const e = await engine(page);
    const node = IR.nodes.find((n) => n.uid === uid);
    const pos = expectedPosition(uid);
    stat.visited += 1;
    const anchors = await anchorCounts(page);
    L.check(scene, `${uid} 左右の取り付け点`, anchorsOk(anchors));
    L.check(scene, `${uid} 左に成果物のビューが見える`, await visibleViaTab(page, counter, TAB_ART, `${LEFT} ${ARTIFACT}`));
    // Step の題と do（右の TERM 側）。題は Step ごとに異なるので、一致すれば Step ごとに変わったことになる。
    const texts = await page.evaluate(([t, d]) => ({
      titles: [...document.querySelectorAll(t)].map((x) => x.textContent), dos: [...document.querySelectorAll(d)].map((x) => x.textContent),
    }), [TITLE, DO]);
    mark(stat.title, texts.titles.length === 1 && texts.titles[0] === node.step.title
      && texts.dos.length === 1 && texts.dos[0] === `いまやること${node.step.do}`);
    L.check(scene, `${uid} 題と do が見える`, await visibleViaTab(page, counter, TAB_OPS, TITLE) && await visibleViaTab(page, counter, TAB_OPS, DO));
    // 位置の表示（SHELL-8）。章が変わったら章の ID と K が変わる（SHELL-10）。
    const pf = await positionFailures(page, uid);
    mark(stat.position, pf.length === 0);
    if (pf.length) L.check(scene, `${uid} 位置: ${pf.join(' / ')}`, false);
    if (lastChapter !== null && lastChapter !== pos.chapter) stat.crossings += 1;
    lastChapter = pos.chapter;
    // 右の変化: ログの項目が前に入った時点より増える。
    const logs = await logCount(page);
    if (prev) mark(stat.right, logs > prev.logs);
    // 左の変化: 前の Step の effects（put・set_attr）で左の指紋が変わる。engine.artifacts が変わったのに左が変わらない回数も数える。
    const print = await leftPrint(page);
    const arts = JSON.stringify(e.artifacts);
    if (prev) {
      if (hasLeftEffect(prev.uid)) mark(stat.left, print !== prev.print);
      if (arts !== prevArtifacts) L.check(scene, `${prev.uid} 状態の成果物が変わったら左も変わる`, print !== prev.print);
    }
    L.check(scene, `${uid} 状態と世界が一致`, e.worldMatchesState === true && e.incrementalMatches === true);
    prev = { uid, print, logs };
    prevArtifacts = arts;
    lastStep = uid;
  };

  const onStep = async (uid, status) => {
    if (status === 'entered') await observeEntered(uid);
    if (hooks.afterStep) await hooks.afterStep(uid, status, { page, width, counter });
  };
  const d = paneAware(page, mouseDriver(page), counter);
  await journey(page, d, scene, L, null, { chaptered: true, onStep });

  // 最後の Step の effects も左へ出る（達成の時点と、入った時点の指紋を比べる）。
  const e = await engine(page);
  if (prev && hasLeftEffect(prev.uid)) mark(stat.left, (await leftPrint(page)) !== prev.print);
  const statuses = e.stateWithoutLog.run.slices.main?.status ?? {};
  const achieved = Object.values(statuses).filter((s) => s === 'achieved').length;
  L.check(scene, `${LAST} で終わった`, e.current === LAST && lastStep === LAST);
  L.check(scene, '20 Step を訪れた', stat.visited === STEPS);
  L.check(scene, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
  L.check(scene, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
  const narrow = width < 960;
  L.check(scene, narrow ? 'タブを 1 回以上切り替えた' : 'タブの切り替えが 0 回', narrow ? counter.clicks >= 1 : counter.clicks === 0);
  return {
    stat, counter, achieved, current: e.current,
    // 幅をまたぐ比較用（世界状態と各 Step の状態）。
    world: JSON.stringify(e.stateWithoutLog.run.world),
    statuses: JSON.stringify(statuses),
  };
}

// 記録の 1 行。入力した値や下書きの中身は出さない。様式は docs/testing.md 5.269。
export function recordLine({ width, csp, result, L, rec }) {
  const s = result.stat;
  const f = (p) => `${p[0]}/${p[1]}`;
  return `SHELL-JOURNEY width=${width} csp=${csp ? 'on' : 'off'} steps=${result.achieved}/${STEPS} crossings=${s.crossings}`
    + ` left-changes=${f(s.left)} right-changes=${f(s.right)} title=${f(s.title)} position=${f(s.position)}`
    + ` tab-clicks=${result.counter.clicks} external=${rec.external.length} page-errors=${rec.pageErrors.length} ledger=${L.summary()}`;
}
