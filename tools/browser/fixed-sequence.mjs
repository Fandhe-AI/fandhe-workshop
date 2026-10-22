// 固定の入力列（凍結した IR と組）の記録・読み込み・再生と、履歴の `ops` の揺れる欄の検出（TASK-114 2/2。DLV-15）。
//
// 役割: l1-l2-fixed.test.mjs が使う部品。1/2 の誘導（ch0-3-journey.mjs の journey）はコードで、教材やコードが変わると比べる列も
// 一緒に変わる。ここでは journey の操作を一度だけ記録して JSON の列にし、それを流した IR の写し（凍結）と SHA-256 で組にして固定する。
// 試験ではないため `.test.mjs` にしない（試験ファイルを import すると、試験が二重に登録されるため）。
//
// 列（sequence.json。版 1）の項目は 3 種。
//   { op, args }                         操作（mouseDriver の口の名前と引数）
//   { expect, status, ports }            Step の状態と 4 つの口の数。画面の語は入れない（文面の変更で固定の列が壊れないため）
//   { still, kind }                      拒否（rejected）・問い返し（clarified）の後も同じ Step のままであること
// 再生の前に、凍結した IR が配信されていることを `assertServedIr` で確かめ（呼び出し元が page を開く前に呼ぶ。違えば `ir-mismatch` で
// 最初に断る）、`replaySequence` が操作を順に流す。
// 出力に履歴の本文・入力した値・DOM の文字を載せない。
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { mouseDriver, ports, waitCount, DRAFT, REJECTED, CLARIFIED, LOG, T } from './ch0-3-journey.mjs';
import { domObserver } from './l2-journey.mjs';
import { firstDiffPath } from './screen-snapshot.mjs';

export const SEQUENCE_FORMAT = 1;
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');

/** 記録できる操作（mouseDriver の口）。 */
const RECORDED_OPS = ['choose', 'point', 'decide', 'revise', 'send', 'sendMultiline', 'completeAndSend', 'esc', 'nextChapter'];

/** driver の操作を `entries` に記録しながら、そのまま実行する包み。 */
export function recordingDriver(driver, entries) {
  const out = {};
  for (const name of RECORDED_OPS) {
    out[name] = (...args) => {
      entries.push({ op: name, args });
      return driver[name](...args);
    };
  }
  return out;
}

/** observer の確認を `entries` に記録しながら、そのまま実行する包み。口の数は確認が通った直後の画面から読む。 */
export function recordingObserver(observer, entries) {
  return {
    async at(page, uid, status, timeout) {
      await observer.at(page, uid, status, timeout);
      entries.push({ expect: uid, status, ports: await ports(page) });
    },
    async stillAt(page, uid) {
      const ok = await observer.stillAt(page, uid);
      const kind = await page.evaluate((sel) => {
        const rows = document.querySelectorAll(`${sel} li`);
        return rows.length === 0 ? null : rows[rows.length - 1].getAttribute('data-kind');
      }, LOG);
      entries.push({ still: uid, kind });
      return ok;
    },
  };
}

/** `dir` の sequence.json と IR の写しを読み、IR の SHA-256 が列の記載と一致することを確かめて返す。 */
export function loadSequence(dir) {
  const seq = JSON.parse(fs.readFileSync(path.join(dir, 'sequence.json'), 'utf8'));
  if (seq.sequence_format !== SEQUENCE_FORMAT) throw new Error(`unsupported sequence_format: ${seq.sequence_format}`);
  if (!Array.isArray(seq.entries) || !Array.isArray(seq.expected_ops)) throw new Error('sequence has no entries or expected_ops');
  if (path.basename(seq.ir_file) !== seq.ir_file) throw new Error('ir_file must be a plain file name');
  const irBytes = fs.readFileSync(path.join(dir, seq.ir_file));
  if (sha(irBytes) !== seq.ir_sha256) throw new Error('ir-mismatch: the frozen IR does not match ir_sha256');
  return { seq, irBytes };
}

/** 配信先が返す IR（`<url>ir/main.json`）が凍結した IR と同じバイト列であることを確かめる。違えば最初に理由つきで断る。 */
export async function assertServedIr(url, seq) {
  const res = await fetch(new URL('ir/main.json', url));
  const got = sha(Buffer.from(await res.arrayBuffer()));
  if (res.status !== 200 || got !== seq.ir_sha256) {
    throw new Error(`ir-mismatch: served IR (${got.slice(0, 12)}) is not the frozen IR (${seq.ir_sha256.slice(0, 12)})`);
  }
}

/**
 * 固定の列 `seq.entries` を page に流す。確認の不一致は台帳 `L` に積み、操作の失敗（待ちの時間切れ）は例外にする。
 * journey の台帳でしか見ていない待ち（問い返しの後に下書きが空になる・拒否の後に下書きが全選択になる。JUDGE-27・TERM-22）は、
 * 教材に依らないエンジンの一般則として、ここで持つ。
 */
export async function replaySequence(page, seq, { order, L, onStep }) {
  const driver = mouseDriver(page);
  const observer = domObserver(order);
  let rejected = 0;
  let clarified = 0;
  for (const e of seq.entries) {
    if (e.op !== undefined) {
      if (!RECORDED_OPS.includes(e.op)) throw new Error(`unknown op: ${e.op}`);
      await driver[e.op](...e.args);
    } else if (e.expect !== undefined) {
      await observer.at(page, e.expect, e.status);
      const got = await ports(page);
      L.check('fixed', `${e.expect} ${e.status} の口`, JSON.stringify(got) === JSON.stringify(e.ports));
      await onStep(e.expect, e.status);
    } else if (e.still !== undefined) {
      if (e.kind === 'rejected') {
        rejected += 1;
        await waitCount(page, REJECTED, rejected);
      } else if (e.kind === 'clarified') {
        clarified += 1;
        await waitCount(page, CLARIFIED, clarified);
      } else {
        throw new Error(`unknown still kind: ${e.kind}`);
      }
      L.check('fixed', `${e.still} ${e.kind} で進まない`, await observer.stillAt(page, e.still));
      // 書き戻し・選択は 0 ミリ秒のタイマーで非同期に付くため、空になる（問い返し）・全選択になる（拒否）のを待つ。
      const settled = await page.waitForFunction(([sel, kind]) => {
        const el = document.querySelector(sel);
        if (!el) return false;
        if (kind === 'clarified') return el.value === '';
        return el.value === '' || (el.selectionStart === 0 && el.selectionEnd === el.value.length);
      }, [DRAFT, e.kind], { timeout: T }).then(() => true, () => false);
      L.check('fixed', `${e.still} ${e.kind} の後の下書きの状態`, settled);
    } else {
      throw new Error('unknown entry');
    }
  }
}

/** 書き出した `ops` と凍結した `expected_ops` を比べる。同じなら null、違えば最初に違ったパス（値は出さない）。 */
export function compareOps(expected, actual) {
  const a = JSON.stringify(expected);
  const b = JSON.stringify(actual);
  return a === b ? null : (firstDiffPath(a, b) ?? '$');
}

// ---- 履歴の `ops` の揺れる欄の検出 --------------------------------------------------------------------------------------------
// `ops` の要素の鍵は、core の `Op`・`Input`・`Control`・`TickAddr`・`HistoryOp` の serde の綴り（snake_case）と
// testdata/contract/history/*.json に現れる綴りだけにする。時刻・乱数・端末の識別子を足す鍵は、ここに無いので検出される。
// 限界: 実際の実行に現れなかった種類は、鍵の許可リストで型の上からしか保証できない。
export const OP_KEY_ALLOWLIST = new Set([
  'draft', 'submit', 'input', 'tick', 'control',
  'choice', 'command', 'text', 'ui', 'view', 'path', 'approval', 'decision', 'target', 'cancel',
  'generation', 'step', 'index',
  'stop', 'save', 'open_hint', 'go_to', 'restore', 'begin', 'set_guidance',
]);
const ISO_DATETIME = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/;
const MAX_COUNT = 2 ** 31;

/** `ops` を歩き、許可リストに無い鍵・時刻らしい値を持つ欄のパス（値は含まない）の一覧を返す。 */
export function volatileOps(ops) {
  const found = [];
  const walk = (v, p) => {
    if (typeof v === 'number') {
      if (!Number.isInteger(v) || v < 0 || v >= MAX_COUNT) found.push(`${p}(number)`);
    } else if (typeof v === 'string') {
      if (ISO_DATETIME.test(v)) found.push(`${p}(datetime)`);
    } else if (Array.isArray(v)) {
      v.forEach((x, i) => walk(x, `${p}[${i}]`));
    } else if (v !== null && typeof v === 'object') {
      for (const [k, x] of Object.entries(v)) {
        if (!OP_KEY_ALLOWLIST.has(k)) found.push(`${p}.${k}(key)`);
        walk(x, `${p}.${k}`);
      }
    }
  };
  walk(ops, '$');
  return found;
}

/** `ops` に現れた鍵と文字列の種類（`reload` など）の件数（ログ用。値は出さない）。 */
export function opKinds(ops) {
  const kinds = {};
  const add = (k) => { kinds[k] = (kinds[k] ?? 0) + 1; };
  for (const o of ops) {
    if (typeof o === 'string') add(o);
    else for (const [k, v] of Object.entries(o)) {
      add(k);
      if ((k === 'input' || k === 'control') && v !== null && typeof v === 'object') for (const kk of Object.keys(v)) add(`${k}.${kk}`);
      else if (k === 'control' && typeof v === 'string') add(`control.${v}`);
    }
  }
  return kinds;
}
