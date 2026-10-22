// L2 の配布物（zip）を、Rust も node も見えない模擬環境で展開・起動し、自動ブラウザで CH-0〜CH-1 を完走する試験
// （TASK-112 2/2。D-58・D-71・DLV-13。`make test-l2-run` から呼ばれる。`make test-browser` には入れない）。
//
// 目的: 受講者の操作「展開・./start.sh・URL を開く」の 3 つだけで、教材が最後まで動くことを確かめる。
// 模擬環境: `PATH=/usr/bin:/bin` だけで起動する（cargo・rustc・rustup・node が見えない）。環境変数は PATH・HOME・PORT・
// FANDHE_NO_OPEN だけを渡し、親の環境を引き継がない。試験の前提として、この PATH に上の 4 つが無いことを先に確かめる（見つかれば失敗）。
// 1 回ごとに新しい展開先・新しい起動・新しい browser context で行い、3 回すべてが通ることを求める（3/3）。
// 各回の合否: ページ内エラー・CSP の違反・コンソールの error と warning・2xx 以外の応答・配信元以外への要求がどれも 0 件で、
// 同梱の IR（./ir/main.json）が 200 で 1 回取得され、CH-1 の 6 Step を通って `main/st-2-1` に入る。
// 教材は試験データ（crates/web/tests/data/ch0_3/ir.json）で、本物の CH-0〜CH-1 の 50 操作ではない（docs/testing.md 5.237）。
//
// 入力（環境変数。未設定・不在は skip にせず失敗にする）:
//   FANDHE_L2_ZIP  tools/build-l2.mjs が作った linux-x64 の zip（scripts/test-l2-run.sh が作る）
//   FANDHE_L2_IR   zip に同梱した IR の JSON（Step の並びを読む）
// 出力に入力した値は載せない（件数・所要時間・合否だけ）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, openPage, waitMounted, summarize, cspConsoleMessages,
} from './harness.mjs';
import { ledger } from './ch0-3-journey.mjs';
import { runL2Journey, stepOrder, L2_STOP } from './l2-journey.mjs';

const SIM_PATH = '/usr/bin:/bin';
const TOOLS = ['cargo', 'rustc', 'rustup', 'node'];
const RUNS = 3;
let browser;
let zip;
let order;

/**
 * `PATH=pathValue` だけの環境で `tools` のうち見つかるもの（パス）を返す（模擬環境の前提の検査）。
 * `command -v` は dash だと先頭の 1 件しか見ないため、コマンドごとに 1 回ずつ呼ぶ。
 */
export function visibleTools(pathValue, tools = TOOLS) {
  const found = [];
  for (const t of tools) {
    const r = spawnSync('/bin/sh', ['-c', 'command -v "$1"', 'sh', t], { env: { PATH: pathValue }, encoding: 'utf8' });
    if (r.status === 0 && r.stdout.trim() !== '') found.push(r.stdout.trim());
  }
  return found;
}

function freePort() {
  return new Promise((resolve, reject) => {
    const s = net.createServer();
    s.once('error', reject);
    s.listen(0, '127.0.0.1', () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
  });
}

async function waitOk(url, ms = 20000) {
  const end = Date.now() + ms;
  for (;;) {
    try {
      const r = await fetch(url);
      if (r.status === 200) return;
    } catch {
      // まだ待ち受けていない。
    }
    if (Date.now() > end) throw new Error('server did not become ready');
    await new Promise((r) => setTimeout(r, 100));
  }
}

before(async () => {
  zip = process.env.FANDHE_L2_ZIP;
  const irFile = process.env.FANDHE_L2_IR;
  assert.ok(zip && fs.statSync(zip, { throwIfNoEntry: false })?.isFile(), 'FANDHE_L2_ZIP must point to the L2 zip');
  assert.ok(irFile && fs.statSync(irFile, { throwIfNoEntry: false })?.isFile(), 'FANDHE_L2_IR must point to the IR json');
  order = stepOrder(fs.readFileSync(irFile, 'utf8'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

test('模擬環境の PATH に cargo・rustc・rustup・node が見えない', () => {
  assert.deepEqual(visibleTools(SIM_PATH), []);
});

test('対照: 前提の検査は、見えるコマンドを検出する', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l2-ctl-'));
  try {
    const fake = path.join(dir, 'node');
    fs.writeFileSync(fake, '#!/bin/sh\nexit 0\n', { mode: 0o755 });
    assert.deepEqual(visibleTools(`${dir}:${SIM_PATH}`).map((p) => path.basename(p)), ['node']);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

for (let n = 1; n <= RUNS; n += 1) {
  test(`${n}/${RUNS} 回目: 展開・./start.sh・URL を開くだけで CH-0〜CH-1 を完走する`, async () => {
    const t0 = Date.now();
    const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l2-run-'));
    let child = null;
    let context = null;
    try {
      // 人の操作 1: 展開。
      const un = spawnSync('/usr/bin/unzip', ['-q', zip, '-d', work], { env: { PATH: SIM_PATH }, encoding: 'utf8' });
      assert.equal(un.status, 0, 'unzip failed');
      const top = path.join(work, 'fandhe-workshop-l2');
      // 人の操作 2: ./start.sh。
      const port = await freePort();
      child = spawn('./start.sh', [], {
        cwd: top,
        env: { PATH: SIM_PATH, HOME: work, PORT: String(port), FANDHE_NO_OPEN: '1' },
        stdio: ['ignore', 'pipe', 'pipe'],
        detached: true,
      });
      let out = '';
      child.stdout.on('data', (d) => { out += d; });
      child.stderr.on('data', (d) => { out += d; });
      const url = `http://127.0.0.1:${port}/`;
      await waitOk(url);
      assert.ok(out.includes(url), 'start.sh did not print the URL');

      // 人の操作 3: URL を開く。
      const origin = `http://127.0.0.1:${port}`;
      const opened = await openPage(browser, { origin });
      context = opened.context;
      const { page, rec } = opened;
      const L = ledger();
      await page.goto(url);
      await waitMounted(page);
      await runL2Journey(page, L, { order, scene: `回 ${n}` });
      // 最後の観察（main/st-2-1 の入力待ち）は journey の中で行われている。ここでは取得と計数を確かめる。
      const counts = summarize(rec);
      const irReqs = rec.requests.filter((r) => r.url === `${origin}/ir/main.json`);
      L.check('計数', '同梱の IR が 200 で 1 回取得された', irReqs.length === 1 && irReqs[0].status === 200);
      L.check('計数', '2xx 以外の応答が 0 件', rec.requests.every((r) => r.status >= 200 && r.status < 300));
      L.check('計数', 'ページ内エラーが 0 件', counts.pageErrors === 0);
      L.check('計数', 'CSP の違反が 0 件', counts.csp === 0 && cspConsoleMessages(rec).length === 0);
      L.check('計数', 'コンソールの error が 0 件', counts.consoleErrors === 0);
      L.check('計数', 'コンソールの warning が 0 件', rec.consoleWarnings.length === 0);
      L.check('計数', '配信元以外への要求が 0 件', counts.external === 0 && counts.failed === 0);
      L.check('計数', 'ダイアログ・ポップアップ・クラッシュが 0 件', counts.dialogs === 0 && counts.popups === 0 && counts.crashed === 0);
      console.log(`L2 完走 ${n}/${RUNS}: ${L.summary()} 項目・要求 ${counts.requests} 件・${Date.now() - t0} ms（終点 ${L2_STOP}）`);
      assert.deepEqual(L.failed(), [], JSON.stringify(counts));
    } finally {
      if (context) await context.close().catch(() => {});
      if (child?.pid) {
        try {
          process.kill(-child.pid, 'SIGTERM');
        } catch {
          // すでに終了している。
        }
        await new Promise((r) => {
          if (child.exitCode !== null || child.signalCode !== null) r();
          else child.once('exit', r);
        });
      }
      fs.rmSync(work, { recursive: true, force: true });
    }
  });
}
