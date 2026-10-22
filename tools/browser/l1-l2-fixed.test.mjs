// 固定の入力列での L1・L2 の比較と、履歴の `ops` の揺れる欄の検査（TASK-114 2/2。DLV-15。`make test-l2-run` から呼ばれる）。
//
// 目的: 1/2（l1-l2-match.test.mjs）の入力列はコード（journey）で、教材 ch0_3 やコードが変わると比べる列も一緒に変わる。
// ここでは、事前に固定した列（crates/web/tests/data/fixed_sequence_ch0_1/sequence.json）を、凍結した IR の写し（同ディレクトリの
// ir.json。SHA-256 を列に書いて固定）と組にして流す。配信する木の `ir/main.json` を写しへ差し替えて配るため、ch0_3 や本物の教材が
// 後で変わっても同じ IR で再生でき、再生できなくなることが無い（PoC-10 で凍結した列が教材の修正の後に再生できなくなった穴を埋める）。
// 写しと違う IR に当たったときは、page を開く前（遷移・マウント待ちより前）に `ir-mismatch` で断る。
//
// 構成:
//   1. 前提: 差し替えた L1 の木と L2 の木が、同じファイルの集合で同じ SHA-256（差がビルドでなく配信の差だけであること）
//   2. 固定の列を 3 配信先 × 3 回（9 回）流し、凍結した期待（口の数・`ops`・content_hash）と 9 回の相互の一致（観察の列・snapshot・history）を求める
//   3. 対照: 配信する IR が別でも差し替えて再生できる／差し替えなしでは最初に断る／期待を 1 つ変えると不一致のパスを報告する
//   4. 履歴の `ops` の揺れる欄: 許可リストの検査・時計と乱数をずらした 2 回の実行が 9 回と同じバイト列・検出器の対照
//   5. 凍結（FANDHE_FREEZE_SEQUENCE の指定時だけ。CI では流さない）: journey の操作を記録して列を書き出す。書き出した後は必ず失敗で終える
//
// 入力（環境変数。未設定・不在は skip にせず失敗にする）:
//   FANDHE_L2_ZIP・FANDHE_L2_IR・FANDHE_L2_SITE  l1-l2-match.test.mjs と同じ
//   FANDHE_FIXED_SEQ_DIR       固定の列のディレクトリ（sequence.json と ir.json）
//   FANDHE_FREEZE_SEQUENCE     凍結の出力先ディレクトリ（crates/web/tests/data/ の下。指定時は凍結の試験だけを流す）
// 出力に履歴の本文・入力した値・DOM の文字を載せない（件数・ハッシュの先頭 12 桁・パス・合否・時間だけ）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launchBrowser, channelFromEnv, openPage } from './harness.mjs';
import { journey, mouseDriver } from './ch0-3-journey.mjs';
import { stepOrder, domObserver, L2_STOP } from './l2-journey.mjs';
import { unzipTo } from './l2-server.mjs';
import { sha256, firstDiffPath } from './screen-snapshot.mjs';
import { runOnce as runOnceWith, withTarget as withTargetWith, treeDiff, overlaidSite, IR_REL } from './l1-l2-runner.mjs';
import {
  recordingDriver, recordingObserver, loadSequence, assertServedIr, replaySequence, compareOps, volatileOps, opKinds, SEQUENCE_FORMAT,
} from './fixed-sequence.mjs';

const RUNS = 3;
const TARGETS = ['L1-a', 'L1-b', 'L2'];
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const FREEZE = process.env.FANDHE_FREEZE_SEQUENCE ?? '';
const ctx = { browser: null, order: null, tmpDir: null, zip: null, siteDir: null };
const results = [];
let loaded = null;
const short = (h) => h.slice(0, 12);

before(async () => {
  const zip = process.env.FANDHE_L2_ZIP;
  const irFile = process.env.FANDHE_L2_IR;
  const siteDir = process.env.FANDHE_L2_SITE;
  assert.ok(zip && fs.statSync(zip, { throwIfNoEntry: false })?.isFile(), 'FANDHE_L2_ZIP must point to the L2 zip');
  assert.ok(irFile && fs.statSync(irFile, { throwIfNoEntry: false })?.isFile(), 'FANDHE_L2_IR must point to the IR json');
  assert.ok(siteDir && fs.statSync(siteDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_L2_SITE must point to the delivery directory');
  ctx.zip = zip;
  ctx.siteDir = siteDir;
  ctx.tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l1-l2-fixed-'));
  if (FREEZE) {
    ctx.order = stepOrder(fs.readFileSync(irFile, 'utf8'));
  } else {
    const dir = process.env.FANDHE_FIXED_SEQ_DIR;
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_FIXED_SEQ_DIR must point to the fixed sequence directory');
    loaded = loadSequence(dir);
    // 順序は、凍結した IR から導く（配信する IR ではなく、固定の列と組の IR）。
    ctx.order = stepOrder(loaded.irBytes.toString('utf8'));
  }
  ctx.browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (ctx.browser) await ctx.browser.close();
  if (ctx.tmpDir) fs.rmSync(ctx.tmpDir, { recursive: true, force: true });
});

const runOnce = (label, origin, url, opts) => runOnceWith(ctx, label, origin, url, opts);
const withTarget = (target, fn, opts) => withTargetWith(ctx, target, fn, opts);

/**
 * 固定の列を再生する runOnce の指定。配信されている IR が凍結した IR であることは、page を開く前（遷移・マウント待ちより前）に
 * 確かめる（precheck）。違う IR がマウントに失敗・時間切れになるより先に、`ir-mismatch` の理由で断るため。
 */
const replayOpts = (url, extra = {}) => ({
  precheck: () => assertServedIr(url, loaded.seq),
  drive: (page, { L, onStep }) => replaySequence(page, loaded.seq, { order: ctx.order, L, onStep }),
  ...extra,
});

/** `fn` の実行中に browser context が開かれた回数を数える（断りが page を開く前に来たことの確認用）。 */
async function countContexts(fn) {
  const browser = ctx.browser;
  const orig = browser.newContext;
  let opened = 0;
  browser.newContext = (...args) => {
    opened += 1;
    return orig.apply(browser, args);
  };
  try {
    await fn();
  } finally {
    browser.newContext = orig;
  }
  return opened;
}

/** 凍結した期待との照合。不一致の項目の名前（値は含まない）の一覧を返す。 */
function expectationProblems(r) {
  const out = [];
  const opsPath = compareOps(loaded.seq.expected_ops, r.parsed.ops);
  if (opsPath !== null) out.push(`ops@${opsPath}`);
  if (r.parsed.versions?.content_hash !== loaded.seq.content_hash) out.push('content_hash');
  return out;
}

if (FREEZE) {
  test('凍結: journey の操作を記録して固定の列を書き出す（書き出した後は必ず失敗で終える）', async () => {
    const out = path.resolve(FREEZE);
    const data = path.join(ROOT, 'crates', 'web', 'tests', 'data') + path.sep;
    assert.ok((out + path.sep).startsWith(data), 'FANDHE_FREEZE_SEQUENCE must be a directory under crates/web/tests/data/');
    assert.ok(fs.statSync(out, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_FREEZE_SEQUENCE must be an existing directory');
    const irBytes = fs.readFileSync(process.env.FANDHE_L2_IR);
    const entries = [];
    await withTarget('L1-a', async (origin, url) => {
      const r = await runOnce('freeze', origin, url, {
        drive: async (page, { L, onStep }) => {
          const observe = recordingObserver(domObserver(ctx.order), entries);
          await journey(page, recordingDriver(mouseDriver(page), entries), 'freeze', L, L2_STOP, { observe, onStep });
        },
      });
      assert.deepEqual(r.failedItems, []);
      assert.deepEqual(r.problems, []);
      assert.deepEqual(volatileOps(r.parsed.ops), []);
      const seq = {
        sequence_format: SEQUENCE_FORMAT,
        frozen_on: new Date().toISOString().slice(0, 10),
        source: 'tools/browser/ch0-3-journey.mjs の journey を mouseDriver で main/st-2-1 まで流して記録した（TASK-114 2/2）',
        ir_file: 'ir.json',
        ir_sha256: sha256(irBytes),
        content_hash: r.parsed.versions.content_hash,
        stop_at: L2_STOP,
        entries,
        expected_ops: r.parsed.ops,
      };
      fs.writeFileSync(path.join(out, 'ir.json'), irBytes);
      // 見やすさのため、項目と期待の `ops` は 1 件 1 行にする（欄の順と値は JSON.stringify と同じ）。
      const { entries: es, expected_ops: eo, ...head } = seq;
      const lines = (list) => `[\n${list.map((x) => `    ${JSON.stringify(x)}`).join(',\n')}\n  ]`;
      const headText = Object.entries(head).map(([k, v]) => `  ${JSON.stringify(k)}: ${JSON.stringify(v)},\n`).join('');
      fs.writeFileSync(path.join(out, 'sequence.json'), `{\n${headText}  "entries": ${lines(es)},\n  "expected_ops": ${lines(eo)}\n}\n`);
      console.log(`凍結: 項目 ${entries.length} 件・ops ${r.parsed.ops.length} 件・ir ${short(seq.ir_sha256)}・content_hash ${short(seq.content_hash)}`);
    });
    assert.fail('frozen: remove FANDHE_FREEZE_SEQUENCE and run again (a freeze run is never counted as a pass)');
  });
} else {
  test('前提: IR を差し替えた L1 の木と L2 の木が、同じファイルの集合で同じ SHA-256 である', () => {
    const work = fs.mkdtempSync(path.join(ctx.tmpDir, 'tree-'));
    const l1 = overlaidSite(ctx.siteDir, ctx.tmpDir, loaded.irBytes);
    try {
      const top = unzipTo(ctx.zip, work);
      fs.writeFileSync(path.join(top, 'site', IR_REL), loaded.irBytes);
      const d = treeDiff(l1, path.join(top, 'site'));
      console.log(`差し替えた木の照合: ファイル ${d.files} 件・片方だけ ${d.onlyOne} 件・内容の違い ${d.differ} 件`);
      assert.ok(d.files >= 1);
      assert.deepEqual({ onlyOne: d.onlyOne, differ: d.differ }, { onlyOne: 0, differ: 0 });
    } finally {
      fs.rmSync(l1, { recursive: true, force: true });
      fs.rmSync(work, { recursive: true, force: true });
    }
  });

  for (const target of TARGETS) {
    for (let n = 1; n <= RUNS; n += 1) {
      test(`${target} ${n}/${RUNS} 回目: 固定の列を流し、凍結した期待と照合する`, async () => {
        await withTarget(target, async (origin, url) => {
          const r = await runOnce(`${target}#${n}`, origin, url, replayOpts(url));
          results.push(r);
          console.log(`${r.label}: 観察 ${JSON.parse(r.obsText).length} 件・要求 ${r.requests} 件・obs ${short(r.hashes.obs)}・`
            + `snapshot ${short(r.hashes.snapshot)}・history ${short(r.hashes.history)}・${r.ms} ms`);
          assert.deepEqual(r.failedItems, []);
          assert.deepEqual(r.problems, []);
          assert.deepEqual(expectationProblems(r), []);
          assert.deepEqual(volatileOps(r.parsed.ops), [], `${r.label}: volatile fields in ops`);
        }, { overlay: loaded.irBytes });
      });
    }
  }

  test('9 回の観察の列・snapshot.json・history.json がそれぞれ 1 種類（バイト単位で一致）', () => {
    assert.equal(results.length, TARGETS.length * RUNS, '9 回すべてが通らなかったため比べない');
    const base = results[0];
    for (const key of ['obs', 'snapshot', 'history']) {
      const kinds = new Set(results.map((r) => r.hashes[key]));
      const text = { obs: 'obsText', snapshot: 'snapshot', history: 'history' }[key];
      const where = results.filter((r) => r.hashes[key] !== base.hashes[key])
        .map((r) => `${r.label}@${firstDiffPath(base[text], r[text])}(${short(r.hashes[key])})`);
      console.log(`${key}: 種類 ${kinds.size}（先頭 ${short(base.hashes[key])}）`);
      assert.equal(kinds.size, 1, `${key} differs from ${base.label}: ${where.join(' ')}`);
    }
  });

  test('対照: 配信元の IR が別でも、凍結した IR に差し替えて固定の列を再生できる（教材が変わっても再生できる）', async () => {
    // 配信する木の IR を、末尾に改行を足した（バイト列の違う）IR にしてから、凍結した IR を差し替える。
    const mutant = Buffer.concat([loaded.irBytes, Buffer.from('\n')]);
    assert.notEqual(sha256(mutant), loaded.seq.ir_sha256, 'the mutant IR differs from the frozen IR');
    const mutantSite = overlaidSite(ctx.siteDir, ctx.tmpDir, mutant);
    const saved = ctx.siteDir;
    try {
      ctx.siteDir = mutantSite;
      await withTarget('L1-a', async (origin, url) => {
        const r = await runOnce('L1-a#mutant-overlay', origin, url, replayOpts(url));
        assert.deepEqual(r.failedItems, []);
        assert.deepEqual(r.problems, []);
        assert.deepEqual(expectationProblems(r), []);
      }, { overlay: loaded.irBytes });
    } finally {
      ctx.siteDir = saved;
      fs.rmSync(mutantSite, { recursive: true, force: true });
    }
  });

  test('対照: 差し替えなしで別の IR に当たると、page を開く前に ir-mismatch で断る', async () => {
    const mutant = Buffer.concat([loaded.irBytes, Buffer.from('\n')]);
    const mutantSite = overlaidSite(ctx.siteDir, ctx.tmpDir, mutant);
    const saved = ctx.siteDir;
    try {
      ctx.siteDir = mutantSite;
      await withTarget('L1-a', async (origin, url) => {
        const t0 = Date.now();
        const opened = await countContexts(() => assert.rejects(runOnce('L1-a#mutant', origin, url, replayOpts(url)), /ir-mismatch/));
        const ms = Date.now() - t0;
        console.log(`ir-mismatch で断るまで ${ms} ms・開いた context ${opened} 件`);
        assert.equal(opened, 0, 'the refusal must come before the page is opened (before navigation and the mount wait)');
        assert.ok(ms < 15000, 'the refusal must come before any step times out');
      });
    } finally {
      ctx.siteDir = saved;
      fs.rmSync(mutantSite, { recursive: true, force: true });
    }
  });

  test('対照: 差し替えなしで JSON として読めない IR に当たっても、page を開く前に ir-mismatch で断る', async () => {
    // JSON として読めない IR（凍結した IR の前半だけ）。現状のエンジンは断りの画面でマウントを終えるが、IR の照合はマウントの
    // 結果に依らない（マウントの失敗・時間切れより先に ir-mismatch で断る）ことを、page を開いた回数 0 件で確かめる。
    const broken = loaded.irBytes.subarray(0, Math.floor(loaded.irBytes.length / 2));
    const brokenSite = overlaidSite(ctx.siteDir, ctx.tmpDir, broken);
    const saved = ctx.siteDir;
    try {
      ctx.siteDir = brokenSite;
      await withTarget('L1-a', async (origin, url) => {
        const t0 = Date.now();
        const opened = await countContexts(() => assert.rejects(runOnce('L1-a#broken', origin, url, replayOpts(url)), /ir-mismatch/));
        const ms = Date.now() - t0;
        console.log(`不正な IR で ir-mismatch で断るまで ${ms} ms・開いた context ${opened} 件`);
        assert.equal(opened, 0, 'the refusal must come before the page is opened (before navigation and the mount wait)');
        assert.ok(ms < 15000, 'the refusal must come before the mount wait times out');
      });
    } finally {
      ctx.siteDir = saved;
      fs.rmSync(brokenSite, { recursive: true, force: true });
    }
  });

  test('対照: 期待の 1 値を変えると、比較は最初に違ったパスを報告して不一致になる', () => {
    assert.ok(results.length >= 1);
    const ops = structuredClone(loaded.seq.expected_ops);
    assert.equal(compareOps(ops, results[0].parsed.ops), null);
    const i = ops.findIndex((o) => typeof o === 'object' && o.submit !== undefined);
    assert.ok(i >= 0, 'expected_ops has a submit');
    ops[i].submit = `${ops[i].submit}x`;
    assert.equal(compareOps(ops, results[0].parsed.ops), `$.${i}.submit`);
    assert.equal(compareOps(ops.slice(1), results[0].parsed.ops) === null, false);
  });

  test('履歴の ops: 凍結した期待と 9 回の履歴のすべてで、許可リストに無い鍵・時刻らしい値が無い', () => {
    assert.deepEqual(volatileOps(loaded.seq.expected_ops), []);
    assert.ok(results.length >= 1);
    for (const r of results) assert.deepEqual(volatileOps(r.parsed.ops), []);
    console.log(`現れた ops の種類（件数）: ${JSON.stringify(opKinds(loaded.seq.expected_ops))}`);
  });

  test('履歴の ops: 時計と乱数をずらした 2 回の実行が、9 回と同じ history.json のバイト列になる', async () => {
    assert.ok(results.length >= 1);
    const shifts = [
      { at: '2001-02-03T04:05:06Z', seed: 1 },
      { at: '2088-08-08T08:08:08Z', seed: 2 },
    ];
    for (const s of shifts) {
      await withTarget('L1-a', async (origin, url) => {
        const prepare = async ({ page, context }) => {
          await page.clock.setFixedTime(new Date(s.at));
          // 回ごとに違う種の決まった列にする（同じ種なら同じ列。Math.random に頼る欄があれば、2 回で履歴が違ってくる）。
          await context.addInitScript((seed) => {
            let x = seed;
            Math.random = () => {
              x = (Math.imul(x, 1664525) + 1013904223) >>> 0;
              return x / 4294967296;
            };
          }, s.seed);
        };
        const r = await runOnce(`L1-a#clock${s.seed}`, origin, url, replayOpts(url, { prepare }));
        assert.deepEqual(r.failedItems, []);
        assert.deepEqual(r.problems, []);
        // 差し替えが効いていること（時計が本当に固定した時刻だったこと）は、別の画面の取得ではなく同じ構成の新しい page で確かめる。
        assert.equal(r.hashes.history, results[0].hashes.history,
          `history differs with shifted clock/random (${s.seed}): ${firstDiffPath(results[0].history, r.history)}`);
        console.log(`時計 ${s.at}・乱数の種 ${s.seed}: history ${short(r.hashes.history)}（9 回と一致）`);
      }, { overlay: loaded.irBytes });
    }
    // 自己確認: 同じ差し替えの page で Date.now() が固定した時刻を返す。
    const { page, context } = await openPage(ctx.browser, { origin: 'http://127.0.0.1:1' });
    try {
      await page.clock.setFixedTime(new Date(shifts[0].at));
      assert.equal(await page.evaluate(() => new Date(Date.now()).toISOString().slice(0, 4)), '2001');
    } finally {
      await context.close().catch(() => {});
    }
  });

  test('対照: 検出器は、時刻・未知の鍵・時刻らしい値を持つ合成の ops のそれぞれのパスを返す', () => {
    assert.deepEqual(volatileOps([{ tick: { generation: 1, step: 'a', index: 0, at: 1730000000000 } }]), ['$[0].tick.at(key)', '$[0].tick.at(number)']);
    assert.deepEqual(volatileOps([{ submit: 'x', time: 'y' }]), ['$[0].time(key)']);
    assert.deepEqual(volatileOps([{ submit: '2026-10-22T00:00:00Z' }]), ['$[0].submit(datetime)']);
    assert.deepEqual(volatileOps([{ foo: 1 }]), ['$[0].foo(key)']);
    assert.deepEqual(volatileOps([{ tick: { generation: 1730000000000, step: 'a', index: 0 } }]), ['$[0].tick.generation(number)']);
    assert.deepEqual(volatileOps([{ draft: 'a' }, { submit: 'a' }, 'reload', { control: 'save' }, { control: { go_to: 'x' } }]), []);
  });
}
