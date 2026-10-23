// L2 の配布物（zip）を、Rust も node も見えない模擬環境で展開・起動し、自動ブラウザで CH-0〜CH-1 を完走する試験
// （TASK-112 2/2。D-58・D-71・DLV-13。`make test-l2-run` から呼ばれる。`make test-browser` には入れない）。
//
// 目的: 受講者の操作「展開・./start.sh・URL を開く」の 3 つだけで、教材が最後まで動くことを確かめる。
// 模擬環境: `PATH=/usr/bin:/bin` だけで起動する（cargo・rustc・rustup・node が見えない）。環境変数は PATH・HOME・PORT・
// FANDHE_NO_OPEN だけを渡し、親の環境を引き継がない。試験の前提として、この PATH に上の 4 つが無いことを先に確かめる（見つかれば失敗）。
// 1 回ごとに新しい展開先・新しい起動・新しい browser context で行い、3 回すべてが通ることを求める（3/3）。
// 各回の合否: ページ内エラー・CSP の違反・コンソールの error と warning・2xx 以外の応答・配信元以外への要求がどれも 0 件で、
// 同梱の IR（./ir/main.json）が 200 で 1 回取得され、CH-1 の 6 Step を通って `main/st-2-1` に入る。
// 教材は試験データ（crates/web/tests/data/ch0_3/ir.json）で、本物の CH-0〜CH-1 の 50 操作ではない（docs/testing.md 5.239）。
//
// TASK-113（DLV-14。docs/testing.md 5.241b）で足した検査:
//   - 待ち受けが 127.0.0.1 だけであること。LISTEN のソケットの一覧（Linux は /proc/net/tcp*、macOS は /usr/sbin/lsof）と、
//     自分の非 loopback の IPv4 などへの接続が ECONNREFUSED で断られることの 2 方向で確かめる
//     （時間切れ・到達不能は「拒否」と区別して失敗にする。届かないことを拒否と取り違えないため）。
//   - start.sh が開く URL（127.0.0.1 だけ）を表示すること。
//   - 起動（spawn から最初の 200 まで）とマウント（goto から data-mounted まで）の時間を記録する。閾値では判定しない。
//   - 対照: 0.0.0.0 で待ち受ける別のサーバと、start.sh から `-i 127.0.0.1` を外した写しで、同じ判定が
//     「127.0.0.1 以外あり」を検出し、非 loopback の IPv4 から接続できる。
// 限界: 物理的に別のホストからの接続ではなく、同じ機械から自分の非 loopback のアドレスへ接続する模擬である。
//
// 入力（環境変数。未設定・不在は skip にせず失敗にする）:
//   FANDHE_L2_ZIP  tools/build-l2.mjs が作った linux-x64 の zip（scripts/test-l2-run.sh が作る）
//   FANDHE_L2_IR   zip に同梱した IR の JSON（Step の並びを読む）
// 出力に入力した値は載せない（件数・所要時間・合否だけ。非 loopback のアドレスも件数で出す）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, openPage, waitMounted, summarize, cspConsoleMessages,
} from './harness.mjs';
import { ledger } from './ch0-3-journey.mjs';
import { runL2Journey, stepOrder, L2_STOP } from './l2-journey.mjs';
import { SIM_PATH, startL2 as startL2In, stopL2 } from './l2-server.mjs';

const TOOLS = ['cargo', 'rustc', 'rustup', 'node'];
const RUNS = 3;
let browser;
let zip;
let order;
// zip は before で確定するため、共有の起動の手順へ渡す薄い包みにする。
const startL2 = (work, opts) => startL2In(zip, work, opts);
const startTimes = [];
const mountTimes = [];

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

/** /proc/net/tcp の 16 進の 1 アドレス（IPv4 は 8 桁、IPv6 は 32 桁。4 バイトずつ little-endian）を文字列にする。 */
export function decodeProcAddr(hex) {
  const bytes = [];
  for (let w = 0; w < hex.length; w += 8) {
    const word = hex.slice(w, w + 8);
    for (let i = 6; i >= 0; i -= 2) bytes.push(parseInt(word.slice(i, i + 2), 16));
  }
  if (bytes.length === 4) return bytes.join('.');
  if (bytes.every((b) => b === 0)) return '::';
  if (bytes.slice(0, 15).every((b) => b === 0) && bytes[15] === 1) return '::1';
  if (bytes.slice(0, 10).every((b) => b === 0) && bytes[10] === 0xff && bytes[11] === 0xff) return bytes.slice(12).join('.');
  return 'v6:other';
}

/** /proc/net/tcp・tcp6 の本文から、状態が LISTEN（0A）で番号が `port` の行のアドレスを返す。 */
export function listenAddrs(text, port) {
  const out = [];
  for (const line of text.split('\n').slice(1)) {
    const f = line.trim().split(/\s+/);
    if (f.length < 4 || f[3] !== '0A') continue;
    const [addr, p] = f[1].split(':');
    if (parseInt(p, 16) === port) out.push(decodeProcAddr(addr));
  }
  return out;
}

/** `lsof -nP -iTCP:<port> -sTCP:LISTEN` の出力から待ち受けのアドレス（`*` は全アドレス）を返す。 */
export function parseLsof(text) {
  const out = [];
  for (const line of text.split('\n')) {
    const m = /(\S+):\d+ \(LISTEN\)/.exec(line);
    if (m) out.push(m[1].replace(/^\[|\]$/g, ''));
  }
  return out;
}

/** 当のポートで待ち受けているアドレスの集合（重複なし・整列）。未対応の OS は失敗にする（skip にしない）。 */
export function listeningHosts(port) {
  let hosts;
  if (process.platform === 'linux') {
    hosts = ['/proc/net/tcp', '/proc/net/tcp6'].flatMap((f) => listenAddrs(fs.readFileSync(f, 'utf8'), port));
  } else if (process.platform === 'darwin') {
    // シェルを経由せず、引数は配列で渡す。
    const r = spawnSync('/usr/sbin/lsof', ['-nP', `-iTCP:${port}`, '-sTCP:LISTEN'], { encoding: 'utf8' });
    hosts = parseLsof(r.stdout ?? '');
  } else {
    throw new Error('unsupported platform for the listening-socket check');
  }
  return [...new Set(hosts)].sort();
}

/** 自分の非 loopback の IPv4 アドレス。 */
export function nonLoopbackV4() {
  return Object.values(os.networkInterfaces()).flat()
    .filter((a) => a && !a.internal && (a.family === 'IPv4' || a.family === 4))
    .map((a) => a.address);
}

/** `host:port` へ TCP 接続して結果を返す（'connected' / errno の code / 'timeout'）。 */
export function connectOutcome(host, port, ms = 3000) {
  return new Promise((resolve) => {
    const s = net.connect({ host, port });
    const done = (v) => {
      s.destroy();
      resolve(v);
    };
    s.setTimeout(ms, () => done('timeout'));
    s.once('connect', () => done('connected'));
    s.once('error', (e) => done(e.code ?? 'error'));
  });
}

export function median(xs) {
  const a = [...xs].sort((x, y) => x - y);
  return a[Math.floor(a.length / 2)];
}

/** 非 loopback の IPv4（と Linux では 127.0.0.2）への接続の結果。宛先の値は返さず、件数と結果だけにする。 */
async function foreignOutcomes(port) {
  const targets = nonLoopbackV4();
  const nonLoopback = targets.length;
  if (process.platform === 'linux') targets.push('127.0.0.2');
  const results = [];
  for (const h of targets) results.push(await connectOutcome(h, port));
  return { nonLoopback, results };
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

test('復号の対照: /proc/net/tcp と lsof の解析は 0.0.0.0・:: を 127.0.0.1 以外として検出する', () => {
  const head = '  sl  local_address rem_address   st tx_queue rx_queue\n';
  const row = (addr, port, st = '0A') => `   0: ${addr}:${port.toString(16).toUpperCase().padStart(4, '0')} 00000000:0000 ${st} 00000000:00000000 00:00000000 00000000\n`;
  assert.deepEqual(listenAddrs(head + row('0100007F', 8787), 8787), ['127.0.0.1']);
  assert.deepEqual(listenAddrs(head + row('00000000', 8787), 8787), ['0.0.0.0']);
  assert.deepEqual(listenAddrs(head + row('00000000000000000000000000000000', 8787), 8787), ['::']);
  assert.deepEqual(listenAddrs(head + row('00000000000000000000000001000000', 8787), 8787), ['::1']);
  assert.deepEqual(listenAddrs(head + row('0100007F', 9999), 8787), []);
  assert.deepEqual(listenAddrs(head + row('0100007F', 8787, '01'), 8787), []);
  assert.deepEqual(parseLsof('miniserve 1 u 5u IPv4 0x1 0t0 TCP 127.0.0.1:8787 (LISTEN)\n'), ['127.0.0.1']);
  assert.deepEqual(parseLsof('miniserve 1 u 5u IPv4 0x1 0t0 TCP *:8787 (LISTEN)\n'), ['*']);
  assert.deepEqual(parseLsof('miniserve 1 u 5u IPv6 0x1 0t0 TCP [::1]:8787 (LISTEN)\n'), ['::1']);
});

for (let n = 1; n <= RUNS; n += 1) {
  test(`${n}/${RUNS} 回目: 展開・./start.sh・URL を開くだけで CH-0〜CH-1 を完走する`, async () => {
    const t0 = Date.now();
    const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l2-run-'));
    let run = null;
    let context = null;
    try {
      // 人の操作 1・2: 展開・./start.sh。
      run = await startL2(work);
      const { port, url, startMs } = run;
      const L = ledger();
      L.check('起動', 'start.sh が URL を表示した', run.out.includes(url));
      const hosts = run.out.match(/https?:\/\/[^\s/]+/g) ?? [];
      L.check('起動', '表示した URL の host が 127.0.0.1 だけ', hosts.length > 0 && hosts.every((u) => u === `http://127.0.0.1:${port}`));
      L.check('待ち受け', 'LISTEN のソケットが 127.0.0.1 だけ', JSON.stringify(listeningHosts(port)) === '["127.0.0.1"]');
      const foreign = await foreignOutcomes(port);
      L.check('待ち受け', '非 loopback の IPv4 が 1 件以上ある', foreign.nonLoopback >= 1);
      L.check('待ち受け', '他のアドレスへの接続がすべて ECONNREFUSED', foreign.results.length >= 1 && foreign.results.every((r) => r === 'ECONNREFUSED'));
      L.check('待ち受け', '127.0.0.1 へは接続できる', (await connectOutcome('127.0.0.1', port)) === 'connected');

      // 人の操作 3: URL を開く。
      const origin = `http://127.0.0.1:${port}`;
      const opened = await openPage(browser, { origin });
      context = opened.context;
      const { page, rec } = opened;
      const tMount = process.hrtime.bigint();
      await page.goto(url);
      await waitMounted(page);
      const mountMs = Number(process.hrtime.bigint() - tMount) / 1e6;
      L.check('時間', '起動とマウントの時間が有限の正の数', Number.isFinite(startMs) && startMs > 0 && Number.isFinite(mountMs) && mountMs > 0);
      startTimes.push(startMs);
      mountTimes.push(mountMs);
      console.log(`L2 時間 ${n}/${RUNS}: 起動 ${startMs.toFixed(0)} ms（待ち間隔 10 ms）・マウント ${mountMs.toFixed(0)} ms・非 loopback の IPv4 ${foreign.nonLoopback} 件・他のアドレスへの接続 ${foreign.results.length} 件（拒否 ${foreign.results.filter((r) => r === 'ECONNREFUSED').length} 件）`);
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
      await stopL2(run);
      fs.rmSync(work, { recursive: true, force: true });
    }
  });
}

test('起動とマウントの時間の記録（機械の時間。閾値では判定しない）', () => {
  assert.equal(startTimes.length, RUNS, '3 回すべてが通らなかったため中央値を出さない');
  const f = (xs) => xs.map((x) => x.toFixed(0)).join('/');
  console.log(`L2 起動 中央値 ${median(startTimes).toFixed(0)} ms（n=${RUNS}: ${f(startTimes)}）・マウント 中央値 ${median(mountTimes).toFixed(0)} ms（n=${RUNS}: ${f(mountTimes)}）`);
  assert.ok([...startTimes, ...mountTimes].every((x) => Number.isFinite(x) && x > 0));
});

test('対照 A: 0.0.0.0 で待ち受ける別のサーバを同じ判定が検出し、非 loopback の IPv4 から接続できる', async () => {
  const srv = net.createServer((s) => s.destroy());
  await new Promise((r) => srv.listen(0, '0.0.0.0', r));
  try {
    const { port } = srv.address();
    assert.ok(listeningHosts(port).some((h) => h !== '127.0.0.1'), 'a wildcard listener must be detected');
    const targets = nonLoopbackV4();
    assert.ok(targets.length >= 1, 'a non-loopback IPv4 address is required');
    for (const h of targets) assert.equal(await connectOutcome(h, port), 'connected');
  } finally {
    await new Promise((r) => srv.close(r));
  }
});

test('対照 B: start.sh から -i 127.0.0.1 を外すと 127.0.0.1 以外で待ち受け、非 loopback の IPv4 から接続できる', async () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l2-ctlb-'));
  let run = null;
  try {
    let replaced = 0;
    run = await startL2(work, {
      mutate: (t) => {
        replaced = t.split(' -i 127.0.0.1').length - 1;
        return t.replace(' -i 127.0.0.1', '');
      },
    });
    assert.equal(replaced, 1, 'the -i option must appear exactly once in start.sh');
    assert.ok(listeningHosts(run.port).some((h) => h !== '127.0.0.1'), 'the unrestricted server must listen beyond 127.0.0.1');
    const targets = nonLoopbackV4();
    assert.ok(targets.length >= 1, 'a non-loopback IPv4 address is required');
    for (const h of targets) assert.equal(await connectOutcome(h, run.port), 'connected');
  } finally {
    await stopL2(run);
    fs.rmSync(work, { recursive: true, force: true });
  }
});
