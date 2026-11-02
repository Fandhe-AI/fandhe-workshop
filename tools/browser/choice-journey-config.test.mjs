// 設定ファイル（tools/browser/choice-journey/*.json）の読み込み・検証・期待との照合（choice-journey-config.mjs）の試験
// （TASK-748 2/2。SECOND-4・SECOND-7）。ブラウザ・ビルド不要（node:test だけ）。記録は docs/testing.md 5.437。
//
//   L  同梱の設定 3 本が loadConfig を通り、名前が一意
//   N  負の対照。形・値の誤りは ConfigError（未知の欄・format・name・base・source・パスの封じ込め・操作の種類・status・decision・edits）
//   E  applyEdits。from が 0 回・2 回出る書き換えと、写しの外・シンボリックリンクの対象を拒否し、元は書き換えない
//   J  judge の表駆動。合う結果は空、期待を 1 つだけ外した結果は理由がちょうど 1 件
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

test('L: 同梱の設定が 3 本あり、すべて loadConfig を通り名前が一意', () => {
  assert.ok(FILES.length >= 3, `config files: ${FILES.length}`);
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
    for (const file of ['../outside.txt', 'absent.txt', 'sub', 'link.txt']) {
      assert.throws(() => applyEdits(work, [{ file, from: 'secret', to: 'x' }]), ConfigError, file);
    }
    assert.equal(fs.readFileSync(outside, 'utf8'), 'secret\n', 'outside file untouched');
  });
});

// judge の入力。期待に合う結果（完走・達成 2 件）から、1 つの欄だけを外す。
const config = () => parseConfig({
  ...base(),
  expect: {
    completed: true, reviewAchieved: 2, nextChapter: 0, inputAreaAbsent: true, viewSwitchVisible: 0,
  },
}, '/tmp');
const screen = () => ({
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
