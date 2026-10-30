// 信頼できない入力への安全性の「観察の記録」の試験（BUILD-4。TASK-79 2/2）。試作の browser-test-poc7.mjs に当たる。
//
// pass／fail 型の試験（log-lines-xss・input-limit・json-depth など）が「あるべき値」を assert するのに対し、
// ここは今のビルドで観察できることを記録する。観察値（件数・段・時間・有無）は assert せず、t.diagnostic() で TAP の出力
// （CI のログ）に `[節名] ...` の 1 行で残す。assert するのは基盤だけである。
//   環境変数がそろっていること・ページがマウントすること・test-hooks の口があること・ページの例外で落ちないこと・
//   各節の空振りの検出（送った件数が 0 でない等。「完走した」が何も確かめていない状態を防ぐ）。
// 試作は finally の process.exit(0) が例外を覆い、失敗しても終了コード 0 になった。ここは node:test で、例外は必ず失敗になる。
//
// 節の担当（育てる先。別ファイルを増やさず、この 1 ファイルの該当の節を育てる）。観察できない部分は「未実装」と記録する。
//   data   : 試験データの SHA-256 の照合（不一致は前提の崩れとして失敗）
//   xss    : E1 は pack-body-xss.test.mjs（TASK-144）が pass／fail で確かめる。E2・E3 は TASK-145、E4 の置き換えた履歴は history-xss.test.mjs（TASK-146 1/2）、E4 の正規の経路は TASK-146 2/2、E5 は input-field-xss.test.mjs（TASK-147 1/2）、E6 は url-xss.test.mjs（TASK-147 2/2）が確かめ、本試験は配信設定の入力欄から送って記録する
//   oversize: 10 倍の水準は TASK-154（#855）・TASK-155
//   nest   : 深い入れ子・巨大な配列
//   cycle  : 巡回する IR の画面での起動口は無い（D-68 で `?ir=` の入り口は無くなった。実行時の教材の取得が入った後の課題）。遷移の上限は TASK-160（#444）
//   csp    : 配置と内容の検査は csp-placement.test.mjs（TASK-151 1/2）
//   secret : 警告の挙動は TASK-164（#541〜#544）
//
// 入力: 環境変数 FANDHE_WEB_DIR（配信設定）と FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
// 秘密情報らしい値: 部品から test の中だけで組み立て、完成値は出力・diagnostic・assert のメッセージのどこにも出さない。
// 出すのは形の ID（S-01〜S-10）と有無・件数だけにする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, SAFETY_KEYS,
} from './harness.mjs';
import { loadXssCases, e5Payloads } from './xss-cases.mjs';

const BASE = '/fandhe-safety-observe/';
const INPUT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const RECEIVED = '[data-testid="part-log-list"] > li[data-kind="received"]';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const SAFETY_DIR = path.join(ROOT, 'testdata', 'untrusted-input-safety');
const IR_MAX = 20 * 1024 * 1024;
const HISTORY_MAX = 10 * 1024 * 1024;

let browser;

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
  return dir;
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const log = (t, section, msg) => t.diagnostic(`[${section}] ${msg}`);

async function openMounted(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

// ページの例外・CSP 違反で落ちないこと（基盤の確認）。値は観察として別に記録する。
function assertNoPageErrors(rec, what) {
  assert.equal(rec.pageErrors.length, 0, `${what}: page errors ${rec.pageErrors.length}`);
}

// 送信後にページが静止するまで待つ。setTimeout 経由の dispatch・img の onerror（404 後）・描画の反映を、
// 件数ではなく効果として採取してから数えるため。ネットワークの idle → 描画 2 フレーム → タイマーの順に待つ。
async function settle(page, ms = 100) {
  await page.waitForLoadState('networkidle').catch(() => {});
  await page.evaluate((wait) => new Promise((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, wait)));
  }), ms);
}

const countsLine = (c) => SAFETY_KEYS.map((k) => `${k}=${c[k]}`).join(' ');

// 1. data: 試験データの SHA-256 の照合。
test('data: 試験データの SHA-256 manifest を照合する', (t) => {
  const manifests = fs.readdirSync(SAFETY_DIR).filter((f) => f.endsWith('.sha256')).sort();
  let total = 0;
  const bad = [];
  for (const m of manifests) {
    const lines = fs.readFileSync(path.join(SAFETY_DIR, m), 'utf8').split('\n').filter((l) => l.trim() !== '');
    let n = 0;
    for (const line of lines) {
      const mt = /^([0-9a-f]{64}) [ *]?(.+)$/.exec(line);
      assert.ok(mt, `${m}: unparsable manifest line`);
      const rel = mt[2];
      assert.ok(!rel.split('/').includes('..'), `${m}: path escapes: ${rel}`);
      const sum = crypto.createHash('sha256').update(fs.readFileSync(path.join(SAFETY_DIR, rel))).digest('hex');
      if (sum !== mt[1]) bad.push(rel);
      n += 1;
    }
    log(t, 'data', `${m}: ${n} files`);
    total += n;
  }
  log(t, 'data', `manifests=${manifests.length} files=${total} mismatches=${bad.length}`);
  assert.ok(total > 0, 'no manifest entries were read');
  assert.deepEqual(bad, [], 'test data differs from manifest');
});

// 2. xss: 入り口別の集計と E5 の観察。
test('xss: 入り口別の件数と、E5 を入力欄から送ったときの安全性の計数を記録する', async (t) => {
  const all = loadXssCases();
  const byEntry = {};
  for (const c of all) {
    for (const e of c.entries ?? []) byEntry[e] = (byEntry[e] ?? 0) + 1;
  }
  assert.ok(all.length > 0, 'no cases were read');
  log(t, 'xss', `cases=${all.length} byEntry=${JSON.stringify(byEntry)}`);
  log(t, 'xss', 'E1: pack-body-xss.test.mjs（TASK-144）、E2: structure-xss.test.mjs（TASK-145 1/2）、E3: practice-data-xss.test.mjs（TASK-145 2/2）が確かめる。E6: url-xss.test.mjs（TASK-147 2/2）が確かめる。E4: history-xss.test.mjs（TASK-146 1/2）が置き換えた履歴の断りを確かめる。正規の経路（入力→保存→復元）は TASK-146 2/2（未実装）');

  const payloads = e5Payloads();
  let sent = 0;
  for (const csp of [true, false]) {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openMounted(srv);
      const before = await page.$$eval(RECEIVED, (l) => l.length);
      let n = 0;
      for (const p of payloads) {
        await page.fill(INPUT, p.payload);
        await page.click(SUBMIT);
        await settle(page, 50);
        n += 1;
      }
      await settle(page, 500);
      sent += n;
      const received = await page.$$eval(RECEIVED, (l) => l.length);
      const counts = await collectSafety(page, rec);
      log(t, 'xss', `E5 CSP=${csp ? 'on' : 'off'} sent=${n} received+=${received - before} ${countsLine(counts)}`);
      assertNoPageErrors(rec, `xss csp=${csp}`);
      await context.close();
    }));
  }
  assert.equal(sent, payloads.length * 2, 'E5 send count');
});

// 3. oversize: 過大入力。
test('oversize: IR・履歴の上限の前後の文字列を関門に渡した結果を記録する', async (t) => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openMounted(srv);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.checkJsonLoad), 'function', 'test hooks are missing');
    let calls = 0;
    for (const [entry, max] of [['ir', IR_MAX], ['history', HISTORY_MAX]]) {
      for (const [label, size] of [['exact', max], ['over1', max + 1]]) {
        const r = await page.evaluate(([e, n]) => {
          const text = ' '.repeat(n);
          const t0 = performance.now();
          const out = JSON.parse(window.__fandheTest.checkJsonLoad(e, text));
          return { out, ms: Math.round(performance.now() - t0) };
        }, [entry, size]);
        calls += 1;
        log(t, 'oversize', `${entry} ${label} bytes=${size} ok=${r.out.ok} stage=${r.out.stage ?? '-'} reason=${r.out.reason ?? '-'} ms=${r.ms}`);
      }
    }
    log(t, 'oversize', '10 倍の水準は入れない（CI のメモリと時間）。IR は ir-size-limit.test.mjs（TASK-154）、履歴は TASK-155 で扱う');

    // 入力 1 回: 20,000・20,001 文字。
    for (const n of [20000, 20001]) {
      await page.fill(INPUT, 'あ'.repeat(n));
      const s = JSON.parse(await page.evaluate(() => window.__fandheTest.stateJson()));
      calls += 1;
      log(t, 'oversize', `input chars=${n} draftCodePoints=${s.draftCodePoints} inputOverLimit=${s.inputOverLimit}`);
    }
    assert.ok(calls > 0, 'no oversize call was made');
    assertNoPageErrors(rec, 'oversize');
    await context.close();
  });
});

// 4. nest: 深い入れ子・巨大な配列（generate.sh nested・bigarray と同じ規則）。
test('nest: 10,000 段の入れ子と 1,000,000 要素の配列を関門に渡した結果を記録する', async (t) => {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openMounted(srv);
    let calls = 0;
    const depth = await page.evaluate(() => {
      const text = '['.repeat(10000) + ']'.repeat(10000);
      const t0 = performance.now();
      return { out: JSON.parse(window.__fandheTest.checkJsonDepth(text)), ms: Math.round(performance.now() - t0) };
    });
    calls += 1;
    log(t, 'nest', `checkJsonDepth levels=10000 ok=${depth.out.ok} reason=${depth.out.reason ?? '-'} ms=${depth.ms}`);
    for (const entry of ['ir', 'history']) {
      for (const kind of ['nested', 'bigarray']) {
        const r = await page.evaluate(([e, k]) => {
          const text = k === 'nested' ? '['.repeat(10000) + ']'.repeat(10000) : `[${new Array(1000000).fill('0').join(',')}]`;
          const t0 = performance.now();
          return { out: JSON.parse(window.__fandheTest.checkJsonLoad(e, text)), ms: Math.round(performance.now() - t0) };
        }, [entry, kind]);
        calls += 1;
        log(t, 'nest', `${entry} ${kind} ok=${r.out.ok} stage=${r.out.stage ?? '-'} reason=${r.out.reason ?? '-'} ms=${r.ms}`);
      }
    }
    assert.ok(calls > 0, 'no nest call was made');
    assertNoPageErrors(rec, 'nest');
    await context.close();
  });
});

// 5. cycle: 巡回する IR。
test('cycle: 巡回する IR の 4 ファイルを関門に渡した結果を記録する', async (t) => {
  const dir = path.join(SAFETY_DIR, 'test-data-addendum-3', 'cycle-ir');
  const files = fs.readdirSync(dir).filter((f) => f.endsWith('.json')).sort();
  assert.equal(files.length, 4, 'cycle-ir file count');
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openMounted(srv);
    for (const f of files) {
      const text = fs.readFileSync(path.join(dir, f), 'utf8');
      const out = JSON.parse(await page.evaluate(([e, s]) => window.__fandheTest.checkJsonLoad(e, s), ['ir', text]));
      log(t, 'cycle', `${f} bytes=${Buffer.byteLength(text)} ok=${out.ok} stage=${out.stage ?? '-'} reason=${out.reason ?? '-'}`);
    }
    log(t, 'cycle', '画面での起動口は無い（D-68 で ?ir= は読まない。実行時の教材の取得が入った後の課題）。遷移の上限は transition-limit.test.mjs が確かめる（TASK-160・#444）');
    assertNoPageErrors(rec, 'cycle');
    await context.close();
  });
});

// 6. csp: CSP の配置。
test('csp: index.html の CSP とスクリプトの配置、マウント後の計数を記録する', async (t) => {
  for (const [name, env] of [['dist', 'FANDHE_WEB_DIR'], ['test-hooks', 'FANDHE_WEB_TEST_DIR']]) {
    const html = fs.readFileSync(path.join(process.env[env], 'index.html'), 'utf8');
    const meta = /<meta\b[^>]*http-equiv\s*=\s*["']?content-security-policy["']?[^>]*>/i.exec(html);
    const content = meta ? (/content\s*=\s*"([^"]*)"/i.exec(meta[0])?.[1] ?? '?') : null;
    const scripts = [...html.matchAll(/<script\b([^>]*)>/gi)];
    const external = scripts.filter((m) => /\bsrc\s*=/i.test(m[1])).length;
    log(t, 'csp', `${name}: meta=${content === null ? 'none' : JSON.stringify(content)} inlineScripts=${scripts.length - external} externalScripts=${external}`);
    assert.ok(html.length > 0, `${name}: empty index.html`);
  }
  for (const csp of [true, false]) {
    await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context, rec } = await openMounted(srv);
      const counts = await collectSafety(page, rec);
      log(t, 'csp', `mounted CSP=${csp ? 'on' : 'off'} ${countsLine(counts)}`);
      assertNoPageErrors(rec, `csp=${csp}`);
      await context.close();
    }));
  }
  log(t, 'csp', '配置と内容の検査は csp-placement.test.mjs（TASK-151 1/2）。インライン不使用は TASK-151 2/2（#540）');
});

// 7. secret: 秘密情報らしい文字列。
function expand(token) {
  const seq = /^seq(\d+)$/.exec(token);
  if (seq) return '0123456789abcdefghijklmnopqrstuvwxyz'.slice(0, Number(seq[1]));
  const rep = /^(\d+)×(.)$/u.exec(token);
  if (rep) return rep[2].repeat(Number(rep[1]));
  throw new Error('unknown placeholder');
}
const build = (shape) => shape.parts.join('').replaceAll('\\n', '\n').replace(/\{([^}]+)\}/gu, (_m, tk) => expand(tk));

test('secret: 10 形を入力欄から送ったときの警告の有無を記録する（値は出さない）', async (t) => {
  const file = path.join(SAFETY_DIR, 'test-data-addendum-4', 'secret-shapes.json');
  const { shapes } = JSON.parse(fs.readFileSync(file, 'utf8'));
  assert.equal(shapes.length, 10, 'shape count');
  await withServer({ dir: process.env.FANDHE_WEB_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openMounted(srv);
    let sent = 0;
    let warned = 0;
    for (const s of shapes) {
      await page.fill(INPUT, build(s));
      await page.click(SUBMIT);
      await settle(page);
      // 送信後の画面で数える。形ごとに警告ノードの有無を見て、その形に帰属させる。
      const w = await page.evaluate(() => document.querySelectorAll('[data-testid*="secret" i], [data-testid*="warn" i], [role="alert"]').length);
      sent += 1;
      if (w > 0) warned += 1;
    }
    log(t, 'secret', `shapes sent=${sent} warned=${warned}/${shapes.length}（警告の挙動は TASK-164・#541〜#544 で育てる。未実装なら 0）`);
    assert.equal(sent, 10, 'secret shapes sent');
    assertNoPageErrors(rec, 'secret');
    await context.close();
  });
});
