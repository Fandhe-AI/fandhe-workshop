// 設定ファイル（tools/browser/choice-journey/*.json）の読み込み・検証・期待との照合（choice-journey-config.mjs）の試験
// （TASK-748 2/2。SECOND-4・SECOND-7）。ブラウザ・ビルド不要（node:test だけ）。記録は docs/testing.md 5.437。
//
//   L  同梱の設定 8 本が loadConfig を通り、名前が一意
//   N  負の対照。形・値の誤りは ConfigError（未知の欄・format・name・base・source・パスの封じ込め・操作の種類・status・decision・edits）
//   E  applyEdits。from が 0 回・2 回出る書き換えと、写しの外・シンボリックリンクの対象を拒否し、元は書き換えない
//   J  judge の表駆動。合う結果は空、期待を 1 つだけ外した結果は理由がちょうど 1 件
//   (TASK-749 2/3) N に左の領域の欄（visibleViews・leftSwitchAbsent・rowPaths・marks）の不正な形、J に左の領域の照合を足した
//   S  静的な検査。設定に書かれた教材固有の文字列が、道具の 3 ファイルのソースに 1 件も出ない
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  loadConfig, parseConfig, applyEdits, judge, ConfigError,
} from './choice-journey-config.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const CONFIG_DIR = path.join(HERE, 'choice-journey');
const FILES = fs.readdirSync(CONFIG_DIR).filter((f) => f.endsWith('.json')).sort();

const base = () => ({
  format: 1,
  name: 'sample',
  source: { pack: 'p' },
  base: '/sample/',
  initial: { uid: 'a/b', status: 'entered' },
  sequence: [{ kind: 'choice', id: 'x', expect: { uid: 'a/b', status: 'accepted' } }],
  expect: { completed: true },
});
const clone = (o) => JSON.parse(JSON.stringify(o));

test('L: 同梱の設定が 8 本あり、すべて loadConfig を通り名前が一意', () => {
  assert.ok(FILES.length >= 8, `config files: ${FILES.length}`);
  const names = FILES.map((f) => loadConfig(path.join(CONFIG_DIR, f)).name);
  assert.equal(new Set(names).size, names.length);
  for (const f of FILES) assert.equal(loadConfig(path.join(CONFIG_DIR, f)).name, f.replace(/\.json$/, ''), 'name matches file name');
});

test('L: 読めない・JSON でない設定は ConfigError', () => {
  assert.throws(() => loadConfig(path.join(CONFIG_DIR, 'absent.json')), ConfigError);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-cj-cfg-'));
  try {
    const f = path.join(dir, 'bad.json');
    fs.writeFileSync(f, '{ not json');
    assert.throws(() => loadConfig(f), ConfigError);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

const BAD = [
  ['format 違い', (c) => { c.format = 2; }],
  ['未知の欄（最上位）', (c) => { c.nme = 'x'; }],
  ['name の不正文字', (c) => { c.name = 'Bad Name'; }],
  ['name が長い', (c) => { c.name = 'a'.repeat(65); }],
  ['base の形', (c) => { c.base = 'sample/'; }],
  ['base に連続した /', (c) => { c.base = '/a/b/'; }],
  ['source の両方指定', (c) => { c.source = { pack: 'p', ir: 'i.json' }; }],
  ['source が空', (c) => { c.source = {}; }],
  ['source.ir に edits', (c) => { c.source = { ir: 'i.json', edits: [] }; }],
  ['source.pack が絶対パス', (c) => { c.source = { pack: '/etc' }; }],
  ['source.ir が絶対パス', (c) => { c.source = { ir: '/etc/passwd' }; }],
  ['edits[].file が .. を含む', (c) => { c.source = { pack: 'p', edits: [{ file: '../x', from: 'a', to: 'b' }] }; }],
  ['edits[].file が絶対パス', (c) => { c.source = { pack: 'p', edits: [{ file: '/x', from: 'a', to: 'b' }] }; }],
  ['edits[].from が空', (c) => { c.source = { pack: 'p', edits: [{ file: 'x', from: '', to: 'b' }] }; }],
  ['edits の未知の欄', (c) => { c.source = { pack: 'p', edits: [{ file: 'x', from: 'a', to: 'b', all: true }] }; }],
  ['未知の kind', (c) => { c.sequence[0].kind = 'type'; }],
  ['sequence が空', (c) => { c.sequence = []; }],
  ['choice に path', (c) => { c.sequence[0].path = 'x'; }],
  ['choice の id が不正', (c) => { c.sequence[0].id = 'a"]b'; }],
  ['row の path に引用符', (c) => { c.sequence[0] = { kind: 'row', path: 'a"b', expect: c.sequence[0].expect }; }],
  ['未知の status', (c) => { c.sequence[0].expect.status = 'done'; }],
  ['initial の status が不正', (c) => { c.initial.status = 'x'; }],
  ['未知の decision', (c) => { c.sequence[0] = { kind: 'decide', decision: 'approve-all', expect: c.sequence[0].expect }; }],
  ['expect が操作ごとに無い', (c) => { delete c.sequence[0].expect; }],
  ['expect.completed が真偽でない', (c) => { c.expect.completed = 'yes'; }],
  ['stoppedAfter が completed true と併用', (c) => { c.expect.stoppedAfter = 'a/b:entered'; }],
  ['stoppedAfter の形', (c) => { c.expect = { completed: false, stoppedAfter: 'a/b' }; }],
  ['reviewAchieved が負', (c) => { c.expect.reviewAchieved = -1; }],
  ['expect の未知の欄', (c) => { c.expect.externalMax = 5; }],
  ['viewport の範囲', (c) => { c.viewport = { width: 10, height: 800 }; }],
  ['entryPorts が空', (c) => { c.expect.entryPorts = []; }],
  ['entryPorts の未知の部品', (c) => { c.expect.entryPorts = ['choice-btn', 'x-btn']; }],
  ['entryPorts の重複', (c) => { c.expect.entryPorts = ['choice-btn', 'choice-btn']; }],
  ['entryAtStart が配列でない', (c) => { c.expect.entryAtStart = 'choice-btn'; }],
  ['entryAtStart の未知の部品', (c) => { c.expect.entryAtStart = ['a"]b']; }],
  ['inputGuidesAbsent が真偽でない', (c) => { c.expect.inputGuidesAbsent = 1; }],
  ['fileInputs が負', (c) => { c.expect.fileInputs = -1; }],
  ['fileInputs が範囲外', (c) => { c.expect.fileInputs = 11; }],
  ['controls が連想配列でない', (c) => { c.expect.controls = ['stop']; }],
  ['controls の未知の名前', (c) => { c.expect.controls = { 'a"]b': true }; }],
  ['controls の値が真偽でない', (c) => { c.expect.controls = { stop: 'yes' }; }],
  ['visibleViews の未知のビュー', (c) => { c.expect.visibleViews = ['document', 'world']; }],
  ['visibleViews の重複', (c) => { c.expect.visibleViews = ['document', 'document']; }],
  ['visibleViews が配列でない', (c) => { c.expect.visibleViews = 'document'; }],
  ['leftSwitchAbsent が真偽でない', (c) => { c.expect.leftSwitchAbsent = 1; }],
  ['rowPaths の重複', (c) => { c.expect.rowPaths = ['r/a', 'r/a']; }],
  ['rowPaths の経路に引用符', (c) => { c.expect.rowPaths = ['r"a']; }],
  ['rowPaths が 65 件', (c) => { c.expect.rowPaths = Array.from({ length: 65 }, (_, i) => `r/${i}`); }],
  ['marks の未知の値', (c) => { c.expect.rowPaths = ['r/a']; c.sequence[0].marks = { 'r/a': 'none' }; }],
  ['marks の値の大文字', (c) => { c.expect.rowPaths = ['r/a']; c.sequence[0].marks = { 'r/a': 'CHANGE' }; }],
  ['marks の経路に引用符', (c) => { c.expect.rowPaths = ['r"a']; c.sequence[0].marks = { 'r"a': 'add' }; }],
  ['marks が 65 件', (c) => {
    const paths = Array.from({ length: 65 }, (_, i) => `r/${i}`);
    c.expect.rowPaths = paths;
    c.sequence[0].marks = Object.fromEntries(paths.map((x) => [x, 'add']));
  }],
  ['marks が連想配列でない', (c) => { c.expect.rowPaths = ['r/a']; c.sequence[0].marks = ['r/a']; }],
  ['rowPaths の無い marks', (c) => { c.sequence[0].marks = {}; }],
  ['rowPaths に無い経路の marks', (c) => { c.expect.rowPaths = ['r/a']; c.sequence[0].marks = { 'r/b': 'add' }; }],
  ['appearance が配列', (c) => { c.expect.appearance = []; }],
  ['appearance が空', (c) => { c.expect.appearance = {}; }],
  ['appearance の未知の欄', (c) => { c.expect.appearance = { fontSize: 18 }; }],
  ['appearance.theme が 4 値の外', (c) => { c.expect.appearance = { theme: 'dark' }; }],
  ['appearance.bodyPx が小数', (c) => { c.expect.appearance = { bodyPx: 17.5 }; }],
  ['appearance.bodyPx が範囲外', (c) => { c.expect.appearance = { bodyPx: 7 }; }],
  ['appearance.lineHeightRatio が文字列', (c) => { c.expect.appearance = { lineHeightRatio: '1.75' }; }],
  ['appearance.lineHeightRatio が範囲外', (c) => { c.expect.appearance = { lineHeightRatio: 3.5 }; }],
  ['appearance.inlineStyleZero が真偽でない', (c) => { c.expect.appearance = { inlineStyleZero: 1 }; }],
  ['narrowTabs が真偽でない', (c) => { c.expect.narrowTabs = 'yes'; }],
  ['narrowTabs が真で幅 1280', (c) => { c.expect.narrowTabs = true; }],
  ['narrowTabs が偽で幅 800', (c) => { c.viewport = { width: 800, height: 800 }; c.expect.narrowTabs = false; }],
  ['csp が真偽の列でない', (c) => { c.csp = ['on']; }],
];
for (const [label, mutate] of BAD) {
  test(`N: 不正な設定を拒否する（${label}）`, () => {
    const ok = parseConfig(base(), '/tmp');
    assert.equal(ok.name, 'sample', 'control: the base config is accepted');
    const c = clone(base());
    mutate(c);
    assert.throws(() => parseConfig(c, '/tmp'), ConfigError);
  });
}

test('N: visibleViews は ViewKind の data-view の 7 種だけを通し、実在しない名前を拒否する', () => {
  const real = ['file_tree', 'document', 'diff', 'task_tree', 'card_list', 'check_list', 'app_demo'];
  const withViews = (views) => { const c = clone(base()); c.expect.visibleViews = views; return c; };
  assert.deepEqual(parseConfig(withViews(real), "/tmp").expect.visibleViews, real);
  for (const k of real) assert.deepEqual(parseConfig(withViews([k]), "/tmp").expect.visibleViews, [k]);
  for (const k of ['tree', 'table', 'list', 'graph', 'timeline']) {
    assert.throws(() => parseConfig(withViews([k]), "/tmp"), ConfigError, k);
  }
});

test('N: 省略した欄の既定（csp 2 通り・viewport 1280x800）', () => {
  const c = parseConfig(base(), '/tmp');
  assert.deepEqual(c.csp, [true, false]);
  assert.deepEqual(c.viewport, { width: 1280, height: 800 });
});

function withTmp(fn) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-cj-edit-'));
  try {
    return fn(dir);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

test('E: applyEdits は from がちょうど 1 回のときだけ書き換え、$ の特殊な置換を解釈しない', () => {
  withTmp((dir) => {
    fs.writeFileSync(path.join(dir, 'a.yaml'), 'k: one\nm: two\n');
    applyEdits(dir, [{ file: 'a.yaml', from: 'one', to: "$& $1 $'" }]);
    assert.equal(fs.readFileSync(path.join(dir, 'a.yaml'), 'utf8'), "k: $& $1 $'\nm: two\n");
  });
});

test('E: applyEdits は from が 0 回・2 回以上のとき拒否する', () => {
  withTmp((dir) => {
    fs.writeFileSync(path.join(dir, 'a.yaml'), 'x x\n');
    assert.throws(() => applyEdits(dir, [{ file: 'a.yaml', from: 'x', to: 'y' }]), ConfigError, 'twice');
    assert.throws(() => applyEdits(dir, [{ file: 'a.yaml', from: 'zzz', to: 'y' }]), ConfigError, 'zero');
    assert.equal(fs.readFileSync(path.join(dir, 'a.yaml'), 'utf8'), 'x x\n', 'unchanged after refusal');
  });
});

test('E: applyEdits は写しの外・存在しないファイル・ディレクトリ・シンボリックリンクを拒否する', () => {
  withTmp((dir) => {
    const work = path.join(dir, 'work');
    const outside = path.join(dir, 'outside.txt');
    fs.mkdirSync(path.join(work, 'sub'), { recursive: true });
    fs.writeFileSync(outside, 'secret\n');
    fs.symlinkSync(outside, path.join(work, 'link.txt'));
    // 親ディレクトリ側のシンボリックリンク（最終要素は通常のファイル）
    fs.mkdirSync(path.join(dir, 'outdir'));
    fs.writeFileSync(path.join(dir, 'outdir', 'f.txt'), 'secret\n');
    fs.symlinkSync(path.join(dir, 'outdir'), path.join(work, 'dirlink'));
    for (const file of ['../outside.txt', 'absent.txt', 'sub', 'link.txt', 'dirlink/f.txt']) {
      assert.throws(() => applyEdits(work, [{ file, from: 'secret', to: 'x' }]), ConfigError, file);
    }
    assert.equal(fs.readFileSync(outside, 'utf8'), 'secret\n', 'outside file untouched');
    assert.equal(fs.readFileSync(path.join(dir, 'outdir', 'f.txt'), 'utf8'), 'secret\n', 'outside dir file untouched');
  });
});

// judge の入力。期待に合う結果（完走・達成 2 件）から、1 つの欄だけを外す。
const config = () => parseConfig({
  ...base(),
  expect: {
    completed: true,
    reviewAchieved: 2,
    nextChapter: 0,
    inputAreaAbsent: true,
    viewSwitchVisible: 0,
    entryPorts: ['choice-btn'],
    entryAtStart: ['choice-btn'],
    inputGuidesAbsent: true,
    fileInputs: 1,
    controls: { stop: true, export: true, import: true, hintContainer: true, hintOpen: false },
  },
}, '/tmp');
const part = (count, visible) => ({ count, visible });
const screen = () => ({
  parts: {
    'choice-btn': part(2, 2),
    'stop-btn': part(1, 1),
    'key-hint': part(0, 0),
    'op-guide': part(0, 0),
    'os-field': part(0, 0),
    'os-select': part(0, 0),
    'shell-export': part(1, 1),
    'shell-import': part(1, 0),
    'shell-hint': part(1, 0),
    'hint-open': part(0, 0),
  },
  notice: { count: 1, visible: 0, textLength: 0 },
  fileInputs: { visible: 1, inImport: 1 },
  inputArea: {
    absent: {
      textarea: 0, 'draft-input': 0, 'submit-input': 0, 'candidate-list': 0, 'candidate-btn': 0, select: 0,
    },
    visibleInputs: [],
  },
  viewSwitch: { visibleButtons: 0 },
});
const good = () => ({
  completed: true,
  stoppedAt: null,
  review: [{ step: 'a', achieved: 'true' }, { step: 'b', achieved: 'true' }],
  nextChapter: 0,
  start: screen(),
  end: screen(),
  safety: [],
  external: 0,
  pageErrors: 0,
  consoleErrors: 0,
  cspWarnings: 0,
  run: { raw: 0, mouse: 5 },
  irFetch: [200],
});

test('J: judge は期待に合う結果で空を返す', () => {
  assert.deepEqual(judge(config(), good()), []);
});

const OFF = [
  ['completed', (r) => { r.completed = false; r.stoppedAt = null; }, ['completed']],
  ['review 件数', (r) => { r.review[1].achieved = 'false'; }, ['reviewAchieved']],
  ['nextChapter', (r) => { r.nextChapter = 1; }, ['nextChapter']],
  ['入力の口（終了時）', (r) => { r.end.inputArea.absent.textarea = 1; }, ['inputAreaAbsent@end']],
  ['見える input（開始時）', (r) => { r.start.inputArea.visibleInputs = ['|input|text']; }, ['visibleInputs@start']],
  ['ビュー切り替え（終了時）', (r) => { r.end.viewSwitch.visibleButtons = 2; }, ['viewSwitchVisible@end']],
  ['入口に承認のボタン（終了時）', (r) => { r.end.parts['approval-btn'] = part(1, 1); }, ['entryPorts@end']],
  ['入口に入力欄（開始時）', (r) => { r.start.parts['draft-input'] = part(1, 1); }, ['entryPorts@start']],
  ['開始時に選択肢のボタンが無い', (r) => { r.start.parts['choice-btn'] = part(0, 0); }, ['entryAtStart']],
  ['キーの案内（終了時）', (r) => { r.end.parts['key-hint'] = part(1, 1); }, ['inputGuidesAbsent@end']],
  ['OS の選択（開始時）', (r) => { r.start.parts['os-select'] = part(1, 1); }, ['inputGuidesAbsent@start']],
  ['入力の注意書きに文字（終了時）', (r) => { r.end.notice.textLength = 5; }, ['inputNotice@end']],
  ['ファイル選択が 0 個（開始時）', (r) => { r.start.fileInputs = { visible: 0, inImport: 0 }; }, ['fileInputs@start']],
  ['ファイル選択が 2 個', (r) => { r.end.fileInputs = { visible: 2, inImport: 2 }; }, ['fileInputs@end']],
  ['ファイル選択が読み込みの外', (r) => { r.end.fileInputs = { visible: 1, inImport: 0 }; }, ['fileInputs@end']],
  ['停止のボタンが見えない', (r) => { r.end.parts['stop-btn'] = part(1, 0); }, ['controls.stop@end']],
  ['書き出しが無い', (r) => { r.start.parts['shell-export'] = part(0, 0); }, ['controls.export@start']],
  ['読み込みの入れ物が無い', (r) => { r.end.parts['shell-import'] = part(0, 0); }, ['controls.import@end']],
  ['ヒントを開くが出る', (r) => { r.end.parts['hint-open'] = part(1, 1); }, ['controls.hintOpen@end']],
  ['外部通信', (r) => { r.external = 1; }, ['external']],
  ['ページ内エラー', (r) => { r.pageErrors = 1; }, ['pageErrors']],
  ['コンソールエラー', (r) => { r.consoleErrors = 1; }, ['consoleErrors']],
  ['CSP の警告', (r) => { r.cspWarnings = 1; }, ['cspWarnings']],
  ['安全の計数', (r) => { r.safety = ['x']; }, ['safety']],
  ['ir の取得', (r) => { r.irFetch = [200, 200]; }, ['irFetch']],
  ['実マウス操作', (r) => { r.run = { raw: 0, mouse: 0 }; }, ['noMouseOps']],
  ['raw の click', (r) => { r.run = { raw: 1, mouse: 5 }; }, ['rawClicks']],
];
for (const [label, mutate, want] of OFF) {
  test(`J: judge は 1 欄だけ外れた結果で理由がちょうど ${want.length} 件（${label}）`, () => {
    const r = good();
    mutate(r);
    const why = judge(config(), r);
    // 「completed を外す」だけは、同時に review の件数に影響しないこと
    assert.deepEqual(why, want);
  });
}

// 左の領域（TASK-749 2/3）。1 操作の設定で、行 2 本（r/a・r/b）・文書ビューだけ・切り替えなし・r/a が【変更】の主張。
const leftConfig = () => parseConfig({
  ...base(),
  sequence: [{
    kind: 'choice', id: 'x', expect: { uid: 'a/b', status: 'accepted' }, marks: { 'r/a': 'change' },
  }],
  expect: {
    completed: true, visibleViews: ['document'], leftSwitchAbsent: true, rowPaths: ['r/a', 'r/b'],
  },
}, '/tmp');
const leftAt = (marks = {}) => ({
  settled: true,
  panes: { tablist: false, selected: null, left: true, right: true },
  views: { shown: ['document'], total: 7 },
  select: { count: 1, visible: 0, buttons: 0, visibleButtons: 0 },
  absent: { 'world-list': 0, 'view-tabs': 0, 'view-tab': 0 },
  rows: ['r/b', 'r/a'].map((p) => {
    const mark = marks[p] ?? '';
    return { path: p, mark, glyph: { '': '', change: '~', add: '+' }[mark], text: { '': '', change: '【変更】', add: '【追加】' }[mark] };
  }),
});
const goodLeft = () => ({ ...good(), left: { start: leftAt(), ops: [{ index: 0, ...leftAt({ 'r/a': 'change' }) }], end: leftAt() } });

test('J: judge は左の領域の期待に合う結果で空を返し、行の並びは主張しない', () => {
  assert.deepEqual(judge(leftConfig(), goodLeft()), []);
  const r = goodLeft();
  r.left.end.rows.reverse();
  assert.deepEqual(judge(leftConfig(), r), []);
  // 左の主張が無い設定は left が無くても通る（他の設定を壊さない）。
  assert.deepEqual(judge(config(), good()), []);
});

const LEFT_OFF = [
  ['left が無い', (r) => { delete r.left; }, ['left-missing']],
  ['ビューが 2 本見える（終了時）', (r) => { r.left.end.views.shown = ['document', 'list']; }, ['visibleViews@end']],
  ['見えるビューが無い（開始時）', (r) => { r.left.start.views.shown = []; }, ['visibleViews@start']],
  ['切り替えのボタンが 1 個（操作の後）', (r) => { r.left.ops[0].select.buttons = 1; r.left.ops[0].select.visibleButtons = 1; }, ['leftSwitch@op[0]']],
  ['切り替えの入れ物が見える（開始時）', (r) => { r.left.start.select.visible = 1; }, ['leftSwitch@start']],
  ['world-list が 1 件（終了時）', (r) => { r.left.end.absent['world-list'] = 1; }, ['leftSwitch@end']],
  ['行が 1 本足りない（終了時）', (r) => { r.left.end.rows.pop(); }, ['rowPaths@end']],
  ['行が 1 本余る（操作の後）', (r) => { r.left.ops[0].rows.push({ path: 'r/c', mark: '', glyph: '', text: '' }); }, ['rowPaths@op[0]']],
  ['行の経路が重複する（終了時）', (r) => { r.left.end.rows[0].path = 'r/a'; }, ['rowPaths@end']],
  ['印の値が違う（操作の後）', (r) => { r.left.ops[0].rows.find((x) => x.path === 'r/a').mark = 'add'; r.left.ops[0].rows.find((x) => x.path === 'r/a').glyph = '+'; r.left.ops[0].rows.find((x) => x.path === 'r/a').text = '【追加】'; }, ['marks@op[0]']],
  ['印の無いはずの行に印がある（操作の後）', (r) => { Object.assign(r.left.ops[0].rows.find((x) => x.path === 'r/b'), { mark: 'change', glyph: '~', text: '【変更】' }); }, ['marks@op[0]']],
  ['印の文字が空で data-mark が change（3 手段の不一致）', (r) => { r.left.ops[0].rows.find((x) => x.path === 'r/a').text = ''; }, ['markGlyphText@op[0]']],
  ['記号が違う（終了時）', (r) => { Object.assign(r.left.end.rows[0], { mark: 'add', glyph: '~', text: '【追加】' }); }, ['markGlyphText@end']],
  ['未知の data-mark（開始時）', (r) => { r.left.start.rows[0].mark = 'move'; }, ['markGlyphText@start']],
  ['左の描画が揃わない（操作の後）', (r) => { r.left.ops[0].settled = false; }, ['leftUnsettled@op[0]']],
];
for (const [label, mutate, want] of LEFT_OFF) {
  test(`J: judge は左の領域が 1 欄だけ外れた結果で理由がちょうど 1 件（${label}）`, () => {
    const r = goodLeft();
    mutate(r);
    assert.deepEqual(judge(leftConfig(), r), want);
  });
}

// 拒否の操作に marks を宣言した設定（観察が無ければ不合格、あれば照合される）。
const rejLeftConfig = () => {
  const c = leftConfig();
  c.sequence[0].expect = { uid: 'a/b', status: 'rejected' };
  return c;
};
test('J: 拒否の操作の marks は観察があれば照合され、観察が無ければ不合格', () => {
  assert.deepEqual(judge(rejLeftConfig(), goodLeft()).filter((x) => x.includes('marks')), []);
  const none = goodLeft();
  none.left.ops = [];
  assert.deepEqual(judge(rejLeftConfig(), none).filter((x) => x.includes('marks')), ['marksMissing@op[0]']);
  const wrong = goodLeft();
  wrong.left.ops[0] = { index: 0, ...leftAt() };
  assert.deepEqual(judge(rejLeftConfig(), wrong).filter((x) => x.includes('marks')), ['marks@op[0]']);
});

test('J: 完走しない期待（stoppedAfter）は止まった位置が一致するときだけ合格し、完走すると不合格', () => {
  const c = parseConfig({ ...base(), expect: { completed: false, stoppedAfter: 'a/b:entered' } }, '/tmp');
  const r = good();
  r.completed = false;
  r.stoppedAt = { index: 1, kind: 'row', cause: 'no-target', after: 'a/b:entered' };
  assert.deepEqual(judge(c, r), []);
  r.stoppedAt.after = 'a/b:accepted';
  assert.deepEqual(judge(c, r), ['stoppedAfter']);
  assert.deepEqual(judge(c, good()), ['completed', 'stoppedAfter']);
});

test('S: 設定に書かれた教材固有の文字列が、道具の 3 ファイルのソースに 1 件も出ない', () => {
  const sources = ['choice-journey.mjs', 'choice-journey-config.mjs', 'choice-journey-run.mjs']
    .map((f) => [f, fs.readFileSync(path.join(HERE, f), 'utf8')]);
  const needles = new Set();
  for (const f of FILES) {
    const c = JSON.parse(fs.readFileSync(path.join(CONFIG_DIR, f), 'utf8'));
    for (const op of c.sequence) {
      for (const v of [op.id, op.path, op.expect.uid]) if (v) needles.add(v);
    }
    needles.add(c.initial.uid);
    for (const e of c.source.edits ?? []) { needles.add(e.from.trim()); needles.add(e.to.trim()); }
  }
  assert.ok(needles.size >= 10, `needles: ${needles.size}`);
  for (const [file, src] of sources) {
    for (const n of needles) assert.ok(!src.includes(n), `${file} must not contain a pack-specific value`);
  }
});

// 拒否の場面（TASK-750。SECOND-12 の画面の部分）。記録は docs/testing.md 5.446。
const rejBase = () => {
  const c = base();
  c.sequence = [
    { kind: 'choice', id: 'x', expect: { uid: 'a/b', status: 'rejected' }, scene: 1, rejection: 'rj-a', category: 'not_in_this_step' },
    { kind: 'choice', id: 'y', expect: { uid: 'a/b', status: 'accepted' } },
    { kind: 'choice', id: 'z', expect: { uid: 'a/b', status: 'rejected' }, scene: 6, rejection: 'rj-b', category: 'bad_arg_shape' },
  ];
  c.expect = {
    completed: true, rejectScenes: [1, 6], rejectCategories: ['bad_arg_shape', 'not_in_this_step'], excluded: [{ scene: 5, reason: 'engine-when-not-evaluated' }],
  };
  return c;
};

test('R: 拒否の宣言の正の例は通り、集合は昇順に整えられる', () => {
  const c = rejBase();
  c.expect.rejectCategories = ['not_in_this_step', 'bad_arg_shape'];
  const p = parseConfig(c, '/tmp');
  assert.deepEqual(p.expect.rejectScenes, [1, 6]);
  assert.deepEqual(p.expect.rejectCategories, ['bad_arg_shape', 'not_in_this_step']);
});

const BAD_REJ = [
  ['範囲外の場面', (c) => { c.sequence[0].scene = 9; }],
  ['場面が数でない', (c) => { c.sequence[0].scene = '1'; }],
  ['未知の分類', (c) => { c.sequence[0].category = 'other'; }],
  ['拒否に場面が無い', (c) => { delete c.sequence[0].scene; }],
  ['拒否に拒否の ID が無い', (c) => { delete c.sequence[0].rejection; }],
  ['拒否の ID に不正文字', (c) => { c.sequence[0].rejection = 'a b'; }],
  ['拒否でない操作に場面', (c) => { c.sequence[1].scene = 2; }],
  ['拒否でない操作に分類', (c) => { c.sequence[1].category = 'wrong_order'; }],
  ['initial に rejected', (c) => { c.initial.status = 'rejected'; }],
  ['場面の重複', (c) => { c.sequence[2].scene = 1; }],
  ['rejectScenes が操作の列と違う', (c) => { c.expect.rejectScenes = [1, 7]; }],
  ['rejectScenes が無い', (c) => { delete c.expect.rejectScenes; }],
  ['rejectScenes の重複', (c) => { c.expect.rejectScenes = [1, 1]; }],
  ['未知の分類（rejectCategories）', (c) => { c.expect.rejectCategories = ['x']; }],
  ['除外が主張と重なる', (c) => { c.expect.excluded = [{ scene: 1, reason: 'r' }]; }],
  ['除外の場面が範囲外', (c) => { c.expect.excluded = [{ scene: 0, reason: 'r' }]; }],
  ['除外の理由が不正文字', (c) => { c.expect.excluded = [{ scene: 5, reason: 'A B' }]; }],
  ['除外の未知の欄', (c) => { c.expect.excluded = [{ scene: 5, reason: 'r', note: 'x' }]; }],
  ['除外の重複', (c) => { c.expect.excluded = [{ scene: 5, reason: 'r' }, { scene: 5, reason: 's' }]; }],
];
for (const [name, mutate] of BAD_REJ) {
  test(`R: 負の対照。${name}は ConfigError`, () => {
    const c = rejBase();
    mutate(c);
    assert.throws(() => parseConfig(c, '/tmp'), ConfigError);
  });
}

test('R: judge は拒否の記録を 1 か所ずつ外すと欄名つきの理由を返す', () => {
  const cfg = parseConfig(rejBase(), '/tmp');
  const rec = (index, scene, category) => ({
    index, scene, category, irCategory: category, reasonOk: true, nextOk: true, rowsUnchanged: true, stepUnchanged: true, liveResponse: { count: 0, hasText: false },
  });
  const good = () => ({
    completed: true, stoppedAt: null, review: [], safety: [], external: 0, pageErrors: 0, consoleErrors: 0, cspWarnings: 0, run: { raw: 0, mouse: 3 }, irFetch: [200], nextChapter: 0,
    start: { inputArea: { absent: {}, visibleInputs: [] }, viewSwitch: { visibleButtons: 0 } }, end: { inputArea: { absent: {}, visibleInputs: [] }, viewSwitch: { visibleButtons: 0 } },
    rejections: [rec(0, 1, 'not_in_this_step'), rec(2, 6, 'bad_arg_shape')],
  });
  const strip = (why) => why.filter((x) => x.startsWith('reject'));
  assert.deepEqual(strip(judge(cfg, good())), []);
  const cases = [
    ['missing', (r) => { r.rejections.pop(); }, ['reject[2].missing', 'rejectScenes', 'rejectCategories']],
    ['category', (r) => { r.rejections[0].category = 'wrong_order'; }, ['reject[0].category', 'rejectScenes', 'rejectCategories']],
    ['irCategory', (r) => { r.rejections[0].irCategory = 'wrong_order'; }, ['reject[0].category', 'rejectScenes']],
    ['text(reason)', (r) => { r.rejections[0].reasonOk = false; }, ['reject[0].text', 'rejectScenes']],
    ['text(next)', (r) => { r.rejections[0].nextOk = false; }, ['reject[0].text', 'rejectScenes']],
    ['rows', (r) => { r.rejections[1].rowsUnchanged = false; }, ['reject[2].rows', 'rejectScenes']],
    ['step', (r) => { r.rejections[1].stepUnchanged = false; }, ['reject[2].step', 'rejectScenes']],
  ];
  for (const [name, mutate, want] of cases) {
    const r = good();
    mutate(r);
    assert.deepEqual(strip(judge(cfg, r)), want, name);
  }
});

test('R: 拒否の操作が無い設定の judge は拒否の理由を返さない', () => {
  const cfg = parseConfig(base(), '/tmp');
  const r = { completed: true, stoppedAt: null, safety: [], external: 0, pageErrors: 0, consoleErrors: 0, cspWarnings: 0, run: { raw: 0, mouse: 1 }, irFetch: [200], start: {}, end: {} };
  assert.deepEqual(judge(cfg, r).filter((x) => x.startsWith('reject')), []);
});

// 外観と幅 960px 未満のタブ（TASK-749 3/3。SECOND-10）。
const tabConfig = () => parseConfig({
  ...base(),
  viewport: { width: 800, height: 800 },
  expect: {
    completed: true, inputAreaAbsent: true, entryPorts: ['choice-btn'], inputGuidesAbsent: true, narrowTabs: true, appearance: { inlineStyleZero: true },
  },
}, '/tmp');
const paneScreen = (selected) => ({
  ...screen(),
  inlineStyle: { attrs: 0, elements: 0, elementsTotal: 0 },
  theme: { values: [], main: null },
  font: { body: { px: 16, lineHeightRatio: 1.2 } },
  panes: { tablist: true, selected, left: selected === 'artifacts', right: selected === 'ops' },
});
const goodTabs = () => ({ ...good(), start: paneScreen('ops'), end: paneScreen('ops'), artifactsTab: { start: paneScreen('artifacts'), end: paneScreen('artifacts') } });

test('T: judge は幅 800px のタブの期待に合う結果で空を返す', () => {
  assert.deepEqual(judge(tabConfig(), goodTabs()), []);
});

const TAB_OFF = [
  ['属性の style（開始時）', (r) => { r.start.inlineStyle.attrs = 1; }, ['inlineStyle@start']],
  ['中身のある style 要素（終了時）', (r) => { r.end.inlineStyle.elements = 1; }, ['inlineStyle@end']],
  ['成果物タブの style 属性', (r) => { r.artifactsTab.end.inlineStyle.attrs = 1; }, ['inlineStyle@artifacts-end']],
  ['tablist が見えない（開始時）', (r) => { r.start.panes.tablist = false; }, ['tabs@start']],
  ['操作タブで左が見える（終了時）', (r) => { r.end.panes.left = true; }, ['tabs@end']],
  ['成果物タブで右が見える', (r) => { r.artifactsTab.end.panes.right = true; }, ['tabs@artifacts-end']],
  ['成果物タブの選択が違う', (r) => { r.artifactsTab.start.panes.selected = 'ops'; }, ['tabs@artifacts-start']],
  ['成果物タブの測定が無い', (r) => { r.artifactsTab = null; }, ['artifactsTab-missing']],
  ['成果物タブの終了が無い', (r) => { r.artifactsTab.end = null; }, ['artifactsTab-missing@end']],
  ['成果物タブに入力欄', (r) => { r.artifactsTab.start.inputArea.absent.textarea = 1; }, ['inputAreaAbsent@artifacts-start']],
  ['成果物タブに見える入力', (r) => { r.artifactsTab.end.inputArea.visibleInputs = ['|input|text']; }, ['visibleInputs@artifacts-end']],
  ['成果物タブに案内', (r) => { r.artifactsTab.start.parts['key-hint'] = part(1, 1); }, ['inputGuidesAbsent@artifacts-start']],
  ['成果物タブに承認のボタン', (r) => { r.artifactsTab.end.parts['approval-btn'] = part(1, 1); }, ['entryPorts@artifacts-end']],
];
for (const [label, mutate, want] of TAB_OFF) {
  test(`T: judge は幅 800px のタブで 1 欄だけ外れた結果で理由が ${want.length} 件（${label}）`, () => {
    const r = goodTabs();
    mutate(r);
    assert.deepEqual(judge(tabConfig(), r), want);
  });
}

test('T: 外観の theme・bodyPx・lineHeightRatio は宣言したときだけ照合され、測った値が違えば欄名の理由を返す', () => {
  const cfg = (a) => parseConfig({ ...base(), expect: { completed: true, appearance: a } }, '/tmp');
  const r = () => ({ ...good(), start: paneScreen('ops'), end: paneScreen('ops') });
  assert.deepEqual(judge(cfg({ inlineStyleZero: true }), r()), []);
  assert.deepEqual(judge(cfg({ theme: 'reading', bodyPx: 18, lineHeightRatio: 1.75 }), r()).sort(),
    ['bodyPx@end', 'bodyPx@start', 'lineHeight@end', 'lineHeight@start', 'theme@end', 'theme@start']);
  const ok = r();
  for (const m of [ok.start, ok.end]) { m.theme.main = 'reading'; m.font.body = { px: 18, lineHeightRatio: 1.75 }; }
  assert.deepEqual(judge(cfg({ theme: 'reading', bodyPx: 18, lineHeightRatio: 1.75 }), ok), []);
});

test('T: narrowTabs が偽の設定は tablist が見えると不合格', () => {
  const c = parseConfig({ ...base(), expect: { completed: true, narrowTabs: false } }, '/tmp');
  const r = good();
  r.start = screen();
  r.end = screen();
  r.start.panes = { tablist: false };
  r.end.panes = { tablist: false };
  assert.deepEqual(judge(c, r), []);
  r.end.panes = { tablist: true };
  assert.deepEqual(judge(c, r), ['tabs@end']);
});

test('T: 左の領域が見えない時点（操作タブのまま測った）は leftHidden で不合格', () => {
  const r = goodLeft();
  r.left.ops[0].panes.left = false;
  assert.deepEqual(judge(leftConfig(), r), ['leftHidden@op[0]']);
});

test('T: 800px 版と 1280px 版の対の設定は、名前・幅・narrowTabs 以外が同じ', () => {
  const load = (n) => loadConfig(path.join(CONFIG_DIR, `${n}.json`));
  for (const n of ['review-request', 'review-request-derived']) {
    const a = load(n);
    const b = load(`${n}-800`);
    assert.equal(a.expect.narrowTabs, false);
    assert.equal(b.expect.narrowTabs, true);
    assert.equal(b.viewport.width, 800);
    const strip = (c) => { const x = clone(c); delete x.name; delete x.viewport; delete x.expect.narrowTabs; return x; };
    assert.deepEqual(strip(b), strip(a));
  }
});
