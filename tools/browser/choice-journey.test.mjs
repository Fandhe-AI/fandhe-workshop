// 入力欄のない教材を完走させ画面の値を測る道具（choice-journey.mjs）の試験（TASK-748 1/2。SECOND-4・SECOND-7）。CSP あり・なしの両方で流す。
//
// 教材は docs/examples/choice-only-pack（読むだけ。書き換えるのは一時ディレクトリの写し）。配信物は FANDHE_WEB_AUTHOR_DIR（IR 同梱の配信する設定）を
// 一時ディレクトリへ写して ir/main.json を差し替えたもので、wasm は作り直さない。
//
// 解釈（最大の限界）: 例の最後の Step は「行を指す」操作を要するが、画面では指せる行が 0 件になり、そのままの例は最後の Step で止まる。
// 原因は 2 つ: (1) `left_pane.world_list: hidden` が行を作らない（docs/known-gaps.md の PACK-22）、(2) 行を作る put が開始点の effects にあり、
// 画面は開始点の effects を適用しない（docs/cli.md の経過措置。CLI の test は適用するので通る）。そこで完走は、world_list を shown にし
// put を st-pick の受付の effects へ移した派生版で確かめ、そのままの例は「最後の Step に入るまで」と止まった位置の記録にとどめる。
//   P  前提。validate が 0。例と派生版が compile できる。IR の差が pack.left_pane.world_list だけ。stepsOf が 2 節点
//   A  派生版を開始から完走まで進める。2 節点が達成・復習で達成・入力の口が無い・ビューの切り替えが見えない・通信とエラー 0 件
//   B  そのままの例。最初の Step の達成と最後の Step への進入まで。指せる行が 0 件なら PACK-22 の停止の位置を diagnostic に出す（不合格にしない）
//   C  道具の対照。A の開始の時点のページに入力欄・style 属性・ボタンを足すと、測った値が増える
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
  ABSENT, stepsOf, prepareDelivery, compilePack, measureScreen, runJourney, summaryLine,
} from './choice-journey.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PACK = path.resolve(HERE, '../../docs/examples/choice-only-pack');
const BASE = '/fandhe-choice-journey/';
const WORLD_LINE = 'world_list: hidden ';

// 例の tests/playthrough.yaml（pt-main）に沿った操作の列と期待値。2/2 で設定ファイルへ移す。
const SEQUENCE = [
  { kind: 'choice', id: 'opt-a' },
  { kind: 'choice', id: 'ans-yes' },
  { kind: 'row', path: 'memo/draft.md' },
  { kind: 'row', path: 'memo/draft.md' },
];
const EXPECT = [
  { uid: 'main/st-pick', status: 'accepted' },
  { uid: 'main/st-point', status: 'entered' },
  { uid: 'main/st-point', status: 'accepted' },
  { uid: 'main/st-point', status: 'achieved' },
];
const INITIAL = { uid: 'main/st-pick', status: 'entered' };

let browser;
let bin;
let webDir;
let irShown;
let irAsIs;
let stepInfo;

const diag = (s) => console.log(`# diagnostic: ${s}`);

// 派生版の書き換えは 2 つ。world_list を shown にし、行を作る put を開始点の effects から st-pick の受付の effects へ移す。
// 後者が要るのは、画面が開始点の effects を適用しないため（docs/cli.md の経過措置）。
const SET_ATTR = '- set_attr: { path: memo/draft.md, attr: detail, value: A を選びました }';
const PUT_ON_ACCEPT = '- put: { path: memo/draft.md, fixture: fx-note, attrs: { title: 下書き, detail: A を選びました } }';
function replaceOnce(file, from, to) {
  const text = fs.readFileSync(file, 'utf8');
  const next = text.replace(from, to);
  assert.notEqual(next, text, `mutation applied: ${path.basename(file)}`);
  fs.writeFileSync(file, next, 'utf8');
}
function mutateWorld(work) {
  replaceOnce(path.join(work, 'workshop.yaml'), WORLD_LINE, 'world_list: shown  ');
  replaceOnce(path.join(work, 'chapters/c-pick/chapter.yaml'), SET_ATTR, PUT_ON_ACCEPT);
}

before(async () => {
  bin = packBinFromEnv();
  webDir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(webDir && fs.statSync(webDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS');
  irAsIs = compilePack({ bin, packDir: PACK });
  irShown = compilePack({ bin, packDir: PACK, edit: mutateWorld });
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

  async function runWith(irText, label) {
    const { dir: copy, cleanup } = prepareDelivery({ webDir, irText });
    try {
      let out;
      await withCspMode({ dir: copy, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        out = await runJourney({
          browser, srv, ir: JSON.parse(irText), sequence: SEQUENCE, expect: EXPECT, initial: INITIAL,
          titles: stepInfo.titles, finals: stepInfo.finals,
        });
      }));
      console.log(summaryLine(label, csp, out));
      return out;
    } finally {
      cleanup();
    }
  }

  function commonClaims(r) {
    assert.deepEqual(r.safety, [], 'safety counts are zero');
    assert.equal(r.external, 0, 'no external requests');
    assert.equal(r.pageErrors, 0, 'no page errors');
    assert.equal(r.consoleErrors, 0, 'no console errors');
    assert.equal(r.cspWarnings, 0, 'no CSP warnings');
    assert.equal(r.run.raw, 0, 'no raw-trigger clicks');
    assert.ok(r.run.mouse > 0, 'real mouse operations');
    assert.deepEqual(r.irFetch, [200], 'ir/main.json fetched once with 200');
    for (const m of [r.start, r.end]) {
      for (const id of ABSENT) assert.equal(m.inputArea.absent[id], 0, `${id} absent`);
      assert.equal(m.inputArea.stop.count, 1, 'one stop button');
      assert.equal(m.viewSwitch.visibleButtons, 0, 'view switch buttons hidden');
      // 見えている input は履歴の読み込み用のファイル選択だけ（0 か 1 件）。
      assert.ok(m.inputArea.visibleInputs.every((x) => x.endsWith('|input|file')), `visible inputs: ${m.inputArea.visibleInputs.join(',')}`);
    }
  }

  test(`A: 派生版（world_list: shown）を開始から完走まで進め、画面の値を測る（${mode}）`, { timeout: 120000 }, async () => {
    const r = await runWith(irShown, 'derived');
    assert.equal(r.stoppedAt, null, `stopped: ${JSON.stringify(r.stoppedAt)}`);
    assert.equal(r.completed, true);
    assert.equal(r.reached.length, 1 + SEQUENCE.length);
    assert.equal(r.review.length, 2);
    assert.ok(r.review.every((x) => x.achieved === 'true'), 'all review rows achieved');
    assert.equal(r.nextChapter, 0, 'one chapter: no next-chapter button');
    commonClaims(r);
    // 値は主張せず diagnostic に出す。data-theme は見つかったときだけ教材の値と一致を主張する。
    const themes = r.end.theme.values;
    if (themes.length > 0) assert.ok(themes.every((t) => t === JSON.parse(irShown).pack.appearance.theme), 'data-theme matches pack.appearance.theme');
    else diag('data-theme absent (not yet wired)');
    diag(`derived csp=${csp} font=${JSON.stringify(r.end.font)} rows=${r.end.rows.count} right=${JSON.stringify(r.end.rightButtons)} inline=${JSON.stringify(r.end.inlineStyle)}`);
  });

  test(`B: そのままの例は最後の Step に入るまで進み、指せる行の有無で完走を分ける（${mode}）`, { timeout: 120000 }, async () => {
    const r = await runWith(irAsIs, 'as-is');
    assert.ok(r.reached.includes('main/st-pick:accepted'), 'st-pick accepted');
    assert.ok(r.reached.includes('main/st-point:entered'), 'st-point entered');
    commonClaims(r);
    if (r.end.rows.count === 0) {
      diag(`PACK-22 / starts effects not applied by the screen: rows=0 stoppedAt=${r.stoppedAt ? `${r.stoppedAt.cause}@${r.stoppedAt.after}` : 'none'}`);
      assert.equal(r.completed, false, 'cannot complete without rows');
    } else {
      assert.equal(r.stoppedAt, null);
      assert.equal(r.completed, true);
    }
  });
}

test('C: 道具の対照。入力欄・style 属性・ボタンを足すと測った値が増える（CSP あり）', { timeout: 120000 }, async () => {
  const { dir: copy, cleanup } = prepareDelivery({ webDir, irText: irShown });
  try {
    await withCspMode({ dir: copy, csp: true }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
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
        assert.equal(after.inputArea.absent['draft-input'], before.inputArea.absent['draft-input'] + 1);
        assert.equal(after.inputArea.absent.textarea, before.inputArea.absent.textarea + 2);
        assert.equal(after.inputArea.absent.select, before.inputArea.absent.select + 1);
        // visibleInputs は input と textarea だけを数える（select は含めない）。足した textarea 2 個の分だけ増える。
        assert.equal(after.inputArea.visibleInputs.length, before.inputArea.visibleInputs.length + 2);
        assert.equal(after.inlineStyle.attrs, before.inlineStyle.attrs + 1);
        assert.equal(after.rightButtons.length, before.rightButtons.length + 1);
      } finally {
        await context.close();
      }
    }));
  } finally {
    cleanup();
  }
});
