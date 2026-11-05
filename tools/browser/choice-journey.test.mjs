// 入力欄のない教材を完走させ画面の値を測る道具（choice-journey.mjs）の試験（TASK-748 1/2・2/2。SECOND-4・SECOND-7）。CSP あり・なしの両方で流す。
//
// 教材固有の値（操作の列・各操作の後の期待・派生版を作る書き換え・結果の期待）は tools/browser/choice-journey/*.json の設定ファイルにあり、
// 本試験は設定を choice-journey-config.mjs で読む（設定の形の試験は choice-journey-config.test.mjs。CLI は choice-journey-run.mjs）。
// 教材は docs/examples/choice-only-pack（読むだけ。書き換えるのは一時ディレクトリの写し）。配信物は FANDHE_WEB_AUTHOR_DIR（IR 同梱の配信する設定）を
// 一時ディレクトリへ写して ir/main.json を差し替えたもので、wasm は作り直さない。
//
// 解釈（最大の限界）: 例の最後の Step は「行を指す」操作を要するが、画面では指せる行が 0 件になり、そのままの例は最後の Step で止まる。
// 原因は 2 つ: (1) `left_pane.world_list: hidden` が行を作らない（docs/known-gaps.md の PACK-22）、(2) 行を作る put が開始点の effects にあり、
// 画面は開始点の effects を適用しない（docs/cli.md の経過措置。CLI の test は適用するので通る）。そこで完走は、world_list を shown にし
// put を st-pick の受付の effects へ移した派生版で確かめ、そのままの例は「最後の Step に入るまで」と止まった位置の記録にとどめる。
//   P  前提。validate が 0。例と派生版が compile できる。IR の差が pack.left_pane.world_list だけ。stepsOf が 2 節点
//   A  派生版の設定を開始から完走まで進める。judge が空（2 節点が達成・復習で達成・入力の口が無い・ビューの切り替えが見えない・通信とエラー 0 件）
//   B  そのままの例の設定。最後の Step に入るまでで止まる期待（PACK-22 の停止の位置）を judge が合格にする
//   C  道具の対照。A の開始の時点のページに入力欄・style 属性・ボタンを足すと、測った値が増える
//   F  第 2 教材の誤答の場面（review-request-reject-scenes。TASK-750・SECOND-12 の画面の部分）。7 場面の拒否の行が IR の分類・文面と一致し、
//      拒否の前後で左の行が変わらず、完走する。負の対照 2 つ（表示する IR の文面だけ変える・受け付けられる操作を拒否と宣言する）で judge が不合格にする
//   E  第 2 教材（template-workshop/examples/review-request。TASK-749 1/3）の入力の領域と右の領域。そのままの教材は 2 つ目の Step の確認（指す）で止まる位置を、
//      派生版（world_list を shown、練習データの 5 行を st-1 の受付へ移す）は完走を、judge が合格にする。入口は選択肢のボタンだけ・案内 4 種と入力の注意書きの文字が無い・
//      見える input は読み込み用のファイル選択 1 つ・右の領域の操作要素（停止・保存・書き出し・読み込み）が出る。案内レベルのボタンは製品の画面に無く、ヒントを開くは教材がヒントを持たないので出ない
//   D  別の教材の設定（IR で指定。3 Step・選択肢・指す・承認）を同じ道具で完走させる（設定を差し替えるだけで別の教材にも使える証拠）
//   G  道具の対照（TASK-749 2/3。CSP ありの 1 回）。派生版で opt-start を押した後のページ（行 5 本が【追加】）に 4 つの手を加え、測った値が変わって
//      judge が欄名の理由を返すこと: 印の文字を消す（markGlyphText）・別のビューを見せる（visibleViews）・切り替えのボタンを足す（leftSwitch）・行を 1 本消す（rowPaths）。手を加えない測定は理由が空
//      E は、さらに左の領域（SECOND-6・SECOND-8）。見えるビューは文書だけ・切り替えが無い・そのままの教材は行 0 件（PACK-22）、派生版は行 5 本と受付ごとの変化の印
//   H  幅 800px（TASK-749 3/3。5.449）。tablist が出る画面で、操作ごとに実クリックでタブを切り替え、「成果物」タブで入力の領域と左の領域の期待が成り立つ。
//      H-対照は、操作タブのまま左を測ると leftHidden、中身のある style 属性・<style> 要素を足すと inlineStyle の理由が出ること
//   I  依頼文の見せ方（TASK-755。G-4・H-3・SECOND-14・SECOND-21）。第 2 教材は 5 行の属性の形のままで、本文を出さない側を採った（SECOND-14 は検討中）。
//      派生版（vocab 1.3・`body` なし）は行 5 本のどの行にも本文の区画が 0 件。正の対照（派生版に vocab 1.4 と `body: shown` を足した review-request-body-shown）は
//      5 本の行がそれぞれ本文の区画を 1 つ持つ。正の対照の期待を派生版の結果に当てると docBodyRows が不合格になる（数え方が空振りでない証拠）
//   J  道具の対照（TASK-755。CSP ありの 1 回）。派生版で opt-start を押した後のページの行に偽の本文の区画を足すと、judge が docBodyRows の理由を返す
//   K  道具の対照（Issue #2052。CSP ありの 1 回）。派生版で opt-start を押した後の行の文字（印と本文の区画を除く）を測り、含まれる文字の期待は合格、
//      含まれない文字の期待は texts@op[0]・rowTexts@end の理由が出る。行に偽の本文の区画を足しても text は変わらない。今の製品の行は data-kind を持たず kind は空文字
// 確かめないこと: 本文の中身・4 つの選択が連結された依頼文（H-3 で作れない）、data-theme の値・文字の大きさ・行間の主張（製品の画面はまだ data-theme を付けない。第 2 教材の設定では主張せず、測った値を記録する。SECOND-10 は未達）、
// 本物の第 2 教材、人が画面を見た評価。
// 入力: FANDHE_PACK_BIN・FANDHE_WEB_AUTHOR_DIR。未設定・不在は skip にせず失敗にする。出力は名前・件数・合否・数値だけ。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, packBinFromEnv, withServer, withCspMode, openPage, waitMounted,
} from './harness.mjs';
import {
  stepsOf, prepareDelivery, measureScreen, measureLeft, runJourney, summaryLine,
} from './choice-journey.mjs';
import { loadConfig, buildIr, judge } from './choice-journey-config.mjs';
import { choiceSel } from './ch0-3-journey.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PACK = path.resolve(HERE, '../../docs/examples/choice-only-pack');
const CONFIG_DIR = path.join(HERE, 'choice-journey');
const cfgPath = (name) => path.join(CONFIG_DIR, `${name}.json`);

let browser;
let bin;
let webDir;
let irShown;
let irAsIs;
let irOther;
let stepInfo;
let cfgDerived;
let cfgAsIs;
let cfgOther;
let cfgReject;
let irReject;
let cfgRrAsIs;
let cfgRrDerived;
let irRrAsIs;
let irRrDerived;
let cfgRr800;
let cfgRrDerived800;
let cfgRrBody;
let irRrBody;

const diag = (s) => console.log(`# diagnostic: ${s}`);

before(async () => {
  bin = packBinFromEnv();
  webDir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(webDir && fs.statSync(webDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS');
  cfgAsIs = loadConfig(cfgPath('choice-only-pack'));
  cfgDerived = loadConfig(cfgPath('choice-only-pack-derived'));
  cfgOther = loadConfig(cfgPath('input-field-hidden'));
  irAsIs = buildIr({ bin, config: cfgAsIs });
  cfgReject = loadConfig(cfgPath('review-request-reject-scenes'));
  irReject = buildIr({ bin, config: cfgReject });
  irShown = buildIr({ bin, config: cfgDerived });
  irOther = buildIr({ bin, config: cfgOther });
  cfgRrAsIs = loadConfig(cfgPath('review-request'));
  cfgRrDerived = loadConfig(cfgPath('review-request-derived'));
  irRrAsIs = buildIr({ bin, config: cfgRrAsIs });
  irRrDerived = buildIr({ bin, config: cfgRrDerived });
  cfgRr800 = loadConfig(cfgPath('review-request-800'));
  cfgRrDerived800 = loadConfig(cfgPath('review-request-derived-800'));
  cfgRrBody = loadConfig(cfgPath('review-request-body-shown'));
  irRrBody = buildIr({ bin, config: cfgRrBody });
  stepInfo = stepsOf(JSON.parse(irShown));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

test('P: validate が 0 で、例と派生版の IR の差が world_list と r-a の受付の effects だけ、節点が 2 つ', () => {
  const r = spawnSync(bin, ['validate', PACK], { encoding: 'utf8' });
  assert.equal(r.status, 0, `validate exit ${r.status}`);
  const a = JSON.parse(irAsIs);
  const b = JSON.parse(irShown);
  assert.equal(a.pack.left_pane.world_list, 'hidden');
  assert.equal(b.pack.left_pane.world_list, 'shown');
  b.pack.left_pane.world_list = 'hidden';
  const rule = (ir) => ir.nodes.find((n) => n.uid === 'main/st-pick').step.rules.find((r) => r.id === 'r-a').outcome;
  assert.notDeepEqual(rule(b).effects, rule(a).effects, 'accept effects were changed');
  rule(b).effects = rule(a).effects;
  assert.deepEqual(b, a, 'IR differs only in pack.left_pane.world_list and the r-a accept effects');
  assert.deepEqual(stepInfo.order, ['main/st-pick', 'main/st-point']);
  assert.deepEqual(stepInfo.finals, ['main/st-point']);
});

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  // 設定 1 本・IR 1 つを CSP の設定 csp で流し、結果と judge の理由を返す。設定の値はすべて引数から取る。
  // refIrText は期待の文面の出どころ（既定は表示する IR と同じ）。表示だけ変えた対照で別に渡す。
  async function runWith(config, irText, refIrText = irText) {
    const { dir: copy, cleanup } = prepareDelivery({ webDir, irText });
    try {
      const ir = JSON.parse(irText);
      const refIr = JSON.parse(refIrText);
      const info = stepsOf(ir);
      let out;
      await withCspMode({ dir: copy, csp }, (dir) => withServer({ dir, base: config.base }, async (srv) => {
        out = await runJourney({
          browser, srv, ir, refIr, sequence: config.sequence, expect: config.sequence.map((s) => s.expect), initial: config.initial,
          titles: info.titles, finals: info.finals, viewport: config.viewport,
        });
      }));
      console.log(summaryLine(config.name, csp, out));
      return { out, why: judge(config, out) };
    } finally {
      cleanup();
    }
  }

  test(`A: 派生版（world_list を shown にした設定）を開始から完走まで進め、画面の値を測る（${mode}）`, { timeout: 120000 }, async () => {
    const { out: r, why } = await runWith(cfgDerived, irShown);
    assert.deepEqual(why, [], 'judge: no failed expectation');
    assert.equal(r.stoppedAt, null, `stopped: ${JSON.stringify(r.stoppedAt)}`);
    assert.equal(r.completed, true);
    assert.equal(r.reached.length, 1 + cfgDerived.sequence.length);
    assert.equal(r.review.length, 2);
    assert.ok(r.review.every((x) => x.achieved === 'true'), 'all review rows achieved');
    assert.equal(r.start.inputArea.stop.count, 1, 'one stop button');
    // 値は主張せず diagnostic に出す。data-theme は見つかったときだけ教材の値と一致を主張する。
    const themes = r.end.theme.values;
    if (themes.length > 0) assert.ok(themes.every((t) => t === JSON.parse(irShown).pack.appearance.theme), 'data-theme matches pack.appearance.theme');
    else diag('data-theme absent (not yet wired)');
    diag(`derived csp=${csp} font=${JSON.stringify(r.end.font)} rows=${r.end.rows.count} right=${JSON.stringify(r.end.rightButtons)} inline=${JSON.stringify(r.end.inlineStyle)}`);
  });

  test(`B: そのままの例の設定は最後の Step に入るまでで止まる期待が合格になる（${mode}）`, { timeout: 120000 }, async () => {
    const { out: r, why } = await runWith(cfgAsIs, irAsIs);
    assert.deepEqual(why, [], 'judge: no failed expectation');
    assert.ok(r.reached.includes('main/st-pick:accepted'), 'first step accepted');
    assert.ok(r.reached.includes('main/st-point:entered'), 'last step entered');
    assert.equal(r.completed, false, 'cannot complete without rows');
    diag(`PACK-22 / starts effects not applied by the screen: rows=${r.end.rows.count} stoppedAt=${r.stoppedAt ? `${r.stoppedAt.cause}@${r.stoppedAt.after}` : 'none'}`);
  });

  test(`F: 第 2 教材の誤答 7 場面が画面に出て、拒否のあと左の行が変わらず完走する（${mode}）`, { timeout: 120000 }, async () => {
    const { out: r, why } = await runWith(cfgReject, irReject);
    assert.deepEqual(why, [], 'judge: no failed expectation');
    assert.equal(r.completed, true);
    assert.equal(r.rejections.length, 7);
    assert.ok(r.rejections.every((x) => x.reasonOk && x.nextOk && x.rowsUnchanged && x.stepUnchanged), 'every rejection matches the IR and leaves the rows');
    assert.equal(r.external, 0);
    diag(`reject csp=${csp} categories=${[...new Set(r.rejections.map((x) => x.category))].sort().join('+')} live-response=${r.rejections.map((x) => `${x.liveResponse.count}/${x.liveResponse.hasText}`).join(',')}`);
  });

  test(`F-1: 負の対照。表示する IR の拒否の文面だけ変えると text で不合格になる（${mode}）`, { timeout: 120000 }, async () => {
    const shown = JSON.parse(irReject);
    shown.rejections['rj-skip'].message += '（改）';
    const { why } = await runWith(cfgReject, JSON.stringify(shown), irReject);
    const i = cfgReject.sequence.findIndex((o) => o.rejection === 'rj-skip');
    assert.ok(why.includes(`reject[${i}].text`), `text mismatch is detected: ${why.join(',')}`);
    assert.ok(why.includes('rejectScenes'), 'the scene is not counted');
  });

  test(`F-2: 負の対照。受け付けられる操作を拒否と宣言すると missing で不合格になる（${mode}）`, { timeout: 120000 }, async () => {
    const cfg = structuredClone(cfgReject);
    const i = cfg.sequence.findIndex((o) => o.kind === 'choice' && o.id === 'opt-pair');
    Object.assign(cfg.sequence[i], { scene: 2 });
    cfg.sequence[i].expect.status = 'rejected';
    const { why } = await runWith(cfg, irReject);
    assert.ok(why.includes(`reject[${i}].missing`), `missing rejection is detected: ${why.join(',')}`);
  });

  test(`E: 第 2 教材の入力の領域と右の領域。そのままの教材は止まる位置、派生版は完走で judge が合格（${mode}）`, { timeout: 180000 }, async () => {
    for (const [cfg, ir, completed] of [[cfgRrAsIs, irRrAsIs, false], [cfgRrDerived, irRrDerived, true]]) {
      const { out: r, why } = await runWith(cfg, ir);
      assert.deepEqual(why, [], `judge: no failed expectation (${cfg.name})`);
      assert.equal(r.completed, completed);
      for (const m of [r.start, r.end]) {
        assert.equal(m.fileInputs.visible, 1, 'one visible file input');
        assert.equal(m.fileInputs.inImport, 1, 'the file input is inside the import container');
        assert.equal(m.notice.textLength, 0, 'input notice has no text');
      }
      assert.ok(r.start.parts['choice-btn'].visible > 0, 'choice buttons at start');
      // 左の領域（TASK-749 2/3）。見えるビューは文書だけで、切り替えの部品は測った全時点で 0 件。judge が行の集合と印を見る。
      for (const m of [r.left.start, ...r.left.ops, r.left.end]) {
        assert.deepEqual(m.views.shown, ['document'], 'only the document view is visible');
        assert.equal(m.select.visibleButtons, 0);
        assert.equal(m.settled, true);
      }
      assert.equal(r.left.ops.length, r.reached.length - 1, 'left is measured after every reached operation');
      assert.equal(r.left.end.rows.length, completed ? 5 : 0, 'rows: 5 in the derived pack, 0 in the real pack (PACK-22)');
      const markedOps = r.left.ops.filter((o) => o.rows.some((x) => x.mark !== '')).length;
      diag(`second-pack-left ${cfg.name} csp=${csp} views=${r.left.end.views.shown.length}/${r.left.end.views.total} rows-end=${r.left.end.rows.length} ops-measured=${r.left.ops.length} ops-with-marks=${markedOps}`);
      diag(`second-pack ${cfg.name} csp=${csp} stopped=${r.stoppedAt ? `${r.stoppedAt.cause}@${r.stoppedAt.after}` : 'none'} rows=${r.end.rows.count} hint-open=${r.end.parts['hint-open'].count} hint-box=${r.end.parts['shell-hint'].count} checkpoint-restore=${r.end.parts['checkpoint-restore'].visible} delete=${r.end.parts['shell-delete'].count}`);
    }
  });

  test(`H: 幅 800px の「成果物」タブでも入力の領域と左の領域の期待が成り立つ（${mode}）`, { timeout: 180000 }, async () => {
    for (const [cfg, ir, completed] of [[cfgRr800, irRrAsIs, false], [cfgRrDerived800, irRrDerived, true]]) {
      const { out: r, why } = await runWith(cfg, ir);
      assert.deepEqual(why, [], `judge: no failed expectation (${cfg.name})`);
      assert.equal(r.completed, completed);
      assert.equal(r.start.panes.tablist, true, 'tablist is visible at 800px');
      assert.equal(r.start.panes.selected, 'ops');
      assert.equal(r.artifactsTab.end.panes.selected, 'artifacts');
      assert.equal(r.artifactsTab.end.panes.left, true);
      assert.equal(r.artifactsTab.end.rows.count, completed ? 5 : 0);
      assert.equal(r.left.ops.length, r.reached.length - 1);
      assert.ok(r.left.ops.every((m) => m.panes.left === true && m.panes.selected === 'artifacts'), 'left is measured on the artifacts tab');
      diag(`second-pack-800 ${cfg.name} csp=${csp} measured=${2 + 2 + r.left.ops.length} inline=${JSON.stringify(r.end.inlineStyle)} main-theme=${r.end.theme.main} body=${JSON.stringify(r.end.font.body)} file-inputs-artifacts=${JSON.stringify(r.artifactsTab.end.fileInputs)}`);
    }
  });

  test(`I: 依頼文の見せ方。派生版は行に本文が出ず、body: shown の正の対照では 5 行すべてに出る（${mode}）`, { timeout: 180000 }, async () => {
    const plain = await runWith(cfgRrDerived, irRrDerived);
    assert.deepEqual(plain.why, [], 'judge: derived pack shows no body');
    assert.equal(JSON.parse(irRrDerived).pack.vocab, '1.3');
    assert.notEqual(JSON.parse(irRrDerived).views['v-doc'].body, 'shown');
    const shown = await runWith(cfgRrBody, irRrBody);
    assert.deepEqual(shown.why, [], 'judge: body-shown control shows a body in every row');
    assert.equal(JSON.parse(irRrBody).pack.vocab, '1.4');
    assert.equal(JSON.parse(irRrBody).views['v-doc'].body, 'shown');
    assert.equal(plain.out.completed, true);
    assert.equal(shown.out.completed, true);
    assert.equal(plain.out.left.end.rows.length, 5);
    assert.equal(shown.out.left.end.rows.length, 5);
    assert.ok(plain.out.left.end.rows.every((x) => x.docBody === 0) && plain.out.left.end.docBodyTotal === 0, 'derived: no body section');
    assert.ok(shown.out.left.end.rows.every((x) => x.docBody === 1) && shown.out.left.end.docBodyTotal === 5, 'body-shown: one body section per row');
    // 数え方の空振りの確認。互いの期待を当てると docBodyRows が不合格になる。
    assert.ok(judge(cfgRrBody, plain.out).some((w) => w.startsWith('docBodyRows@')), 'body-shown expectation fails on the derived result');
    assert.ok(judge(cfgRrDerived, shown.out).some((w) => w.startsWith('docBodyRows@')), 'derived expectation fails on the body-shown result');
    diag(`doc-body csp=${csp} derived-rows-with-body=0 shown-rows-with-body=${shown.out.left.end.rows.filter((x) => x.docBody > 0).length}`);
  });

  test(`D: 別の教材の設定（IR で指定）を同じ道具で完走させる（${mode}）`, { timeout: 120000 }, async () => {
    const { out: r, why } = await runWith(cfgOther, irOther);
    assert.deepEqual(why, [], 'judge: no failed expectation');
    assert.equal(r.stoppedAt, null, `stopped: ${JSON.stringify(r.stoppedAt)}`);
    assert.equal(r.completed, true);
    assert.equal(r.reached.length, 1 + cfgOther.sequence.length);
    assert.equal(r.review.length, 3);
    diag(`other csp=${csp} steps=${r.stepCount} view-buttons=${r.end.viewSwitch.visibleButtons}`);
  });
}

// 道具の対照で judge に渡す、画面以外の欄が合格の値の結果（画面の欄だけを差し替えて理由を見る）。
const baseResult = () => ({
  completed: true,
  stoppedAt: null,
  review: Array.from({ length: 8 }, (_, i) => ({ step: `s${i}`, achieved: 'true' })),
  nextChapter: 0,
  safety: [],
  external: 0,
  pageErrors: 0,
  consoleErrors: 0,
  cspWarnings: 0,
  run: { raw: 0, mouse: 1 },
  irFetch: [200],
});

test('C: 道具の対照。入力欄・style 属性・ボタンを足すと測った値が増える（CSP あり）', { timeout: 120000 }, async () => {
  const { dir: copy, cleanup } = prepareDelivery({ webDir, irText: irShown });
  try {
    await withCspMode({ dir: copy, csp: true }, (dir) => withServer({ dir, base: cfgDerived.base }, async (srv) => {
      const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
      try {
        await page.goto(srv.url);
        await waitMounted(page);
        await page.waitForSelector('[data-testid="step-title"]', { timeout: 30000 });
        const before = await measureScreen(page);
        await page.evaluate(() => {
          const right = document.querySelector('[data-testid="shell-right"]');
          const t = document.createElement('textarea');
          t.setAttribute('data-testid', 'draft-input');
          right.appendChild(t);
          right.appendChild(document.createElement('textarea'));
          right.appendChild(document.createElement('select'));
          const d = document.createElement('div');
          d.setAttribute('style', 'color: red');
          d.textContent = 'x';
          right.appendChild(d);
          const b = document.createElement('button');
          b.setAttribute('data-testid', 'probe-btn');
          b.textContent = 'x';
          right.appendChild(b);
        });
        const after = await measureScreen(page);
        assert.equal(after.parts['draft-input'].count, before.parts['draft-input'].count + 1);
        assert.equal(after.parts['stop-btn'].visible, before.parts['stop-btn'].visible);
        assert.equal(after.inputArea.absent['draft-input'], before.inputArea.absent['draft-input'] + 1);
        assert.equal(after.inputArea.absent.textarea, before.inputArea.absent.textarea + 2);
        assert.equal(after.inputArea.absent.select, before.inputArea.absent.select + 1);
        // visibleInputs は input と textarea だけを数える（select は含めない）。足した textarea 2 個の分だけ増える。
        assert.equal(after.inputArea.visibleInputs.length, before.inputArea.visibleInputs.length + 2);
        assert.equal(after.inlineStyle.attrs, before.inlineStyle.attrs + 1);
        assert.equal(after.rightButtons.length, before.rightButtons.length + 1);
        // 案内・ファイル選択・注意書きの文字・停止の非表示・承認のボタンも測った値に出て、judge が理由を返す。
        await page.evaluate(() => {
          const right = document.querySelector('[data-testid="shell-right"]');
          const add = (tag, id, attrs = {}) => {
            const e = document.createElement(tag);
            if (id) e.setAttribute('data-testid', id);
            for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
            e.textContent = 'x';
            right.appendChild(e);
            return e;
          };
          add('p', 'key-hint');
          add('input', null, { type: 'file' });
          add('button', 'approval-btn');
          const n = document.querySelector('[data-testid="input-notice"]');
          if (n) n.textContent = 'probe';
          document.querySelector('[data-testid="stop-btn"]')?.setAttribute('hidden', '');
        });
        const probed = await measureScreen(page);
        assert.equal(probed.parts['key-hint'].count, 1);
        assert.equal(probed.parts['approval-btn'].visible, 1);
        assert.equal(probed.fileInputs.visible, after.fileInputs.visible + 1);
        assert.equal(probed.fileInputs.inImport, after.fileInputs.inImport, 'added file input is outside the import container');
        assert.ok(probed.notice.textLength > 0);
        assert.equal(probed.parts['stop-btn'].visible, 0);
        const why = judge(cfgRrDerived, { ...baseResult(), start: probed, end: probed });
        for (const w of ['entryPorts@end', 'inputGuidesAbsent@end', 'inputNotice@end', 'fileInputs@end', 'controls.stop@end']) assert.ok(why.includes(w), `judge reports ${w}`);
      } finally {
        await context.close();
      }
    }));
  } finally {
    cleanup();
  }
});

// 道具の対照（TASK-749 2/3）。派生版で opt-start を押した後（行 5 本が【追加】）のページに手を加え、judge が欄名の理由を返すことを確かめる。
test('G: 道具の対照。印の文字・見えるビュー・切り替えのボタン・行の数を変えると、左の領域の理由が出る（CSP あり）', { timeout: 180000 }, async () => {
  const { dir: copy, cleanup } = prepareDelivery({ webDir, irText: irRrDerived });
  // 開始・終了の観察だけを見る場面なので、操作ごとの marks（操作の観察が要る）を除いた設定で照合する。
  const cfgNoMarks = { ...cfgRrDerived, sequence: cfgRrDerived.sequence.map(({ marks, ...op }) => op) };
  const leftWhy = (m) => judge(cfgNoMarks, { ...baseResult(), start: m.screen, end: m.screen, left: { start: m.left, ops: [], end: m.left } })
    .filter((w) => /^(visibleViews|leftSwitch|rowPaths|markGlyphText|marks|leftUnsettled|left-missing)/.test(w));
  try {
    await withCspMode({ dir: copy, csp: true }, (dir) => withServer({ dir, base: cfgRrDerived.base }, async (srv) => {
      const probe = async (hand) => {
        const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
        try {
          await page.goto(srv.url);
          await waitMounted(page);
          await page.waitForSelector('[data-testid="step-title"]', { timeout: 30000 });
          await page.locator(choiceSel(cfgRrDerived.sequence[0].id)).first().click({ timeout: 4000 });
          await page.waitForFunction(() => document.querySelectorAll('[data-testid="view-list"] > li[data-path]').length === 5, null, { timeout: 10000 });
          if (hand) await page.evaluate(hand);
          return { left: await measureLeft(page), screen: await measureScreen(page) };
        } finally {
          await context.close();
        }
      };
      const base0 = await probe(null);
      assert.equal(base0.left.rows.length, 5, 'control: 5 rows after the first accepted operation');
      assert.ok(base0.left.rows.every((x) => x.mark === 'add'), 'control: all rows are marked as added');
      assert.deepEqual(leftWhy(base0), [], 'control: untouched page has no left-area reason');
      const hands = [
        ['markGlyphText', () => { const e = document.querySelector('[data-testid="mark-text"]'); e.textContent = ''; }, (b, a) => assert.notEqual(a.left.rows.filter((x) => x.markText === '').length, 0)],
        ['visibleViews', () => {
          const other = document.querySelector('[data-testid="view-list"]:not([data-view="document"])');
          for (let e = other; e && e.getAttribute('data-testid') !== 'shell-left'; e = e.parentElement) e.removeAttribute('hidden');
        }, (b, a) => assert.equal(a.left.views.shown.length, b.left.views.shown.length + 1)],
        ['leftSwitch', () => {
          const btn = document.createElement('button');
          btn.setAttribute('data-testid', 'view-select-button');
          btn.textContent = 'x';
          document.querySelector('[data-testid="shell-left"]').appendChild(btn);
        }, (b, a) => assert.equal(a.left.select.visibleButtons, b.left.select.visibleButtons + 1)],
        ['rowPaths', () => { document.querySelector('[data-testid="view-list"] > li[data-path]').remove(); }, (b, a) => assert.equal(a.left.rows.length, b.left.rows.length - 1)],
      ];
      for (const [reason, hand, grew] of hands) {
        const after = await probe(hand);
        grew(base0, after);
        const why = leftWhy(after);
        assert.ok(why.some((w) => w.startsWith(reason)), `judge reports ${reason}: ${why.join(',')}`);
        diag(`left-control ${reason} reasons=${why.length}`);
      }
    }));
  } finally {
    cleanup();
  }
});

test('J: 道具の対照。行に偽の本文の区画を足すと docBodyRows の理由が出る（CSP あり）', { timeout: 120000 }, async () => {
  const { dir: copy, cleanup } = prepareDelivery({ webDir, irText: irRrDerived });
  const cfgNoMarks = { ...cfgRrDerived, sequence: cfgRrDerived.sequence.map(({ marks, ...op }) => op) };
  const bodyWhy = (m) => judge(cfgNoMarks, { ...baseResult(), start: m.screen, end: m.screen, left: { start: m.left, ops: [], end: m.left } })
    .filter((w) => w.startsWith('docBodyRows'));
  try {
    await withCspMode({ dir: copy, csp: true }, (dir) => withServer({ dir, base: cfgRrDerived.base }, async (srv) => {
      const probe = async (hand) => {
        const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
        try {
          await page.goto(srv.url);
          await waitMounted(page);
          await page.waitForSelector('[data-testid="step-title"]', { timeout: 30000 });
          await page.locator(choiceSel(cfgRrDerived.sequence[0].id)).first().click({ timeout: 4000 });
          await page.waitForFunction(() => document.querySelectorAll('[data-testid="view-list"] > li[data-path]').length === 5, null, { timeout: 10000 });
          if (hand) await page.evaluate(hand);
          return { left: await measureLeft(page), screen: await measureScreen(page) };
        } finally {
          await context.close();
        }
      };
      const base0 = await probe(null);
      assert.ok(base0.left.rows.every((x) => x.docBody === 0), 'control: no row has a body section');
      assert.deepEqual(bodyWhy(base0), [], 'control: untouched page has no body reason');
      const inRow = await probe(() => {
        const d = document.createElement('div');
        d.setAttribute('data-testid', 'artifact-doc-body');
        document.querySelector('[data-testid="view-list"] > li[data-path]').appendChild(d);
      });
      assert.equal(inRow.left.rows.filter((x) => x.docBody > 0).length, 1);
      assert.ok(bodyWhy(inRow).some((w) => w.startsWith('docBodyRows')), 'a body section inside a row is reported');
      const outside = await probe(() => {
        const d = document.createElement('div');
        d.setAttribute('data-testid', 'artifact-doc-body');
        document.querySelector('[data-testid="shell-left"]').appendChild(d);
      });
      assert.equal(outside.left.docBodyTotal, 1);
      assert.ok(bodyWhy(outside).some((w) => w.startsWith('docBodyRows')), 'a body section outside the rows is reported');
    }));
  } finally {
    cleanup();
  }
});

test('K: 道具の対照。行の文字を測り、含まれる文字の期待は合格、含まれない文字は texts@・rowTexts@ の理由が出る（CSP あり）', { timeout: 120000 }, async () => {
  const { dir: copy, cleanup } = prepareDelivery({ webDir, irText: irRrDerived });
  try {
    await withCspMode({ dir: copy, csp: true }, (dir) => withServer({ dir, base: cfgRrDerived.base }, async (srv) => {
      const probe = async (hand) => {
        const { page, context } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
        try {
          await page.goto(srv.url);
          await waitMounted(page);
          await page.waitForSelector('[data-testid="step-title"]', { timeout: 30000 });
          await page.locator(choiceSel(cfgRrDerived.sequence[0].id)).first().click({ timeout: 4000 });
          await page.waitForFunction(() => document.querySelectorAll('[data-testid="view-list"] > li[data-path]').length === 5, null, { timeout: 10000 });
          if (hand) await page.evaluate(hand);
          return { left: await measureLeft(page), screen: await measureScreen(page) };
        } finally {
          await context.close();
        }
      };
      const m = await probe(null);
      const rows = m.left.rows;
      assert.equal(rows.length, 5);
      assert.ok(rows.every((x) => x.text !== '' && x.text.includes(x.path)), 'every row has text that includes its path');
      assert.ok(rows.every((x) => x.markText !== '' && !x.text.includes(x.markText)), 'mark text is excluded from the row text');
      assert.ok(rows.every((x) => typeof x.kind === 'string'));
      const path0 = rows[0].path;
      const mk = (opTexts, endTexts) => ({
        ...cfgRrDerived,
        sequence: cfgRrDerived.sequence.map(({ marks, ...op }, i) => (i === 0 ? { ...op, texts: opTexts } : op)),
        expect: { ...cfgRrDerived.expect, rowTexts: endTexts },
      });
      const why = (cfg) => judge(cfg, { ...baseResult(), start: m.screen, end: m.screen, left: { start: m.left, ops: [{ index: 0, ...m.left }], end: m.left } })
        .filter((w) => /^(texts|rowTexts|textsMissing)/.test(w));
      const own = Object.fromEntries(rows.map((x) => [x.path, [x.text]]));
      assert.deepEqual(why(mk(own, own)), [], 'control: measured text as expectation passes');
      const absent = { [path0]: [`${rows[0].text} #absent`] };
      assert.deepEqual(why(mk(absent, undefined)), ['texts@op[0]']);
      assert.deepEqual(why(mk(undefined, absent)), ['rowTexts@end']);
      const withBody = await probe(() => {
        const d = document.createElement('div');
        d.setAttribute('data-testid', 'artifact-doc-body');
        d.textContent = 'probe-body';
        document.querySelector('[data-testid="view-list"] > li[data-path]').appendChild(d);
      });
      assert.deepEqual(withBody.left.rows.map((x) => x.text), rows.map((x) => x.text), 'body section text is excluded');
      diag(`row-text rows=${rows.length} kindsEmpty=${rows.filter((x) => x.kind === '').length}`);
    }));
  } finally {
    cleanup();
  }
});

// 道具の対照（TASK-749 3/3）。派生版の幅 800px で、左を「操作」タブのまま測ると leftHidden が出ること、中身のある style 属性と <style> 要素を足すと
// inlineStyle が出ること、手を加えない測定では理由が空であること。
test('H-対照: 道具の対照。800px で操作タブのまま左を測る・style を足すと理由が出る（CSP あり）', { timeout: 180000 }, async () => {
  const { dir: copy, cleanup } = prepareDelivery({ webDir, irText: irRrDerived });
  try {
    await withCspMode({ dir: copy, csp: true }, (dir) => withServer({ dir, base: cfgRrDerived800.base }, async (srv) => {
      const { page, context } = await openPage(browser, { origin: srv.origin, viewport: cfgRrDerived800.viewport });
      try {
        await page.goto(srv.url);
        await waitMounted(page);
        await page.waitForSelector('[data-testid="step-title"]', { timeout: 30000 });
        const screens = { start: await measureScreen(page) };
        const leftOps = await measureLeft(page);
        assert.equal(leftOps.panes.left, false, 'left is hidden on the ops tab');
        await page.locator('#pane-tab-artifacts').click({ timeout: 4000 });
        await page.waitForFunction(() => document.querySelector('[data-testid="pane-tabs"]')?.getAttribute('data-pane') === 'artifacts', null, { timeout: 4000 });
        const leftArt = await measureLeft(page);
        screens.artifacts = await measureScreen(page);
        const cfg = { ...cfgRrDerived800, sequence: cfgRrDerived800.sequence.map(({ marks, ...op }) => op) };
        const base1 = () => ({
          ...baseResult(), start: screens.start, end: screens.start, artifactsTab: { start: screens.artifacts, end: screens.artifacts },
        });
        const pick = (why, re) => why.filter((w) => re.test(w));
        const re = /^(leftHidden|inlineStyle|tabs@|artifactsTab)/;
        assert.deepEqual(pick(judge(cfg, { ...base1(), left: { start: leftArt, ops: [], end: leftArt } }), re), [], 'control: untouched page');
        assert.ok(judge(cfg, { ...base1(), left: { start: leftOps, ops: [], end: leftOps } }).includes('leftHidden@start'), 'ops-tab measurement is rejected');
        await page.evaluate(() => {
          const d = document.createElement('div');
          d.setAttribute('style', 'color: red');
          d.textContent = 'x';
          document.querySelector('[data-testid="shell-left"]').appendChild(d);
          const st = document.createElement('style');
          st.textContent = 'body { color: red }';
          document.head.appendChild(st);
        });
        const probed = await measureScreen(page);
        assert.equal(probed.inlineStyle.attrs, screens.artifacts.inlineStyle.attrs + 1);
        assert.equal(probed.inlineStyle.elements, screens.artifacts.inlineStyle.elements + 1);
        const why = judge(cfg, { ...base1(), artifactsTab: { start: probed, end: probed }, left: { start: leftArt, ops: [], end: leftArt } });
        assert.ok(why.includes('inlineStyle@artifacts-start'), `judge reports inlineStyle: ${why.join(',')}`);
        diag(`appearance-control reasons=${pick(why, re).length}`);
      } finally {
        await context.close();
      }
    }));
  } finally {
    cleanup();
  }
});
