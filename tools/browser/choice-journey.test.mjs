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
// 確かめないこと: data-theme の値・文字の大きさ・行間の主張（製品の画面はまだ data-theme を付けない。TASK-749 の担当）、
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
  stepsOf, prepareDelivery, measureScreen, runJourney, summaryLine,
} from './choice-journey.mjs';
import { loadConfig, buildIr, judge } from './choice-journey-config.mjs';

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
      diag(`second-pack ${cfg.name} csp=${csp} stopped=${r.stoppedAt ? `${r.stoppedAt.cause}@${r.stoppedAt.after}` : 'none'} rows=${r.end.rows.count} hint-open=${r.end.parts['hint-open'].count} hint-box=${r.end.parts['shell-hint'].count} checkpoint-restore=${r.end.parts['checkpoint-restore'].visible} delete=${r.end.parts['shell-delete'].count}`);
    }
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
