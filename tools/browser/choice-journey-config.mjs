// 入力欄のない教材の完走の道具（choice-journey.mjs）に渡す、教材固有の値の設定ファイル（JSON。版 1）の読み込み・検証と、
// 測った結果と設定の期待との照合（TASK-748 2/2。SECOND-4・SECOND-7）。
//
// 役割: 道具（choice-journey.mjs）は教材固有の値を持たず、合否も持たない。教材ごとの違い（操作の列・各操作の後の期待・派生版を作る書き換え・
// 結果の期待）はこの設定ファイルに書き、差し替えるだけで別の教材にも同じ道具を使えるようにする。本モジュールは
// 呼び出し元（choice-journey-run.mjs の CLI と choice-journey.test.mjs）から使われ、設定を未信頼の入力として扱って
// 形を厳しく検証し（fail-closed。未知の欄・範囲外の値はすべて ConfigError）、期待との照合（judge）の理由は欄名だけで返す。
// 設定の置き場: tools/browser/choice-journey/<name>.json。形式の表は docs/testing.md 5.437。外観（appearance）と幅 960px 未満のタブ（narrowTabs）は TASK-749 3/3（5.449）。左の領域（visibleViews・leftSwitchAbsent・rowPaths・操作ごとの marks）は TASK-749 2/3（5.447）。
// 拒否の場面の宣言（操作の scene・rejection・category と expect の rejectScenes・rejectCategories・excluded）の形と、観察との照合 judgeRejections もここにある（TASK-750。記録は 5.446）。
// 出力に教材の文面・入力値・パスの全体を出さない（ログの偽装と情報の流出を避ける）。
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import {
  compilePack, ABSENT, PORTS, GUIDES, CONTROLS, LEFT_ABSENT, MARK_TABLE,
} from './choice-journey.mjs';

/** 設定の形・値が不正なとき。CLI は終了コード 2（誤用）に対応づける。 */
export class ConfigError extends Error {}

// 見える箱を持たない入れ物（`shell-import` は label と input を包み、`shell-hint` は段が無いと空）。controls は DOM にあることで判定する。
const CONTAINERS = ['import', 'hintContainer'];
const STATUSES = ['entered', 'accepted', 'achieved'];
// 操作の期待にだけ許す「拒否された（Step は動かない）」。開いた直後の initial には使えない。
const OP_STATUSES = [...STATUSES, 'rejected'];
// docs/spec の拒否の分類 8 値（IR の rejections[].category と同じ閉じた一覧）。
const CATEGORIES = ['unknown_command', 'missing_arg', 'bad_arg_shape', 'not_in_this_step', 'wrong_order', 'busy', 'not_supported', 'free_text_unjudged'];
const SCENES = [1, 2, 3, 4, 5, 6, 7, 8];
// crates/web/src/terminal_area.rs の DECISION_VALUES と同じ 3 値。
const DECISIONS = ['approve', 'reject', 'revise'];
const KINDS = ['choice', 'row', 'decide', 'next'];
const MAX_SEQUENCE = 500;
const MAX_EDITS = 32;
// 文書ビューを含む 7 種（crates/web/src/artifact_view.rs の ViewKind::as_str と同じ閉じた一覧。data-view の値）。印の値は MARK_TABLE の add・remove・change（印なしは書かず、marks に載せない行が印なし）。
const VIEW_KINDS = ['file_tree', 'document', 'diff', 'task_tree', 'card_list', 'check_list', 'app_demo'];
const MARK_VALUES = ['add', 'remove', 'change'];
const MAX_ROWS = 64;
// contract::appearance::Theme と同じ 4 値の閉じた一覧（data-theme の値）。
const THEMES = ['standard', 'mono', 'high_contrast', 'reading'];
// 幅がこの値未満なら tablist が出る（crates/web/static/app.css の 960px の境目）。
const TAB_BREAKPOINT = 960;

const fail = (where, msg) => { throw new ConfigError(`${where}: ${msg}`); };
const isObj = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);

/** 許可した欄だけを持つこと。綴りの誤りを黙って通さない。 */
function onlyKeys(obj, allowed, where) {
  for (const k of Object.keys(obj)) if (!allowed.includes(k)) fail(where, `unknown field ${JSON.stringify(k.slice(0, 40))}`);
}

function str(v, where, re) {
  if (typeof v !== 'string' || !re.test(v)) fail(where, 'invalid string');
  return v;
}

function bool(v, where) {
  if (typeof v !== 'boolean') fail(where, 'must be boolean');
  return v;
}

function count(v, where) {
  if (!Number.isInteger(v) || v < 0 || v > 100000) fail(where, 'must be a non-negative integer');
  return v;
}

// 属性セレクタの文脈を壊さない形に限る（ページ内で評価する関数は固定で、設定の文字列から組み立てない）。
const UID_RE = /^[A-Za-z0-9_/-]{1,100}$/;
const ID_RE = /^[A-Za-z0-9_-]{1,100}$/;
const PATH_RE = /^[^"'\\\n\r\0]{1,200}$/;

function pair(v, where, statuses = STATUSES) {
  if (!isObj(v)) fail(where, 'must be an object');
  onlyKeys(v, ['uid', 'status'], where);
  return { uid: str(v.uid, `${where}.uid`, UID_RE), status: str(v.status, `${where}.status`, new RegExp(`^(${statuses.join('|')})$`)) };
}

/** 設定のディレクトリからの相対パス。絶対パスと NUL は拒否する。inside が真なら `..` も拒否する。 */
function relPath(v, where, { inside }) {
  if (typeof v !== 'string' || v === '' || v.length > 300 || v.includes('\0')) fail(where, 'invalid path');
  if (path.isAbsolute(v)) fail(where, 'absolute path is not allowed');
  if (inside && v.split(/[\\/]+/).includes('..')) fail(where, 'path must stay inside');
  return v;
}

function parseEdits(v, where) {
  if (!Array.isArray(v) || v.length > MAX_EDITS) fail(where, 'must be an array');
  return v.map((e, i) => {
    const w = `${where}[${i}]`;
    if (!isObj(e)) fail(w, 'must be an object');
    onlyKeys(e, ['file', 'from', 'to'], w);
    for (const k of ['from', 'to']) if (typeof e[k] !== 'string' || e[k].length > 2000) fail(`${w}.${k}`, 'must be a string');
    if (e.from === '') fail(`${w}.from`, 'must not be empty');
    return { file: relPath(e.file, `${w}.file`, { inside: true }), from: e.from, to: e.to };
  });
}

function parseSource(v, where) {
  if (!isObj(v)) fail(where, 'must be an object');
  onlyKeys(v, ['pack', 'validate', 'edits', 'ir'], where);
  const hasPack = v.pack !== undefined;
  const hasIr = v.ir !== undefined;
  if (hasPack === hasIr) fail(where, 'exactly one of pack and ir is required');
  if (hasIr) {
    if (v.validate !== undefined || v.edits !== undefined) fail(where, 'validate and edits need source.pack');
    return { ir: relPath(v.ir, `${where}.ir`, { inside: false }) };
  }
  return {
    pack: relPath(v.pack, `${where}.pack`, { inside: false }),
    validate: v.validate === undefined ? false : bool(v.validate, `${where}.validate`),
    edits: v.edits === undefined ? [] : parseEdits(v.edits, `${where}.edits`),
  };
}

function parseSequence(v, where) {
  if (!Array.isArray(v) || v.length < 1 || v.length > MAX_SEQUENCE) fail(where, 'must be a non-empty array');
  return v.map((op, i) => {
    const w = `${where}[${i}]`;
    if (!isObj(op)) fail(w, 'must be an object');
    const kind = str(op.kind, `${w}.kind`, new RegExp(`^(${KINDS.join('|')})$`));
    const extra = { choice: 'id', row: 'path', decide: 'decision', next: null }[kind];
    onlyKeys(op, ['kind', 'expect', 'marks', 'texts', 'scene', 'rejection', 'category', ...(extra ? [extra] : [])], w);
    const out = { kind, expect: pair(op.expect, `${w}.expect`, OP_STATUSES) };
    if (op.marks !== undefined) out.marks = parseMarks(op.marks, `${w}.marks`);
    if (op.texts !== undefined) out.texts = parseTexts(op.texts, `${w}.texts`);
    // 拒否の期待の操作だけが場面・拒否の ID・分類を持つ（3 欄とも必須）。それ以外の操作に書くと誤りとして拒否する。
    const rejected = out.expect.status === 'rejected';
    for (const k of ['scene', 'rejection', 'category']) if ((op[k] !== undefined) !== rejected) fail(`${w}.${k}`, rejected ? 'required with status rejected' : 'only with status rejected');
    if (rejected) {
      if (!SCENES.includes(op.scene)) fail(`${w}.scene`, 'must be an integer from 1 to 8');
      out.scene = op.scene;
      out.rejection = str(op.rejection, `${w}.rejection`, ID_RE);
      out.category = str(op.category, `${w}.category`, new RegExp(`^(${CATEGORIES.join('|')})$`));
    }
    if (kind === 'choice') out.id = str(op.id, `${w}.id`, ID_RE);
    if (kind === 'row') out.path = str(op.path, `${w}.path`, PATH_RE);
    if (kind === 'decide') out.decision = str(op.decision, `${w}.decision`, new RegExp(`^(${DECISIONS.join('|')})$`));
    return out;
  });
}

const MAX_TEXTS_PER_PATH = 16;
const TEXT_MAX = 200;

/**
 * 行の文字の期待（経路 → 含む文字の一覧。Issue #2052・TASK-799・D-100）。測る側が空白を詰めるので、詰めていない期待は
 * 決して合わない誤りとして拒否する。空の宣言は何も主張しないので拒否する。エラーに期待の文字は出さない。
 */
function parseTexts(v, where) {
  if (!isObj(v)) fail(where, 'must be an object');
  const keys = Object.keys(v);
  if (keys.length < 1 || keys.length > MAX_ROWS) fail(where, 'must have 1 to max entries');
  return Object.fromEntries(keys.map((k) => {
    str(k, `${where} key`, PATH_RE);
    const w = `${where}[${JSON.stringify(k.slice(0, 40))}]`;
    const a = v[k];
    return [k, textList(a, w)];
  }));
}

/** 文字の一覧（1〜MAX_TEXTS_PER_PATH 件・各 1〜200 文字・制御文字なし・空白を詰めた形・重複なし）の検証。parseTexts と expect.screenTextAbsent が共用する。 */
function textList(a, w) {
  if (!Array.isArray(a) || a.length < 1 || a.length > MAX_TEXTS_PER_PATH) fail(w, 'must be a non-empty array of limited size');
  a.forEach((t, j) => {
    if (typeof t !== 'string' || t.length < 1 || t.length > TEXT_MAX) fail(`${w}[${j}]`, 'must be a string of 1 to 200 characters');
    // eslint-disable-next-line no-control-regex
    if (/[\u0000-\u001f\u007f]/.test(t)) fail(`${w}[${j}]`, 'must not contain control characters');
    if (t !== t.replace(/\s+/g, ' ').trim()) fail(`${w}[${j}]`, 'must have collapsed whitespace and no edge whitespace');
  });
  if (new Set(a).size !== a.length) fail(w, 'must not repeat a text');
  return [...a];
}

/** 操作の後に左の行が持つ変化の印（経路 → add | remove | change）。載せない行は印なしの主張になる。`{}` は「印なし」。 */
function parseMarks(v, where) {
  if (!isObj(v)) fail(where, 'must be an object');
  const keys = Object.keys(v);
  if (keys.length > MAX_ROWS) fail(where, 'too many entries');
  return Object.fromEntries(keys.map((k) => {
    str(k, `${where} key`, PATH_RE);
    return [k, str(v[k], `${where}[${JSON.stringify(k.slice(0, 40))}]`, new RegExp(`^(${MARK_VALUES.join('|')})$`))];
  }));
}

/** 経路の集合（重複なし）。 */
function pathList(v, where) {
  if (!Array.isArray(v) || v.length > MAX_ROWS) fail(where, 'must be an array');
  const seen = new Set();
  for (const p of v) {
    str(p, where, PATH_RE);
    if (seen.has(p)) fail(where, 'duplicate path');
    seen.add(p);
  }
  return [...v];
}

/** 整数の集合（昇順・重複なし）の配列。 */
function intSet(v, where, allowed) {
  if (!Array.isArray(v) || v.length > allowed.length || !v.every((n) => allowed.includes(n)) || new Set(v).size !== v.length) fail(where, 'must be a unique list of allowed values');
  return [...v].sort((a, b) => a - b);
}

/**
 * 外観の主張（TASK-749 3/3。SECOND-10）。theme は contract::appearance::Theme と同じ 4 値の閉じた一覧、数値は範囲を検査する。
 * 製品の画面は data-theme をまだ付けない（docs/known-gaps.md の PACK-21）ので、theme・bodyPx・lineHeightRatio は結線後に書く欄。
 */
function parseAppearance(v, where) {
  if (!isObj(v)) fail(where, 'must be an object');
  onlyKeys(v, ['theme', 'bodyPx', 'lineHeightRatio', 'inlineStyleZero'], where);
  if (Object.keys(v).length === 0) fail(where, 'must not be empty');
  const out = {};
  if (v.theme !== undefined) {
    if (typeof v.theme !== 'string' || !THEMES.includes(v.theme)) fail(`${where}.theme`, 'must be one of the appearance values');
    out.theme = v.theme;
  }
  if (v.bodyPx !== undefined) {
    if (!Number.isInteger(v.bodyPx) || v.bodyPx < 8 || v.bodyPx > 64) fail(`${where}.bodyPx`, 'must be an integer from 8 to 64');
    out.bodyPx = v.bodyPx;
  }
  if (v.lineHeightRatio !== undefined) {
    if (typeof v.lineHeightRatio !== 'number' || !Number.isFinite(v.lineHeightRatio) || v.lineHeightRatio < 1 || v.lineHeightRatio > 3) fail(`${where}.lineHeightRatio`, 'must be a number from 1 to 3');
    out.lineHeightRatio = v.lineHeightRatio;
  }
  if (v.inlineStyleZero !== undefined) out.inlineStyleZero = bool(v.inlineStyleZero, `${where}.inlineStyleZero`);
  return out;
}

function parseExpect(v, where) {
  if (!isObj(v)) fail(where, 'must be an object');
  onlyKeys(v, [
    'completed', 'stoppedAfter', 'reviewAchieved', 'nextChapter', 'inputAreaAbsent', 'viewSwitchVisible',
    'rejectScenes', 'rejectCategories', 'excluded',
    'entryPorts', 'entryAtStart', 'inputGuidesAbsent', 'fileInputs', 'controls',
    'visibleViews', 'leftSwitchAbsent', 'rowPaths', 'rowTexts', 'screenTextAbsent', 'appearance', 'narrowTabs', 'docBodyRows',
  ], where);
  const out = { completed: bool(v.completed, `${where}.completed`) };
  if (v.stoppedAfter !== undefined) {
    if (out.completed) fail(`${where}.stoppedAfter`, 'only with completed false');
    const m = /^([A-Za-z0-9_/-]{1,100}):(entered|accepted|achieved)$/.exec(typeof v.stoppedAfter === 'string' ? v.stoppedAfter : '');
    if (!m) fail(`${where}.stoppedAfter`, 'must be <uid>:<status>');
    out.stoppedAfter = v.stoppedAfter;
  }
  for (const k of ['reviewAchieved', 'nextChapter', 'viewSwitchVisible']) if (v[k] !== undefined) out[k] = count(v[k], `${where}.${k}`);
  if (v.inputAreaAbsent !== undefined) out.inputAreaAbsent = bool(v.inputAreaAbsent, `${where}.inputAreaAbsent`);
  if (v.rejectScenes !== undefined) out.rejectScenes = intSet(v.rejectScenes, `${where}.rejectScenes`, SCENES);
  if (v.rejectCategories !== undefined) {
    if (!Array.isArray(v.rejectCategories) || !v.rejectCategories.every((c) => CATEGORIES.includes(c)) || new Set(v.rejectCategories).size !== v.rejectCategories.length) fail(`${where}.rejectCategories`, 'must be a unique list of categories');
    out.rejectCategories = [...v.rejectCategories].sort();
  }
  if (v.excluded !== undefined) {
    if (!Array.isArray(v.excluded) || v.excluded.length > SCENES.length) fail(`${where}.excluded`, 'must be an array');
    out.excluded = v.excluded.map((x, i) => {
      const w = `${where}.excluded[${i}]`;
      if (!isObj(x)) fail(w, 'must be an object');
      onlyKeys(x, ['scene', 'reason'], w);
      if (!SCENES.includes(x.scene)) fail(`${w}.scene`, 'must be an integer from 1 to 8');
      return { scene: x.scene, reason: str(x.reason, `${w}.reason`, /^[a-z0-9-]{1,64}$/) };
    });
    if (new Set(out.excluded.map((x) => x.scene)).size !== out.excluded.length) fail(`${where}.excluded`, 'scenes must be unique');
  }
  for (const k of ['entryPorts', 'entryAtStart']) if (v[k] !== undefined) out[k] = portList(v[k], `${where}.${k}`);
  if (out.entryPorts !== undefined && out.entryPorts.length === 0) fail(`${where}.entryPorts`, 'must not be empty');
  if (v.inputGuidesAbsent !== undefined) out.inputGuidesAbsent = bool(v.inputGuidesAbsent, `${where}.inputGuidesAbsent`);
  if (v.fileInputs !== undefined) {
    out.fileInputs = count(v.fileInputs, `${where}.fileInputs`);
    if (out.fileInputs > 10) fail(`${where}.fileInputs`, 'out of range');
  }
  if (v.visibleViews !== undefined) {
    if (!Array.isArray(v.visibleViews) || v.visibleViews.length > VIEW_KINDS.length) fail(`${where}.visibleViews`, 'must be an array');
    if (!v.visibleViews.every((k) => typeof k === 'string' && VIEW_KINDS.includes(k))) fail(`${where}.visibleViews`, 'unknown view kind');
    if (new Set(v.visibleViews).size !== v.visibleViews.length) fail(`${where}.visibleViews`, 'duplicate view kind');
    out.visibleViews = [...v.visibleViews];
  }
  if (v.leftSwitchAbsent !== undefined) out.leftSwitchAbsent = bool(v.leftSwitchAbsent, `${where}.leftSwitchAbsent`);
  if (v.rowPaths !== undefined) out.rowPaths = pathList(v.rowPaths, `${where}.rowPaths`);
  if (v.rowTexts !== undefined) out.rowTexts = parseTexts(v.rowTexts, `${where}.rowTexts`);
  // 画面に出てはいけない文字（含まないことの主張。rowPaths は前提にしない）。
  if (v.screenTextAbsent !== undefined) out.screenTextAbsent = textList(v.screenTextAbsent, `${where}.screenTextAbsent`);
  if (v.docBodyRows !== undefined) {
    out.docBodyRows = count(v.docBodyRows, `${where}.docBodyRows`);
    if (out.rowPaths !== undefined && out.docBodyRows > out.rowPaths.length) fail(`${where}.docBodyRows`, 'must not exceed expect.rowPaths');
    if (out.docBodyRows > 100) fail(`${where}.docBodyRows`, 'out of range');
  }
  if (v.appearance !== undefined) out.appearance = parseAppearance(v.appearance, `${where}.appearance`);
  if (v.narrowTabs !== undefined) out.narrowTabs = bool(v.narrowTabs, `${where}.narrowTabs`);
  if (v.controls !== undefined) {
    if (!isObj(v.controls)) fail(`${where}.controls`, 'must be an object');
    onlyKeys(v.controls, Object.keys(CONTROLS), `${where}.controls`);
    out.controls = Object.fromEntries(Object.entries(v.controls).map(([k, b]) => [k, bool(b, `${where}.controls.${k}`)]));
  }
  return out;
}

/**
 * 拒否の宣言の整合。操作の列にある拒否の場面の集合が rejectScenes と一致し、除外した場面が主張と重ならず操作にも無いこと。
 * 除外は「画面で出せないと分かっている場面」の記録で、合格には数えない。
 */
function checkRejectionDeclarations(sequence, expect) {
  const scenes = sequence.filter((o) => o.scene !== undefined).map((o) => o.scene);
  if (new Set(scenes).size !== scenes.length) fail('sequence', 'rejection scenes must be unique');
  if (scenes.length > 0 && expect.rejectScenes === undefined) fail('expect.rejectScenes', 'required when the sequence has rejections');
  if (expect.rejectScenes !== undefined && JSON.stringify([...scenes].sort((a, b) => a - b)) !== JSON.stringify(expect.rejectScenes)) fail('expect.rejectScenes', 'must equal the scenes in the sequence');
  for (const x of expect.excluded ?? []) {
    if (scenes.includes(x.scene)) fail('expect.excluded', 'a scene cannot be both claimed and excluded');
  }
}

/** 入口の部品の名前の列。PORTS の閉じた一覧から選び、重複を許さない。 */
function portList(v, where) {
  if (!Array.isArray(v) || v.length > PORTS.length) fail(where, 'must be an array');
  const seen = new Set();
  for (const id of v) {
    if (typeof id !== 'string' || !PORTS.includes(id)) fail(where, 'unknown port');
    if (seen.has(id)) fail(where, 'duplicate port');
    seen.add(id);
  }
  return [...v];
}

/** 設定ファイルを読み、検証し、パスを解決して返す。不正は ConfigError。 */
export function loadConfig(file) {
  let text;
  try {
    text = fs.readFileSync(file, 'utf8');
  } catch {
    throw new ConfigError('config: cannot read file');
  }
  let raw;
  try {
    raw = JSON.parse(text);
  } catch {
    throw new ConfigError('config: not valid JSON');
  }
  return parseConfig(raw, path.dirname(path.resolve(file)));
}

/** 読み込み済みの値の検証（試験が値を直接渡せるよう分ける）。dir は相対パスの基準。 */
export function parseConfig(raw, dir) {
  if (!isObj(raw)) fail('config', 'must be an object');
  onlyKeys(raw, ['format', 'name', 'source', 'base', 'viewport', 'initial', 'sequence', 'expect', 'csp'], 'config');
  if (raw.format !== 1) fail('format', 'must be 1');
  const name = str(raw.name, 'name', /^[a-z0-9-]{1,64}$/);
  const source = parseSource(raw.source, 'source');
  const base = str(raw.base, 'base', /^\/[a-z0-9-]+\/$/);
  let viewport = { width: 1280, height: 800 };
  if (raw.viewport !== undefined) {
    if (!isObj(raw.viewport)) fail('viewport', 'must be an object');
    onlyKeys(raw.viewport, ['width', 'height'], 'viewport');
    for (const k of ['width', 'height']) {
      const n = raw.viewport[k];
      if (!Number.isInteger(n) || n < 200 || n > 4000) fail(`viewport.${k}`, 'out of range');
    }
    viewport = { width: raw.viewport.width, height: raw.viewport.height };
  }
  const initial = pair(raw.initial, 'initial');
  const sequence = parseSequence(raw.sequence, 'sequence');
  const expect = parseExpect(raw.expect, 'expect');
  // 印の主張は行の集合の主張が前提（印を載せる行が実在すること）。
  sequence.forEach((op, i) => {
    if (op.marks === undefined) return;
    if (expect.rowPaths === undefined) fail(`sequence[${i}].marks`, 'needs expect.rowPaths');
    if (Object.keys(op.marks).some((k) => !expect.rowPaths.includes(k))) fail(`sequence[${i}].marks`, 'path is not in expect.rowPaths');
  });
  sequence.forEach((op, i) => {
    if (op.texts === undefined) return;
    if (expect.rowPaths === undefined) fail(`sequence[${i}].texts`, 'needs expect.rowPaths');
    if (Object.keys(op.texts).some((k) => !expect.rowPaths.includes(k))) fail(`sequence[${i}].texts`, 'path is not in expect.rowPaths');
  });
  if (expect.rowTexts !== undefined) {
    if (expect.rowPaths === undefined) fail('expect.rowTexts', 'needs expect.rowPaths');
    if (Object.keys(expect.rowTexts).some((k) => !expect.rowPaths.includes(k))) fail('expect.rowTexts', 'path is not in expect.rowPaths');
  }
  checkRejectionDeclarations(sequence, expect);
  if (expect.narrowTabs !== undefined && expect.narrowTabs !== (viewport.width < TAB_BREAKPOINT)) fail('expect.narrowTabs', `must match viewport.width (tabs appear below ${TAB_BREAKPOINT}px)`);
  let csp = [true, false];
  if (raw.csp !== undefined) {
    if (!Array.isArray(raw.csp) || raw.csp.length < 1 || raw.csp.length > 2 || !raw.csp.every((x) => typeof x === 'boolean')) fail('csp', 'must be a list of booleans');
    csp = [...new Set(raw.csp)];
  }
  const resolved = source.ir !== undefined
    ? { ir: source.ir, irFile: path.resolve(dir, source.ir) }
    : { ...source, packDir: path.resolve(dir, source.pack) };
  return { format: 1, name, source: resolved, base, viewport, initial, sequence, expect, csp, dir };
}

/**
 * 写し（work）の中のファイルへ書き換えを当てる。from は写しの中でちょうど 1 回出ること（0 回・複数回は拒否）。
 * 対象は通常のファイルに限り、解決後に写しの外へ出る・シンボリックリンクは拒否する（元の pack は書き換えない）。
 */
export function applyEdits(work, edits) {
  const root = fs.realpathSync(work);
  for (const [i, e] of edits.entries()) {
    const w = `edits[${i}]`;
    const target = path.resolve(root, e.file);
    if (target !== root && !target.startsWith(root + path.sep)) throw new ConfigError(`${w}: file escapes the pack copy`);
    // 親ディレクトリのシンボリックリンク経由で写しの外へ出ないよう、実体のパスでも包含を確かめる。
    let real;
    try {
      real = fs.realpathSync(target);
    } catch {
      throw new ConfigError(`${w}: file is not a regular file in the pack`);
    }
    if (real !== target || !real.startsWith(root + path.sep)) throw new ConfigError(`${w}: file escapes the pack copy`);
    const st = fs.lstatSync(target, { throwIfNoEntry: false });
    if (!st || !st.isFile()) throw new ConfigError(`${w}: file is not a regular file in the pack`);
    const text = fs.readFileSync(target, 'utf8');
    const n = text.split(e.from).length - 1;
    if (n !== 1) throw new ConfigError(`${w}: from must occur exactly once (found ${n})`);
    fs.writeFileSync(target, text.replace(e.from, () => e.to), 'utf8');
  }
}

/** 設定から IR の文字列を作る。pack は（任意で validate の後に）書き換えを当てて compile、ir はファイルを読む。 */
export function buildIr({ bin, config }) {
  const s = config.source;
  if (s.irFile !== undefined) {
    try {
      return fs.readFileSync(s.irFile, 'utf8');
    } catch {
      throw new ConfigError('source.ir: cannot read file');
    }
  }
  if (s.validate) {
    const r = spawnSync(bin, ['validate', s.packDir], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    if (r.status !== 0) throw new Error(`validate exit ${r.status}`);
  }
  return compilePack({ bin, packDir: s.packDir, edit: s.edits.length > 0 ? (work) => applyEdits(work, s.edits) : null });
}

/**
 * runJourney の結果を設定の期待と照合する。返すのは不合格の理由の配列（空なら合格）。
 * 理由は欄名だけで、教材の文面・値を含めない。外部通信・ページ内エラー・コンソールエラー・CSP の警告・安全の計数が 0 であること、
 * `ir/main.json` が 1 回 200 で取れたこと、実マウス操作で進めたことは設定で外せない固定の主張。
 */
export function judge(config, r) {
  const why = [];
  const e = config.expect;
  if (r.completed !== e.completed) why.push('completed');
  if (e.completed && r.stoppedAt !== null) why.push('stoppedAt');
  if (!e.completed && e.stoppedAfter !== undefined && r.stoppedAt?.after !== e.stoppedAfter) why.push('stoppedAfter');
  if (e.reviewAchieved !== undefined && (r.review ?? []).filter((x) => x.achieved === 'true').length !== e.reviewAchieved) why.push('reviewAchieved');
  if (e.nextChapter !== undefined && r.nextChapter !== e.nextChapter) why.push('nextChapter');
  for (const [label, m] of [['start', r.start], ['end', r.end]]) {
    if (!m) { why.push(`${label}-missing`); continue; }
    if (e.inputAreaAbsent === true) {
      if (ABSENT.some((id) => m.inputArea.absent[id] !== 0)) why.push(`inputAreaAbsent@${label}`);
      // 見えている input は履歴の読み込み用のファイル選択だけ。
      if (!m.inputArea.visibleInputs.every((x) => x.endsWith('|input|file'))) why.push(`visibleInputs@${label}`);
    }
    if (e.viewSwitchVisible !== undefined && m.viewSwitch.visibleButtons !== e.viewSwitchVisible) why.push(`viewSwitchVisible@${label}`);
    const seen = (id) => (m.parts?.[id]?.visible ?? 0) > 0;
    // 見えている入口は、許した部品と停止のボタンだけ（approval-btn・入力欄・候補などは許したときだけ）。
    if (e.entryPorts !== undefined && PORTS.some((id) => id !== 'stop-btn' && seen(id) && !e.entryPorts.includes(id))) why.push(`entryPorts@${label}`);
    if (label === 'start' && e.entryAtStart !== undefined && !e.entryAtStart.every(seen)) why.push('entryAtStart');
    if (e.inputGuidesAbsent === true) {
      if (GUIDES.some((id) => (m.parts?.[id]?.count ?? 1) !== 0)) why.push(`inputGuidesAbsent@${label}`);
      // 注意書きは常に 1 個あるが、入力欄がないので文字は空でなければならない。
      if ((m.notice?.textLength ?? 1) !== 0) why.push(`inputNotice@${label}`);
    }
    if (e.fileInputs !== undefined && (m.fileInputs?.visible !== e.fileInputs || m.fileInputs?.inImport !== e.fileInputs)) why.push(`fileInputs@${label}`);
    for (const [name, want] of Object.entries(e.controls ?? {})) {
      const part = m.parts?.[CONTROLS[name]] ?? { count: 0, visible: 0 };
      // 入れ物（中身が空だと箱が 0 になる）は DOM にあること、ボタンは見えることを「ある」とする。「ない」は DOM に 0 件。
      const present = CONTAINERS.includes(name) ? part.count > 0 : part.visible > 0;
      if (present !== want || (!want && part.count !== 0)) why.push(`controls.${name}@${label}`);
    }
  }
  why.push(...judgeAppearance(config, r));
  why.push(...judgeTabs(config, r));
  why.push(...judgeLeft(config, r));
  why.push(...judgeRejections(config, r));
  if ((r.safety ?? []).length !== 0) why.push('safety');
  if (r.external !== 0) why.push('external');
  if (r.pageErrors !== 0) why.push('pageErrors');
  if (r.consoleErrors !== 0) why.push('consoleErrors');
  if (r.cspWarnings !== 0) why.push('cspWarnings');
  if (r.run?.raw !== 0) why.push('rawClicks');
  if (!(r.run?.mouse > 0)) why.push('noMouseOps');
  if (JSON.stringify(r.irFetch) !== '[200]') why.push('irFetch');
  return why;
}

/**
 * 外観の照合（TASK-749 3/3。SECOND-10）。測った全時点（start・end と、あれば成果物タブの start・end）で見る。理由は欄名と時点だけ。
 */
function judgeAppearance(config, r) {
  const a = config.expect.appearance;
  if (a === undefined) return [];
  const why = [];
  const points = [['start', r.start], ['end', r.end], ['artifacts-start', r.artifactsTab?.start], ['artifacts-end', r.artifactsTab?.end]];
  for (const [label, m] of points) {
    if (!m) { if (label === 'start' || label === 'end') why.push(`appearance-missing@${label}`); continue; }
    if (a.inlineStyleZero === true && ((m.inlineStyle?.attrs ?? 1) !== 0 || (m.inlineStyle?.elements ?? 1) !== 0)) why.push(`inlineStyle@${label}`);
    if (a.theme !== undefined && m.theme?.main !== a.theme) why.push(`theme@${label}`);
    if (a.bodyPx !== undefined && m.font?.body?.px !== a.bodyPx) why.push(`bodyPx@${label}`);
    if (a.lineHeightRatio !== undefined && !(Math.abs((m.font?.body?.lineHeightRatio ?? -1) - a.lineHeightRatio) <= 0.01)) why.push(`lineHeight@${label}`);
  }
  return why;
}

/**
 * 幅 960px 未満の「操作」「成果物」タブの照合（TASK-749 3/3）。narrowTabs が真なら、操作タブでは右だけ・成果物タブでは左だけが見え、
 * 成果物タブの時点にも入力の領域の期待を当てる。偽なら tablist が出ないこと。宣言が無ければ何も見ない。
 */
function judgeTabs(config, r) {
  const e = config.expect;
  if (e.narrowTabs === undefined) return [];
  const why = [];
  if (e.narrowTabs === false) {
    for (const [label, m] of [['start', r.start], ['end', r.end]]) if (!m || m.panes?.tablist !== false) why.push(`tabs@${label}`);
    return why;
  }
  for (const [label, m] of [['start', r.start], ['end', r.end]]) {
    const p = m?.panes;
    if (!p || p.tablist !== true || p.selected !== 'ops' || p.right !== true || p.left !== false) why.push(`tabs@${label}`);
  }
  if (!r.artifactsTab) return [...why, 'artifactsTab-missing'];
  for (const label of ['start', 'end']) {
    const m = r.artifactsTab[label];
    const w = `artifacts-${label}`;
    if (!m) { why.push(`artifactsTab-missing@${label}`); continue; }
    const p = m.panes;
    if (!p || p.tablist !== true || p.selected !== 'artifacts' || p.left !== true || p.right !== false) why.push(`tabs@${w}`);
    if (e.inputAreaAbsent === true) {
      if (ABSENT.some((id) => m.inputArea.absent[id] !== 0)) why.push(`inputAreaAbsent@${w}`);
      if (!m.inputArea.visibleInputs.every((x) => x.endsWith('|input|file'))) why.push(`visibleInputs@${w}`);
    }
    const seen = (id) => (m.parts?.[id]?.visible ?? 0) > 0;
    if (e.entryPorts !== undefined && PORTS.some((id) => id !== 'stop-btn' && seen(id) && !e.entryPorts.includes(id))) why.push(`entryPorts@${w}`);
    if (e.inputGuidesAbsent === true) {
      if (GUIDES.some((id) => (m.parts?.[id]?.count ?? 1) !== 0)) why.push(`inputGuidesAbsent@${w}`);
      if ((m.notice?.textLength ?? 1) !== 0) why.push(`inputNotice@${w}`);
    }
  }
  return why;
}

/**
 * 左の領域（measureLeft の結果）の照合（TASK-749 2/3。SECOND-6・SECOND-8）。理由は欄名と時点（start・end・op[i]）だけ。
 * 見えるビューと切り替えの不在は測った全時点、行の集合は終了と marks のある操作、変化の印は marks のある操作で見る。
 * expect.screenTextAbsent は測った全時点で画面全体の文字（screenText）に宣言した文字が無いことを見る。
 * 行の文字は texts のある操作（texts@op[i]）と終了（rowTexts@end）で見る（Issue #2052）。
 * 印の 3 手段（data-mark・記号・文字）の一致は、何も主張しない設定でも測った全時点の全行で見る。
 */
function judgeLeft(config, r) {
  const e = config.expect;
  const why = [];
  const marked = config.sequence.some((op) => op.marks !== undefined);
  const texted = config.sequence.some((op) => op.texts !== undefined);
  const wants = e.visibleViews !== undefined || e.leftSwitchAbsent !== undefined || e.rowPaths !== undefined || e.docBodyRows !== undefined || e.rowTexts !== undefined || e.screenTextAbsent !== undefined || marked || texted;
  if (!wants) return why;
  const l = r.left;
  if (!l || !l.start || !l.end) return ['left-missing'];
  // 宣言した marks の操作に観察が無ければ不合格にする（観察の有無で素通りさせない）。
  config.sequence.forEach((op, i) => {
    if (op.marks !== undefined && !l.ops.some((o) => o.index === i)) why.push(`marksMissing@op[${i}]`);
  });
  config.sequence.forEach((op, i) => {
    if (op.texts !== undefined && !l.ops.some((o) => o.index === i)) why.push(`textsMissing@op[${i}]`);
  });
  // 経路の行が無い、または期待の文字のどれかを含まなければ偽（部分一致。文字は理由に出さない）。
  const textsOk = (rows, want) => Object.entries(want).every(([p, list]) => {
    const row = rows.find((x) => x.path === p);
    return row !== undefined && list.every((t) => typeof row.text === 'string' && row.text.includes(t));
  });
  const points = [['start', l.start, null], ...l.ops.map((o) => [`op[${o.index}]`, o, o.index]), ['end', l.end, null]];
  for (const [label, m, index] of points) {
    if (m.settled !== true) why.push(`leftUnsettled@${label}`);
    // 左の領域が実際に見えていること（幅 960px 未満で操作タブのまま測った素通りの合格を防ぐ。TASK-749 3/3）。
    if (m.panes?.left !== true) why.push(`leftHidden@${label}`);
    // 画面全体の文字に宣言した文字が無いこと。測れていなければ不合格（素通り防止）。理由に文字は出さない。
    if (e.screenTextAbsent !== undefined) {
      if (typeof m.screenText !== 'string' || m.screenText === '') why.push(`screenTextMissing@${label}`);
      else if (e.screenTextAbsent.some((t) => m.screenText.includes(t))) why.push(`screenTextAbsent@${label}`);
    }
    if (e.visibleViews !== undefined && JSON.stringify([...m.views.shown].sort()) !== JSON.stringify([...e.visibleViews].sort())) why.push(`visibleViews@${label}`);
    if (e.leftSwitchAbsent === true) {
      const s = m.select;
      if (s.buttons !== 0 || s.visibleButtons !== 0 || s.visible !== 0 || LEFT_ABSENT.some((id) => m.absent[id] !== 0)) why.push(`leftSwitch@${label}`);
    }
    if (m.rows.some((row) => {
      const want = MARK_TABLE[row.mark];
      return want === undefined || row.glyph !== want[0] || row.markText !== want[1];
    })) why.push(`markGlyphText@${label}`);
    const marks = index === null ? undefined : config.sequence[index].marks;
    const texts = index === null ? undefined : config.sequence[index].texts;
    const paths = m.rows.map((row) => row.path);
    const setEq = (want) => paths.length === want.length && new Set(paths).size === paths.length && want.every((p) => paths.includes(p));
    if (e.rowPaths !== undefined && (label === 'end' || marks !== undefined || texts !== undefined) && !setEq(e.rowPaths)) why.push(`rowPaths@${label}`);
    if (marks !== undefined && m.rows.some((row) => row.mark !== (marks[row.path] ?? ''))) why.push(`marks@${label}`);
    if (texts !== undefined && !textsOk(m.rows, texts)) why.push(`texts@${label}`);
    if (label === 'end' && e.rowTexts !== undefined && !textsOk(m.rows, e.rowTexts)) why.push('rowTexts@end');
    // 本文の区画を持つ行の数と、左の領域全体の区画の数（行の外の区画は不一致にする）。TASK-755。
    // 0 件の主張は測った全時点で見る。正の件数は行が揃う時点（終了と marks のある操作）だけで見る（開始の時点は行が無い）。
    if (e.docBodyRows !== undefined && (e.docBodyRows === 0 || label === 'end' || marks !== undefined)) {
      const inRows = m.rows.reduce((n, row) => n + row.docBody, 0);
      if (m.rows.filter((row) => row.docBody > 0).length !== e.docBodyRows || m.docBodyTotal !== inRows) why.push(`docBodyRows@${label}`);
    }
  }
  return why;
}

/**
 * 拒否の操作ごとの照合（SECOND-12 の画面の部分）。拒否の行が出たこと・分類と文面が IR と一致すること・拒否の前後で左の行と Step が
 * 変わらないことを見る。理由は欄名（reject[<操作の番号>].<欄>）だけで、教材の文面を含めない。
 * 拒否の操作が無い設定では何も返さない。
 */
export function judgeRejections(config, r) {
  const ops = config.sequence.map((op, index) => ({ op, index })).filter(({ op }) => op.expect.status === 'rejected');
  const why = [];
  if (ops.length === 0) return why;
  const passScenes = [];
  const seen = new Set();
  for (const { op, index } of ops) {
    const rec = (r.rejections ?? []).find((x) => x.index === index);
    const w = `reject[${index}]`;
    if (!rec) { why.push(`${w}.missing`); continue; }
    const before = why.length;
    if (rec.category !== op.category || rec.irCategory !== op.category) why.push(`${w}.category`);
    if (!rec.reasonOk || !rec.nextOk) why.push(`${w}.text`);
    if (!rec.rowsUnchanged) why.push(`${w}.rows`);
    if (!rec.stepUnchanged) why.push(`${w}.step`);
    if (rec.category) seen.add(rec.category);
    if (why.length === before) passScenes.push(op.scene);
  }
  const e = config.expect;
  if (e.rejectScenes !== undefined && JSON.stringify([...passScenes].sort((a, b) => a - b)) !== JSON.stringify(e.rejectScenes)) why.push('rejectScenes');
  if (e.rejectCategories !== undefined && JSON.stringify([...seen].sort()) !== JSON.stringify(e.rejectCategories)) why.push('rejectCategories');
  return why;
}
