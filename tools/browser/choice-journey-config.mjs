// 入力欄のない教材の完走の道具（choice-journey.mjs）に渡す、教材固有の値の設定ファイル（JSON。版 1）の読み込み・検証と、
// 測った結果と設定の期待との照合（TASK-748 2/2。SECOND-4・SECOND-7）。
//
// 役割: 道具（choice-journey.mjs）は教材固有の値を持たず、合否も持たない。教材ごとの違い（操作の列・各操作の後の期待・派生版を作る書き換え・
// 結果の期待）はこの設定ファイルに書き、差し替えるだけで別の教材にも同じ道具を使えるようにする。本モジュールは
// 呼び出し元（choice-journey-run.mjs の CLI と choice-journey.test.mjs）から使われ、設定を未信頼の入力として扱って
// 形を厳しく検証し（fail-closed。未知の欄・範囲外の値はすべて ConfigError）、期待との照合（judge）の理由は欄名だけで返す。
// 設定の置き場: tools/browser/choice-journey/<name>.json。形式の表は docs/testing.md 5.437。
// 拒否の場面の宣言（操作の scene・rejection・category と expect の rejectScenes・rejectCategories・excluded）の形と、観察との照合 judgeRejections もここにある（TASK-750。記録は 5.446）。
// 出力に教材の文面・入力値・パスの全体を出さない（ログの偽装と情報の流出を避ける）。
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { compilePack, ABSENT } from './choice-journey.mjs';

/** 設定の形・値が不正なとき。CLI は終了コード 2（誤用）に対応づける。 */
export class ConfigError extends Error {}

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
    onlyKeys(op, ['kind', 'expect', 'scene', 'rejection', 'category', ...(extra ? [extra] : [])], w);
    const out = { kind, expect: pair(op.expect, `${w}.expect`, OP_STATUSES) };
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

/** 整数の集合（昇順・重複なし）の配列。 */
function intSet(v, where, allowed) {
  if (!Array.isArray(v) || v.length > allowed.length || !v.every((n) => allowed.includes(n)) || new Set(v).size !== v.length) fail(where, 'must be a unique list of allowed values');
  return [...v].sort((a, b) => a - b);
}

function parseExpect(v, where) {
  if (!isObj(v)) fail(where, 'must be an object');
  onlyKeys(v, ['completed', 'stoppedAfter', 'reviewAchieved', 'nextChapter', 'inputAreaAbsent', 'viewSwitchVisible', 'rejectScenes', 'rejectCategories', 'excluded'], where);
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
  checkRejectionDeclarations(sequence, expect);
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
  }
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
